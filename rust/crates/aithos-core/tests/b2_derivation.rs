//! Conformance vector B2 — content-tree derivation (spec 01.3, 02.5).
//! Expected values generated independently (Python blake3).

use aithos_core::derive::{derive_key, node_key, section_label};
use aithos_core::ids::Sid;
use aithos_core::path::{NodePath, Zone};
use serde::Deserialize;

#[derive(Deserialize)]
struct B2 {
    zone_dk_hex: String,
    folder_sids: Vec<String>,
    section_sid: String,
    sibling_section_sid: String,
    tag: String,
    folder1_key_hex: String,
    deep_section_key_hex: String,
    sibling_section_key_hex: String,
    tag_anchor_folder1_hex: String,
    tag_anchor_zone_root_hex: String,
}

fn vector() -> B2 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/b2-derivation.json"
    ));
    serde_json::from_str(raw).expect("vector b2-derivation.json parses")
}

fn key32(hex_str: &str) -> [u8; 32] {
    hex::decode(hex_str).unwrap().try_into().unwrap()
}

#[test]
fn b2_deep_chain_and_anchors() {
    let v = vector();
    let zone = key32(&v.zone_dk_hex);
    let sids: Vec<Sid> = v
        .folder_sids
        .iter()
        .map(|s| Sid::parse(s).unwrap())
        .collect();
    let section = Sid::parse(&v.section_sid).unwrap();

    let folder1 = NodePath::folder(Zone::Circle, vec![sids[0]]);
    assert_eq!(hex::encode(node_key(&zone, &folder1)), v.folder1_key_hex);

    let deep = NodePath::section(Zone::Circle, sids.clone(), section);
    assert_eq!(hex::encode(node_key(&zone, &deep)), v.deep_section_key_hex);

    let sibling = NodePath::section(
        Zone::Circle,
        vec![sids[1]],
        Sid::parse(&v.sibling_section_sid).unwrap(),
    );
    assert_eq!(
        hex::encode(node_key(&zone, &sibling)),
        v.sibling_section_key_hex
    );

    let anchor_folder = NodePath::tag_view(Zone::Circle, vec![sids[0]], &v.tag).unwrap();
    assert_eq!(
        hex::encode(node_key(&zone, &anchor_folder)),
        v.tag_anchor_folder1_hex
    );
    let anchor_root = NodePath::tag_view(Zone::Circle, vec![], &v.tag).unwrap();
    assert_eq!(
        hex::encode(node_key(&zone, &anchor_root)),
        v.tag_anchor_zone_root_hex
    );
}

#[test]
fn b2_folder_key_alone_derives_descendants() {
    let v = vector();
    let zone = key32(&v.zone_dk_hex);
    let sids: Vec<Sid> = v
        .folder_sids
        .iter()
        .map(|s| Sid::parse(s).unwrap())
        .collect();
    let section = Sid::parse(&v.section_sid).unwrap();

    // From the deepest folder's key, one local derivation reaches the section
    // without ever touching the zone key again.
    let deepest_folder = NodePath::folder(Zone::Circle, sids);
    let folder_key = node_key(&zone, &deepest_folder);
    assert_eq!(
        hex::encode(derive_key(&section_label(&section), &folder_key)),
        v.deep_section_key_hex
    );
}
