//! CB2 K1.1-B/K1.2-M-B vector consumer.
//!
//! These tests exercise only existing generic JCS/SHA-256 primitives and lock
//! the independent oracle inventory.  They deliberately do not implement a
//! test-local mutation validator or claim that the future typed Core APIs are
//! GREEN.

use std::collections::BTreeSet;

use aithos_core::jcs;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
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

const VECTOR_SHA256: &str = "dc3a57de91ff50895b680111da4da981e1518ad2dcd638d862e63be9eb29b83d";
const OPERATION_PROFILE_KEY: &str = "aithos-operation-facts-core";
const STATE_PROFILE_KEY: &str = "aithos-state-fact-core";

const POSITIVE_IDS: [&str; 13] = [
    "ethos-create",
    "ethos-edit",
    "ethos-delete",
    "ethos-redact",
    "structure-create-folder",
    "structure-rename-folder",
    "structure-rename-section",
    "structure-delete-folder",
    "structure-move-folder",
    "structure-move-section",
    "vault-create",
    "vault-edit",
    "vault-delete",
];

const OPERATION_NEGATIVE_IDS: [&str; 23] = [
    "missing-envelope-profile",
    "extra-envelope-member",
    "unknown-envelope-profile",
    "kind-family-mismatch",
    "facts-ref-digest-mismatch",
    "unknown-domain",
    "unknown-domain-verb",
    "missing-family-member",
    "clear-display-path",
    "unknown-zone",
    "unknown-node-kind",
    "section-create-in-structure",
    "null-source",
    "destination-on-rename",
    "noncanonical-target-sid",
    "duplicate-source-sid",
    "noncanonical-source-order",
    "destination-contains-target",
    "cross-zone-destination",
    "invalid-create-transition",
    "equal-present-state-digests",
    "mismatched-vault-record-key",
    "clear-vault-record-name",
];

const STATE_NEGATIVE_IDS: [&str; 15] = [
    "absent-state-has-reference",
    "present-state-missing-reference",
    "unknown-state-ref-profile",
    "nonlowercase-state-ref-digest",
    "unknown-state-fact-profile",
    "empty-objects",
    "unsorted-objects",
    "duplicate-key-commitment",
    "missing-object-member",
    "extra-object-member",
    "malformed-byte-commitment",
    "missing-affected-object",
    "unrelated-extra-object",
    "clear-store-key",
    "state-digest-mismatch",
];

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 K1 vector parses")
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
fn cb2_k1_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_k1_historical_vector_hashes_preexisting_green() {
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
fn cb2_k1_state_commitment_bytes_preexisting_green() {
    let vector = vector();
    let domains = &vector["commitment_domains"];
    let state_key_domain = domains["state_key"].as_str().expect("state-key domain");
    let state_bytes_domain = domains["state_bytes"].as_str().expect("state-bytes domain");
    let state_fact_domain = domains["state_fact"].as_str().expect("state-fact domain");

    for (name, fixture) in vector["states"].as_object().expect("state fixtures") {
        let mut objects = Vec::new();
        for input in fixture["input_objects"].as_array().expect("state inputs") {
            let store_key = input["store_key_utf8"].as_str().expect("store key");
            let stored_bytes = hex::decode(
                input["stored_bytes_hex"]
                    .as_str()
                    .expect("stored bytes hex"),
            )
            .expect("stored bytes");
            let key_commitment = commitment(state_key_domain, store_key.as_bytes());
            let byte_commitment = commitment(state_bytes_domain, &stored_bytes);
            assert_eq!(
                key_commitment,
                input["key_commitment"].as_str().expect("key commitment"),
                "{name}"
            );
            assert_eq!(
                byte_commitment,
                input["byte_commitment"].as_str().expect("byte commitment"),
                "{name}"
            );
            objects.push(json!({
                "key_commitment": key_commitment,
                "byte_commitment": byte_commitment,
            }));
        }
        objects.sort_by(|left, right| {
            left["key_commitment"]
                .as_str()
                .cmp(&right["key_commitment"].as_str())
        });
        let document = json!({
            STATE_PROFILE_KEY: vector["profiles"][STATE_PROFILE_KEY]
                .as_str()
                .expect("state profile"),
            "objects": objects,
        });
        assert_eq!(&document, &fixture["document"], "{name}");

        let document_jcs = jcs::canonicalize(&document).expect("state JCS");
        assert_eq!(
            document_jcs,
            fixture["document_jcs"].as_str().expect("state JCS fixture"),
            "{name}"
        );
        assert_eq!(
            commitment(state_fact_domain, document_jcs.as_bytes()),
            fixture["digest"].as_str().expect("state digest"),
            "{name}"
        );
    }
}

#[test]
fn cb2_k1_operation_commitment_bytes_preexisting_green() {
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
        assert_eq!(document["kind"].as_str(), Some("mutation"), "{case_id}");
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
        let expected_members: BTreeSet<String> = match (
            facts["domain"].as_str().expect("mutation domain"),
            facts["verb"].as_str().expect("mutation verb"),
        ) {
            ("ethos", _) => ["after", "before", "dir", "domain", "sid", "verb", "zone"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ("structure", "create") => [
                "after",
                "before",
                "destination",
                "domain",
                "node_kind",
                "sid",
                "verb",
                "zone",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            ("structure", "rename" | "delete") => [
                "after",
                "before",
                "domain",
                "node_kind",
                "sid",
                "source",
                "verb",
                "zone",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            ("structure", "move") => [
                "after",
                "before",
                "destination",
                "domain",
                "node_kind",
                "sid",
                "source",
                "verb",
                "zone",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            ("vault-config", _) => [
                "after",
                "before",
                "connector",
                "domain",
                "record_key",
                "verb",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            other => panic!("{case_id}: unregistered positive family {other:?}"),
        };
        assert_eq!(object_keys(facts), expected_members, "{case_id}");
    }
}

#[test]
fn cb2_k1_closed_family_and_error_inventory_preliminary() {
    let vector = vector();
    assert_eq!(
        string_array(&vector["inventory"]["positive_case_ids"]),
        POSITIVE_IDS
    );
    assert_eq!(
        string_array(&vector["inventory"]["operation_negative_ids"]),
        OPERATION_NEGATIVE_IDS
    );
    assert_eq!(
        string_array(&vector["inventory"]["state_negative_ids"]),
        STATE_NEGATIVE_IDS
    );
    assert_eq!(
        string_array(&vector["inventory"]["required_error_variants"]),
        ["InvalidOperationFacts", "InvalidStateFact"]
    );

    let operation_negatives = vector["negative_cases"]["operation_facts"]
        .as_array()
        .expect("operation negatives");
    assert_eq!(operation_negatives.len(), OPERATION_NEGATIVE_IDS.len());
    for (case, expected_id) in operation_negatives.iter().zip(OPERATION_NEGATIVE_IDS) {
        assert_eq!(case["id"].as_str(), Some(expected_id));
        assert_eq!(
            case["must_fail"].as_str(),
            Some("InvalidOperationFacts"),
            "{expected_id}"
        );
    }

    let state_negatives = vector["negative_cases"]["state_facts"]
        .as_array()
        .expect("state negatives");
    assert_eq!(state_negatives.len(), STATE_NEGATIVE_IDS.len());
    for (case, expected_id) in state_negatives.iter().zip(STATE_NEGATIVE_IDS) {
        assert_eq!(case["id"].as_str(), Some(expected_id));
        assert_eq!(
            case["must_fail"].as_str(),
            Some("InvalidStateFact"),
            "{expected_id}"
        );
    }
}
