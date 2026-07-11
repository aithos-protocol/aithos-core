//! BDD acceptance harness (cucumber-rs). Gherkin features live at the repo
//! root in `features/`; step definitions grow with each phase of
//! docs/EXECUTION-PLAN.md and are never rewritten, only extended.

use aithos_bundle::bundle::{Bundle, SectionSpec, ZoneIndex};
use aithos_bundle::entropy::{EntropySource, SeqEntropy};
use aithos_bundle::grants::GrantSpec;
use aithos_bundle::log::{LogFilter, LogHit};
use aithos_bundle::manifest::{sha256_hex, Manifest};
use aithos_bundle::{MemStore, Store};
use aithos_core::derive::{derive_key, node_key, section_label};
use aithos_core::did::{DidDocument, EpochTransition};
use aithos_core::header::{Header, Line, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::keys::ed2x;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{verify_chain, Mandate, PerimeterEntry};
use aithos_core::path::{NodePath, Zone};
use aithos_core::wire;
use cucumber::{given, then, when, World};
use ed25519_dalek::SigningKey;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

// --- step D fixtures ---
const NOW: &str = "2026-07-09T00:00:00Z";
const BODY: &str = "Le corps de la note, ephemere et precieux.";
const PUB_BODY: &str = "Bio publique, lisible par le monde entier.";
const SELF_BODY: &str = "Souvenir intime, jamais signe.";

// --- step E fixtures: mandates ---
const NB: &str = "2026-07-01T00:00:00Z";
const NA7: &str = "2026-07-08T00:00:00Z";
const NA30: &str = "2026-07-31T00:00:00Z";
const DAY1: &str = "2026-07-02T00:00:00Z";
const DAY8: &str = "2026-07-09T00:00:00Z";

fn agent_sk(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}
const AGENT: u8 = 0xA1;
const HELPER: u8 = 0xA2;
const FOURTH: u8 = 0xA3;

fn dir_spec(dir: &str) -> GrantSpec {
    GrantSpec {
        zone: Zone::Circle,
        dir: dir.to_owned(),
        tag: None,
    }
}

fn tag_spec(dir: &str, tag: &str) -> GrantSpec {
    GrantSpec {
        zone: Zone::Circle,
        dir: dir.to_owned(),
        tag: Some(tag.to_owned()),
    }
}

fn sid(n: u128) -> Sid {
    Sid(ulid::Ulid::from(n))
}

// --- step C fixtures: header seals (behavioral; byte-exactness lives in C1) ---
const DID_C: &str = "did:aithos:test-header";
const NODE_A: &str = "/e/circle";
const NODE_OTHER: &str = "/e/self";
const CHILD_NODE: &str = "/e/circle/d/00000000000000000000000001";
const DK: [u8; 32] = [0x77; 32];
const DK2: [u8; 32] = [0x66; 32];
const PARENT_KEY: [u8; 32] = [0x55; 32];

fn xsk(b: u8) -> StaticSecret {
    StaticSecret::from([b; 32])
}
fn owner_rec() -> Recipient {
    Recipient::owner(XPublicKey::from(&xsk(0x0A)))
}
fn grantee_rec(name: &str, b: u8) -> Recipient {
    Recipient {
        to: name.to_owned(),
        kid: name.to_owned(),
        pubkey: XPublicKey::from(&xsk(b)),
    }
}
fn eph(i: u8) -> [u8; 32] {
    [0x40 + i; 32]
}
fn non(i: u8) -> [u8; 24] {
    [0x60 + i; 24]
}

const BUNDLE: &str = "file://local";
const REVOCATIONS: &str = "gamma/gamma.jsonl";

/// One derived identity, as its public keys (hex), in a fixed order:
/// root_sign, content_sign, owner_kex (§01.1).
type PublicIdentity = Vec<String>;

fn public_identity(keys: &OwnerKeys) -> PublicIdentity {
    vec![
        hex::encode(keys.root_sign.verifying_key().to_bytes()),
        hex::encode(keys.content_sign.verifying_key().to_bytes()),
        hex::encode(keys.owner_kex_pub().to_bytes()),
    ]
}

#[derive(Debug, Default, World)]
pub struct ProtocolWorld {
    seeds: Vec<Vec<u8>>,
    identities: Vec<PublicIdentity>,
    rejection: Option<String>,
    /// Entropy injected for succession keypairs (the core never draws RNG).
    succession_entropy: Vec<[u8; 32]>,
    succession_pubs: Vec<String>,
    did_doc: Option<DidDocument>,
    prev_doc: Option<DidDocument>,
    next_doc: Option<DidDocument>,
    transition: Option<Result<(), String>>,
    // --- step B: derivation ---
    zone_dk: Option<[u8; 32]>,
    deep_path: Option<NodePath>,
    node_keys: Vec<[u8; 32]>,
    folder_key: Option<[u8; 32]>,
    // --- step C: headers ---
    header: Option<Header>,
    saved_line: Option<Line>,
    opened: Vec<Result<[u8; 32], String>>,
    wrap_obj: Option<Wrap>,
    // --- step D: bundle ---
    bundle: Option<Bundle<MemStore>>,
    ent: SeqEntropy,
    read_body: Option<Result<String, String>>,
    inspected: String,
    // --- step E: mandates ---
    chain: Vec<Mandate>,
    helper_chain: Vec<Mandate>,
    chain_result: Option<Result<(), String>>,
    granted_folder: String,
    e_folders: Vec<String>,
    // --- step F: gamma ---
    gamma_result: Option<Result<String, String>>,
    audit_chain: Vec<Mandate>,
    query_hits: Option<Result<Vec<LogHit>, String>>,
    sealed_probe: Vec<Result<aithos_core::gamma::Body, String>>,
    // --- step F+: constraints ---
    gamma_baseline: usize,
    receipt: Option<serde_json::Value>,
    // --- step G: revocation ---
    survivor_chain: Vec<Mandate>,
    holder_chain: Vec<Mandate>,
    revoked_at: String,
    g_result: Option<Result<String, String>>,
    // --- step G+: obligations ---
    gplus_checks: Option<serde_json::Value>,
    sib_chains: Vec<Vec<Mandate>>,
    // --- step H1: merkle ---
    h_proof: Option<aithos_core::merkle::Proof>,
    h_old_proof: Option<aithos_core::merkle::Proof>,
    // --- step H2: gamma roots ---
    h2_proof: Option<aithos_core::merkle::Proof>,
    h2_counters: Vec<(String, aithos_core::gamma::GammaCounters)>,
    h2_verdict: Option<Result<(), String>>,
    // --- step I: concurrency ---
    i_other: Option<Bundle<MemStore>>,
    i_result: Option<Result<(), String>>,
    i_hashes: Vec<String>,
    i_surfaced: Option<Result<Vec<String>, String>>,
}

impl ProtocolWorld {
    fn grant_to_agent(&mut self, specs: &[GrantSpec], na: &str, issue_depth: u32) {
        let owner = self.owner(0);
        let mandate = self
            .bundle
            .as_mut()
            .unwrap()
            .grant(
                &owner,
                "agent",
                &agent_sk(AGENT).verifying_key(),
                specs,
                NB,
                na,
                issue_depth,
                &mut self.ent,
            )
            .expect("grant succeeds");
        self.chain = vec![mandate];
    }

    fn verify_chain_at(&self, chain: &[Mandate], at: &str) -> Result<(), String> {
        let doc = self
            .bundle
            .as_ref()
            .unwrap()
            .store
            .get("did.json")
            .unwrap()
            .unwrap();
        let doc: aithos_core::did::DidDocument = serde_json::from_slice(&doc).unwrap();
        verify_chain(chain, &doc, at).map_err(|e| e.to_string())
    }

    fn agent_reads(&self, chain: &[Mandate], sk_byte: u8, path: &str) -> Result<String, String> {
        self.bundle
            .as_ref()
            .unwrap()
            .read_section_as_agent(chain, &agent_sk(sk_byte), Zone::Circle, path, DAY1)
            .map_err(|e| e.to_string())
    }

    fn add_named_section(&mut self, folder: &str, name: &str, tags: &[String]) {
        let owner = self.owner(0);
        let bundle = self.bundle.as_mut().unwrap();
        bundle
            .ensure_folder(Zone::Circle, folder, &owner, &mut self.ent)
            .unwrap();
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: folder,
                    name,
                    title: "note",
                    tags,
                    body: BODY,
                    now: NOW,
                },
                &owner,
                &mut self.ent,
            )
            .unwrap();
    }
}

impl ProtocolWorld {
    fn init_bundle(&mut self) {
        self.seeds.push((0u8..32).collect());
        let owner = self.owner(0);
        let succession = succession_from_entropy([9u8; 32]);
        self.bundle = Some(
            Bundle::init(
                MemStore::default(),
                &owner,
                &succession.verifying_key(),
                &mut self.ent,
                NOW,
            )
            .expect("bundle init"),
        );
    }

    fn add_circle_section(&mut self, folder: &str, name: &str, tag: &str) {
        let owner = self.owner(0);
        let bundle = self.bundle.as_mut().unwrap();
        bundle
            .ensure_folder(Zone::Circle, folder, &owner, &mut self.ent)
            .unwrap();
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: folder,
                    name,
                    title: "note",
                    tags: &[tag.to_owned()],
                    body: BODY,
                    now: NOW,
                },
                &owner,
                &mut self.ent,
            )
            .unwrap();
    }

    fn publish_bundle(&mut self) {
        let owner = self.owner(0);
        self.bundle.as_mut().unwrap().publish(&owner, NOW).unwrap();
    }

    fn latest_manifest(&self) -> Manifest {
        let bytes = self
            .bundle
            .as_ref()
            .unwrap()
            .store
            .get("manifest.json")
            .unwrap()
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}

impl ProtocolWorld {
    fn open_into(&mut self, version: u64, kid: &str, sk_byte: u8) {
        let r = self
            .header
            .as_ref()
            .unwrap()
            .open(DID_C, version, kid, &xsk(sk_byte))
            .map_err(|e| e.to_string());
        self.opened.push(r);
    }
}

impl ProtocolWorld {
    fn derive_from(&mut self, seed_index: usize) {
        let seed = MasterSeed::from_slice(&self.seeds[seed_index]).expect("valid seed");
        self.identities
            .push(public_identity(&OwnerKeys::genesis(&seed)));
    }

    fn owner(&self, seed_index: usize) -> OwnerKeys {
        let seed = MasterSeed::from_slice(&self.seeds[seed_index]).expect("valid seed");
        OwnerKeys::genesis(&seed)
    }

    fn build_doc(&self, seed_index: usize, entropy_index: usize) -> DidDocument {
        let owner = self.owner(seed_index);
        let succession = succession_from_entropy(self.succession_entropy[entropy_index]);
        DidDocument::build(
            &owner,
            &succession.verifying_key(),
            vec![BUNDLE.to_owned()],
            REVOCATIONS.to_owned(),
        )
        .expect("DID document builds")
    }
}

// ---------------------------------------------------------------- givens

#[given("a master seed")]
fn a_master_seed(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
}

#[given("two different master seeds")]
fn two_master_seeds(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
    w.seeds.push((100u8..132).collect());
}

#[given("a 31-byte seed candidate")]
fn a_short_seed(w: &mut ProtocolWorld) {
    w.seeds.push(vec![7u8; 31]);
}

#[given("a master seed and a succession keypair")]
fn seed_and_succession(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
    w.succession_entropy.push([9u8; 32]);
}

#[given("a signed DID document")]
fn a_signed_did_document(w: &mut ProtocolWorld) {
    seed_and_succession(w);
    w.did_doc = Some(w.build_doc(0, 0));
}

#[given("an identity and its successor identity")]
fn identity_and_successor(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
    w.seeds.push((100u8..132).collect());
    w.succession_entropy.push([9u8; 32]);
    w.succession_entropy.push([11u8; 32]);
    w.prev_doc = Some(w.build_doc(0, 0));
    w.next_doc = Some(w.build_doc(1, 1));
}

#[given("a zone key")]
fn a_zone_key(w: &mut ProtocolWorld) {
    w.zone_dk = Some([0xAB; 32]);
}

#[given("a path of three nested folders ending in a section")]
#[given("a folder three levels deep containing a section")]
fn a_deep_path(w: &mut ProtocolWorld) {
    w.deep_path = Some(NodePath::section(
        Zone::Circle,
        vec![sid(1), sid(2), sid(3)],
        sid(7),
    ));
}

#[given("two sibling folders each containing a section")]
fn sibling_folders(_w: &mut ProtocolWorld) {
    // Fixed sids (folder 1 / section 7, folder 2 / section 8), used below.
}

#[given("a zone key and a folder containing a section")]
fn zone_folder_section(w: &mut ProtocolWorld) {
    a_zone_key(w);
    w.deep_path = Some(NodePath::section(Zone::Circle, vec![sid(1)], sid(7)));
    // Key BEFORE the rename.
    w.node_keys
        .push(node_key(&w.zone_dk.unwrap(), w.deep_path.as_ref().unwrap()));
}

#[given("a zone key and a folder")]
fn zone_and_folder(w: &mut ProtocolWorld) {
    a_zone_key(w);
    w.deep_path = Some(NodePath::folder(Zone::Circle, vec![sid(1)]));
}

#[given("a node key and two recipients, the owner and a grantee")]
fn dk_and_two_recipients(_w: &mut ProtocolWorld) {
    // Fixed fixtures: DK, owner (0x0A), grantee g1 (0x21).
}

#[given("a sealed header for the owner and a grantee")]
fn sealed_header_owner_grantee(w: &mut ProtocolWorld) {
    w.header = Some(
        Header::build(
            DID_C,
            NODE_A,
            &DK,
            &[owner_rec(), grantee_rec("g1", 0x21)],
            &[eph(1), eph(2)],
            &[non(1), non(2)],
        )
        .unwrap(),
    );
}

#[given("a sealed header for the owner on one node")]
#[given("a sealed header for the owner")]
fn sealed_header_owner_only(w: &mut ProtocolWorld) {
    let header = Header::build(DID_C, NODE_A, &DK, &[owner_rec()], &[eph(1)], &[non(1)]).unwrap();
    w.saved_line = Some(header.key_versions["1"].lines[0].clone());
    w.header = Some(header);
}

#[given("a node key and a single grantee recipient")]
fn single_grantee(_w: &mut ProtocolWorld) {}

#[given("a sealed header for the owner and two grantees")]
fn sealed_header_three(w: &mut ProtocolWorld) {
    w.header = Some(
        Header::build(
            DID_C,
            NODE_A,
            &DK,
            &[
                owner_rec(),
                grantee_rec("g1", 0x21),
                grantee_rec("g2", 0x22),
            ],
            &[eph(1), eph(2), eph(3)],
            &[non(1), non(2), non(3)],
        )
        .unwrap(),
    );
}

#[given("a derived node rotated to a fresh random key")]
fn derived_node_rotated(_w: &mut ProtocolWorld) {
    // Fixtures: parent key PARENT_KEY, child CHILD_NODE rotated to DK2 v2.
}

#[given("a fresh identity")]
fn a_fresh_identity(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
}

#[given("an initialised bundle")]
fn an_initialised_bundle(w: &mut ProtocolWorld) {
    w.init_bundle();
}

#[given("a published bundle")]
#[given("a bundle with two editions")]
fn a_published_bundle(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets/perso", "note1", "toto");
    w.publish_bundle();
}

#[given(expr = "a published bundle with section {string} in circle {string}")]
fn published_with_section(w: &mut ProtocolWorld, name: String, folder: String) {
    w.init_bundle();
    w.add_circle_section(&folder, &name, "toto");
    w.publish_bundle();
}

#[given(expr = "a published bundle with a public section {string} in folder {string}")]
fn published_public(w: &mut ProtocolWorld, name: String, folder: String) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .section_add(
            &SectionSpec {
                zone: Zone::Public,
                folder_path: &folder,
                name: &name,
                title: "bio",
                tags: &[],
                body: PUB_BODY,
                now: NOW,
            },
            &owner,
            &mut w.ent,
        )
        .unwrap();
    w.publish_bundle();
}

#[given(expr = "a bundle with a self folder {string} containing section {string}")]
fn bundle_with_self(w: &mut ProtocolWorld, folder: String, name: String) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .section_add(
            &SectionSpec {
                zone: Zone::Self_,
                folder_path: &folder,
                name: &name,
                title: "cicatrice au genou",
                tags: &["sante".to_owned()],
                body: SELF_BODY,
                now: NOW,
            },
            &owner,
            &mut w.ent,
        )
        .unwrap();
    w.publish_bundle();
}

// --- step E givens ---

#[given("an owner and an agent keypair")]
fn owner_and_agent(w: &mut ProtocolWorld) {
    w.init_bundle();
}

#[given(expr = "a mandate whose kex_pubkey does not match its signing key")]
fn mandate_bad_kex(w: &mut ProtocolWorld) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .ensure_folder(Zone::Circle, "projets", &owner, &mut w.ent)
        .unwrap();
    w.grant_to_agent(&[dir_spec("projets")], NA7, 0);
    let mut m = w.chain[0].clone();
    // Wrong kex, then honestly re-signed by root: only the kex CHECK can catch it.
    m.grantee.kex_pubkey = aithos_core::wire::x25519_pub_to_multibase(
        &ed2x(&agent_sk(HELPER).verifying_key()).to_bytes(),
    );
    m.resign(&owner.root_sign).unwrap();
    w.chain = vec![m];
    w.chain_result = Some(w.verify_chain_at(&w.chain, DAY1));
}

#[given(expr = "circle sections {string} tagged {string} and {string} untagged in folder {string}")]
fn tagged_and_untagged(
    w: &mut ProtocolWorld,
    tagged: String,
    tag: String,
    untagged: String,
    folder: String,
) {
    w.init_bundle();
    w.add_named_section(&folder, &tagged, &[tag]);
    w.add_named_section(&folder, &untagged, &[]);
    w.granted_folder = folder;
}

#[given(expr = "circle sections in sibling folders {string} and {string}")]
#[given(expr = "circle sections in folders {string} and {string}")]
fn sections_in_two_folders(w: &mut ProtocolWorld, f1: String, f2: String) {
    w.init_bundle();
    w.add_named_section(&f1, "note", &[]);
    w.add_named_section(&f2, "note", &[]);
    w.add_named_section("archives", "note", &[]);
    w.e_folders = vec![f1, f2];
}

#[given(expr = "tagged {string} and untagged sections in both {string} and {string}")]
fn tagged_in_both(w: &mut ProtocolWorld, tag: String, f1: String, f2: String) {
    w.init_bundle();
    for f in [&f1, &f2] {
        w.add_named_section(f, "tagged", std::slice::from_ref(&tag));
        w.add_named_section(f, "plain", &[]);
    }
    w.e_folders = vec![f1, f2];
}

#[given(expr = "an agent granted read on circle folder {string} with issue depth 1")]
#[given(expr = "an agent granted read on circle folder {string} for 7 days with issue depth 1")]
fn agent_with_issue(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    w.add_named_section(&folder, "note", &[]);
    // A nested subfolder so delegation on "<folder>/perso" has a real target.
    w.add_named_section(&format!("{folder}/perso"), "note", &[]);
    w.add_named_section("archives", "note", &[]);
    w.grant_to_agent(&[dir_spec(&folder)], NA7, 1);
    w.granted_folder = folder;
}

#[given("a helper at the end of a depth-1 chain")]
fn helper_end_of_chain(w: &mut ProtocolWorld) {
    agent_with_issue(w, "projets".to_owned());
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &w.chain[0].clone(),
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA7,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![w.chain[0].clone(), sub];
}

// ----------------------------------------------------------------- whens

#[when("I derive the owner keys")]
fn derive_once(w: &mut ProtocolWorld) {
    w.derive_from(0);
}

#[when("I derive the owner keys twice")]
fn derive_twice(w: &mut ProtocolWorld) {
    w.derive_from(0);
    w.derive_from(0);
}

#[when("I derive the owner keys from each seed")]
fn derive_from_each(w: &mut ProtocolWorld) {
    for i in 0..w.seeds.len() {
        w.derive_from(i);
    }
}

#[when("I try to derive the owner keys")]
fn try_derive(w: &mut ProtocolWorld) {
    match MasterSeed::from_slice(&w.seeds[0]) {
        Ok(seed) => {
            w.identities
                .push(public_identity(&OwnerKeys::genesis(&seed)));
        }
        Err(e) => w.rejection = Some(e.to_string()),
    }
}

#[when("I generate a succession keypair twice for the same seed")]
fn generate_succession_twice(w: &mut ProtocolWorld) {
    // Owner keys: derived from the seed, twice — must be identical.
    w.derive_from(0);
    w.derive_from(0);
    // Succession: from two independent entropy draws — must differ.
    for entropy in [[1u8; 32], [2u8; 32]] {
        let key = succession_from_entropy(entropy);
        w.succession_pubs
            .push(hex::encode(key.verifying_key().to_bytes()));
    }
}

#[when("I build the DID document")]
fn build_did_document(w: &mut ProtocolWorld) {
    w.did_doc = Some(w.build_doc(0, 0));
}

#[when("one byte of it is altered")]
fn tamper_document(w: &mut ProtocolWorld) {
    let doc = w.did_doc.as_mut().expect("a signed DID document");
    doc.revocations.push('x');
}

#[when("the transition is signed by the succession key")]
fn transition_by_succession(w: &mut ProtocolWorld) {
    let (prev, next) = (w.prev_doc.clone().unwrap(), w.next_doc.clone().unwrap());
    let succession = succession_from_entropy(w.succession_entropy[0]);
    let tr = EpochTransition::sign(
        &succession,
        prev.id.clone(),
        next.id,
        "2026-07-09T00:00:00Z".to_owned(),
    )
    .expect("transition signs");
    w.transition = Some(tr.verify(&prev).map_err(|e| e.to_string()));
}

#[when("the transition is signed by the root key itself")]
fn transition_by_root(w: &mut ProtocolWorld) {
    let (prev, next) = (w.prev_doc.clone().unwrap(), w.next_doc.clone().unwrap());
    let owner = w.owner(0);
    let tr = EpochTransition::sign_with(
        &owner.root_sign,
        "#root",
        prev.id.clone(),
        next.id,
        "2026-07-09T00:00:00Z".to_owned(),
    )
    .expect("transition signs");
    w.transition = Some(tr.verify(&prev).map_err(|e| e.to_string()));
}

#[when("I derive the section key twice")]
fn derive_section_twice(w: &mut ProtocolWorld) {
    let (zone, path) = (w.zone_dk.unwrap(), w.deep_path.clone().unwrap());
    w.node_keys.push(node_key(&zone, &path));
    w.node_keys.push(node_key(&zone, &path));
}

#[when("I derive the keys of two sibling folders")]
fn derive_siblings(w: &mut ProtocolWorld) {
    let zone = w.zone_dk.unwrap();
    for n in [1u128, 2] {
        w.node_keys.push(node_key(
            &zone,
            &NodePath::folder(Zone::Circle, vec![sid(n)]),
        ));
    }
}

#[when("I derive the folder's key from the zone key")]
fn derive_folder_key(w: &mut ProtocolWorld) {
    let folders = w.deep_path.as_ref().unwrap().folders.clone();
    w.folder_key = Some(node_key(
        &w.zone_dk.unwrap(),
        &NodePath::folder(Zone::Circle, folders),
    ));
}

#[when("I hold only the first folder's key")]
fn hold_first_folder(w: &mut ProtocolWorld) {
    w.folder_key = Some(node_key(
        &w.zone_dk.unwrap(),
        &NodePath::folder(Zone::Circle, vec![sid(1)]),
    ));
}

#[when("the folder is renamed")]
fn rename_folder(w: &mut ProtocolWorld) {
    // Names are metadata (§02.2): they are not even an input of the key
    // functions. Re-derive after the "rename" — sids unchanged.
    w.node_keys
        .push(node_key(&w.zone_dk.unwrap(), w.deep_path.as_ref().unwrap()));
}

#[when(expr = "I derive the tag view {string} at the folder and at the zone root")]
fn derive_tag_anchors(w: &mut ProtocolWorld, tag: String) {
    let zone = w.zone_dk.unwrap();
    let folders = w.deep_path.as_ref().unwrap().folders.clone();
    let local = NodePath::tag_view(Zone::Circle, folders.clone(), &tag).unwrap();
    let root = NodePath::tag_view(Zone::Circle, vec![], &tag).unwrap();
    w.node_keys.push(node_key(&zone, &local));
    w.node_keys.push(node_key(&zone, &root));
    w.node_keys
        .push(node_key(&zone, &NodePath::folder(Zone::Circle, folders)));
}

#[when("the node key is sealed into a header")]
fn seal_into_header(w: &mut ProtocolWorld) {
    sealed_header_owner_grantee(w);
}

#[when("a third keypair tries every line")]
fn stranger_tries(w: &mut ProtocolWorld) {
    for kid in ["owner-kex", "g1"] {
        w.open_into(1, kid, 0x99);
    }
}

#[when("one byte of a line's ciphertext is corrupted")]
fn corrupt_line(w: &mut ProtocolWorld) {
    let header = w.header.as_mut().unwrap();
    let kv = header.key_versions.get_mut("1").unwrap();
    let c = &mut kv.lines[0].c;
    let flipped = if c.starts_with('0') { "1" } else { "0" };
    c.replace_range(0..1, flipped);
    w.open_into(1, "owner-kex", 0x0A);
}

#[when("its owner line is replayed on a different node's header")]
fn replay_line_other_node(w: &mut ProtocolWorld) {
    let stolen = w.header.as_ref().unwrap().key_versions["1"].lines[0].clone();
    let mut other =
        Header::build(DID_C, NODE_OTHER, &DK, &[owner_rec()], &[eph(4)], &[non(4)]).unwrap();
    other.key_versions.get_mut("1").unwrap().lines[0] = stolen;
    w.header = Some(other);
    w.open_into(1, "owner-kex", 0x0A);
}

#[when("a header is built without the owner line")]
fn build_without_owner(w: &mut ProtocolWorld) {
    match Header::build(
        DID_C,
        NODE_A,
        &DK,
        &[grantee_rec("g1", 0x21)],
        &[eph(1)],
        &[non(1)],
    ) {
        Ok(_) => panic!("a header without an owner line must be rejected"),
        Err(e) => w.rejection = Some(e.to_string()),
    }
}

#[when("a line for a new grantee is appended")]
fn append_grantee_line(w: &mut ProtocolWorld) {
    w.header
        .as_mut()
        .unwrap()
        .append_line(DID_C, 1, &DK, &grantee_rec("g1", 0x21), eph(5), non(5))
        .unwrap();
}

#[when("the node is rotated without the first grantee")]
fn rotate_without_g1(w: &mut ProtocolWorld) {
    w.header
        .as_mut()
        .unwrap()
        .rotate(
            DID_C,
            2,
            &DK2,
            &[owner_rec(), grantee_rec("g2", 0x22)],
            &[eph(6), eph(7)],
            &[non(6), non(7)],
        )
        .unwrap();
}

#[when("the rotator posts the up-link wrap under the parent key")]
fn post_uplink_wrap(w: &mut ProtocolWorld) {
    w.wrap_obj = Some(Wrap::seal(
        DID_C,
        NODE_A,
        &PARENT_KEY,
        CHILD_NODE,
        2,
        &DK2,
        non(9),
    ));
}

#[when("I initialise its bundle")]
fn initialise_bundle(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let succession = succession_from_entropy([9u8; 32]);
    w.bundle = Some(
        Bundle::init(
            MemStore::default(),
            &owner,
            &succession.verifying_key(),
            &mut w.ent,
            NOW,
        )
        .expect("bundle init"),
    );
}

#[when(expr = "I create circle folder {string} with a section {string} tagged {string}")]
fn create_circle_content(w: &mut ProtocolWorld, folder: String, name: String, tag: String) {
    w.add_circle_section(&folder, &name, &tag);
}

#[when("I publish the edition")]
#[when("the edition is republished")]
fn publish_edition(w: &mut ProtocolWorld) {
    w.publish_bundle();
}

#[when("one byte of a pinned file is altered")]
fn alter_pinned_file(w: &mut ProtocolWorld) {
    let bundle = w.bundle.as_mut().unwrap();
    let mut bytes = bundle.store.get("e/circle/index.json").unwrap().unwrap();
    bytes[10] ^= 1;
    bundle.store.put("e/circle/index.json", &bytes).unwrap();
}

#[when("the newest manifest claims a wrong predecessor hash")]
fn wrong_predecessor(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let latest = w.latest_manifest();
    let forged = Manifest::build(
        &owner.root_sign,
        latest.edition.height + 1,
        "0".repeat(64),
        NOW.to_owned(),
        latest.files.clone(),
        latest.roots.clone(),
        latest.gamma_roots.clone(),
        latest.gamma_counts_root.clone(),
        latest.gamma_head.clone(),
    )
    .unwrap();
    let bundle = w.bundle.as_mut().unwrap();
    let bytes = serde_json::to_vec_pretty(&forged).unwrap();
    bundle
        .store
        .put(&format!("manifests/{}.json", forged.edition.height), &bytes)
        .unwrap();
    bundle.store.put("manifest.json", &bytes).unwrap();
}

#[when(expr = "the owner reads {string} from circle")]
fn owner_reads_circle(w: &mut ProtocolWorld, path: String) {
    let owner = w.owner(0);
    w.read_body = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .read_section(Zone::Circle, &path, &owner)
            .map_err(|e| e.to_string()),
    );
}

#[when(expr = "the folder {string} is renamed to {string}")]
fn rename_the_folder(w: &mut ProtocolWorld, name: String, new_name: String) {
    let owner = w.owner(0);
    let full = format!("projets/{name}");
    w.bundle
        .as_mut()
        .unwrap()
        .rename_folder(Zone::Circle, &full, &new_name, &owner, &mut w.ent)
        .unwrap();
}

#[when(expr = "a stranger with no key reads {string} from public")]
fn stranger_reads_public(w: &mut ProtocolWorld, path: String) {
    // No owner keys anywhere in this step.
    w.read_body = Some(
        Bundle::<MemStore>::public_read(&w.bundle.as_ref().unwrap().store, &path)
            .map_err(|e| e.to_string()),
    );
}

#[when("I inspect every file of the self zone as a stranger")]
fn inspect_self_zone(w: &mut ProtocolWorld) {
    let store = &w.bundle.as_ref().unwrap().store;
    let mut all = String::new();
    for path in store.list("e/self/").unwrap() {
        all.push_str(&String::from_utf8_lossy(
            &store.get(&path).unwrap().unwrap(),
        ));
    }
    w.inspected = all;
}

// --- step E whens ---

#[when(expr = "the owner grants the agent read on circle folder {string} for 7 days")]
#[when(expr = "the owner grants the agent read on circle folder {string}")]
fn grant_on_folder(w: &mut ProtocolWorld, folder: String) {
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .ensure_folder(Zone::Circle, &folder, &owner, &mut w.ent)
        .unwrap();
    w.grant_to_agent(&[dir_spec(&folder)], NA7, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent read on folder {string} restricted to tag {string}")]
fn grant_on_folder_tag(w: &mut ProtocolWorld, folder: String, tag: String) {
    w.grant_to_agent(&[tag_spec(&folder, &tag)], NA7, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent read on folders {string} and {string} in one mandate")]
fn grant_two_folders(w: &mut ProtocolWorld, f1: String, f2: String) {
    w.grant_to_agent(&[dir_spec(&f1), dir_spec(&f2)], NA7, 0);
}

#[when(expr = "the owner grants read on both folders restricted to tag {string} in one mandate")]
fn grant_two_folders_tagged(w: &mut ProtocolWorld, tag: String) {
    let (f1, f2) = (w.e_folders[0].clone(), w.e_folders[1].clone());
    w.grant_to_agent(&[tag_spec(&f1, &tag), tag_spec(&f2, &tag)], NA7, 0);
}

#[when(expr = "the agent delegates read on folder {string} to a helper")]
fn agent_delegates(w: &mut ProtocolWorld, folder: String) {
    let parent = w.chain[0].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA7,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![parent, sub];
    w.chain_result = Some(w.verify_chain_at(&w.helper_chain, DAY1));
}

#[when("the agent delegates the same perimeter to a helper for 30 days")]
fn agent_delegates_too_long(w: &mut ProtocolWorld) {
    let parent = w.chain[0].clone();
    let folder = w.granted_folder.clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![parent, sub];
    w.chain_result = Some(w.verify_chain_at(&w.helper_chain, DAY1));
}

#[when("the helper tries to delegate to a fourth key")]
fn helper_delegates_further(w: &mut ProtocolWorld) {
    let parent = w.helper_chain[1].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(HELPER),
            "fourth",
            &agent_sk(FOURTH).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA7,
            &mut w.ent,
        )
        .unwrap();
    let mut chain = w.helper_chain.clone();
    chain.push(sub);
    w.chain_result = Some(w.verify_chain_at(&chain, DAY1));
}

// --- step E thens ---

#[then("the mandate verifies at day 1")]
fn mandate_ok_day1(w: &mut ProtocolWorld) {
    assert_eq!(w.verify_chain_at(&w.chain, DAY1), Ok(()));
}

#[then("the mandate is rejected at day 8")]
fn mandate_dead_day8(w: &mut ProtocolWorld) {
    assert!(w.verify_chain_at(&w.chain, DAY8).is_err());
}

#[then("mandate verification is rejected")]
fn mandate_rejected(w: &mut ProtocolWorld) {
    let res = w.chain_result.as_ref().unwrap();
    assert!(res.is_err());
    assert!(
        res.as_ref().unwrap_err().contains("kex"),
        "must be rejected FOR the kex binding: {res:?}"
    );
}

#[then(expr = "the agent reads {string} with its own keypair")]
fn agent_reads_path(w: &mut ProtocolWorld, path: String) {
    assert_eq!(w.agent_reads(&w.chain, AGENT, &path).as_deref(), Ok(BODY));
}

#[then(expr = "the agent reads {string}")]
fn agent_reads_in_folder(w: &mut ProtocolWorld, name: String) {
    let path = format!("{}/{name}", w.granted_folder);
    assert_eq!(w.agent_reads(&w.chain, AGENT, &path).as_deref(), Ok(BODY));
}

#[then(expr = "{string} stays out of the agent's reach")]
fn name_out_of_reach(w: &mut ProtocolWorld, name: String) {
    let path = format!("{}/{name}", w.granted_folder);
    assert!(w.agent_reads(&w.chain, AGENT, &path).is_err());
}

#[then(expr = "the agent reads the section under {string}")]
#[then(expr = "the agent reads the section under {string} with its single keypair")]
#[then(expr = "the agent reads the section under {string} with the same keypair")]
fn agent_reads_under(w: &mut ProtocolWorld, folder: String) {
    assert_eq!(
        w.agent_reads(&w.chain, AGENT, &format!("{folder}/note"))
            .as_deref(),
        Ok(BODY)
    );
}

#[then(expr = "the agent cannot read the section under {string}")]
#[then(expr = "a section under {string} stays out of the agent's reach")]
fn agent_blocked_under(w: &mut ProtocolWorld, folder: String) {
    assert!(w
        .agent_reads(&w.chain, AGENT, &format!("{folder}/note"))
        .is_err());
}

#[then("the agent reads the tagged section of each folder with one keypair")]
fn agent_reads_tagged_both(w: &mut ProtocolWorld) {
    for f in w.e_folders.clone() {
        assert_eq!(
            w.agent_reads(&w.chain, AGENT, &format!("{f}/tagged"))
                .as_deref(),
            Ok(BODY),
            "tagged section of {f}"
        );
    }
}

#[then("every untagged section stays out of the agent's reach")]
fn untagged_blocked_both(w: &mut ProtocolWorld) {
    for f in w.e_folders.clone() {
        assert!(
            w.agent_reads(&w.chain, AGENT, &format!("{f}/plain"))
                .is_err(),
            "plain section of {f} must stay sealed"
        );
    }
}

#[then("the helper's chain verifies")]
fn helper_chain_ok(w: &mut ProtocolWorld) {
    assert_eq!(w.chain_result.clone().unwrap(), Ok(()));
}

#[then(expr = "the helper reads the section under {string}")]
fn helper_reads_under(w: &mut ProtocolWorld, folder: String) {
    assert_eq!(
        w.agent_reads(&w.helper_chain, HELPER, &format!("{folder}/note"))
            .as_deref(),
        Ok(BODY)
    );
}

#[then("the helper's chain is rejected")]
#[then("the new chain is rejected")]
fn helper_chain_rejected(w: &mut ProtocolWorld) {
    assert!(w.chain_result.clone().unwrap().is_err());
}

// ----------------------------------------------------------------- thens

#[then("both derivations yield the same public identity")]
fn same_identity(w: &mut ProtocolWorld) {
    assert_eq!(w.identities.len(), 2);
    assert_eq!(w.identities[0], w.identities[1]);
}

#[then("the two identities share no public key")]
fn unrelated_identities(w: &mut ProtocolWorld) {
    assert_eq!(w.identities.len(), 2);
    let shared = w.identities[0]
        .iter()
        .filter(|k| w.identities[1].contains(k))
        .count();
    assert_eq!(shared, 0, "identities must share no public key");
}

#[then("the three public keys are pairwise distinct")]
fn domain_separated(w: &mut ProtocolWorld) {
    let id = &w.identities[0];
    let unique: std::collections::BTreeSet<_> = id.iter().collect();
    assert_eq!(unique.len(), id.len(), "keys must be pairwise distinct");
}

#[then("the two succession keys differ")]
fn succession_keys_differ(w: &mut ProtocolWorld) {
    assert_eq!(w.succession_pubs.len(), 2);
    assert_ne!(w.succession_pubs[0], w.succession_pubs[1]);
}

#[then("the owner keys are identical both times")]
fn owner_keys_identical(w: &mut ProtocolWorld) {
    assert_eq!(w.identities[0], w.identities[1]);
}

#[then("it contains the root, content, kex and succession public keys")]
fn doc_contains_four_keys(w: &mut ProtocolWorld) {
    let doc = w.did_doc.as_ref().expect("a DID document");
    let owner = w.owner(0);
    let succession = succession_from_entropy(w.succession_entropy[0]);
    assert_eq!(
        doc.keys.root,
        wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes())
    );
    assert_eq!(
        doc.keys.content,
        wire::ed25519_pub_to_multibase(&owner.content_sign.verifying_key().to_bytes())
    );
    assert_eq!(
        doc.keys.kex,
        wire::x25519_pub_to_multibase(&owner.owner_kex_pub().to_bytes())
    );
    assert_eq!(
        doc.keys.succession,
        wire::ed25519_pub_to_multibase(&succession.verifying_key().to_bytes())
    );
}

#[then("its identifier is derived from the root public key")]
fn doc_id_from_root(w: &mut ProtocolWorld) {
    let doc = w.did_doc.as_ref().expect("a DID document");
    let root = w.owner(0).root_sign.verifying_key().to_bytes();
    assert_eq!(doc.id, wire::did_aithos(&root));
}

#[then("its signature verifies under the root key")]
fn doc_signature_verifies(w: &mut ProtocolWorld) {
    w.did_doc
        .as_ref()
        .unwrap()
        .verify()
        .expect("valid document");
}

#[then("verification is rejected")]
fn verification_rejected(w: &mut ProtocolWorld) {
    assert!(w.did_doc.as_ref().unwrap().verify().is_err());
}

#[then("the successor DID document is accepted")]
fn successor_accepted(w: &mut ProtocolWorld) {
    assert_eq!(w.transition.as_ref().unwrap(), &Ok(()));
}

#[then("the transition is rejected")]
fn transition_rejected(w: &mut ProtocolWorld) {
    assert!(w.transition.as_ref().unwrap().is_err());
}

#[then("both derivations yield the same key")]
fn same_key(w: &mut ProtocolWorld) {
    assert_eq!(w.node_keys[0], w.node_keys[1]);
}

#[then("the two folder keys are unrelated")]
fn sibling_keys_unrelated(w: &mut ProtocolWorld) {
    assert_ne!(w.node_keys[0], w.node_keys[1]);
}

#[then("the folder key alone derives the section beneath it")]
fn folder_derives_section(w: &mut ProtocolWorld) {
    let path = w.deep_path.as_ref().unwrap();
    let via_zone = node_key(&w.zone_dk.unwrap(), path);
    let aithos_core::path::Leaf::Section(section) = &path.leaf else {
        panic!("path must end in a section");
    };
    let via_folder = derive_key(&section_label(section), &w.folder_key.unwrap());
    assert_eq!(via_folder, via_zone, "no need to touch the zone key again");
}

#[then("no derivation from it yields the second folder's section key")]
fn no_sideways_reach(w: &mut ProtocolWorld) {
    let zone = w.zone_dk.unwrap();
    let target = node_key(
        &zone,
        &NodePath::section(Zone::Circle, vec![sid(2)], sid(8)),
    );
    let from_f1 = w.folder_key.unwrap();
    // Candidate derivations an attacker holding folder 1 could attempt:
    let candidates = [
        derive_key(&section_label(&sid(8)), &from_f1),
        node_key(
            &from_f1,
            &NodePath::section(Zone::Circle, vec![sid(2)], sid(8)),
        ),
        derive_key(&aithos_core::derive::folder_label(&sid(2)), &from_f1),
    ];
    for c in candidates {
        assert_ne!(c, target, "sideways derivation must never reach a sibling");
    }
}

#[then("every derived key is unchanged")]
fn keys_unchanged(w: &mut ProtocolWorld) {
    assert_eq!(w.node_keys[0], w.node_keys[1], "rename must never re-key");
}

#[then("the two anchors differ from each other and from the folder key")]
fn anchors_distinct(w: &mut ProtocolWorld) {
    let unique: std::collections::BTreeSet<_> = w.node_keys.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "local anchor, root anchor, folder key all distinct"
    );
}

#[then("the owner opens the header and recovers the node key")]
fn owner_opens(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 1, "owner-kex", &xsk(0x0A))
        .unwrap();
    assert_eq!(dk, DK);
}

#[then("the grantee opens the header and recovers the node key")]
#[then("the new grantee opens the node key")]
fn grantee_opens(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 1, "g1", &xsk(0x21))
        .unwrap();
    assert_eq!(dk, DK);
}

#[then("it recovers nothing")]
fn stranger_recovers_nothing(w: &mut ProtocolWorld) {
    assert!(!w.opened.is_empty());
    assert!(w.opened.iter().all(Result::is_err));
}

#[then("opening that line is rejected")]
#[then("opening it there is rejected")]
fn opening_rejected(w: &mut ProtocolWorld) {
    assert!(w.opened.last().unwrap().is_err());
}

#[then("the header is rejected as invalid")]
fn header_invalid(w: &mut ProtocolWorld) {
    let msg = w.rejection.as_deref().unwrap();
    assert!(msg.contains("I3"), "rejection must name I3: {msg}");
}

#[then("the owner line is byte-identical to before")]
fn owner_line_untouched(w: &mut ProtocolWorld) {
    let header = w.header.as_ref().unwrap();
    let owner_line = header.key_versions["1"]
        .lines
        .iter()
        .find(|l| l.to == "owner")
        .unwrap();
    assert_eq!(owner_line, w.saved_line.as_ref().unwrap());
}

#[then("the surviving grantee opens the new node key")]
fn survivor_opens(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 2, "g2", &xsk(0x22))
        .unwrap();
    assert_eq!(dk, DK2);
}

#[then("the first grantee cannot open the new version")]
fn revoked_cannot_open(w: &mut ProtocolWorld) {
    assert!(w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 2, "g1", &xsk(0x21))
        .is_err());
}

#[then("the owner opens the new version too")]
fn owner_opens_new(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 2, "owner-kex", &xsk(0x0A))
        .unwrap();
    assert_eq!(dk, DK2);
}

#[then("a parent holder recovers the new node key through the wrap")]
fn parent_recovers_via_wrap(w: &mut ProtocolWorld) {
    let dk = w
        .wrap_obj
        .as_ref()
        .unwrap()
        .open(DID_C, &PARENT_KEY)
        .unwrap();
    assert_eq!(dk, DK2);
}

#[then("edition 1 verifies offline")]
#[then("its integrity checks against the signed edition")]
fn edition_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().verify().expect("edition valid");
}

#[then("the manifest pins the DID document")]
fn manifest_pins_did(w: &mut ProtocolWorld) {
    let manifest = w.latest_manifest();
    let did_bytes = w
        .bundle
        .as_ref()
        .unwrap()
        .store
        .get("did.json")
        .unwrap()
        .unwrap();
    assert_eq!(
        manifest.files.get("did.json").unwrap(),
        &sha256_hex(&did_bytes)
    );
}

#[then("edition 2 verifies and pins edition 1 as its predecessor")]
fn edition_two_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().verify().expect("edition valid");
    let latest = w.latest_manifest();
    assert_eq!(latest.edition.height, 2);
    let first: Manifest = serde_json::from_slice(
        &w.bundle
            .as_ref()
            .unwrap()
            .store
            .get("manifests/1.json")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(latest.edition.prev_hash, first.chain_hash().unwrap());
}

#[then("edition verification is rejected")]
fn edition_rejected(w: &mut ProtocolWorld) {
    assert!(w.bundle.as_ref().unwrap().verify().is_err());
}

#[then("the section body comes back intact")]
fn body_intact(w: &mut ProtocolWorld) {
    assert_eq!(w.read_body.as_ref().unwrap().as_deref(), Ok(BODY));
}

#[then(expr = "the owner reads the same section at {string}")]
fn reads_at_new_path(w: &mut ProtocolWorld, path: String) {
    let owner = w.owner(0);
    let body = w
        .bundle
        .as_ref()
        .unwrap()
        .read_section(Zone::Circle, &path, &owner)
        .expect("readable at the renamed path");
    assert_eq!(body, BODY);
}

#[then("the section body is readable in clear")]
fn public_body_readable(w: &mut ProtocolWorld) {
    assert_eq!(w.read_body.as_ref().unwrap().as_deref(), Ok(PUB_BODY));
}

#[then("no folder name, section name, title or tag appears anywhere")]
fn self_leaks_nothing(w: &mut ProtocolWorld) {
    for needle in [
        "enfance",
        "cicatrices",
        "blessure",
        "cicatrice au genou",
        "sante",
    ] {
        assert!(
            !w.inspected.contains(needle),
            "self zone leaked the string '{needle}'"
        );
    }
}

#[then("the owner still reconstructs the full tree from sealed descriptors")]
fn owner_reconstructs_tree(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let tree = w
        .bundle
        .as_ref()
        .unwrap()
        .zone_tree(Zone::Self_, &owner)
        .unwrap();
    for expected in [
        "enfance",
        "enfance/cicatrices",
        "enfance/cicatrices/blessure",
    ] {
        assert!(
            tree.contains(&expected.to_owned()),
            "missing {expected} in {tree:?}"
        );
    }
}

#[then(expr = "genesis is rejected with {string}")]
fn rejected_with(w: &mut ProtocolWorld, expected: String) {
    let rejection = w
        .rejection
        .as_deref()
        .expect("genesis should have been rejected");
    assert!(
        rejection.contains(&expected),
        "rejection '{rejection}' should mention '{expected}'"
    );
    assert!(
        w.identities.is_empty(),
        "no identity may exist after rejection"
    );
}

// ------------------------------------------------------- step F: gamma ---

const AUDITOR: u8 = 0xA4;
const NA_FAR: &str = "2027-07-01T00:00:00Z";
const D0: &str = "2026-07-01T00:00:00Z"; // "day 0" of the F scenarios

fn day(n: u32, hms: &str) -> String {
    // Days relative to D0 (July 2026 has 31 days) — enough for two months.
    let (mo, d) = if n < 31 { (7, n + 1) } else { (8, n - 30) };
    format!("2026-{mo:02}-{d:02}T{hms}Z")
}

impl ProtocolWorld {
    fn gbundle(&mut self) -> &mut Bundle<MemStore> {
        self.bundle.as_mut().unwrap()
    }

    fn did_document(&self) -> DidDocument {
        let bytes = self
            .bundle
            .as_ref()
            .unwrap()
            .store
            .get("did.json")
            .unwrap()
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn store_cert(&mut self, m: &Mandate) {
        let bytes = serde_json::to_vec_pretty(m).unwrap();
        self.gbundle()
            .store
            .put(&format!("certs/{}.json", m.id), &bytes)
            .unwrap();
    }

    /// Root mandate over connector actions (and optionally more entries),
    /// certificate + stored — the act-plane sibling of `grant_to_agent`.
    fn grant_act(
        &mut self,
        extra: Vec<aithos_core::mandate::PerimeterEntry>,
        constraints: serde_json::Value,
        na: &str,
    ) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        let owner = self.owner(0);
        let mut perimeter = vec![PerimeterEntry::parse("act.x.gmail.*").unwrap()];
        perimeter.extend(extra);
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 900)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter,
                constraints,
                not_before: NB.into(),
                not_after: na.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    /// Sub-mandate over an action pattern, minted by the AGENT for HELPER;
    /// logging the grant is the caller's (spec-mandated) duty.
    fn delegate_act(
        &mut self,
        pattern: &str,
        constraints: serde_json::Value,
        log_grant: bool,
    ) -> Result<String, String> {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        let parent = self.chain[0].clone();
        let child = M::build_sub(
            &parent,
            &agent_sk(AGENT),
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 950)),
                subject: parent.subject.clone(),
                grantee_id: "urn:aithos:agent:helper".into(),
                grantee_label: "helper".into(),
                grantee_pub: &agent_sk(HELPER).verifying_key(),
                perimeter: vec![PerimeterEntry::parse(pattern).unwrap()],
                constraints,
                not_before: NB.into(),
                not_after: parent.not_after.clone(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&child);
        self.helper_chain = vec![parent.clone(), child.clone()];
        if log_grant {
            let mut ent = std::mem::take(&mut self.ent);
            let r = self
                .gbundle()
                .log_delegated_grant(
                    &[parent],
                    &agent_sk(AGENT),
                    &child.id,
                    &day(1, "00:30:00"),
                    &mut ent,
                )
                .map(|()| child.id.clone())
                .map_err(|e| e.to_string());
            self.ent = ent;
            return r;
        }
        Ok(child.id)
    }

    fn try_action(&mut self, helper: bool, action: &str, at: &str) -> Result<String, String> {
        self.try_action_full(helper, action, at, None, None)
    }

    fn try_action_full(
        &mut self,
        helper: bool,
        action: &str,
        at: &str,
        budget: Option<serde_json::Value>,
        sealed_args: Option<serde_json::Value>,
    ) -> Result<String, String> {
        let chain = if helper {
            self.helper_chain.clone()
        } else {
            self.chain.clone()
        };
        let sk = agent_sk(if helper { HELPER } else { AGENT });
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_action(
                &chain,
                &sk,
                &aithos_bundle::log::ActionSpec {
                    connector: "gmail",
                    action,
                    args_hash: "sha256:00",
                    now: at,
                    budget,
                    sealed_args,
                },
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn try_inference(
        &mut self,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        budget_ref: Option<&str>,
        at: &str,
    ) -> Result<String, String> {
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_inference(
                &chain,
                &agent_sk(AGENT),
                &aithos_bundle::log::InferenceSpec {
                    provider: "provider",
                    model,
                    tokens_in,
                    tokens_out,
                    budget_ref,
                    now: at,
                },
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn beacon(&mut self, seq: u64, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.gbundle()
            .log_heartbeat(&owner, seq, at, &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn segment_lines(&mut self, seg: &str) -> Vec<Vec<u8>> {
        self.gbundle()
            .store
            .get(seg)
            .unwrap()
            .unwrap()
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .map(<[u8]>::to_vec)
            .collect()
    }
}

// --- F givens ---

#[given("a bundle with a three-entry log")]
fn three_entry_log(w: &mut ProtocolWorld) {
    w.init_bundle();
    for (i, hms) in ["01:00:00", "02:00:00", "03:00:00"].iter().enumerate() {
        w.beacon(i as u64 + 1, &day(8, hms));
    }
}

#[given("a bundle with entries logged in two different months")]
fn two_month_log(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.beacon(1, D0);
    w.beacon(2, &day(35, "00:00:00"));
    w.publish_bundle();
}

#[given(expr = "an agent granted action rights on connector {string}")]
fn act_grant_connector(w: &mut ProtocolWorld, _connector: String) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
}

#[given("an agent granted action rights for 7 days")]
fn act_grant_7d(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA7);
}

#[given("an agent granted action rights with max_actions 3")]
fn act_grant_budget(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({"max_actions": 3}), NA30);
}

#[given("an agent granted action rights with max_actions 3 and issue depth 1")]
fn act_grant_budget_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"max_actions": 3}),
        NA30,
    );
}

#[given("the agent delegates its perimeter to a helper")]
fn delegates_perimeter(w: &mut ProtocolWorld) {
    w.delegate_act("act.x.gmail.*", serde_json::json!({}), true)
        .unwrap();
}

#[given("an agent granted action rights with max_actions_per 2 per 24 hours")]
fn act_grant_window(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"max_actions_per": {"window": "24h", "n": 2}}),
        NA30,
    );
}

#[given(expr = "an agent granted gmail actions with rate_limit 2 {string} per 72 hours")]
fn act_grant_rate(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"rate_limit": {"action": action, "window": "72h", "n": 2}}),
        NA30,
    );
}

#[given("an agent granted issue rights with max_children 2")]
fn act_grant_children(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"max_children": 2}),
        NA30,
    );
}

#[given("an agent that minted a sub-mandate without logging the grant")]
fn minted_unlogged(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), false)
        .unwrap();
}

#[given("a head mandate with heartbeat every 30 days grace 72 hours")]
fn head_mandate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"heartbeat": {"every": "30d", "grace": "72h"}}),
        NA_FAR,
    );
}

#[given("an owner beacon at day 0")]
fn beacon_day0(w: &mut ProtocolWorld) {
    w.beacon(1, D0);
}

#[given("a head mandate suspended by owner silence")]
fn suspended_mandate(w: &mut ProtocolWorld) {
    head_mandate(w);
    beacon_day0(w);
    // Day 34 sits beyond every+grace (33d): the mandate is suspended.
    w.gamma_result = Some(w.try_action(false, "reply", &day(34, "00:00:00")));
    assert!(w.gamma_result.as_ref().unwrap().is_err());
}

#[given("an agent under a mandate with freshness 24 hours")]
fn freshness_mandate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({"freshness": "24h"}), NA30);
    w.beacon(1, D0); // the anchor entry
}

#[given("a published bundle with a circle section")]
fn published_circle_section(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
}

#[given("a bundle whose log records mutations and actions")]
fn log_with_mutations_and_actions(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto"); // sealed mutation @NOW
    w.grant_act(vec![], serde_json::json!({"max_actions": 5}), NA30);
    w.try_action(false, "reply", &day(14, "01:00:00")).unwrap();
}

#[given(expr = "logged mutations on sections under {string} and under {string}")]
fn mutations_two_folders(w: &mut ProtocolWorld, f1: String, f2: String) {
    w.init_bundle();
    w.add_named_section(&f1, "note1", &[]);
    w.add_named_section(&f2, "note2", &[]);
    w.e_folders = vec![f1, f2];
}

#[given("an agent granted action rights and no read grant")]
fn act_no_read(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto"); // a sealed entry exists
    w.grant_act(vec![], serde_json::json!({}), NA30);
}

#[given("a bundle whose log records mutations and actions over two months")]
fn log_two_months(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA_FAR);
    w.try_action(false, "reply", &day(4, "00:00:00")).unwrap(); // day 4: outside
    w.add_circle_section("projets", "note1", "toto"); // NOW = day 8: mutation
    w.try_action(false, "reply", &day(14, "00:00:00")).unwrap(); // inside
    w.try_action(false, "label", &day(35, "00:00:00")).unwrap(); // inside
}

#[given(expr = "logged mutations by the owner and by an agent under {string}")]
fn mutations_for_audit(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    w.add_named_section(&folder, "note1", &[]);
    w.add_named_section(&folder, "note2", &[]);
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(14, "01:00:00")).unwrap();
}

#[given(expr = "an auditor granted read.gamma on action {string} from day 1 to day 30")]
fn scoped_auditor(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, &action, &day(19, "12:00:00")).unwrap();
    let owner = w.owner(0);
    let entry = format!(
        "read.gamma#action={action}&since={}&until={}",
        day(0, "00:00:00"),
        day(29, "23:59:59")
    );
    let m = Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: "mandate_000000000000000000000AUDIT".into(),
            subject: w.bundle.as_ref().unwrap().did.clone(),
            grantee_id: "urn:aithos:agent:auditor".into(),
            grantee_label: "auditor".into(),
            grantee_pub: &agent_sk(AUDITOR).verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse(&entry).unwrap()],
            constraints: aithos_core::mandate::MandateSpec::no_constraints(),
            not_before: NB.into(),
            not_after: NA30.into(),
            issued_at: NB.into(),
            nonce: hex::encode(w.ent.e16()),
        },
    )
    .unwrap();
    w.store_cert(&m);
    w.audit_chain = vec![m];
}

// --- F whens ---

#[when("the owner appends a section addition and a heartbeat")]
fn owner_appends_both(w: &mut ProtocolWorld) {
    w.add_circle_section("projets", "note1", "toto");
    w.beacon(1, NOW);
    w.publish_bundle();
}

#[when("one byte of the middle entry is altered")]
fn tamper_middle_entry(w: &mut ProtocolWorld) {
    let seg = "gamma/2026-07.jsonl";
    let mut lines = w.segment_lines(seg);
    let line = &mut lines[1];
    let idx = line
        .windows(9)
        .position(|win| win == b"\"value\":\"")
        .expect("signature field")
        + 9;
    line[idx] = if line[idx] == b'0' { b'1' } else { b'0' };
    let mut joined: Vec<u8> = Vec::new();
    for l in &lines {
        joined.extend_from_slice(l);
        joined.push(b'\n');
    }
    w.gbundle().store.put(seg, &joined).unwrap();
}

#[when("an entry is appended whose prev is not the current head")]
fn append_wrong_prev(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let rogue = aithos_core::gamma::owner_entry(
        aithos_core::gamma::EntrySpec {
            id: "gamma_000000000000000000000ROGUE".into(),
            prev: entries[0].chain_hash().unwrap(), // not the head
            prevs: None,
            at: day(8, "04:00:00"),
            kind: aithos_core::gamma::Kind::Heartbeat,
            target: None,
            payload: Some(serde_json::json!({"seq": 99})),
            body_enc: None,
        },
        &owner.content_sign,
    )
    .unwrap();
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap();
    bytes.extend_from_slice(aithos_core::jcs::canonicalize(&rogue).unwrap().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[when("the owner appends a section addition")]
fn owner_appends_section(w: &mut ProtocolWorld) {
    w.add_circle_section("projets", "note1", "toto");
}

#[when("the agent appends an action entry")]
fn agent_appends_action(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", DAY1));
}

#[when("the agent appends an action entry timestamped at day 8")]
fn agent_appends_day8(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", DAY8));
}

#[when("the agent appends three action entries")]
fn agent_appends_three(w: &mut ProtocolWorld) {
    for hms in ["01:00:00", "02:00:00", "03:00:00"] {
        w.try_action(false, "reply", &day(1, hms)).unwrap();
    }
}

#[when("the agent appends one action and the helper appends two")]
fn subtree_appends(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(1, "01:00:00")).unwrap();
    w.try_action(true, "reply", &day(1, "02:00:00")).unwrap();
    w.try_action(true, "reply", &day(1, "03:00:00")).unwrap();
}

#[when("the agent appends two actions on day 1")]
fn two_actions_day1(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(1, "01:00:00")).unwrap();
    w.try_action(false, "label", &day(1, "02:00:00")).unwrap();
}

#[when(expr = "the agent appends two {string} actions on day 1")]
fn two_kind_actions_day1(w: &mut ProtocolWorld, action: String) {
    w.try_action(false, &action, &day(1, "01:00:00")).unwrap();
    w.try_action(false, &action, &day(1, "02:00:00")).unwrap();
}

#[when("the agent delegates twice, each grant logged")]
fn delegates_twice(w: &mut ProtocolWorld) {
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    w.delegate_act("act.x.gmail.label", serde_json::json!({}), true)
        .unwrap();
}

#[when("the helper presents an action under that chain")]
fn helper_presents(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(true, "reply", &day(1, "05:00:00")));
}

#[when("the head agent forges a heartbeat with its own key")]
fn forge_beacon(w: &mut ProtocolWorld) {
    use ed25519_dalek::Signer;
    let head = w.gbundle().gamma_head().unwrap();
    let mut forged = aithos_core::gamma::Entry {
        v: 1,
        id: "gamma_000000000000000000000FORGE".into(),
        prev: head,
        prevs: None,
        at: day(2, "00:00:00"),
        kind: "heartbeat".into(),
        target: None,
        authorized_by: None,
        authorized_via: None,
        payload: Some(serde_json::json!({"seq": 77})),
        body_enc: None,
        signature: aithos_core::did::SignatureBlock {
            alg: "ed25519".into(),
            key: "#content".into(),
            value: String::new(),
        },
    };
    let mut unsigned = forged.clone();
    unsigned.signature.value = String::new();
    forged.signature.value = hex::encode(
        agent_sk(AGENT)
            .sign(&aithos_core::jcs::canonical_bytes(&unsigned).unwrap())
            .to_bytes(),
    );
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap_or_default();
    bytes.extend_from_slice(aithos_core::jcs::canonicalize(&forged).unwrap().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[when("the owner beacons again")]
fn owner_beacons_again(w: &mut ProtocolWorld) {
    w.beacon(2, &day(40, "00:00:00"));
}

#[when("the agent presents a request anchored to the current log head")]
fn present_fresh_anchor(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let head = aithos_core::gamma::head(&entries).unwrap();
    w.gamma_result = Some(
        aithos_core::gamma::check_anchor(&entries, &head, "24h", &day(0, "12:00:00"))
            .map(|()| "fresh".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("the agent presents a request anchored to a head 48 hours old")]
fn present_stale_anchor(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let head = aithos_core::gamma::head(&entries).unwrap();
    w.gamma_result = Some(
        aithos_core::gamma::check_anchor(&entries, &head, "24h", &day(2, "00:00:00"))
            .map(|()| "fresh".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("the owner logs a modification of that section")]
fn owner_logs_modification(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_section_modify(
            &owner,
            Zone::Circle,
            "projets/note1",
            serde_json::json!({"change": "reworded"}),
            &day(9, "00:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when("someone with no key reads the log files")]
fn stranger_reads(_w: &mut ProtocolWorld) {
    // Nothing to acquire: verification below runs on public files alone.
}

impl ProtocolWorld {
    /// Try to open every sealed entry body with the agent's keys (physics
    /// probe, scenario "A subtree grant opens exactly its entries").
    fn probe_sealed_entries(&mut self) {
        if !self.sealed_probe.is_empty() {
            return;
        }
        let entries = self.gbundle().gamma_entries().unwrap();
        let chain = self.chain.clone();
        self.sealed_probe = entries
            .iter()
            .filter(|e| e.body_enc.is_some())
            .map(|e| {
                self.bundle
                    .as_ref()
                    .unwrap()
                    .open_entry_as_agent(&chain, &agent_sk(AGENT), e)
                    .map_err(|e| e.to_string())
            })
            .collect();
    }
}

#[when("the agent appends an action entry knowing only the pinned log head")]
fn blind_append(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", &day(14, "02:00:00")));
}

#[when(expr = "the owner queries actions of kind {string} on {string} from day 10 to day 40")]
fn owner_queries(w: &mut ProtocolWorld, kind: String, _connector: String) {
    let owner = w.owner(0);
    let filter = LogFilter {
        kind: Some(kind),
        since: Some(day(10, "00:00:00")),
        until: Some(day(40, "23:59:59")),
        ..LogFilter::default()
    };
    w.query_hits = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .log_query_owner(&owner, &filter)
            .map_err(|e| e.to_string()),
    );
}

#[when("the owner grants an auditor read.gamma with the zone keys")]
fn grant_auditor(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    // Physics: a zone-root read grant delivers the circle keys.
    w.bundle
        .as_mut()
        .unwrap()
        .grant(
            &owner,
            "auditor",
            &agent_sk(AUDITOR).verifying_key(),
            &[dir_spec("")],
            NB,
            NA30,
            0,
            &mut w.ent,
        )
        .unwrap();
    // Certificate: the read.gamma mandate, full log.
    let m = Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: "mandate_00000000000000000000AUDIT2".into(),
            subject: w.bundle.as_ref().unwrap().did.clone(),
            grantee_id: "urn:aithos:agent:auditor".into(),
            grantee_label: "auditor".into(),
            grantee_pub: &agent_sk(AUDITOR).verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse("read.gamma").unwrap()],
            constraints: aithos_core::mandate::MandateSpec::no_constraints(),
            not_before: NB.into(),
            not_after: NA30.into(),
            issued_at: NB.into(),
            nonce: hex::encode(w.ent.e16()),
        },
    )
    .unwrap();
    w.store_cert(&m);
    w.audit_chain = vec![m];
}

#[when("the auditor queries replies of day 20")]
fn auditor_queries_day20(w: &mut ProtocolWorld) {
    let query = aithos_core::mandate::GammaQuery {
        action: Some("reply".into()),
        since: Some(day(19, "00:00:00")),
        until: Some(day(20, "23:59:59")),
        ..Default::default()
    };
    let filter = LogFilter {
        action: Some("reply".into()),
        since: Some(day(19, "00:00:00")),
        until: Some(day(20, "23:59:59")),
        ..LogFilter::default()
    };
    w.query_hits = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .log_query_as_agent(&w.audit_chain, &agent_sk(AUDITOR), &query, &filter, DAY1)
            .map_err(|e| e.to_string()),
    );
}

// --- F thens ---

#[then("the log verifies offline")]
fn log_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("the manifest pins the log head")]
fn manifest_pins_head(w: &mut ProtocolWorld) {
    let latest = w.latest_manifest();
    let head = w.bundle.as_ref().unwrap().gamma_head().unwrap();
    assert!(!head.is_empty(), "log should not be empty");
    assert_eq!(latest.gamma_head, head, "manifest must pin the log tip");
}

#[then("the log lives in two pinned segment files")]
fn two_segments_pinned(w: &mut ProtocolWorld) {
    let latest = w.latest_manifest();
    let segs: Vec<&String> = latest
        .files
        .keys()
        .filter(|p| p.starts_with("gamma/"))
        .collect();
    assert_eq!(segs.len(), 2, "expected two pinned segments, got {segs:?}");
}

#[then("the whole chain verifies across the boundary")]
fn chain_across_boundary(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("log verification is rejected")]
fn log_verification_rejected(w: &mut ProtocolWorld) {
    if let Some(Err(_)) = &w.gamma_result {
        return; // the append itself was refused — fail-closed upstream
    }
    assert!(
        w.bundle.as_ref().unwrap().gamma_verify().is_err(),
        "log verification should reject"
    );
}

#[then("the entry verifies with no mandate attached")]
fn owner_entry_verifies(w: &mut ProtocolWorld) {
    let doc = w.did_document();
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert!(last.authorized_by.is_none() && last.authorized_via.is_none());
    aithos_core::gamma::verify_owner_entry(last, &doc).unwrap();
}

#[then("the entry verifies against the chain at its own timestamp")]
fn delegated_entry_verifies(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    let doc = w.did_document();
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    aithos_core::gamma::verify_delegated_entry(last, &w.chain, &doc).unwrap();
}

#[then("a fourth action entry is rejected")]
fn fourth_rejected(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "reply", &day(1, "04:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("budget")),
        "expected budget exhaustion, got {r:?}"
    );
}

#[then("a further action by either key is rejected")]
fn either_key_rejected(w: &mut ProtocolWorld) {
    for helper in [false, true] {
        let r = w.try_action(helper, "reply", &day(1, "06:00:00"));
        assert!(
            r.as_ref().is_err_and(|e| e.contains("budget")),
            "helper={helper}: expected budget exhaustion, got {r:?}"
        );
    }
}

#[then("a third action on day 1 is rejected")]
fn third_day1_rejected(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "reply", &day(1, "03:00:00"));
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then("an action on day 2 verifies")]
fn day2_verifies(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(2, "03:00:01")).unwrap();
}

#[then(expr = "a third {string} on day 2 is rejected")]
fn third_kind_rejected(w: &mut ProtocolWorld, action: String) {
    let r = w.try_action(false, &action, &day(2, "01:00:00"));
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then(expr = "a {string} action on day 2 verifies")]
fn kind_day2_verifies(w: &mut ProtocolWorld, action: String) {
    w.try_action(false, &action, &day(2, "02:00:00")).unwrap();
}

#[then("a third delegation is rejected")]
fn third_delegation_rejected(w: &mut ProtocolWorld) {
    let r = w.delegate_act("act.x.gmail.send", serde_json::json!({}), true);
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then("the action is rejected")]
fn action_rejected(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(
        r.as_ref().is_err_and(|e| e.contains("grant never logged")),
        "expected the unlogged-grant rejection, got {r:?}"
    );
}

#[then("an action at day 20 verifies")]
fn action_day20(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(20, "00:00:00")).unwrap();
}

#[then("an action at day 34 is rejected")]
fn action_day34(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "reply", &day(34, "00:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("heartbeat stale")),
        "got {r:?}"
    );
}

#[then("the next action verifies")]
fn next_action_verifies(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(41, "00:00:00")).unwrap();
}

#[then("the beacon is rejected")]
fn beacon_rejected(w: &mut ProtocolWorld) {
    assert!(
        w.bundle.as_ref().unwrap().gamma_verify().is_err(),
        "a forged beacon must fail log verification"
    );
    // And it never counts as liveness: the mandate stays suspended.
    let doc = w.did_document();
    let entries = w.gbundle().gamma_entries().unwrap();
    let ok = entries
        .iter()
        .filter(|e| e.kind == "heartbeat")
        .filter(|e| aithos_core::gamma::verify_owner_entry(e, &doc).is_ok())
        .count();
    assert_eq!(ok, 0, "no verifiable beacon should exist");
}

#[then("the request verifies")]
fn request_verifies(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
}

#[then("the request is rejected")]
fn request_rejected(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(
        r.as_ref().is_err_and(|e| e.contains("stale")),
        "expected a stale anchor, got {r:?}"
    );
}

#[then("a reader of that section opens the entry body")]
fn reader_opens_body(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_owner(
            &owner,
            &LogFilter {
                kind: Some("section.modify".into()),
                ..LogFilter::default()
            },
        )
        .unwrap();
    assert_eq!(hits.len(), 1);
    let body = hits[0].body.as_ref().expect("body opens");
    assert_eq!(body.payload["change"], "reworded");
}

#[then("the entry alone reveals only kind, time and author — not the target")]
fn entry_reveals_nothing(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last.kind, "section.modify");
    assert!(last.target.is_none(), "target must live inside the body");
    assert!(last.payload.is_none(), "payload must be sealed");
    assert!(last.body_enc.is_some());
}

#[then("the chain and the budgets still verify")]
fn skeleton_verifies(w: &mut ProtocolWorld) {
    // Public files alone: chain, signatures, and countable headers.
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
    let entries = w.gbundle().gamma_entries().unwrap();
    let n = aithos_core::gamma::count_actions(&entries, &w.chain[0].id, None, None);
    assert_eq!(n, 1, "the skeleton must stay countable");
}

#[then("no target, tag or content is revealed")]
fn nothing_revealed(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    for e in entries.iter().filter(|e| e.kind.starts_with("section.")) {
        assert!(e.target.is_none() && e.payload.is_none() && e.body_enc.is_some());
    }
}

#[then(expr = "the agent opens the bodies of the {string} entries by their hints")]
fn agent_opens_granted(w: &mut ProtocolWorld, folder: String) {
    w.probe_sealed_entries();
    let opened: Vec<&aithos_core::gamma::Body> = w
        .sealed_probe
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .collect();
    assert_eq!(opened.len(), 1, "exactly the granted subtree opens");
    let dir = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, &folder)
        .unwrap();
    let node = NodePath::parse(&opened[0].target).unwrap();
    assert!(node.folders.starts_with(&dir));
}

#[then(expr = "the {string} entry bodies stay sealed to it")]
fn other_stays_sealed(w: &mut ProtocolWorld, _folder: String) {
    w.probe_sealed_entries();
    let sealed = w.sealed_probe.iter().filter(|r| r.is_err()).count();
    assert_eq!(sealed, 1, "the out-of-perimeter body must not open");
}

#[then("the entry chains and verifies")]
fn entry_chains_verifies(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("exactly the matching entries come back")]
fn matching_come_back(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hits.len(), 2, "day-4 action and the mutation must be out");
    assert!(hits
        .iter()
        .all(|h| h.entry.kind == "action" && h.entry.target.as_deref() == Some("x.gmail")));
}

#[then("the owner opens every sealed body among them")]
fn owner_opens_all(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert!(hits
        .iter()
        .filter(|h| h.entry.body_enc.is_some())
        .all(|h| h.body.is_some()));
}

#[then("the auditor opens every entry body, including acts it never made")]
fn auditor_opens_all(w: &mut ProtocolWorld) {
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_as_agent(
            &w.audit_chain,
            &agent_sk(AUDITOR),
            &aithos_core::mandate::GammaQuery::default(),
            &LogFilter::default(),
            DAY1,
        )
        .unwrap();
    let sealed = hits.iter().filter(|h| h.entry.body_enc.is_some()).count();
    let opened = hits.iter().filter(|h| h.body.is_some()).count();
    assert_eq!(sealed, 2, "both owner mutations are in view");
    assert_eq!(
        opened, sealed,
        "every sealed body must open for the auditor"
    );
}

#[then("the matching entries come back")]
fn scoped_matching(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hits.len(), 1);
}

#[then("a query for day 40 is refused")]
fn day40_refused(w: &mut ProtocolWorld) {
    let query = aithos_core::mandate::GammaQuery {
        action: Some("reply".into()),
        since: Some(day(39, "00:00:00")),
        until: Some(day(40, "23:59:59")),
        ..Default::default()
    };
    let r = w.bundle.as_ref().unwrap().log_query_as_agent(
        &w.audit_chain,
        &agent_sk(AUDITOR),
        &query,
        &LogFilter::default(),
        DAY1,
    );
    assert!(r.is_err(), "an out-of-window query must be refused");
}

#[then(expr = "a query for action {string} is refused")]
fn action_refused(w: &mut ProtocolWorld, action: String) {
    let query = aithos_core::mandate::GammaQuery {
        action: Some(action),
        since: Some(day(19, "00:00:00")),
        until: Some(day(20, "23:59:59")),
        ..Default::default()
    };
    let r = w.bundle.as_ref().unwrap().log_query_as_agent(
        &w.audit_chain,
        &agent_sk(AUDITOR),
        &query,
        &LogFilter::default(),
        DAY1,
    );
    assert!(r.is_err(), "an out-of-perimeter action must be refused");
}

// ------------------------------------------ step F+: advanced constraints ---

const PROVIDER: u8 = 0xC3;

fn win(anchor: &str, duration: &str) -> serde_json::Value {
    serde_json::json!({"anchor": anchor, "duration": duration})
}

fn win_periodic(anchor: &str, duration: &str, period: &str) -> serde_json::Value {
    serde_json::json!({"anchor": anchor, "duration": duration, "period": period})
}

/// The founding two-profile budgets (spec §04.11 example).
fn founding_budgets() -> serde_json::Value {
    serde_json::json!({"budgets": [
        {"id": "haiku", "models": ["claude-haiku"], "token_budget": 10000,
         "active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "7d")],
         "max_actions": 1},
        {"id": "gemma", "models": ["gemma"], "token_budget": 25000}
    ]})
}

fn cite(profile: &str, model: &str, tokens: u64) -> serde_json::Value {
    serde_json::json!({"budget_ref": profile, "model": model, "tokens": tokens})
}

fn provider_receipt(args_hash: &str, model: &str, tokens: u64, signer: u8) -> serde_json::Value {
    use ed25519_dalek::Signer;
    let payload = serde_json::json!({"args_hash": args_hash, "model": model, "tokens": tokens});
    let bytes = aithos_core::jcs::canonical_bytes(&payload).unwrap();
    let sig = hex::encode(agent_sk(signer).sign(&bytes).to_bytes());
    let mut r = payload;
    r["sig"] = serde_json::json!(sig);
    r
}

fn attestation_budgets() -> serde_json::Value {
    let key = wire::ed25519_pub_to_multibase(&agent_sk(PROVIDER).verifying_key().to_bytes());
    serde_json::json!({"budgets": [
        {"id": "haiku", "require_attestation": true, "attestation_key": key}
    ]})
}

// --- F+ givens: windows ---

#[given("an agent granted gmail actions active from day 3 14:00 for 4 hours")]
fn grant_oneshot_window(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [win(&day(3, "14:00:00"), "4h")]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours")]
fn grant_weekly_window(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "7d")]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours until day 20")]
fn grant_weekly_until(w: &mut ProtocolWorld) {
    w.init_bundle();
    let mut window = win_periodic(&day(1, "14:00:00"), "4h", "7d");
    window["until"] = serde_json::json!(day(20, "00:00:00"));
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [window]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours, 2 occurrences")]
fn grant_weekly_count(w: &mut ProtocolWorld) {
    w.init_bundle();
    let mut window = win_periodic(&day(1, "14:00:00"), "4h", "7d");
    window["count"] = serde_json::json!(2);
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [window]}),
        NA30,
    );
}

#[given("an agent granted gmail actions active on day 3 morning and day 5 evening")]
fn grant_union_windows(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [
            win(&day(3, "08:00:00"), "4h"),
            win(&day(5, "18:00:00"), "4h")
        ]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every day at 14:00 for 4 hours with max_actions_per 2 per 24 hours")]
fn grant_daily_with_rolling(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({
            "active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "1d")],
            "max_actions_per": {"window": "24h", "n": 2}
        }),
        NA30,
    );
}

#[given("an agent granted gmail actions active from day 1 to day 20 with issue depth 1")]
fn grant_window_with_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"active_windows": [win(&day(1, "00:00:00"), "19d")]}),
        NA30,
    );
}

#[given("an agent granted gmail actions for 7 days, active daily 14:00 for 4 hours")]
fn grant_7d_daily(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "1d")]}),
        NA7,
    );
}

// --- F+ givens: budgets ---

#[given("a mandate with two budget profiles:")]
#[given("a mandate with two budget profiles")]
#[given("the founding two-profile mandate")]
fn founding_mandate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], founding_budgets(), NA30);
}

#[given(expr = "a profile {string} with a {int} token budget")]
fn single_profile(w: &mut ProtocolWorld, id: String, budget: u64) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"budgets": [{"id": id, "token_budget": budget}]}),
        NA30,
    );
}

#[given(expr = "{int} tokens already consumed on {string}")]
fn tokens_consumed(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[given(expr = "profile {string} has spent its single action")]
fn profile_spent(w: &mut ProtocolWorld, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(cite(&profile, "claude-haiku", 100)),
        None,
    )
    .unwrap();
}

#[given(expr = "a profile {string} allowing model {string}")]
fn profile_allowing(w: &mut ProtocolWorld, id: String, model: String) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"budgets": [{"id": id, "models": [model]}]}),
        NA30,
    );
}

#[given("the founding two-profile mandate with issue depth 1")]
fn founding_with_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    let mut c = founding_budgets();
    c["budgets"].as_array_mut().unwrap(); // keep shape
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        c,
        NA30,
    );
}

#[given("the agent delegates the gemma perimeter to a helper")]
fn delegates_gemma(w: &mut ProtocolWorld) {
    w.delegate_act("act.x.gmail.*", founding_budgets(), true)
        .unwrap();
}

#[given(expr = "logged actions of {int}, {int} and {int} tokens on {string}")]
fn logged_actions_tokens(w: &mut ProtocolWorld, a: u64, b: u64, c: u64, profile: String) {
    for (i, t) in [a, b, c].into_iter().enumerate() {
        w.try_action_full(
            false,
            "reply",
            &day(1, &format!("0{}:00:00", i + 1)),
            Some(serde_json::json!({"budget_ref": profile, "tokens": t})),
            None,
        )
        .unwrap();
    }
}

// --- F+ givens: attestation ---

#[given(expr = "a profile {string} that requires attestation")]
fn profile_requires_attestation(w: &mut ProtocolWorld, _id: String) {
    w.init_bundle();
    w.grant_act(vec![], attestation_budgets(), NA30);
}

#[given("a provider attestation key pinned in the mandate")]
fn provider_key_pinned(_w: &mut ProtocolWorld) {
    // Pinned by the attestation_budgets() fixture above.
}

#[given("a valid receipt for an earlier action's args_hash")]
fn earlier_receipt(w: &mut ProtocolWorld) {
    w.receipt = Some(provider_receipt(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "claude-haiku",
        4000,
        PROVIDER,
    ));
}

// --- F+ givens: inference & kinds ---

#[given(expr = "logged inferences of {int} and {int} total tokens citing {string}")]
fn logged_inferences(w: &mut ProtocolWorld, a: u64, b: u64, profile: String) {
    for (i, t) in [a, b].into_iter().enumerate() {
        w.try_inference(
            "gemma",
            t - 1000,
            1000,
            Some(&profile),
            &day(1, &format!("0{}:00:00", i + 1)),
        )
        .unwrap();
    }
}

#[given(expr = "a logged action of {int} declared tokens citing {string}")]
fn logged_action_tokens(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[given("a bundle whose log records section additions, a modification and actions")]
fn log_with_kinds(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.add_named_section("projets", "note2", &[]);
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_section_modify(
            &owner,
            Zone::Circle,
            "projets/note1",
            serde_json::json!({"change": "x"}),
            &day(9, "00:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(10, "00:00:00")).unwrap();
}

#[given(expr = "an agent granted read on circle folder {string}")]
fn plain_read_grant(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    w.add_named_section(&folder, "note1", &[]);
    w.grant_to_agent(&[dir_spec(&folder)], NA30, 0);
    w.granted_folder = folder;
    w.gamma_baseline = w.gbundle().gamma_entries().unwrap().len();
}

#[given(expr = "an agent granted read on circle folder {string} with log_reads")]
fn read_grant_log_reads(w: &mut ProtocolWorld, folder: String) {
    plain_read_grant(w, folder);
    // Same keypair, the mandate re-issued with the log_reads duty.
    let owner = w.owner(0);
    let mut m = w.chain[0].clone();
    m.constraints = serde_json::json!({"log_reads": true});
    m.resign(&owner.root_sign).unwrap();
    w.store_cert(&m);
    w.chain = vec![m];
}

// --- F+ givens: sealed args ---

#[given(expr = "an agent granted gmail {string} with sealed-args audit")]
fn grant_with_audit(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    let _ = action;
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .grant_audit_line(&owner, &agent_sk(AGENT).verifying_key(), &mut ent)
        .unwrap();
    w.ent = ent;
}

#[given("a logged action with sealed args")]
fn logged_sealed_action(w: &mut ProtocolWorld) {
    grant_with_audit(w, "reply".to_owned());
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": "alice@example.com"})),
    )
    .unwrap();
}

#[given(expr = "a mandate whose action_params allow replies only to {string}")]
fn mandate_action_params(w: &mut ProtocolWorld, addr: String) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"action_params": {"reply": {"recipients_allow": [addr]}}}),
        NA30,
    );
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .grant_audit_line(&owner, &agent_sk(AGENT).verifying_key(), &mut ent)
        .unwrap();
    w.ent = ent;
}

#[given(expr = "a logged reply whose sealed args name {string}")]
fn logged_reply_naming(w: &mut ProtocolWorld, addr: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": addr})),
    )
    .unwrap();
}

// --- F+ whens ---

#[when(expr = "the agent appends an action at day {int} {word}")]
fn action_at(w: &mut ProtocolWorld, d: u32, hm: String) {
    w.gamma_result = Some(w.try_action(false, "reply", &day(d, &format!("{hm}:00"))));
}

#[when("the agent delegates the perimeter active from day 3 14:00 for 4 hours")]
fn delegate_windowed_inside(w: &mut ProtocolWorld) {
    w.delegate_act(
        "act.x.gmail.*",
        serde_json::json!({"active_windows": [win(&day(3, "14:00:00"), "4h")]}),
        true,
    )
    .unwrap();
}

#[when("the agent delegates the perimeter active from day 15 to day 40")]
fn delegate_windowed_outside(w: &mut ProtocolWorld) {
    w.delegate_act(
        "act.x.gmail.*",
        serde_json::json!({"active_windows": [win(&day(15, "00:00:00"), "600h")]}),
        true,
    )
    .unwrap();
    w.chain_result = Some(w.verify_chain_at(&w.helper_chain.clone(), DAY1));
}

#[when("the agent appends two actions inside the day 3 window")]
fn two_in_window(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(3, "14:30:00")).unwrap();
    w.try_action(false, "label", &day(3, "15:00:00")).unwrap();
}

#[when(
    expr = "the agent acts citing profile {string} with model {string} and {int} tokens at day {int} {word}"
)]
fn act_citing_full(
    w: &mut ProtocolWorld,
    profile: String,
    model: String,
    tokens: u64,
    d: u32,
    hm: String,
) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(d, &format!("{hm}:00")),
        Some(cite(&profile, &model, tokens)),
        None,
    ));
}

#[when("the agent acts without citing any budget_ref")]
fn act_uncited(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", &day(1, "15:00:00")));
}

#[when(expr = "the agent acts citing {string} with {int} declared tokens")]
fn act_citing_tokens(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "02:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    ));
}

#[when(expr = "the agent acts citing profile {string} with model {string} and {int} tokens")]
fn act_citing_short(w: &mut ProtocolWorld, profile: String, model: String, tokens: u64) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "16:00:00"),
        Some(cite(&profile, &model, tokens)),
        None,
    ));
}

#[when(expr = "the agent acts citing {string} with model {string}")]
fn act_citing_model(w: &mut ProtocolWorld, profile: String, model: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(cite(&profile, &model, 10)),
        None,
    ));
}

#[when(expr = "the agent acts citing profile {string} at day {int} {word}")]
fn act_citing_at(w: &mut ProtocolWorld, profile: String, d: u32, hm: String) {
    let model = if profile == "haiku" {
        "claude-haiku"
    } else {
        "gemma"
    };
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(d, &format!("{hm}:00")),
        Some(cite(&profile, model, 10)),
        None,
    ));
}

#[when(expr = "the agent acts citing budget_ref {string}")]
fn act_citing_unknown(w: &mut ProtocolWorld, profile: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({"budget_ref": profile})),
        None,
    ));
}

#[when(expr = "the helper acts citing {string} with {int} tokens")]
fn helper_acts_citing(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    w.try_action_full(
        true,
        "reply",
        &day(2, "01:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "model": "gemma", "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[when(expr = "the agent acts citing {string} with a declared usage and no receipt")]
fn act_no_receipt(w: &mut ProtocolWorld, profile: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "model": "claude-haiku", "tokens": 500})),
        None,
    ));
}

#[when(expr = "the agent acts citing {string} carrying a receipt signed by the provider")]
fn act_with_receipt(w: &mut ProtocolWorld, profile: String) {
    let receipt = provider_receipt("sha256:00", "claude-haiku", 8412, PROVIDER);
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({
            "budget_ref": profile, "model": "claude-haiku", "tokens": 500,
            "receipt": receipt
        })),
        None,
    ));
}

#[when("the agent acts carrying a receipt signed by the agent itself")]
fn act_with_forged_receipt(w: &mut ProtocolWorld) {
    let receipt = provider_receipt("sha256:00", "claude-haiku", 8412, AGENT);
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({
            "budget_ref": "haiku", "model": "claude-haiku", "tokens": 500,
            "receipt": receipt
        })),
        None,
    ));
}

#[when("the agent replays that receipt on a new action")]
fn replay_receipt(w: &mut ProtocolWorld) {
    let receipt = w.receipt.clone().unwrap();
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({
            "budget_ref": "haiku", "model": "claude-haiku", "tokens": 500,
            "receipt": receipt
        })),
        None,
    ));
}

#[when(
    expr = "the container logs an inference on {string} of {int} tokens in and {int} out citing {string}"
)]
fn container_logs_inference(
    w: &mut ProtocolWorld,
    model: String,
    tin: u64,
    tout: u64,
    profile: String,
) {
    w.gamma_result = Some(w.try_inference(&model, tin, tout, Some(&profile), &day(1, "16:00:00")));
}

#[when(expr = "the container logs an inference of {int} total tokens citing {string}")]
fn container_logs_total(w: &mut ProtocolWorld, total: u64, profile: String) {
    w.gamma_result = Some(w.try_inference(
        "gemma",
        total - 100,
        100,
        Some(&profile),
        &day(2, "10:00:00"),
    ));
}

#[when("the container logs an inference citing no budget_ref")]
fn container_logs_uncited(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_inference("gemma", 100, 10, None, &day(1, "16:00:00")));
}

#[when(expr = "an entry of kind {string} is forced onto the log")]
fn force_unknown_kind(w: &mut ProtocolWorld, kind: String) {
    let head = w.gbundle().gamma_head().unwrap();
    let line = serde_json::json!({
        "v": 1, "id": "gamma_00000000000000000000BANANA", "prev": head,
        "at": day(1, "00:00:00"), "kind": kind, "payload": {},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""}
    });
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap_or_default();
    bytes.extend_from_slice(line.to_string().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[when(expr = "the owner queries the kind class {string}")]
fn query_kind_class(w: &mut ProtocolWorld, class: String) {
    let owner = w.owner(0);
    w.query_hits = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .log_query_owner(
                &owner,
                &LogFilter {
                    kind: Some(class),
                    ..LogFilter::default()
                },
            )
            .map_err(|e| e.to_string()),
    );
}

#[when(expr = "the agent reads a section under {string}")]
fn agent_reads_section(w: &mut ProtocolWorld, folder: String) {
    let path = format!("{folder}/note1");
    w.read_body = Some(w.agent_reads(&w.chain.clone(), AGENT, &path));
}

#[when(expr = "the agent reads a section under {string} and logs its read")]
fn agent_reads_and_logs(w: &mut ProtocolWorld, folder: String) {
    let path = format!("{folder}/note1");
    w.read_body = Some(w.agent_reads(&w.chain.clone(), AGENT, &path));
    let chain = w.chain.clone();
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_read_as_agent(
            &chain,
            &agent_sk(AGENT),
            Zone::Circle,
            &path,
            &day(9, "01:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when(expr = "the agent acts with arguments naming recipient {string}")]
fn act_with_sealed_args(w: &mut ProtocolWorld, addr: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": addr, "body": "hello"})),
    ));
}

#[when("the sealed body is swapped for another one")]
fn swap_sealed_body(w: &mut ProtocolWorld) {
    // A lying agent: clear args_hash from one argument object, sealed body
    // from another — signed consistently, appended, caught only by audit.
    let owner = w.owner(0);
    let key = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_key_owner(&owner, "gmail")
        .unwrap();
    let mut ent = std::mem::take(&mut w.ent);
    let body = aithos_core::gamma::seal_body(
        &key,
        &w.bundle.as_ref().unwrap().did.clone(),
        "x.gmail",
        1,
        &serde_json::json!({"recipient": "mallory@evil.example"}),
        &ent.e24(),
    )
    .unwrap();
    w.ent = ent;
    let entries = w.gbundle().gamma_entries().unwrap();
    let spec = aithos_core::gamma::EntrySpec {
        id: "gamma_00000000000000000000000LIE".into(),
        prev: aithos_core::gamma::head(&entries).unwrap(),
        prevs: None,
        at: day(1, "02:00:00"),
        kind: aithos_core::gamma::Kind::Action,
        target: Some("x.gmail".into()),
        payload: Some(serde_json::json!({"action": "reply", "args_hash": "sha256:00"})),
        body_enc: Some(body),
    };
    let via: Vec<String> = w.chain.iter().map(|m| m.id.clone()).collect();
    let entry = aithos_core::gamma::delegated_entry(spec, via, &agent_sk(AGENT)).unwrap();
    let bundle = w.gbundle();
    bundle.gamma_append(&entry).unwrap();
}

#[when(expr = "the agent asks the container to reply to {string}")]
fn container_asked(w: &mut ProtocolWorld, addr: String) {
    // The container evaluates action_params on the REAL args (tier X)
    // before any entry exists.
    let verdict = aithos_core::constraints::check_action_params(
        &w.chain[0].constraints,
        "reply",
        &serde_json::json!({"recipient": addr}),
    );
    w.gamma_baseline = w.gbundle().gamma_entries().unwrap().len();
    w.gamma_result = Some(
        verdict
            .map(|()| "allowed".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("the owner audits the log against the mandate predicates")]
fn owner_audits(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let mandate = w.chain[0].clone();
    w.gamma_result = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .audit_log_against(&owner, &mandate)
            .map(|n| n.to_string())
            .map_err(|e| e.to_string()),
    );
}

// --- F+ thens ---

#[then("the action verifies")]
fn action_verifies(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_ok(), "action should verify, got {r:?}");
}

#[then("the action is refused as out of window")]
#[then("the action is refused as over budget")]
#[then("the action is refused as model not allowed")]
#[then("the action is refused")]
fn fplus_action_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_err(), "action should be refused, got {r:?}");
}

#[then(expr = "an action exactly at day {int} {word} verifies")]
#[then(expr = "an action at day {int} {word} verifies, F+")]
fn action_at_verifies_exact(w: &mut ProtocolWorld, d: u32, hms: String) {
    let hms = if hms.matches(':').count() == 1 {
        format!("{hms}:00")
    } else {
        hms
    };
    w.try_action(false, "reply", &day(d, &hms)).unwrap();
}

#[then(expr = "an action at day {int} {word} verifies")]
fn action_at_verifies(w: &mut ProtocolWorld, d: u32, hms: String) {
    action_at_verifies_exact(w, d, hms);
}

#[then(expr = "an action exactly at day {int} {word} is refused")]
#[then(expr = "an action at day {int} {word} is refused")]
fn action_at_refused(w: &mut ProtocolWorld, d: u32, hms: String) {
    let hms = if hms.matches(':').count() == 1 {
        format!("{hms}:00")
    } else {
        hms
    };
    let r = w.try_action(false, "reply", &day(d, &hms));
    assert!(r.is_err(), "day {d} {hms} should be refused, got {r:?}");
}

#[then("an action in the day 3 morning window verifies")]
fn union_day3(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(3, "09:00:00")).unwrap();
}

#[then("an action in the day 5 evening window verifies")]
fn union_day5(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(5, "19:00:00")).unwrap();
}

#[then("an action on day 4 noon is refused")]
fn union_day4_refused(w: &mut ProtocolWorld) {
    assert!(w.try_action(false, "reply", &day(4, "12:00:00")).is_err());
}

#[then("a third action inside the day 3 window is refused by the rolling limit")]
fn third_in_window_refused(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "send", &day(3, "15:30:00"));
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then("an action inside the day 4 window verifies")]
fn day4_window_ok(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(4, "14:30:00")).unwrap();
}

#[then("the helper's action inside that window verifies")]
fn helper_windowed_ok(w: &mut ProtocolWorld) {
    w.try_action(true, "reply", &day(3, "15:00:00")).unwrap();
}

#[then(expr = "an action at day {int} {word} is refused even though {word} is in phase")]
fn refused_beyond_validity(w: &mut ProtocolWorld, d: u32, hms: String, _phase: String) {
    let r = w.try_action(false, "reply", &day(d, &format!("{hms}:00")));
    assert!(r.is_err(), "beyond validity should refuse, got {r:?}");
}

#[then(expr = "the log shows {int} tokens consumed on profile {string}")]
fn log_shows_tokens(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let got = aithos_core::constraints::tally_tokens(&entries, &w.chain[0].id, &profile);
    assert_eq!(got, tokens);
}

#[then(expr = "an action citing {string} with {int} declared tokens verifies")]
fn citing_tokens_ok(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "03:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[then(expr = "the same action citing profile {string} verifies")]
fn same_action_other_profile(w: &mut ProtocolWorld, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(2, "09:00:00"),
        Some(cite(&profile, "gemma", 10)),
        None,
    )
    .unwrap();
}

#[then(expr = "an agent action citing {string} with {int} tokens is refused as over budget")]
fn agent_citing_refused(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    let r = w.try_action_full(
        false,
        "reply",
        &day(2, "02:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "model": "gemma", "tokens": tokens})),
        None,
    );
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then(expr = "any verifier counts {int} tokens consumed on {string}")]
fn verifier_counts_tokens(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(
        aithos_core::constraints::tally_tokens(&entries, &w.chain[0].id, &profile),
        tokens
    );
}

#[then(expr = "the remaining budget admits at most {int} declared tokens")]
fn headroom_check(w: &mut ProtocolWorld, headroom: u64) {
    let over = w.try_action_full(
        false,
        "reply",
        &day(2, "01:00:00"),
        Some(serde_json::json!({"budget_ref": "gemma", "tokens": headroom + 1})),
        None,
    );
    assert!(over.is_err(), "over headroom should refuse");
    w.try_action_full(
        false,
        "reply",
        &day(2, "02:00:00"),
        Some(serde_json::json!({"budget_ref": "gemma", "tokens": headroom})),
        None,
    )
    .unwrap();
}

#[then("the receipt's usage overrides the declared tokens in the tally")]
fn receipt_overrides(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(
        aithos_core::constraints::tally_tokens(&entries, &w.chain[0].id, "haiku"),
        8412
    );
}

#[then(expr = "the entry is of kind {string}")]
fn entry_of_kind(w: &mut ProtocolWorld, kind: String) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(entries.last().unwrap().kind, kind);
}

#[then("it reveals provider, model, token counts and budget_ref")]
fn inference_reveals_counters(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let p = entries.last().unwrap().payload.as_ref().unwrap();
    for k in ["provider", "model", "tokens_in", "tokens_out", "budget_ref"] {
        assert!(p.get(k).is_some(), "missing {k}");
    }
}

#[then("no prompt or response text exists anywhere in the log files")]
fn no_prompt_in_log(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    for e in entries.iter().filter(|e| e.kind == "inference") {
        let keys: Vec<&String> = e
            .payload
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .collect();
        for k in keys {
            assert!(
                ["provider", "model", "tokens_in", "tokens_out", "budget_ref"]
                    .contains(&k.as_str()),
                "unexpected inference payload key {k}"
            );
        }
        assert!(e.body_enc.is_none());
    }
}

#[then("the inference is refused as over budget")]
#[then("the inference is refused")]
fn inference_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_err(), "inference should be refused, got {r:?}");
}

#[then(expr = "an inference of {int} total tokens citing {string} verifies")]
fn inference_fits(w: &mut ProtocolWorld, total: u64, profile: String) {
    w.try_inference(
        "gemma",
        total - 100,
        100,
        Some(&profile),
        &day(2, "11:00:00"),
    )
    .unwrap();
}

#[then("every section entry comes back")]
fn sections_come_back(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hits.len(), 3, "two adds + one modify");
    assert!(hits.iter().all(|h| h.entry.kind.starts_with("section.")));
}

#[then("no action or heartbeat entry does")]
fn no_action_in_class(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert!(hits
        .iter()
        .all(|h| h.entry.kind != "action" && h.entry.kind != "heartbeat"));
}

#[then("no gamma entry is appended")]
fn no_entry_appended(w: &mut ProtocolWorld) {
    assert!(w.read_body.as_ref().unwrap().is_ok(), "the read succeeds");
    let len = w.gbundle().gamma_entries().unwrap().len();
    assert_eq!(len, w.gamma_baseline, "reading must not touch the log");
}

#[then(expr = "an {string} entry signed by the agent chains onto the log")]
fn read_entry_chains(w: &mut ProtocolWorld, kind: String) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last.kind, kind);
    assert!(last.authorized_by.is_some());
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("its sealed body names the section it read")]
fn read_body_names_section(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_owner(
            &owner,
            &LogFilter {
                kind: Some("ethos.read".into()),
                ..LogFilter::default()
            },
        )
        .unwrap();
    assert_eq!(hits.len(), 1);
    let body = hits[0].body.as_ref().expect("owner opens the read body");
    assert!(body.target.contains("/s/"), "target must name a section");
}

#[then("the entry carries a clear args_hash and a sealed args body")]
fn entry_has_sealed_args(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert!(last.payload.as_ref().unwrap().get("args_hash").is_some());
    assert!(last.body_enc.is_some());
}

#[then("the owner reopens the arguments and finds the recipient")]
fn owner_reopens_args(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let args = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_action_args(&owner, entries.last().unwrap())
        .unwrap();
    assert_eq!(args["recipient"], "alice@example.com");
}

#[then("a stranger sees only the hash")]
fn stranger_sees_hash(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    let clear = serde_json::to_string(last.payload.as_ref().unwrap()).unwrap();
    assert!(!clear.contains("alice@example.com"), "args must not leak");
    assert!(clear.contains("sha256:"));
}

#[then("the audit rejects the entry as inconsistent")]
fn audit_rejects(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let r = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_action_args(&owner, entries.last().unwrap());
    assert!(r.is_err(), "mismatched args must fail the audit, got {r:?}");
}

#[then("the container refuses before anything is logged")]
fn container_refuses(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_err());
    let len = w.gbundle().gamma_entries().unwrap().len();
    assert_eq!(len, w.gamma_baseline, "nothing may be logged");
}

#[then("the audit reports every logged action compliant")]
fn audit_compliant(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_ok(), "audit should pass, got {r:?}");
    assert_ne!(r.as_ref().unwrap(), "0", "at least one action audited");
}

fn read_json<T: serde::de::DeserializeOwned>(b: &Bundle<MemStore>, path: &str) -> T {
    serde_json::from_slice(&b.store.get(path).unwrap().unwrap()).unwrap()
}

// ------------------------------------------------------ step G: revocation ---

const SURV: u8 = 0xB7; // survivor
const HOLD: u8 = 0xB8; // zone holder
const WDOG: u8 = 0xB9; // watchdog
const G_AT: &str = "2026-07-11T00:00:00Z"; // the revocation instant
const G_AFTER: &str = "2026-07-12T00:00:00Z";
const G_BEFORE: &str = "2026-07-10T00:00:00Z";

fn kid_of(sk: u8) -> String {
    wire::ed25519_pub_to_multibase(&agent_sk(sk).verifying_key().to_bytes())
}

impl ProtocolWorld {
    fn gb(&mut self) -> &mut Bundle<MemStore> {
        self.bundle.as_mut().unwrap()
    }

    /// Grant a named agent read on a circle folder; return its root chain.
    fn grant_read_named(&mut self, label: &str, sk: u8, dir: &str, na: &str) -> Vec<Mandate> {
        let owner = self.owner(0);
        let m = self
            .bundle
            .as_mut()
            .unwrap()
            .grant(
                &owner,
                label,
                &agent_sk(sk).verifying_key(),
                &[dir_spec(dir)],
                NB,
                na,
                0,
                &mut self.ent,
            )
            .expect("grant succeeds");
        vec![m]
    }

    /// Mint and store a root mandate with an explicit perimeter (watchdogs).
    fn mint_root(
        &mut self,
        label: &str,
        sk: u8,
        perimeter: Vec<PerimeterEntry>,
        na: &str,
    ) -> Mandate {
        use aithos_core::mandate::{Mandate as M, MandateSpec};
        let owner = self.owner(0);
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 700)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.to_owned(),
                grantee_pub: &agent_sk(sk).verifying_key(),
                perimeter,
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: na.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        m
    }

    fn revoke_owner(&mut self, mandate_id: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.gb()
            .log_revoke_owner(&owner, mandate_id, "test", at, &mut ent)
            .unwrap();
        self.ent = ent;
        self.revoked_at = at.to_owned();
    }

    fn rotate(&mut self, folder: &str, revoked_sk: u8) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.gb()
            .rotate_folder(&owner, folder, &kid_of(revoked_sk), &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn read_at(&self, chain: &[Mandate], sk: u8, path: &str, at: &str) -> Result<String, String> {
        self.bundle
            .as_ref()
            .unwrap()
            .read_section_as_agent(chain, &agent_sk(sk), Zone::Circle, path, at)
            .map_err(|e| e.to_string())
    }

    fn verify_revocable_at(&self, chain: &[Mandate], at: &str) -> Result<(), String> {
        let bundle = self.bundle.as_ref().unwrap();
        let doc = self.did_document();
        let revs = bundle.active_revocations().map_err(|e| e.to_string())?;
        aithos_core::mandate::verify_chain_revocable(chain, &doc, at, &revs)
            .map_err(|e| e.to_string())
    }
}

// --- G givens ---

#[given("two agents granted read on circle folder \"projets\" and a zone holder")]
fn two_agents_and_holder(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    w.survivor_chain = w.grant_read_named("survivor", SURV, "projets", NA30);
    w.holder_chain = w.grant_read_named("holder", HOLD, "", NA30);
}

#[given("two agents holding lines on circle folder \"projets\"")]
fn two_agents_lines(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    w.survivor_chain = w.grant_read_named("survivor", SURV, "projets", NA30);
}

#[given("a zone holder reading folder \"projets\" by pure derivation")]
fn zone_holder_derivation(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    w.holder_chain = w.grant_read_named("holder", HOLD, "", NA30);
    // Baseline: the holder reads by derivation before any rotation.
    assert!(w
        .read_at(&w.holder_chain.clone(), HOLD, "projets/note1", DAY1)
        .is_ok());
}

#[given("an agent granted action rights")]
fn g_agent_action(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
}

#[given("an agent that acted before being revoked")]
fn agent_acted_before(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", G_BEFORE).unwrap();
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
}

#[given("an agent with issue depth 1 that delegated to a helper")]
fn agent_delegated_helper(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.*", serde_json::json!({}), true)
        .unwrap();
}

#[given("two unrelated agents granted action rights")]
fn two_unrelated_agents(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    // A second, independent agent (SURV keypair).
    let owner = w.owner(0);
    let m = aithos_core::mandate::Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: "mandate_0000000000000000000000SURV".into(),
            subject: w.bundle.as_ref().unwrap().did.clone(),
            grantee_id: "urn:aithos:agent:other".into(),
            grantee_label: "other".into(),
            grantee_pub: &agent_sk(SURV).verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse("act.x.gmail.*").unwrap()],
            constraints: aithos_core::mandate::MandateSpec::no_constraints(),
            not_before: NB.into(),
            not_after: NA30.into(),
            issued_at: NB.into(),
            nonce: hex::encode(w.ent.e16()),
        },
    )
    .unwrap();
    w.store_cert(&m);
    w.survivor_chain = vec![m];
}

#[given("a watchdog granted only the revoke right over circle \"projets\"")]
fn watchdog_grant(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    let scope =
        PerimeterEntry::parse(&format!("revoke.circle#dir={}", w.resolve_dir("projets"))).unwrap();
    let wd = w.mint_root("watchdog", WDOG, vec![scope], NA30);
    w.holder_chain = vec![wd];
}

#[given("an agent whose mandate expired yesterday")]
fn agent_expired(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    // Window ends at NA7 = 2026-07-08; "now" is DAY8 = 2026-07-09.
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA7);
}

#[given("an agent that exfiltrated nothing but held folder \"projets\"")]
fn agent_held_folder(w: &mut ProtocolWorld) {
    two_agents_lines(w);
}

#[given("a helper cut by its parent's revocation")]
fn helper_cut(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    // Owner grants the agent read + issue on projets, agent delegates to helper.
    let owner = w.owner(0);
    let m = w
        .bundle
        .as_mut()
        .unwrap()
        .grant(
            &owner,
            "agent",
            &agent_sk(AGENT).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA30,
            1,
            &mut w.ent,
        )
        .unwrap();
    w.chain = vec![m];
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &w.chain[0].clone(),
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![w.chain[0].clone(), sub];
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
}

#[given("a rotated header version for folder \"projets\"")]
fn rotated_header(w: &mut ProtocolWorld) {
    two_agents_lines(w);
}

#[given("a rotated folder \"projets\" under the circle zone")]
fn rotated_folder_holder(w: &mut ProtocolWorld) {
    zone_holder_derivation(w);
    w.rotate("projets", AGENT);
}

impl ProtocolWorld {
    fn resolve_dir(&self, display: &str) -> String {
        let dirs = self
            .bundle
            .as_ref()
            .unwrap()
            .resolve_folder(Zone::Circle, display)
            .unwrap();
        dirs.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("/")
    }
}

// --- G whens ---

#[when("the owner revokes the first agent with rotation")]
fn revoke_with_rotation(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
    w.rotate("projets", AGENT);
    w.publish_bundle();
}

#[given("the owner revokes the agent's mandate")]
#[when("the owner revokes the agent's mandate")]
#[when("the owner revokes the agent's mandate for real")]
fn revoke_agent(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
}

#[when("the agent revokes the helper's mandate")]
fn agent_revokes_helper(w: &mut ProtocolWorld) {
    let helper_id = w.helper_chain[1].id.clone();
    let parent = vec![w.chain[0].clone()];
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(
            &parent,
            &agent_sk(AGENT),
            &helper_id,
            "test",
            G_AT,
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when("the first agent forges a revocation of the second's mandate")]
fn forge_revocation(w: &mut ProtocolWorld) {
    let target = w.survivor_chain[0].id.clone();
    let attacker = vec![w.chain[0].clone()];
    let mut ent = std::mem::take(&mut w.ent);
    w.g_result = Some(
        w.gb()
            .log_revoke_as(
                &attacker,
                &agent_sk(AGENT),
                &target,
                "malice",
                G_AT,
                &mut ent,
            )
            .map(|_| "appended".into())
            .map_err(|e| e.to_string()),
    );
    w.ent = ent;
}

#[when("the watchdog revokes the projets agent's mandate")]
fn watchdog_revokes(w: &mut ProtocolWorld) {
    let target = w.chain[0].id.clone();
    let wd = w.holder_chain.clone();
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(&wd, &agent_sk(WDOG), &target, "watchdog", G_AT, &mut ent)
        .unwrap();
    w.ent = ent;
}

#[when("the owner rotates \"projets\" out of a revoked agent")]
#[when("a manager rotates the node in passing")]
fn owner_rotates(w: &mut ProtocolWorld) {
    w.rotate("projets", AGENT);
    w.publish_bundle();
}

#[when("the owner revokes it with rotation and re-encryption")]
fn revoke_reencrypt(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
    w.rotate("projets", AGENT);
    w.publish_bundle();
}

#[when("the new version claims a line for a key absent from the old version")]
fn smuggle_recipient(w: &mut ProtocolWorld) {
    // Build a rotation that seals to an intruder never present in v1.
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let file = format!(
        "e/circle/hdr/{}.json",
        hex::encode(&blake3::hash(node.to_string().as_bytes()).as_bytes()[..12])
    );
    let mut header: Header = read_json(w.bundle.as_ref().unwrap(), &file);
    let intruder = Recipient {
        to: kid_of(WDOG),
        kid: kid_of(WDOG),
        pubkey: ed2x(&agent_sk(WDOG).verifying_key()),
    };
    let doc = w.did_document();
    let owner_kex = aithos_core::wire::multibase_to_x25519_pub(&doc.keys.kex).unwrap();
    let owner_rec = Recipient::owner(owner_kex.into());
    header
        .rotate(
            &w.bundle.as_ref().unwrap().did.clone(),
            2,
            &[9u8; 32],
            &[owner_rec, intruder],
            &[[1u8; 32], [2u8; 32]],
            &[[1u8; 24], [2u8; 24]],
        )
        .unwrap();
    w.g_result = Some(
        header
            .check_rotation(2)
            .map(|_| "ok".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("someone without the parent key posts an up-link wrap")]
fn bogus_wrap(w: &mut ProtocolWorld) {
    // Overwrite the up-link wrap with one sealed under a WRONG via key.
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let zroot = NodePath::zone_root(Zone::Circle);
    let file = format!(
        "e/circle/wraps/{}.json",
        hex::encode(&blake3::hash(format!("{zroot}\u{0}{node}").as_bytes()).as_bytes()[..12])
    );
    let bogus = Wrap::seal(
        &w.bundle.as_ref().unwrap().did.clone(),
        &zroot.to_string(),
        &[0xEEu8; 32], // not the real zone key
        &node.to_string(),
        2,
        &[9u8; 32],
        [7u8; 24],
    );
    let bytes = serde_json::to_vec_pretty(&bogus).unwrap();
    w.gb().store.put(&file, &bytes).unwrap();
}

#[when("the owner grants the helper a fresh mandate on the same folder")]
fn readopt_helper(w: &mut ProtocolWorld) {
    // Fresh mandate to the SAME helper keypair on the same folder.
    w.holder_chain = w.grant_read_named("helper-readopted", HELPER, "projets", NA30);
}

#[when("the agent presents its chain after the revocation instant")]
fn present_after(w: &mut ProtocolWorld) {
    w.g_result = Some(
        w.verify_revocable_at(&w.chain.clone(), G_AFTER)
            .map(|()| "valid".into()),
    );
}

#[when("the head agent forges a heartbeat with its own key for G")]
fn _unused_g(_w: &mut ProtocolWorld) {}

// --- G thens ---

#[then("the revoked agent reads nothing written after the cut")]
fn revoked_reads_nothing(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.chain.clone(), AGENT, "projets/note1", G_AFTER);
    assert!(r.is_err(), "revoked agent must not read, got {r:?}");
}

#[then("the surviving agent reads new content without lifting a finger")]
fn survivor_reads(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.survivor_chain.clone(), SURV, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "survivor must read, got {r:?}");
}

#[then("the zone holder still reads the folder through the up-link wrap")]
fn holder_reads_uplink(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), HOLD, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "zone holder must read via up-link, got {r:?}");
}

#[then("a \"revoke\" entry signed by the owner chains onto the log")]
fn revoke_entry_present(w: &mut ProtocolWorld) {
    let entries = w.gb().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last.kind, "revoke");
    assert_eq!(last.signature.key, "#content");
}

#[then("the chain is rejected as revoked")]
#[then("the helper's chain is rejected as revoked")]
#[then("the projets agent's chain is rejected as revoked")]
fn chain_rejected_revoked(w: &mut ProtocolWorld) {
    let chain = if !w.helper_chain.is_empty() && w.g_context_helper() {
        w.helper_chain.clone()
    } else {
        w.chain.clone()
    };
    let r = w.verify_revocable_at(&chain, G_AFTER);
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "expected revoked, got {r:?}"
    );
}

impl ProtocolWorld {
    fn g_context_helper(&self) -> bool {
        // The helper scenarios revoke the parent (w.chain) and check helper.
        !self.helper_chain.is_empty()
    }
}

#[then("the action logged before revoked_at still verifies at its own timestamp")]
fn old_action_valid(w: &mut ProtocolWorld) {
    assert!(w.verify_revocable_at(&w.chain.clone(), G_BEFORE).is_ok());
}

#[then("an action timestamped after revoked_at is rejected")]
fn new_action_rejected(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.chain.clone(), G_AFTER);
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "got {r:?}"
    );
}

#[then("the revocation entry is rejected")]
fn revocation_rejected(w: &mut ProtocolWorld) {
    let r = w.g_result.as_ref().unwrap();
    assert!(r.is_err(), "forged revocation must be rejected, got {r:?}");
}

#[then("the second agent's chain still verifies")]
fn second_chain_ok(w: &mut ProtocolWorld) {
    assert!(w
        .verify_revocable_at(&w.survivor_chain.clone(), G_AFTER)
        .is_ok());
}

#[then("the watchdog itself cannot open a single body")]
fn watchdog_reads_nothing(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), WDOG, "projets/note1", G_BEFORE);
    assert!(r.is_err(), "watchdog holds no content key, got {r:?}");
}

#[then("the folder's header gains a version without the revoked line")]
fn header_new_version(w: &mut ProtocolWorld) {
    // The When already revoked + rotated; here we only inspect the header.
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let file = format!(
        "e/circle/hdr/{}.json",
        hex::encode(&blake3::hash(node.to_string().as_bytes()).as_bytes()[..12])
    );
    let header: Header = read_json(w.bundle.as_ref().unwrap(), &file);
    assert!(header.key_versions.contains_key("2"));
    let v2 = &header.key_versions["2"];
    assert!(
        !v2.lines.iter().any(|l| l.kid == kid_of(AGENT)),
        "revoked line present"
    );
}

#[then("the survivor opens the new version with its unchanged keypair")]
fn survivor_opens_v2(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.survivor_chain.clone(), SURV, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "survivor opens v2, got {r:?}");
}

#[then("the zone holder keeps reading through the up-link wrap")]
fn holder_keeps_reading(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), HOLD, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "holder reads via wrap, got {r:?}");
}

#[then("the wrap is bound to the node and its new key version")]
fn wrap_bound(w: &mut ProtocolWorld) {
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let zroot = NodePath::zone_root(Zone::Circle);
    let file = format!(
        "e/circle/wraps/{}.json",
        hex::encode(&blake3::hash(format!("{zroot}\u{0}{node}").as_bytes()).as_bytes()[..12])
    );
    let wrap: Wrap = read_json(w.bundle.as_ref().unwrap(), &file);
    assert_eq!(wrap.node, node.to_string());
    assert_eq!(wrap.key_version, 2);
}

#[then("header verification is rejected")]
fn header_rejected(w: &mut ProtocolWorld) {
    let r = w.g_result.as_ref().unwrap();
    assert!(r.is_err(), "smuggled recipient must be rejected, got {r:?}");
}

#[then("the wrap is rejected")]
fn wrap_rejected(w: &mut ProtocolWorld) {
    // The zone holder tries to read through the bogus wrap: it cannot open.
    let r = w.read_at(&w.holder_chain.clone(), HOLD, "projets/note1", G_AFTER);
    assert!(r.is_err(), "a bogus wrap grants nothing, got {r:?}");
}

#[then("the agent's actions are rejected by every verifier")]
fn expired_actions_rejected(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.chain.clone(), DAY8);
    assert!(
        r.is_err(),
        "expired mandate must fail the window, got {r:?}"
    );
}

#[then("its key still opens content written under the old version")]
fn key_still_opens(w: &mut ProtocolWorld) {
    // No rotation happened: a fresh grant to another agent still reads the
    // very same content — proving expiry turned no lock.
    w.survivor_chain = w.grant_read_named("survivor", SURV, "projets", NA30);
    let r = w.read_at(&w.survivor_chain.clone(), SURV, "projets/note1", DAY1);
    assert!(r.is_ok(), "content unchanged by mere expiry, got {r:?}");
}

#[then("the old key opens nothing written since")]
fn old_key_dead(w: &mut ProtocolWorld) {
    // After the manager's rotation, a NON-survivor derivation is stale: the
    // revoked/expired agent's chain no longer reads new content.
    let r = w.read_at(&w.chain.clone(), AGENT, "projets/note1", DAY8);
    assert!(r.is_err(), "old key must be dead after rotation, got {r:?}");
}

#[then("the folder's existing bodies are rewritten under the new key")]
fn bodies_reencrypted(w: &mut ProtocolWorld) {
    let index: aithos_bundle::bundle::ZoneIndex =
        read_json(w.bundle.as_ref().unwrap(), "e/circle/index.json");
    assert!(
        index.sections.iter().all(|s| s.key_version == 2),
        "sections must be re-encrypted at v2"
    );
}

#[then("the revoked key opens neither the new bodies nor the new lines")]
fn revoked_key_dead(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.chain.clone(), AGENT, "projets/note1", G_AFTER);
    assert!(r.is_err(), "revoked key opens nothing, got {r:?}");
}

#[then("the helper reads again with the same keypair")]
fn helper_readopted(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), HELPER, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "re-adopted helper reads, got {r:?}");
}

// ----------------------------------------- move-as-rotation (spec 02.9) ---

fn hdr_path_of(node: &NodePath) -> String {
    format!(
        "e/circle/hdr/{}.json",
        hex::encode(&blake3::hash(node.to_string().as_bytes()).as_bytes()[..12])
    )
}

fn wrap_path_of(via: &NodePath, node: &NodePath) -> String {
    format!(
        "e/circle/wraps/{}.json",
        hex::encode(&blake3::hash(format!("{via}\u{0}{node}").as_bytes()).as_bytes()[..12])
    )
}

#[given(expr = "a section {string} the agent reads by derivation")]
fn section_read_by_derivation(w: &mut ProtocolWorld, path: String) {
    let (folder, name) = path.rsplit_once('/').expect("folder/name");
    w.add_named_section(folder, name, &[]);
    let r = w.agent_reads(&w.chain.clone(), AGENT, &path);
    assert_eq!(r.as_deref(), Ok(BODY), "derivation read fails: {r:?}");
}

#[when(expr = "the owner moves folder {string} under {string}")]
fn owner_moves_folder(w: &mut ProtocolWorld, folder: String, dest: String) {
    // Scenario permutations share this step: seed the fixture ends that the
    // Given did not create (the new-parent scenario grants "projets" only).
    if w.bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, &folder)
        .is_err()
    {
        assert_eq!(folder, "archives/old", "unexpected move fixture");
        w.add_named_section(&folder, "note1", &[]);
    }
    let owner = w.owner(0);
    let bundle = w.bundle.as_mut().unwrap();
    bundle
        .ensure_folder(Zone::Circle, &dest, &owner, &mut w.ent)
        .expect("destination exists");
    bundle
        .move_folder(&owner, &folder, &dest, &mut w.ent)
        .expect("move succeeds");
}

#[then("the agent still derives the folder's old key — it cannot be un-taught")]
fn agent_still_derives_old_key(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let b = w.bundle.as_ref().unwrap();
    // The moved folder sits at its new address; its sid is stable.
    let new_chain = b.resolve_folder(Zone::Circle, "projets/old").unwrap();
    let m_sid = *new_chain.last().unwrap();
    let a_chain = b.resolve_folder(Zone::Circle, "archives").unwrap();
    // Agent side: its own line on "archives", one derivation step down.
    let a_node = NodePath::folder(Zone::Circle, a_chain.clone());
    let header: Header = read_json(b, &hdr_path_of(&a_node));
    let sk = agent_sk(AGENT);
    let kid = wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes());
    let kex = aithos_core::keys::grantee_kex_secret(&sk);
    let dk_a = header
        .open(&b.did, 1, &kid, &kex)
        .expect("agent line opens");
    let derived = derive_key(&aithos_core::derive::folder_label(&m_sid), &dk_a);
    // Owner side: the same old key straight from the zone root — the move
    // could not un-teach it, which is exactly why it had to rotate.
    let mut old_chain = a_chain;
    old_chain.push(m_sid);
    let expected = node_key(
        &b.zone_dk(Zone::Circle, &owner).unwrap(),
        &NodePath::folder(Zone::Circle, old_chain),
    );
    assert_eq!(derived, expected, "old-parent derivation must survive");
}

#[then(expr = "the agent's read of {string} is rejected as outside its perimeter")]
fn read_rejected_outside_perimeter(w: &mut ProtocolWorld, path: String) {
    let r = w.agent_reads(&w.chain.clone(), AGENT, &path);
    let err = r.expect_err("the old parent's grant must not cover the moved node");
    assert!(err.contains("not covered"), "wrong rejection: {err}");
}

#[then("the folder carries a fresh key version at its new path")]
fn fresh_version_at_new_path(w: &mut ProtocolWorld) {
    let b = w.bundle.as_ref().unwrap();
    let chain = b.resolve_folder(Zone::Circle, "projets/old").unwrap();
    let node = NodePath::folder(Zone::Circle, chain);
    let header: Header = read_json(b, &hdr_path_of(&node));
    assert_eq!(header.node, node.to_string(), "header binds the new path");
    assert_eq!(header.latest_version(), 2, "fresh key version");
    assert!(
        !header.key_versions.contains_key("1"),
        "old versions stay at the old address"
    );
}

#[then(expr = "the agent reads new content at {string} with its unchanged keypair")]
fn survivor_reads_new_content(w: &mut ProtocolWorld, folder: String) {
    // The owner writes NEW content after the move; the direct line was
    // re-sealed as a survivor, so the same keypair keeps reading.
    w.add_named_section(&folder, "note2", &[]);
    let r = w.agent_reads(&w.chain.clone(), AGENT, &format!("{folder}/note2"));
    assert_eq!(r.as_deref(), Ok(BODY), "survivor read fails: {r:?}");
}

#[then(expr = "the agent reads {string} through the wrap posted under {string}")]
fn reads_through_parent_wrap(w: &mut ProtocolWorld, path: String, parent: String) {
    let b = w.bundle.as_ref().unwrap();
    let p_chain = b.resolve_folder(Zone::Circle, &parent).unwrap();
    let m_chain = b
        .resolve_folder(Zone::Circle, &format!("{parent}/old"))
        .unwrap();
    let via = NodePath::folder(Zone::Circle, p_chain);
    let node = NodePath::folder(Zone::Circle, m_chain);
    assert!(
        b.store.get(&wrap_path_of(&via, &node)).unwrap().is_some(),
        "the up-link wrap must hang under the new parent"
    );
    let r = w.agent_reads(&w.chain.clone(), AGENT, &path);
    assert_eq!(r.as_deref(), Ok(BODY), "wrap read fails: {r:?}");
}

// --- step G+: obligations (spec 04.12) ---

const APPROVER: u8 = 0xB5;
const APPROVER2: u8 = 0xB6;
const GUARD: u8 = 0xD4;
const DUAL: u8 = 0xB7;
const SIB: u8 = 0xB8;
const STRANGER: u8 = 0xEE;
/// The G+ entry instant; receipts date relative to it.
const GP_AT: &str = "2026-07-04T12:00:00Z";
const G_ARGS: &str = "sha256:aa11";

fn mb_of(b: u8) -> String {
    wire::ed25519_pub_to_multibase(&agent_sk(b).verifying_key().to_bytes())
}

fn ob_json(
    id: &str,
    check: &str,
    attestors: &[u8],
    applies_to: &str,
    verdict: &str,
    max_age: Option<&str>,
) -> serde_json::Value {
    let mut o = serde_json::json!({
        "id": id, "check": check,
        "attestor": attestors.iter().map(|b| mb_of(*b)).collect::<Vec<_>>(),
        "applies_to": applies_to, "verdict": verdict,
    });
    if let Some(d) = max_age {
        o["max_age"] = serde_json::json!(d);
    }
    o
}

fn approval_ob() -> serde_json::Value {
    ob_json(
        "publish-approval",
        "human.approve",
        &[APPROVER],
        "act.x.social.publish",
        "approve",
        Some("5m"),
    )
}

fn guard_ob() -> serde_json::Value {
    ob_json(
        "pii-guard",
        "guardrail.pii",
        &[GUARD],
        "act.x.social.publish",
        "pass",
        None,
    )
}

/// Sign the §04.12 payload and return the checks[] rider.
fn ob_receipt(
    sk: &SigningKey,
    obligation: &str,
    coords: (&str, &str, &str), // (mandate_id, action, args_hash)
    verdict: &str,
    at: &str,
    presented: Option<&str>,
) -> serde_json::Value {
    use ed25519_dalek::Signer;
    let (mandate_id, action, args_hash) = coords;
    let mut payload = serde_json::json!({
        "obligation": obligation, "mandate_id": mandate_id, "action": action,
        "args_hash": args_hash, "verdict": verdict, "at": at,
    });
    if let Some(d) = presented {
        payload["presented_digest"] = serde_json::json!(d);
    }
    let sig = hex::encode(
        sk.sign(&aithos_core::jcs::canonical_bytes(&payload).unwrap())
            .to_bytes(),
    );
    let mut check = serde_json::json!({
        "obligation": obligation, "args_hash": args_hash,
        "verdict": verdict, "at": at, "sig": sig,
    });
    if let Some(d) = presented {
        check["presented_digest"] = serde_json::json!(d);
    }
    check
}

fn approver_receipt(w: &ProtocolWorld, at: &str, presented: Option<&str>) -> serde_json::Value {
    ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&w.chain.last().unwrap().id, "publish", G_ARGS),
        "approve",
        at,
        presented,
    )
}

impl ProtocolWorld {
    fn grant_act_patterns(
        &mut self,
        patterns: &[&str],
        issue: Option<u32>,
        constraints: serde_json::Value,
    ) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        self.init_bundle();
        let owner = self.owner(0);
        let mut perimeter: Vec<PerimeterEntry> = patterns
            .iter()
            .map(|p| PerimeterEntry::parse(p).unwrap())
            .collect();
        if let Some(depth) = issue {
            perimeter.push(PerimeterEntry::Issue { depth });
        }
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 970)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter,
                constraints,
                not_before: NB.into(),
                not_after: NA30.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    /// Sub-mandate minted by AGENT — attenuation judged at verification,
    /// never at mint (build_sub only builds and signs).
    fn mint_sub(
        &mut self,
        grantee: u8,
        label: &str,
        pattern: &str,
        constraints: serde_json::Value,
        log_grant: bool,
    ) -> Result<Vec<Mandate>, String> {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        let parent = self.chain[0].clone();
        let child = M::build_sub(
            &parent,
            &agent_sk(AGENT),
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 980)),
                subject: parent.subject.clone(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.into(),
                grantee_pub: &agent_sk(grantee).verifying_key(),
                perimeter: vec![PerimeterEntry::parse(pattern).unwrap()],
                constraints,
                not_before: NB.into(),
                not_after: parent.not_after.clone(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .map_err(|e| e.to_string())?;
        self.store_cert(&child);
        if log_grant {
            let mut ent = std::mem::take(&mut self.ent);
            let r = self.gbundle().log_delegated_grant(
                std::slice::from_ref(&parent),
                &agent_sk(AGENT),
                &child.id,
                &day(1, "00:40:00"),
                &mut ent,
            );
            self.ent = ent;
            r.map_err(|e| e.to_string())?;
        }
        Ok(vec![parent, child])
    }

    fn try_action_checked(
        &mut self,
        chain: &[Mandate],
        sk: &SigningKey,
        connector: &str,
        action: &str,
        checks: Option<serde_json::Value>,
    ) -> Result<String, String> {
        self.gamma_baseline = self.gbundle().gamma_entries().unwrap().len();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_action_with_checks(
                chain,
                sk,
                &aithos_bundle::log::ActionSpec {
                    connector,
                    action,
                    args_hash: G_ARGS,
                    now: GP_AT,
                    budget: None,
                    sealed_args: None,
                },
                checks,
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn publish_with(&mut self, checks: Option<serde_json::Value>) {
        let chain = self.chain.clone();
        let r = self.try_action_checked(&chain, &agent_sk(AGENT), "social", "publish", checks);
        self.gamma_result = Some(r);
    }

    fn appended_checks(&mut self) -> Option<serde_json::Value> {
        let id = self
            .gamma_result
            .as_ref()
            .unwrap()
            .as_ref()
            .expect("action should have appended")
            .clone();
        self.gbundle()
            .gamma_entries()
            .unwrap()
            .into_iter()
            .find(|e| e.id == id)
            .and_then(|e| e.payload.as_ref().and_then(|p| p.get("checks")).cloned())
    }
}

// --- givens ---

#[given("an agent granted social publish under a guardrail obligation")]
#[given("an agent granted social publish under a guardrail obligation with no max_age")]
fn gplus_guardrail_grant(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [guard_ob()]}),
    );
}

#[given("an agent granted gmail send and social publish under a publish-only guardrail obligation")]
fn gplus_two_connectors(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.gmail.*", "act.x.social.*"],
        None,
        serde_json::json!({"obligations": [guard_ob()]}),
    );
}

#[given("an agent granted social actions under a guardrail obligation on every social action")]
fn gplus_wildcard_grant(w: &mut ProtocolWorld) {
    let ob = ob_json(
        "pii-guard",
        "guardrail.pii",
        &[GUARD],
        "act.x.social.*",
        "pass",
        None,
    );
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [ob]}),
    );
}

#[given("an agent granted social publish requiring human approval within 5 minutes")]
fn gplus_approval_grant(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [approval_ob()]}),
    );
}

#[given("an agent granted social publish requiring approval by one of two approvers")]
fn gplus_two_approvers(w: &mut ProtocolWorld) {
    let ob = ob_json(
        "publish-approval",
        "human.approve",
        &[APPROVER, APPROVER2],
        "act.x.social.publish",
        "approve",
        Some("5m"),
    );
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [ob]}),
    );
}

#[given("an agent granted social actions requiring human approval on publish")]
fn gplus_approval_on_publish(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [approval_ob()]}),
    );
}

#[given("an agent granted gmail send with counter_sign on send")]
fn gplus_counter_sign_grant(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.gmail.*"],
        None,
        serde_json::json!({"counter_sign": ["send"]}),
    );
}

#[given("an agent granted social publish under dual control with a second agent")]
fn gplus_dual_control(w: &mut ProtocolWorld) {
    let ob = ob_json(
        "four-eyes",
        "agent.countersign",
        &[DUAL],
        "act.x.social.publish",
        "approve",
        None,
    );
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [ob]}),
    );
}

#[given("an agent granted social publish under both a guardrail and a human approval obligation")]
fn gplus_both_obligations(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [guard_ob(), approval_ob()]}),
    );
}

#[given("two sibling sub-mandates that may publish under an ancestor approval obligation")]
fn gplus_siblings(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        Some(1),
        serde_json::json!({"obligations": [approval_ob()]}),
    );
    let inherited = serde_json::json!({"obligations": [approval_ob()]});
    let a = w
        .mint_sub(
            HELPER,
            "sib-a",
            "act.x.social.publish",
            inherited.clone(),
            true,
        )
        .unwrap();
    let b = w
        .mint_sub(SIB, "sib-b", "act.x.social.publish", inherited, true)
        .unwrap();
    w.sib_chains = vec![a, b];
}

#[given("a head mandate requiring human approval on publish")]
fn gplus_head_mandate(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        Some(1),
        serde_json::json!({"obligations": [approval_ob()]}),
    );
}

#[given("a sub-mandate that adds a guardrail obligation on publish")]
fn gplus_sub_adds(w: &mut ProtocolWorld) {
    let child = serde_json::json!({"obligations": [approval_ob(), guard_ob()]});
    let chain = w
        .mint_sub(HELPER, "helper", "act.x.social.publish", child, true)
        .unwrap();
    w.helper_chain = chain;
}

#[given("the approver signed a receipt over what was shown on the device")]
fn gplus_signed_wysiwys(w: &mut ProtocolWorld) {
    w.gplus_checks = Some(approver_receipt(
        w,
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    ));
}

// --- whens ---

#[when(expr = "the agent publishes with a receipt whose verdict is {string}")]
fn gplus_publish_verdict(w: &mut ProtocolWorld, verdict: String) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        &verdict,
        "2026-07-04T11:59:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the agent publishes with a pass receipt signed 2 days earlier")]
fn gplus_publish_aged(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-02T12:00:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when(expr = "the agent deletes a post with a receipt whose verdict is {string}")]
fn gplus_delete_verdict(w: &mut ProtocolWorld, verdict: String) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "delete", G_ARGS),
        &verdict,
        "2026-07-04T11:59:00Z",
        None,
    );
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "social", "delete", Some(check));
    w.gamma_result = Some(r);
}

#[when("the agent sends a mail without any receipt")]
fn gplus_send_bare(w: &mut ProtocolWorld) {
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "gmail", "send", None);
    w.gamma_result = Some(r);
}

#[when("the agent publishes without any receipt")]
fn gplus_publish_bare(w: &mut ProtocolWorld) {
    w.publish_with(None);
}

#[when(expr = "the approver signs the prepared publish {int} minutes before the entry")]
fn gplus_approve_before(w: &mut ProtocolWorld, minutes: u32) {
    let at = format!("2026-07-04T11:{:02}:00Z", 60 - minutes);
    let check = approver_receipt(w, &at, Some("sha256:rendered-ship-it"));
    w.publish_with(Some(check));
}

#[when("the approver signs the prepared publish 2 minutes after the entry's clock")]
fn gplus_approve_after(w: &mut ProtocolWorld) {
    let check = approver_receipt(w, "2026-07-04T12:02:00Z", Some("sha256:rendered-ship-it"));
    w.publish_with(Some(check));
}

#[when("the approver signs the prepared publish without a presented digest")]
fn gplus_approve_no_digest(w: &mut ProtocolWorld) {
    let check = approver_receipt(w, "2026-07-04T11:58:00Z", None);
    w.publish_with(Some(check));
}

#[when(expr = "the approver signs the prepared publish with verdict {string}")]
fn gplus_approve_verdict(w: &mut ProtocolWorld, verdict: String) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        &verdict,
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the agent presents an approval receipt bound to other args")]
fn gplus_other_args(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", "sha256:bb22"),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the agent swaps the receipt's presented_digest before appending")]
fn gplus_swap_digest(w: &mut ProtocolWorld) {
    let mut check = w.gplus_checks.take().unwrap();
    check["presented_digest"] = serde_json::json!("sha256:rendered-something-else");
    w.publish_with(Some(check));
}

#[when("the agent presents an approval receipt signed by a stranger key")]
fn gplus_stranger(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(STRANGER),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the second approver signs the prepared publish")]
fn gplus_second_approver(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER2),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the agent presents an approval receipt citing a different obligation id")]
fn gplus_other_obligation(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "other-obligation",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the approver's receipt for a delete is presented on a publish with identical args")]
fn gplus_cross_action(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "delete", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the owner co-signs the prepared send and the agent appends it")]
fn gplus_owner_cosign(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &owner.content_sign,
        "co_sign",
        (&leaf, "send", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-mail-to-alice"),
    );
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "gmail", "send", Some(check));
    w.gamma_result = Some(r);
}

#[when(
    "the first sibling's approval receipt is presented by the second sibling with identical args"
)]
fn gplus_sibling_replay(w: &mut ProtocolWorld) {
    let a_leaf = w.sib_chains[0].last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&a_leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    let b_chain = w.sib_chains[1].clone();
    let r = w.try_action_checked(&b_chain, &agent_sk(SIB), "social", "publish", Some(check));
    w.gamma_result = Some(r);
}

#[when("the second agent signs the prepared publish")]
fn gplus_dual_signs(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(DUAL),
        "four-eyes",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:59:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the agent publishes with both receipts")]
fn gplus_both_receipts(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:00Z",
        None,
    );
    let approve = approver_receipt(w, "2026-07-04T11:58:00Z", Some("sha256:rendered-ship-it"));
    let chain = w.chain.clone();
    let r = w.try_action_checked(
        &chain,
        &agent_sk(AGENT),
        "social",
        "publish",
        Some(serde_json::json!([guard, approve])),
    );
    w.gamma_result = Some(r);
}

#[when("the sub-agent publishes with both receipts")]
fn gplus_sub_both(w: &mut ProtocolWorld) {
    let leaf = w.helper_chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:00Z",
        None,
    );
    let approve = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    let chain = w.helper_chain.clone();
    let r = w.try_action_checked(
        &chain,
        &agent_sk(HELPER),
        "social",
        "publish",
        Some(serde_json::json!([guard, approve])),
    );
    w.gamma_result = Some(r);
}

#[when("the sub-agent publishes with only the guardrail receipt")]
fn gplus_sub_guard_only(w: &mut ProtocolWorld) {
    let leaf = w.helper_chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:00Z",
        None,
    );
    let chain = w.helper_chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(HELPER), "social", "publish", Some(guard));
    w.gamma_result = Some(r);
}

#[when("a sub-mandate is minted with no obligations")]
fn gplus_sub_drops(w: &mut ProtocolWorld) {
    let chain = w
        .mint_sub(
            HELPER,
            "helper",
            "act.x.social.publish",
            serde_json::json!({}),
            false,
        )
        .unwrap();
    w.chain_result = Some(w.verify_chain_at(&chain, GP_AT));
}

#[when("a sub-mandate is minted with the same obligation loosened to 1 hour")]
fn gplus_sub_loosens(w: &mut ProtocolWorld) {
    let mut loosened = approval_ob();
    loosened["max_age"] = serde_json::json!("1h");
    let chain = w
        .mint_sub(
            HELPER,
            "helper",
            "act.x.social.publish",
            serde_json::json!({"obligations": [loosened]}),
            false,
        )
        .unwrap();
    w.chain_result = Some(w.verify_chain_at(&chain, GP_AT));
}

// --- thens ---

#[then("the action appends with the receipt recorded in its checks")]
#[then("the action appends with the co_sign receipt recorded in its checks")]
fn gplus_appended_with_receipt(w: &mut ProtocolWorld) {
    let checks = w.appended_checks().expect("entry should carry checks");
    assert_eq!(
        checks.as_array().map(Vec::len),
        Some(1),
        "one receipt rides"
    );
}

#[then("the action appends with both receipts recorded in its checks")]
fn gplus_appended_with_both(w: &mut ProtocolWorld) {
    let checks = w.appended_checks().expect("entry should carry checks");
    assert_eq!(
        checks.as_array().map(Vec::len),
        Some(2),
        "two receipts ride"
    );
}

#[then("the action appends with no checks recorded")]
fn gplus_appended_bare(w: &mut ProtocolWorld) {
    assert!(w.appended_checks().is_none(), "no checks should ride");
}

#[then("the action is refused as obligation unsatisfied")]
fn gplus_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    match r {
        Err(e) => assert!(
            e.contains("obligation unsatisfied"),
            "want GammaObligationUnsatisfied, got: {e}"
        ),
        Ok(id) => panic!("action should be refused, appended {id}"),
    }
}

#[then("the log gains no entry")]
fn gplus_no_entry(w: &mut ProtocolWorld) {
    let len = w.gbundle().gamma_entries().unwrap().len();
    assert_eq!(
        len, w.gamma_baseline,
        "a refused action must append nothing"
    );
}

#[then("deleting a post without any receipt is refused as obligation unsatisfied")]
fn gplus_delete_bare_refused(w: &mut ProtocolWorld) {
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "social", "delete", None);
    match r {
        Err(e) => assert!(e.contains("obligation unsatisfied"), "got: {e}"),
        Ok(id) => panic!("bare delete should be refused, appended {id}"),
    }
}

#[then("publishing with only the guardrail receipt is refused as obligation unsatisfied")]
fn gplus_guard_only_refused(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:30Z",
        None,
    );
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "social", "publish", Some(guard));
    match r {
        Err(e) => assert!(e.contains("obligation unsatisfied"), "got: {e}"),
        Ok(id) => panic!("guardrail alone should not discharge, appended {id}"),
    }
}

#[then("the chain is refused at verification time")]
fn gplus_chain_refused(w: &mut ProtocolWorld) {
    let r = w.chain_result.as_ref().unwrap();
    assert!(r.is_err(), "chain should be refused, got {r:?}");
}

// --- step H1: merkle state roots (spec 02.10) ---

const H_NOW: &str = "2026-07-09T12:00:00Z";

impl ProtocolWorld {
    fn h_manifest(&mut self) -> Manifest {
        let bytes = self
            .gbundle()
            .store
            .get("manifest.json")
            .unwrap()
            .expect("an edition was published");
        serde_json::from_slice(&bytes).unwrap()
    }

    fn h_tree(&mut self, height: u64) -> aithos_bundle::state::StateTree {
        let bytes = self
            .gbundle()
            .store
            .get(&format!("manifests/tree-{height}.json"))
            .unwrap()
            .expect("tree sidecar");
        serde_json::from_slice(&bytes).unwrap()
    }

    fn h_root(&mut self, zone: &str) -> [u8; 32] {
        let m = self.h_manifest();
        <[u8; 32]>::try_from(hex::decode(&m.roots[zone]).unwrap()).unwrap()
    }

    fn h_add(&mut self, zone: Zone, folder: &str, name: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        let b = self.gbundle();
        if !folder.is_empty() {
            b.ensure_folder(zone, folder, &owner, &mut ent).unwrap();
        }
        b.section_add(
            &SectionSpec {
                zone,
                folder_path: folder,
                name,
                title: "note",
                tags: &[],
                body: BODY,
                now: H_NOW,
            },
            &owner,
            &mut ent,
        )
        .unwrap();
        self.ent = ent;
    }

    fn h_publish(&mut self) {
        let owner = self.owner(0);
        self.gbundle().publish(&owner, H_NOW).unwrap();
    }

    fn h_diff(&mut self) -> std::collections::BTreeMap<String, &'static str> {
        let h = self.h_manifest().edition.height;
        let old = self.h_tree(h - 1);
        let new = self.h_tree(h);
        aithos_bundle::state::tree_diff(&old, &new)
    }
}

// --- H1 givens ---

#[given("a bundle with content in every zone")]
#[given("a published edition with content in every zone")]
fn h_full_bundle(w: &mut ProtocolWorld, step: &cucumber::gherkin::Step) {
    w.init_bundle();
    w.h_add(Zone::Public, "", "bio");
    w.h_add(Zone::Circle, "projets", "note1");
    w.h_add(Zone::Self_, "", "souvenir");
    if step.value.starts_with("a published") {
        w.h_publish();
    }
}

#[given("a bundle whose self zone holds nothing")]
fn h_empty_self(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.h_add(Zone::Public, "", "bio");
}

#[given("a published edition with a circle section under a folder")]
fn h_folder_section(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.h_add(Zone::Circle, "projets", "note1");
    w.h_add(Zone::Circle, "projets", "note2");
    w.h_publish();
}

#[given("a published edition with three self blobs")]
fn h_three_self(w: &mut ProtocolWorld) {
    w.init_bundle();
    for name in ["a", "b", "c"] {
        w.h_add(Zone::Self_, "", name);
    }
    w.h_publish();
}

#[given("a published edition with a circle folder \"archives/old\" holding a section")]
fn h_movable(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.h_add(Zone::Circle, "archives/old", "note1");
    w.h_add(Zone::Circle, "projets", "keep");
    w.h_publish();
}

// --- H1 whens ---

#[when("the owner publishes an edition")]
fn h_when_publish(w: &mut ProtocolWorld) {
    w.h_publish();
}

#[when("the owner adds a circle section and republishes")]
fn h_add_circle_republish(w: &mut ProtocolWorld) {
    w.h_add(Zone::Circle, "projets", "note-h");
    w.h_publish();
}

#[when("the owner adds one more circle section and republishes")]
fn h_add_one_republish(w: &mut ProtocolWorld) {
    w.h_add(Zone::Circle, "projets", "fresh");
    w.h_publish();
}

#[when("the owner republishes without any change")]
fn h_republish(w: &mut ProtocolWorld) {
    w.h_publish();
}

#[when("a verifier asks for the section's inclusion proof")]
fn h_ask_proof(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    w.h_proof = Some(p);
}

#[when("the mirror alters the section's title inside the proven row")]
fn h_tamper_row(w: &mut ProtocolWorld) {
    let mut p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    let bytes = hex::decode(&p.payload).unwrap();
    let (row, hh) = bytes.split_at(bytes.len() - 32);
    let mut row: serde_json::Value = serde_json::from_slice(row).unwrap();
    row["title"] = serde_json::json!("tampered");
    let mut payload = aithos_core::jcs::canonical_bytes(&row).unwrap();
    payload.extend_from_slice(hh);
    p.payload = hex::encode(payload);
    w.h_proof = Some(p);
}

#[when("the owner grants an agent the folder and republishes")]
fn h_grant_republish(w: &mut ProtocolWorld) {
    let old = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    w.h_old_proof = Some(old);
    w.grant_to_agent(
        &[GrantSpec {
            zone: Zone::Circle,
            dir: "projets".into(),
            tag: None,
        }],
        NA30,
        1,
    );
    w.h_publish();
}

#[when("the mirror forges a proof that treats a leaf hash as an interior node")]
fn h_forge_leaf_as_node(w: &mut ProtocolWorld) {
    let mut p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    // replace the sibling H_node step with an H_leaf wrap over the same bytes
    if let aithos_core::merkle::ProofStep::Node { hash, .. } = p.steps[0].clone() {
        p.steps[0] = aithos_core::merkle::ProofStep::Wrap {
            pre: String::new(),
            post: hash,
        };
    }
    w.h_proof = Some(p);
}

#[when("the mirror forges a proof that presents an interior hash as a leaf")]
fn h_forge_node_as_leaf(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    // claim the two sibling leaves' concatenation as a leaf payload and
    // skip the H_node step: H_leaf(l ‖ r) ≠ H_node(l, r) by domain
    let leaf = aithos_core::merkle::h_leaf(&hex::decode(&p.payload).unwrap());
    let aithos_core::merkle::ProofStep::Node { hash, side } = p.steps[0].clone() else {
        panic!("first step should be the sibling");
    };
    let sib: [u8; 32] = hex::decode(&hash).unwrap().try_into().unwrap();
    let mut spliced = Vec::new();
    match side {
        aithos_core::merkle::Side::Right => {
            spliced.extend_from_slice(&leaf);
            spliced.extend_from_slice(&sib);
        }
        aithos_core::merkle::Side::Left => {
            spliced.extend_from_slice(&sib);
            spliced.extend_from_slice(&leaf);
        }
    }
    let forged = aithos_core::merkle::Proof {
        payload: hex::encode(spliced),
        steps: p.steps[1..].to_vec(),
        root: p.root,
    };
    w.h_proof = Some(forged);
}

#[when("a verifier asks for one self blob's inclusion proof")]
fn h_ask_self_proof(w: &mut ProtocolWorld) {
    let index: aithos_bundle::bundle::SelfIndex =
        serde_json::from_slice(&w.gbundle().store.get("e/self/index.json").unwrap().unwrap())
            .unwrap();
    let sid = index.blobs[1].sid.clone();
    let p = w.gbundle().prove_self(&sid).unwrap();
    w.h_proof = Some(p);
}

#[when("the owner moves the folder under \"projets\" and republishes")]
fn h_move_republish(w: &mut ProtocolWorld) {
    let old = w
        .gbundle()
        .prove_section(Zone::Circle, "archives/old/note1")
        .unwrap();
    w.h_old_proof = Some(old);
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .move_folder(&owner, "archives/old", "projets", &mut ent)
        .unwrap();
    w.ent = ent;
    w.h_publish();
}

// --- H1 thens ---

#[then("the manifest pins a root for public, circle, self and the vault")]
fn h_four_roots(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    for zone in ["public", "circle", "self", "vault"] {
        let r = m.roots.get(zone).unwrap_or_else(|| panic!("{zone} root"));
        assert_eq!(r.len(), 64, "{zone} root must be 32 hex bytes");
    }
}

#[then("the flat file pins are still present and verify")]
fn h_flat_pins_verify(w: &mut ProtocolWorld) {
    assert!(
        !w.h_manifest().files.is_empty(),
        "flat pins ride beside roots"
    );
    w.gbundle().verify().expect("the edition must verify");
}

#[then("an independent recomputation from the store yields the same four roots")]
fn h_recompute(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let tree = w.gbundle().state_tree().unwrap();
    assert_eq!(tree.roots, m.roots, "recomputed roots must match");
}

#[then("the self root is thirty-two zero bytes")]
fn h_self_empty(w: &mut ProtocolWorld) {
    assert_eq!(w.h_manifest().roots["self"], "00".repeat(32));
}

#[then("the circle root changes")]
fn h_circle_changed(w: &mut ProtocolWorld) {
    let h = w.h_manifest().edition.height;
    let prev: Manifest = serde_json::from_slice(
        &w.gbundle()
            .store
            .get(&format!("manifests/{}.json", h - 1))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_ne!(w.h_manifest().roots["circle"], prev.roots["circle"]);
}

#[then("the public root and the self root are unchanged")]
fn h_others_unchanged(w: &mut ProtocolWorld) {
    let h = w.h_manifest().edition.height;
    let prev: Manifest = serde_json::from_slice(
        &w.gbundle()
            .store
            .get(&format!("manifests/{}.json", h - 1))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let m = w.h_manifest();
    assert_eq!(m.roots["public"], prev.roots["public"]);
    assert_eq!(m.roots["self"], prev.roots["self"]);
}

#[then("the proof verifies against the circle root of the signed manifest")]
fn h_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h_root("circle");
    let p = w.h_proof.as_ref().unwrap();
    aithos_core::merkle::verify_proof(p, &root).expect("proof must verify");
}

#[then("the proof is refused")]
fn h_proof_refused(w: &mut ProtocolWorld) {
    let root = w.h_root("circle");
    let p = w.h_proof.as_ref().unwrap();
    let got = aithos_core::merkle::verify_proof(p, &root);
    assert!(
        matches!(got, Err(aithos_core::error::Error::MerkleProofInvalid(_))),
        "expected MerkleProofInvalid, got {got:?}"
    );
}

#[then("the old edition's proof for the folder no longer verifies against the new root")]
#[then("the old edition's proof for the section no longer verifies against the new root")]
fn h_old_proof_dead(w: &mut ProtocolWorld) {
    let root = w.h_root("circle");
    let p = w.h_old_proof.as_ref().unwrap();
    assert!(
        aithos_core::merkle::verify_proof(p, &root).is_err(),
        "the old proof must die against the new root"
    );
}

#[then("a fresh proof carries the new header hash and verifies")]
fn h_fresh_proof(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    let root = w.h_root("circle");
    aithos_core::merkle::verify_proof(&p, &root).expect("fresh proof verifies");
    let old = w.h_old_proof.as_ref().unwrap();
    assert_ne!(
        serde_json::to_string(&p.steps).unwrap(),
        serde_json::to_string(&old.steps).unwrap(),
        "the folder prefix must carry the new header"
    );
}

#[then("the section proves against the new root through its new address")]
fn h_proof_new_address(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/old/note1")
        .unwrap();
    let root = w.h_root("circle");
    aithos_core::merkle::verify_proof(&p, &root).expect("new-address proof verifies");
}

#[then("the proof verifies against the self root")]
fn h_self_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h_root("self");
    let p = w.h_proof.as_ref().unwrap();
    aithos_core::merkle::verify_proof(p, &root).expect("self proof verifies");
}

#[then("the proof carries no name, no path and no sibling row")]
fn h_self_proof_opaque(w: &mut ProtocolWorld) {
    let p = w.h_proof.as_ref().unwrap();
    assert!(
        p.steps
            .iter()
            .all(|s| matches!(s, aithos_core::merkle::ProofStep::Node { .. })),
        "self proofs are sibling hashes only"
    );
    let json = serde_json::to_string(p).unwrap();
    for leak in ["name", "title", "wrap", "souvenir"] {
        assert!(!json.contains(leak), "self proof leaks '{leak}'");
    }
}

#[then("the edition diff descends to exactly the new section")]
fn h_diff_exact(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    let added: Vec<&String> = diff
        .iter()
        .filter(|(k, v)| **v == "added" && k.contains("/s/"))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(added.len(), 1, "exactly one section appears: {diff:?}");
    assert!(added[0].starts_with("circle:d/"), "it is the circle one");
}

#[then("no other zone appears in the diff")]
fn h_diff_one_zone(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    assert!(
        diff.keys().all(|k| k.starts_with("circle:")),
        "only circle may change: {diff:?}"
    );
}

#[then("the edition diff descends into both the old and the new parent")]
fn h_diff_both_parents(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    let removed = diff.values().filter(|v| **v == "removed").count();
    let added = diff.values().filter(|v| **v == "added").count();
    assert!(removed > 0, "the old parent loses the subtree: {diff:?}");
    assert!(added > 0, "the new parent gains the subtree: {diff:?}");
}

#[then("the edition diff is empty")]
fn h_diff_empty(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    assert!(diff.is_empty(), "no change, no diff: {diff:?}");
}

// -------------------------------------- step H2: committed gamma roots
// Spec 07.10 — per-segment roots + counts trie in the manifest, count /
// absence / completeness proofs over the log (h2-gamma-roots.feature).

use aithos_core::gamma::{
    counts_tally, prove_absence, prove_count, prove_entry, segment_root, verify_absence,
    verify_complete_actions, verify_count_proof, AbsenceProof, GammaCounters,
};

impl ProtocolWorld {
    fn h2_tallies(&mut self) -> std::collections::BTreeMap<String, GammaCounters> {
        counts_tally(&self.gbundle().gamma_entries().unwrap())
    }

    fn h2_counts_root(&mut self) -> [u8; 32] {
        <[u8; 32]>::try_from(hex::decode(self.h_manifest().gamma_counts_root).unwrap()).unwrap()
    }

    fn h2_segment_root(&mut self, month: &str) -> [u8; 32] {
        let hex_root = self.h_manifest().gamma_roots[month].root.clone();
        <[u8; 32]>::try_from(hex::decode(hex_root).unwrap()).unwrap()
    }

    /// Inclusion proofs for every `action` entry running under `mandate_id`,
    /// segment by segment — the mirror's honest answer.
    fn h2_action_proofs(&mut self, mandate_id: &str) -> Vec<(String, aithos_core::merkle::Proof)> {
        let mut out = Vec::new();
        let months: Vec<String> = self.h_manifest().gamma_roots.keys().cloned().collect();
        for month in months {
            let lines = self.segment_lines(&format!("gamma/{month}.jsonl"));
            let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
            for (i, line) in lines.iter().enumerate() {
                let e: aithos_core::gamma::Entry = serde_json::from_slice(line).unwrap();
                let runs_under = e.kind == "action"
                    && e.authorized_via
                        .as_ref()
                        .is_some_and(|v| v.iter().any(|m| m == mandate_id));
                if runs_under {
                    out.push((month.clone(), prove_entry(&refs, i).unwrap()));
                }
            }
        }
        out
    }
}

#[given("a bundle whose log spans two months of delegated actions")]
fn h2_two_months(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), "2026-08-31T00:00:00Z");
    w.try_action(false, "reply", &day(1, "10:00:00")).unwrap();
    w.try_action(false, "reply", &day(32, "10:00:00")).unwrap();
}

#[given("a bundle whose log is empty")]
fn h2_empty_log(w: &mut ProtocolWorld) {
    w.init_bundle();
}

#[given("a published edition whose log counts a delegated action")]
fn h2_one_action(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(1, "10:00:00")).unwrap();
    w.h_publish();
}

#[given(
    "a published edition whose log counts actions, a sub-grant and budget tokens under a mandate"
)]
fn h2_full_meter(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"budgets": [{"id": "haiku", "token_budget": 100000}]}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    w.try_action_full(
        false,
        "reply",
        &day(1, "10:00:00"),
        Some(serde_json::json!({"budget_ref": "haiku", "tokens": 2700})),
        None,
    )
    .unwrap();
    w.try_inference(
        "claude-haiku",
        1200,
        300,
        Some("haiku"),
        &day(1, "11:00:00"),
    )
    .unwrap();
    w.h_publish();
}

#[given("a published edition whose log holds an action by a sub-delegate")]
#[given("a published edition whose log counts two mandates apart in id order")]
fn h2_sub_delegate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    w.try_action(true, "reply", &day(1, "10:00:00")).unwrap();
    w.h_publish();
}

#[given("a published edition whose log counts three mandates")]
fn h2_three_mandates(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    let first_chain = w.helper_chain.clone();
    w.delegate_act("act.x.gmail.label", serde_json::json!({}), true)
        .unwrap();
    w.try_action(true, "label", &day(1, "10:00:00")).unwrap();
    w.helper_chain = first_chain;
    w.try_action(true, "reply", &day(2, "10:00:00")).unwrap();
    w.h_publish();
    assert_eq!(w.h2_tallies().len(), 3, "root and two children counted");
}

#[given("a published edition whose log counts three actions under a mandate")]
fn h2_three_actions(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    for (d, hms) in [(1, "10:00:00"), (2, "10:00:00"), (3, "10:00:00")] {
        w.try_action(false, "reply", &day(d, hms)).unwrap();
    }
    w.h_publish();
}

#[given("a published edition whose log spans one month")]
fn h2_one_month(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(1, "10:00:00")).unwrap();
    w.try_action(false, "reply", &day(2, "10:00:00")).unwrap();
    w.h_publish();
}

#[then("the manifest commits a gamma root and entry count for each of the two segments")]
fn h2_two_segments_committed(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let months: Vec<String> = m.gamma_roots.keys().cloned().collect();
    assert_eq!(
        months,
        ["2026-07", "2026-08"],
        "one root per non-empty month"
    );
    for (month, seg) in &m.gamma_roots {
        assert_eq!(seg.root.len(), 64, "{month}: hex root");
        assert!(seg.n > 0, "{month}: committed entry count");
    }
}

#[then("the manifest commits a gamma counts root")]
fn h2_counts_root_committed(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    assert_eq!(m.gamma_counts_root.len(), 64);
    assert_ne!(m.gamma_counts_root, "0".repeat(64), "mandates were counted");
}

#[then("the content roots and the flat file pins still verify")]
fn h2_additive(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    assert_eq!(m.roots.len(), 4, "the §02.10 roots still ride");
    assert!(!m.files.is_empty(), "the flat pins still ride");
    w.gbundle().verify().unwrap();
}

#[then("an independent recomputation from the store yields the same gamma roots and counts root")]
fn h2_recompute_identical(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let (roots, counts) = w.gbundle().gamma_state().unwrap();
    assert_eq!(roots, m.gamma_roots);
    assert_eq!(counts, m.gamma_counts_root);
}

#[then("the manifest commits no gamma segment roots")]
fn h2_no_segments(w: &mut ProtocolWorld) {
    assert!(w.h_manifest().gamma_roots.is_empty());
}

#[then("the gamma counts root is thirty-two zero bytes")]
fn h2_zero_counts_root(w: &mut ProtocolWorld) {
    assert_eq!(w.h_manifest().gamma_counts_root, "0".repeat(64));
}

#[when("a mirror rewrites a clear counter field inside the last log entry")]
fn h2_tamper_last_entry(w: &mut ProtocolWorld) {
    let seg = "gamma/2026-07.jsonl";
    let mut lines = w.segment_lines(seg);
    let mut entry: serde_json::Value = serde_json::from_slice(lines.last().unwrap()).unwrap();
    entry["payload"]["action"] = serde_json::Value::String("send".into());
    *lines.last_mut().unwrap() = aithos_core::jcs::canonical_bytes(&entry).unwrap();
    let mut bytes = Vec::new();
    for line in &lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[then("edition verification is refused")]
fn h2_verify_refused(w: &mut ProtocolWorld) {
    // Root recomputation ALONE catches it (the rule under test)…
    let m = w.h_manifest();
    let (roots, _) = w.gbundle().gamma_state().unwrap();
    assert_ne!(roots, m.gamma_roots, "the recomputed segment root died");
    // …and full verification is refused, fail-closed.
    assert!(w.gbundle().verify().is_err());
}

#[when("a verifier asks for the action entry's inclusion proof")]
fn h2_prove_entry(w: &mut ProtocolWorld) {
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
    let idx = lines
        .iter()
        .position(|l| {
            serde_json::from_slice::<aithos_core::gamma::Entry>(l)
                .unwrap()
                .kind
                == "action"
        })
        .expect("the fixture logged an action");
    w.h2_proof = Some(prove_entry(&refs, idx).unwrap());
}

#[then("the gamma proof verifies offline against the committed segment root")]
fn h2_entry_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h2_segment_root("2026-07");
    let proof = w.h2_proof.as_ref().unwrap();
    aithos_core::merkle::verify_proof(proof, &root).unwrap();
}

#[when("the mirror alters the entry's action name inside the proven bytes")]
fn h2_tamper_proven_bytes(w: &mut ProtocolWorld) {
    h2_prove_entry(w);
    let proof = w.h2_proof.as_mut().unwrap();
    let mut entry: serde_json::Value =
        serde_json::from_slice(&hex::decode(&proof.payload).unwrap()).unwrap();
    entry["payload"]["action"] = serde_json::Value::String("send".into());
    proof.payload = hex::encode(aithos_core::jcs::canonical_bytes(&entry).unwrap());
}

#[then("the gamma proof is refused")]
fn h2_entry_proof_refused(w: &mut ProtocolWorld) {
    let root = w.h2_segment_root("2026-07");
    let proof = w.h2_proof.as_ref().unwrap();
    let err = aithos_core::merkle::verify_proof(proof, &root).unwrap_err();
    assert!(matches!(
        err,
        aithos_core::error::Error::MerkleProofInvalid(_)
    ));
}

#[when("a verifier asks for the mandate's count proof")]
fn h2_prove_count(w: &mut ProtocolWorld) {
    let id = w.chain[0].id.clone();
    let tallies = w.h2_tallies();
    w.h2_proof = Some(prove_count(&tallies, &id).unwrap());
}

#[then("the count proof verifies offline against the committed counts root")]
fn h2_count_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h2_counts_root();
    let proof = w.h2_proof.clone().unwrap();
    let (id, counters) = verify_count_proof(&proof, &root).unwrap();
    w.h2_counters = vec![(id, counters)];
}

#[then("the proven counters equal the raw tallies of the chain")]
fn h2_counters_equal_tallies(w: &mut ProtocolWorld) {
    let (id, counters) = w.h2_counters[0].clone();
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(
        counters.actions,
        aithos_core::gamma::count_actions(&entries, &id, None, None) as u64,
        "actions == raw subtree tally"
    );
    assert_eq!(
        counters.children,
        aithos_core::gamma::count_children(&entries, &id) as u64,
        "children == raw tally"
    );
    let haiku = &counters.budgets["haiku"];
    assert_eq!(
        haiku.actions,
        aithos_core::constraints::count_profile_entries(&entries, &id, "haiku") as u64,
        "budget actions == raw tally"
    );
    assert_eq!(
        haiku.tokens,
        aithos_core::constraints::tally_tokens(&entries, &id, "haiku"),
        "budget tokens == raw tally (attested override included)"
    );
    let raw_entries = entries
        .iter()
        .filter(|e| e.authorized_via.as_ref().is_some_and(|v| v.contains(&id)))
        .count() as u64;
    assert_eq!(counters.entries, raw_entries, "entries == the audit total");
}

#[when("a verifier proves the counts of the root mandate and of the leaf mandate")]
fn h2_prove_both(w: &mut ProtocolWorld) {
    let root_id = w.chain[0].id.clone();
    let leaf_id = w.helper_chain[1].id.clone();
    let tallies = w.h2_tallies();
    let pinned = w.h2_counts_root();
    w.h2_counters = [root_id, leaf_id]
        .iter()
        .map(|id| {
            let proof = prove_count(&tallies, id).unwrap();
            verify_count_proof(&proof, &pinned).unwrap()
        })
        .collect();
}

#[then("both count leaves carry that action")]
fn h2_both_carry(w: &mut ProtocolWorld) {
    for (id, counters) in &w.h2_counters {
        assert_eq!(
            counters.actions, 1,
            "{id}: the sub-delegate's action counts"
        );
    }
}

#[when("a verifier asks whether an id between them was ever counted")]
fn h2_ask_absence(w: &mut ProtocolWorld) {
    let tallies = w.h2_tallies();
    let ids: Vec<&String> = tallies.keys().collect();
    assert_eq!(ids.len(), 2);
    let absent = format!("{}0", ids[0]); // strictly between the two, byte order
    assert!(ids[0] < &absent && &absent < ids[1]);
    let pinned = w.h2_counts_root();
    let proof = prove_absence(&tallies, &absent).unwrap();
    assert!(
        proof.left.is_some() && proof.right.is_some(),
        "two bracketing leaves"
    );
    w.h2_verdict = Some(verify_absence(&absent, &proof, &pinned).map_err(|e| e.to_string()));
}

#[then("the mirror proves absence with two adjacent leaves bracketing the id")]
fn h2_absence_ok(w: &mut ProtocolWorld) {
    assert_eq!(w.h2_verdict.clone().unwrap(), Ok(()));
}

#[when("the mirror claims absence of the middle one with the outer two leaves")]
fn h2_forge_absence(w: &mut ProtocolWorld) {
    let tallies = w.h2_tallies();
    let ids: Vec<String> = tallies.keys().cloned().collect();
    let forged = AbsenceProof {
        left: Some(prove_count(&tallies, &ids[0]).unwrap()),
        right: Some(prove_count(&tallies, &ids[2]).unwrap()),
    };
    let pinned = w.h2_counts_root();
    w.h2_verdict = Some(verify_absence(&ids[1], &forged, &pinned).map_err(|e| e.to_string()));
}

#[then("the absence proof is refused")]
fn h2_absence_refused(w: &mut ProtocolWorld) {
    let msg = w.h2_verdict.clone().unwrap().unwrap_err();
    assert!(msg.contains("absence proof invalid"), "{msg}");
}

#[when("a mirror answers the mandate's action query with only two proven entries")]
fn h2_withhold_answer(w: &mut ProtocolWorld) {
    let id = w.chain[0].id.clone();
    let proofs = w.h2_action_proofs(&id);
    assert_eq!(proofs.len(), 3, "the honest answer would be three");
    let tallies = w.h2_tallies();
    let count_proof = prove_count(&tallies, &id).unwrap();
    let pinned = w.h2_counts_root();
    let m = w.h_manifest();
    let segment_roots = m
        .gamma_roots
        .iter()
        .map(|(month, seg)| {
            let root = <[u8; 32]>::try_from(hex::decode(&seg.root).unwrap()).unwrap();
            (month.clone(), root)
        })
        .collect();
    w.h2_verdict = Some(
        verify_complete_actions(&id, &count_proof, &proofs[..2], &segment_roots, &pinned)
            .map(|_| ())
            .map_err(|e| e.to_string()),
    );
}

#[then("the answer is refused against the proven count of three")]
fn h2_withhold_refused(w: &mut ProtocolWorld) {
    let msg = w.h2_verdict.clone().unwrap().unwrap_err();
    assert!(msg.contains("withhold detected"), "{msg}");
    assert!(msg.contains("proven count of 3"), "{msg}");
}

#[when("a mirror serves the segment with one entry withheld")]
fn h2_withhold_segment(w: &mut ProtocolWorld) {
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let served: Vec<&[u8]> = lines
        .iter()
        .take(lines.len() - 1)
        .map(Vec::as_slice)
        .collect();
    let committed = w.h_manifest().gamma_roots["2026-07"].clone();
    let recomputed = segment_root(&served);
    w.h2_verdict = Some(
        if hex::encode(recomputed) == committed.root || served.len() as u64 == committed.n {
            Ok(())
        } else {
            Err("segment omission died on root and count".into())
        },
    );
}

#[then("the recomputed segment root dies against the committed root and count")]
fn h2_segment_omission_dies(w: &mut ProtocolWorld) {
    assert!(w.h2_verdict.clone().unwrap().is_err());
}

// ------------------------------------------- step I: concurrency (02.6 + 07.6)
// Disjoint merge, the two-predecessor merge entry, 3-way index merge by
// sid, fork refusal and nearest-common-manager resolution
// (i-concurrency.feature).

use aithos_bundle::merge::ForkResolver;

// The I timeline sits after the D/H fixture NOW (2026-07-09).
const I_ANC: &str = "2026-07-10T00:00:00Z"; // the shared ancestor edition
const I_W1: &str = "2026-07-10T01:00:00Z"; // this copy's write
const I_W2: &str = "2026-07-10T02:00:00Z"; // the other copy's write
const I_PUB: &str = "2026-07-10T03:00:00Z"; // both competing editions
const I_MERGE: &str = "2026-07-10T04:00:00Z"; // the merge / resolution
const I_AFTER: &str = "2026-07-10T05:00:00Z"; // life after the join

impl ProtocolWorld {
    /// The §02.6 fixture: publish the shared ancestor edition, then split
    /// the world into two byte-identical copies of the whole store.
    fn i_split(&mut self) {
        let owner = self.owner(0);
        self.gbundle().publish(&owner, I_ANC).unwrap();
        let b = self.bundle.as_ref().unwrap();
        self.i_other = Some(Bundle {
            store: b.store.clone(),
            did: b.did.clone(),
        });
    }

    fn i_bundle(&mut self, other: bool) -> &mut Bundle<MemStore> {
        if other {
            self.i_other.as_mut().unwrap()
        } else {
            self.bundle.as_mut().unwrap()
        }
    }

    fn i_add_on(&mut self, other: bool, folder: &str, name: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        let b = self.i_bundle(other);
        b.ensure_folder(Zone::Circle, folder, &owner, &mut ent)
            .unwrap();
        b.section_add(
            &SectionSpec {
                zone: Zone::Circle,
                folder_path: folder,
                name,
                title: "note",
                tags: &[],
                body: BODY,
                now: at,
            },
            &owner,
            &mut ent,
        )
        .unwrap();
        self.ent = ent;
    }

    fn i_rewrite_on(&mut self, other: bool, path: &str, body: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.i_bundle(other)
            .section_rewrite(Zone::Circle, path, body, &owner, at, &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn i_delete_on(&mut self, other: bool, path: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.i_bundle(other)
            .section_delete(Zone::Circle, path, &owner, at, &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn i_publish_on(&mut self, other: bool, at: &str) {
        let owner = self.owner(0);
        self.i_bundle(other).publish(&owner, at).unwrap();
    }

    fn i_action_on(&mut self, other: bool, action: &str, at: &str) -> Result<String, String> {
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .i_bundle(other)
            .log_action(
                &chain,
                &agent_sk(AGENT),
                &aithos_bundle::log::ActionSpec {
                    connector: "gmail",
                    action,
                    args_hash: "sha256:00",
                    now: at,
                    budget: None,
                    sealed_args: None,
                },
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    /// Merge the other copy's competing edition into this one.
    fn i_merge(&mut self) -> Result<(), String> {
        let owner = self.owner(0);
        let other = self.i_other.take().unwrap();
        let r = self
            .gbundle()
            .edition_merge(&other, &owner, I_MERGE)
            .map_err(|e| e.to_string());
        self.i_other = Some(other);
        r
    }

    /// Root WRITE mandate over one circle folder — the delegate a fork
    /// resolution names as its nearest common manager.
    fn i_grant_write_dir(&mut self, folder: &str) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, Verb};
        let owner = self.owner(0);
        let dir = self
            .bundle
            .as_ref()
            .unwrap()
            .resolve_folder(Zone::Circle, folder)
            .unwrap();
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 970)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter: vec![aithos_core::mandate::PerimeterEntry::Ethos {
                    verb: Verb::Write,
                    zone: Zone::Circle,
                    dir,
                    tag: None,
                }],
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: NA30.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    fn i_resolve(&mut self, delegate: bool) -> Result<Vec<String>, String> {
        let owner = self.owner(0);
        let chain = self.chain.clone();
        let sk = agent_sk(AGENT);
        let resolver = if delegate {
            ForkResolver::Delegate {
                chain: &chain,
                sk: &sk,
            }
        } else {
            ForkResolver::Owner(&owner)
        };
        let other = self.i_other.take().unwrap();
        let r = self
            .bundle
            .as_mut()
            .unwrap()
            .resolve_fork(&other, &resolver, I_MERGE)
            .map_err(|e| e.to_string());
        self.i_other = Some(other);
        r
    }

    fn i_parent_manifests(&mut self) -> (Manifest, Manifest) {
        let h = self.h_manifest().edition.height;
        let low: Manifest = serde_json::from_slice(
            &self
                .gbundle()
                .store
                .get(&format!("manifests/{}.json", h - 1))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let alt: Manifest = serde_json::from_slice(
            &self
                .gbundle()
                .store
                .get(&format!("manifests/{}-alt.json", h - 1))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        (low, alt)
    }
}

// --- I givens ---

#[given("two copies of a published bundle")]
fn i_two_copies(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "seed", &[]);
    w.i_split();
}

#[given("two copies of a published bundle holding a circle section")]
fn i_two_copies_with_section(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.i_split();
}

#[given("two copies of a published bundle whose agents each logged an action")]
fn i_two_copies_with_actions(w: &mut ProtocolWorld) {
    w.init_bundle();
    // A logged mutation gives the shared log a real tip to fork from.
    w.add_named_section("projets", "seed", &[]);
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.i_split();
    w.i_action_on(false, "reply", I_W1).unwrap();
    w.i_action_on(true, "label", I_W2).unwrap();
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("two copies of a published bundle whose agent may act three times in total")]
fn i_two_copies_budget(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "seed", &[]);
    w.grant_act(vec![], serde_json::json!({ "max_actions": 3 }), NA30);
    w.i_split();
}

#[given("each copy adds a circle section under a different folder")]
fn i_disjoint_adds(w: &mut ProtocolWorld) {
    w.i_add_on(false, "alpha", "note-a", I_W1);
    w.i_add_on(true, "beta", "note-b", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("each copy adds a differently-named section under the same folder")]
fn i_same_folder_adds(w: &mut ProtocolWorld) {
    w.i_add_on(false, "projets", "note-a", I_W1);
    w.i_add_on(true, "projets", "note-b", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("one copy deletes that section while the other adds a sibling")]
fn i_delete_vs_add(w: &mut ProtocolWorld) {
    w.i_delete_on(true, "projets/note1", I_W2);
    w.i_add_on(false, "projets", "note2", I_W1);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("each copy modifies that same section differently")]
fn i_same_node_writes(w: &mut ProtocolWorld) {
    w.i_rewrite_on(false, "projets/note1", "the winning body", I_W1);
    w.i_rewrite_on(true, "projets/note1", "the losing body", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("each copy logs two actions under that mandate")]
fn i_two_actions_each(w: &mut ProtocolWorld) {
    w.i_action_on(false, "reply", I_W1).unwrap();
    w.i_action_on(false, "reply", "2026-07-10T01:30:00Z")
        .unwrap();
    w.i_action_on(true, "label", I_W2).unwrap();
    w.i_action_on(true, "label", "2026-07-10T02:30:00Z")
        .unwrap();
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("two competing editions modifying the same section")]
fn i_fork_fixture(w: &mut ProtocolWorld) {
    i_two_copies_with_section(w);
    i_same_node_writes(w);
}

#[given("two competing editions modifying the same section under a delegate's folder")]
fn i_fork_under_delegate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.i_grant_write_dir("projets");
    w.i_split();
    i_same_node_writes(w);
}

#[given("two competing editions touching a folder outside the delegate's grant")]
fn i_fork_outside_delegate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.add_named_section("autre", "note2", &[]);
    w.i_grant_write_dir("projets");
    w.i_split();
    w.i_rewrite_on(false, "autre/note2", "the winning body", I_W1);
    w.i_rewrite_on(true, "autre/note2", "the losing body", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

// --- I whens ---

#[when("either party publishes the merge edition")]
fn i_when_merge(w: &mut ProtocolWorld) {
    w.i_result = Some(w.i_merge());
}

#[when("a party attempts the merge edition")]
fn i_when_merge_attempt(w: &mut ProtocolWorld) {
    w.i_result = Some(w.i_merge());
}

#[when("each party computes the merge edition independently")]
fn i_when_both_merge(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    // Pre-merge clones so each party merges from the same starting point.
    let mine = Bundle {
        store: w.bundle.as_ref().unwrap().store.clone(),
        did: w.bundle.as_ref().unwrap().did.clone(),
    };
    let mut theirs = Bundle {
        store: w.i_other.as_ref().unwrap().store.clone(),
        did: w.i_other.as_ref().unwrap().did.clone(),
    };
    w.i_merge().unwrap();
    theirs.edition_merge(&mine, &owner, I_MERGE).unwrap();
    for b in [w.bundle.as_ref().unwrap(), &theirs] {
        let bytes = b.store.get("manifest.json").unwrap().unwrap();
        w.i_hashes.push(sha256_hex(&bytes));
    }
}

#[when("a verifier is shown both branches")]
fn i_when_fork_check(w: &mut ProtocolWorld) {
    let other = w.i_other.take().unwrap();
    w.i_result = Some(w.gbundle().fork_check(&other).map_err(|e| e.to_string()));
    w.i_other = Some(other);
}

#[when("the covering delegate publishes the resolving edition naming the winner")]
fn i_when_delegate_resolves(w: &mut ProtocolWorld) {
    w.i_surfaced = Some(w.i_resolve(true));
}

#[when("the delegate attempts the resolving edition")]
fn i_when_delegate_attempts(w: &mut ProtocolWorld) {
    w.i_surfaced = Some(w.i_resolve(true));
}

#[when("the owner publishes the resolving edition naming the winner")]
fn i_when_owner_resolves(w: &mut ProtocolWorld) {
    w.i_surfaced = Some(w.i_resolve(false));
}

// --- I thens ---

#[then("the merge manifest pins the lowest-hash parent and lists both parents ascending")]
fn i_then_ordering(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let m = w.h_manifest();
    assert_eq!(m.merges.len(), 2, "two parents");
    assert!(m.merges[0] < m.merges[1], "ascending edition hashes");
    assert_eq!(m.edition.prev_hash, m.merges[0], "prev_hash = the lowest");
    let (low, alt) = w.i_parent_manifests();
    assert_eq!(low.chain_hash().unwrap(), m.merges[0], "low parent slot");
    assert_eq!(alt.chain_hash().unwrap(), m.merges[1], "high parent slot");
}

#[then("both sections are present and the edition verifies")]
fn i_then_both_present(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let b = w.gbundle();
    assert_eq!(
        b.read_section(Zone::Circle, "alpha/note-a", &owner)
            .unwrap(),
        BODY
    );
    assert_eq!(
        b.read_section(Zone::Circle, "beta/note-b", &owner).unwrap(),
        BODY
    );
    b.verify().expect("the merged edition verifies");
}

#[then("the two merged manifests hash identically")]
fn i_then_identical(w: &mut ProtocolWorld) {
    assert_eq!(w.i_hashes.len(), 2);
    assert_eq!(
        w.i_hashes[0], w.i_hashes[1],
        "byte-identical merge manifests from either merger"
    );
}

#[then("the folder's index carries both rows in sid order")]
fn i_then_sid_order(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let index: ZoneIndex = serde_json::from_slice(
        &w.gbundle()
            .store
            .get("e/circle/index.json")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let names: Vec<&str> = index.sections.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"note-a") && names.contains(&"note-b"));
    let sids: Vec<&String> = index.sections.iter().map(|r| &r.sid).collect();
    let mut sorted = sids.clone();
    sorted.sort();
    assert_eq!(sids, sorted, "rows land in sid order");
}

#[then("the edition verifies")]
fn i_then_verifies(w: &mut ProtocolWorld) {
    w.gbundle().verify().expect("the merged edition verifies");
}

#[then("the deleted section stays absent from the merged index")]
fn i_then_deletion_holds(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let index: ZoneIndex = serde_json::from_slice(
        &w.gbundle()
            .store
            .get("e/circle/index.json")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(
        !index.sections.iter().any(|r| r.name == "note1"),
        "no resurrection through the merge"
    );
}

#[then("the sibling is present")]
fn i_then_sibling(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let b = w.gbundle();
    assert_eq!(
        b.read_section(Zone::Circle, "projets/note2", &owner)
            .unwrap(),
        BODY
    );
    b.verify().expect("the merged edition verifies");
}

#[then("the merge is refused as a same-node conflict")]
fn i_then_conflict(w: &mut ProtocolWorld) {
    let err = w
        .i_result
        .clone()
        .unwrap()
        .expect_err("the merge is refused");
    assert!(err.contains("same-node conflict"), "got: {err}");
}

#[then("the merge entry cites both sub-chain tips in prevs")]
fn i_then_merge_entry(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let m = w.h_manifest();
    let (low, alt) = w.i_parent_manifests();
    let entries = w.gbundle().gamma_entries().unwrap();
    let join = entries.last().unwrap();
    assert_eq!(join.kind, "merge");
    assert_eq!(join.chain_hash().unwrap(), m.gamma_head);
    assert_eq!(
        join.prevs.clone().unwrap(),
        vec![low.gamma_head, alt.gamma_head],
        "both tips, ordered like merges"
    );
}

#[then("the merged log verifies from genesis through the join")]
fn i_then_log_verifies(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    aithos_core::gamma::verify_links(&entries).expect("links verify through the join");
    w.gbundle().gamma_verify().expect("every entry verifies");
    w.gbundle().verify().expect("the edition verifies");
}

#[then("the merged segment lays out the lowest-hash parent's entries first")]
fn i_then_layout(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let (low, alt) = w.i_parent_manifests();
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let pos = |head: &str| {
        lines
            .iter()
            .position(|l| {
                let e: aithos_core::gamma::Entry = serde_json::from_slice(l).unwrap();
                e.chain_hash().unwrap() == head
            })
            .expect("tip entry in the merged segment")
    };
    assert!(
        pos(&low.gamma_head) < pos(&alt.gamma_head),
        "sub-chain LOW lays out before sub-chain HIGH"
    );
}

#[then("the manifest's gamma segment root and count match an independent recomputation")]
fn i_then_roots_recommitted(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
    let committed = &m.gamma_roots["2026-07"];
    assert_eq!(
        hex::encode(aithos_core::gamma::segment_root(&refs)),
        committed.root,
        "recomputed root"
    );
    assert_eq!(refs.len() as u64, committed.n, "recomputed count");
}

#[then("a fifth action after the merge is refused as budget spent")]
fn i_then_budget_spent(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let err = w
        .i_action_on(false, "reply", I_AFTER)
        .expect_err("the budget tallies across both sub-chains");
    assert!(err.contains("max_actions"), "got: {err}");
}

#[then("neither branch is canonical and the conflict is surfaced")]
fn i_then_fork_surfaced(w: &mut ProtocolWorld) {
    let err = w
        .i_result
        .clone()
        .unwrap()
        .expect_err("the fork is refused");
    assert!(err.contains("same-node conflict"), "got: {err}");
}

#[then("the resolving edition verifies and extends the winning branch")]
fn i_then_resolution_verifies(w: &mut ProtocolWorld) {
    w.i_surfaced
        .clone()
        .unwrap()
        .expect("the resolution succeeds");
    let owner = w.owner(0);
    let m = w.h_manifest();
    assert_eq!(
        m.resolves_fork, m.edition.prev_hash,
        "extends the named winner"
    );
    let b = w.gbundle();
    b.verify().expect("the resolving edition verifies");
    assert_eq!(
        b.read_section(Zone::Circle, "projets/note1", &owner)
            .or_else(|_| b.read_section(Zone::Circle, "autre/note2", &owner))
            .unwrap(),
        "the winning body",
        "content is the winning branch's"
    );
}

#[then("the losing branch's write is surfaced, not replayed")]
fn i_then_loser_surfaced(w: &mut ProtocolWorld) {
    let surfaced = w.i_surfaced.clone().unwrap().unwrap();
    assert!(!surfaced.is_empty(), "the losing labels are reported");
    let h = w.h_manifest().edition.height;
    assert!(
        w.gbundle()
            .store
            .get(&format!("manifests/{}-alt.json", h - 1))
            .unwrap()
            .is_some(),
        "the losing manifest is kept, surfaced"
    );
}

#[then("the resolution is refused for lack of authority")]
fn i_then_resolution_refused(w: &mut ProtocolWorld) {
    let err = w
        .i_surfaced
        .clone()
        .unwrap()
        .expect_err("the delegate is out of perimeter");
    assert!(err.contains("resolution rejected"), "got: {err}");
}

// ------------------------------------------------------------------ main

fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../features");
    // Ritual (docs/EXECUTION-PLAN.md): each phase's .feature is co-written and
    // committed BEFORE implementation, its scenarios tagged @wip. The filter
    // keeps the suite green until a scenario is implemented and untagged.
    futures::executor::block_on(
        ProtocolWorld::cucumber().filter_run(features, |_, _, scenario| {
            !scenario.tags.iter().any(|t| t == "wip")
        }),
    );
}
