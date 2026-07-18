//! CB2 W1/A1/K1 operation-projection vector consumer.
//!
//! Existing generic JCS/SHA-256 primitives reproduce the independent oracle.
//! The future typed operation validator and `InvalidOperation` public variant
//! remain a deliberate COMPILE-RED-PRELIMINAIRE gate.

use std::collections::BTreeSet;

use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));
const E1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/e1-mandate.json"
));
const F1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f1-gamma-chain.json"
));
const MUTATION_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
));
const STRUCTURAL_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-structural.json"
));

const VECTOR_SHA256: &str = "99bc175e0b4f07dece0afa828fff22be9be75d4b66f8b00fee3e7d427021e14c";
const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 operation projection vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_text(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn commitment(domain: &str, payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(payload);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn object_keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("fixture object")
        .keys()
        .cloned()
        .collect()
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
fn cb2_w1_projection_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_w1_projection_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("e1-mandate.json", E1_BYTES.as_bytes()),
        ("f1-gamma-chain.json", F1_BYTES.as_bytes()),
        (
            "cb2-operation-facts-mutation.json",
            MUTATION_BYTES.as_bytes(),
        ),
        (
            "cb2-operation-facts-structural.json",
            STRUCTURAL_BYTES.as_bytes(),
        ),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            vector["historical_vector_sha256"][name]
                .as_str()
                .expect("historical hash"),
            "{name}"
        );
    }
}

#[test]
fn cb2_w1_projection_certificate_address_preexisting_green() {
    let vector = vector();
    let certificate = &vector["fixtures"]["certificate"];
    let certificate_jcs = jcs::canonicalize(certificate).expect("certificate JCS");
    assert_eq!(
        certificate_jcs,
        vector["fixtures"]["certificate_jcs"]
            .as_str()
            .expect("oracle certificate JCS")
    );
    assert_eq!(
        sha256_text(certificate_jcs.as_bytes()),
        vector["fixtures"]["certificate_digest"]
            .as_str()
            .expect("certificate digest")
    );
}

#[test]
fn cb2_w1_projection_commitment_and_reference_bytes_preexisting_green() {
    let vector = vector();
    let positives = vector["positive_cases"].as_array().expect("positives");
    assert_eq!(positives.len(), 4);
    let top_keys: BTreeSet<String> = [
        "aithos-operation-core",
        "occurrence",
        "subject",
        "at",
        "history_heads",
        "authority",
        "operation",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let reference_keys: BTreeSet<String> = ["aithos-operation-core", "occurrence", "commitment"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    for case in positives {
        let projection = &case["projection"];
        assert_eq!(object_keys(projection), top_keys, "{}", case["id"]);
        let projection_jcs = jcs::canonicalize(projection).expect("projection JCS");
        assert_eq!(
            projection_jcs,
            case["projection_jcs"].as_str().expect("oracle JCS"),
            "{}",
            case["id"]
        );
        let derived = commitment(OPERATION_DOMAIN, projection_jcs.as_bytes());
        assert_eq!(case["commitment"].as_str(), Some(derived.as_str()));
        let reference = &case["operation_ref"];
        assert_eq!(object_keys(reference), reference_keys);
        assert_eq!(reference["occurrence"], projection["occurrence"]);
        assert_eq!(reference["commitment"].as_str(), Some(derived.as_str()));
    }

    assert_ne!(
        positives[0]["commitment"], positives[3]["commitment"],
        "distinct occurrence anchors keep otherwise identical effects distinct"
    );
}

#[test]
fn cb2_w1_projection_negative_and_api_inventory_preliminary() {
    let vector = vector();
    let projection_cases = vector["negative_projection_cases"]
        .as_array()
        .expect("projection negatives");
    let reference_cases = vector["negative_reference_cases"]
        .as_array()
        .expect("reference negatives");
    assert_eq!(projection_cases.len(), 32);
    assert_eq!(reference_cases.len(), 6);
    assert_eq!(
        string_array(&vector["inventory"]["projection_negative_ids"]),
        projection_cases
            .iter()
            .map(|case| case["id"].as_str().expect("projection case id"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        string_array(&vector["inventory"]["reference_negative_ids"]),
        reference_cases
            .iter()
            .map(|case| case["id"].as_str().expect("reference case id"))
            .collect::<Vec<_>>()
    );
    assert!(projection_cases.iter().all(|case| matches!(
        case["must_fail"].as_str(),
        Some("InvalidOperation" | "InvalidOperationFacts")
    )));
    assert!(reference_cases
        .iter()
        .all(|case| case["must_fail"] == "InvalidOperation"));
    assert_eq!(
        vector["inventory"]["operation_error_variant"].as_str(),
        Some("InvalidOperation")
    );
    assert_eq!(
        vector["inventory"]["facts_error_variant"].as_str(),
        Some("InvalidOperationFacts")
    );
    assert_eq!(
        vector["inventory"]["sc1_complete_bytes_are_out_of_scope"].as_bool(),
        Some(true)
    );
}
