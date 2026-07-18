//! CB2 Bundle transaction and local trust-boundary vector consumer.
//!
//! Generic JSON/JCS/SHA-256 operations reproduce the independent Python
//! oracle. Transactional Store support, confinement enforcement, opaque typed
//! capabilities and keyless export/cold replay remain explicit later gates.

use std::collections::BTreeSet;

use aithos_core::jcs;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-boundaries.json"
));
const VECTOR_SHA256: &str = "73149da64fbdc73bcfd81f8a3d11c83e9421e43f2053ad649bf0db0f585ee187";

const A1_GENESIS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/a1-genesis.json"
));
const A2_DID: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/a2-did.json"
));
const BUNDLE_COEXISTENCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-version-coexistence.json"
));
const DRAFT2_CARRIERS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-draft2-carriers.json"
));
const H1_MERKLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/h1-merkle.json"
));
const H2_GAMMA_ROOTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/h2-gamma-roots.json"
));
const I1_CONCURRENCY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/i1-concurrency.json"
));

const BUNDLE_LIB_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
const BUNDLE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bundle.rs"));
const LOG_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/log.rs"));
const MERGE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/merge.rs"));

fn vector() -> Value {
    serde_json::from_slice(VECTOR_BYTES).expect("CB2 Bundle boundary vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn snapshot_digest(value: &Value) -> String {
    let canonical = jcs::canonicalize(value).expect("fixture snapshot JCS");
    format!("sha256:{}", sha256_hex(canonical.as_bytes()))
}

fn state(value: Option<&Value>) -> Value {
    match value {
        Some(Value::String(bytes)) => json!({
            "state": "present",
            "sha256": sha256_hex(bytes.as_bytes()),
        }),
        None => json!({"state": "absent"}),
        Some(_) => panic!("fixture Store value is a string"),
    }
}

fn build_write_set(old: &Value, new: &Value) -> Value {
    let old = old.as_object().expect("old snapshot object");
    let new = new.as_object().expect("new snapshot object");
    let paths: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    Value::Array(
        paths
            .into_iter()
            .filter_map(|path| {
                let before = state(old.get(path));
                let after = state(new.get(path));
                (before != after).then(|| {
                    json!({
                        "path": path,
                        "before": before,
                        "after": after,
                    })
                })
            })
            .collect(),
    )
}

fn name_accepted(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn sid_accepted(value: &str) -> bool {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    value.len() == 26 && value.bytes().all(|byte| ALPHABET.contains(&byte))
}

fn hash_accepted(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn display_path_accepted(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with(['/', '\\'])
        && !value.contains(['\\', '\0'])
        && value
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | "..") && name_accepted(segment))
}

fn store_key_accepted(value: &str) -> bool {
    if value.is_empty() || value.starts_with(['/', '\\']) || value.contains(['\\', '\0']) {
        return false;
    }
    let segments: Vec<_> = value.split('/').collect();
    if segments
        .iter()
        .any(|segment| matches!(*segment, "" | "." | ".."))
    {
        return false;
    }
    if matches!(value, "manifest.json" | "did.json")
        || matches!(
            value,
            "e/public/index.json"
                | "e/circle/index.json"
                | "e/self/index.json"
                | "gamma/gamma.jsonl"
        )
    {
        return true;
    }
    if let Some(relative) = value
        .strip_prefix("e/public/")
        .and_then(|rest| rest.strip_suffix(".md"))
    {
        return display_path_accepted(relative);
    }
    if segments.len() == 4
        && segments[0] == "e"
        && matches!(segments[1], "circle" | "self")
        && segments[2] == "blobs"
    {
        return segments[3].strip_suffix(".enc").is_some_and(sid_accepted);
    }
    if segments.len() == 4
        && segments[0] == "e"
        && matches!(segments[1], "circle" | "self")
        && segments[2] == "hdr"
    {
        return segments[3]
            .strip_suffix(".json")
            .is_some_and(|stem| stem == "root" || sid_accepted(stem));
    }
    if segments.len() == 2 && segments[0] == "certs" {
        return segments[1]
            .strip_suffix(".json")
            .and_then(|stem| stem.strip_prefix("mandate_"))
            .is_some_and(sid_accepted);
    }
    if segments.len() == 2 && segments[0] == "gamma" {
        let archive = segments[1].strip_suffix(".jsonl").is_some_and(|stem| {
            let bytes = stem.as_bytes();
            bytes.len() == 7
                && bytes[0..4].iter().all(u8::is_ascii_digit)
                && bytes[4] == b'-'
                && bytes[5..7].iter().all(u8::is_ascii_digit)
        });
        return segments[1] == "gamma.jsonl" || archive;
    }
    if segments.len() == 2 && segments[0] == "manifests" {
        let stem = segments[1].strip_suffix(".json").unwrap_or_default();
        return stem.bytes().all(|byte| byte.is_ascii_digit())
            || stem
                .strip_prefix("tree-")
                .is_some_and(|height| height.bytes().all(|byte| byte.is_ascii_digit()))
            || ["index-public-", "index-circle-", "index-self-"]
                .iter()
                .any(|prefix| {
                    stem.strip_prefix(prefix)
                        .is_some_and(|height| height.bytes().all(|byte| byte.is_ascii_digit()))
                });
    }
    if segments.len() == 2 && matches!(segments[0], "changesets" | "evidence") {
        return segments[1].strip_suffix(".json").is_some_and(hash_accepted);
    }
    if segments.len() >= 3 && segments[0] == "x" {
        let connector = segments[1];
        let connector_accepted = connector
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
            && name_accepted(connector);
        let intermediate_accepted = segments[2..segments.len() - 1]
            .iter()
            .all(|segment| name_accepted(segment));
        let final_accepted = segments
            .last()
            .and_then(|last| {
                last.strip_suffix(".enc")
                    .or_else(|| last.strip_suffix(".json"))
            })
            .is_some_and(name_accepted);
        return connector_accepted && intermediate_accepted && final_accepted;
    }
    false
}

fn capability_verdict(capability: &Value, request: &Value) -> &'static str {
    let Some(capability) = capability.as_object() else {
        return "refused";
    };
    let Some(request) = request.as_object() else {
        return "refused";
    };
    let capability_keys: BTreeSet<_> = capability.keys().map(String::as_str).collect();
    let request_keys: BTreeSet<_> = request.keys().map(String::as_str).collect();
    if capability_keys != BTreeSet::from(["class", "context", "opaque_fixture_label"])
        || request_keys != BTreeSet::from(["class", "context", "protocol_object"])
        || capability["class"] != request["class"]
        || capability["context"] != request["context"]
    {
        return "refused";
    }
    let expected_object = match capability["class"].as_str() {
        Some("sign_manifest") => "edition_manifest",
        Some("sign_gamma") => "gamma_entry",
        Some("open_body") => "sealed_body",
        Some("wrap_header") => "header_line",
        Some("audit_args") => "sealed_action_args",
        Some("open_config") => "vault_config",
        _ => return "refused",
    };
    if request["protocol_object"] == expected_object {
        "accepted"
    } else {
        "refused"
    }
}

fn contains_secret_shape(value: &Value) -> bool {
    const FORBIDDEN: &[&str] = &[
        "seed",
        "private_key",
        "secret_key",
        "owner_keys",
        "dk",
        "credential",
        "plaintext",
        "capability",
    ];
    match value {
        Value::Object(object) => object.iter().any(|(name, value)| {
            FORBIDDEN.contains(&name.to_ascii_lowercase().as_str()) || contains_secret_shape(value)
        }),
        Value::Array(array) => array.iter().any(contains_secret_shape),
        _ => false,
    }
}

#[test]
fn cb2_bundle_boundary_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES), VECTOR_SHA256);
}

#[test]
fn cb2_bundle_boundary_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("a1-genesis.json", A1_GENESIS),
        ("a2-did.json", A2_DID),
        ("cb2-bundle-version-coexistence.json", BUNDLE_COEXISTENCE),
        ("cb2-draft2-carriers.json", DRAFT2_CARRIERS),
        ("h1-merkle.json", H1_MERKLE),
        ("h2-gamma-roots.json", H2_GAMMA_ROOTS),
        ("i1-concurrency.json", I1_CONCURRENCY),
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
fn cb2_bundle_transaction_write_set_and_recovery_preexisting_green() {
    let vector = vector();
    let transaction = &vector["transaction"];
    assert_eq!(
        snapshot_digest(&transaction["old_snapshot"]),
        transaction["old_snapshot_digest"]
    );
    assert_eq!(
        snapshot_digest(&transaction["new_snapshot"]),
        transaction["new_snapshot_digest"]
    );
    assert_eq!(
        build_write_set(&transaction["old_snapshot"], &transaction["new_snapshot"]),
        transaction["write_set"]
    );
    assert_eq!(transaction["linearization_count"], 1);
    assert_eq!(transaction["staging_outside_canonical_namespace"], true);
    assert_eq!(
        transaction["internal_generation_metadata_is_not_wire"],
        true
    );

    let failures = transaction["failure_cases"]
        .as_array()
        .expect("failure cases");
    assert_eq!(failures.len(), 12);
    for case in failures {
        assert_eq!(case["visible_snapshot"], "old", "{}", case["id"]);
        assert_eq!(case["canonical_digest"], transaction["old_snapshot_digest"]);
        assert_eq!(case["staging_is_canonical"], false);
    }

    let recoveries = transaction["recovery_cases"]
        .as_array()
        .expect("recovery cases");
    assert_eq!(recoveries.len(), 4);
    assert_eq!(
        recoveries
            .iter()
            .filter(|case| case["visible_snapshot"] == "old")
            .count(),
        2
    );
    assert_eq!(
        recoveries
            .iter()
            .filter(|case| case["visible_snapshot"] == "new")
            .count(),
        2
    );
}

#[test]
fn cb2_bundle_confinement_decisions_preexisting_green() {
    let vector = vector();
    let confinement = &vector["confinement"];
    let cases = confinement["cases"].as_array().expect("confinement cases");
    assert_eq!(cases.len(), 20);
    for case in cases {
        let value = case["value"].as_str().expect("path value");
        let accepted = match case["input_kind"].as_str() {
            Some("display_path") => display_path_accepted(value),
            Some("store_key" | "cold_load_key" | "recovery_key") => store_key_accepted(value),
            _ => false,
        } && !case["resolved_outside_root"]
            .as_bool()
            .expect("resolved-outside-root flag");
        let actual = if accepted { "accepted" } else { "refused" };
        assert_eq!(actual, case["expected"], "{}", case["id"]);
    }
    assert_eq!(confinement["accepted_count"], 5);
    assert_eq!(confinement["refused_count"], 15);
    assert_eq!(confinement["signed_manifest_cannot_authorize_escape"], true);
}

#[test]
fn cb2_bundle_capability_keyless_and_api_inventory_preliminary() {
    let vector = vector();
    let capabilities = &vector["capabilities"];
    let positives = capabilities["positive_cases"]
        .as_array()
        .expect("positive capability cases");
    let negatives = capabilities["negative_cases"]
        .as_array()
        .expect("negative capability cases");
    assert_eq!(positives.len(), 6);
    assert_eq!(negatives.len(), 18);
    for case in positives.iter().chain(negatives) {
        assert_eq!(
            capability_verdict(&case["capability"], &case["request"]),
            case["expected"],
            "{}",
            case["id"]
        );
    }
    assert_eq!(capabilities["no_stable_encoding_promoted"], true);
    assert_eq!(capabilities["no_generic_sign_open_wrap_oracle"], true);
    assert_eq!(
        capabilities["no_raw_seed_private_key_dk_or_credential"],
        true
    );
    assert_eq!(
        capabilities["session_binding"]["ambient_capability_pool"],
        false
    );

    let export = &vector["keyless_export"];
    assert_eq!(snapshot_digest(&export["objects"]), export["export_digest"]);
    assert!(!contains_secret_shape(&export["objects"]));
    assert_eq!(export["secret_shape_detected"], false);
    assert_eq!(export["network_participates"], false);
    assert_eq!(export["provider_cas_participates"], false);
    let cold_cases = export["cold_cases"].as_array().expect("cold cases");
    assert_eq!(cold_cases.len(), 8);
    for case in cold_cases {
        let actual = if case["defect"] == "none" {
            "accepted"
        } else {
            "refused"
        };
        assert_eq!(actual, case["expected"], "{}", case["id"]);
    }

    assert!(BUNDLE_LIB_SOURCE.contains("pub trait Store"));
    for method in ["fn get(", "fn put(", "fn list("] {
        assert!(BUNDLE_LIB_SOURCE.contains(method), "{method}");
    }
    assert!(BUNDLE_SOURCE.contains("&OwnerKeys"));
    assert!(BUNDLE_SOURCE.contains("&StaticSecret"));
    for absent in [
        "pub fn export_keyless",
        "pub fn cold_verify",
        "pub fn import_keyless",
        "OpaqueCapability",
    ] {
        assert!(!BUNDLE_SOURCE.contains(absent), "{absent}");
    }
}

#[test]
fn cb2_cb7_bundle_transaction_confinement_api_gate() {
    for present in [
        "fn begin_transaction(",
        "fn commit_transaction(",
        "fn rollback_transaction(",
        "fn recover_transaction(",
        "pub fn validate_store_key(",
        "pub fn validate_display_path(",
        "struct FsTransaction",
        ".aithos-current",
    ] {
        assert!(BUNDLE_LIB_SOURCE.contains(present), "{present}");
    }
    assert!(BUNDLE_SOURCE.contains("pub fn transaction<"));
    assert!(BUNDLE_SOURCE.contains("fn write_object("));
    assert_eq!(BUNDLE_SOURCE.matches("self.store.put(").count(), 1);
    assert!(!LOG_SOURCE.contains("self.store.put("));
    assert!(!MERGE_SOURCE.contains("self.store.put("));
    assert!(!BUNDLE_LIB_SOURCE.contains("self.root.join(path)"));
}
