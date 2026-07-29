//! Conformance vector A2 — DID document + identity-epoch transition
//! (spec 01.4, 10.4). Expected canonical strings were generated
//! independently (Python blake3 + PyNaCl + base58); Ed25519 signatures are
//! deterministic (RFC 8032), so JCS strings must match byte for byte.

use aithos_core::did::{DidDocument, EpochTransition, DID_VERSION};
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use ed25519_dalek::Signer;
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

/// One named way to make a DID document semantically invalid.
type DidDefect = (&'static str, Box<dyn Fn(&mut DidDocument)>);

/// Re-sign a mutated document under its own root key, so the ONLY thing a
/// rejection can be attributed to is the semantic control under test.
fn resign(doc: &mut DidDocument, root: &ed25519_dalek::SigningKey) {
    doc.signature.value = String::new();
    let bytes = jcs::canonical_bytes(&*doc).unwrap();
    doc.signature.value = hex::encode(root.sign(&bytes).to_bytes());
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
    tr.verify_succession(&doc1, &doc2)
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
    assert!(rogue.verify_succession(&doc1, &doc2).is_err());

    // Root-signed but CLAIMING #succession: signature check still rejects.
    let forged = EpochTransition::sign_with(
        &owner1.root_sign,
        "#succession",
        doc1.id.clone(),
        doc2.id.clone(),
        "2026-07-09T00:00:00Z".to_owned(),
    )
    .unwrap();
    assert!(forged.verify_succession(&doc1, &doc2).is_err());
}

// ---------------------------------------------------------------- AID-001
//
// A correct root signature is necessary, never sufficient. Every case below
// is CORRECTLY re-signed under its own root key, so only the semantic control
// under test can explain the rejection.

#[test]
fn aid001_signed_but_semantically_invalid_documents_are_rejected() {
    let v = vector();
    let (owner, _, base) = identity(&v, &v.seed_hex, &v.succession_seed_hex);
    let root = &owner.root_sign;

    let x25519_of_content = {
        let bytes = aithos_core::wire::multibase_to_ed25519_pub(&base.keys.content).unwrap();
        aithos_core::wire::x25519_pub_to_multibase(&bytes)
    };
    let ed25519_of_kex = {
        let bytes = aithos_core::wire::multibase_to_x25519_pub(&base.keys.kex).unwrap();
        aithos_core::wire::ed25519_pub_to_multibase(&bytes)
    };

    let cases: Vec<DidDefect> = vec![
        (
            "content key is not multibase at all",
            Box::new(|d: &mut DidDocument| d.keys.content = "not-a-key".to_owned()),
        ),
        (
            "content key carries the X25519 codec",
            Box::new(move |d: &mut DidDocument| d.keys.content = x25519_of_content.clone()),
        ),
        (
            "kex key carries the Ed25519 codec",
            Box::new(move |d: &mut DidDocument| d.keys.kex = ed25519_of_kex.clone()),
        ),
        (
            "kex key is truncated",
            Box::new(|d: &mut DidDocument| d.keys.kex.truncate(8)),
        ),
        (
            "succession key is malformed",
            Box::new(|d: &mut DidDocument| d.keys.succession = "z6Mk".to_owned()),
        ),
        (
            "unsupported document version",
            Box::new(|d: &mut DidDocument| d.version = "9.9.9".to_owned()),
        ),
        (
            "unsupported signature algorithm",
            Box::new(|d: &mut DidDocument| d.signature.alg = "secp256k1".to_owned()),
        ),
        (
            "signature fragment is not #root",
            Box::new(|d: &mut DidDocument| d.signature.key = "#content".to_owned()),
        ),
    ];

    for (label, defect) in cases {
        let mut doc = base.clone();
        defect(&mut doc);
        resign(&mut doc, root);
        // The signature itself is beyond reproach — only semantics fail.
        assert!(
            matches!(
                doc.verify(),
                Err(aithos_core::error::Error::InvalidDidDocument(_))
            ),
            "correctly re-signed document must still be rejected: {label}"
        );
    }

    // Control: the same re-signing pipeline on an UNMODIFIED document passes,
    // so the rejections above cannot be blamed on `resign`.
    let mut control = base.clone();
    resign(&mut control, root);
    control
        .verify()
        .expect("re-signed pristine document verifies");
}

#[test]
fn aid001_unknown_wire_members_are_refused_not_dropped() {
    let v = vector();
    let (_, _, doc) = identity(&v, &v.seed_hex, &v.succession_seed_hex);
    let jcs_text = jcs::canonicalize(&doc).unwrap();

    for (label, inject) in [
        ("top-level", r#"{"aithos-did-core""#),
        ("keys", r#""keys":{"#),
        ("signature", r#""signature":{"#),
    ] {
        let (needle, replacement) = match label {
            "top-level" => (inject, r#"{"aithos-extra":"x","aithos-did-core""#),
            "keys" => (inject, r#""keys":{"extra":"x","#),
            _ => (inject, r#""signature":{"extra":"x","#),
        };
        let wire = jcs_text.replacen(needle, replacement, 1);
        assert_ne!(wire, jcs_text, "the {label} injection must apply");
        assert!(
            serde_json::from_str::<DidDocument>(&wire).is_err(),
            "an unknown {label} member must be refused, not silently dropped"
        );
    }
}

// ---------------------------------------------------------------- AID-002
//
// The transition is a triple: previous document, transition, successor
// document. A verdict that never sees the successor proves nothing about it.

#[test]
fn aid002_transition_binds_the_presented_successor_document() {
    let v = vector();
    let (owner1, succession1, doc1) = identity(&v, &v.seed_hex, &v.succession_seed_hex);
    let (_, succession2, doc2) =
        identity(&v, &v.successor_seed_hex, &v.successor_succession_seed_hex);
    let at = "2026-07-09T00:00:00Z".to_owned();

    let valid =
        EpochTransition::sign(&succession1, doc1.id.clone(), doc2.id.clone(), at.clone()).unwrap();
    valid
        .verify_succession(&doc1, &doc2)
        .expect("the complete, honest triple is accepted");

    // A THIRD identity, never named by the transition, is presented instead.
    let (_, _, doc3) = identity(&v, &v.successor_succession_seed_hex, &v.successor_seed_hex);
    assert_ne!(doc3.id, doc2.id);
    assert!(
        valid.verify_succession(&doc1, &doc3).is_err(),
        "a successor document the transition never named must be refused"
    );

    // The successor is presented, but it is itself invalid.
    let mut broken = doc2.clone();
    broken.revocations.push('x');
    assert!(
        valid.verify_succession(&doc1, &broken).is_err(),
        "an unverifiable successor document must be refused"
    );

    // Correctly re-signed successor whose content key is malformed: the
    // successor must go through the SAME strict verdict as the predecessor.
    let (owner2, _, _) = identity(&v, &v.successor_seed_hex, &v.successor_succession_seed_hex);
    let mut malformed = doc2.clone();
    malformed.keys.content = "not-a-key".to_owned();
    resign(&mut malformed, &owner2.root_sign);
    let tr_to_malformed = EpochTransition::sign(
        &succession1,
        doc1.id.clone(),
        malformed.id.clone(),
        at.clone(),
    )
    .unwrap();
    assert!(
        tr_to_malformed
            .verify_succession(&doc1, &malformed)
            .is_err(),
        "a signed but malformed successor must be refused"
    );

    // Same DID before and after: an epoch that goes nowhere.
    let self_transition =
        EpochTransition::sign(&succession1, doc1.id.clone(), doc1.id.clone(), at.clone()).unwrap();
    assert!(
        self_transition.verify_succession(&doc1, &doc1).is_err(),
        "prev_did == next_did is not a succession"
    );

    // Malformed next_did — rejected before any document is even presented.
    let malformed_next = EpochTransition::sign(
        &succession1,
        doc1.id.clone(),
        "did:aithos:zzz".to_owned(),
        at.clone(),
    )
    .unwrap();
    assert!(malformed_next.verify_declaration(&doc1).is_err());
    let not_a_did =
        EpochTransition::sign(&succession1, doc1.id.clone(), "nope".to_owned(), at.clone())
            .unwrap();
    assert!(not_a_did.verify_declaration(&doc1).is_err());

    // Signed by ANOTHER identity's succession key.
    let foreign =
        EpochTransition::sign(&succession2, doc1.id.clone(), doc2.id.clone(), at.clone()).unwrap();
    assert!(foreign.verify_succession(&doc1, &doc2).is_err());

    // Root-signed, both honestly (#root) and while claiming #succession.
    for fragment in ["#root", "#succession"] {
        let rogue = EpochTransition::sign_with(
            &owner1.root_sign,
            fragment,
            doc1.id.clone(),
            doc2.id.clone(),
            at.clone(),
        )
        .unwrap();
        assert!(rogue.verify_succession(&doc1, &doc2).is_err());
    }

    // prev_did naming a document other than the one supplied.
    let wrong_prev =
        EpochTransition::sign(&succession1, doc2.id.clone(), doc2.id.clone(), at.clone()).unwrap();
    assert!(wrong_prev.verify_succession(&doc1, &doc2).is_err());

    // Unsupported version / algorithm, each correctly re-signed.
    for mutate in [
        |t: &mut EpochTransition| t.version = "9.9.9".to_owned(),
        |t: &mut EpochTransition| t.signature.alg = "secp256k1".to_owned(),
    ] {
        let mut tr = valid.clone();
        mutate(&mut tr);
        tr.signature.value = String::new();
        let bytes = jcs::canonical_bytes(&tr).unwrap();
        tr.signature.value = hex::encode(succession1.sign(&bytes).to_bytes());
        assert!(tr.verify_succession(&doc1, &doc2).is_err());
    }
    assert_eq!(valid.version, DID_VERSION);
}
