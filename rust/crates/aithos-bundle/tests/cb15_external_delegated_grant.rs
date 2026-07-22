//! G4/P7 contract for a gateway-prepared Gamma grant signed outside Bundle.
//! The entry is the existing Gamma v1 wire; Bundle never receives the key.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::FsStore;
use aithos_core::did::DidDocument;
use aithos_core::gamma::Entry;
use aithos_core::mandate::Mandate;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb15-external-delegated-grant.json"
));
const VECTOR_SHA256: &str = "bf7a73082b4ef93f76672dc0ae42178c720b515d22df311be073a35d4e4fc440";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB15 external-grant vector parses")
}

fn verify(candidate: &Value) -> aithos_core::Result<()> {
    let did: DidDocument = serde_json::from_value(candidate["did"].clone()).unwrap();
    let chain: Vec<Mandate> = serde_json::from_value(candidate["minting_chain"].clone()).unwrap();
    let child: Mandate = serde_json::from_value(candidate["child"].clone()).unwrap();
    let entry: Entry = serde_json::from_value(candidate["signed_entry"].clone()).unwrap();
    Bundle::<FsStore>::verify_external_delegated_grant(&entry, &chain, &child, &did, &[], &[])
}

#[test]
fn cb15_vector_hash_is_frozen() {
    assert_eq!(
        hex::encode(Sha256::digest(VECTOR_BYTES.as_bytes())),
        VECTOR_SHA256
    );
}

#[test]
fn cb15_existing_grant_wire_is_accepted_without_delegate_key_custody() {
    verify(&vector()["positive"]).expect("the exact externally signed grant verifies");
}

#[test]
fn cb15_binding_head_time_and_signature_fail_closed() {
    for case in vector()["negative_cases"].as_array().unwrap() {
        assert!(verify(&case["candidate"]).is_err(), "{}", case["id"]);
    }
}
