//! Conformance vector H2 — committed gamma roots (spec 07.10): per-segment
//! roots in chain order, the counts trie, count / entry / absence proofs,
//! and the withhold negatives. Every expected value computed independently
//! in Python (anchored on the committed B2, H1 and F2 vectors).

use std::collections::BTreeMap;

use aithos_core::error::Error;
use aithos_core::gamma::{
    counts_leaf_payload, counts_root, counts_tally, prove_absence, prove_count, prove_entry,
    segment_root, verify_absence, verify_complete_actions, verify_count_proof, AbsenceProof,
    Entry, GammaCounters,
};
use aithos_core::merkle::{verify_proof, Proof};
use serde_json::Value;

fn vector() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/h2-gamma-roots.json"
    )))
    .expect("valid vector json")
}

fn hx32(v: &Value) -> [u8; 32] {
    <[u8; 32]>::try_from(hex::decode(v.as_str().unwrap()).unwrap()).unwrap()
}

fn segment_lines(v: &Value, seg: &str) -> Vec<String> {
    v["tree"]["segments"][seg]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_owned())
        .collect()
}

fn parsed_entries(v: &Value) -> Vec<Entry> {
    let mut out = Vec::new();
    for seg in ["2026-07", "2026-08"] {
        for line in segment_lines(v, seg) {
            out.push(serde_json::from_str(&line).unwrap());
        }
    }
    out
}

fn proof_from(v: &Value) -> Proof {
    serde_json::from_value(v.clone()).unwrap()
}

/// Segment roots over the exact committed bytes, chain order, root + n.
#[test]
fn segment_roots_match_python() {
    let v = vector();
    for seg in ["2026-07", "2026-08"] {
        let lines = segment_lines(&v, seg);
        let refs: Vec<&[u8]> = lines.iter().map(|l| l.as_bytes()).collect();
        let expected = &v["tree"]["gamma_roots"][seg];
        assert_eq!(
            hex::encode(segment_root(&refs)),
            expected["root"].as_str().unwrap(),
            "{seg} root"
        );
        assert_eq!(refs.len() as u64, expected["n"].as_u64().unwrap(), "{seg} n");
    }
}

/// The counts trie: tallies, leaf payloads, root — all Python-anchored.
#[test]
fn counts_trie_matches_python() {
    let v = vector();
    let tallies = counts_tally(&parsed_entries(&v));

    let expected: BTreeMap<String, GammaCounters> =
        serde_json::from_value(v["tree"]["counts"].clone()).unwrap();
    assert_eq!(tallies, expected, "trie counters == raw tallies (crossed)");

    let leaves: Vec<String> = v["tree"]["counts_leaves_hex"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h.as_str().unwrap().to_owned())
        .collect();
    for ((id, c), expected_leaf) in tallies.iter().zip(&leaves) {
        let leaf = aithos_core::merkle::h_leaf(&counts_leaf_payload(id, c).unwrap());
        assert_eq!(&hex::encode(leaf), expected_leaf, "{id} leaf");
    }
    assert_eq!(
        hex::encode(counts_root(&tallies).unwrap()),
        v["tree"]["gamma_counts_root_hex"].as_str().unwrap(),
        "counts root"
    );
    assert_eq!(
        hex::encode(counts_root(&BTreeMap::new()).unwrap()),
        v["tree"]["empty_counts_root_hex"].as_str().unwrap(),
        "empty trie root"
    );
}

/// The Python-built proofs verify; the Rust-built ones are byte-identical.
#[test]
fn proofs_replay_and_match() {
    let v = vector();

    // entry inclusion: e3 in 2026-07
    let e3 = proof_from(&v["tree"]["proof_entry_e3"]);
    let jul_root = hx32(&v["tree"]["gamma_roots"]["2026-07"]["root"]);
    verify_proof(&e3, &jul_root).expect("python entry proof verifies");
    let lines = segment_lines(&v, "2026-07");
    let refs: Vec<&[u8]> = lines.iter().map(|l| l.as_bytes()).collect();
    let ours = prove_entry(&refs, 2).unwrap();
    assert_eq!(serde_json::to_value(&ours).unwrap(), v["tree"]["proof_entry_e3"]);

    // count proof: the leaf mandate
    let tallies = counts_tally(&parsed_entries(&v));
    let root = hx32(&v["tree"]["gamma_counts_root_hex"]);
    let cp = proof_from(&v["tree"]["proof_count_leaf_mandate"]);
    let (id, counters) = verify_count_proof(&cp, &root).expect("python count proof verifies");
    assert_eq!(counters, tallies[&id]);
    let ours = prove_count(&tallies, &id).unwrap();
    assert_eq!(
        serde_json::to_value(&ours).unwrap(),
        v["tree"]["proof_count_leaf_mandate"]
    );
}

/// Absence by sorted adjacency: the ghost mandate was never counted.
#[test]
fn absence_of_the_ghost() {
    let v = vector();
    let a = &v["tree"]["proof_absence_ghost"];
    let ghost = a["absent_id"].as_str().unwrap();
    let root = hx32(&v["tree"]["gamma_counts_root_hex"]);

    let python_proof = AbsenceProof {
        left: Some(Proof {
            payload: a["left"]["payload"].as_str().unwrap().to_owned(),
            steps: serde_json::from_value(a["left"]["steps"].clone()).unwrap(),
            root: v["tree"]["gamma_counts_root_hex"].as_str().unwrap().to_owned(),
        }),
        right: Some(Proof {
            payload: a["right"]["payload"].as_str().unwrap().to_owned(),
            steps: serde_json::from_value(a["right"]["steps"].clone()).unwrap(),
            root: v["tree"]["gamma_counts_root_hex"].as_str().unwrap().to_owned(),
        }),
    };
    verify_absence(ghost, &python_proof, &root).expect("python absence proof verifies");

    let tallies = counts_tally(&parsed_entries(&v));
    let ours = prove_absence(&tallies, ghost).unwrap();
    verify_absence(ghost, &ours, &root).expect("rust absence proof verifies");

    // a counted mandate cannot be proven absent
    let counted = tallies.keys().next().unwrap().clone();
    assert!(matches!(
        prove_absence(&tallies, &counted),
        Err(Error::GammaAbsenceInvalid(_))
    ));
}

/// Completeness: k proven actions against the count leaf — and one short.
#[test]
fn completeness_closes_the_withhold() {
    let v = vector();
    let tallies = counts_tally(&parsed_entries(&v));
    let root = hx32(&v["tree"]["gamma_counts_root_hex"]);
    let leaf_id = tallies.keys().nth(1).unwrap().clone(); // …21, 3 actions

    let mut segment_roots = BTreeMap::new();
    let mut proofs = Vec::new();
    for seg in ["2026-07", "2026-08"] {
        segment_roots.insert(
            seg.to_owned(),
            hx32(&v["tree"]["gamma_roots"][seg]["root"]),
        );
        let lines = segment_lines(&v, seg);
        let refs: Vec<&[u8]> = lines.iter().map(|l| l.as_bytes()).collect();
        for (i, line) in lines.iter().enumerate() {
            let e: Entry = serde_json::from_str(line).unwrap();
            let is_action_under = e.kind == "action"
                && e.authorized_via
                    .as_ref()
                    .is_some_and(|via| via.iter().any(|m| *m == leaf_id));
            if is_action_under {
                proofs.push((seg.to_owned(), prove_entry(&refs, i).unwrap()));
            }
        }
    }
    assert_eq!(proofs.len(), 3, "fixture holds three actions under the leaf");

    let count_proof = prove_count(&tallies, &leaf_id).unwrap();
    let entries =
        verify_complete_actions(&leaf_id, &count_proof, &proofs, &segment_roots, &root)
            .expect("the full answer verifies");
    assert_eq!(entries.len(), 3);

    // the mirror withholds one — detected against the proven count
    let short = &proofs[..2];
    assert!(matches!(
        verify_complete_actions(&leaf_id, &count_proof, short, &segment_roots, &root),
        Err(Error::GammaWithholdDetected(_))
    ));
}

/// The negatives: tampered bytes, forged absence, withheld segment prefix.
#[test]
fn negatives_die() {
    let v = vector();
    let jul_root = hx32(&v["tree"]["gamma_roots"]["2026-07"]["root"]);

    // tampered entry bytes die on the proof
    let mut e3 = proof_from(&v["tree"]["proof_entry_e3"]);
    let mut entry: Value =
        serde_json::from_slice(&hex::decode(&e3.payload).unwrap()).unwrap();
    entry["payload"]["action"] = Value::String(
        v["tree"]["negatives"]["tampered_action_name"].as_str().unwrap().to_owned(),
    );
    e3.payload = hex::encode(aithos_core::jcs::canonical_bytes(&entry).unwrap());
    assert!(matches!(
        verify_proof(&e3, &jul_root),
        Err(Error::MerkleProofInvalid(_))
    ));

    // forged absence: the outer leaves are NOT adjacent around the middle id
    let tallies = counts_tally(&parsed_entries(&v));
    let root = hx32(&v["tree"]["gamma_counts_root_hex"]);
    let ids: Vec<String> = tallies.keys().cloned().collect();
    let forged = AbsenceProof {
        left: Some(prove_count(&tallies, &ids[0]).unwrap()),
        right: Some(prove_count(&tallies, &ids[2]).unwrap()),
    };
    assert!(matches!(
        verify_absence(&ids[1], &forged, &root),
        Err(Error::GammaAbsenceInvalid(_))
    ));

    // a served segment missing one entry recomputes to a dead root
    let lines = segment_lines(&v, "2026-07");
    let withheld: Vec<&[u8]> = lines.iter().take(3).map(|l| l.as_bytes()).collect();
    assert_ne!(segment_root(&withheld), jul_root);
    assert_ne!(withheld.len() as u64, v["tree"]["gamma_roots"]["2026-07"]["n"].as_u64().unwrap());
}
