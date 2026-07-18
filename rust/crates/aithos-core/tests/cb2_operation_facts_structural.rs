//! CB2 K1.2-GRRP-B structural operation-facts vector consumer.
//!
//! Existing generic JCS/SHA-256 primitives reproduce independent oracle bytes.
//! The future typed validator and changeset document API are deliberately not
//! implemented in this test.

use std::collections::BTreeSet;

use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-structural.json"
));
const E1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/e1-mandate.json"
));
const F1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f1-gamma-chain.json"
));
const F2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f2-gamma-counting.json"
));
const G1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/g1-revocation.json"
));

const VECTOR_SHA256: &str = "1be1d239055e5a66f0c64d3ca2c2b00cd102bac44a7cd8c2147d16dce8d275e3";
const PROFILE_KEY: &str = "aithos-operation-facts-core";

const POSITIVE_IDS: [&str; 11] = [
    "grant",
    "revoke-no-reason",
    "revoke-with-reason",
    "rotate-ethos-zone",
    "rotate-ethos-node",
    "rotate-vault",
    "rotate-identity",
    "publication-genesis",
    "publication-normal",
    "publication-merge",
    "publication-resolution",
];

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 K1 structural vector parses")
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
fn cb2_k1_structural_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_k1_structural_historical_hashes_preexisting_green() {
    let vector = vector();
    let expected = &vector["historical_vector_sha256"];
    for (name, bytes) in [
        ("e1-mandate.json", E1_BYTES.as_bytes()),
        ("f1-gamma-chain.json", F1_BYTES.as_bytes()),
        ("f2-gamma-counting.json", F2_BYTES.as_bytes()),
        ("g1-revocation.json", G1_BYTES.as_bytes()),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            expected[name].as_str().expect("historical vector hash"),
            "{name}"
        );
    }
}

#[test]
fn cb2_k1_structural_reference_preimages_preexisting_green() {
    let vector = vector();
    let fixtures = &vector["fixtures"];
    let domains = &vector["commitment_domains"];

    let certificate = &fixtures["certificate"];
    assert_eq!(
        jcs::canonicalize(&certificate["document"]).expect("certificate JCS"),
        certificate["document_jcs"]
            .as_str()
            .expect("certificate JCS fixture")
    );
    assert_eq!(
        sha256_text(
            certificate["document_jcs"]
                .as_str()
                .expect("certificate JCS")
                .as_bytes()
        ),
        certificate["certificate_digest"]
            .as_str()
            .expect("certificate digest")
    );

    for name in [
        "state_before",
        "state_after",
        "identity_before",
        "identity_after",
    ] {
        let state = &fixtures[name];
        let state_jcs = jcs::canonicalize(&state["document"]).expect("state JCS");
        assert_eq!(
            state_jcs,
            state["document_jcs"].as_str().expect("state JCS fixture")
        );
        assert_eq!(
            commitment(
                domains["state_fact"].as_str().expect("state domain"),
                state_jcs.as_bytes(),
            ),
            state["digest"].as_str().expect("state digest")
        );
    }

    let transition = &fixtures["identity_transition"];
    assert_eq!(
        sha256_text(
            transition["document_jcs"]
                .as_str()
                .expect("transition JCS")
                .as_bytes(),
        ),
        transition["digest"].as_str().expect("transition digest")
    );

    let changeset = &fixtures["changeset"];
    assert_eq!(
        commitment(
            domains["changeset"].as_str().expect("changeset domain"),
            changeset["document_jcs"]
                .as_str()
                .expect("changeset JCS")
                .as_bytes(),
        ),
        changeset["digest"].as_str().expect("changeset digest")
    );
}

#[test]
fn cb2_k1_structural_operation_bytes_preexisting_green() {
    let vector = vector();
    let domain = vector["commitment_domains"]["operation_facts"]
        .as_str()
        .expect("operation-facts domain");
    let positives = vector["positive_cases"].as_array().expect("positives");
    assert_eq!(
        positives
            .iter()
            .map(|case| case["id"].as_str().expect("case id"))
            .collect::<Vec<_>>(),
        POSITIVE_IDS
    );
    for case in positives {
        let case_id = case["id"].as_str().expect("case id");
        let document = &case["document"];
        assert_eq!(
            object_keys(document),
            [PROFILE_KEY, "facts", "kind"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            "{case_id}"
        );
        let document_jcs = jcs::canonicalize(document).expect("operation JCS");
        assert_eq!(
            document_jcs,
            case["document_jcs"].as_str().expect("oracle JCS"),
            "{case_id}"
        );
        let digest = commitment(domain, document_jcs.as_bytes());
        assert_eq!(case["digest"].as_str(), Some(digest.as_str()), "{case_id}");
        assert_eq!(
            case["facts_ref"]["digest"].as_str(),
            Some(digest.as_str()),
            "{case_id}"
        );
    }
}

#[test]
fn cb2_k1_structural_closed_family_inventory_preliminary() {
    let vector = vector();
    assert_eq!(
        string_array(&vector["inventory"]["positive_case_ids"]),
        POSITIVE_IDS
    );
    assert_eq!(
        vector["inventory"]["required_error_variant"].as_str(),
        Some("InvalidOperationFacts")
    );
    assert_eq!(
        vector["inventory"]["changeset_document_is_syntactic_only"].as_bool(),
        Some(true)
    );
    assert_eq!(
        vector["inventory"]["derived_rotation_is_not_an_occurrence"].as_bool(),
        Some(true)
    );
    let negatives = vector["negative_cases"].as_array().expect("negatives");
    assert!(negatives.len() >= 40);
    let ids: Vec<_> = negatives
        .iter()
        .map(|case| case["id"].as_str().expect("negative id"))
        .collect();
    assert_eq!(string_array(&vector["inventory"]["negative_case_ids"]), ids);
    for case in negatives {
        assert_eq!(case["must_fail"].as_str(), Some("InvalidOperationFacts"));
    }
}
