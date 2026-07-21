//! BDD acceptance harness for `tests/features/witness/witness-service.feature`
//! — the annexe C service (lot A / P5): the REAL [`WitnessService`] over
//! the REAL `MemObjects` layout and the in-memory feed, signing with the
//! p4 vector's witness key (the KMS impl is the same seam, proven at the
//! deployed gate). Manifests are the committed p2 chain (m1/m2/m2b — the
//! fork pair IS the equivocation fixture). Instants are injected — the
//! C.2 idempotence and the daily root are clock-driven, never wall-clock,
//! never a sleep.

use std::sync::{Arc, OnceLock};

use aithos_provider::objects::{MemObjects, ObjectStore};
use aithos_provider::witness::{
    is_equivocation, verify_checkpoint, verify_daily_root, verify_keys_doc, Checkpoint, DailyRoot,
    LocalWitnessSigner, WitnessKeyRegistry, WitnessKeys,
};
use aithos_provider::witness_service::{FeedStore, HeadsEvent, MemFeed, WitnessService};
use cucumber::{given, then, when, World as _};
use ed25519_dalek::SigningKey;

// ------------------------------------------------------------- fixtures

struct Fixtures {
    witness_seed: [u8; 32],
    tenant: String,
    did: String,
    m1_jcs: String,
    m1_hash: String,
    m2_jcs: String,
    m2_hash: String,
    m2b_jcs: String,
    m2b_hash: String,
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let vectors = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");
        let p4: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{vectors}/p4-witness-checkpoint.json")).unwrap(),
        )
        .unwrap();
        let p2: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{vectors}/p2-store-cas.json")).unwrap(),
        )
        .unwrap();
        let s = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap().to_owned();
        Fixtures {
            witness_seed: hex::decode(s(&p4, "witness_sk_hex"))
                .unwrap()
                .try_into()
                .unwrap(),
            tenant: s(&p2, "tenant"),
            did: s(&p2, "did"),
            m1_jcs: s(&p2["manifests"], "m1_jcs"),
            m1_hash: s(&p2["manifests"], "m1_chain_hash"),
            m2_jcs: s(&p2["manifests"], "m2_jcs"),
            m2_hash: s(&p2["manifests"], "m2_chain_hash"),
            m2b_jcs: s(&p2["manifests"], "m2b_jcs"),
            m2b_hash: s(&p2["manifests"], "m2b_chain_hash"),
        }
    })
}

/// The second replay DID of the sweep scenarios — a distinct row key
/// serving the SAME committed manifest bytes (the witness signs what the
/// row keys and the layout stores; the checkpoint's `did` is the row's).
const SECOND_DID: &str = "did:aithos:zSecondReplayDidRowKey";

// --------------------------------------------------------------- world

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct WitnessWorld {
    objects: Arc<MemObjects>,
    feed: Arc<MemFeed>,
    service: WitnessService,
    /// The heads rows the heartbeat sweep reads: (tenant, did, height, m).
    sweep_rows: Vec<(String, String, u64, String)>,
    second_did: bool,
}

impl std::fmt::Debug for WitnessWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WitnessWorld")
    }
}

impl WitnessWorld {
    fn new() -> Self {
        let f = fixtures();
        let objects = Arc::new(MemObjects::new());
        let feed = Arc::new(MemFeed::new());
        let service = WitnessService::new(
            Box::new(LocalWitnessSigner::new(SigningKey::from_bytes(
                &f.witness_seed,
            ))),
            objects.clone() as Arc<dyn ObjectStore>,
            feed.clone() as Arc<dyn FeedStore>,
        );
        Self {
            objects,
            feed,
            service,
            sweep_rows: Vec::new(),
            second_did: false,
        }
    }

    async fn put_manifest(&self, did: &str, jcs: &str) {
        self.objects
            .put(
                &fixtures().tenant,
                did,
                "manifest.json",
                jcs.as_bytes().to_vec(),
            )
            .await
            .unwrap();
    }

    async fn feed_lines(&self, did: &str) -> Vec<Checkpoint> {
        match self.feed.get(&format!("{did}.jsonl")).await.unwrap() {
            None => Vec::new(),
            Some((bytes, _)) => String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect(),
        }
    }

    async fn raw_lines(&self, did: &str) -> Vec<String> {
        match self.feed.get(&format!("{did}.jsonl")).await.unwrap() {
            None => Vec::new(),
            Some((bytes, _)) => String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect(),
        }
    }

    fn registry(&self) -> WitnessKeyRegistry {
        WitnessKeyRegistry::from([self.service.witness_key()])
    }

    fn event(&self, height: u64, hash: &str, old_height: Option<u64>) -> HeadsEvent {
        let f = fixtures();
        HeadsEvent {
            tenant: f.tenant.clone(),
            did: f.did.clone(),
            old_height,
            height,
            manifest_chain_hash: hash.to_owned(),
        }
    }
}

// ---------------------------------------------------------- background

#[given(expr = "a witness service over an in-memory feed signing with the p4 witness key")]
async fn given_service(world: &mut WitnessWorld) {
    // The world builds it; the background also publishes keys.json, the
    // boot gesture of the binary.
    world.service.publish_keys().await.unwrap();
}

#[given(expr = "the store layout holds the p2 manifest chain for tenant {string}")]
async fn given_layout(world: &mut WitnessWorld, tenant: String) {
    let f = fixtures();
    assert_eq!(tenant, f.tenant, "the fixture tenant is the p2 vector's");
    world.put_manifest(&f.did, &f.m1_jcs).await;
    world.sweep_rows = vec![(f.tenant.clone(), f.did.clone(), 1, f.m1_hash.clone())];
}

#[given(expr = "the store layout also holds the second replay DID at height 1")]
async fn given_second_did(world: &mut WitnessWorld) {
    let f = fixtures();
    world.put_manifest(SECOND_DID, &f.m1_jcs).await;
    world.sweep_rows.push((
        f.tenant.clone(),
        SECOND_DID.to_owned(),
        1,
        f.m1_hash.clone(),
    ));
    world.second_did = true;
}

// --------------------------------------------------------------- when

#[when(expr = "the heads stream delivers the publish of height {int} at {string}")]
async fn when_publish(world: &mut WitnessWorld, height: u64, at: String) {
    let f = fixtures();
    let (jcs, hash) = match height {
        1 => (&f.m1_jcs, &f.m1_hash),
        2 => (&f.m2_jcs, &f.m2_hash),
        other => panic!("no fixture manifest at height {other}"),
    };
    world.put_manifest(&f.did, jcs).await;
    let event = world.event(height, hash, height.checked_sub(1).filter(|h| *h > 0));
    world.service.on_event(&event, &at).await.unwrap();
}

#[when(expr = "the heads stream delivers the conflicting publish of height {int} at {string}")]
async fn when_conflicting(world: &mut WitnessWorld, height: u64, at: String) {
    let f = fixtures();
    assert_eq!(height, 2, "the committed fork pair is m2/m2b");
    // The competing store head (the C.4 fork): the layout now serves m2b.
    world.put_manifest(&f.did, &f.m2b_jcs).await;
    let event = world.event(2, &f.m2b_hash, Some(1));
    world.service.on_event(&event, &at).await.unwrap();
}

#[when(expr = "the heads stream delivers a gamma-only advance at {string}")]
async fn when_gamma_only(world: &mut WitnessWorld, at: String) {
    let f = fixtures();
    // A gamma append rewrites the row without advancing the edition:
    // old height == new height.
    let event = world.event(1, &f.m1_hash, Some(1));
    let outcome = world.service.on_event(&event, &at).await.unwrap();
    assert_eq!(
        outcome,
        aithos_provider::witness_service::Observed::NotAnEdition
    );
}

#[when(expr = "the heads stream delivers a publish whose manifest is missing at {string}")]
async fn when_missing(world: &mut WitnessWorld, at: String) {
    let f = fixtures();
    // Height 2 announced, but the layout still serves m1 — and for the
    // "missing" flavor the manifest object is absent entirely.
    world
        .objects
        .put(&f.tenant, &f.did, "manifest.json", Vec::new())
        .await
        .unwrap();
    let event = world.event(2, &f.m2_hash, Some(1));
    world.service.on_event(&event, &at).await.unwrap();
}

#[when(expr = "the heads stream delivers a publish whose stored manifest mismatches at {string}")]
async fn when_mismatch(world: &mut WitnessWorld, at: String) {
    let f = fixtures();
    // The layout serves m1 while the row announces the m2 head — the A.5
    // crash window (heads ahead of the object).
    let event = world.event(2, &f.m2_hash, Some(1));
    world.service.on_event(&event, &at).await.unwrap();
}

#[when(expr = "the missing manifest is deposited in the store layout")]
async fn when_heal(world: &mut WitnessWorld) {
    let f = fixtures();
    world.put_manifest(&f.did, &f.m2_jcs).await;
}

#[when(expr = "the pending sweep runs at {string}")]
async fn when_sweep(world: &mut WitnessWorld, at: String) {
    world.service.sweep_pending(&at).await.unwrap();
}

#[when(expr = "the daily heartbeat runs at {string}")]
async fn when_heartbeat(world: &mut WitnessWorld, at: String) {
    let rows = world.sweep_rows.clone();
    world.service.heartbeat(&rows, &at).await.unwrap();
}

#[when(expr = "the root sweep runs at {string}")]
async fn when_root_sweep(world: &mut WitnessWorld, at: String) {
    // The sweep the binary runs at boot AND at every tick (verdict D1):
    // seals every finished day found in the feeds, never the running one.
    world
        .service
        .publish_missing_roots(&at[..10])
        .await
        .unwrap();
}

#[when(expr = "the day rolls over at {string}")]
async fn when_rollover(world: &mut WitnessWorld, at: String) {
    // The binary's rollover tick publishes the root of the ENDED day.
    let day = &at[..10];
    let ended = match day {
        "2026-07-17" => "2026-07-16",
        other => panic!("no fixture rollover for {other}"),
    };
    world.service.publish_daily_root(ended).await.unwrap();
}

// --------------------------------------------------------------- then

#[then(expr = "the DID feed has exactly {int} line(s)")]
async fn then_feed_count(world: &mut WitnessWorld, count: usize) {
    assert_eq!(world.feed_lines(&fixtures().did).await.len(), count);
}

#[then(expr = "the DID feed stays empty")]
async fn then_feed_empty(world: &mut WitnessWorld) {
    assert!(world.feed_lines(&fixtures().did).await.is_empty());
}

#[then(expr = "each replay DID feed has exactly {int} line(s)")]
async fn then_each_feed_count(world: &mut WitnessWorld, count: usize) {
    assert!(world.second_did, "the scenario seeds the second DID");
    assert_eq!(world.feed_lines(&fixtures().did).await.len(), count);
    assert_eq!(world.feed_lines(SECOND_DID).await.len(), count);
}

#[then(expr = "the last feed line is a checkpoint of height {int} for the replay DID")]
async fn then_last_checkpoint(world: &mut WitnessWorld, height: u64) {
    let lines = world.feed_lines(&fixtures().did).await;
    let last = lines.last().expect("a feed line");
    assert_eq!(last.edition_height, height);
    assert_eq!(last.did, fixtures().did);
}

#[then(expr = "the checkpoint's manifest_hash is the observed manifest's chain hash")]
async fn then_manifest_hash(world: &mut WitnessWorld) {
    let lines = world.feed_lines(&fixtures().did).await;
    let last = lines.last().expect("a feed line");
    assert_eq!(last.manifest_hash, format!("sha256:{}", fixtures().m1_hash));
}

#[then(expr = "the checkpoint's gamma_head is copied from the observed manifest")]
async fn then_gamma_head(world: &mut WitnessWorld) {
    let f = fixtures();
    let manifest: aithos_bundle::manifest::Manifest = serde_json::from_str(&f.m1_jcs).unwrap();
    let lines = world.feed_lines(&f.did).await;
    assert_eq!(
        lines.last().expect("a feed line").gamma_head,
        manifest.gamma_head
    );
}

#[then(expr = "the last feed line verifies under the published key registry")]
async fn then_verifies(world: &mut WitnessWorld) {
    let lines = world.feed_lines(&fixtures().did).await;
    let last = lines.last().expect("a feed line");
    assert!(verify_checkpoint(last, &world.registry()));
    // Outside the published registry the same line is not evidence.
    assert!(!verify_checkpoint(last, &WitnessKeyRegistry::new()));
}

#[then(expr = "both feed lines are checkpoints of height {int} with the same manifest_hash")]
async fn then_both_same(world: &mut WitnessWorld, height: u64) {
    let lines = world.feed_lines(&fixtures().did).await;
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|c| c.edition_height == height));
    assert_eq!(lines[0].manifest_hash, lines[1].manifest_hash);
}

#[then(expr = "the last checkpoint's observed_at is {string}")]
async fn then_observed_at(world: &mut WitnessWorld, at: String) {
    let lines = world.feed_lines(&fixtures().did).await;
    assert_eq!(lines.last().expect("a feed line").observed_at, at);
}

#[then(expr = "the two feed lines are freshness, never an equivocation")]
async fn then_freshness(world: &mut WitnessWorld) {
    let lines = world.feed_lines(&fixtures().did).await;
    assert_eq!(lines.len(), 2);
    assert!(!is_equivocation(&lines[0], &lines[1], &world.registry()));
}

#[then(expr = "the observation is left pending for the next sweep")]
async fn then_pending(world: &mut WitnessWorld) {
    assert_eq!(world.service.pending_count(), 1);
}

#[then(expr = "the verifier finds an equivocation from the feed lines alone")]
async fn then_equivocation(world: &mut WitnessWorld) {
    let lines = world.feed_lines(&fixtures().did).await;
    let reg = world.registry();
    let pair: Vec<&Checkpoint> = lines.iter().filter(|c| c.edition_height == 2).collect();
    assert_eq!(pair.len(), 2, "the fork pair is in the public feed");
    assert!(is_equivocation(pair[0], pair[1], &reg));
}

#[then(expr = "the daily root for {string} is published")]
async fn then_root_published(world: &mut WitnessWorld, date: String) {
    assert!(world
        .feed
        .get(&format!("roots/{date}.json"))
        .await
        .unwrap()
        .is_some());
}

#[then(expr = "the daily root's n equals the day's distinct feed lines")]
async fn then_root_n(world: &mut WitnessWorld) {
    let (bytes, _) = world
        .feed
        .get("roots/2026-07-16.json")
        .await
        .unwrap()
        .unwrap();
    let root: DailyRoot = serde_json::from_slice(&bytes).unwrap();
    let mut lines = world.raw_lines(&fixtures().did).await;
    if world.second_did {
        lines.extend(world.raw_lines(SECOND_DID).await);
    }
    let mut day: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("\"observed_at\":\"2026-07-16T"))
        .collect();
    day.sort();
    day.dedup();
    assert_eq!(root.n, day.len() as u64);
    assert!(root.n >= 2, "the scenario observes more than one DID");
}

#[then(expr = "the daily root verifies under the published key registry")]
async fn then_root_verifies(world: &mut WitnessWorld) {
    let (bytes, _) = world
        .feed
        .get("roots/2026-07-16.json")
        .await
        .unwrap()
        .unwrap();
    let root: DailyRoot = serde_json::from_slice(&bytes).unwrap();
    assert!(verify_daily_root(&root, &world.registry()));
}

#[then(expr = "rebuilding the daily root from the day's feed lines is byte-identical")]
async fn then_root_byte_identical(world: &mut WitnessWorld) {
    let (bytes, _) = world
        .feed
        .get("roots/2026-07-16.json")
        .await
        .unwrap()
        .unwrap();
    let day_lines = world.service.day_lines("2026-07-16").await.unwrap();
    let rebuilt = aithos_provider::witness::build_daily_root(
        &aithos_provider::witness::LocalWitnessSigner::new(SigningKey::from_bytes(
            &fixtures().witness_seed,
        )),
        "2026-07-16",
        &day_lines,
    );
    assert_eq!(serde_jcs::to_string(&rebuilt).unwrap().into_bytes(), bytes);
}

#[then(expr = "keys.json is published and lists the witness key")]
async fn then_keys_published(world: &mut WitnessWorld) {
    let (bytes, _) = world.feed.get("keys.json").await.unwrap().unwrap();
    let doc: WitnessKeys = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc.keys, vec![world.service.witness_key()]);
}

#[then(expr = "keys.json verifies under its own witness key")]
async fn then_keys_verify(world: &mut WitnessWorld) {
    let (bytes, _) = world.feed.get("keys.json").await.unwrap().unwrap();
    let doc: WitnessKeys = serde_json::from_slice(&bytes).unwrap();
    assert!(verify_keys_doc(&doc));
    // A tampered registry never verifies.
    let mut tampered = doc.clone();
    tampered.keys.push("z6MkNotARealKey".into());
    assert!(!verify_keys_doc(&tampered));
}

fn main() {
    futures::executor::block_on(
        WitnessWorld::cucumber()
            .fail_on_skipped()
            .run_and_exit(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/features/witness"
            )),
    );
}
