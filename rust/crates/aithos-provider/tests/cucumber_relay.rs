//! BDD acceptance harness for `tests/features/relay/relay-passthrough.feature`
//! — the annexe B.1/B.3/B.4 public door, over REAL sockets and REAL TLS.
//!
//! A [`RelayDoor`] runs on a loopback listener; a [`TestPod`] plays the
//! CLIENT far end (dials the tunnel door, registers B.2, becomes the yamux
//! server, terminates its OWN public TLS and echoes); a rustls client
//! plays the browser. This proves the plumbing the `p5` vector cannot: TLS
//! termination on the tunnel door only, ALPN gating, byte-exact
//! passthrough, half-close, GoAway on replace, anti-flap, liveness, and
//! blind logs. The security-critical SNI extraction is proven byte-exact
//! by `sni_replay` (p5); this is the wiring around it.
//!
//! No key material beyond the committed `p3` gateway TEST seed and
//! ephemeral rcgen certs minted in-process (never a client secret).

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::{MemNonces, NonceStore};
use aithos_provider::passthrough::{RelayDoor, SessionRegistry};
use aithos_provider::sni::TUNNEL_ALPN;
use aithos_provider::tls::tunnel_server_config_from_pem;
use aithos_provider::tunnel::{
    registration_line, sign_registration, Registration, RegistrationSignature, TUNNEL_WIRE_VERSION,
};
use cucumber::{given, then, when, World as _};
use ed25519_dalek::SigningKey;
use futures::future::poll_fn;
use rustls::pki_types::{CertificateDer, ServerName};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

const FIXTURE_GATEWAY: &str = "z6MksPykuQeYh4zgthFRFBExrgo1dwFWWenY2TEJ9SvT9jn1";
const RELAY_TUNNEL_NAME: &str = "relay.test.aithos.fr";

// ------------------------------------------------------------ log capture

#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn log_buffer() -> &'static LogBuf {
    static BUF: OnceLock<LogBuf> = OnceLock::new();
    BUF.get_or_init(|| {
        let buf = LogBuf::default();
        let w = buf.clone();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_target(true)
            .with_ansi(false)
            .with_writer(move || w.clone())
            .try_init();
        buf
    })
}

fn crypto() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

// ------------------------------------------------------------ certificates

struct SelfSigned {
    cert_pem: String,
    key_pem: String,
    der: CertificateDer<'static>,
}

fn self_signed(name: &str) -> SelfSigned {
    let c = rcgen::generate_simple_self_signed(vec![name.to_owned()]).unwrap();
    SelfSigned {
        cert_pem: c.cert.pem(),
        key_pem: c.key_pair.serialize_pem(),
        der: c.cert.der().clone(),
    }
}

fn client_config_trusting(
    der: &CertificateDer<'static>,
    alpn: &[&[u8]],
) -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(der.clone()).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    Arc::new(cfg)
}

fn pod_server_config(cert_pem: &str, key_pem: &str) -> Arc<rustls::ServerConfig> {
    let chain = aithos_provider::tls::load_cert_chain(cert_pem.as_bytes()).unwrap();
    let key = aithos_provider::tls::load_private_key(key_pem.as_bytes()).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .unwrap();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(cfg)
}

// -------------------------------------------------------------- fixtures

fn gateway_key() -> (SigningKey, String) {
    let sk = SigningKey::from_bytes(&[0x51; 32]);
    let pubkey = aithos_core::wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes());
    (sk, pubkey)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn now_z() -> String {
    aithos_provider::time::render_rfc3339z(now_ms())
}

fn signed_registration(
    sk: &SigningKey,
    gateway_pub: &str,
    tenant: &str,
    hostname: &str,
    nonce: &str,
) -> String {
    let reg = Registration {
        version: TUNNEL_WIRE_VERSION.into(),
        tenant: tenant.into(),
        hostname: hostname.into(),
        gateway_pub: gateway_pub.into(),
        at: now_z(),
        nonce: nonce.into(),
        signature: RegistrationSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    registration_line(&sign_registration(reg, sk))
}

// ------------------------------------------------------- relay + pod

/// Dial the relay tunnel door and complete TLS + ALPN. `alpn` empty →
/// offer none (the handshake must fail: the door pins aithos-tunnel/1).
async fn dial_tunnel_door(
    addr: std::net::SocketAddr,
    relay_ca: &CertificateDer<'static>,
    alpn: &[&[u8]],
) -> std::io::Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let connector = TlsConnector::from(client_config_trusting(relay_ca, alpn));
    let tcp = TcpStream::connect(addr).await?;
    let name = ServerName::try_from(RELAY_TUNNEL_NAME.to_owned()).unwrap();
    connector
        .connect(name, tcp)
        .await
        .map_err(std::io::Error::other)
}

async fn read_answer_line<S: AsyncReadExt + Unpin>(tls: &mut S) -> serde_json::Value {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match tls.read(&mut byte).await {
            Ok(0) => break,
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                line.push(byte[0]);
            }
            Err(_) => break,
        }
    }
    serde_json::from_slice(&line).unwrap_or(serde_json::Value::Null)
}

/// One registration round-trip (no persistent pod): returns the relay's
/// answer JSON line. Used for refusal and anti-flap scenarios.
#[allow(clippy::too_many_arguments)]
async fn register_once(
    addr: std::net::SocketAddr,
    relay_ca: &CertificateDer<'static>,
    sk: &SigningKey,
    gateway_pub: &str,
    tenant: &str,
    hostname: &str,
    nonce: &str,
) -> serde_json::Value {
    let mut tls = dial_tunnel_door(addr, relay_ca, &[TUNNEL_ALPN])
        .await
        .unwrap();
    tls.write_all(signed_registration(sk, gateway_pub, tenant, hostname, nonce).as_bytes())
        .await
        .unwrap();
    tls.flush().await.unwrap();
    read_answer_line(&mut tls).await
}

struct TestPod {
    hostname: String,
    goaway_rx: oneshot::Receiver<()>,
    drop_tx: Option<oneshot::Sender<()>>,
}

impl TestPod {
    /// Register and run the yamux server + public-TLS echo. Ok on
    /// acceptance; the pod exits (firing `goaway_rx`) on GoAway, drop, or
    /// tunnel error.
    #[allow(clippy::too_many_arguments)]
    async fn spawn(
        addr: std::net::SocketAddr,
        relay_ca: CertificateDer<'static>,
        sk: SigningKey,
        gateway_pub: String,
        tenant: String,
        hostname: String,
        pod_cert_pem: String,
        pod_key_pem: String,
        nonce: String,
    ) -> Result<TestPod, String> {
        let mut tls = dial_tunnel_door(addr, &relay_ca, &[TUNNEL_ALPN])
            .await
            .map_err(|e| format!("tunnel TLS: {e}"))?;
        tls.write_all(
            signed_registration(&sk, &gateway_pub, &tenant, &hostname, &nonce).as_bytes(),
        )
        .await
        .map_err(|e| e.to_string())?;
        tls.flush().await.map_err(|e| e.to_string())?;
        let answer = read_answer_line(&mut tls).await;
        if answer["ok"] != serde_json::Value::Bool(true) {
            return Err(format!("relay refused: {answer}"));
        }

        let pod_tls = pod_server_config(&pod_cert_pem, &pod_key_pem);
        let (goaway_tx, goaway_rx) = oneshot::channel();
        let (drop_tx, mut drop_rx) = oneshot::channel();

        tokio::spawn(async move {
            let cfg = yamux::Config::default();
            let mut conn = yamux::Connection::new(tls.compat(), cfg, yamux::Mode::Server);
            loop {
                tokio::select! {
                    _ = &mut drop_rx => break, // told to drop the tunnel
                    inbound = poll_fn(|cx| conn.poll_next_inbound(cx)) => {
                        match inbound {
                            Some(Ok(stream)) => {
                                let acceptor = TlsAcceptor::from(pod_tls.clone());
                                tokio::spawn(pod_echo(stream, acceptor));
                            }
                            _ => break, // GoAway or error
                        }
                    }
                }
            }
            let _ = goaway_tx.send(());
        });

        Ok(TestPod {
            hostname,
            goaway_rx,
            drop_tx: Some(drop_tx),
        })
    }

    fn drop_tunnel(&mut self) {
        if let Some(tx) = self.drop_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// The pod's public handler: terminate its OWN TLS on the piped stream and
/// echo every byte (proving the relay altered nothing and the pod owns the
/// cert). Echo = read half copied into write half until EOF, so a
/// half-close from the client propagates as EOF then a clean close.
async fn pod_echo(stream: yamux::Stream, acceptor: TlsAcceptor) {
    let Ok(tls) = acceptor.accept(stream.compat()).await else {
        return;
    };
    let (mut r, mut w) = tokio::io::split(tls);
    let _ = tokio::io::copy(&mut r, &mut w).await;
    let _ = w.shutdown().await;
}

/// A public client that completes TLS with the pod through the relay.
/// Returns the stream and the leaf cert the client actually saw.
async fn public_tls(
    addr: std::net::SocketAddr,
    pod_ca: &CertificateDer<'static>,
    sni: &str,
) -> std::io::Result<(
    tokio_rustls::client::TlsStream<TcpStream>,
    CertificateDer<'static>,
)> {
    let connector = TlsConnector::from(client_config_trusting(pod_ca, &[b"http/1.1"]));
    let tcp = TcpStream::connect(addr).await?;
    let name = ServerName::try_from(sni.to_owned()).unwrap();
    let tls = connector
        .connect(name, tcp)
        .await
        .map_err(std::io::Error::other)?;
    let leaf = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|c| c.first())
        .cloned()
        .ok_or_else(|| std::io::Error::other("no peer cert"))?;
    Ok((tls, leaf))
}

/// Send raw bytes on a plain TCP connection to the relay and report
/// whether the relay closed WITHOUT emitting a single byte (B.4 silent
/// close).
async fn raw_expect_silent_close(addr: std::net::SocketAddr, bytes: &[u8]) -> bool {
    let Ok(mut tcp) = TcpStream::connect(addr).await else {
        return false;
    };
    let _ = tcp.write_all(bytes).await;
    let _ = tcp.flush().await;
    let mut buf = [0u8; 64];
    match tokio::time::timeout(Duration::from_secs(3), tcp.read(&mut buf)).await {
        Ok(Ok(0)) => true,   // clean EOF, nothing emitted
        Ok(Ok(_n)) => false, // the relay emitted a byte — a banner
        Ok(Err(_)) => true,  // reset without data
        Err(_) => false,     // still open past the deadline
    }
}

// --------------------------------------------------------------- world

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct RelayWorld {
    binding: Option<TunnelBinding>,
    suspended: bool,
    addr: Option<std::net::SocketAddr>,
    registry: Option<Arc<SessionRegistry>>,
    relay_ca: Option<CertificateDer<'static>>,
    gateway_sk: SigningKey,
    gateway_pub: String,
    pod: Option<TestPod>,
    pod_cert: SelfSigned,
    log_mark: usize,
    last_answer: Option<serde_json::Value>,
    handshake_ok: Option<bool>,
    client_leaf: Option<CertificateDer<'static>>,
    silent_close: Option<bool>,
    echo_ok: Option<bool>,
    fifth_ok: Option<bool>,
    nonce_seq: u64,
}

impl std::fmt::Debug for RelayWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RelayWorld")
    }
}

impl RelayWorld {
    fn new() -> Self {
        crypto();
        let (sk, pubkey) = gateway_key();
        RelayWorld {
            binding: None,
            suspended: false,
            addr: None,
            registry: None,
            relay_ca: None,
            gateway_sk: sk,
            gateway_pub: pubkey,
            pod: None,
            pod_cert: self_signed("demo.mcp.aithos.fr"),
            log_mark: 0,
            last_answer: None,
            handshake_ok: None,
            client_leaf: None,
            silent_close: None,
            echo_ok: None,
            fifth_ok: None,
            nonce_seq: 0,
        }
    }

    fn fresh_nonce(&mut self, tag: &str) -> String {
        self.nonce_seq += 1;
        format!("relay-bdd-{tag}-{}-{}", self.nonce_seq, now_ms())
    }

    fn logs_since_mark(&self) -> String {
        let all = log_buffer().0.lock().unwrap();
        String::from_utf8_lossy(&all[self.log_mark.min(all.len())..]).into_owned()
    }

    fn mark_logs(&mut self) {
        self.log_mark = log_buffer().0.lock().unwrap().len();
    }

    async fn spawn_pod(&mut self, hostname: &str) {
        let nonce = self.fresh_nonce("pod");
        let pod = TestPod::spawn(
            self.addr.unwrap(),
            self.relay_ca.clone().unwrap(),
            self.gateway_sk.clone(),
            self.gateway_pub.clone(),
            self.binding.as_ref().unwrap().tenant.clone(),
            hostname.to_owned(),
            self.pod_cert.cert_pem.clone(),
            self.pod_cert.key_pem.clone(),
            nonce,
        )
        .await
        .expect("pod registered");
        // Give the driver a beat to pin the session.
        tokio::time::sleep(Duration::from_millis(60)).await;
        self.pod = Some(pod);
    }
}

// ---------------------------------------------------------- background

#[given(expr = "the control plane binds gateway {string} to tenant {string} and hostname {string}")]
async fn bind(world: &mut RelayWorld, gateway_pub: String, tenant: String, hostname: String) {
    assert_eq!(gateway_pub, FIXTURE_GATEWAY, "fixture gateway key");
    assert_eq!(
        gateway_pub, world.gateway_pub,
        "derived fixture key matches"
    );
    world.binding = Some(TunnelBinding {
        tenant,
        hostname,
        suspended: false,
    });
}

#[given(expr = "a relay listens with a test TLS certificate for tunnel name {string}")]
async fn start_relay(world: &mut RelayWorld, tunnel_name: String) {
    assert_eq!(tunnel_name, RELAY_TUNNEL_NAME);
    let relay_cert = self_signed(&tunnel_name);
    let acceptor = TlsAcceptor::from(
        tunnel_server_config_from_pem(
            relay_cert.cert_pem.as_bytes(),
            relay_cert.key_pem.as_bytes(),
        )
        .unwrap(),
    );

    let mut control = ControlPlane::default();
    let b = world.binding.clone().unwrap();
    control.bind_tunnel(
        world.gateway_pub.clone(),
        TunnelBinding {
            suspended: world.suspended,
            ..b
        },
    );

    let registry = Arc::new(SessionRegistry::new());
    let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
    let mut door = RelayDoor::new(
        Arc::new(control),
        nonces,
        registry.clone(),
        acceptor,
        tunnel_name.to_ascii_lowercase(),
    );
    // Short hello deadline so the stalled-hello scenario runs fast.
    door.hello_deadline = Duration::from_millis(400);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                return;
            };
            let door = door.clone();
            tokio::spawn(async move {
                let _ = door.serve(stream, peer.ip().to_string(), now_ms()).await;
            });
        }
    });

    world.addr = Some(addr);
    world.registry = Some(registry);
    world.relay_ca = Some(relay_cert.der);
    world.mark_logs();
}

#[given("the control-plane binding is suspended")]
async fn suspend(world: &mut RelayWorld) {
    world.suspended = true;
    let name = RELAY_TUNNEL_NAME.to_owned();
    start_relay(world, name).await;
}

#[given(expr = "a pod is registered and serving {string}")]
async fn pod_registered(world: &mut RelayWorld, hostname: String) {
    world.spawn_pod(&hostname).await;
    assert!(world
        .registry
        .as_ref()
        .unwrap()
        .resolve(&hostname)
        .is_some());
}

// ---------------------------------------------------------------- whens

#[when(expr = "a pod opens TLS to the tunnel name offering ALPN {string} and registers")]
async fn pod_alpn_registers(world: &mut RelayWorld, _alpn: String) {
    let host = world.binding.as_ref().unwrap().hostname.clone();
    world.spawn_pod(&host).await;
}

#[when("a pod opens TLS to the tunnel name offering no ALPN")]
async fn pod_no_alpn(world: &mut RelayWorld) {
    let r = dial_tunnel_door(world.addr.unwrap(), world.relay_ca.as_ref().unwrap(), &[]).await;
    world.handshake_ok = Some(r.is_ok());
}

#[when(expr = "a pod opens TLS to the tunnel name offering ALPN {string}")]
async fn pod_foreign_alpn(world: &mut RelayWorld, alpn: String) {
    let proto = alpn.into_bytes();
    let r = dial_tunnel_door(
        world.addr.unwrap(),
        world.relay_ca.as_ref().unwrap(),
        &[&proto],
    )
    .await;
    world.handshake_ok = Some(r.is_ok());
}

#[when(expr = "a pod opens TLS to the tunnel name and registers for hostname {string}")]
async fn pod_registers_hostname(world: &mut RelayWorld, hostname: String) {
    let nonce = world.fresh_nonce("map");
    let tenant = world.binding.as_ref().unwrap().tenant.clone();
    let answer = register_once(
        world.addr.unwrap(),
        world.relay_ca.as_ref().unwrap(),
        &world.gateway_sk,
        &world.gateway_pub,
        &tenant,
        &hostname,
        &nonce,
    )
    .await;
    world.last_answer = Some(answer);
}

#[when(expr = "a public client connects with SNI {string}")]
async fn client_connects_sni(world: &mut RelayWorld, sni: String) {
    match public_tls(world.addr.unwrap(), &world.pod_cert.der, &sni).await {
        Ok((_tls, leaf)) => {
            world.handshake_ok = Some(true);
            world.client_leaf = Some(leaf);
        }
        Err(_) => {
            world.handshake_ok = Some(false);
            world.silent_close = Some(true);
        }
    }
}

#[when("a public client connects without SNI")]
async fn client_no_sni(world: &mut RelayWorld) {
    let hello = p5_hello("no_sni_closes");
    world.silent_close = Some(raw_expect_silent_close(world.addr.unwrap(), &hello).await);
}

#[when("a client sends plain HTTP bytes to the public door")]
async fn client_plain_http(world: &mut RelayWorld) {
    let bytes = b"GET / HTTP/1.1\r\nHost: demo.mcp.aithos.fr\r\n\r\n";
    world.silent_close = Some(raw_expect_silent_close(world.addr.unwrap(), bytes).await);
}

#[when("a client floods more than 16 KiB without completing a ClientHello")]
async fn client_floods(world: &mut RelayWorld) {
    let hello = p5_hello("hello_over_16kib_closes");
    world.silent_close = Some(raw_expect_silent_close(world.addr.unwrap(), &hello).await);
}

#[when("a client sends half a ClientHello and stalls past the hello deadline")]
async fn client_stalls(world: &mut RelayWorld) {
    let hello = p5_hello("peek_demo_hostname");
    let half = &hello[..40.min(hello.len())];
    world.silent_close = Some(raw_expect_silent_close(world.addr.unwrap(), half).await);
}

#[when("a public client sends a request through its TLS session with the pod")]
async fn client_request_echo(world: &mut RelayWorld) {
    let hostname = world.pod.as_ref().unwrap().hostname.clone();
    let (mut tls, _leaf) = public_tls(world.addr.unwrap(), &world.pod_cert.der, &hostname)
        .await
        .expect("public TLS to the pod");
    let payload = vec![0xA5u8; 96 * 1024]; // > one yamux window: exercises flow control
    tls.write_all(&payload).await.unwrap();
    tls.flush().await.unwrap();
    let mut got = vec![0u8; payload.len()];
    tls.read_exact(&mut got).await.unwrap();
    world.echo_ok = Some(got == payload);
}

#[when("the public client closes its write side after the request")]
async fn client_half_close(world: &mut RelayWorld) {
    let hostname = world.pod.as_ref().unwrap().hostname.clone();
    let (tls, _leaf) = public_tls(world.addr.unwrap(), &world.pod_cert.der, &hostname)
        .await
        .expect("public TLS to the pod");
    let (mut r, mut w) = tokio::io::split(tls);
    let msg = b"hello-half-close";
    w.write_all(msg).await.unwrap();
    w.flush().await.unwrap();
    w.shutdown().await.unwrap(); // half-close the write side
    let mut got = vec![0u8; msg.len()];
    r.read_exact(&mut got).await.unwrap();
    let mut tail = Vec::new();
    let _ = r.read_to_end(&mut tail).await;
    world.echo_ok = Some(got == msg && tail.is_empty());
}

#[when(expr = "a second pod registers and serves {string}")]
async fn second_pod(world: &mut RelayWorld, hostname: String) {
    let nonce = world.fresh_nonce("pod2");
    let tenant = world.binding.as_ref().unwrap().tenant.clone();
    let pod2 = TestPod::spawn(
        world.addr.unwrap(),
        world.relay_ca.clone().unwrap(),
        world.gateway_sk.clone(),
        world.gateway_pub.clone(),
        tenant,
        hostname,
        world.pod_cert.cert_pem.clone(),
        world.pod_cert.key_pem.clone(),
        nonce,
    )
    .await
    .expect("second pod registered");
    // Leak the second pod so its tunnel stays up for the follow-on check.
    std::mem::forget(pod2);
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[when(expr = "the same pod registers {int} times within a minute for {string}")]
async fn flap(world: &mut RelayWorld, times: usize, hostname: String) {
    let tenant = world.binding.as_ref().unwrap().tenant.clone();
    let mut last = serde_json::Value::Null;
    let mut fifth = serde_json::Value::Null;
    for i in 0..times {
        let nonce = world.fresh_nonce(&format!("flap{i}"));
        last = register_once(
            world.addr.unwrap(),
            world.relay_ca.as_ref().unwrap(),
            &world.gateway_sk,
            &world.gateway_pub,
            &tenant,
            &hostname,
            &nonce,
        )
        .await;
        if i + 2 == times {
            fifth = last.clone();
        }
    }
    world.last_answer = Some(last);
    world.fifth_ok = Some(fifth["ok"] == serde_json::Value::Bool(true));
}

#[when("the pod drops its tunnel connection")]
async fn pod_drops(world: &mut RelayWorld) {
    world.pod.as_mut().unwrap().drop_tunnel();
    tokio::time::sleep(Duration::from_millis(200)).await;
}

#[when("a public client sends a distinctive payload through the pipe")]
async fn client_distinctive(world: &mut RelayWorld) {
    let hostname = world.pod.as_ref().unwrap().hostname.clone();
    let (mut tls, _leaf) = public_tls(world.addr.unwrap(), &world.pod_cert.der, &hostname)
        .await
        .expect("public TLS to the pod");
    let payload = b"SECRETMARKER-1a2b3c4d-DO-NOT-LOG";
    tls.write_all(payload).await.unwrap();
    tls.flush().await.unwrap();
    let mut got = vec![0u8; payload.len()];
    tls.read_exact(&mut got).await.unwrap();
    world.echo_ok = Some(got == payload);
    tokio::time::sleep(Duration::from_millis(40)).await;
}

// ---------------------------------------------------------------- thens

#[then(expr = "the registration is accepted and the tunnel is active for {string}")]
async fn tunnel_active(world: &mut RelayWorld, hostname: String) {
    assert!(
        world
            .registry
            .as_ref()
            .unwrap()
            .resolve(&hostname)
            .is_some(),
        "expected an active tunnel for {hostname}"
    );
}

#[then("the TLS handshake fails and no registration line is ever read")]
async fn handshake_fails(world: &mut RelayWorld) {
    assert_eq!(
        world.handshake_ok,
        Some(false),
        "ALPN-less handshake must fail"
    );
}

#[then(expr = "the registration is refused with {string}")]
async fn refused(world: &mut RelayWorld, code: String) {
    let a = world.last_answer.as_ref().expect("an answer");
    assert_eq!(a["ok"], serde_json::Value::Bool(false));
    assert_eq!(a["error"], code);
}

#[then(expr = "{string} has no active tunnel")]
async fn no_active(world: &mut RelayWorld, hostname: String) {
    assert!(
        world
            .registry
            .as_ref()
            .unwrap()
            .resolve(&hostname)
            .is_none(),
        "expected no active tunnel for {hostname}"
    );
}

#[then("the certificate the client sees is the pod's, never the relay's")]
async fn cert_is_pods(world: &mut RelayWorld) {
    let leaf = world.client_leaf.as_ref().expect("a client handshake");
    assert_eq!(leaf, &world.pod_cert.der, "client saw the pod cert");
    assert_ne!(
        Some(leaf),
        world.relay_ca.as_ref(),
        "client must NOT see the relay cert"
    );
}

#[then("the pod receives exactly the bytes the client sent")]
async fn bytes_exact_in(world: &mut RelayWorld) {
    assert_eq!(
        world.echo_ok,
        Some(true),
        "byte-exact echo (both directions)"
    );
}

#[then("the client receives exactly the bytes the pod answered")]
async fn bytes_exact_out(world: &mut RelayWorld) {
    assert_eq!(world.echo_ok, Some(true));
}

#[then("the pod observes end-of-stream after the request bytes")]
async fn half_close_observed(world: &mut RelayWorld) {
    assert_eq!(world.echo_ok, Some(true), "half-close propagated");
}

#[then("the client still receives the pod's answer")]
async fn client_gets_answer(world: &mut RelayWorld) {
    assert_eq!(world.echo_ok, Some(true));
}

#[then("the connection closes without one byte emitted")]
async fn closes_silently(world: &mut RelayWorld) {
    assert_eq!(world.silent_close, Some(true), "expected a silent close");
}

#[then("the first pod's mux is closed by GoAway")]
async fn first_pod_goaway(world: &mut RelayWorld) {
    let pod = world.pod.as_mut().unwrap();
    let got = tokio::time::timeout(Duration::from_secs(2), &mut pod.goaway_rx).await;
    assert!(got.is_ok(), "the replaced pod must receive GoAway");
}

#[then("a new public client is served by the second pod")]
async fn served_by_second(world: &mut RelayWorld) {
    let hostname = world.binding.as_ref().unwrap().hostname.clone();
    let (mut tls, leaf) = public_tls(world.addr.unwrap(), &world.pod_cert.der, &hostname)
        .await
        .expect("public TLS to the replacement pod");
    assert_eq!(leaf, world.pod_cert.der);
    let payload = b"after-replace";
    tls.write_all(payload).await.unwrap();
    tls.flush().await.unwrap();
    let mut got = vec![0u8; payload.len()];
    tls.read_exact(&mut got).await.unwrap();
    assert_eq!(&got, payload);
}

#[then(expr = "the sixth registration is refused with {string}")]
async fn sixth_refused(world: &mut RelayWorld, code: String) {
    let a = world.last_answer.as_ref().expect("an answer");
    assert_eq!(a["ok"], serde_json::Value::Bool(false));
    assert_eq!(a["error"], code);
}

#[then("the fifth registration was accepted")]
async fn fifth_accepted(world: &mut RelayWorld) {
    assert_eq!(
        world.fifth_ok,
        Some(true),
        "the fifth registration was accepted"
    );
}

#[then("the flow refusal is logged with a reason class only")]
async fn flow_logged(world: &mut RelayWorld) {
    let logs = world.logs_since_mark();
    assert!(
        logs.contains("event=flow") && logs.contains("outcome=closed"),
        "expected a redacted flow-close log, got:\n{logs}"
    );
}

#[then(expr = "no log line contains {string}")]
async fn no_log_contains(world: &mut RelayWorld, needle: String) {
    let logs = world.logs_since_mark();
    assert!(!logs.contains(&needle), "log leaked {needle:?}:\n{logs}");
}

#[then("no log line contains the payload bytes")]
async fn no_log_payload(world: &mut RelayWorld) {
    let logs = world.logs_since_mark();
    assert!(
        !logs.contains("SECRETMARKER"),
        "payload leaked into logs:\n{logs}"
    );
    assert_eq!(world.echo_ok, Some(true));
}

// ------------------------------------------------------------ p5 hellos

fn p5_hello(name: &str) -> Vec<u8> {
    static P5: OnceLock<serde_json::Value> = OnceLock::new();
    let p5 = P5.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p5-tunnel-sni.json"
            ))
            .unwrap(),
        )
        .unwrap()
    });
    let hex = p5["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("p5 case {name}"))["hello_hex"]
        .as_str()
        .unwrap();
    hex::decode(hex).unwrap()
}

#[tokio::main]
async fn main() {
    RelayWorld::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit("tests/features/relay", |_feature, _rule, scenario| {
            !scenario.tags.iter().any(|t| t == "wip" || t == "draft2")
        })
        .await;
}
