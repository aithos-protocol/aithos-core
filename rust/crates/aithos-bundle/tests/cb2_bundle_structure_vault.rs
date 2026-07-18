//! CB2 structural, revocation and vault Bundle vector consumer.
//!
//! Generic data checks reproduce the approved CB10 matrices. Transactional
//! structural/revocation flows and exact connector-vault CRUD remain explicit
//! later compile gates.

use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-structure-vault.json"
));
const VECTOR_SHA256: &str = "ebaf4871a233f1f4631f1ec21c63a3cec68fd0829c6a9dddfe3d6d9aa4179f5d";

const G1_REVOCATION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/g1-revocation.json"
));
const G2_ROTATION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/g2-rotation.json"
));
const FACTS_MUTATION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
));
const FACTS_STRUCTURAL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-structural.json"
));
const CONNECTOR_CATALOG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-connector-catalog.json"
));
const DRAFT2_CARRIERS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-draft2-carriers.json"
));
const BUNDLE_BOUNDARIES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-boundaries.json"
));
const AUTHORITY_FLOWS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-authority-flows.json"
));

const BUNDLE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bundle.rs"));
const REVOKE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/revoke.rs"));
const STATE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/state.rs"));
const STRUCTURE_SOURCE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/structure.rs"));
const VAULT_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vault.rs"));

fn vector() -> Value {
    serde_json::from_slice(VECTOR_BYTES).expect("CB2 Bundle structure/vault vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn state_digest(value: &Value) -> String {
    let canonical = jcs::canonicalize(value).expect("fixture state JCS");
    format!("sha256:{}", sha256_hex(canonical.as_bytes()))
}

fn structural_verdict(case: &Value) -> &'static str {
    let operation = case["operation"].as_str().expect("structural operation");
    let source = case["source_verb"].as_str();
    let destination = case["destination_verb"].as_str();
    let accepted = match operation {
        "list_read_folder" => {
            matches!(
                source,
                Some("read" | "edit" | "append" | "delete" | "write")
            )
        }
        "create_child_folder" => matches!(destination, Some("append" | "write")),
        "rename_folder" => matches!(source, Some("edit" | "append" | "write")),
        "delete_empty_folder" => matches!(source, Some("delete" | "write")),
        "move_folder" => {
            matches!(source, Some("edit" | "append" | "write"))
                && matches!(destination, Some("append" | "write"))
        }
        "delete_nonempty_folder" => {
            matches!(source, Some("delete" | "write"))
                && case["complete_subtree"].as_bool() == Some(true)
        }
        _ => false,
    };
    if accepted {
        "accepted"
    } else {
        "refused"
    }
}

fn vault_access_verdict(case: &Value) -> &'static str {
    if case["authority"] == "act.x.calendar.config" {
        "cannot open /x/mail"
    } else if case["authority"] != "act.x.mail.config" {
        "refused as unauthorized"
    } else if case["line"] == "exact /x/mail line" {
        "authorized and readable"
    } else if case["line"] == "no vault line" {
        "authorized but unreadable"
    } else {
        "unreadable"
    }
}

#[test]
fn cb2_bundle_structure_vault_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES), VECTOR_SHA256);
}

#[test]
fn cb2_bundle_structure_vault_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("g1-revocation.json", G1_REVOCATION),
        ("g2-rotation.json", G2_ROTATION),
        ("cb2-operation-facts-mutation.json", FACTS_MUTATION),
        ("cb2-operation-facts-structural.json", FACTS_STRUCTURAL),
        ("cb2-connector-catalog.json", CONNECTOR_CATALOG),
        ("cb2-draft2-carriers.json", DRAFT2_CARRIERS),
        ("cb2-bundle-boundaries.json", BUNDLE_BOUNDARIES),
        ("cb2-bundle-authority-flows.json", AUTHORITY_FLOWS),
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
fn cb2_bundle_structural_authority_and_atomic_consequences_preexisting_green() {
    let vector = vector();
    assert_eq!(
        state_digest(&vector["initial_state"]),
        vector["initial_state_digest"]
    );
    let structural = &vector["structural"];
    let cases = structural["authority_cases"]
        .as_array()
        .expect("structural authority cases");
    assert_eq!(cases.len(), 26);
    for case in cases {
        assert_eq!(structural_verdict(case), case["expected"], "{}", case["id"]);
    }
    assert_eq!(structural["covered_read_hides_siblings"], true);
    assert_eq!(structural["new_wire_verb"], false);
    let derived = structural["derived_cases"]
        .as_array()
        .expect("derived structural cases");
    assert_eq!(derived.len(), 3);
    for case in derived {
        assert_eq!(case["one_transaction"], true);
        assert!(case["consequences"]
            .as_array()
            .expect("consequences")
            .iter()
            .any(|value| value == "manifest"));
    }
    let failures = structural["failure_cases"]
        .as_array()
        .expect("structural failures");
    assert_eq!(failures.len(), 7);
    for case in failures {
        assert_eq!(case["expected"], "refused");
        assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
    }
    let self_cases = structural["self_cases"]
        .as_array()
        .expect("self structure cases");
    assert_eq!(self_cases.len(), 4);
    assert_eq!(
        self_cases
            .iter()
            .filter(|case| case["expected"] == "accepted")
            .count(),
        2
    );
}

#[test]
fn cb2_bundle_revocation_rotation_reopen_preexisting_green() {
    let vector = vector();
    let revocation = &vector["revocation"];
    let success = &revocation["success"];
    assert_eq!(success["linearization_count"], 1);
    assert_eq!(success["revoked_line_opens_new_material"], false);
    assert_eq!(success["fresh_keyless_store_verifies"], true);
    assert_eq!(
        success["steps"].as_array().expect("revocation steps").len(),
        6
    );
    let failures = revocation["failure_cases"]
        .as_array()
        .expect("revocation failures");
    assert_eq!(failures.len(), 6);
    for case in failures {
        assert_eq!(case["expected"], "refused");
        assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
        assert!(case["reachable_attempt_artifacts"]
            .as_array()
            .expect("reachable attempt artifacts")
            .is_empty());
    }
    let time = revocation["time_cases"]
        .as_array()
        .expect("revocation time cases");
    assert_eq!(time[0]["expected"], "accepted");
    assert_eq!(time[1]["expected"], "refused");
    assert_eq!(time[0]["state_source"], "verified prior prefix");
    assert_eq!(time[1]["state_source"], "verified prior prefix");
}

#[test]
fn cb2_bundle_vault_isolation_and_cb10_api_inventory() {
    let vector = vector();
    let vault = &vector["vault"];
    assert_eq!(vault["config_is_outside_business_classes"], true);
    assert_eq!(vault["wildcard_covers_config"], false);
    assert_eq!(vault["inferred_binding_or_cosign"], false);
    assert_eq!(vault["network_participates"], false);
    assert_eq!(vault["upstream_effect_is_modeled"], false);

    let crud = vault["crud_cases"].as_array().expect("vault CRUD cases");
    assert_eq!(crud.len(), 4);
    for case in crud {
        assert_eq!(case["authority"], "act.x.mail.config");
        assert_eq!(case["line"], "exact /x/mail line");
        assert_eq!(case["expected"], "accepted");
        assert_eq!(case["external_mail_action_granted"], false);
    }
    let access = vault["access_cases"]
        .as_array()
        .expect("vault access cases");
    assert_eq!(access.len(), 7);
    for case in access {
        assert_eq!(vault_access_verdict(case), case["expected"]);
    }
    for case in vault["capability_substitution_cases"]
        .as_array()
        .expect("capability substitutions")
    {
        assert_eq!(case["expected"], "refused");
    }
    for case in vault["atomic_cases"]
        .as_array()
        .expect("vault atomic cases")
    {
        assert_eq!(case["changed_connector"], "mail");
        assert_eq!(case["unchanged_connector"], "calendar");
        assert_eq!(case["credential_in_keyless_output"], false);
        assert_eq!(case["one_transaction"], true);
    }
    for case in vault["failure_cases"]
        .as_array()
        .expect("vault failure cases")
    {
        assert_eq!(case["expected"], "refused");
        assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
    }
    assert_eq!(
        vault["public_forbidden"],
        serde_json::json!(["credential", "config plaintext", "private key", "DK"])
    );

    assert!(BUNDLE_SOURCE.contains("pub fn rename_folder("));
    assert!(REVOKE_SOURCE.contains("pub fn rotate_folder("));
    assert!(REVOKE_SOURCE.contains("pub fn move_folder("));
    assert!(REVOKE_SOURCE.contains("self.put_json("));
    assert!(BUNDLE_SOURCE.contains("\"e/x/header.json\""));
    assert!(STATE_SOURCE.contains("self.store.list(\"e/x/\")"));
    assert!(STATE_SOURCE.contains("path.ends_with(\"header.json\")"));
    for (source, api) in [
        (STRUCTURE_SOURCE, "pub fn structural_operation("),
        (REVOKE_SOURCE, "pub fn revoke_transaction("),
        (VAULT_SOURCE, "pub fn vault_config_operation("),
        (VAULT_SOURCE, "pub fn open_vault_with_capability("),
        (VAULT_SOURCE, "pub fn rotate_vault_connector("),
    ] {
        assert!(
            source.contains(api),
            "CB10 closed API surface is missing {api}"
        );
    }
    assert!(VAULT_SOURCE.contains("e/x/{connector}/header.json"));
    assert!(VAULT_SOURCE.contains("e/x/{connector}/manifest.enc"));
    assert!(VAULT_SOURCE.contains("exact act.x.{connector}.config authority is required"));
}
