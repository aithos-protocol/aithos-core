//! Regression proof for delegated Ethos writes on a provider-primary context.
//!
//! The local MCP coverage already proves that a `write.circle` session exposes
//! and executes `ethos.create`. This test keeps the same Core authority but
//! replaces the FS store with the real Provider wire. It guards the regression
//! where the permanent read-only transport signer caused a Provider 403.

use std::sync::Arc;

use aithos_bundle::bundle::Bundle;
use aithos_bundle::entropy::EntropySource;
use aithos_bundle::remote::{KeySigner, RemoteStore};
use aithos_bundle::Store;
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
use aithos_core::path::Zone;
use aithos_gateway::config::StoreConfig;
use aithos_gateway::core_bridge::{
    gateway_pub_multibase, owner_add_section, owner_deliver_circle_line, owner_grant_context,
    owner_grant_session_delegate, owner_init_context, Bridge, MandateWindow,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::store_adapter::{replicate_owner_history, GatewayStore};
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, AppState};
use aithos_provider::time::render_rfc3339z;
use ed25519_dalek::SigningKey;
use serde_json::json;

const MASTER: [u8; 32] = [0x71; 32];
const AGENT_SEED: [u8; 32] = [0x72; 32];
const GATEWAY_SEED: [u8; 32] = [0x73; 32];
const WRITER_SEED: [u8; 32] = [0x74; 32];
const LABEL: &str = "remote-writes";
const TENANT: &str = "delegated";
const RESOURCE: &str = "https://gateway.example/mcp";

struct SaltedEntropy {
    salt: u64,
    counter: u64,
}

impl SaltedEntropy {
    fn fresh() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            salt: NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            counter: 0,
        }
    }
}

impl EntropySource for SaltedEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        let mut out = Vec::new();
        while out.len() < buf.len() {
            self.counter += 1;
            let mut block = [0_u8; 16];
            block[..8].copy_from_slice(&self.salt.to_be_bytes());
            block[8..].copy_from_slice(&self.counter.to_be_bytes());
            out.extend_from_slice(&block);
        }
        buf.copy_from_slice(&out[..buf.len()]);
    }
}

fn epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64
}

fn at(seconds: i64) -> String {
    render_rfc3339z(seconds * 1_000)
}

fn owner() -> OwnerKeys {
    OwnerKeys::genesis(&MasterSeed::from_bytes(aithos_core::derive::derive_key(
        &format!("aithos-gw/v1/context/{LABEL}"),
        &MASTER,
    )))
}

async fn boot_provider(did: &str) -> String {
    let bootstrap = json!({
        "tenants": [{ "tenant": TENANT, "dids": [{ "did": did }] }],
    });
    let (control, _preloads, _seeds) =
        ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("provider bootstrap");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("provider listener");
    let port = listener.local_addr().expect("provider address").port();
    let state = Arc::new(AppState {
        control: Arc::new(control),
        objects: Arc::new(MemObjects::new()),
        heads: Arc::new(MemHeads::new()),
        deposit_locks: Default::default(),
        nonces: Arc::new(MemNonces::new(600)),
        dns: Arc::new(MemDnsTxt::new()),
        acme: AcmeState::new(),
        authority: format!("127.0.0.1:{port}"),
        test_now_enabled: false,
        browser_origins: Default::default(),
    });
    tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.ok();
    });
    format!("http://127.0.0.1:{port}")
}

fn owner_replicate(local_root: &std::path::Path, url: &str, did: &str) {
    let owner = owner();
    let signer = Arc::new(KeySigner::owner("#root", owner.root_sign.clone()));
    let mut remote = RemoteStore::new(
        url,
        TENANT,
        did,
        signer,
        Arc::new(|| at(epoch_seconds())),
        Box::new(SaltedEntropy::fresh()),
    )
    .expect("owner remote client");
    replicate_owner_history(local_root, &mut remote).expect("owner replication");
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_primary_session_writes_are_scoped_and_fail_closed() {
    let scratch = tempfile::tempdir().expect("scratch");
    let local_root = scratch.path().join("ethos");
    let local_store = GatewayStore::from_config(&StoreConfig::Fs {
        root: local_root.clone(),
    })
    .expect("local store");
    let keyholder = Keyholder::from_entropy(AGENT_SEED, GATEWAY_SEED);
    let gateway_pub = gateway_pub_multibase(&keyholder);
    let agent_pub = aithos_core::wire::ed25519_pub_to_multibase(
        &SigningKey::from_bytes(&AGENT_SEED)
            .verifying_key()
            .to_bytes(),
    );
    let writer = SigningKey::from_bytes(&WRITER_SEED);
    let writer_pub =
        aithos_core::wire::ed25519_pub_to_multibase(&writer.verifying_key().to_bytes());
    let start = epoch_seconds();
    let now = at(start);
    let window = MandateWindow {
        not_before: at(start - 60),
        not_after: at(start + 86_400),
    };
    let mut entropy = SaltedEntropy::fresh();

    let did = owner_init_context(&MASTER, LABEL, local_store.clone(), &now, &mut entropy)
        .expect("owner init");
    owner_grant_context(
        &MASTER,
        LABEL,
        &agent_pub,
        &gateway_pub,
        &["remote.read".to_owned()],
        local_store.clone(),
        &window,
        &now,
        &mut entropy,
    )
    .expect("gateway equipment");
    // Model the observed effective transport surface precisely: the
    // long-lived gateway agent may read circle, but it has no mutation
    // authority. The per-session `write.circle` chain below is the authority
    // that must replace this transport signer for the write.
    let technical_owner = owner();
    let technical_agent = Mandate::build_root(
        &technical_owner.root_sign,
        &MandateSpec {
            id: "mandate_01J000000000000000000000E8".to_owned(),
            subject: did.clone(),
            grantee_id: "urn:aithos:agent:provider-reader".to_owned(),
            grantee_label: "provider reader".to_owned(),
            grantee_pub: &SigningKey::from_bytes(&AGENT_SEED).verifying_key(),
            perimeter: vec![
                PerimeterEntry::parse("act.x.gateway.remote_read").expect("remote read action"),
                PerimeterEntry::parse("read.circle").expect("circle read perimeter"),
            ],
            constraints: json!({}),
            not_before: window.not_before.clone(),
            not_after: window.not_after.clone(),
            issued_at: now.clone(),
            nonce: "88".repeat(16),
        },
    )
    .expect("technical read-only mandate");
    let technical_agent_id = technical_agent.id.clone();
    let mut local_bundle = Bundle::open(local_store.clone()).expect("local bundle");
    local_bundle
        .store
        .put(
            &format!("certs/{technical_agent_id}.json"),
            &serde_json::to_vec_pretty(&technical_agent).expect("technical mandate JSON"),
        )
        .expect("technical mandate store");
    local_bundle
        .log_owner_grant(&technical_owner, &technical_agent_id, &now, &mut entropy)
        .expect("technical mandate grant");
    let mut state: serde_json::Value = serde_json::from_slice(
        &local_bundle
            .store
            .get(aithos_gateway::core_bridge::STATE_PATH)
            .expect("gateway state read")
            .expect("gateway state"),
    )
    .expect("gateway state JSON");
    state["agent_mandate"] = json!(technical_agent_id.clone());
    local_bundle
        .store
        .put(
            aithos_gateway::core_bridge::STATE_PATH,
            &serde_json::to_vec_pretty(&state).expect("gateway state encode"),
        )
        .expect("gateway state update");
    owner_add_section(
        &MASTER,
        LABEL,
        "circle",
        "notes/existing",
        "read succeeds before the delegated write",
        local_store.clone(),
        &now,
        &mut entropy,
    )
    .expect("seed circle section");
    let parent_id = owner_grant_session_delegate(
        &MASTER,
        LABEL,
        &writer_pub,
        RESOURCE,
        &["read.circle".to_owned(), "write.circle".to_owned()],
        local_store.clone(),
        &window,
        &now,
        &mut entropy,
    )
    .expect("writer parent");
    owner_deliver_circle_line(
        &MASTER,
        LABEL,
        &gateway_pub,
        local_store.clone(),
        &now,
        &mut entropy,
    )
    .expect("gateway circle line");

    let parent_bytes = local_store
        .get(&format!("certs/{parent_id}.json"))
        .expect("parent read")
        .expect("parent certificate");
    let parent: Mandate = serde_json::from_slice(&parent_bytes).expect("parent JSON");
    let gateway_signing = SigningKey::from_bytes(&GATEWAY_SEED);
    let mut constraints = parent.constraints.clone();
    constraints["session_bind"] = json!(gateway_pub.clone());
    let leaf = Mandate::build_sub(
        &parent,
        &writer,
        &MandateSpec {
            id: "mandate_01J000000000000000000000E9".to_owned(),
            subject: did.clone(),
            grantee_id: "urn:aithos:agent:remote-write-session".to_owned(),
            grantee_label: "remote write session".to_owned(),
            grantee_pub: &gateway_signing.verifying_key(),
            perimeter: vec![
                PerimeterEntry::parse("read.circle").expect("read perimeter"),
                PerimeterEntry::parse("write.circle").expect("write perimeter"),
            ],
            constraints,
            not_before: window.not_before.clone(),
            not_after: window.not_after.clone(),
            issued_at: now.clone(),
            nonce: "99".repeat(16),
        },
    )
    .expect("session leaf");
    local_store
        .clone()
        .put(
            &format!("certs/{}.json", leaf.id),
            &serde_json::to_vec_pretty(&leaf).expect("session leaf JSON"),
        )
        .expect("session leaf store");
    let mut read_only_constraints = parent.constraints.clone();
    read_only_constraints["session_bind"] = json!(gateway_pub.clone());
    let read_only_leaf = Mandate::build_sub(
        &parent,
        &writer,
        &MandateSpec {
            id: "mandate_01J000000000000000000000E7".to_owned(),
            subject: did.clone(),
            grantee_id: "urn:aithos:agent:remote-read-session".to_owned(),
            grantee_label: "remote read session".to_owned(),
            grantee_pub: &gateway_signing.verifying_key(),
            perimeter: vec![PerimeterEntry::parse("read.circle").expect("read perimeter")],
            constraints: read_only_constraints,
            not_before: window.not_before.clone(),
            not_after: window.not_after.clone(),
            issued_at: now.clone(),
            nonce: "77".repeat(16),
        },
    )
    .expect("read-only session leaf");
    local_store
        .clone()
        .put(
            &format!("certs/{}.json", read_only_leaf.id),
            &serde_json::to_vec_pretty(&read_only_leaf).expect("read-only leaf JSON"),
        )
        .expect("read-only leaf store");
    let read_only_chain = vec![parent.clone(), read_only_leaf];
    let chain = vec![parent, leaf];

    let provider_url = boot_provider(&did).await;
    let root = local_root.clone();
    let url = provider_url.clone();
    let replicated_did = did.clone();
    tokio::task::spawn_blocking(move || owner_replicate(&root, &url, &replicated_did))
        .await
        .expect("replication task");

    let remote_store = GatewayStore::from_config_with_identity(
        &StoreConfig::Remote {
            url: provider_url,
            tenant: TENANT.to_owned(),
            did,
            mandate: vec![technical_agent_id.clone()],
            local: Some(local_root),
        },
        &keyholder,
        || Box::new(SaltedEntropy::fresh()),
    )
    .expect("provider-primary store");
    let remote_tap = match &remote_store {
        GatewayStore::Remote { remote, .. } => remote.clone(),
        _ => unreachable!("the fixture is provider-primary"),
    };
    let permanent_store = remote_store.clone();
    let expected_mandate: Vec<String> = chain.iter().map(|item| item.id.clone()).collect();
    let expected_gateway = gateway_pub;
    let expected_permanent_key = agent_pub;
    let expected_permanent_mandate = technical_agent_id;

    let result = tokio::task::spawn_blocking(move || {
        let readable = Bundle::open(remote_store.clone())
            .expect("remote bundle")
            .read_section_as_agent(
                &chain,
                &gateway_signing,
                Zone::Circle,
                "notes/existing",
                &now,
            )
            .expect("the delegated session can read circle");
        assert_eq!(readable, "read succeeds before the delegated write");

        let mut bridge = Bridge::open(
            remote_store,
            Arc::new(keyholder),
            Box::new(SaltedEntropy::fresh()),
        )
        .expect("remote bridge");
        let created_digest = bridge.ethos_create_for_chain(
            &chain,
            "circle",
            "notes",
            "created-remotely",
            "Created remotely",
            &["regression".to_owned()],
            "the write mandate must reach the Provider",
            &now,
        )?;
        let edited_digest = bridge.ethos_edit_for_chain(
            &chain,
            "circle",
            "notes/created-remotely",
            "the delegated edit also reaches the Provider",
            &created_digest,
            &now,
        )?;
        bridge.ethos_delete_for_chain(
            &chain,
            "circle",
            "notes/created-remotely",
            Some(&edited_digest),
            &now,
        )?;

        for zone in ["public", "self"] {
            let refused = bridge.ethos_create_for_chain(
                &chain,
                zone,
                "",
                "must-stay-refused",
                "Must stay refused",
                &[],
                "x",
                &now,
            );
            assert!(
                matches!(
                    refused,
                    Err(aithos_gateway::GatewayError::MandateDenied { .. })
                ),
                "{zone} must remain outside delegated mutations: {refused:?}"
            );
        }

        let mutation_count = || {
            remote_tap
                .0
                .lock()
                .expect("remote tap lock")
                .sent_envelopes()
                .into_iter()
                .filter(|envelope| {
                    envelope["method"] == "PUT"
                        || (envelope["method"] == "POST"
                            && envelope["path"]
                                .as_str()
                                .is_some_and(|path| path.ends_with("/gamma")))
                })
                .count()
        };
        let before_read_only_attempt = mutation_count();
        let read_only_refused = bridge.ethos_create_for_chain(
            &read_only_chain,
            "circle",
            "notes",
            "read-only-intrusion",
            "Read-only intrusion",
            &[],
            "must not be stored",
            &now,
        );
        assert!(
            matches!(
                read_only_refused,
                Err(aithos_gateway::GatewayError::MandateDenied { .. })
            ),
            "a read-only session must remain unable to create: {read_only_refused:?}"
        );
        assert_eq!(
            mutation_count(),
            before_read_only_attempt,
            "the refused read-only session must send no Provider mutation"
        );

        let public_index = permanent_store
            .get("e/public/index.json")
            .expect("permanent reader request");
        assert!(
            public_index.is_some(),
            "the permanent reader remains usable"
        );
        let permanent_envelope = remote_tap
            .0
            .lock()
            .expect("remote tap lock")
            .last_envelope()
            .expect("permanent reader envelope");
        assert_eq!(permanent_envelope["method"], "GET");
        assert_eq!(permanent_envelope["key"], expected_permanent_key);
        assert_eq!(
            permanent_envelope["mandate"],
            json!([expected_permanent_mandate]),
            "a delegated write must not replace the permanent transport authority"
        );

        Ok::<_, aithos_gateway::GatewayError>(
            remote_tap
                .0
                .lock()
                .expect("remote tap lock")
                .sent_envelopes(),
        )
    })
    .await
    .expect("remote write task");

    let envelopes = result.unwrap_or_else(|error| {
        panic!("a verified write.circle session must persist through the Provider, got: {error:?}")
    });
    let mutation_envelopes: Vec<&serde_json::Value> = envelopes
        .iter()
        .filter(|envelope| {
            envelope["method"] == "PUT"
                || (envelope["method"] == "POST"
                    && envelope["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("/gamma")))
        })
        .collect();
    assert!(
        !mutation_envelopes.is_empty(),
        "the create/edit/delete sequence must reach the Provider"
    );
    for envelope in mutation_envelopes {
        assert_eq!(envelope["key"], expected_gateway);
        assert_eq!(envelope["mandate"], json!(expected_mandate));
    }
}
