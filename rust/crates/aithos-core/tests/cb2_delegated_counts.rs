//! CB2 D7 delegated-count vector consumer.
//!
//! Existing JCS and Merkle primitives reproduce the independent Python bytes.
//! The future typed delegated-count validator and its public error variant are
//! deliberately only inventoried here because CB2 cannot add production code.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::{
    jcs,
    merkle::{h_leaf, mroot, verify_proof, Proof, EMPTY_ROOT},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-delegated-counts.json"
));
const H2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/h2-gamma-roots.json"
));
const GEN_H2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/gen-h2.py"
));

const VECTOR_SHA256: &str = "c1edd459b00ff72f2693e54370a60d2c8b981c18ee10d213a4b26897ed2618f1";
const H2_SHA256: &str = "c497b9b2bced8fbf449ab249ea193cda956d2438a003f7bd06f6173c3486ef50";
const GEN_H2_SHA256: &str = "09a6e7b8170c5d42d3dff1ede7967c73128535dc058ba86c0c87ebd07f752180";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 D7 vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn string_array(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("fixture string array")
        .iter()
        .map(|item| item.as_str().expect("fixture string"))
        .collect()
}

#[test]
fn cb2_d7_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_d7_historical_h2_is_byte_identical_preexisting_green() {
    let vector = vector();
    assert_eq!(sha256_hex(H2_BYTES.as_bytes()), H2_SHA256);
    assert_eq!(sha256_hex(GEN_H2_BYTES.as_bytes()), GEN_H2_SHA256);
    assert_eq!(
        vector["historical_vector_sha256"]["h2-gamma-roots.json"].as_str(),
        Some(H2_SHA256)
    );
    assert_eq!(
        vector["historical_vector_sha256"]["gen-h2.py"].as_str(),
        Some(GEN_H2_SHA256)
    );

    let h2: Value = serde_json::from_str(H2_BYTES).expect("historical H2 parses");
    assert_eq!(
        h2["tree"]["gamma_counts_root_hex"].as_str(),
        Some("2f233fb3bf9da2426b2ceee9a7f606d9ba235db5875575257a53ebf76b84c0a8")
    );
}

#[test]
fn cb2_d7_leaf_root_and_proof_bytes_preexisting_green() {
    let vector = vector();
    let positive = &vector["positive"];
    let counts: BTreeMap<String, Value> =
        serde_json::from_value(positive["expected_counts"].clone()).expect("counts map");
    let oracle_leaves = positive["leaves"].as_array().expect("oracle leaves");
    let mut rust_leaves = Vec::new();

    for ((mandate_id, counters), oracle) in counts.iter().zip(oracle_leaves) {
        assert_eq!(oracle["mandate_id"].as_str(), Some(mandate_id.as_str()));
        let mut payload = mandate_id.as_bytes().to_vec();
        payload.push(0);
        payload.extend_from_slice(jcs::canonicalize(counters).expect("counter JCS").as_bytes());
        assert_eq!(hex::encode(&payload), oracle["payload_hex"]);
        let leaf = h_leaf(&payload);
        assert_eq!(hex::encode(leaf), oracle["leaf_hex"]);
        rust_leaves.push(leaf);
    }

    let root = mroot(&rust_leaves);
    assert_eq!(
        hex::encode(root),
        positive["delegated_counts"]["root"]
            .as_str()
            .expect("delegated count root")
    );
    assert_eq!(
        positive["delegated_counts"]
            .as_object()
            .expect("closed reference")
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        ["aithos-delegated-counts-core", "root"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(
        positive["empty_root"].as_str(),
        Some(hex::encode(EMPTY_ROOT).as_str())
    );

    let proof: Proof =
        serde_json::from_value(positive["proof_leaf_mandate"].clone()).expect("v1 proof wire");
    verify_proof(&proof, &root).expect("independent D7 proof verifies");
}

#[test]
fn cb2_d7_occurrence_dedup_and_subtree_inventory_preexisting_green() {
    let vector = vector();
    let positive = &vector["positive"];
    let views = positive["evidence_views"]
        .as_array()
        .expect("evidence views");
    let occurrences: BTreeSet<_> = views
        .iter()
        .filter(|view| view["actor"] == "grantee")
        .filter(|view| view["opposable"] != false)
        .map(|view| view["occurrence"].as_str().expect("occurrence"))
        .collect();
    assert_eq!(occurrences.len(), 14);
    assert!(
        views.len() > occurrences.len(),
        "cross-view duplicates exist"
    );
    assert_eq!(
        positive["expected_counted_occurrences"]
            .as_array()
            .expect("counted occurrences")
            .len(),
        14
    );
    assert_eq!(
        positive["two_ethos_mutations_plus_publication_delta"],
        serde_json::json!({"mutations": 2, "consumptions": 3})
    );
    assert_eq!(
        positive["historical_children_delta_for_direct_grant"].as_u64(),
        Some(1)
    );
    assert_eq!(
        positive["non_occurrences"][0]["delta"],
        serde_json::json!({"mutations": 0, "consumptions": 0})
    );
}

#[test]
fn cb2_d7_closed_negative_and_api_inventory_preliminary() {
    let vector = vector();
    let counter_cases = vector["negative_counter_cases"]
        .as_array()
        .expect("counter negatives");
    let mandate_cases = vector["negative_mandate_cases"]
        .as_array()
        .expect("mandate negatives");
    assert_eq!(counter_cases.len(), 36);
    assert_eq!(mandate_cases.len(), 13);
    assert_eq!(
        string_array(&vector["inventory"]["counter_negative_ids"]),
        counter_cases
            .iter()
            .map(|case| case["id"].as_str().expect("counter case id"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        string_array(&vector["inventory"]["mandate_negative_ids"]),
        mandate_cases
            .iter()
            .map(|case| case["id"].as_str().expect("mandate case id"))
            .collect::<Vec<_>>()
    );
    assert!(counter_cases
        .iter()
        .all(|case| case["must_fail"] == "InvalidDelegatedCounts"));
    assert!(mandate_cases
        .iter()
        .all(|case| case["must_fail"] == "InvalidMandate"));
    assert_eq!(
        vector["inventory"]["counter_error_variant"].as_str(),
        Some("InvalidDelegatedCounts")
    );
    assert_eq!(
        vector["inventory"]["mandate_error_variant"].as_str(),
        Some("InvalidMandate")
    );
    assert_eq!(
        vector["inventory"]["historical_gamma_counts_root_is_not_reinterpreted"].as_bool(),
        Some(true)
    );
}
