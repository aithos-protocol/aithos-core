//! CB2 CAT1 signed connector-catalog vector consumer.
//!
//! Existing JCS, SHA-256, multibase and Ed25519 primitives reproduce the
//! independent oracle. Typed catalog, approval, pin and action-class APIs
//! remain deliberate COMPILE-RED-PRELIMINAIRE gates for later Core bundles.

use std::collections::BTreeSet;

use aithos_core::{jcs, wire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-connector-catalog.json"
));
const A2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/a2-did.json"
));
const E1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/e1-mandate.json"
));
const AI_FACTS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-action-inference.json"
));
const RECEIPTS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-receipts.json"
));
const GPLUS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/gplus-obligations.json"
));

const VECTOR_SHA256: &str = "f73b35d29602217983c6401f06fbb49c73032a955d0ac14356d3a988181fe43c";
const FACTS_DOMAIN: &str = "aithos-core/v1/operation-facts";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 CAT1 vector parses")
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

fn expected_class_authorization(
    catalog: &Value,
    action: &str,
    authority: &str,
    owner_co_sign: bool,
) -> bool {
    let Some(class) = catalog["actions"]
        .as_array()
        .expect("catalog actions")
        .iter()
        .find(|row| row["name"] == action)
        .and_then(|row| row["class"].as_str())
    else {
        return false;
    };
    let connector = catalog["connector"].as_str().expect("connector");
    let exact = format!("act.x.{connector}.{action}");
    let wildcard = format!("act.x.{connector}.*");
    if authority == exact {
        return class != "binding" || owner_co_sign;
    }
    if authority == wildcard {
        return matches!(class, "read" | "act");
    }
    false
}

#[test]
fn cb2_cat1_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_cat1_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("a2-did.json", A2_BYTES.as_bytes()),
        ("e1-mandate.json", E1_BYTES.as_bytes()),
        (
            "cb2-operation-facts-action-inference.json",
            AI_FACTS_BYTES.as_bytes(),
        ),
        ("cb2-operation-receipts.json", RECEIPTS_BYTES.as_bytes()),
        ("gplus-obligations.json", GPLUS_BYTES.as_bytes()),
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
fn cb2_cat1_signed_documents_and_complete_digests_preexisting_green() {
    let vector = vector();
    let did = &vector["owner_did"]["document"];
    let did_jcs = jcs::canonicalize(did).expect("owner DID JCS");
    assert_eq!(
        did_jcs,
        vector["owner_did"]["document_jcs"]
            .as_str()
            .expect("oracle DID JCS")
    );
    let did_preimage = jcs::canonicalize(&with_empty_signature(did)).expect("owner DID preimage");
    verify_hex_signature(
        did["keys"]["root"].as_str().expect("owner root key"),
        did_preimage.as_bytes(),
        did["signature"]["value"]
            .as_str()
            .expect("owner DID signature"),
    );

    let catalog = &vector["catalog"]["document"];
    assert_eq!(
        object_keys(catalog),
        [
            "aithos-connector-catalog-core",
            "connector",
            "catalog_version",
            "actions",
            "signature",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert!(catalog["actions"]
        .as_array()
        .expect("catalog actions")
        .iter()
        .all(|action| object_keys(action)
            == ["name", "class"].into_iter().map(str::to_owned).collect()));
    let catalog_preimage =
        jcs::canonicalize(&with_empty_signature(catalog)).expect("catalog preimage JCS");
    assert_eq!(
        catalog_preimage,
        vector["catalog"]["preimage_jcs"]
            .as_str()
            .expect("oracle catalog preimage")
    );
    verify_hex_signature(
        catalog["signature"]["key"]
            .as_str()
            .expect("catalog signer key"),
        catalog_preimage.as_bytes(),
        catalog["signature"]["value"]
            .as_str()
            .expect("catalog signature"),
    );
    let catalog_jcs = jcs::canonicalize(catalog).expect("complete catalog JCS");
    assert_eq!(
        catalog_jcs,
        vector["catalog"]["document_jcs"]
            .as_str()
            .expect("oracle complete catalog JCS")
    );
    assert_eq!(
        sha256_text(catalog_jcs.as_bytes()),
        vector["catalog"]["catalog_digest"]
    );

    let approval = &vector["approval"]["document"];
    assert_eq!(
        object_keys(approval),
        [
            "aithos-connector-catalog-approval-core",
            "subject",
            "connector",
            "catalog_version",
            "catalog_digest",
            "approved_at",
            "signature",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(approval["subject"], did["id"]);
    assert_eq!(
        approval["catalog_digest"],
        vector["catalog"]["catalog_digest"]
    );
    let approval_preimage =
        jcs::canonicalize(&with_empty_signature(approval)).expect("approval preimage JCS");
    assert_eq!(
        approval_preimage,
        vector["approval"]["preimage_jcs"]
            .as_str()
            .expect("oracle approval preimage")
    );
    verify_hex_signature(
        did["keys"]["content"].as_str().expect("owner content key"),
        approval_preimage.as_bytes(),
        approval["signature"]["value"]
            .as_str()
            .expect("approval signature"),
    );
    let approval_jcs = jcs::canonicalize(approval).expect("complete approval JCS");
    assert_eq!(
        approval_jcs,
        vector["approval"]["document_jcs"]
            .as_str()
            .expect("oracle complete approval JCS")
    );
    assert_eq!(
        sha256_text(approval_jcs.as_bytes()),
        vector["approval"]["approval_digest"]
    );
    assert_ne!(
        catalog["signature"]["key"], did["keys"]["content"],
        "catalog proof and owner approval are distinct"
    );
}

#[test]
fn cb2_cat1_pin_chain_facts_and_class_bytes_preexisting_green() {
    let vector = vector();
    let pin = &vector["catalog_pin"];
    assert_eq!(
        object_keys(pin),
        [
            "connector",
            "catalog_version",
            "catalog_digest",
            "approval_digest",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(pin["catalog_digest"], vector["catalog"]["catalog_digest"]);
    assert_eq!(
        pin["approval_digest"],
        vector["approval"]["approval_digest"]
    );

    let chain = vector["draft3_chain"].as_array().expect("draft3 chain");
    assert_eq!(chain.len(), 2);
    assert!(chain
        .iter()
        .all(|mandate| mandate["aithos-mandate-core"] == "1.0.0-draft.3"));
    assert_eq!(chain[1]["parent"], chain[0]["id"]);
    assert_eq!(
        chain[0]["constraints"]["catalog_pins"], chain[1]["constraints"]["catalog_pins"],
        "catalog pins remain byte-identical through attenuation"
    );
    assert_eq!(chain[0]["constraints"]["catalog_pins"][0], *pin);

    let facts = &vector["action_facts"]["facts"];
    assert_eq!(
        facts["catalog_ref"],
        json!({
            "catalog_version": pin["catalog_version"],
            "catalog_digest": pin["catalog_digest"],
            "approval_digest": pin["approval_digest"],
        })
    );
    assert!(
        !facts
            .as_object()
            .expect("action facts object")
            .contains_key("class"),
        "action class is derived, never caller supplied"
    );
    let facts_document = json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "facts": facts,
    });
    let facts_jcs = jcs::canonicalize(&facts_document).expect("action facts JCS");
    assert_eq!(
        facts_jcs,
        vector["action_facts"]["facts_jcs"]
            .as_str()
            .expect("oracle action facts JCS")
    );
    assert_eq!(
        vector["action_facts"]["facts_ref"],
        json!({
            "aithos-operation-facts-core": "1.0.0-draft.1",
            "digest": commitment(FACTS_DOMAIN, facts_jcs.as_bytes()),
        })
    );
    assert_eq!(vector["action_facts"]["derived_class"], "act");

    let catalog = &vector["catalog"]["document"];
    for case in vector["class_cases"].as_array().expect("class cases") {
        let observed = expected_class_authorization(
            catalog,
            case["action"].as_str().expect("action"),
            case["authority"].as_str().expect("authority"),
            case["owner_co_sign"].as_bool().expect("owner co-sign"),
        );
        assert_eq!(
            observed,
            case["expected_authorized"]
                .as_bool()
                .expect("authorization result"),
            "{}",
            case["action"]
        );
    }
}

#[test]
fn cb2_cat1_closed_negative_and_api_inventory_preliminary() {
    let vector = vector();
    let catalog = vector["negative_catalog_cases"]
        .as_array()
        .expect("catalog negatives");
    let approval = vector["negative_approval_cases"]
        .as_array()
        .expect("approval negatives");
    let chain = vector["negative_chain_cases"]
        .as_array()
        .expect("chain negatives");
    let facts = vector["negative_action_facts_cases"]
        .as_array()
        .expect("action-facts negatives");
    assert_eq!(catalog.len(), 27);
    assert_eq!(approval.len(), 22);
    assert_eq!(chain.len(), 19);
    assert_eq!(facts.len(), 8);
    assert!(catalog
        .iter()
        .chain(approval)
        .all(|case| case["must_fail"] == "InvalidCatalog"));
    assert!(chain
        .iter()
        .all(|case| case["must_fail"] == "InvalidMandate"));
    assert!(facts
        .iter()
        .all(|case| case["must_fail"] == "InvalidOperationFacts"));
    for (inventory, cases) in [
        ("catalog_negative_ids", catalog),
        ("approval_negative_ids", approval),
        ("chain_negative_ids", chain),
        ("action_facts_negative_ids", facts),
    ] {
        assert_eq!(
            vector["inventory"][inventory],
            Value::Array(cases.iter().map(|case| case["id"].clone()).collect()),
            "{inventory}"
        );
    }
    assert_eq!(
        vector["inventory"]["catalog_error_variant"],
        "InvalidCatalog"
    );
    assert_eq!(vector["inventory"]["chain_error_variant"], "InvalidMandate");
    assert_eq!(
        vector["inventory"]["action_facts_error_variant"],
        "InvalidOperationFacts"
    );
    assert_eq!(
        vector["inventory"]["catalog_and_approval_are_distinct"],
        true
    );
    assert_eq!(vector["inventory"]["config_is_outside_catalog"], true);
    assert_eq!(
        vector["inventory"]["class_is_derived_not_caller_supplied"],
        true
    );
}
