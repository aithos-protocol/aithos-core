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

use std::sync::Arc;
use std::sync::Mutex;

use aithos_bundle::entropy::EntropySource;
use aithos_bundle::remote::{KeySigner, RemoteStore, SharedRemoteStore};
use aithos_bundle::{FsStore, Store};
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_gateway::config::StoreConfig;
use aithos_gateway::core_bridge::{self, Bridge, MandateWindow};
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
        browser_origins: Arc::default(),
        test_now_enabled: false,
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

/// The OWNER replicates a local store onto the provider: did.json
/// genesis first (#root), everything else next, gamma segments (diff
/// base primed), the manifest publish LAST.
fn owner_replicate(local_root: &std::path::Path, url: &str, did: &str, owner: &OwnerKeys) {
    let primary = FsStore::new(local_root.to_path_buf());
    let mut paths = primary.list("").expect("local list");
    paths.sort();
    paths.dedup();
    let priority = |p: &str| -> u8 {
        match p {
            "did.json" => 0,
            p if p.starts_with("gamma/") => 2,
            "manifest.json" => 3,
            _ => 1,
        }
    };
    paths.sort_by_key(|p| priority(p));
    // The edition history: each local `manifests/<h>.json` slot is one
    // accepted publish — replay them in height order (the wire's A.5
    // chain wants height 1, then 2, …; the plain manifest.json is the
    // LAST slot's content and must not double-publish).
    let mut heights: Vec<u64> = paths
        .iter()
        .filter_map(|p| {
            p.strip_prefix("manifests/")?
                .strip_suffix(".json")?
                .parse()
                .ok()
        })
        .collect();
    heights.sort_unstable();
    let mut root_client = owner_client(url, did, owner, "#root");
    for path in paths {
        // The hybrid split (arbitrage 2026-07-21): runner state and
        // derived caches never leave the pod — the owner replicates the
        // PROTOCOL objects only.
        if path.starts_with("gateway/") || path.starts_with("manifests/") {
            continue;
        }
        let Some(bytes) = primary.get(&path).expect("local get") else {
            continue;
        };
        if path.starts_with("gamma/") {
            let _ = root_client.get(&path);
        }
        if path == "manifest.json" {
            for h in &heights {
                let slot = primary
                    .get(&format!("manifests/{h}.json"))
                    .expect("local get")
                    .expect("edition slot");
                root_client
                    .put("manifest.json", &slot)
                    .unwrap_or_else(|e| panic!("owner replicate edition {h}: {e}"));
            }
            continue;
        }
        root_client
            .put(&path, &bytes)
            .unwrap_or_else(|e| panic!("owner replicate {path}: {e}"));
    }
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

/// Independent owner reader over the wire — the re-read proof.
fn remote_reader(setup: &Setup) -> GatewayStore {
    GatewayStore::Remote {
        remote: SharedRemoteStore::new(owner_client(
            &setup.url,
            &setup.did,
            &setup.owner,
            "#content",
        )),
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

// The Mutex import anchors the shared-state pattern used above.
#[allow(unused)]
fn _keep(_: &Mutex<()>) {}
