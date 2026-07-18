//! aithos-relay: the public passthrough hub (INFRA-PROVIDER annexe B,
//! contrat C2, lot P6).
//!
//! **Jalon M2 (this binary):** one public door on `:443`. Every inbound
//! connection is SNI-peeked WITHOUT terminating TLS (annexe B.1/B.4); the
//! peek routes it (see [`aithos_provider::passthrough::RelayDoor`]):
//!
//! - the relay's own tunnel name + ALPN `aithos-tunnel/1` → terminate the
//!   relay's OWN TLS, read the B.2 registration line, and — on accept —
//!   pin a live yamux tunnel (the pod is the yamux server, the relay the
//!   client);
//! - a hostname with an active tunnel → pipe the raw bytes (ClientHello
//!   first) into a fresh yamux stream; the pod terminates the public TLS
//!   (its key stays client-side, A3);
//! - anything else → silent close (no banner, nothing to enumerate).
//!
//! Fail-closed startup, environment-only config, no CLIENT secret. The
//! relay does hold ONE secret of its OWN — its tunnel-door TLS key
//! (`relay.aithos.fr`), a provider certificate, never a client's:
//!
//! | Variable | Rôle |
//! |---|---|
//! | `AITHOS_RELAY_LISTEN`         | public door bind (default `0.0.0.0:8443`) |
//! | `AITHOS_RELAY_TUNNEL_NAME`    | REQUIRED — the relay's own SNI (the tunnel door), e.g. `relay.aithos.fr` |
//! | `AITHOS_RELAY_TLS_CERT[_PEM]` | REQUIRED — tunnel-door cert chain: `_PEM` content (Secrets Manager) or a file path |
//! | `AITHOS_RELAY_TLS_KEY[_PEM]`  | REQUIRED — tunnel-door private key: `_PEM` content (Secrets Manager) or a file path |
//! | `AITHOS_RELAY_BOOTSTRAP`      | REQUIRED — control-plane mappings (gateway_pub ↔ tenant ↔ hostname); P7 replaces with DynamoDB |
//! | `AITHOS_RELAY_NONCE_BACKEND`  | `dynamodb` (default) or `memory` (dev/tests) |
//! | `AITHOS_RELAY_NONCE_TABLE`    | REQUIRED when backend is dynamodb |
//! | `AITHOS_RELAY_NONCE_WINDOW_SECS` | reservation window, clamped ≥ 600 (B.2) |
//!
//! The relay authenticates pods by their signed registration line (zero
//! new client secret). DynamoDB access rides the Fargate task role.

use std::sync::Arc;

use aithos_provider::control::ControlPlane;
use aithos_provider::nonces::{DynamoDbNonces, MemNonces, NonceStore, MIN_WINDOW_SECS};
use aithos_provider::passthrough::{RelayDoor, SessionRegistry};
use aithos_provider::tls::tunnel_server_config_from_pem;
use aithos_provider::tunnel::TUNNEL_WIRE_VERSION;

fn required(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("fatal: {name} is required (fail-closed startup)");
            std::process::exit(2);
        }
    }
}

/// Load PEM material for `base`: prefer `{base}_PEM` (content, from Secrets
/// Manager on Fargate), else `{base}` (a file path, for local/dev). Missing
/// or empty → fail-closed exit.
fn load_pem(base: &str) -> Vec<u8> {
    if let Ok(pem) = std::env::var(format!("{base}_PEM")) {
        if !pem.trim().is_empty() {
            return pem.into_bytes();
        }
    }
    let path = required(base);
    std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("fatal: cannot read {base} at {path}: {e}");
        std::process::exit(2);
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    // Install the ring crypto provider process-wide (the tunnel door's TLS).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let listen = std::env::var("AITHOS_RELAY_LISTEN").unwrap_or_else(|_| "0.0.0.0:8443".into());
    let tunnel_name = required("AITHOS_RELAY_TUNNEL_NAME").to_ascii_lowercase();
    let bootstrap = required("AITHOS_RELAY_BOOTSTRAP");

    // The tunnel-door TLS material (the relay's OWN cert — a provider
    // secret, never a client's). On Fargate it arrives as env CONTENT
    // injected from Secrets Manager (`*_PEM`); locally/dev it is a file
    // PATH. Content wins when present. Fail-closed: no material → no boot.
    let cert_pem = load_pem("AITHOS_RELAY_TLS_CERT");
    let key_pem = load_pem("AITHOS_RELAY_TLS_KEY");
    let tls_config = tunnel_server_config_from_pem(&cert_pem, &key_pem).unwrap_or_else(|e| {
        eprintln!("fatal: tunnel TLS config rejected: {e}");
        std::process::exit(2);
    });
    let acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    let (control, _preloads) = match ControlPlane::load_bootstrap(&bootstrap) {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("fatal: bootstrap rejected: {e}");
            std::process::exit(2);
        }
    };
    let control = Arc::new(control);

    let window_secs = std::env::var("AITHOS_RELAY_NONCE_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(MIN_WINDOW_SECS)
        .max(MIN_WINDOW_SECS);
    let backend = std::env::var("AITHOS_RELAY_NONCE_BACKEND").unwrap_or_else(|_| "dynamodb".into());
    let nonces: Arc<dyn NonceStore> = match backend.as_str() {
        "memory" => {
            tracing::warn!("nonce backend = memory: single-instance anti-rejeu (dev/tests only)");
            Arc::new(MemNonces::new(window_secs))
        }
        "dynamodb" => {
            let table = required("AITHOS_RELAY_NONCE_TABLE");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(DynamoDbNonces::new(
                aws_sdk_dynamodb::Client::new(&config),
                table,
                window_secs,
            ))
        }
        other => {
            eprintln!("fatal: unknown nonce backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    let door = RelayDoor::new(
        control,
        nonces,
        Arc::new(SessionRegistry::new()),
        acceptor,
        tunnel_name.clone(),
    );

    let listener = match tokio::net::TcpListener::bind(&listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("fatal: cannot bind {listen}: {e}");
            std::process::exit(2);
        }
    };
    eprintln!(
        "aithos-relay {TUNNEL_WIRE_VERSION} (M2) public door on {listen}, tunnel name {tunnel_name}"
    );

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!("accept error: {e}");
                continue;
            }
        };
        // Redline B.3 (M2): TCP keepalive, short idle — the pod TUNNEL
        // socket is the target (a pod dead without a FIN is detected, its
        // NAT mapping stays warm); on public flows it is inert pipe
        // hygiene. Never fatal: liveness aid, not a security gate.
        if let Err(e) = aithos_provider::keepalive::enable_tunnel_keepalive(&stream) {
            tracing::warn!("tcp keepalive not set on accepted socket: {e}");
        }
        let door = door.clone();
        tokio::spawn(async move {
            let peer_ip = peer.ip().to_string();
            if let Err(e) = door.serve(stream, peer_ip, now_ms()).await {
                tracing::debug!("connection dropped: {e}");
            }
        });
    }
}
