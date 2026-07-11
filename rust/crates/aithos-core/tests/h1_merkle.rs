//! Conformance vector H1 — Merkle state roots (spec 02.10): domain-separated
//! hashing, left-heavy mroot, node composition, v1 proof replay, tamper and
//! splice negatives. Every expected value computed independently in Python.

use aithos_core::error::Error;
use aithos_core::merkle::{h_leaf, h_node, mroot, verify_proof, Proof, EMPTY_ROOT};
use serde_json::Value;

fn vector() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/h1-merkle.json"
    )))
    .expect("valid vector json")
}

fn jcs_bytes(v: &Value) -> Vec<u8> {
    aithos_core::jcs::canonical_bytes(v).unwrap()
}

fn hx(v: &Value) -> [u8; 32] {
    <[u8; 32]>::try_from(hex::decode(v.as_str().unwrap()).unwrap()).unwrap()
}

/// Rebuild every node of the fixture tree and cross every hash.
#[test]
fn tree_hashes_match_python() {
    let v = vector();
    let (rows, tree) = (&v["rows"], &v["tree"]);
    let zeros = [0u8; 32];

    let header_f = *blake3::hash(&jcs_bytes(&rows["header_f"])).as_bytes();
    assert_eq!(header_f, hx(&tree["header_f_hash_hex"]), "header hash");

    let leaf = |row: &Value, hh: &[u8; 32]| {
        let mut p = jcs_bytes(row);
        p.extend_from_slice(hh);
        h_leaf(&p)
    };
    let s7 = leaf(&rows["s7"], &zeros);
    let s8 = leaf(&rows["s8"], &zeros);
    let s9 = leaf(&rows["s9"], &zeros);
    assert_eq!(s7, hx(&tree["leaves"]["s7"]));
    assert_eq!(s8, hx(&tree["leaves"]["s8"]));
    assert_eq!(s9, hx(&tree["leaves"]["s9"]));

    let children = mroot(&[s7, s8]);
    assert_eq!(children, hx(&tree["folder_children_mroot_hex"]));
    let mut fp = jcs_bytes(&rows["folder"]);
    fp.extend_from_slice(&header_f);
    fp.extend_from_slice(&children);
    let folder1 = h_leaf(&fp);
    assert_eq!(folder1, hx(&tree["leaves"]["folder1"]));

    // tag view: H_leaf("t/urgent" ‖ zeros ‖ mroot([H_leaf(sid7 0x00 b3(wrap))]))
    let sid7 = rows["s7"]["sid"].as_str().unwrap();
    let mut wp = sid7.as_bytes().to_vec();
    wp.push(0);
    wp.extend_from_slice(blake3::hash(&jcs_bytes(&rows["wrap7"])).as_bytes());
    let wrap_leaf = h_leaf(&wp);
    assert_eq!(wrap_leaf, hx(&tree["leaves"]["wrap7"]));
    let mut tp = b"t/urgent".to_vec();
    tp.extend_from_slice(&zeros);
    tp.extend_from_slice(&mroot(&[wrap_leaf]));
    let tag_view = h_leaf(&tp);
    assert_eq!(tag_view, hx(&tree["leaves"]["tag_urgent"]));

    // root: children sorted d < s < t, odd list exercises the ceil split
    let root_children = mroot(&[folder1, s9, tag_view]);
    assert_eq!(root_children, hx(&tree["root_children_mroot_hex"]));
    assert_eq!(
        root_children,
        h_node(&h_node(&folder1, &s9), &tag_view),
        "left-heavy odd split"
    );
    let mut zp = b"z/circle".to_vec();
    zp.extend_from_slice(&zeros);
    zp.extend_from_slice(&root_children);
    assert_eq!(h_leaf(&zp), hx(&tree["circle_root_hex"]), "zone root");

    // flat self: leaves sorted by sid, root = mroot directly
    let mut selfs: Vec<(String, [u8; 32])> = rows["self_rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["sid"].as_str().unwrap().to_owned(), leaf(r, &zeros)))
        .collect();
    selfs.sort();
    let hashes: Vec<[u8; 32]> = selfs.into_iter().map(|(_, h)| h).collect();
    assert_eq!(mroot(&hashes), hx(&tree["self_root_hex"]), "self root");

    assert_eq!(EMPTY_ROOT, hx(&tree["empty_root_hex"]));
}

#[test]
fn the_python_proof_replays_to_the_root() {
    let v = vector();
    let tree = &v["tree"];
    let zeros = [0u8; 32];
    let mut payload = jcs_bytes(&v["rows"]["s7"]);
    payload.extend_from_slice(&zeros);
    let proof = Proof {
        payload: hex::encode(&payload),
        steps: serde_json::from_value(tree["proof_s7"].clone()).unwrap(),
        root: tree["circle_root_hex"].as_str().unwrap().to_owned(),
    };
    let root = hx(&tree["circle_root_hex"]);
    verify_proof(&proof, &root).expect("the Python proof must verify");
}

#[test]
fn tamper_and_splice_die() {
    let v = vector();
    let tree = &v["tree"];
    let zeros = [0u8; 32];
    let root = hx(&tree["circle_root_hex"]);
    let steps: Vec<aithos_core::merkle::ProofStep> =
        serde_json::from_value(tree["proof_s7"].clone()).unwrap();

    // tampered row
    let mut row = v["rows"]["s7"].clone();
    row["title"] = serde_json::json!("Note 1 (tampered)");
    let mut payload = jcs_bytes(&row);
    payload.extend_from_slice(&zeros);
    let bad = Proof {
        payload: hex::encode(&payload),
        steps: steps.clone(),
        root: tree["circle_root_hex"].as_str().unwrap().to_owned(),
    };
    assert!(matches!(
        verify_proof(&bad, &root),
        Err(Error::MerkleProofInvalid(_))
    ));

    // splice: a leaf over (l ‖ r) is never the interior node over (l, r)
    let l = hx(&tree["leaves"]["s7"]);
    let r = hx(&tree["leaves"]["s8"]);
    let mut spliced = Vec::new();
    spliced.extend_from_slice(&l);
    spliced.extend_from_slice(&r);
    assert_ne!(h_leaf(&spliced), h_node(&l, &r));

    // an H_leaf wrap where an H_node step belongs dies on the domain
    let mut wrong = steps;
    wrong[0] = aithos_core::merkle::ProofStep::Wrap {
        pre: String::new(),
        post: tree["leaves"]["s8"].as_str().unwrap().to_owned(),
    };
    let mut payload = jcs_bytes(&v["rows"]["s7"]);
    payload.extend_from_slice(&zeros);
    let bad = Proof {
        payload: hex::encode(&payload),
        steps: wrong,
        root: tree["circle_root_hex"].as_str().unwrap().to_owned(),
    };
    assert!(matches!(
        verify_proof(&bad, &root),
        Err(Error::MerkleProofInvalid(_))
    ));
}
