//! SDK-1 RED/GREEN contract: the public reader is anonymous and cannot
//! accidentally be used as an authenticated/private store client.

use std::sync::Arc;

use aithos_bundle::remote::{PublicRemoteError, PublicRemoteStore};
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, AppState};

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
}
