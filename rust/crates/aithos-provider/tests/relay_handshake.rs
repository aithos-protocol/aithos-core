//! Integration test of the relay registration handshake over REAL TCP
//! sockets (annexe B.2, lot P6 jalon M1). A relay server task runs
//! `serve_registration`; a client dials in, sends a signed line, reads the
//! one-line answer. The pure verification is already proven byte-exact
//! against `p3` (tunnel_replay + cucumber_tunnel); this proves the framing
//! and the socket round-trip. The TLS/ALPN wrapper and the yamux
//! passthrough are M2 (deploy-gated).

use std::sync::Arc;

use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::{MemNonces, NonceStore};
use aithos_provider::relay::serve_registration;
use aithos_provider::time::parse_rfc3339z_ms;
use aithos_provider::tunnel::{
    registration_line, sign_registration, Registration, RegistrationSignature, TUNNEL_WIRE_VERSION,
};
use ed25519_dalek::SigningKey;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const NOW: &str = "2026-07-16T12:00:00Z";

fn gateway() -> (SigningKey, String) {
    let sk = SigningKey::from_bytes(&[0x51; 32]);
    let pubkey = aithos_core::wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes());
    (sk, pubkey)
}

fn control(gateway_pub: &str, suspended: bool) -> ControlPlane {
    let mut plane = ControlPlane::default();
    // P7b: the B.2 step 4 joins the tenant state — the fixture names it.
    plane.seed_tenant("acme", false);
    plane.bind_tunnel(
        gateway_pub.to_owned(),
        TunnelBinding {
            tenant: "acme".into(),
            hostname: "demo.mcp.aithos.fr".into(),
            suspended,
        },
    );
    plane
}

fn signed_line(sk: &SigningKey, gateway_pub: &str, hostname: &str, nonce: &str) -> String {
    let reg = Registration {
        version: TUNNEL_WIRE_VERSION.into(),
        tenant: "acme".into(),
        hostname: hostname.into(),
        gateway_pub: gateway_pub.into(),
        at: NOW.into(),
        nonce: nonce.into(),
        signature: RegistrationSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    registration_line(&sign_registration(reg, sk))
}

/// Spawn a relay accepting connections with shared state; returns its addr.
async fn spawn_relay(control: ControlPlane, nonces: Arc<dyn NonceStore>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let now_ms = parse_rfc3339z_ms(NOW).unwrap();
    let control = Arc::new(control);
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let control = control.clone();
            let nonces = nonces.clone();
            tokio::spawn(async move {
                let _ = serve_registration(stream, &control, nonces.as_ref(), now_ms).await;
            });
        }
    });
    addr
}

/// Send one framed line, read the one-line answer.
async fn exchange(addr: std::net::SocketAddr, line: &str) -> String {
    let stream = TcpStream::connect(addr).await.unwrap();
    let (read_half, mut write_half) = stream.into_split();
    write_half.write_all(line.as_bytes()).await.unwrap();
    write_half.flush().await.unwrap();
    let mut reader = BufReader::new(read_half);
    let mut answer = String::new();
    reader.read_line(&mut answer).await.unwrap();
    answer.trim_end().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_valid_registration_is_accepted_over_a_real_socket() {
    let (sk, pubkey) = gateway();
    let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
    let addr = spawn_relay(control(&pubkey, false), nonces).await;

    let line = signed_line(&sk, &pubkey, "demo.mcp.aithos.fr", "relay-int-ok-1");
    let answer = exchange(addr, &line).await;
    assert_eq!(answer, r#"{"aithos-tunnel":"1.0.0-draft.1","ok":true}"#);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_hostname_is_refused_mapping_mismatch() {
    let (sk, pubkey) = gateway();
    let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
    let addr = spawn_relay(control(&pubkey, false), nonces).await;

    let line = signed_line(&sk, &pubkey, "other.mcp.aithos.fr", "relay-int-map-1");
    let answer = exchange(addr, &line).await;
    let parsed: serde_json::Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(parsed["ok"], serde_json::Value::Bool(false));
    assert_eq!(parsed["error"], "mapping_mismatch");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_line_is_refused_over_a_real_socket() {
    let (sk, pubkey) = gateway();
    // One shared nonce store: the second presentation replays.
    let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
    let addr = spawn_relay(control(&pubkey, false), nonces).await;

    let line = signed_line(&sk, &pubkey, "demo.mcp.aithos.fr", "relay-int-replay-1");
    let first = exchange(addr, &line).await;
    assert_eq!(first, r#"{"aithos-tunnel":"1.0.0-draft.1","ok":true}"#);

    let second = exchange(addr, &line).await;
    let parsed: serde_json::Value = serde_json::from_str(&second).unwrap();
    assert_eq!(parsed["error"], "nonce_replayed");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_suspended_binding_is_refused_over_a_real_socket() {
    let (sk, pubkey) = gateway();
    let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
    let addr = spawn_relay(control(&pubkey, true), nonces).await;

    let line = signed_line(&sk, &pubkey, "demo.mcp.aithos.fr", "relay-int-susp-1");
    let answer = exchange(addr, &line).await;
    let parsed: serde_json::Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(parsed["error"], "suspended");
}
