//! G1b end-to-end opacity: real TLS terminates after a real provider-verified
//! tunnel and yamux stream. The provider-side capture contains ciphertext only.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use aithos_gateway::config::{RelayCertificateConfig, RelayConfig, RelayReconnectConfig};
use aithos_gateway::core_bridge::gateway_pub_multibase;
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::public_tls::{load_private_pem, public_tls_slot, PublicTlsAcceptor};
use aithos_gateway::relay::{RelayClient, RelayHealth, RelayInputs, RelayReadiness, TUNNEL_ALPN};
use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::MemNonces;
use aithos_provider::passthrough::{
    read_registration_line, spawn_pod_tunnel, SessionRegistry, TunnelSession,
};
use aithos_provider::time::parse_rfc3339z_ms;
use aithos_provider::tunnel::{answer, verify_registration};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::compat::FuturesAsyncReadCompatExt;

const NOW: &str = "2026-07-16T12:00:10Z";
const AT: &str = "2026-07-16T12:00:00Z";
const HOSTNAME: &str = "demo.mcp.aithos.fr";
const TUNNEL_NAME: &str = "relay.test.aithos.fr";

struct FakeRelay {
    addr: std::net::SocketAddr,
    ca: CertificateDer<'static>,
    sessions: mpsc::UnboundedReceiver<Arc<TunnelSession>>,
}

async fn fake_relay(gateway_pub: String) -> FakeRelay {
    let certificate = rcgen::generate_simple_self_signed(vec![TUNNEL_NAME.to_owned()]).unwrap();
    let ca = certificate.cert.der().clone();
    let server_tls = aithos_provider::tls::tunnel_server_config_from_pem(
        certificate.cert.pem().as_bytes(),
        certificate.key_pair.serialize_pem().as_bytes(),
    )
    .unwrap();
    let acceptor = TlsAcceptor::from(server_tls);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut control = ControlPlane::default();
    control.seed_tenant("acme", false);
    control.bind_tunnel(
        gateway_pub,
        TunnelBinding {
            tenant: "acme".into(),
            hostname: HOSTNAME.into(),
            suspended: false,
        },
    );
    let control = Arc::new(control);
    let nonces = Arc::new(MemNonces::new(600));
    let registry = Arc::new(SessionRegistry::new());
    let now_ms = parse_rfc3339z_ms(NOW).unwrap();
    let (session_tx, sessions) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(tcp).await.unwrap();
        assert_eq!(tls.get_ref().1.alpn_protocol(), Some(TUNNEL_ALPN));
        let line = read_registration_line(&mut tls).await.unwrap();
        let verdict = verify_registration(&line, control.as_ref(), nonces.as_ref(), now_ms).await;
        tls.write_all(format!("{}\n", answer(&verdict)).as_bytes())
            .await
            .unwrap();
        tls.flush().await.unwrap();
        let facts = verdict.unwrap();
        let session = spawn_pod_tunnel(registry, facts, tls);
        session_tx.send(session).unwrap();
    });

    FakeRelay { addr, ca, sessions }
}

fn relay_config(addr: std::net::SocketAddr) -> RelayConfig {
    RelayConfig {
        endpoint: format!("https://{addr}"),
        tunnel_name: TUNNEL_NAME.into(),
        tenant: "acme".into(),
        hostname: HOSTNAME.into(),
        cert: RelayCertificateConfig::Pem {
            cert_file: PathBuf::from("public-chain.pem"),
            key_file: PathBuf::from("public-key.pem"),
        },
        reconnect: RelayReconnectConfig {
            base_ms: 1,
            max_ms: 10,
            jitter_percent: 0,
        },
    }
}

fn identity() -> Arc<Keyholder> {
    Arc::new(Keyholder::from_entropy([0x42; 32], [0x51; 32]))
}

async fn read_headers<S>(stream: &mut S) -> std::io::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut one = [0u8; 1];
    while bytes.len() < 8 * 1024 {
        let read = stream.read(&mut one).await?;
        if read == 0 {
            return Ok(Vec::new());
        }
        bytes.push(one[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok(bytes);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "headers too large",
    ))
}

async fn serve_public_stream(acceptor: PublicTlsAcceptor, stream: yamux::Stream) {
    let Ok(mut tls) = acceptor.accept(stream).await else {
        return;
    };
    loop {
        let Ok(headers) = read_headers(&mut tls).await else {
            return;
        };
        if headers.is_empty() {
            return;
        }
        let Ok(text) = std::str::from_utf8(&headers) else {
            return;
        };
        let Some(sentinel) = text
            .lines()
            .find_map(|line| line.strip_prefix("x-sentinel: "))
        else {
            return;
        };
        if sentinel.is_empty() || sentinel.len() > 128 {
            return;
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n{sentinel}",
            sentinel.len()
        );
        if tls.write_all(response.as_bytes()).await.is_err() || tls.flush().await.is_err() {
            return;
        }
    }
}

async fn exchange<S>(stream: &mut S, sentinel: &str) -> String
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let request = format!(
        "GET /control/v1/status HTTP/1.1\r\nhost: {HOSTNAME}\r\nx-sentinel: {sentinel}\r\nconnection: keep-alive\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    let headers = read_headers(stream).await.unwrap();
    let headers = std::str::from_utf8(&headers).unwrap();
    let length: usize = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .unwrap()
        .parse()
        .unwrap();
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).await.unwrap();
    String::from_utf8(body).unwrap()
}

struct RecordingIo<S> {
    inner: S,
    capture: Arc<Mutex<Vec<u8>>>,
}

impl<S> RecordingIo<S> {
    fn new(inner: S, capture: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { inner, capture }
    }
}

impl<S> AsyncRead for RecordingIo<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(context, buffer);
        if matches!(result, Poll::Ready(Ok(()))) {
            self.capture
                .lock()
                .unwrap()
                .extend_from_slice(&buffer.filled()[before..]);
        }
        result
    }
}

impl<S> AsyncWrite for RecordingIo<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write(context, bytes) {
            Poll::Ready(Ok(written)) => {
                self.capture
                    .lock()
                    .unwrap()
                    .extend_from_slice(&bytes[..written]);
                Poll::Ready(Ok(written))
            }
            other => other,
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

fn public_tls() -> (
    tempfile::TempDir,
    PublicTlsAcceptor,
    TlsConnector,
    CertificateDer<'static>,
) {
    use std::os::unix::fs::PermissionsExt as _;

    let certificate = rcgen::generate_simple_self_signed(vec![HOSTNAME.to_owned()]).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let cert_file = temporary.path().join("cert.pem");
    let key_file = temporary.path().join("key.pem");
    std::fs::write(&cert_file, certificate.cert.pem()).unwrap();
    std::fs::write(&key_file, certificate.key_pair.serialize_pem()).unwrap();
    std::fs::set_permissions(&cert_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config = load_private_pem(&cert_file, &key_file, HOSTNAME, UnixTime::now()).unwrap();
    let (_activator, acceptor) = public_tls_slot(config);

    let ca = certificate.cert.der().clone();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.clone()).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    (
        temporary,
        acceptor,
        TlsConnector::from(Arc::new(config)),
        ca,
    )
}

async fn connect_public(
    session: &TunnelSession,
    connector: &TlsConnector,
    capture: Arc<Mutex<Vec<u8>>>,
) -> tokio_rustls::client::TlsStream<RecordingIo<tokio_util::compat::Compat<yamux::Stream>>> {
    let stream = session.open_stream().await.unwrap().compat();
    connector
        .connect(
            ServerName::try_from(HOSTNAME.to_owned()).unwrap(),
            RecordingIo::new(stream, capture),
        )
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn relay_capture_is_opaque_and_public_tls_streams_remain_isolated() {
    let identity = identity();
    let mut relay = fake_relay(gateway_pub_multibase(&identity)).await;
    let client =
        RelayClient::with_root_certificates(relay_config(relay.addr), vec![relay.ca]).unwrap();
    let (_tls_files, acceptor, public_connector, _public_ca) = public_tls();
    let health = RelayHealth::new(RelayReadiness::Disabled);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner = tokio::spawn(async move {
        client
            .run(
                identity,
                RelayInputs {
                    clock: Arc::new(|| AT.into()),
                    nonce: Arc::new(|| "g1b-tunnel-nonce-0001".into()),
                    jitter: Arc::new(|| 0),
                },
                health,
                shutdown_rx,
                move |stream| serve_public_stream(acceptor.clone(), stream),
            )
            .await
            .unwrap();
    });
    let session = tokio::time::timeout(Duration::from_secs(2), relay.sessions.recv())
        .await
        .unwrap()
        .unwrap();

    let capture_a = Arc::new(Mutex::new(Vec::new()));
    let capture_b = Arc::new(Mutex::new(Vec::new()));
    let (mut first, mut second) = tokio::join!(
        connect_public(&session, &public_connector, Arc::clone(&capture_a)),
        connect_public(&session, &public_connector, Arc::clone(&capture_b)),
    );
    let sentinel_a = "G1B-SENTINEL-ALPHA-8f71";
    let sentinel_b = "G1B-SENTINEL-BRAVO-3c92";
    let (answer_a, answer_b) = tokio::join!(
        exchange(&mut first, sentinel_a),
        exchange(&mut second, sentinel_b),
    );
    assert_eq!(answer_a, sentinel_a);
    assert_eq!(answer_b, sentinel_b);

    drop(first);
    let capture_c = Arc::new(Mutex::new(Vec::new()));
    let mut third = connect_public(&session, &public_connector, Arc::clone(&capture_c)).await;
    let sentinel_b2 = "G1B-SENTINEL-BRAVO-STILL-2a18";
    let sentinel_c = "G1B-SENTINEL-CHARLIE-1d04";
    let (answer_b2, answer_c) = tokio::join!(
        exchange(&mut second, sentinel_b2),
        exchange(&mut third, sentinel_c),
    );
    assert_eq!(answer_b2, sentinel_b2);
    assert_eq!(answer_c, sentinel_c);

    let captures = [capture_a, capture_b, capture_c];
    for capture in captures {
        let bytes = capture.lock().unwrap();
        for sentinel in [sentinel_a, sentinel_b, sentinel_b2, sentinel_c] {
            assert!(!bytes
                .windows(sentinel.len())
                .any(|window| window == sentinel.as_bytes()));
        }
    }

    drop(second);
    drop(third);
    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), runner)
        .await
        .unwrap()
        .unwrap();
}
