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
//! | `AITHOS_STORE_CONTROL_BACKEND` | `dynamodb` (the P7 control-plane table — tenants, DID bindings, B.5 mappings) or `memory` (default: the bootstrap file rules, dev/tests — an old task definition still boots) |
//! | `AITHOS_STORE_CONTROL_TABLE`   | REQUIRED when the control backend is dynamodb |
//! | `AITHOS_STORE_CONTROL_TTL_SECS`| control freshness bound (default 30, arbitrage gate contrat P7 — the < 60 s suspension promise holds with margin) |
//! | `AITHOS_STORE_BOOTSTRAP`    | REQUIRED when the control backend is memory (tenant read-model + tunnel mappings + verified public did.json preloads). OPTIONAL under dynamodb — and it must then carry ZERO tenant/tunnel/preload: the table is the ONLY source of tenants in prod (P7) |
//! | `AITHOS_STORE_OBJECTS_BACKEND` | `s3` (the durable layout, étape 6) or `memory` (default: per-task, ephemeral — an old task definition still boots) |
//! | `AITHOS_STORE_OBJECTS_BUCKET`  | REQUIRED when the objects backend is s3 |
//! | `AITHOS_STORE_HEADS_BACKEND`   | `dynamodb` (the A.5 CAS table, étape 6) or `memory` (default) |
//! | `AITHOS_STORE_HEADS_TABLE`     | REQUIRED when the heads backend is dynamodb |
//! | `AITHOS_STORE_NONCE_BACKEND`| `dynamodb` (default) or `memory` (single instance, dev/tests) |
//! | `AITHOS_STORE_NONCE_TABLE`  | REQUIRED when backend is dynamodb |
//! | `AITHOS_STORE_NONCE_WINDOW_SECS` | reservation window, clamped ≥ 600 (A.2 #6) |
//! | `AITHOS_STORE_DNS_BACKEND`  | `route53` (the deployed B.5 surface), `memory` (dev/tests), or `off` (default: /acme effects refuse 503, the data plane serves) |
//! | `AITHOS_STORE_ACME_ZONE_ID` | REQUIRED when the DNS backend is route53 — the delegated mcp zone |
//! | `AITHOS_STORE_BROWSER_ORIGINS` | optional comma-separated exact origins allowed to perform signed browser publications; HTTP is loopback-only |
//! | `AITHOS_STORE_TEST_NOW`     | `1` enables the `X-Aithos-Test-Now` override — replay harness ONLY, never set in a deployment |
//!
//! No secret ever enters this process: the bootstrap carries public keys
//! and public documents; DynamoDB and Route 53 access ride the task role
//! (OIDC/IMDS), never a long-lived credential.

use std::sync::Arc;

use aithos_provider::acme::AcmeState;
use aithos_provider::control::{CachedControl, ControlPlane, ControlStore, DynamoDbControl};
use aithos_provider::dns::{DnsTxt, MemDnsTxt, NoDnsTxt, Route53DnsTxt};
use aithos_provider::heads::{DynamoDbHeads, HeadsTable, MemHeads};
use aithos_provider::nonces::{DynamoDbNonces, MemNonces, NonceStore, MIN_WINDOW_SECS};
use aithos_provider::objects::{MemObjects, ObjectStore, S3Objects};
use aithos_provider::service::{build_router, parse_browser_origins, AppState};
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
    let browser_origins = match parse_browser_origins(
        &std::env::var("AITHOS_STORE_BROWSER_ORIGINS").unwrap_or_default(),
    ) {
        Ok(origins) => Arc::new(origins),
        Err(error) => {
            eprintln!("fatal: AITHOS_STORE_BROWSER_ORIGINS rejected: {error}");
            std::process::exit(2);
        }
    };

    // P7 — the control-plane seam. Default memory: the bootstrap file
    // rules, exactly the P1/P6 shape (an old task definition boots the
    // new binary unchanged). `dynamodb` swaps the SAME three lookups onto
    // the control table behind the freshness cache.
    let control_backend =
        std::env::var("AITHOS_STORE_CONTROL_BACKEND").unwrap_or_else(|_| "memory".into());
    let bootstrap_env = std::env::var("AITHOS_STORE_BOOTSTRAP")
        .ok()
        .filter(|v| !v.trim().is_empty());

    let (bootstrap_plane, preloads, head_seeds) = match (&*control_backend, &bootstrap_env) {
        // Memory backend: the bootstrap is REQUIRED (fail-closed, P1).
        ("memory", None) => {
            eprintln!(
                "fatal: AITHOS_STORE_BOOTSTRAP is required when the control backend is \
                 memory (fail-closed startup)"
            );
            std::process::exit(2);
        }
        // Dynamodb backend without a bootstrap: the P7 resting state —
        // the image embarks NO tenant file at all.
        (_, None) => (ControlPlane::default(), Vec::new(), Vec::new()),
        (_, Some(path)) => match ControlPlane::load_bootstrap(path) {
            Ok(loaded) => loaded,
            Err(e) => {
                // The bootstrap holds public material only; its errors are
                // startup diagnostics, not request-path logs.
                eprintln!("fatal: bootstrap rejected: {e}");
                std::process::exit(2);
            }
        },
    };

    // P7 fail-closed guard (arbitrage gate contrat 2026-07-20): once the
    // table rules, NO tenant, tunnel mapping, preload or head seed may
    // ride the image — a bootstrap that carries any refuses to boot.
    if control_backend == "dynamodb"
        && (!bootstrap_plane.is_empty() || !preloads.is_empty() || !head_seeds.is_empty())
    {
        eprintln!(
            "fatal: the control backend is dynamodb but the bootstrap carries tenants, \
             tunnels or preloads — the control table is the only source of tenants \
             (fail-closed startup, P7 gate contrat)"
        );
        std::process::exit(2);
    }

    // Étape 6 — the durable backends behind the seams. Defaults stay
    // memory so an old task definition still boots the new binary; the
    // Terraform task definition opts into s3/dynamodb explicitly.
    let objects_backend =
        std::env::var("AITHOS_STORE_OBJECTS_BACKEND").unwrap_or_else(|_| "memory".into());
    let heads_backend =
        std::env::var("AITHOS_STORE_HEADS_BACKEND").unwrap_or_else(|_| "memory".into());

    // Décision ② du gate P2/étape 6 (2026-07-20, gravée INFRA-PROVIDER
    // §8) : embedded replay material never persists — a durable backend
    // refuses to boot with bootstrap preloads or head seeds.
    let durable = objects_backend != "memory" || heads_backend != "memory";
    if durable && (!preloads.is_empty() || !head_seeds.is_empty()) {
        eprintln!(
            "fatal: the bootstrap carries preloads/head seeds but a durable backend is \
             configured — replay material never persists (fail-closed startup, \
             décision gate étape 6)"
        );
        std::process::exit(2);
    }

    let objects: Arc<dyn ObjectStore> = match objects_backend.as_str() {
        "memory" => {
            tracing::warn!("objects backend = memory: per-task, ephemeral (dev/tests only)");
            let mem = MemObjects::new();
            for (tenant, did, key, bytes) in preloads {
                if mem.put(&tenant, &did, &key, bytes).await.is_err() {
                    eprintln!("fatal: preload rejected (fail-closed startup)");
                    std::process::exit(2);
                }
            }
            Arc::new(mem)
        }
        "s3" => {
            let bucket = required("AITHOS_STORE_OBJECTS_BUCKET");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(S3Objects::new(aws_sdk_s3::Client::new(&config), bucket))
        }
        other => {
            eprintln!("fatal: unknown objects backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    // The A.5 heads table — the CAS seam. Seeds are replay fixtures only
    // (memory backend; a durable backend refused them above).
    let heads: Arc<dyn HeadsTable> = match heads_backend.as_str() {
        "memory" => {
            tracing::warn!("heads backend = memory: per-task, ephemeral (dev/tests only)");
            let mem_heads = MemHeads::new();
            for (tenant, did, record) in head_seeds {
                mem_heads.seed(&tenant, &did, record);
            }
            Arc::new(mem_heads)
        }
        "dynamodb" => {
            let table = required("AITHOS_STORE_HEADS_TABLE");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(DynamoDbHeads::new(
                aws_sdk_dynamodb::Client::new(&config),
                table,
            ))
        }
        other => {
            eprintln!("fatal: unknown heads backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

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

    let control: Arc<dyn ControlStore> = match control_backend.as_str() {
        "memory" => Arc::new(bootstrap_plane),
        "dynamodb" => {
            let table = required("AITHOS_STORE_CONTROL_TABLE");
            let ttl_secs = std::env::var("AITHOS_STORE_CONTROL_TTL_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(30);
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(CachedControl::new(
                DynamoDbControl::new(aws_sdk_dynamodb::Client::new(&config), table),
                ttl_secs,
            ))
        }
        other => {
            eprintln!("fatal: unknown control backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    let state = Arc::new(AppState {
        control,
        objects,
        heads,
        deposit_locks: Default::default(),
        nonces,
        dns,
        acme: AcmeState::new(),
        authority: authority.clone(),
        browser_origins,
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
