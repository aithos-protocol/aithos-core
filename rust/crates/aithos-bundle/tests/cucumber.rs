//! BDD acceptance harness (cucumber-rs). Gherkin features live at the repo
//! root in `features/`; step definitions grow with each phase of
//! docs/EXECUTION-PLAN.md and are never rewritten, only extended.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::manifest::{sha256_hex, Manifest};
use aithos_bundle::{MemStore, Store};
use aithos_core::derive::{derive_key, node_key, section_label};
use aithos_core::did::{DidDocument, EpochTransition};
use aithos_core::header::{Header, Line, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::path::{NodePath, Zone};
use aithos_core::wire;
use cucumber::{given, then, when, World};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

// --- step D fixtures ---
const NOW: &str = "2026-07-09T00:00:00Z";
const BODY: &str = "Le corps de la note, ephemere et precieux.";
const PUB_BODY: &str = "Bio publique, lisible par le monde entier.";
const SELF_BODY: &str = "Souvenir intime, jamais signe.";

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
                Zone::Circle,
                folder,
                name,
                "note",
                &[tag.to_owned()],
                BODY,
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
            Zone::Public,
            &folder,
            &name,
            "bio",
            &[],
            PUB_BODY,
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
            Zone::Self_,
            &folder,
            &name,
            "cicatrice au genou",
            &["sante".to_owned()],
            SELF_BODY,
            &owner,
            &mut w.ent,
        )
        .unwrap();
    w.publish_bundle();
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
