//! P4 — the §3.6 client-side gate: « navigation sur cache local (hit)
//! p50 < 5 ms ». The RemoteStore's A.6 cache is the measured surface:
//! an IMMUTABLE object (a mandate cert) is fetched once over the wire,
//! then re-read 1 000 times — every re-read must be a LOCAL hit (zero
//! wire), and the p50 must sit under the gate.
//!
//! Network-independent by design (the wire is hit exactly once): the
//! sandbox pre-measure and the official run on Mathieu's machine
//! measure the same thing — the client's own cache path. Run with:
//! `cargo test -p aithos-provider --test remote_cache_nav -- --nocapture`

use std::sync::Arc;
use std::time::Instant;

use aithos_bundle::entropy::EntropySource;
use aithos_bundle::remote::{KeySigner, RemoteStore};
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, AppState};

#[path = "fixtures/vectors.rs"]
mod fixtures_vectors;

struct SeqEntropy(u64);

impl EntropySource for SeqEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            self.0 += 1;
            let bytes = self.0.to_be_bytes();
            let n = chunk.len().min(8);
            chunk[..n].copy_from_slice(&bytes[..n]);
        }
    }
}

/// Deterministic harness instant, mid-window of the frozen p1 mandate
/// (2026-07-01 → 2026-08-01): the real clock made this test a time bomb
/// that went off when the vector window closed on 2026-08-01.
const TEST_NOW: &str = "2026-07-15T12:00:00Z";

fn harness_now() -> String {
    TEST_NOW.to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn local_cache_navigation_p50_under_5ms() {
    // The p1 fixtures: vector DID, its did.json and the committed cert.
    let vectors = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");
    let p1: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{vectors}/p1-store-envelope.json")).unwrap(),
    )
    .unwrap();
    let tenant = p1["tenant"].as_str().unwrap().to_owned();
    let did = p1["did"].as_str().unwrap().to_owned();
    let mandate: serde_json::Value =
        serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
    let mandate_id = mandate["id"].as_str().unwrap();
    // `a1-genesis.json` est un vecteur `shared` (owner: core) : il se
    // résout via le helper SPL-1 (`AITHOS_VECTORS_DIR` côté dépôt
    // service, repli monorepo inchangé ici).
    let a1: serde_json::Value =
        serde_json::from_str(&fixtures_vectors::vector_str("a1-genesis.json")).unwrap();
    let seed: [u8; 32] = hex::decode(a1["seed_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let root_sk = ed25519_dalek::SigningKey::from_bytes(&aithos_core::derive::derive_key(
        aithos_core::derive::CTX_ROOT_SIGN,
        &seed,
    ));

    // The real service, in-process, cert preloaded (bootstrap shape).
    let bootstrap = serde_json::json!({
        "tenants": [{
            "tenant": tenant,
            "dids": [{
                "did": did,
                "did_json": p1["did_json_jcs"],
                "objects": [{
                    "key": format!("certs/{mandate_id}.json"),
                    "utf8": p1["mandate_jcs"],
                }],
            }],
        }],
    });
    let (control, preloads, _seeds) =
        ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("bootstrap");
    let objects = Arc::new(MemObjects::new());
    for (t, d, key, bytes) in preloads {
        use aithos_provider::objects::ObjectStore as _;
        objects.put(&t, &d, &key, bytes).await.expect("preload");
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(AppState {
        control: Arc::new(control),
        objects,
        heads: Arc::new(MemHeads::new()),
        deposit_locks: Default::default(),
        nonces: Arc::new(MemNonces::new(600)),
        dns: Arc::new(MemDnsTxt::new()),
        acme: AcmeState::new(),
        authority: format!("127.0.0.1:{port}"),
        test_now_enabled: true,
        browser_origins: Arc::default(),
    });
    tokio::spawn(async move {
        // Deterministic service clock: every request carries the harness
        // instant, matching the client's injected `at` (skew 0).
        let router = build_router(state).layer(axum::middleware::from_fn(
            |mut request: axum::extract::Request, next: axum::middleware::Next| async move {
                request.headers_mut().insert(
                    "x-aithos-test-now",
                    axum::http::HeaderValue::from_static(TEST_NOW),
                );
                next.run(request).await
            },
        ));
        axum::serve(listener, router).await.ok();
    });

    let url = format!("http://127.0.0.1:{port}");
    let cert_path = format!("certs/{mandate_id}.json");
    let (p50_us, wire_hits) = tokio::task::spawn_blocking(move || {
        let client = RemoteStore::new(
            &url,
            &tenant,
            &did,
            Arc::new(KeySigner::owner("#root", root_sk)),
            Arc::new(harness_now),
            Box::new(SeqEntropy(0)),
        )
        .expect("client");
        use aithos_bundle::Store as _;
        // One wire fetch — the immutable class lands in the local cache.
        let first = client.get(&cert_path).expect("wire get").expect("cert");
        assert!(!first.is_empty());
        // 1 000 navigations: every one a LOCAL hit, zero wire.
        let mut times = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let t0 = Instant::now();
            let again = client.get(&cert_path).expect("cache get").expect("cert");
            times.push(t0.elapsed().as_micros() as u64);
            assert_eq!(again, first, "the cache serves the exact bytes");
        }
        times.sort_unstable();
        let wire = client
            .sent_requests()
            .iter()
            .filter(|r| r.path.ends_with(&format!("/{cert_path}")))
            .count();
        (times[times.len() / 2], wire)
    })
    .await
    .expect("join");

    println!("cache nav p50: {p50_us} µs over 1000 hits ({wire_hits} wire fetch)");
    assert_eq!(wire_hits, 1, "the immutable class never re-rides the wire");
    assert!(
        p50_us < 5_000,
        "§3.6 gate: local cache navigation p50 {p50_us} µs ≥ 5 ms"
    );
}
