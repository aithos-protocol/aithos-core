//! G1b B.5 client against the real provider verifier and DNS effect.
//! The provider remains dev-only; the production gateway graph stays acyclic.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aithos_gateway::core_bridge::gateway_pub_multibase;
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::public_tls::AcmeTxtClient;
use aithos_gateway::GatewayError;
use aithos_provider::acme::AcmeState;
use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::dns::{MemDnsTxt, ACME_TXT_TTL_SECS};
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, AppState, DepositLocks};
use aithos_provider::time::render_rfc3339z;
use tokio::net::TcpListener;

const HOSTNAME: &str = "demo.mcp.aithos.fr";

fn identity() -> Arc<Keyholder> {
    Arc::new(Keyholder::from_entropy([0x42; 32], [0x51; 32]))
}

#[tokio::test]
async fn signed_put_and_delete_apply_only_to_the_bound_gateway_hostname() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let identity = identity();
    let mut control = ControlPlane::default();
    control.seed_tenant("acme", false);
    control.bind_tunnel(
        gateway_pub_multibase(&identity),
        TunnelBinding {
            tenant: "acme".into(),
            hostname: HOSTNAME.into(),
            suspended: false,
        },
    );
    let dns = Arc::new(MemDnsTxt::new());
    let state = Arc::new(AppState {
        control: Arc::new(control),
        objects: Arc::new(MemObjects::new()),
        heads: Arc::new(MemHeads::new()),
        deposit_locks: DepositLocks::default(),
        nonces: Arc::new(MemNonces::new(600)),
        dns: dns.clone(),
        acme: AcmeState::new(),
        authority: addr.to_string(),
        test_now_enabled: false,
        browser_origins: Default::default(),
    });
    let server = tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.unwrap();
    });

    let sequence = Arc::new(AtomicUsize::new(0));
    let client = AcmeTxtClient::new(
        &format!("http://{addr}"),
        identity,
        Arc::new(|| {
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            render_rfc3339z(millis)
        }),
        {
            let sequence = Arc::clone(&sequence);
            Arc::new(move || {
                format!(
                    "g1b-acme-nonce-{:08}",
                    sequence.fetch_add(1, Ordering::SeqCst)
                )
            })
        },
    )
    .unwrap();
    let value = "P6_test-challenge_value_01";
    client.present_txt(HOSTNAME, value).await.unwrap();
    assert_eq!(
        dns.record_of("_acme-challenge.demo.mcp.aithos.fr"),
        Some((value.into(), ACME_TXT_TTL_SECS))
    );

    let refusal = client
        .present_txt("neighbor.mcp.aithos.fr", "P6_neighbor_value_02")
        .await
        .unwrap_err();
    assert!(matches!(
        refusal,
        GatewayError::RelayUnavailable(reason) if reason == "acme_store_refused"
    ));
    assert!(dns
        .record_of("_acme-challenge.neighbor.mcp.aithos.fr")
        .is_none());

    client.retire_txt(HOSTNAME, value).await.unwrap();
    assert!(dns
        .record_of("_acme-challenge.demo.mcp.aithos.fr")
        .is_none());
    server.abort();
    let _ = server.await;
}
