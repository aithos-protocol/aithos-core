//! BDD acceptance harness (cucumber-rs). Gherkin features live at the repo
//! root in `features/`; step definitions grow with each phase of
//! docs/EXECUTION-PLAN.md and are never rewritten, only extended.

use aithos_bundle::bundle::{Bundle, SectionSpec};
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
use aithos_core::mandate::{verify_chain, Mandate};
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
    fn delegate_act(&mut self, pattern: &str, log_grant: bool) -> Result<String, String> {
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
                constraints: MandateSpec::no_constraints(),
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
    w.delegate_act("act.x.gmail.*", true).unwrap();
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
    w.delegate_act("act.x.gmail.reply", false).unwrap();
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
    w.delegate_act("act.x.gmail.reply", true).unwrap();
    w.delegate_act("act.x.gmail.label", true).unwrap();
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
    let r = w.delegate_act("act.x.gmail.send", true);
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
