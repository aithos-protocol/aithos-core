//! CB2 K1.2-R-B read-facts vector consumer.
//!
//! These tests exercise only existing generic JCS/SHA-256 primitives and lock
//! the independent oracle inventory.  They deliberately do not implement a
//! test-local read validator or claim that the future typed Core API is GREEN.

use std::collections::BTreeSet;

use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-read.json"
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

const VECTOR_SHA256: &str = "b8b4014b390fc3ebc65897f8b59d084da78c97137e9ad7f7703dbe5b0e0194db";
const OPERATION_PROFILE_KEY: &str = "aithos-operation-facts-core";

const POSITIVE_IDS: [&str; 6] = [
    "ethos-public",
    "ethos-circle",
    "ethos-self",
    "gamma-unfiltered",
    "gamma-filtered",
    "vault-config",
];

const NEGATIVE_IDS: [&str; 21] = [
    "missing-envelope-profile",
    "extra-envelope-member",
    "kind-family-mismatch",
    "unknown-read-domain",
    "missing-read-member",
    "extra-read-member",
    "null-source-edition",
    "unknown-ethos-zone",
    "noncanonical-ethos-sid",
    "mismatched-ethos-target",
    "malformed-source-edition",
    "mismatched-source-edition",
    "empty-source-head",
    "mismatched-source-head",
    "noncanonical-gamma-query",
    "duplicate-gamma-selector",
    "mismatched-request-digest",
    "mismatched-vault-connector",
    "mismatched-vault-record-key",
    "clear-display-path",
    "clear-vault-record-name",
];

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 K1 read vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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
fn cb2_k1_read_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_k1_read_historical_vector_hashes_preexisting_green() {
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
fn cb2_k1_read_source_commitment_bytes_preexisting_green() {
    let vector = vector();
    let fixtures = &vector["fixtures"];
    let domains = &vector["commitment_domains"];

    let source_manifest = &fixtures["source_manifest"];
    let mut unsigned = source_manifest["document"].clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    let preimage = jcs::canonicalize(&unsigned).expect("manifest chain-hash JCS");
    assert_eq!(
        preimage,
        source_manifest["chain_hash_preimage_jcs"]
            .as_str()
            .expect("manifest preimage fixture")
    );
    assert_eq!(
        format!("sha256:{}", sha256_hex(preimage.as_bytes())),
        source_manifest["source_edition"]
            .as_str()
            .expect("source edition fixture")
    );

    let request_domain = domains["gamma_read_request"]
        .as_str()
        .expect("Gamma request domain");
    for (name, query) in fixtures["queries"].as_object().expect("query fixtures") {
        let canonical = query["canonical"].as_str().expect("canonical query");
        assert_eq!(
            commitment(request_domain, canonical.as_bytes()),
            query["request_digest"].as_str().expect("request digest"),
            "{name}"
        );
    }

    let vault = &fixtures["vault"];
    assert_eq!(
        commitment(
            domains["state_key"].as_str().expect("state-key domain"),
            vault["store_key_utf8"]
                .as_str()
                .expect("vault store key")
                .as_bytes(),
        ),
        vault["record_key"].as_str().expect("vault record key")
    );
}

#[test]
fn cb2_k1_read_operation_commitment_bytes_preexisting_green() {
    let vector = vector();
    let operation_domain = vector["commitment_domains"]["operation_facts"]
        .as_str()
        .expect("operation-facts domain");
    let cases = vector["positive_cases"].as_array().expect("positive cases");

    assert_eq!(
        cases
            .iter()
            .map(|case| case["id"].as_str().expect("case id"))
            .collect::<Vec<_>>(),
        POSITIVE_IDS
    );

    for case in cases {
        let case_id = case["id"].as_str().expect("case id");
        let document = &case["document"];
        assert_eq!(
            object_keys(document),
            [OPERATION_PROFILE_KEY, "facts", "kind"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(document["kind"].as_str(), Some("read"), "{case_id}");
        assert_eq!(&document["facts"], &case["facts"], "{case_id}");

        let document_jcs = jcs::canonicalize(document).expect("operation JCS");
        assert_eq!(
            document_jcs,
            case["document_jcs"]
                .as_str()
                .expect("operation JCS fixture"),
            "{case_id}"
        );
        let digest = commitment(operation_domain, document_jcs.as_bytes());
        assert_eq!(
            digest,
            case["digest"].as_str().expect("operation digest"),
            "{case_id}"
        );
        assert_eq!(
            object_keys(&case["facts_ref"]),
            [OPERATION_PROFILE_KEY, "digest"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            case["facts_ref"][OPERATION_PROFILE_KEY], document[OPERATION_PROFILE_KEY],
            "{case_id}"
        );
        assert_eq!(
            case["facts_ref"]["digest"].as_str(),
            Some(digest.as_str()),
            "{case_id}"
        );

        let facts = &case["facts"];
        let expected_members: BTreeSet<String> =
            match facts["domain"].as_str().expect("read domain") {
                "ethos" => ["domain", "sid", "source_edition", "zone"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                "gamma" => ["domain", "request_digest", "source_head"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                "vault-config" => ["connector", "domain", "record_key", "source_edition"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                other => panic!("{case_id}: unregistered positive read domain {other}"),
            };
        assert_eq!(object_keys(facts), expected_members, "{case_id}");
    }
}

#[test]
fn cb2_k1_read_closed_family_and_error_inventory_preliminary() {
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

    let negatives = vector["negative_cases"].as_array().expect("read negatives");
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
