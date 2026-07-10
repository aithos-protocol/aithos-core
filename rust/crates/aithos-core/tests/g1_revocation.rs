//! Conformance vector G1 — the revoke gamma entry and its authority /
//! forward-only verdicts (spec 06.4). Independent Python generator.

use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::gamma::{verify_owner_entry, Entry};
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::revocation::{chain_revoked_at, revocations, Revocation};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct G1 {
    seed_hex: String,
    revoked_mandate_id: String,
    entry_jcs: String,
    forward_only: Value,
}

fn vector() -> G1 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/g1-revocation.json"
    )))
    .expect("valid vector json")
}

fn owner(v: &G1) -> OwnerKeys {
    let seed: [u8; 32] = hex::decode(&v.seed_hex).unwrap().try_into().unwrap();
    OwnerKeys::genesis(&MasterSeed::from_bytes(seed))
}

fn did_doc(v: &G1) -> DidDocument {
    let succession = succession_from_entropy([9u8; 32]);
    DidDocument::build(
        &owner(v),
        &succession.verifying_key(),
        vec![],
        String::new(),
    )
    .unwrap()
}

#[test]
fn owner_revoke_entry_verifies() {
    let v = vector();
    let entry: Entry = serde_json::from_str(&v.entry_jcs).unwrap();
    assert_eq!(entry.kind, "revoke");
    assert_eq!(entry.target.as_deref(), Some(v.revoked_mandate_id.as_str()));
    // Byte-for-byte JCS + owner content signature.
    assert_eq!(aithos_core::jcs::canonicalize(&entry).unwrap(), v.entry_jcs);
    verify_owner_entry(&entry, &did_doc(&v)).unwrap();
}

#[test]
fn forward_only_verdicts_match_python() {
    let v = vector();
    let entry: Entry = serde_json::from_str(&v.entry_jcs).unwrap();
    let revs: Vec<Revocation> = revocations(std::slice::from_ref(&entry));
    assert_eq!(revs.len(), 1);

    // A dummy chain whose only mandate is the revoked one.
    let chain = vec![dummy_mandate(&v.revoked_mandate_id)];
    for (at, verdict) in v.forward_only.as_object().unwrap() {
        if at == "revoked_at" {
            continue;
        }
        let got = chain_revoked_at(&chain, &revs, at);
        match verdict.as_str().unwrap() {
            "valid" => got.unwrap_or_else(|e| panic!("{at} should be valid: {e}")),
            "MandateRevoked" => assert!(
                matches!(got, Err(Error::MandateRevoked(_))),
                "{at} should be revoked"
            ),
            other => panic!("unknown verdict {other}"),
        }
    }
}

/// A minimal Mandate carrying just the id the revocation targets.
fn dummy_mandate(id: &str) -> aithos_core::mandate::Mandate {
    serde_json::from_value(serde_json::json!({
        "aithos-mandate-core": "1.0.0-draft.1",
        "id": id,
        "subject": "did:aithos:x",
        "parent": null,
        "issued_by": "did:aithos:x#root",
        "grantee": {"id": "a", "label": "a", "pubkey": "z", "kex_pubkey": "z"},
        "perimeter": [],
        "constraints": {},
        "not_before": "2026-07-01T00:00:00Z",
        "not_after": "2026-08-01T00:00:00Z",
        "issued_at": "2026-07-01T00:00:00Z",
        "nonce": "00",
        "signature": {"alg": "ed25519", "key": "#root", "value": ""}
    }))
    .unwrap()
}
