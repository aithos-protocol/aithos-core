//! CB2 SC1 session-certificate and proof vector consumer.
//!
//! Existing JCS, SHA-256, multibase and Ed25519 primitives reproduce the
//! independent oracle.  The future typed SC1 validator, `InvalidSession`
//! variant and `max_sessions` lifecycle remain deliberate compile gates.

use std::collections::BTreeSet;

use aithos_core::{jcs, wire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-session-proof.json"
));
const E1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/e1-mandate.json"
));
const PROJECTION_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));
const MUTATION_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
));

const VECTOR_SHA256: &str = "17553dd95f515e8045e17e8e46816b1d7e2007d4985eec0048fcac34197bef74";
const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";
const NATIVE_LEAF_DOMAIN: &[u8] = b"aithos-core/cb2/native-leaf-proof\0";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 SC1 vector parses")
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

fn unsigned_with_empty_signature(value: &Value) -> Value {
    let mut unsigned = value.clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    unsigned
}

fn without_member(value: &Value, member: &str) -> Value {
    let mut unsigned = value.clone();
    unsigned
        .as_object_mut()
        .expect("fixture object")
        .remove(member)
        .expect("fixture member");
    unsigned
}

#[test]
fn cb2_sc1_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES.as_bytes()), VECTOR_SHA256);
}

#[test]
fn cb2_sc1_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("e1-mandate.json", E1_BYTES.as_bytes()),
        ("cb2-operation-projection.json", PROJECTION_BYTES.as_bytes()),
        (
            "cb2-operation-facts-mutation.json",
            MUTATION_BYTES.as_bytes(),
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
}

#[test]
fn cb2_sc1_mandate_certificate_and_digest_bytes_preexisting_green() {
    let vector = vector();
    let positive = &vector["positive"];
    let mandate = &positive["mandate"];
    let certificate = &positive["certificate"];

    let mandate_preimage =
        jcs::canonicalize(&unsigned_with_empty_signature(mandate)).expect("mandate preimage JCS");
    let root_key = mandate["subject"]
        .as_str()
        .expect("mandate subject")
        .strip_prefix("did:aithos:")
        .expect("Aithos DID root key");
    verify_hex_signature(
        root_key,
        mandate_preimage.as_bytes(),
        mandate["signature"]["value"]
            .as_str()
            .expect("mandate signature"),
    );

    assert_eq!(
        object_keys(certificate),
        [
            "aithos-session-core",
            "subject",
            "mandate_id",
            "key",
            "not_before",
            "not_after",
            "signature",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(certificate["aithos-session-core"], "1.0.0-draft.1");
    assert_eq!(certificate["subject"], mandate["subject"]);
    assert_eq!(certificate["mandate_id"], mandate["id"]);
    assert_eq!(certificate["key"], mandate["constraints"]["session_bind"]);
    assert_eq!(
        certificate["signature"]["key"],
        mandate["grantee"]["pubkey"]
    );

    let certificate_preimage =
        jcs::canonicalize(&unsigned_with_empty_signature(certificate)).expect("SC1 preimage JCS");
    assert_eq!(
        certificate_preimage,
        positive["certificate_preimage_jcs"]
            .as_str()
            .expect("oracle SC1 preimage JCS")
    );
    verify_hex_signature(
        certificate["signature"]["key"]
            .as_str()
            .expect("SC1 signer key"),
        certificate_preimage.as_bytes(),
        certificate["signature"]["value"]
            .as_str()
            .expect("SC1 signature"),
    );

    let certificate_jcs = jcs::canonicalize(certificate).expect("complete SC1 JCS");
    assert_eq!(
        certificate_jcs,
        positive["certificate_jcs"]
            .as_str()
            .expect("oracle complete SC1 JCS")
    );
    assert_eq!(
        sha256_text(certificate_jcs.as_bytes()),
        positive["certificate_digest"]
            .as_str()
            .expect("oracle SC1 digest")
    );
}

#[test]
fn cb2_sc1_operation_and_double_possession_bytes_preexisting_green() {
    let vector = vector();
    let positive = &vector["positive"];
    let projection = &positive["operation_projection"];
    let operation_ref = &positive["operation_ref"];
    let proof = &positive["session_proof"];
    let native = &positive["native_leaf_proof_fixture"];

    let projection_jcs = jcs::canonicalize(projection).expect("session-bound projection JCS");
    assert_eq!(
        projection_jcs,
        positive["operation_projection_jcs"]
            .as_str()
            .expect("oracle projection JCS")
    );
    let operation_commitment = commitment(OPERATION_DOMAIN, projection_jcs.as_bytes());
    assert_eq!(
        operation_ref,
        &serde_json::json!({
            "aithos-operation-core": "1.0.0-draft.1",
            "occurrence": projection["occurrence"],
            "commitment": operation_commitment,
        })
    );
    assert_eq!(
        projection["authority"]["session"]["key"],
        positive["certificate"]["key"]
    );
    assert_eq!(
        projection["authority"]["session"]["certificate_digest"],
        positive["certificate_digest"]
    );

    assert_eq!(
        object_keys(proof),
        ["aithos-session-proof-core", "operation_ref", "key", "sig"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(proof["operation_ref"], *operation_ref);
    assert_eq!(proof["key"], positive["certificate"]["key"]);
    let session_preimage =
        jcs::canonicalize(&without_member(proof, "sig")).expect("session proof preimage JCS");
    assert_eq!(
        session_preimage,
        positive["session_proof_preimage_jcs"]
            .as_str()
            .expect("oracle session preimage JCS")
    );
    verify_hex_signature(
        proof["key"].as_str().expect("session key"),
        session_preimage.as_bytes(),
        proof["sig"].as_str().expect("session signature"),
    );

    let mut native_message = NATIVE_LEAF_DOMAIN.to_vec();
    native_message.extend_from_slice(
        jcs::canonicalize(operation_ref)
            .expect("operation reference JCS")
            .as_bytes(),
    );
    assert_eq!(
        hex::encode(&native_message),
        positive["native_leaf_proof_message_hex"]
            .as_str()
            .expect("oracle native message")
    );
    assert_eq!(native["key"], positive["mandate"]["grantee"]["pubkey"]);
    verify_hex_signature(
        native["key"].as_str().expect("native leaf key"),
        &native_message,
        native["sig"].as_str().expect("native leaf signature"),
    );
}

#[test]
fn cb2_sc1_closed_negative_and_api_inventory_preliminary() {
    let vector = vector();
    let negatives = vector["negative_cases"].as_array().expect("SC1 negatives");
    assert_eq!(negatives.len(), 29);
    assert_eq!(
        vector["inventory"]["negative_ids"],
        Value::Array(negatives.iter().map(|case| case["id"].clone()).collect())
    );
    assert!(negatives
        .iter()
        .all(|case| case["must_fail"] == "InvalidSession"));
    assert_eq!(
        vector["inventory"]["required_error_variant"],
        "InvalidSession"
    );
    assert_eq!(
        vector["inventory"]["native_leaf_proof_is_test_fixture_not_wire"],
        true
    );
    assert_eq!(
        vector["inventory"]["sc1_conveys_no_perimeter_or_authority"],
        true
    );
    assert_eq!(
        vector["inventory"]["max_sessions_lifecycle_is_out_of_scope"],
        true
    );
}
