//! P3 — the remote journal, end to end against the REAL provider
//! service (in-process, real localhost socket, nothing mocked on the
//! wire). Two proofs, one per §3.5 mode:
//!
//! - **mode B (provider-primary)**: the journal is owner-initialized
//!   locally (the owner's machine), replicated once by the OWNER onto
//!   the provider (did.json genesis first, artifacts, gamma, manifest
//!   publish last), then the gateway opens it `store: remote` and
//!   writes a memory note THROUGH the wire — every read and write is a
//!   signed A.2 envelope under the memory-pen chain, and the note is
//!   re-read from the provider by an independent owner reader.
//!
//! - **mode A (local-primary + réplique)**: the same journal shape on
//!   `store: replicated` — the note lands on fs first, the asynchronous
//!   post-append sweep pushes it to the provider (POST /gamma via the
//!   diff base), and the provider's copy converges byte-exact.
//!
//! The DEMO-LEA replay rides on these mechanics (the P3 gate); this
//! test pins the wire seam itself.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use aithos_bundle::entropy::EntropySource;
use aithos_bundle::remote::{KeySigner, RemoteStore, SharedRemoteStore};
use aithos_bundle::{FsStore, Store};
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_gateway::config::{StoreConfig, ToolAccess};
use aithos_gateway::core_bridge::{self, Bridge, MandateWindow};
use aithos_gateway::hub::{
    approve_manifest, ProposedManifest, ProposedTool, ToolApproval, MANIFEST_VERSION,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::store_adapter::GatewayStore;
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::MemObjects;
use aithos_provider::service::{build_router, AppState};
use aithos_provider::time::render_rfc3339z;

const MASTER: [u8; 32] = [0x42; 32];
const TENANT: &str = "acme";

// ------------------------------------------------------------- helpers

/// Deterministic, salted test entropy (each consumer must mint distinct
/// nonces — A.2 #6).
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
        // Unique, deterministic bytes — a test needs distinct nonces and
        // ids, never cryptographic quality.
        let mut out = Vec::new();
        while out.len() < buf.len() {
            self.counter += 1;
            let mut block = [0u8; 16];
            block[..8].copy_from_slice(&self.salt.to_be_bytes());
            block[8..].copy_from_slice(&self.counter.to_be_bytes());
            out.extend_from_slice(&block);
        }
        buf.copy_from_slice(&out[..buf.len()]);
    }
}

fn real_now() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    render_rfc3339z(ms - ms.rem_euclid(1000))
}

/// Boot the REAL store service on a localhost socket, the given DID
/// enrolled for the tenant (did.json arrives by WIRE, never seeded).
async fn boot_service(did: &str) -> (String, u16) {
    let bootstrap = serde_json::json!({
        "tenants": [{ "tenant": TENANT, "dids": [{ "did": did }] }],
    });
    let (control, _preloads, _seeds) =
        ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("bootstrap");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
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
    let router = build_router(state);
    tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });
    (format!("http://127.0.0.1:{port}"), port)
}

/// An owner-signed client on the service (the owner's own machine).
fn owner_client(url: &str, did: &str, owner: &OwnerKeys, fragment: &str) -> RemoteStore {
    let sk = match fragment {
        "#root" => owner.root_sign.clone(),
        _ => owner.content_sign.clone(),
    };
    RemoteStore::new(
        url,
        TENANT,
        did,
        Arc::new(KeySigner::owner(fragment, sk)),
        Arc::new(real_now),
        Box::new(SaltedEntropy::fresh()),
    )
    .expect("owner client")
}

/// The OWNER replicates a local store onto the provider through the
/// production history-replay seam. Calling it again must publish only
/// editions newer than the provider head.
fn owner_replicate(local_root: &std::path::Path, url: &str, did: &str, owner: &OwnerKeys) {
    let mut root_client = owner_client(url, did, owner, "#root");
    aithos_gateway::store_adapter::replicate_owner_history(local_root, &mut root_client)
        .expect("owner replication");
}

fn pubs(keyholder: &Keyholder) -> (String, String) {
    let agent = ed25519_dalek::SigningKey::from_bytes(&[0x11; 32]);
    let gateway = ed25519_dalek::SigningKey::from_bytes(&[0x22; 32]);
    let _ = keyholder;
    (
        aithos_core::wire::ed25519_pub_to_multibase(&agent.verifying_key().to_bytes()),
        aithos_core::wire::ed25519_pub_to_multibase(&gateway.verifying_key().to_bytes()),
    )
}

struct Setup {
    tmp: tempfile::TempDir,
    url: String,
    did: String,
    owner: OwnerKeys,
    memory_mandate: String,
    keyholder_seed: ([u8; 32], [u8; 32]),
}

/// Owner-initialize a journal locally, enroll its DID, replicate it to
/// the provider — the shared prologue of both modes.
async fn setup() -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let journal_root = tmp.path().join("journal");
    let keyholder = Keyholder::from_entropy([0x11; 32], [0x22; 32]);
    let (agent_pub, gateway_pub) = pubs(&keyholder);
    let now = real_now();
    let start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let window = MandateWindow {
        not_before: render_rfc3339z(start * 1000),
        not_after: render_rfc3339z((start + 30 * 86_400) * 1000),
    };
    let outcome = core_bridge::owner_init_journal(
        &MASTER,
        "lea",
        &agent_pub,
        &gateway_pub,
        None,
        GatewayStore::from_config(&StoreConfig::Fs {
            root: journal_root.clone(),
        })
        .unwrap(),
        &window,
        &now,
        &mut SaltedEntropy::fresh(),
    )
    .expect("owner-init-journal");
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(aithos_core::derive::derive_key(
        "aithos-gw/v1/journal/lea",
        &MASTER,
    )));
    let (url, _port) = boot_service(&outcome.ethos_did).await;
    let root = journal_root.clone();
    let (url2, did2, owner2) = (url.clone(), outcome.ethos_did.clone(), owner_clone());
    tokio::task::spawn_blocking(move || owner_replicate(&root, &url2, &did2, &owner2))
        .await
        .unwrap();
    Setup {
        tmp,
        url,
        did: outcome.ethos_did,
        owner,
        memory_mandate: outcome.memory_mandate.expect("memory pen minted"),
        keyholder_seed: ([0x11; 32], [0x22; 32]),
    }
}

fn owner_clone() -> OwnerKeys {
    OwnerKeys::genesis(&MasterSeed::from_bytes(aithos_core::derive::derive_key(
        "aithos-gw/v1/journal/lea",
        &MASTER,
    )))
}

fn copy_store(source: &std::path::Path, destination: &std::path::Path) {
    let source = FsStore::new(source.to_path_buf());
    let mut destination = FsStore::new(destination.to_path_buf());
    for path in source.list("").expect("source list") {
        let bytes = source
            .get(&path)
            .expect("source get")
            .expect("listed source object");
        destination.put(&path, &bytes).expect("destination put");
    }
}

/// Independent owner reader over the wire — the re-read proof.
fn remote_reader(setup: &Setup) -> GatewayStore {
    GatewayStore::Remote {
        remote: SharedRemoteStore::new(owner_client(
            &setup.url,
            &setup.did,
            &setup.owner,
            "#content",
        )),
        binding_remote: None,
        sidecar: aithos_gateway::store_adapter::Sidecar::Fs(setup.tmp.path().join("journal")),
    }
}

// ------------------------------------------------------------ the tests

#[tokio::test(flavor = "multi_thread")]
async fn mode_b_journal_writes_through_the_wire_and_rereads() {
    let setup = setup().await;
    let (agent_seed, gateway_seed) = setup.keyholder_seed;
    let store = GatewayStore::from_config_with_identity(
        &StoreConfig::Remote {
            url: setup.url.clone(),
            tenant: TENANT.into(),
            did: setup.did.clone(),
            mandate: vec![setup.memory_mandate.clone()],
            // The hybrid sidecar (arbitrage 2026-07-21): the runner's
            // own keys stay on ITS disk — here the owner-init output.
            local: Some(setup.tmp.path().join("journal")),
        },
        &Keyholder::from_entropy(agent_seed, gateway_seed),
        || Box::new(SaltedEntropy::fresh()),
    )
    .expect("remote journal store");

    // The gateway opens the journal and writes one memory note — every
    // byte of it over the signed wire (mode B: the provider IS the
    // primary; there is no local copy to fall back on).
    let note = tokio::task::spawn_blocking(move || {
        let mut bridge = Bridge::open(
            store,
            Arc::new(Keyholder::from_entropy(agent_seed, gateway_seed)),
            Box::new(SaltedEntropy::fresh()),
        )
        .expect("bridge opens over the wire");
        bridge
            .journal_write(
                "Note distante",
                &["memo".into()],
                "écrite à travers le wire A.2",
                &real_now(),
            )
            .expect("journal_write over the wire")
    })
    .await
    .unwrap();
    assert!(!note.name.is_empty());

    // Re-read from the provider by an INDEPENDENT owner reader: the
    // note is there, sealed, and the gamma carries the delegated
    // section.add — nothing lived only in process memory.
    let reader = remote_reader(&setup);
    let notes = tokio::task::spawn_blocking(move || {
        core_bridge::journal_notes_view(reader).expect("notes readable from the provider")
    })
    .await
    .unwrap();
    assert!(
        notes.iter().any(|n| n.title == "Note distante"),
        "the note was served by the provider: {notes:?}"
    );
    drop(setup.tmp);
}

#[tokio::test(flavor = "multi_thread")]
async fn mode_a_replicates_the_appended_note_asynchronously() {
    let setup = setup().await;
    let (agent_seed, gateway_seed) = setup.keyholder_seed;
    let journal_root = setup.tmp.path().join("journal");
    let store = GatewayStore::from_config_with_identity(
        &StoreConfig::Replicated {
            root: journal_root.clone(),
            url: setup.url.clone(),
            tenant: TENANT.into(),
            did: setup.did.clone(),
            mandate: vec![setup.memory_mandate.clone()],
        },
        &Keyholder::from_entropy(agent_seed, gateway_seed),
        || Box::new(SaltedEntropy::fresh()),
    )
    .expect("replicated journal store");

    let store_for_join = store.clone();
    tokio::task::spawn_blocking(move || {
        let mut bridge = Bridge::open(
            store,
            Arc::new(Keyholder::from_entropy(agent_seed, gateway_seed)),
            Box::new(SaltedEntropy::fresh()),
        )
        .expect("bridge opens on the fs primary");
        bridge
            .journal_write(
                "Note répliquée",
                &["memo".into()],
                "primaire fs, réplique asynchrone",
                &real_now(),
            )
            .expect("journal_write on the primary");
        // The deliberate drain: wait out the in-flight sweep(s).
        store_for_join.join_replication();
    })
    .await
    .unwrap();

    // The provider converged: the same note re-read from the wire.
    let reader = remote_reader(&setup);
    let notes = tokio::task::spawn_blocking(move || {
        core_bridge::journal_notes_view(reader).expect("notes readable from the provider")
    })
    .await
    .unwrap();
    assert!(
        notes.iter().any(|n| n.title == "Note répliquée"),
        "the mode A sweep pushed the note: {notes:?}"
    );
    drop(setup.tmp);
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_context_pulls_a_new_owner_binding_without_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let owner_root = tmp.path().join("owner-context");
    let gateway_root = tmp.path().join("gateway-context");
    let keyholder = Keyholder::from_entropy([0x11; 32], [0x22; 32]);
    let (agent_pub, gateway_pub) = pubs(&keyholder);
    let now = real_now();
    let start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let window = MandateWindow {
        not_before: render_rfc3339z((start - 60) * 1000),
        not_after: render_rfc3339z((start + 30 * 86_400) * 1000),
    };
    let owner_store = || {
        GatewayStore::from_config(&StoreConfig::Fs {
            root: owner_root.clone(),
        })
        .unwrap()
    };
    let mut entropy = SaltedEntropy::fresh();
    let did =
        core_bridge::owner_init_context(&MASTER, "operations", owner_store(), &now, &mut entropy)
            .expect("context genesis");
    let equipped = core_bridge::owner_grant_context(
        &MASTER,
        "operations",
        &agent_pub,
        &gateway_pub,
        &["github__get_me".to_owned()],
        owner_store(),
        &window,
        &now,
        &mut entropy,
    )
    .expect("gateway context grant");
    copy_store(&owner_root, &gateway_root);
    assert!(
        FsStore::new(gateway_root.clone())
            .get("e/x/notes-live/header.json")
            .unwrap()
            .is_none(),
        "gateway starts before the Owner publishes the binding"
    );

    let (url, _) = boot_service(&did).await;
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(aithos_core::derive::derive_key(
        "aithos-gw/v1/context/operations",
        &MASTER,
    )));
    let owner_root_for_first_publish = owner_root.clone();
    let url_for_first_publish = url.clone();
    let did_for_first_publish = did.clone();
    let owner_for_first_publish = OwnerKeys::genesis(&MasterSeed::from_bytes(
        aithos_core::derive::derive_key("aithos-gw/v1/context/operations", &MASTER),
    ));
    tokio::task::spawn_blocking(move || {
        owner_replicate(
            &owner_root_for_first_publish,
            &url_for_first_publish,
            &did_for_first_publish,
            &owner_for_first_publish,
        )
    })
    .await
    .unwrap();

    let schema = serde_json::json!({
        "type": "object",
        "properties": { "query": { "type": "string" } },
        "additionalProperties": false,
    });
    let pin =
        core_bridge::manifest_tool_pin("search", Some("Search notes"), &schema).expect("tool pin");
    let proposed = ProposedManifest {
        version: MANIFEST_VERSION.to_owned(),
        server: "notes-live".to_owned(),
        tools: vec![ProposedTool {
            name: "search".to_owned(),
            description: Some("Search notes".to_owned()),
            input_schema: schema,
            pin_sha256: pin,
        }],
    };
    let approved = approve_manifest(
        &proposed,
        &BTreeMap::from([(
            "search".to_owned(),
            ToolApproval::granted(ToolAccess::Write),
        )]),
    )
    .expect("Owner TOFU approval");
    core_bridge::owner_enroll_server(
        &MASTER,
        "operations",
        &agent_pub,
        &gateway_pub,
        &approved,
        owner_store(),
        &window,
        &real_now(),
        &mut entropy,
    )
    .expect("Owner binding publication");

    let owner_root_for_update = owner_root.clone();
    let url_for_update = url.clone();
    let did_for_update = did.clone();
    tokio::task::spawn_blocking(move || {
        owner_replicate(
            &owner_root_for_update,
            &url_for_update,
            &did_for_update,
            &owner,
        )
    })
    .await
    .unwrap();

    let agent_mandate = equipped.agent_mandate;
    let replicated = GatewayStore::from_config_with_identity(
        &StoreConfig::Replicated {
            root: gateway_root.clone(),
            url: url.clone(),
            tenant: TENANT.into(),
            did: did.clone(),
            mandate: vec![agent_mandate.clone()],
        },
        &keyholder,
        || Box::new(SaltedEntropy::fresh()),
    )
    .expect("replicated context");
    let refresh_store = replicated.clone();
    tokio::task::spawn_blocking(move || {
        refresh_store
            .refresh_connector_binding("notes-live")
            .expect("hot binding refresh")
    })
    .await
    .unwrap();

    let reopened = tokio::task::spawn_blocking(move || {
        Bridge::open(
            replicated,
            Arc::new(Keyholder::from_entropy([0x11; 32], [0x22; 32])),
            Box::new(SaltedEntropy::fresh()),
        )
        .expect("bridge remains openable after refresh")
        .verified_hub_manifest("notes-live")
        .expect("refreshed binding verifies")
    })
    .await
    .unwrap();
    assert_eq!(reopened, approved);

    // The deployed demo currently uses provider-primary mode. Its normal
    // store identity is still the pre-binding agent mandate; the targeted
    // refresh and subsequent reads must therefore use the stable gateway
    // governance identity from the local sidecar.
    let remote = GatewayStore::from_config_with_identity(
        &StoreConfig::Remote {
            url,
            tenant: TENANT.into(),
            did,
            mandate: vec![agent_mandate],
            local: Some(gateway_root),
        },
        &keyholder,
        || Box::new(SaltedEntropy::fresh()),
    )
    .expect("provider-primary context");
    remote
        .refresh_connector_binding("notes-live")
        .expect("provider-primary hot binding refresh");
    let remote_manifest = Bridge::open(
        remote,
        Arc::new(Keyholder::from_entropy([0x11; 32], [0x22; 32])),
        Box::new(SaltedEntropy::fresh()),
    )
    .expect("provider-primary bridge remains openable")
    .verified_hub_manifest("notes-live")
    .expect("provider-primary binding verifies");
    assert_eq!(remote_manifest, approved);
}

// The Mutex import anchors the shared-state pattern used above.
#[allow(unused)]
fn _keep(_: &Mutex<()>) {}
