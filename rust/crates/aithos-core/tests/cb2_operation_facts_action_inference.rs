//! CB2 K1.2-AI-B action/inference-facts vector consumer.
//!
//! Existing generic JCS/SHA-256 primitives are used only to reproduce the
//! independent oracle bytes. No test-local validator claims the absent typed
//! Core operation-facts API is GREEN.

use std::collections::BTreeSet;

use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-action-inference.json"
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
const GPLUS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/gplus-obligations.json"
));

const VECTOR_SHA256: &str = "4e1f319439ec4cc5faef2012a2a8aa198ba8d62fced2f330d2bbd13580c78b71";
const PROFILE_KEY: &str = "aithos-operation-facts-core";

const POSITIVE_IDS: [&str; 8] = [
    "action-plain",
    "action-budget",
    "action-purpose",
    "action-budget-purpose",
    "inference-plain",
    "inference-budget",
    "inference-purpose",
    "inference-budget-purpose",
];

const NEGATIVE_IDS: [&str; 35] = [
    "missing-envelope-profile",
    "extra-envelope-member",
    "kind-family-mismatch",
    "missing-action-member",
    "extra-action-member",
    "null-action-member",
    "empty-connector",
    "mismatched-action",
    "missing-catalog-member",
    "extra-catalog-member",
    "empty-catalog-version",
    "malformed-catalog-digest",
    "mismatched-catalog-digest",
    "mismatched-approval-digest",
    "malformed-args-hash",
    "mismatched-action-arguments",
    "action-post-effect-tokens",
    "action-usage-receipt",
    "missing-inference-member",
    "empty-provider",
    "empty-model",
    "malformed-request-digest",
    "mismatched-inference-request",
    "inference-args-hash",
    "inference-post-effect-counters",
    "missing-applicable-budget",
    "volunteered-budget",
    "empty-budget-ref",
    "extra-budget-member",
    "unknown-budget-state",
    "missing-applicable-purpose",
    "volunteered-purpose",
    "mismatched-purpose-ref",
    "null-purpose",
    "facts-ref-digest-mismatch",
];

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 K1 action/inference vector parses")
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
fn cb2_k1_action_inference_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_k1_action_inference_historical_hashes_preexisting_green() {
    let vector = vector();
    let expected = &vector["historical_vector_sha256"];
    for (name, bytes) in [
        ("e1-mandate.json", E1_BYTES.as_bytes()),
        ("f1-gamma-chain.json", F1_BYTES.as_bytes()),
        ("f2-gamma-counting.json", F2_BYTES.as_bytes()),
        ("gplus-obligations.json", GPLUS_BYTES.as_bytes()),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            expected[name].as_str().expect("historical vector hash"),
            "{name}"
        );
    }
}

#[test]
fn cb2_k1_action_inference_preimage_bytes_preexisting_green() {
    let vector = vector();
    let fixtures = &vector["fixtures"];
    let domains = &vector["commitment_domains"];

    let action = &fixtures["action"];
    let args_jcs = jcs::canonicalize(&action["args"]).expect("action args JCS");
    assert_eq!(
        args_jcs,
        action["args_jcs"]
            .as_str()
            .expect("action args JCS fixture")
    );
    assert_eq!(
        sha256_text(args_jcs.as_bytes()),
        action["args_hash"].as_str().expect("args_hash fixture")
    );

    let inference = &fixtures["inference"];
    let request_bytes = hex::decode(
        inference["request_body_hex"]
            .as_str()
            .expect("private request bytes"),
    )
    .expect("request hex");
    assert_eq!(
        commitment(
            domains["inference_request"]
                .as_str()
                .expect("inference request domain"),
            &request_bytes,
        ),
        inference["request_digest"]
            .as_str()
            .expect("request digest fixture")
    );

    assert_eq!(
        commitment(
            domains["purpose"].as_str().expect("purpose domain"),
            fixtures["purpose_text"]
                .as_str()
                .expect("purpose text")
                .as_bytes(),
        ),
        fixtures["purpose_ref"]
            .as_str()
            .expect("purpose ref fixture")
    );

    let catalog = &fixtures["catalog"];
    assert_eq!(
        jcs::canonicalize(&catalog["catalog_document"]).expect("catalog JCS"),
        catalog["catalog_document_jcs"]
            .as_str()
            .expect("catalog JCS fixture")
    );
    assert_eq!(
        sha256_text(
            catalog["catalog_document_jcs"]
                .as_str()
                .expect("catalog JCS")
                .as_bytes(),
        ),
        catalog["catalog_digest"]
            .as_str()
            .expect("catalog digest fixture")
    );
    assert_eq!(
        sha256_text(
            catalog["approval_document_jcs"]
                .as_str()
                .expect("approval JCS")
                .as_bytes(),
        ),
        catalog["approval_digest"]
            .as_str()
            .expect("approval digest fixture")
    );
}

#[test]
fn cb2_k1_action_inference_operation_bytes_preexisting_green() {
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
        let kind = document["kind"].as_str().expect("operation kind");
        let expected_members: BTreeSet<String> = match kind {
            "action" => [
                "connector",
                "action",
                "catalog_ref",
                "args_hash",
                "budget",
                "purpose",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            "inference" => ["provider", "model", "request_digest", "budget", "purpose"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            other => panic!("{case_id}: unknown kind {other}"),
        };
        assert_eq!(
            object_keys(&document["facts"]),
            expected_members,
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
fn cb2_k1_action_inference_closed_family_inventory_preliminary() {
    let vector = vector();
    assert_eq!(
        string_array(&vector["inventory"]["positive_case_ids"]),
        POSITIVE_IDS
    );
    assert_eq!(
        string_array(&vector["inventory"]["negative_case_ids"]),
        NEGATIVE_IDS
    );
    assert_eq!(
        vector["inventory"]["required_error_variant"].as_str(),
        Some("InvalidOperationFacts")
    );
    assert_eq!(
        vector["inventory"]["catalog_documents_are_syntactic_only"].as_bool(),
        Some(true)
    );

    let negatives = vector["negative_cases"].as_array().expect("negatives");
    assert_eq!(negatives.len(), NEGATIVE_IDS.len());
    for (case, expected_id) in negatives.iter().zip(NEGATIVE_IDS) {
        assert_eq!(case["id"].as_str(), Some(expected_id));
        assert_eq!(
            case["must_fail"].as_str(),
            Some("InvalidOperationFacts"),
            "{expected_id}"
        );
    }
}
