//! CB2 Gamma-v2 and semantic-replay vector consumer.
//!
//! Existing generic JCS, SHA-256, Ed25519 and H2 Merkle primitives reproduce
//! the independent Python oracle. Typed Gamma-v2 admission and the single pure
//! semantic replay front door remain explicit COMPILE-RED-PRELIMINAIRE gates.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::{gamma, jcs, wire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-gamma-v2-replay.json"
));
const F1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f1-gamma-chain.json"
));
const F2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f2-gamma-counting.json"
));
const F3_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f3-gamma-liveness.json"
));
const H2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/h2-gamma-roots.json"
));
const PROJECTION_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));

const VECTOR_SHA256: &str = "a3cc536ea452940af061ce421c238e08f0894923562b8c8193dbb8d8b853cd06";
const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 Gamma-v2 vector parses")
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

fn with_empty_signature(value: &Value) -> Value {
    let mut unsigned = value.clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    unsigned
}

fn verify_hex_signature(key: &str, message: &[u8], signature: &str) {
    let key_bytes = wire::multibase_to_ed25519_pub(key).expect("fixture multibase Ed25519 key");
    let key = VerifyingKey::from_bytes(&key_bytes).expect("fixture Ed25519 public key");
    let signature_bytes: [u8; 64] = hex::decode(signature)
        .expect("fixture signature hex")
        .try_into()
        .expect("64-byte fixture signature");
    key.verify(message, &Signature::from_bytes(&signature_bytes))
        .expect("fixture signature verifies");
}

fn edge_verdict(case: &Value) -> bool {
    fn rank_manifest(value: &str) -> Option<u8> {
        match value {
            "1.0.0-draft.1" => Some(1),
            "1.0.0-draft.2" => Some(2),
            _ => None,
        }
    }
    fn rank_gamma(value: &str) -> Option<u8> {
        match value {
            "v1" => Some(1),
            "v2" => Some(2),
            _ => None,
        }
    }
    let Some(parent_manifest) = case["parent_manifest"].as_str().and_then(rank_manifest) else {
        return false;
    };
    let Some(child_manifest) = case["child_manifest"].as_str().and_then(rank_manifest) else {
        return false;
    };
    let Some(parent_gamma) = case["parent_gamma"].as_str().and_then(rank_gamma) else {
        return false;
    };
    let Some(child_gamma) = case["child_gamma"].as_str().and_then(rank_gamma) else {
        return false;
    };
    parent_manifest == parent_gamma
        && child_manifest == child_gamma
        && child_manifest >= parent_manifest
        && child_gamma >= parent_gamma
}

#[test]
fn cb2_gamma_v2_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_gamma_v2_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("f1-gamma-chain.json", F1_BYTES.as_bytes()),
        ("f2-gamma-counting.json", F2_BYTES.as_bytes()),
        ("f3-gamma-liveness.json", F3_BYTES.as_bytes()),
        ("h2-gamma-roots.json", H2_BYTES.as_bytes()),
        ("cb2-operation-projection.json", PROJECTION_BYTES.as_bytes()),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            vector["historical_vector_sha256"][name]
                .as_str()
                .expect("historical hash"),
            "{name}"
        );
    }
    assert_eq!(
        vector["inventory"]["historical_bytes_are_not_reinterpreted"],
        true
    );
}

#[test]
fn cb2_gamma_v2_signed_kind_and_reference_bytes_preexisting_green() {
    let vector = vector();
    let cases = vector["kind_cases"].as_array().expect("kind cases");
    assert_eq!(cases.len(), 12);
    let expected_kinds = vector["inventory"]["registered_kinds"]
        .as_array()
        .expect("registered kinds")
        .iter()
        .map(|kind| kind.as_str().expect("kind"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        cases
            .iter()
            .map(|case| case["kind"].as_str().expect("kind"))
            .collect::<BTreeSet<_>>(),
        expected_kinds
    );

    for case in cases {
        let entry = &case["entry"];
        let entry_jcs = jcs::canonicalize(entry).expect("Gamma entry JCS");
        assert_eq!(
            entry_jcs,
            case["entry_jcs"].as_str().expect("oracle entry JCS"),
            "{}",
            case["kind"]
        );
        assert_eq!(
            sha256_text(entry_jcs.as_bytes()),
            case["entry_hash"],
            "{}",
            case["kind"]
        );
        let preimage =
            jcs::canonicalize(&with_empty_signature(entry)).expect("Gamma signature preimage");
        assert_eq!(
            preimage,
            case["preimage_jcs"]
                .as_str()
                .expect("oracle signature preimage"),
            "{}",
            case["kind"]
        );
        verify_hex_signature(
            entry["signature"]["key"].as_str().expect("Gamma signer"),
            preimage.as_bytes(),
            entry["signature"]["value"]
                .as_str()
                .expect("Gamma signature"),
        );

        if case["operation_ref_presence"] == "required" {
            let projection = &case["projection"];
            let projection_jcs = jcs::canonicalize(projection).expect("operation projection JCS");
            assert_eq!(
                object_keys(&entry["operation_ref"]),
                ["aithos-operation-core", "occurrence", "commitment"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            );
            assert_eq!(
                entry["operation_ref"]["occurrence"],
                projection["occurrence"]
            );
            assert_eq!(
                entry["operation_ref"]["commitment"],
                commitment(OPERATION_DOMAIN, projection_jcs.as_bytes())
            );
            assert!(
                !entry["payload"]
                    .as_object()
                    .is_some_and(|payload| payload.contains_key("operation_ref")),
                "operation_ref remains a signed top-level member"
            );
        } else {
            assert!(case["projection"].is_null());
            assert!(
                !entry
                    .as_object()
                    .expect("entry object")
                    .contains_key("operation_ref"),
                "heartbeat carries no operation_ref"
            );
        }
    }
}

#[test]
fn cb2_gamma_v2_monotonicity_occurrence_and_raw_h2_preexisting_green() {
    let vector = vector();
    let edges = vector["monotonicity_cases"]
        .as_array()
        .expect("monotonicity cases");
    assert_eq!(edges.len(), 8);
    for case in edges {
        assert_eq!(
            edge_verdict(case),
            case["expected_accepted"].as_bool().expect("edge verdict")
        );
    }
    let migration = &vector["migration_merge"];
    assert_eq!(migration["manifest_profile"], "1.0.0-draft.2");
    assert_eq!(migration["gamma_kind"], "merge");
    assert_eq!(migration["merge_entry"]["v"], 2);
    assert!(migration["merge_entry"]
        .as_object()
        .expect("merge entry")
        .contains_key("operation_ref"));
    for (jcs_member, hash_member) in [
        ("v1_entry_jcs", "v1_entry_sha256"),
        ("v2_entry_jcs", "v2_entry_sha256"),
    ] {
        assert_eq!(
            sha256_hex(
                migration["retained_parent_bytes"][jcs_member]
                    .as_str()
                    .expect("retained JCS")
                    .as_bytes()
            ),
            migration["retained_parent_bytes"][hash_member]
        );
    }
    assert_eq!(migration["physical_order_is_not_a_causal_edge"], true);

    let mut accepted = BTreeMap::<String, String>::new();
    let first = &vector["kind_cases"]
        .as_array()
        .expect("kind cases")
        .iter()
        .find(|case| case["kind"] == "action")
        .expect("action case")["entry"]["operation_ref"];
    accepted.insert(
        first["occurrence"].as_str().expect("occurrence").to_owned(),
        first["commitment"].as_str().expect("commitment").to_owned(),
    );
    let mut outcomes = Vec::new();
    for case in vector["occurrence_cases"]
        .as_array()
        .expect("occurrence cases")
    {
        let reference = &case["operation_ref"];
        let occurrence = reference["occurrence"].as_str().expect("occurrence");
        let commitment = reference["commitment"].as_str().expect("commitment");
        let outcome = match accepted.get(occurrence) {
            Some(existing) if existing == commitment => "refused-as-replay-before-tally",
            Some(_) => "refused-as-equivocation-before-tally",
            None => {
                accepted.insert(occurrence.to_owned(), commitment.to_owned());
                "accepted-as-distinct-occurrence"
            }
        };
        assert_eq!(outcome, case["expected"].as_str().expect("outcome"));
        outcomes.push(outcome);
    }
    assert_eq!(outcomes.len(), 3);

    let h2 = &vector["raw_h2_fixture"];
    let lines = h2["lines_jcs"].as_array().expect("H2 lines");
    let line_bytes = lines
        .iter()
        .map(|line| line.as_str().expect("Gamma line").as_bytes())
        .collect::<Vec<_>>();
    assert_eq!(hex::encode(gamma::segment_root(&line_bytes)), h2["root"]);
    assert_eq!(line_bytes.len() as u64, h2["n"].as_u64().expect("n"));
    assert_eq!(
        h2["existing_counter_tally"]["mandate_01J00000000000000000000091"]["actions"],
        2
    );
    assert_eq!(h2["non_gamma_evidence"]["contributes_line"], false);
    assert_eq!(h2["non_gamma_evidence"]["contributes_count"], false);
    assert_eq!(h2["mutation_counter_present"], false);
    assert_eq!(h2["total_consumption_counter_present"], false);
}

#[test]
fn cb2_gamma_v2_closed_negative_and_semantic_api_inventory_preliminary() {
    let vector = vector();
    let entries = vector["negative_entry_cases"]
        .as_array()
        .expect("entry negatives");
    let correlation = vector["negative_correlation_cases"]
        .as_array()
        .expect("correlation negatives");
    let positives = vector["semantic_replay_positive_cases"]
        .as_array()
        .expect("semantic positives");
    let semantic = vector["semantic_replay_negative_cases"]
        .as_array()
        .expect("semantic negatives");
    assert_eq!(entries.len(), 35);
    assert_eq!(correlation.len(), 2);
    assert_eq!(positives.len(), 9);
    assert_eq!(semantic.len(), 10);
    assert!(entries
        .iter()
        .all(|case| case["must_fail"] == "InvalidGammaEntry"));
    assert!(correlation
        .iter()
        .all(|case| case["must_fail"] == "InvalidOperation"));
    assert!(positives.iter().all(|case| case["expected"] == "accepted"));
    assert!(semantic.iter().all(|case| {
        case["accepted_prefix_and_counters_unchanged"] == true
            && case["must_fail"].as_str().is_some_and(|variant| {
                matches!(
                    variant,
                    "GammaBudgetExhausted"
                        | "GammaObligationUnsatisfied"
                        | "GammaHeartbeatStale"
                        | "MandateRevoked"
                        | "GammaGrantNotLogged"
                        | "InvalidMandate"
                        | "InvalidGammaEntry"
                        | "InvalidOperation"
                )
            })
    }));
    assert_eq!(
        vector["inventory"]["entry_negative_ids"],
        Value::Array(entries.iter().map(|case| case["id"].clone()).collect())
    );
    assert_eq!(
        vector["inventory"]["correlation_negative_ids"],
        Value::Array(correlation.iter().map(|case| case["id"].clone()).collect())
    );
    assert_eq!(
        vector["inventory"]["semantic_negative_ids"],
        Value::Array(semantic.iter().map(|case| case["id"].clone()).collect())
    );
    assert_eq!(
        vector["inventory"]["entry_error_variant"],
        "InvalidGammaEntry"
    );
    assert_eq!(
        vector["inventory"]["correlation_error_variant"],
        "InvalidOperation"
    );
    assert_eq!(
        vector["inventory"]["semantic_replay_requires_one_pure_front_door"],
        true
    );
}
