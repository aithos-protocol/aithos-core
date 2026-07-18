//! CB2 R2/U1 receipt and draft3 obligation-matcher vector consumer.
//!
//! Existing JCS, SHA-256, multibase and Ed25519 primitives reproduce the
//! independent oracle. Typed receipt validation and draft3 matching remain
//! deliberate COMPILE-RED-PRELIMINAIRE gates for CB4/CB5.

use std::collections::BTreeSet;

use aithos_core::{jcs, wire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-receipts.json"
));
const GPLUS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/gplus-obligations.json"
));
const FPLUS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/fplus-constraints.json"
));
const EPLUS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/eplus-attenuation.json"
));
const PROJECTION_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));
const AI_FACTS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-action-inference.json"
));

const VECTOR_SHA256: &str = "2ce3d53bda43dc28ce599a8f7ec97d0050c3bf61b8f9ade4b51e8a74336ff22c";
const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 R2/U1 vector parses")
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

fn without_sig(value: &Value) -> Value {
    let mut unsigned = value.clone();
    unsigned
        .as_object_mut()
        .expect("receipt object")
        .remove("sig")
        .expect("receipt sig");
    unsigned
}

fn verifying_key(multibase: &str) -> VerifyingKey {
    let bytes = wire::multibase_to_ed25519_pub(multibase).expect("fixture multibase key");
    VerifyingKey::from_bytes(&bytes).expect("fixture Ed25519 public key")
}

fn signature(value: &Value) -> Signature {
    let bytes: [u8; 64] = hex::decode(value["sig"].as_str().expect("receipt signature"))
        .expect("signature hex")
        .try_into()
        .expect("64-byte Ed25519 signature");
    Signature::from_bytes(&bytes)
}

fn assert_signed_by_any(value: &Value, keys: impl IntoIterator<Item = VerifyingKey>) {
    let message = jcs::canonical_bytes(&without_sig(value)).expect("receipt preimage JCS");
    let signature = signature(value);
    assert!(
        keys.into_iter()
            .any(|key| key.verify(&message, &signature).is_ok()),
        "receipt verifies under a pinned key"
    );
}

fn operation_tuple(context: &Value) -> Value {
    let kind = context["kind"].as_str().expect("context kind");
    let native = &context["native"];
    match kind {
        "read" => json!({"kind": "read", "domain": native["domain"]}),
        "mutation" => json!({
            "kind": "mutation",
            "domain": native["domain"],
            "verb": native["verb"],
        }),
        "inference" | "grant" | "revoke" => json!({"kind": kind}),
        "rotate" => json!({"kind": "rotate", "domain": native["domain"]}),
        "publication" => json!({"kind": "publication", "mode": native["mode"]}),
        other => panic!("unsupported fixture matcher context {other}"),
    }
}

#[test]
fn cb2_r2_u1_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_r2_u1_historical_v1_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("gplus-obligations.json", GPLUS_BYTES.as_bytes()),
        ("fplus-constraints.json", FPLUS_BYTES.as_bytes()),
        ("eplus-attenuation.json", EPLUS_BYTES.as_bytes()),
        ("cb2-operation-projection.json", PROJECTION_BYTES.as_bytes()),
        (
            "cb2-operation-facts-action-inference.json",
            AI_FACTS_BYTES.as_bytes(),
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
    assert_eq!(
        vector["inventory"]["historical_v1_is_not_reinterpreted"],
        true
    );
}

#[test]
fn cb2_r2_u1_projection_and_receipt_signature_bytes_preexisting_green() {
    let vector = vector();
    let contexts = vector["contexts"].as_object().expect("operation contexts");
    for (id, context) in contexts {
        let projection_jcs =
            jcs::canonicalize(&context["projection"]).expect("operation projection JCS");
        assert_eq!(
            projection_jcs,
            context["projection_jcs"].as_str().expect("oracle JCS"),
            "{id}"
        );
        assert_eq!(
            context["operation_ref"],
            json!({
                "aithos-operation-core": "1.0.0-draft.1",
                "occurrence": context["projection"]["occurrence"],
                "commitment": commitment(OPERATION_DOMAIN, projection_jcs.as_bytes()),
            }),
            "{id}"
        );
        assert!(
            !projection_jcs.contains("receipt"),
            "post-effect receipt is not a projection input: {id}"
        );
    }

    let positives = &vector["positive_receipts"];
    let r2_without = &positives["r2_without_presented_digest"]["receipt"];
    let r2_with = &positives["r2_with_presented_digest"]["receipt"];
    let r2_mutation = &positives["r2_draft3_mutation"]["receipt"];
    let u1_action = &positives["u1_action"]["receipt"];
    let u1_inference = &positives["u1_inference"]["receipt"];

    assert_eq!(
        object_keys(r2_without),
        [
            "v",
            "family",
            "operation_ref",
            "obligation",
            "verdict",
            "at",
            "sig",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(
        object_keys(r2_with),
        [
            "v",
            "family",
            "operation_ref",
            "obligation",
            "verdict",
            "presented_digest",
            "at",
            "sig",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(object_keys(r2_mutation), object_keys(r2_with));
    assert_eq!(
        object_keys(u1_action),
        ["v", "family", "operation_ref", "model", "tokens", "sig"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(
        object_keys(u1_inference),
        [
            "v",
            "family",
            "operation_ref",
            "tokens_in",
            "tokens_out",
            "sig",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );

    for (name, record) in positives.as_object().expect("positive receipts") {
        let receipt = &record["receipt"];
        assert_eq!(
            jcs::canonicalize(&without_sig(receipt)).expect("receipt preimage"),
            record["preimage_jcs"].as_str().expect("oracle preimage"),
            "{name}"
        );
    }

    let action_attestors = vector["obligations"]["action"]["attestor"]
        .as_array()
        .expect("action attestors")
        .iter()
        .map(|key| verifying_key(key.as_str().expect("attestor key")))
        .collect::<Vec<_>>();
    let mutation_attestors = vector["obligations"]["mutation"]["attestor"]
        .as_array()
        .expect("mutation attestors")
        .iter()
        .map(|key| verifying_key(key.as_str().expect("attestor key")))
        .collect::<Vec<_>>();
    assert_signed_by_any(r2_without, action_attestors.clone());
    assert_signed_by_any(r2_with, action_attestors);
    assert_signed_by_any(r2_mutation, mutation_attestors);

    let usage_key = verifying_key(
        vector["budget_profile"]["attestation_key"]
            .as_str()
            .expect("usage attestation key"),
    );
    assert_signed_by_any(u1_action, [usage_key]);
    assert_signed_by_any(u1_inference, [usage_key]);
    assert_eq!(positives["u1_action"]["actual_tokens"], 8412);
    assert_eq!(positives["u1_inference"]["actual_tokens"], 1500);
}

#[test]
fn cb2_r2_u1_matcher_and_attenuation_bytes_preexisting_green() {
    let vector = vector();
    let contexts = &vector["contexts"];
    let matcher_cases = vector["matcher_cases"].as_array().expect("matcher cases");
    assert_eq!(matcher_cases.len(), 9);
    for case in matcher_cases {
        let context = &contexts[case["context"].as_str().expect("context id")];
        assert_eq!(
            case["matcher"] == operation_tuple(context),
            case["expected_applicable"]
                .as_bool()
                .expect("expected applicability"),
            "{}",
            case["id"]
        );
    }

    let chain = vector["draft3_obligation_chain"]
        .as_array()
        .expect("draft3 chain");
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0]["aithos-mandate-core"], "1.0.0-draft.3");
    assert_eq!(chain[1]["aithos-mandate-core"], "1.0.0-draft.3");
    assert_eq!(chain[1]["parent"], chain[0]["id"]);
    assert_eq!(
        chain[1]["constraints"]["obligations"][0], chain[0]["constraints"]["obligations"][0],
        "inherited obligation remains byte-identical"
    );
    assert_eq!(
        chain[1]["constraints"]["obligations"]
            .as_array()
            .expect("child obligations")
            .len(),
        2,
        "child adds one distinct tightening obligation"
    );
}

#[test]
fn cb2_r2_u1_closed_negative_and_api_inventory_preliminary() {
    let vector = vector();
    let r2 = vector["negative_r2_cases"]
        .as_array()
        .expect("R2 negatives");
    let u1 = vector["negative_u1_cases"]
        .as_array()
        .expect("U1 negatives");
    let matcher = vector["negative_matcher_cases"]
        .as_array()
        .expect("matcher negatives");
    let chain = vector["negative_matcher_chain_cases"]
        .as_array()
        .expect("matcher chain negatives");
    assert_eq!(r2.len(), 25);
    assert_eq!(u1.len(), 31);
    assert_eq!(matcher.len(), 20);
    assert_eq!(chain.len(), 4);
    assert!(r2
        .iter()
        .all(|case| case["must_fail"] == "GammaObligationUnsatisfied"));
    assert!(u1
        .iter()
        .all(|case| case["must_fail"] == "InvalidGammaEntry"));
    assert!(matcher
        .iter()
        .chain(chain)
        .all(|case| case["must_fail"] == "InvalidMandate"));
    assert_eq!(
        vector["inventory"]["r2_error_variant"],
        "GammaObligationUnsatisfied"
    );
    assert_eq!(vector["inventory"]["u1_error_variant"], "InvalidGammaEntry");
    assert_eq!(
        vector["inventory"]["matcher_error_variant"],
        "InvalidMandate"
    );
    assert_eq!(vector["inventory"]["sig_is_omitted_from_preimage"], true);
    assert_eq!(
        vector["inventory"]["receipts_are_not_operation_projection_inputs"],
        true
    );
}
