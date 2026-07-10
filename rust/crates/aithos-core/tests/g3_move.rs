//! Conformance vector G3 — move-as-rotation (spec 02.9): nodal dir
//! containment (04.2), stable derivation labels below the moved node,
//! new-path AAD bindings at the new version, and the up-link wrap via the
//! NEW parent. Independent Python generator, cross-checked against B2.

use aithos_core::derive::{derive_key, folder_label, node_key, section_label};
use aithos_core::header::Wrap;
use aithos_core::ids::Sid;
use aithos_core::mandate::{covers_op, dir_covers, Op, PerimeterEntry, Verb};
use aithos_core::path::{NodePath, Zone};
use aithos_core::seal::{blob_aad, line_aad, wrap_aad};
use serde::Deserialize;

#[derive(Deserialize)]
struct Cast {
    old_parent: String,
    moved: String,
    new_parent: String,
    section: String,
}

#[derive(Deserialize)]
struct Containment {
    dir: Vec<String>,
    chain: Vec<String>,
    covers: bool,
}

#[derive(Deserialize)]
struct G3 {
    subject_did: String,
    sids: Cast,
    containment: Vec<Containment>,
    zone_dk_hex: String,
    old_parent_key_hex: String,
    moved_old_key_hex: String,
    new_parent_key_hex: String,
    moved_new_dk_hex: String,
    section_key_v2_hex: String,
    old_node: String,
    new_node: String,
    parent_node: String,
    new_section_node: String,
    key_version: u64,
    line_aad_hex: String,
    blob_aad_hex: String,
    wrap_aad_hex: String,
    wrap_nonce_hex: String,
    wrap_cipher_hex: String,
}

fn vector() -> G3 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/g3-move.json"
    ));
    serde_json::from_str(raw).expect("vector g3-move.json parses")
}

fn sid(s: &str) -> Sid {
    Sid::parse(s).expect("vector sid parses")
}

fn b32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}

/// The vector's verdict table binds all three faces of §04.2 nodal
/// containment at once: the raw rule, entry-vs-entry (§05.3 chains), and
/// op coverage (§04.5 verification against the CURRENT resolved chain).
#[test]
fn nodal_containment_verdicts() {
    for row in vector().containment {
        let dir: Vec<Sid> = row.dir.iter().map(|s| sid(s)).collect();
        let chain: Vec<Sid> = row.chain.iter().map(|s| sid(s)).collect();
        assert_eq!(
            dir_covers(&dir, &chain),
            row.covers,
            "dir_covers({:?}, {:?})",
            row.dir,
            row.chain
        );
        let parent = PerimeterEntry::Ethos {
            verb: Verb::Read,
            zone: Zone::Circle,
            dir: dir.clone(),
            tag: None,
        };
        let child = PerimeterEntry::Ethos {
            verb: Verb::Read,
            zone: Zone::Circle,
            dir: chain.clone(),
            tag: None,
        };
        assert_eq!(parent.covers(&child), row.covers, "covers {:?}", row.dir);
        let op = Op {
            verb: Verb::Read,
            zone: Zone::Circle,
            folders: &chain,
            tags: &[],
        };
        assert_eq!(
            covers_op(std::slice::from_ref(&parent), &op),
            row.covers,
            "covers_op {:?}",
            row.dir
        );
    }
}

/// Sids are the labels (§02.5): below the moved node every derivation is
/// unchanged; only M's own key is fresh. Stepwise == pathwise derivation.
#[test]
fn derivation_below_moved_node_is_stable() {
    let v = vector();
    let zone_dk = b32(&v.zone_dk_hex);
    let a = sid(&v.sids.old_parent);
    let m = sid(&v.sids.moved);
    let p = sid(&v.sids.new_parent);
    let x = sid(&v.sids.section);

    let dk_a = derive_key(&folder_label(&a), &zone_dk);
    assert_eq!(dk_a, b32(&v.old_parent_key_hex));
    // The old parent derives M's old key forever — un-teachable, which is
    // exactly why a move MUST rotate.
    let dk_m_old = derive_key(&folder_label(&m), &dk_a);
    assert_eq!(dk_m_old, b32(&v.moved_old_key_hex));
    assert_eq!(
        node_key(&zone_dk, &NodePath::folder(Zone::Circle, vec![a, m])),
        dk_m_old,
        "stepwise must equal pathwise derivation"
    );
    assert_eq!(
        derive_key(&folder_label(&p), &zone_dk),
        b32(&v.new_parent_key_hex)
    );
    // Below M the section label is untouched by the move.
    assert_eq!(
        derive_key(&section_label(&x), &b32(&v.moved_new_dk_hex)),
        b32(&v.section_key_v2_hex)
    );
}

/// Every seal binds M's NEW canonical path at the new version, and the
/// up-link wrap seals DK' under the NEW parent's key, byte-identically.
#[test]
fn new_path_bindings_and_parent_wrap() {
    let v = vector();
    assert_eq!(
        hex::encode(line_aad(&v.subject_did, &v.new_node, v.key_version)),
        v.line_aad_hex
    );
    assert_eq!(
        hex::encode(blob_aad(&v.subject_did, &v.new_section_node, v.key_version)),
        v.blob_aad_hex
    );
    assert_eq!(
        hex::encode(wrap_aad(&v.subject_did, &v.new_node, v.key_version)),
        v.wrap_aad_hex
    );

    let nonce: [u8; 24] = hex::decode(&v.wrap_nonce_hex).unwrap().try_into().unwrap();
    let wrap = Wrap::seal(
        &v.subject_did,
        &v.parent_node,
        &b32(&v.new_parent_key_hex),
        &v.new_node,
        v.key_version,
        &b32(&v.moved_new_dk_hex),
        nonce,
    );
    assert_eq!(wrap.c, v.wrap_cipher_hex, "wrap bytes via the new parent");
    assert_eq!(
        wrap.open(&v.subject_did, &b32(&v.new_parent_key_hex))
            .unwrap(),
        b32(&v.moved_new_dk_hex)
    );

    // The move changes the spine, never the node: same terminal sid at
    // both addresses.
    let old = NodePath::parse(&v.old_node).unwrap();
    let new = NodePath::parse(&v.new_node).unwrap();
    assert_eq!(old.folders.last(), new.folders.last());
    assert_ne!(old.folders, new.folders);
}
