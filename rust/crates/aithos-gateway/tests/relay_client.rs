//! G1a integration against the real provider verifier and yamux relay seam.
//! `aithos-provider` is dev-only here; the gateway production graph remains
//! acyclic and independent.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use aithos_gateway::config::{RelayCertificateConfig, RelayConfig, RelayReconnectConfig};
use aithos_gateway::core_bridge::gateway_pub_multibase;
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::relay::{RelayClient, RelayHealth, RelayInputs, RelayReadiness, TUNNEL_ALPN};
use aithos_gateway::GatewayError;
use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::MemNonces;
use aithos_provider::passthrough::{
    read_registration_line, spawn_pod_tunnel, SessionRegistry, TunnelSession,
};
use aithos_provider::time::parse_rfc3339z_ms;
use aithos_provider::tunnel::{answer, verify_registration};
use rustls::pki_types::CertificateDer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio_rustls::TlsAcceptor;
use tokio_util::compat::FuturesAsyncReadCompatExt;

const NOW: &str = "2026-07-16T12:00:10Z";
const AT: &str = "2026-07-16T12:00:00Z";
const HOSTNAME: &str = "demo.mcp.aithos.fr";
const TUNNEL_NAME: &str = "relay.test.aithos.fr";

struct FakeRelay {
    addr: std::net::SocketAddr,
    ca: CertificateDer<'static>,
    lines: mpsc::UnboundedReceiver<Vec<u8>>,
    sessions: mpsc::UnboundedReceiver<Arc<TunnelSession>>,
    refusal_eof: mpsc::UnboundedReceiver<bool>,
}

async fn fake_relay(
    gateway_pub: String,
    mapped_hostname: &str,
    accepted_connections: usize,
) -> FakeRelay {
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
        gateway_pub.clone(),
        TunnelBinding {
            tenant: "acme".into(),
            hostname: mapped_hostname.into(),
            suspended: false,
        },
    );
    let control = Arc::new(control);
    let nonces = Arc::new(MemNonces::new(600));
    let registry = Arc::new(SessionRegistry::new());
    let now_ms = parse_rfc3339z_ms(NOW).unwrap();
    let (line_tx, lines) = mpsc::unbounded_channel();
    let (session_tx, sessions) = mpsc::unbounded_channel();
    let (eof_tx, refusal_eof) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        for _ in 0..accepted_connections {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut tls = acceptor.accept(tcp).await.unwrap();
            assert_eq!(tls.get_ref().1.alpn_protocol(), Some(TUNNEL_ALPN));
            let line = read_registration_line(&mut tls).await.unwrap();
            line_tx.send(line.clone()).unwrap();
            let verdict =
                verify_registration(&line, control.as_ref(), nonces.as_ref(), now_ms).await;
            let response = format!("{}\n", answer(&verdict));
            tls.write_all(response.as_bytes()).await.unwrap();
            tls.flush().await.unwrap();
            match verdict {
                Ok(facts) => {
                    let session = spawn_pod_tunnel(Arc::clone(&registry), facts, tls);
                    session_tx.send(session).unwrap();
                }
                Err(_) => {
                    let mut byte = [0u8; 1];
                    let saw_eof =
                        match tokio::time::timeout(Duration::from_secs(2), tls.read(&mut byte))
                            .await
                        {
                            Ok(Ok(0)) | Ok(Err(_)) => true,
                            Ok(Ok(_)) | Err(_) => false,
                        };
                    eof_tx.send(saw_eof).unwrap();
                }
            }
        }
    });

    FakeRelay {
        addr,
        ca,
        lines,
        sessions,
        refusal_eof,
    }
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

async fn echo(mut stream: tokio_util::compat::Compat<yamux::Stream>) {
    let mut buffer = [0u8; 64];
    loop {
        let read = match stream.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        if stream.write_all(&buffer[..read]).await.is_err() {
            return;
        }
    }
}

async fn wait_for_health(health: &RelayHealth, expected: RelayReadiness) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while health.get() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn tls_b2_and_concurrent_yamux_streams_match_the_provider_contract() {
    let identity = identity();
    let mut relay = fake_relay(gateway_pub_multibase(&identity), HOSTNAME, 1).await;
    let client =
        RelayClient::with_root_certificates(relay_config(relay.addr), vec![relay.ca]).unwrap();
    let health = RelayHealth::new(RelayReadiness::Disabled);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner_health = health.clone();
    let runner = tokio::spawn(async move {
        client
            .run(
                identity,
                RelayInputs {
                    clock: Arc::new(|| AT.into()),
                    nonce: Arc::new(|| "g1-handshake-nonce-0001".into()),
                    jitter: Arc::new(|| 0),
                },
                runner_health,
                shutdown_rx,
                |stream| echo(stream.compat()),
            )
            .await
            .unwrap();
    });
    let provider_session = tokio::time::timeout(Duration::from_secs(2), relay.sessions.recv())
        .await
        .unwrap()
        .unwrap();
    let line = relay.lines.recv().await.unwrap();
    wait_for_health(&health, RelayReadiness::Ready).await;
    let parsed: serde_json::Value =
        serde_json::from_slice(line.strip_suffix(b"\n").unwrap()).unwrap();
    assert_eq!(parsed["at"], AT);
    assert_eq!(parsed["nonce"], "g1-handshake-nonce-0001");

    let (first, second) = tokio::join!(
        provider_session.open_stream(),
        provider_session.open_stream()
    );
    let mut first = first.unwrap().compat();
    let mut second = second.unwrap().compat();
    first.write_all(b"stream-a").await.unwrap();
    second.write_all(b"stream-b").await.unwrap();
    let mut echoed = [0u8; 8];
    first.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"stream-a");
    second.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"stream-b");

    drop(first);
    let mut third = provider_session.open_stream().await.unwrap().compat();
    third.write_all(b"stream-c").await.unwrap();
    third.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"stream-c");
    second.write_all(b"still-b!").await.unwrap();
    second.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"still-b!");
    drop(second);
    drop(third);
    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), runner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(health.get(), RelayReadiness::Disabled);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_provider_refusal_closes_before_any_yamux_frame() {
    let identity = identity();
    let mut relay = fake_relay(gateway_pub_multibase(&identity), "other.mcp.aithos.fr", 1).await;
    let client =
        RelayClient::with_root_certificates(relay_config(relay.addr), vec![relay.ca]).unwrap();
    let error = client
        .connect(&identity, AT, "g1-refusal-nonce-00001")
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error,
        GatewayError::RelayUnavailable(reason)
            if reason == "registration_refused:mapping_mismatch"
    ));
    assert!(relay.refusal_eof.recv().await.unwrap());
    assert!(relay.sessions.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn goaway_reconnects_with_fresh_time_and_nonce_without_restart() {
    let identity = identity();
    let mut relay = fake_relay(gateway_pub_multibase(&identity), HOSTNAME, 2).await;
    let client =
        RelayClient::with_root_certificates(relay_config(relay.addr), vec![relay.ca]).unwrap();
    let clock_counter = Arc::new(AtomicUsize::new(0));
    let nonce_counter = Arc::new(AtomicUsize::new(0));
    let inputs = RelayInputs {
        clock: {
            let counter = Arc::clone(&clock_counter);
            Arc::new(move || {
                let value = counter.fetch_add(1, Ordering::SeqCst);
                format!("2026-07-16T12:00:0{value}Z")
            })
        },
        nonce: {
            let counter = Arc::clone(&nonce_counter);
            Arc::new(move || {
                let value = counter.fetch_add(1, Ordering::SeqCst);
                format!("g1-reconnect-nonce-{value:04}")
            })
        },
        jitter: Arc::new(|| 0),
    };
    let health = RelayHealth::new(RelayReadiness::Disabled);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner_health = health.clone();
    let runner = tokio::spawn(async move {
        client
            .run(identity, inputs, runner_health, shutdown_rx, |stream| {
                echo(stream.compat())
            })
            .await
            .unwrap();
    });

    let first = tokio::time::timeout(Duration::from_secs(2), relay.sessions.recv())
        .await
        .unwrap()
        .unwrap();
    let first_line = relay.lines.recv().await.unwrap();
    wait_for_health(&health, RelayReadiness::Ready).await;
    first.goaway();

    let second = tokio::time::timeout(Duration::from_secs(2), relay.sessions.recv())
        .await
        .unwrap()
        .unwrap();
    let second_line = relay.lines.recv().await.unwrap();
    wait_for_health(&health, RelayReadiness::Ready).await;
    assert_ne!(first_line, second_line);
    let first_json: serde_json::Value =
        serde_json::from_slice(first_line.strip_suffix(b"\n").unwrap()).unwrap();
    let second_json: serde_json::Value =
        serde_json::from_slice(second_line.strip_suffix(b"\n").unwrap()).unwrap();
    assert_ne!(first_json["at"], second_json["at"]);
    assert_ne!(first_json["nonce"], second_json["nonce"]);

    let mut stream = second.open_stream().await.unwrap().compat();
    stream.write_all(b"rejoined").await.unwrap();
    let mut echoed = [0u8; 8];
    stream.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"rejoined");

    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), runner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(health.get(), RelayReadiness::Disabled);
}
