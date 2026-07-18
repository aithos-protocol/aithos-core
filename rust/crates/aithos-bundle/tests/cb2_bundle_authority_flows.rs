//! CB2 owner/grantee Bundle authority-flow vector consumer.
//!
//! The tests reproduce the approved CB8/CB9 pure decision matrices without
//! adding production behavior. Unified owner/grantee operations, exact grant
//! delivery and durable delegated publication remain explicit compile gates.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-authority-flows.json"
));
const VECTOR_SHA256: &str = "30545958c170fda12e53817d3c5b7adb295432a4352e1abcbc7749fdd5c7eca0";

const MANDATE_CONTRACTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-mandate-contracts.json"
));
const OPERATION_PROJECTION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));
const FACTS_MUTATION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
));
const FACTS_READ: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-read.json"
));
const DELEGATED_COUNTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-delegated-counts.json"
));
const SESSION_PROOF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-session-proof.json"
));
const DRAFT2_CARRIERS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-draft2-carriers.json"
));
const BUNDLE_BOUNDARIES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-boundaries.json"
));

const BUNDLE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bundle.rs"));
const GRANTS_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/grants.rs"));
const LOG_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/log.rs"));

fn vector() -> Value {
    serde_json::from_slice(VECTOR_BYTES).expect("CB2 Bundle authority-flow vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn state_digest(value: &Value) -> String {
    let canonical = jcs::canonicalize(value).expect("fixture state JCS");
    format!("sha256:{}", sha256_hex(canonical.as_bytes()))
}

fn parsed_authority(value: &str) -> (&str, &str, Option<(&str, &str)>) {
    let (scope, selector) = value
        .split_once('#')
        .map_or((value, None), |(scope, selector)| (scope, Some(selector)));
    let (verb, zone) = scope.split_once('.').expect("fixture verb.zone authority");
    let selector = selector.map(|selector| {
        let (key, value) = selector
            .split_once('=')
            .expect("fixture selector key=value");
        (key, value)
    });
    (verb, zone, selector)
}

fn verb_covers(operation: &str, verb: &str) -> bool {
    match operation {
        "list" | "read" => matches!(verb, "read" | "edit" | "append" | "delete" | "write"),
        "create" => matches!(verb, "append" | "write"),
        "edit" => matches!(verb, "edit" | "append" | "write"),
        "delete" => matches!(verb, "delete" | "write"),
        _ => false,
    }
}

fn grantee_verdict(case: &Value) -> &'static str {
    let (verb, zone, selector) = parsed_authority(case["authority"].as_str().expect("authority"));
    let operation = case["operation"].as_str().expect("operation");
    if zone != case["zone"].as_str().expect("zone") || !verb_covers(operation, verb) {
        return "refused";
    }
    let selector_matches = match selector {
        None => true,
        Some(("id", value)) => case["target_sid"] == value,
        Some(("dir", value)) => case["target_dir"] == value,
        Some(("tag", value)) => case["target_tags"]
            .as_array()
            .expect("target tags")
            .iter()
            .any(|tag| tag == value),
        Some(_) => false,
    };
    if zone == "self"
        && matches!(operation, "create" | "edit" | "delete")
        && matches!(selector, Some(("dir" | "tag", _)))
    {
        "refused"
    } else if selector_matches {
        "accepted"
    } else {
        "refused"
    }
}

fn content_fence_verdict(case: &Value) -> &'static str {
    if case["authority"] != "valid covering chain" {
        "refused as unauthorized"
    } else if case["key_material"] == "exact valid section line" {
        "readable and authorized"
    } else if case["key_material"] == "no section line" {
        "authorized but unreadable"
    } else {
        "unreadable"
    }
}

#[test]
fn cb2_bundle_authority_flow_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES), VECTOR_SHA256);
}

#[test]
fn cb2_bundle_authority_flow_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("cb2-mandate-contracts.json", MANDATE_CONTRACTS),
        ("cb2-operation-projection.json", OPERATION_PROJECTION),
        ("cb2-operation-facts-mutation.json", FACTS_MUTATION),
        ("cb2-operation-facts-read.json", FACTS_READ),
        ("cb2-delegated-counts.json", DELEGATED_COUNTS),
        ("cb2-session-proof.json", SESSION_PROOF),
        ("cb2-draft2-carriers.json", DRAFT2_CARRIERS),
        ("cb2-bundle-boundaries.json", BUNDLE_BOUNDARIES),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            vector["historical_vector_sha256"][name]
                .as_str()
                .expect("historical vector hash"),
            "{name}"
        );
    }
}

#[test]
fn cb2_bundle_owner_parity_matrix_preexisting_green() {
    let vector = vector();
    assert_eq!(
        state_digest(&vector["initial_state"]),
        vector["initial_state_digest"]
    );
    let cases = vector["owner_cases"].as_array().expect("owner cases");
    assert_eq!(cases.len(), 15);
    let mut combinations = BTreeSet::new();
    for case in cases {
        assert_eq!(case["expected"], "accepted");
        assert_eq!(case["mandate_required"], false);
        assert_eq!(case["mandate_counter_delta"], 0);
        assert_eq!(case["fresh_store_reopen"], true);
        combinations.insert((
            case["zone"].as_str().expect("zone"),
            case["operation"].as_str().expect("operation"),
        ));
    }
    assert_eq!(combinations.len(), 15);
}

#[test]
fn cb2_bundle_grantee_grant_and_content_fences_preexisting_green() {
    let vector = vector();
    let cases = vector["grantee_cases"].as_array().expect("grantee cases");
    assert_eq!(cases.len(), 18);
    for case in cases {
        assert_eq!(grantee_verdict(case), case["expected"], "{}", case["id"]);
        if case["expected"] == "refused" {
            assert_eq!(
                case["refused_visible_state_digest"],
                vector["initial_state_digest"]
            );
        } else {
            assert_eq!(case["accepted_actor"], "grantee");
            assert_eq!(case["accepted_single_chain"], true);
            assert_eq!(case["accepted_journalized"], true);
            assert_eq!(case["accepted_fresh_store_reopen"], true);
        }
    }

    let delivery = vector["grant_delivery_cases"]
        .as_array()
        .expect("grant delivery cases");
    assert_eq!(delivery.len(), 9);
    let expected: BTreeMap<_, _> = [
        ("read.public#id=note", "none"),
        ("read.circle", "zone-root"),
        ("read.self", "zone-root"),
        ("edit.circle#dir=projects", "folder"),
        ("read.circle#tag=toto", "zone-tag-view"),
        ("read.circle#dir=projects&tag=toto", "folder-tag-view"),
        ("edit.self#id=opaque-note", "section"),
        ("act.x.mail.send", "none"),
        ("act.x.mail.config", "connector-vault"),
    ]
    .into_iter()
    .collect();
    for case in delivery {
        let authority = case["authority"].as_str().expect("delivery authority");
        assert_eq!(
            case["required_line"],
            *expected
                .get(authority)
                .expect("registered delivery authority")
        );
    }

    let fences = vector["content_fence_cases"]
        .as_array()
        .expect("content-fence cases");
    assert_eq!(fences.len(), 4);
    for case in fences {
        assert_eq!(content_fence_verdict(case), case["expected"]);
    }
}

#[test]
fn cb2_bundle_delegated_evidence_atomicity_and_api_inventory_preliminary() {
    let vector = vector();
    let evidence = &vector["delegated_evidence"];
    let authorship: BTreeSet<_> = evidence["public_authorship_required_members"]
        .as_array()
        .expect("authorship members")
        .iter()
        .map(|member| member.as_str().expect("member"))
        .collect();
    assert_eq!(
        authorship,
        BTreeSet::from([
            "subject",
            "zone",
            "sid",
            "content_hash",
            "operation_ref",
            "edition",
            "authorized_via",
            "key",
            "sig",
        ])
    );
    assert_eq!(evidence["owner_signature_substitution"], "refused");
    let self_cases = evidence["self_state_cases"]
        .as_array()
        .expect("self state cases");
    assert_eq!(self_cases.len(), 3);
    for case in self_cases {
        assert_eq!(
            case["disclosed"],
            serde_json::json!(["sid", "before_commitment", "after_commitment"])
        );
        assert_eq!(case["forbidden"].as_array().expect("forbidden").len(), 7);
    }

    for field in ["current_authority_cases", "atomic_refusal_cases"] {
        for case in vector[field].as_array().expect("refusal cases") {
            assert_eq!(case["expected"], "refused");
            assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
        }
    }
    let gamma = vector["gamma_read_cases"]
        .as_array()
        .expect("Gamma-read cases");
    assert_eq!(gamma.len(), 3);
    assert_eq!(
        gamma
            .iter()
            .filter(|case| case["expected"] == "refused as unauthorized")
            .count(),
        1
    );

    assert!(GRANTS_SOURCE.contains("pub fn grant("));
    assert!(GRANTS_SOURCE.contains("pub fn section_add_as_agent("));
    assert!(GRANTS_SOURCE.contains("pub fn section_rewrite_as_agent("));
    assert!(GRANTS_SOURCE.contains("pub fn section_delete_as_agent("));
    assert!(GRANTS_SOURCE.contains("delegated writes: circle only this pass"));
    assert!(GRANTS_SOURCE.contains("\"e/circle/index.json\""));
    assert!(LOG_SOURCE.contains("pub fn log_query_as_agent("));
    assert!(BUNDLE_SOURCE.contains("&OwnerKeys"));
    for absent in [
        "pub fn content_operation(",
        "pub fn grantee_content_operation(",
        "pub fn open_bundle_session(",
        "pub fn export_keyless(",
    ] {
        assert!(
            !BUNDLE_SOURCE.contains(absent)
                && !GRANTS_SOURCE.contains(absent)
                && !LOG_SOURCE.contains(absent),
            "{absent}"
        );
    }
}

#[test]
fn cb2_cb8_owner_parity_generic_grants_api_gate() {
    for present in [
        "pub enum OwnerContentOperation",
        "pub enum OwnerContentOutcome",
        "pub fn owner_content_operation(",
        "OwnerContentOperation::List",
        "OwnerContentOperation::Read",
        "OwnerContentOperation::Create",
        "OwnerContentOperation::Edit",
        "OwnerContentOperation::Delete",
    ] {
        assert!(BUNDLE_SOURCE.contains(present), "{present}");
    }
    for present in [
        "pub enum GrantSelector",
        "pub enum GenericGrantRequest",
        "pub enum GrantLineKind",
        "pub fn grant_generic(",
        "GrantLineKind::ConnectorVault",
        "self.transaction(",
    ] {
        assert!(GRANTS_SOURCE.contains(present), "{present}");
    }
}
