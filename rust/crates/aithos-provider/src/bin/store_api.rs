//! aithos-store-api: the always-hot store service (INFRA-PROVIDER §7 —
//! axum on Fargate, no cold start on the hot path).
//!
//! The binary supplies what the library never touches: the listener, the
//! system clock, the AWS session, the bootstrap file. Configuration is
//! environment-only (twelve-factor, matches the Terraform task
//! definition) and **fail-closed at startup**: a missing or inconsistent
//! variable refuses to boot rather than booting permissive.
//!
//! | Variable | Rôle |
//! |---|---|
//! | `AITHOS_STORE_LISTEN`       | bind address (default `0.0.0.0:8080`) |
//! | `AITHOS_STORE_AUTHORITY`    | REQUIRED — the served authority, pinned into every envelope check |
//! | `AITHOS_STORE_BOOTSTRAP`    | REQUIRED — tenant read-model + tunnel mappings (B.5 authority) + verified public did.json preloads (P7 replaces) |
//! | `AITHOS_STORE_NONCE_BACKEND`| `dynamodb` (default) or `memory` (single instance, dev/tests) |
//! | `AITHOS_STORE_NONCE_TABLE`  | REQUIRED when backend is dynamodb |
//! | `AITHOS_STORE_NONCE_WINDOW_SECS` | reservation window, clamped ≥ 600 (A.2 #6) |
//! | `AITHOS_STORE_DNS_BACKEND`  | `route53` (the deployed B.5 surface), `memory` (dev/tests), or `off` (default: /acme effects refuse 503, the data plane serves) |
//! | `AITHOS_STORE_ACME_ZONE_ID` | REQUIRED when the DNS backend is route53 — the delegated mcp zone |
//! | `AITHOS_STORE_TEST_NOW`     | `1` enables the `X-Aithos-Test-Now` override — replay harness ONLY, never set in a deployment |
//!
//! No secret ever enters this process: the bootstrap carries public keys
//! and public documents; DynamoDB and Route 53 access ride the task role
//! (OIDC/IMDS), never a long-lived credential.

use std::sync::Arc;

use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::{DnsTxt, MemDnsTxt, NoDnsTxt, Route53DnsTxt};
use aithos_provider::nonces::{DynamoDbNonces, MemNonces, NonceStore, MIN_WINDOW_SECS};
use aithos_provider::objects::{MemObjects, ObjectStore};
use aithos_provider::service::{build_router, AppState};
use aithos_provider::STORE_WIRE_VERSION;

fn required(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("fatal: {name} is required (fail-closed startup)");
            std::process::exit(2);
        }
    }
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

    let listen = std::env::var("AITHOS_STORE_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let authority = required("AITHOS_STORE_AUTHORITY").to_ascii_lowercase();
    let bootstrap = required("AITHOS_STORE_BOOTSTRAP");

    let (control, preloads) = match ControlPlane::load_bootstrap(&bootstrap) {
        Ok(loaded) => loaded,
        Err(e) => {
            // The bootstrap holds public material only; its errors are
            // startup diagnostics, not request-path logs.
            eprintln!("fatal: bootstrap rejected: {e}");
            std::process::exit(2);
        }
    };

    let objects: Arc<dyn ObjectStore> = Arc::new(MemObjects::new());
    for (tenant, did, bytes) in preloads {
        objects.put(&tenant, &did, "did.json", bytes).await;
    }

    let window_secs = std::env::var("AITHOS_STORE_NONCE_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(MIN_WINDOW_SECS)
        .max(MIN_WINDOW_SECS);
    let backend = std::env::var("AITHOS_STORE_NONCE_BACKEND").unwrap_or_else(|_| "dynamodb".into());
    let nonces: Arc<dyn NonceStore> = match backend.as_str() {
        "memory" => {
            tracing::warn!("nonce backend = memory: single-instance anti-rejeu (dev/tests only)");
            Arc::new(MemNonces::new(window_secs))
        }
        "dynamodb" => {
            let table = required("AITHOS_STORE_NONCE_TABLE");
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

    // The B.5 DNS seam. Default `off`: an old task definition still boots
    // the new binary — the data plane serves, every /acme effect refuses
    // 503 (fail-closed containment, never a silent success).
    let dns_backend = std::env::var("AITHOS_STORE_DNS_BACKEND").unwrap_or_else(|_| "off".into());
    let dns: Arc<dyn DnsTxt> = match dns_backend.as_str() {
        "off" => {
            tracing::warn!("dns backend = off: the /acme/txt surface refuses 503");
            Arc::new(NoDnsTxt)
        }
        "memory" => {
            tracing::warn!("dns backend = memory: records go nowhere (dev/tests only)");
            Arc::new(MemDnsTxt::new())
        }
        "route53" => {
            let zone_id = required("AITHOS_STORE_ACME_ZONE_ID");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(Route53DnsTxt::new(
                aws_sdk_route53::Client::new(&config),
                zone_id,
            ))
        }
        other => {
            eprintln!("fatal: unknown dns backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    let test_now_enabled = std::env::var("AITHOS_STORE_TEST_NOW").as_deref() == Ok("1");
    if test_now_enabled {
        tracing::warn!(
            "TEST CLOCK ENABLED (AITHOS_STORE_TEST_NOW=1) — replay harness only, \
             NEVER in a deployment"
        );
    }

    let state = Arc::new(AppState {
        control,
        objects,
        nonces,
        dns,
        acme: AcmeState::new(),
        authority: authority.clone(),
        test_now_enabled,
    });

    // B.5 hygiene: sweep challenge records older than 10 minutes, on the
    // wall clock (the test clock never reaches a deployment).
    let purge_state = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let swept = purge_state
                .acme
                .purge_stale(purge_state.dns.as_ref(), now_ms)
                .await;
            if swept > 0 {
                tracing::info!("acme purge swept {swept} stale challenge record(s)");
            }
        }
    });

    let listener = match tokio::net::TcpListener::bind(&listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("fatal: cannot bind {listen}: {e}");
            std::process::exit(2);
        }
    };
    eprintln!("aithos-store-api {STORE_WIRE_VERSION} listening on {listen}, authority {authority}");
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("aithos-store-api: shutdown signal");
    };
    if let Err(e) = axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("fatal: server error: {e}");
        std::process::exit(1);
    }
}
