//! Gate 8: prove that plans produced by `aithos-client` cross the exact
//! Gateway transport and are accepted by a real, isolated Provider.
//!
//! This test never starts the Gateway server and never reaches an external
//! host. It deliberately exercises the security boundaries separately:
//! `aithos-client` closes and signs every request, `ProviderTransport` only
//! carries it, and `aithos-provider` performs the real authorization and CAS.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aithos_bundle::bundle::Bundle;
use aithos_bundle::remote::{KeySigner, RemoteStore};
use aithos_client::{
    ArtifactSnapshot, AuthorizationContext, ConnectorBindingIntent, GenesisEntropy, GenesisIntent,
    GenesisPlan, Keyholder as ClientKeyholder, MemoryGenesisKeyholder, MemoryGranteeKeyholder,
    MemoryOwnerKeyholder, MutationGrantIntent, MutationGrantTarget, MutationIntent, OwnerSession,
    ProviderEnvelopePlan, ProviderReadEnvelopePlan, ProviderReadIntent, ProviderReadTarget,
    ProviderUploadIntent, PublicationEntropy, PublicationPlan, ReadLimits, SessionContext,
    SessionParentAction, SessionParentEntropy, SessionParentEthosScope, SessionParentIntent,
    SessionParentPlan,
};
use aithos_core::gamma::Entry;
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry, Verb};
use aithos_core::path::Zone;
use aithos_gateway::config::StoreConfig;
use aithos_gateway::core_bridge::{
    gateway_kex_pub_multibase, gateway_pub_multibase, Bridge, ContextRuntime, Runner, SeqEntropy,
};
use aithos_gateway::ethos_backend::ProviderTransport;
use aithos_gateway::keyholder::Keyholder as GatewayKeyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::store_adapter::GatewayStore;
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, AppState};
use aithos_provider::time::render_rfc3339z;
use ed25519_dalek::{Signer as _, SigningKey};

const TENANT: &str = "client-gateway-e2e";
const OWNER_SEED: [u8; 32] = [0x81; 32];
const GRANTEE_SEED: [u8; 32] = [0x82; 32];
const AGENT_SEED: [u8; 32] = [0x91; 32];
const GATEWAY_SEED: [u8; 32] = [0x92; 32];
const DELEGATE_SEED: [u8; 32] = [0x93; 32];
const RESOURCE: &str = "https://demo.mcp.aithos.fr/mcp";

fn epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64
}

fn at(seconds: i64) -> String {
    render_rfc3339z(seconds * 1_000)
}

async fn boot_provider(did: &str) -> (String, ProviderTransport) {
    let bootstrap = serde_json::json!({
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
        browser_origins: Arc::new(BTreeSet::new()),
    });
    tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.ok();
    });
    let url = format!("http://127.0.0.1:{port}");
    let transport = ProviderTransport::new(&url).expect("Provider transport");
    (url, transport)
}

async fn upload_genesis(
    transport: &ProviderTransport,
    owner: &MemoryGenesisKeyholder,
    plan: &GenesisPlan,
    now: &str,
    nonce: &mut u128,
) {
    for artifact in plan.upload_order() {
        *nonce += 1;
        let envelope = ProviderEnvelopePlan::for_owner_genesis(
            owner,
            plan,
            ProviderUploadIntent::new(
                transport.envelope_host(),
                TENANT,
                artifact,
                now,
                nonce.to_be_bytes(),
            ),
        )
        .expect("closed owner genesis envelope");
        let response = transport.upload(&envelope).await.expect("genesis upload");
        assert!(
            response.status().is_success(),
            "Provider refused genesis artifact {artifact}: {} {}",
            response.status(),
            String::from_utf8_lossy(response.body())
        );
    }
}

async fn upload_owner_publication(
    transport: &ProviderTransport,
    owner: &MemoryGenesisKeyholder,
    plan: &PublicationPlan,
    now: &str,
    nonce: &mut u128,
) {
    for artifact in plan.upload_order() {
        *nonce += 1;
        let envelope = ProviderEnvelopePlan::for_owner_publication(
            owner,
            plan,
            ProviderUploadIntent::new(
                transport.envelope_host(),
                TENANT,
                artifact,
                now,
                nonce.to_be_bytes(),
            ),
        )
        .expect("closed owner publication envelope");
        let response = transport
            .upload(&envelope)
            .await
            .expect("owner publication upload");
        assert!(
            response.status().is_success(),
            "Provider refused owner artifact {artifact}: {} {}",
            response.status(),
            String::from_utf8_lossy(response.body())
        );
    }
}

async fn upload_grantee_publication(
    transport: &ProviderTransport,
    grantee: &MemoryGranteeKeyholder,
    plan: &PublicationPlan,
    now: &str,
    nonce: &mut u128,
) {
    for artifact in plan.upload_order() {
        *nonce += 1;
        let envelope = ProviderEnvelopePlan::for_grantee_publication(
            grantee,
            plan,
            ProviderUploadIntent::new(
                transport.envelope_host(),
                TENANT,
                artifact,
                now,
                nonce.to_be_bytes(),
            ),
        )
        .expect("closed grantee publication envelope");
        let response = transport
            .upload(&envelope)
            .await
            .expect("grantee publication upload");
        assert!(
            response.status().is_success(),
            "Provider refused delegated artifact {artifact}: {} {}",
            response.status(),
            String::from_utf8_lossy(response.body())
        );
    }
}

async fn download_owner_snapshot(
    transport: &ProviderTransport,
    owner: &MemoryGenesisKeyholder,
    did: &str,
    paths: impl IntoIterator<Item = String>,
    now: &str,
    nonce: &mut u128,
) -> BTreeMap<String, Vec<u8>> {
    let mut artifacts = BTreeMap::new();
    for path in paths {
        *nonce += 1;
        let envelope = ProviderReadEnvelopePlan::for_owner(
            owner,
            ProviderReadIntent::new(
                transport.envelope_host(),
                TENANT,
                did,
                ProviderReadTarget::Object(path.clone()),
                now,
                nonce.to_be_bytes(),
            ),
        )
        .expect("closed owner read envelope");
        let response = transport.read(&envelope).await.expect("owner read");
        assert_eq!(
            response.status(),
            reqwest::StatusCode::OK,
            "Provider did not return {path}"
        );
        artifacts.insert(path, response.body().to_vec());
    }
    artifacts
}

async fn download_grantee_projection(
    transport: &ProviderTransport,
    grantee: &MemoryGranteeKeyholder,
    chain: &[aithos_core::mandate::Mandate],
    did: &str,
    paths: impl IntoIterator<Item = String>,
    now: &str,
    nonce: &mut u128,
) -> (BTreeMap<String, Vec<u8>>, Vec<String>) {
    let mut artifacts = BTreeMap::new();
    let mut refused = Vec::new();
    for path in paths {
        *nonce += 1;
        let envelope = ProviderReadEnvelopePlan::for_grantee(
            grantee,
            chain,
            ProviderReadIntent::new(
                transport.envelope_host(),
                TENANT,
                did,
                ProviderReadTarget::Object(path.clone()),
                now,
                nonce.to_be_bytes(),
            ),
        )
        .expect("closed grantee read envelope");
        let response = transport.read(&envelope).await.expect("grantee read");
        if response.status().is_success() {
            artifacts.insert(path, response.body().to_vec());
        } else {
            refused.push(path);
        }
    }
    (artifacts, refused)
}

#[tokio::test(flavor = "multi_thread")]
async fn client_signed_circle_mutation_survives_the_real_provider_and_cold_verifies() {
    let start = epoch_seconds();
    let now = at(start);
    let owner =
        MemoryGenesisKeyholder::from_entropy(OWNER_SEED, [0x83; 32]).expect("owner keyholder");
    let grantee = MemoryGranteeKeyholder::from_seed(GRANTEE_SEED);
    let genesis = GenesisPlan::build(
        &owner,
        GenesisIntent::new(
            now.clone(),
            "guide/welcome",
            "Welcome",
            "# Provider baseline\n",
        ),
        GenesisEntropy::new([0x84; 16], [0x85; 16]),
    )
    .expect("genesis plan");
    let (_provider_url, transport) = boot_provider(genesis.did()).await;
    let mut nonce = 1_u128;
    upload_genesis(&transport, &owner, &genesis, &now, &mut nonce).await;

    let genesis_snapshot = ArtifactSnapshot::try_from_iter(
        genesis
            .artifacts()
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone())),
    )
    .expect("genesis snapshot")
    .cold_verify()
    .expect("verified genesis");
    let grant = PublicationPlan::build_mutation_grant_owner(
        &owner,
        &grantee,
        genesis_snapshot,
        MutationGrantIntent::new(
            Zone::Circle,
            Verb::Append,
            MutationGrantTarget::Zone,
            "gateway-session",
            at(start - 60),
            at(start + 86_400),
            now.clone(),
        ),
        PublicationEntropy::new([0x86; 16], [0x87; 16]),
    )
    .expect("circle mutation grant");
    upload_owner_publication(&transport, &owner, grant.publication(), &now, &mut nonce).await;

    let (projection, refused) = download_grantee_projection(
        &transport,
        &grantee,
        grant.chain(),
        grant.publication().did(),
        grant.publication().artifacts().keys().cloned(),
        &now,
        &mut nonce,
    )
    .await;
    assert!(
        refused.iter().any(|path| path.starts_with("e/self/")),
        "a circle mandate must not expose the self zone: {refused:?}"
    );
    assert!(
        ArtifactSnapshot::try_from_iter(projection)
            .expect("covered projection layout")
            .cold_verify()
            .is_err(),
        "the whole-bundle verifier must not silently accept an incomplete projection"
    );

    let mutation = PublicationPlan::build_grantee(
        &grantee,
        grant.chain(),
        grant
            .publication()
            .cold_verify()
            .expect("verified published grant"),
        MutationIntent::create(
            Zone::Circle,
            "provider-proof",
            "written by the delegated Gateway session",
            now.clone(),
        ),
        PublicationEntropy::new([0x88; 16], [0x89; 16]),
    )
    .expect("delegated circle publication");
    upload_grantee_publication(&transport, &grantee, &mutation, &now, &mut nonce).await;

    let downloaded = download_owner_snapshot(
        &transport,
        &owner,
        mutation.did(),
        mutation.artifacts().keys().cloned(),
        &now,
        &mut nonce,
    )
    .await;
    let verified = ArtifactSnapshot::try_from_iter(downloaded)
        .expect("downloaded snapshot")
        .cold_verify()
        .expect("Provider result must remain a valid cold snapshot");
    assert_eq!(verified.did(), mutation.did());
    let read = OwnerSession::open(
        verified,
        MemoryOwnerKeyholder::from_seed(OWNER_SEED),
        SessionContext::new(now.clone(), [0x90; 32]),
    )
    .expect("owner session over the Provider snapshot")
    .read_content(
        Zone::Circle,
        "provider-proof",
        AuthorizationContext::new(now),
        ReadLimits::default(),
    )
    .expect("read delegated circle mutation from the verified Provider snapshot");
    assert_eq!(read.body(), "written by the delegated Gateway session");
}

#[tokio::test(flavor = "multi_thread")]
async fn gateway_session_working_set_creates_circle_content_on_the_real_provider() {
    let start = epoch_seconds();
    let now = at(start);
    let not_before = at(start - 60);
    let not_after = at(start + 7_200);
    let owner =
        MemoryGenesisKeyholder::from_entropy(OWNER_SEED, [0xa0; 32]).expect("owner keyholder");
    let genesis = GenesisPlan::build(
        &owner,
        GenesisIntent::new(
            now.clone(),
            "guide/welcome",
            "Welcome",
            "# Gateway baseline\n",
        ),
        GenesisEntropy::new([0xa1; 16], [0xa2; 16]),
    )
    .expect("genesis plan");
    let (provider_url, transport) = boot_provider(genesis.did()).await;
    let mut nonce = 10_000_u128;
    upload_genesis(&transport, &owner, &genesis, &now, &mut nonce).await;
    let mut snapshot = ArtifactSnapshot::try_from_iter(
        genesis
            .artifacts()
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone())),
    )
    .expect("genesis snapshot")
    .cold_verify()
    .expect("verified genesis");

    let agent = MemoryGranteeKeyholder::from_seed(AGENT_SEED);
    let gateway = MemoryGranteeKeyholder::from_seed(GATEWAY_SEED);
    let delegate = MemoryGranteeKeyholder::from_seed(DELEGATE_SEED);
    let parent_intent = |id: &str, label: &str| {
        SessionParentIntent::new(
            RESOURCE,
            vec![SessionParentAction::new("github-demo", "get_me")],
            id,
            label,
            not_before.clone(),
            not_after.clone(),
            now.clone(),
            3,
        )
        .with_ethos_scopes(vec![SessionParentEthosScope::new("circle", "write")])
    };

    let agent_parent = SessionParentPlan::issue_to_public_identity(
        &owner,
        &agent.public_keys().expect("agent public identity"),
        &snapshot,
        parent_intent("urn:aithos:agent:gateway-reader", "Gateway reader"),
        SessionParentEntropy::new([0xa3; 16], [0xa4; 16]),
    )
    .expect("agent parent");
    let agent_publication = PublicationPlan::build_session_parent_owner(
        &owner,
        snapshot,
        &agent_parent,
        now.clone(),
        PublicationEntropy::new([0xa5; 16], [0xa6; 16]),
    )
    .expect("agent parent publication");
    upload_owner_publication(&transport, &owner, &agent_publication, &now, &mut nonce).await;
    snapshot = agent_publication
        .cold_verify()
        .expect("verified agent publication");

    let gateway_parent = SessionParentPlan::issue_to_public_identity(
        &owner,
        &gateway.public_keys().expect("gateway public identity"),
        &snapshot,
        parent_intent("urn:aithos:agent:gateway-control", "Gateway control"),
        SessionParentEntropy::new([0xa7; 16], [0xa8; 16]),
    )
    .expect("gateway parent");
    let gateway_publication = PublicationPlan::build_session_parent_owner(
        &owner,
        snapshot,
        &gateway_parent,
        now.clone(),
        PublicationEntropy::new([0xa9; 16], [0xaa; 16]),
    )
    .expect("gateway parent publication");
    upload_owner_publication(&transport, &owner, &gateway_publication, &now, &mut nonce).await;
    snapshot = gateway_publication
        .cold_verify()
        .expect("verified gateway publication");

    let delegate_parent = SessionParentPlan::issue_to_public_identity(
        &owner,
        &delegate.public_keys().expect("delegate public identity"),
        &snapshot,
        parent_intent("urn:aithos:agent:cowork", "Cowork"),
        SessionParentEntropy::new([0xab; 16], [0xac; 16]),
    )
    .expect("delegate parent");
    let delegate_publication = PublicationPlan::build_session_parent_owner(
        &owner,
        snapshot,
        &delegate_parent,
        now.clone(),
        PublicationEntropy::new([0xad; 16], [0xae; 16]),
    )
    .expect("delegate parent publication");
    upload_owner_publication(&transport, &owner, &delegate_publication, &now, &mut nonce).await;
    snapshot = delegate_publication
        .cold_verify()
        .expect("verified delegate publication");

    // A real app-created Ethos already carries at least one owner-published
    // connector binding before Cowork attempts a circle mutation. Retained
    // `e/x/**` objects are outside the mutation working set and must survive
    // the sparse parent publication unchanged.
    let connector_binding = PublicationPlan::build_connector_binding_owner(
        &owner,
        snapshot,
        ConnectorBindingIntent::set(
            "github-demo",
            aithos_core::wire::ed25519_pub_to_multibase(
                &SigningKey::from_bytes(&GATEWAY_SEED)
                    .verifying_key()
                    .to_bytes(),
            ),
            serde_json::json!({
                "version": "aithos-mcp-manifest-v1",
                "server": "github-demo",
                "tools": [{
                    "name": "get_me",
                    "exposed_name": "github-demo__get_me",
                    "description": "Read the connected GitHub identity",
                    "input_schema": {"type": "object", "properties": {}},
                    "pin_sha256": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "risk_class": "read",
                    "granted": true
                }]
            }),
            at(start + 1),
        ),
        PublicationEntropy::new([0xb1; 16], [0xb2; 16]),
    )
    .expect("connector binding publication");
    upload_owner_publication(
        &transport,
        &owner,
        &connector_binding,
        &at(start + 1),
        &mut nonce,
    )
    .await;
    connector_binding
        .cold_verify()
        .expect("verified connector binding publication");

    // Gateway admission delivers only its circle key line. This append is
    // intentionally between manifest editions, as in the hosted demo; the
    // next operation-scoped publication must adopt it without accepting a
    // line addressed to any neighboring key.
    let owner_delivery_keys = OwnerKeys::genesis(&MasterSeed::from_bytes(OWNER_SEED));
    let delivery_now = now.clone();
    let owner_delivery_remote = RemoteStore::new(
        &provider_url,
        TENANT,
        genesis.did(),
        Arc::new(KeySigner::owner(
            "#root",
            owner_delivery_keys.root_sign.clone(),
        )),
        Arc::new(move || delivery_now.clone()),
        Box::new(aithos_bundle::entropy::OsEntropy),
    )
    .expect("owner delivery transport");
    let mut delivery_bundle = Bundle::open(owner_delivery_remote).expect("owner delivery bundle");
    delivery_bundle
        .deliver_zone_line(
            &owner_delivery_keys,
            &SigningKey::from_bytes(&GATEWAY_SEED).verifying_key(),
            Zone::Circle,
            "",
            None,
            &mut SeqEntropy::default(),
        )
        .expect("Gateway circle delivery");

    let sidecar = tempfile::tempdir().expect("sidecar");
    std::fs::create_dir_all(sidecar.path().join("gateway")).expect("sidecar directory");
    std::fs::write(
        sidecar.path().join("gateway/state.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "agent_mandate": agent_parent.publication().mandate_id(),
            "gateway_mandate": gateway_parent.publication().mandate_id(),
        }))
        .expect("state JSON"),
    )
    .expect("state sidecar");

    let gateway_keyholder = Arc::new(GatewayKeyholder::from_entropy(AGENT_SEED, GATEWAY_SEED));
    let remote_store = GatewayStore::from_config_with_identity(
        &StoreConfig::Remote {
            url: provider_url.clone(),
            tenant: TENANT.to_owned(),
            did: genesis.did().to_owned(),
            mandate: vec![agent_parent.publication().mandate_id().to_owned()],
            local: Some(sidecar.path().to_path_buf()),
        },
        &gateway_keyholder,
        || Box::new(SeqEntropy::default()),
    )
    .expect("provider-primary Gateway store");
    let context_bridge = Bridge::open(
        remote_store.clone(),
        Arc::clone(&gateway_keyholder),
        Box::new(SeqEntropy::default()),
    )
    .expect("context bridge");
    let journal_bridge = Bridge::open(
        remote_store,
        Arc::clone(&gateway_keyholder),
        Box::new(SeqEntropy::default()),
    )
    .expect("journal bridge");
    let mut runner = Runner::from_parts(
        BTreeMap::from([(
            "sales".to_owned(),
            ContextRuntime {
                policy: Policy::new(BTreeMap::new()),
                bridge: context_bridge,
            },
        )]),
        journal_bridge,
    );

    let parent: Mandate = delegate_parent.chain()[0].clone();
    let delegate_signing = SigningKey::from_bytes(&DELEGATE_SEED);
    let gateway_signing = SigningKey::from_bytes(&GATEWAY_SEED);
    let gateway_pub = gateway_pub_multibase(&gateway_keyholder);
    let gateway_kex_pub = gateway_kex_pub_multibase(&gateway_keyholder);
    let session_now = at(start + 10);
    let session_signing = SigningKey::from_bytes(&[0xaf; 32]);
    let session_pub =
        aithos_core::wire::ed25519_pub_to_multibase(&session_signing.verifying_key().to_bytes());
    let mut constraints = parent.constraints.clone();
    constraints["session_bind"] = serde_json::json!(session_pub);
    let leaf = Mandate::build_sub(
        &parent,
        &delegate_signing,
        &MandateSpec {
            id: "mandate_01J000000000000000000000F1".to_owned(),
            subject: genesis.did().to_owned(),
            grantee_id: "urn:aithos:agent:gateway-session".to_owned(),
            grantee_label: "Gateway session".to_owned(),
            grantee_pub: &gateway_signing.verifying_key(),
            perimeter: vec![
                PerimeterEntry::parse("act.x.github-demo.get_me").expect("GitHub read"),
                PerimeterEntry::parse("write.circle").expect("circle write"),
            ],
            constraints,
            not_before: not_before.clone(),
            not_after: not_after.clone(),
            issued_at: session_now.clone(),
            nonce: "b0".repeat(16),
        },
    )
    .expect("session leaf");
    let leaf_value = serde_json::to_value(&leaf).expect("leaf JSON");
    let unsigned_grant = runner
        .prepare_session_grant(
            "sales",
            &parent.id,
            &delegate.public_keys().expect("delegate identity").signing,
            &gateway_pub,
            &gateway_kex_pub,
            &session_pub,
            RESOURCE,
            &leaf_value,
            &session_now,
        )
        .expect("prepared session grant");
    let mut grant: Entry = serde_json::from_value(unsigned_grant).expect("grant entry");
    let grant_preimage = serde_jcs::to_vec(&grant).expect("grant preimage");
    grant.signature.value = hex::encode(delegate_signing.sign(&grant_preimage).to_bytes());
    let authority = runner
        .activate_session_leaf(
            "sales",
            &parent.id,
            &delegate.public_keys().expect("delegate identity").signing,
            &gateway_pub,
            &gateway_kex_pub,
            &session_pub,
            RESOURCE,
            &leaf_value,
            &serde_json::to_value(grant).expect("signed grant"),
            &session_now,
        )
        .expect("active session");

    let prepared = runner
        .prepare_ethos_client_create_for_session(
            &authority.context,
            &authority.leaf_id,
            &authority.session_pub,
            &authority.leaf,
            "circle",
            "",
            "client-provider-proof",
            "written from the Gateway through an operation-scoped working set",
            &session_now,
        )
        .expect("closed Gateway mutation");
    let response: serde_json::Value =
        serde_json::from_str(&prepared.execute().await.expect("Provider mutation"))
            .expect("mutation response");
    assert_eq!(response["path"], "client-provider-proof");
    let created_digest = response["digest"]
        .as_str()
        .expect("created digest")
        .to_owned();

    let edit_now = at(start + 20);
    let prepared_edit = runner
        .prepare_ethos_client_edit_for_session(
            &authority.context,
            &authority.leaf_id,
            &authority.session_pub,
            &authority.leaf,
            "circle",
            "client-provider-proof",
            "edited from the same operation-scoped Gateway path",
            &created_digest,
            &edit_now,
        )
        .expect("closed Gateway edit");
    let edit_response: serde_json::Value =
        serde_json::from_str(&prepared_edit.execute().await.expect("Provider edit"))
            .expect("edit response");
    let edited_digest = edit_response["digest"]
        .as_str()
        .expect("edited digest")
        .to_owned();

    let owner_keys = OwnerKeys::genesis(&MasterSeed::from_bytes(OWNER_SEED));
    let read_now = edit_now.clone();
    let owner_remote = RemoteStore::new(
        &provider_url,
        TENANT,
        genesis.did(),
        Arc::new(KeySigner::owner("#root", owner_keys.root_sign.clone())),
        Arc::new(move || read_now.clone()),
        Box::new(aithos_bundle::entropy::OsEntropy),
    )
    .expect("owner Provider reader");
    let stored = Bundle::open(owner_remote)
        .expect("Provider bundle after Gateway mutation")
        .read_section(Zone::Circle, "client-provider-proof", &owner_keys)
        .expect("owner reads Gateway mutation");
    assert_eq!(stored, "edited from the same operation-scoped Gateway path");

    let delete_now = at(start + 30);
    let prepared_delete = runner
        .prepare_ethos_client_delete_for_session(
            &authority.context,
            &authority.leaf_id,
            &authority.session_pub,
            &authority.leaf,
            "circle",
            "client-provider-proof",
            Some(&edited_digest),
            &delete_now,
        )
        .expect("closed Gateway delete");
    let delete_response: serde_json::Value =
        serde_json::from_str(&prepared_delete.execute().await.expect("Provider delete"))
            .expect("delete response");
    assert_eq!(delete_response["deleted"], true);

    let owner_keys = OwnerKeys::genesis(&MasterSeed::from_bytes(OWNER_SEED));
    let final_now = delete_now;
    let final_remote = RemoteStore::new(
        &provider_url,
        TENANT,
        genesis.did(),
        Arc::new(KeySigner::owner("#root", owner_keys.root_sign.clone())),
        Arc::new(move || final_now.clone()),
        Box::new(aithos_bundle::entropy::OsEntropy),
    )
    .expect("final owner Provider reader");
    let deleted = Bundle::open(final_remote)
        .expect("Provider bundle after Gateway delete")
        .read_section(Zone::Circle, "client-provider-proof", &owner_keys);
    assert!(
        deleted.is_err(),
        "the deleted section must no longer resolve"
    );
}
