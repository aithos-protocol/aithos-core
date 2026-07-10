//! Conformance vector E1 — root mandate wire format (spec 04.1).
//! The canonical JCS and its root signature were generated independently
//! (Python); deterministic Ed25519 makes both match byte-for-byte.

use aithos_core::did::DidDocument;
use aithos_core::ids::Sid;
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{verify_chain, Mandate, MandateSpec, PerimeterEntry, Verb};
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;
use serde::Deserialize;

#[derive(Deserialize)]
struct E1 {
    seed_hex: String,
    agent_sk_hex: String,
    mandate_id: String,
    dir_sids: Vec<String>,
    nonce: String,
    not_before: String,
    not_after: String,
    mandate_jcs: String,
    signature_hex: String,
}

fn vector() -> E1 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/e1-mandate.json"
    )))
    .expect("vector e1-mandate.json parses")
}

fn b32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}

fn built(v: &E1) -> (Mandate, OwnerKeys) {
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(b32(&v.seed_hex)));
    let agent = SigningKey::from_bytes(&b32(&v.agent_sk_hex));
    let dir: Vec<Sid> = v.dir_sids.iter().map(|s| Sid::parse(s).unwrap()).collect();
    let m = Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: v.mandate_id.clone(),
            subject: aithos_core::wire::did_aithos(&owner.root_sign.verifying_key().to_bytes()),
            constraints: MandateSpec::no_constraints(),
            grantee_id: "urn:aithos:agent:agent".to_owned(),
            grantee_label: "agent".to_owned(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![
                PerimeterEntry::Ethos {
                    verb: Verb::Read,
                    zone: Zone::Circle,
                    dir,
                    tag: Some("toto".to_owned()),
                },
                PerimeterEntry::Issue { depth: 1 },
            ],
            not_before: v.not_before.clone(),
            not_after: v.not_after.clone(),
            issued_at: v.not_before.clone(),
            nonce: v.nonce.clone(),
        },
    )
    .unwrap();
    (m, owner)
}

#[test]
fn e1_canonical_jcs_and_signature() {
    let v = vector();
    let (m, _) = built(&v);
    assert_eq!(
        jcs::canonicalize(&m).unwrap(),
        v.mandate_jcs,
        "JCS vs Python"
    );
    assert_eq!(
        m.signature.value, v.signature_hex,
        "root signature vs Python"
    );
}

#[test]
fn e1_chain_verifies_and_kex_is_checked() {
    let v = vector();
    let (m, owner) = built(&v);
    let succession = succession_from_entropy([9u8; 32]);
    let doc = DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec!["file://local".to_owned()],
        "gamma/gamma.jsonl".to_owned(),
    )
    .unwrap();
    // Inside window.
    verify_chain(std::slice::from_ref(&m), &doc, "2026-07-02T00:00:00Z").expect("valid at day 1");
    // Past expiry.
    assert!(verify_chain(std::slice::from_ref(&m), &doc, "2026-07-09T00:00:00Z").is_err());
    // Tampered kex binding → rejected.
    let mut bad = m;
    bad.grantee.kex_pubkey = "z6LSbogus".to_owned();
    assert!(verify_chain(&[bad], &doc, "2026-07-02T00:00:00Z").is_err());
}
