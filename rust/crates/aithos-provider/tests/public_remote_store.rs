//! SDK-1 RED/GREEN contract: the public reader is anonymous and cannot
//! accidentally be used as an authenticated/private store client.

use std::collections::BTreeSet;
use std::sync::Arc;

use aithos_bundle::remote::{PublicRemoteError, PublicRemoteStore};
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, parse_browser_origins, AppState};

#[test]
fn signed_browser_origins_are_exact_canonical_and_tls_bounded() {
    let parsed = parse_browser_origins(Some(
        "https://app.aithos.fr,http://127.0.0.1:4173,http://[::1]:4173",
    ))
    .unwrap();
    assert_eq!(parsed.len(), 3);
    for invalid in [
        "*",
        "null",
        "http://app.aithos.fr",
        "https://app.aithos.fr/extra",
        "https://app.aithos.fr,https://app.aithos.fr",
    ] {
        assert!(parse_browser_origins(Some(invalid)).is_err(), "{invalid}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn public_reader_fetches_anonymously_and_refuses_private_paths() {
    let vectors = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");
    let p1: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{vectors}/p1-store-envelope.json")).unwrap(),
    )
    .unwrap();
    let tenant = p1["tenant"].as_str().unwrap();
    let did = p1["did"].as_str().unwrap();
    let bootstrap = serde_json::json!({
        "tenants": [{
            "tenant": tenant,
            "dids": [{
                "did": did,
                "did_json": p1["did_json_jcs"],
                "objects": [{
                    "key": "e/public/welcome.md",
                    "utf8": "# Welcome\n",
                }],
            }],
        }],
    });
    let (control, preloads, _seeds) =
        ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("bootstrap");
    let objects = Arc::new(MemObjects::new());
    for (tenant, did, key, bytes) in preloads {
        use aithos_provider::objects::ObjectStore as _;
        objects
            .put(&tenant, &did, &key, bytes)
            .await
            .expect("preload");
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
        test_now_enabled: false,
        browser_origins: Arc::new(BTreeSet::from(["https://app.aithos.fr".to_owned()])),
    });
    tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.ok();
    });

    let client = PublicRemoteStore::new(&format!("http://127.0.0.1:{port}"), tenant, did)
        .expect("public client");
    assert_eq!(
        client
            .get("e/public/welcome.md")
            .expect("public read")
            .as_deref(),
        Some(b"# Welcome\n".as_slice())
    );
    assert_eq!(client.sent_requests().len(), 1);
    assert!(!client.sent_requests()[0].carried_auth);

    let error = client
        .get("e/self/index.json")
        .expect_err("private path must fail locally");
    assert!(matches!(
        error,
        PublicRemoteError::PathNotPublic { path } if path == "e/self/index.json"
    ));
    assert_eq!(client.sent_requests().len(), 1, "no private wire request");

    let response = reqwest::get(format!(
        "http://127.0.0.1:{port}/t/{tenant}/{did}/e/public/welcome.md"
    ))
    .await
    .expect("browser-like public GET");
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );

    let http = reqwest::Client::new();
    let public_preflight = http
        .request(
            reqwest::Method::OPTIONS,
            format!("http://127.0.0.1:{port}/t/{tenant}/{did}/e/public/welcome.md"),
        )
        .header("Origin", "https://elsewhere.example")
        .header("Access-Control-Request-Method", "GET")
        .header("Access-Control-Request-Headers", "X-Aithos-Store")
        .send()
        .await
        .unwrap();
    assert_eq!(public_preflight.status(), 204);
    assert_eq!(
        public_preflight
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );

    let publication_url = format!("http://127.0.0.1:{port}/t/{tenant}/{did}/manifest.json");
    let signed_preflight = http
        .request(reqwest::Method::OPTIONS, &publication_url)
        .header("Origin", "https://app.aithos.fr")
        .header("Access-Control-Request-Method", "PUT")
        .header(
            "Access-Control-Request-Headers",
            "Content-Type, If-Head, X-Aithos-Auth, X-Aithos-Store",
        )
        .send()
        .await
        .unwrap();
    assert_eq!(signed_preflight.status(), 204);
    assert_eq!(
        signed_preflight
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://app.aithos.fr")
    );
    assert!(signed_preflight
        .headers()
        .get("access-control-allow-credentials")
        .is_none());

    for (origin, method, headers) in [
        (
            "https://neighbor.aithos.fr",
            "PUT",
            "Content-Type, If-Head, X-Aithos-Auth, X-Aithos-Store",
        ),
        (
            "https://app.aithos.fr",
            "POST",
            "Content-Type, If-Head, X-Aithos-Auth, X-Aithos-Store",
        ),
        (
            "https://app.aithos.fr",
            "PUT",
            "Content-Type, If-Head, X-Aithos-Auth, X-Aithos-Store, X-Extra",
        ),
    ] {
        let refused = http
            .request(reqwest::Method::OPTIONS, &publication_url)
            .header("Origin", origin)
            .header("Access-Control-Request-Method", method)
            .header("Access-Control-Request-Headers", headers)
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), 403);
        assert!(refused
            .headers()
            .get("access-control-allow-origin")
            .is_none());
    }

    let refused_actual = http
        .put(&publication_url)
        .header("Origin", "https://neighbor.aithos.fr")
        .header("Content-Type", "application/octet-stream")
        .header("If-Head", "none")
        .header("X-Aithos-Auth", "invalid")
        .header("X-Aithos-Store", "1.0.0-draft.1")
        .body(Vec::new())
        .send()
        .await
        .unwrap();
    assert_eq!(refused_actual.status(), 403);
    assert!(refused_actual
        .headers()
        .get("access-control-allow-origin")
        .is_none());

    let duplicate_origin = http
        .put(&publication_url)
        .header("Origin", "https://app.aithos.fr")
        .header("Origin", "https://neighbor.aithos.fr")
        .header("Content-Type", "application/octet-stream")
        .header("If-Head", "none")
        .header("X-Aithos-Auth", "invalid")
        .header("X-Aithos-Store", "1.0.0-draft.1")
        .body(Vec::new())
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_origin.status(), 403);
    assert!(duplicate_origin
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}
