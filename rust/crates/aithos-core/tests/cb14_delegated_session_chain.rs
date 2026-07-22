//! G4/P7 vectors-first contract for SC1 over a verified non-root leaf.
//! The historical `verify_session` surface and SC1/W1.1 bytes stay frozen.

use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::mandate::Mandate;
use aithos_core::operation::{
    verify_delegated_session, DelegatedSessionEvidence, SessionEvidence,
};
use aithos_core::revocation::Revocation;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb14-delegated-session-chain.json"
));
const VECTOR_SHA256: &str = "1a744d4f7fc48cf0676264f4c56ee38804999395c4e7423e1d1933fa1d9c9a84";

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB14 delegated-session vector parses")
}

fn verify(candidate: &Value) -> aithos_core::Result<Value> {
    let chain: Vec<Mandate> = serde_json::from_value(candidate["chain"].clone()).unwrap();
    let did: DidDocument = serde_json::from_value(candidate["did"].clone()).unwrap();
    let revocations = candidate["revocations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| Revocation {
            mandate_id: item["mandate_id"].as_str().unwrap().to_owned(),
            revoked_at: item["revoked_at"].as_str().unwrap().to_owned(),
        })
        .collect::<Vec<_>>();
    verify_delegated_session(DelegatedSessionEvidence {
        chain: &chain,
        did: &did,
        at: candidate["at"].as_str().unwrap(),
        revocations: &revocations,
        session: SessionEvidence {
            mandate: &candidate["mandate"],
            certificate: &candidate["certificate"],
            projection: &candidate["operation_projection"],
            operation_ref: &candidate["operation_ref"],
            native_leaf_proof: Some(&candidate["native_leaf_proof"]),
            native_leaf_domain: b"aithos-core/cb2/native-leaf-proof\0",
            session_proof: Some(&candidate["session_proof"]),
        },
    })
    .map(|verified| verified.operation_ref().clone())
}

#[test]
fn cb14_vector_hash_is_frozen() {
    assert_eq!(
        hex::encode(Sha256::digest(VECTOR_BYTES.as_bytes())),
        VECTOR_SHA256
    );
}

#[test]
fn cb14_verified_non_root_leaf_reuses_sc1_and_both_proofs() {
    let vector = vector();
    let positive = &vector["positive"];
    assert!(positive["chain"][1]["parent"].is_string());
    assert_eq!(verify(positive).unwrap(), positive["operation_ref"]);
}

#[test]
fn cb14_chain_revocation_leaf_selection_proof_and_time_fail_closed() {
    for case in vector()["negative_cases"].as_array().unwrap() {
        let error = verify(&case["candidate"]).unwrap_err();
        assert!(
            matches!(error, Error::InvalidSession(_)),
            "{} returned {error}",
            case["id"]
        );
    }
}
