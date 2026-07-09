//! BDD acceptance harness (cucumber-rs). Gherkin features live at the repo
//! root in `features/`; step definitions grow with each phase of
//! docs/EXECUTION-PLAN.md and are never rewritten, only extended.

use aithos_core::derive::{derive_key, node_key, section_label};
use aithos_core::did::{DidDocument, EpochTransition};
use aithos_core::header::{Header, Line, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::path::{NodePath, Zone};
use aithos_core::wire;
use cucumber::{given, then, when, World};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

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
