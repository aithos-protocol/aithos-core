//! G1b end-to-end opacity: real TLS terminates after a real provider-verified
//! tunnel and yamux stream. The provider-side capture contains ciphertext only.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use aithos_gateway::config::{
    GatewayConfig, RelayCertificateConfig, RelayConfig, RelayReconnectConfig,
};
use aithos_gateway::core_bridge::gateway_pub_multibase;
use aithos_gateway::credentials::{CredentialBroker, CredentialRef, SecretValue};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::proxy_mcp::{HttpUpstream, Upstream};
use aithos_gateway::public_tls::{load_private_pem, public_tls_slot, PublicTlsAcceptor};
use aithos_gateway::relay::{RelayClient, RelayHealth, RelayInputs, RelayReadiness, TUNNEL_ALPN};
use aithos_gateway::relay_application::relay_application_channel;
use aithos_gateway::upstream_oauth::{self, UpstreamOAuthRegistry};
use aithos_gateway::{GatewayError, Result};
use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::MemNonces;
use aithos_provider::passthrough::{
    read_registration_line, spawn_pod_tunnel, SessionRegistry, TunnelSession,
};
use aithos_provider::time::parse_rfc3339z_ms;
use aithos_provider::tunnel::{answer, verify_registration};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use serde_json::{json, Value};
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

#[derive(Default)]
struct OAuthVault {
    values: Mutex<BTreeMap<(String, String), String>>,
}

impl OAuthVault {
    fn put(&self, path: &str, field: &str, value: &str) {
        self.values
            .lock()
            .unwrap()
            .insert((path.into(), field.into()), value.into());
    }

    fn get(&self, path: &str, field: &str) -> Option<String> {
        self.values
            .lock()
            .unwrap()
            .get(&(path.into(), field.into()))
            .cloned()
    }
}

impl CredentialBroker for OAuthVault {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(async move {
            self.get(&reference.path, &reference.field)
                .map(SecretValue::new)
                .ok_or_else(|| {
                    GatewayError::CredentialUnavailable("test Vault field absent".into())
                })
        })
    }

    fn store<'a>(
        &'a self,
        reference: &'a CredentialRef,
        value: SecretValue,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.put(&reference.path, &reference.field, value.expose());
            Ok(())
        })
    }
}

const OAUTH_CLIENT_SECRET: &str = "g1c-client-secret-sentinel";
const OAUTH_ACCESS_TOKEN: &str = "g1c-access-token-sentinel";
const OAUTH_REFRESH_TOKEN: &str = "g1c-refresh-token-sentinel";
const OAUTH_TOKEN_PATH: &str = "aithos/oauth/protected";
const OAUTH_TOKEN_FIELD: &str = "state";

async fn fake_protected_mcp() -> (
    String,
    Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    Arc<Mutex<Vec<Option<String>>>>,
) {
    use axum::extract::{Form, State};
    use axum::http::HeaderMap;
    use axum::routing::post;
    use axum::{Json, Router};

    #[derive(Clone)]
    struct StateData {
        grants: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
        bearers: Arc<Mutex<Vec<Option<String>>>>,
    }

    let state = StateData {
        grants: Arc::default(),
        bearers: Arc::default(),
    };
    let app = Router::new()
        .route(
            "/token",
            post(
                |State(state): State<StateData>,
                 Form(form): Form<BTreeMap<String, String>>| async move {
                    state.grants.lock().unwrap().push(form);
                    Json(json!({
                        "access_token": OAUTH_ACCESS_TOKEN,
                        "refresh_token": OAUTH_REFRESH_TOKEN,
                        "expires_in": 3600,
                        "token_type": "Bearer",
                        "scope": "resource.read"
                    }))
                },
            ),
        )
        .route(
            "/mcp",
            post(
                |State(state): State<StateData>,
                 headers: HeaderMap,
                 Json(body): Json<Value>| async move {
                    state.bearers.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned),
                    );
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id").cloned().unwrap_or(Value::Null),
                        "result": {"content": [{"type": "text", "text": "protected-ok"}]}
                    }))
                },
            ),
        )
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{address}"), state.grants, state.bearers)
}

fn oauth_gateway_config(base: &str, scratch: &std::path::Path) -> GatewayConfig {
    let text = format!(
        "listen: 127.0.0.1:4870
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth: {{ kind: token-env, env: AITHOS_VAULT_TOKEN }}
servers:
  - name: protected
    transport: http
    url: {base}/mcp
    oauth:
      auth_url: {base}/authorize
      token_url: {base}/token
      client_id: owner-public-client
      client_secret:
        broker: enterprise
        path: aithos/oauth/client
        field: client_secret
      scopes: [resource.read]
      redirect_uri: https://{HOSTNAME}/oauth/callback
      token_vault:
        broker: enterprise
        path: {OAUTH_TOKEN_PATH}
        field: {OAUTH_TOKEN_FIELD}
contexts:
  - name: protected-context
    store: {{ kind: fs, root: {}/context }}
    tools:
      protected__read:
        server: protected
        tool: read
        access: read
journal:
  store: {{ kind: fs, root: {}/journal }}
",
        scratch.display(),
        scratch.display()
    );
    assert!(!text.contains(OAUTH_CLIENT_SECRET));
    assert!(!text.contains(OAUTH_ACCESS_TOKEN));
    assert!(!text.contains(OAUTH_REFRESH_TOKEN));
    GatewayConfig::from_yaml(&text).unwrap()
}

async fn application_request(
    session: &TunnelSession,
    connector: &TlsConnector,
    method: &str,
    target: &str,
) -> (u16, Vec<u8>, Arc<Mutex<Vec<u8>>>) {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let mut tls = connect_public(session, connector, Arc::clone(&capture)).await;
    let request = format!(
        "{method} {target} HTTP/1.1\r\nhost: {HOSTNAME}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    tls.write_all(request.as_bytes()).await.unwrap();
    tls.flush().await.unwrap();
    let headers = read_headers(&mut tls).await.unwrap();
    let headers = std::str::from_utf8(&headers).unwrap();
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap()
        .parse()
        .unwrap();
    let length: usize = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    let mut body = vec![0u8; length];
    tls.read_exact(&mut body).await.unwrap();
    (status, body, capture)
}

#[tokio::test(flavor = "multi_thread")]
async fn one_router_serves_direct_and_relay_with_oauth_vault_custody() {
    use axum::routing::{get, post};
    use axum::Router;

    let scratch = tempfile::tempdir().unwrap();
    let (protected_base, token_grants, resource_bearers) = fake_protected_mcp().await;
    let config = oauth_gateway_config(&protected_base, scratch.path());
    let vault = Arc::new(OAuthVault::default());
    vault.put("aithos/oauth/client", "client_secret", OAUTH_CLIENT_SECRET);
    let mut brokers: BTreeMap<String, Arc<dyn CredentialBroker>> = BTreeMap::new();
    brokers.insert("enterprise".into(), vault.clone());
    let registry = Arc::new(UpstreamOAuthRegistry::from_config(&config, &brokers).unwrap());
    let protected = HttpUpstream::for_server_with_oauth(
        &config.servers.as_ref().unwrap()[0],
        &brokers,
        &registry,
    )
    .unwrap();

    let app = Router::new()
        .route("/mcp", post(|| async { "mcp" }))
        .route("/control/v1/status", get(|| async { "status" }))
        .merge(upstream_oauth::router(Arc::clone(&registry)));
    let direct_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let direct_address = direct_listener.local_addr().unwrap();
    let direct_app = app.clone();
    let direct_task = tokio::spawn(async move {
        axum::serve(direct_listener, direct_app).await.ok();
    });

    let identity = identity();
    let mut fake = fake_relay(gateway_pub_multibase(&identity)).await;
    let relay =
        RelayClient::with_root_certificates(relay_config(fake.addr), vec![fake.ca]).unwrap();
    let (_tls_files, public_acceptor, public_connector, _public_ca) = public_tls();
    let (ingress, listener) = relay_application_channel(64).unwrap();
    let relayed_app = app.clone();
    let application_task = tokio::spawn(async move {
        axum::serve(listener, relayed_app).await.ok();
    });
    let health = RelayHealth::new(RelayReadiness::Disabled);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let relay_task = tokio::spawn(async move {
        relay
            .run(
                identity,
                RelayInputs {
                    clock: Arc::new(|| AT.into()),
                    nonce: Arc::new(|| "g1c-tunnel-nonce-0001".into()),
                    jitter: Arc::new(|| 0),
                },
                health,
                shutdown_rx,
                move |stream| {
                    let ingress = ingress.clone();
                    let acceptor = public_acceptor.clone();
                    async move {
                        ingress.accept(&acceptor, stream).await.unwrap();
                    }
                },
            )
            .await
            .unwrap();
    });
    let session = tokio::time::timeout(Duration::from_secs(2), fake.sessions.recv())
        .await
        .unwrap()
        .unwrap();

    let direct_status = reqwest::get(format!("http://{direct_address}/control/v1/status"))
        .await
        .unwrap();
    assert_eq!(direct_status.status(), reqwest::StatusCode::OK);
    assert_eq!(direct_status.text().await.unwrap(), "status");

    let (mcp_status, mcp_body, mcp_capture) =
        application_request(&session, &public_connector, "POST", "/mcp").await;
    assert_eq!(mcp_status, 200);
    assert_eq!(mcp_body, b"mcp");
    let (control_status, control_body, control_capture) =
        application_request(&session, &public_connector, "GET", "/control/v1/status").await;
    assert_eq!(control_status, 200);
    assert_eq!(control_body, b"status");

    let consent = registry.start("protected").await.unwrap();
    let consent_url = reqwest::Url::parse(&consent.authorization_url).unwrap();
    let state = consent_url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    let pending = vault.get(OAUTH_TOKEN_PATH, OAUTH_TOKEN_FIELD).unwrap();
    let pending: Value = serde_json::from_str(&pending).unwrap();
    let verifier = pending["code_verifier"].as_str().unwrap().to_owned();
    assert_eq!(pending["state"], state);
    assert!(!consent.authorization_url.contains(&verifier));

    let callback_target = format!("/oauth/callback?code=approved-code&state={state}");
    let (callback_status, callback_body, callback_capture) =
        application_request(&session, &public_connector, "GET", &callback_target).await;
    assert_eq!(callback_status, 200);
    assert!(registry.is_connected("protected").await);
    let connected = vault.get(OAUTH_TOKEN_PATH, OAUTH_TOKEN_FIELD).unwrap();
    let connected: Value = serde_json::from_str(&connected).unwrap();
    assert_eq!(connected["access_token"], OAUTH_ACCESS_TOKEN);
    assert_eq!(connected["refresh_token"], OAUTH_REFRESH_TOKEN);
    assert_eq!(token_grants.lock().unwrap().len(), 1);

    let answer = protected
        .forward(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "read", "arguments": {}}
        }))
        .await
        .unwrap();
    assert_eq!(answer["result"]["content"][0]["text"], "protected-ok");
    assert_eq!(
        resource_bearers.lock().unwrap().as_slice(),
        &[Some(format!("Bearer {OAUTH_ACCESS_TOKEN}"))]
    );

    let captures = [mcp_capture, control_capture, callback_capture];
    for capture in captures {
        let ciphertext = capture.lock().unwrap();
        for secret in [
            OAUTH_CLIENT_SECRET,
            OAUTH_ACCESS_TOKEN,
            OAUTH_REFRESH_TOKEN,
            verifier.as_str(),
            state.as_str(),
            "approved-code",
        ] {
            assert!(!ciphertext
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }
    let callback_body = String::from_utf8(callback_body).unwrap();
    for secret in [OAUTH_CLIENT_SECRET, OAUTH_ACCESS_TOKEN, OAUTH_REFRESH_TOKEN] {
        assert!(!callback_body.contains(secret));
    }

    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), relay_task)
        .await
        .unwrap()
        .unwrap();
    direct_task.abort();
    application_task.abort();
}
