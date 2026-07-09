//! Conformance vector A2 — DID document + identity-epoch transition
//! (spec 01.4, 10.4). Expected canonical strings were generated
//! independently (Python blake3 + PyNaCl + base58); Ed25519 signatures are
//! deterministic (RFC 8032), so JCS strings must match byte for byte.

use aithos_core::did::{DidDocument, EpochTransition};
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use serde::Deserialize;

#[derive(Deserialize)]
struct A2 {
    seed_hex: String,
    succession_seed_hex: String,
    successor_seed_hex: String,
    successor_succession_seed_hex: String,
    bundle: String,
    revocations: String,
    did: String,
    successor_did: String,
    did_doc_jcs: String,
    transition_jcs: String,
}

fn vector() -> A2 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/a2-did.json"
    ));
    serde_json::from_str(raw).expect("vector a2-did.json parses")
}

fn seed32(hex_str: &str) -> [u8; 32] {
    hex::decode(hex_str).unwrap().try_into().unwrap()
}

fn identity(
    v: &A2,
    seed_hex: &str,
    succ_hex: &str,
) -> (OwnerKeys, ed25519_dalek::SigningKey, DidDocument) {
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(seed32(seed_hex)));
    let succession = succession_from_entropy(seed32(succ_hex));
    let doc = DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec![v.bundle.clone()],
        v.revocations.clone(),
    )
    .unwrap();
    (owner, succession, doc)
}

#[test]
fn a2_did_document_matches_and_verifies() {
    let v = vector();
    let (_, _, doc) = identity(&v, &v.seed_hex, &v.succession_seed_hex);
    assert_eq!(doc.id, v.did);
    assert_eq!(
        jcs::canonicalize(&doc).unwrap(),
        v.did_doc_jcs,
        "JCS cross-check vs Python"
    );
    doc.verify().expect("well-formed document verifies");
}

#[test]
fn a2_tampered_document_fails_closed() {
    let v = vector();
    let (_, _, doc) = identity(&v, &v.seed_hex, &v.succession_seed_hex);
    let mut tampered = doc.clone();
    tampered.revocations.push('x');
    assert!(tampered.verify().is_err());
    let mut wrong_id = doc;
    wrong_id.id = v.successor_did.clone();
    assert!(wrong_id.verify().is_err());
}

#[test]
fn a2_epoch_transition_succession_only() {
    let v = vector();
    let (owner1, succession1, doc1) = identity(&v, &v.seed_hex, &v.succession_seed_hex);
    let (_, _, doc2) = identity(&v, &v.successor_seed_hex, &v.successor_succession_seed_hex);

    let tr = EpochTransition::sign(
        &succession1,
        doc1.id.clone(),
        doc2.id.clone(),
        "2026-07-09T00:00:00Z".to_owned(),
    )
    .unwrap();
    assert_eq!(
        jcs::canonicalize(&tr).unwrap(),
        v.transition_jcs,
        "JCS cross-check vs Python"
    );
    tr.verify(&doc1)
        .expect("succession-signed transition is accepted");

    // Even the root key itself cannot declare a new master key.
    let rogue = EpochTransition::sign_with(
        &owner1.root_sign,
        "#root",
        doc1.id.clone(),
        doc2.id.clone(),
        "2026-07-09T00:00:00Z".to_owned(),
    )
    .unwrap();
    assert!(rogue.verify(&doc1).is_err());

    // Root-signed but CLAIMING #succession: signature check still rejects.
    let forged = EpochTransition::sign_with(
        &owner1.root_sign,
        "#succession",
        doc1.id.clone(),
        doc2.id.clone(),
        "2026-07-09T00:00:00Z".to_owned(),
    )
    .unwrap();
    assert!(forged.verify(&doc1).is_err());
}
