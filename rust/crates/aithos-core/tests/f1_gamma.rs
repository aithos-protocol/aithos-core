//! Conformance vector F1 — gamma chain, envelope, signatures (spec 07.1-07.3).
//! Expected values generated independently (Python blake3+PyNaCl+hashlib);
//! deterministic Ed25519 + injected nonce make everything byte-for-byte.

use aithos_core::derive::node_key;
use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::gamma::{
    body_hint, delegated_entry, head, open_body, owner_entry, seal_body, verify_delegated_entry,
    verify_links, verify_owner_entry, BodyEnc, Entry, EntrySpec, Kind,
};
use aithos_core::ids::Sid;
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
use aithos_core::path::{NodePath, Zone};
use ed25519_dalek::SigningKey;
use serde::Deserialize;

#[derive(Deserialize)]
struct F1 {
    seed_hex: String,
    agent_sk_hex: String,
    zone_dk_hex: String,
    folder_sid: String,
    section_sid: String,
    target: String,
    body_nonce_hex: String,
    key_version: u64,
    node_key_hex: String,
    hint_hex: String,
    mandate_jcs: String,
    entry1_jcs: String,
    entry1_hash: String,
    entry2_jcs: String,
    entry2_hash: String,
    entry3_jcs: String,
    entry3_hash: String,
    gamma_head: String,
}

fn vector() -> F1 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/f1-gamma-chain.json"
    )))
    .expect("valid vector json")
}

fn hex32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}

fn owner(v: &F1) -> OwnerKeys {
    OwnerKeys::genesis(&MasterSeed::from_bytes(hex32(&v.seed_hex)))
}

fn did_doc(v: &F1) -> DidDocument {
    let owner = owner(v);
    let succession = succession_from_entropy([9u8; 32]);
    DidDocument::build(&owner, &succession.verifying_key(), vec![], String::new()).unwrap()
}

#[test]
fn node_key_and_hint_match() {
    let v = vector();
    let path = NodePath::section(
        Zone::Circle,
        vec![Sid::parse(&v.folder_sid).unwrap()],
        Sid::parse(&v.section_sid).unwrap(),
    );
    assert_eq!(path.to_string(), v.target);
    let nk = node_key(&hex32(&v.zone_dk_hex), &path);
    assert_eq!(hex::encode(nk), v.node_key_hex);
    assert_eq!(body_hint(&nk), v.hint_hex);
}

#[test]
fn entries_rebuild_byte_for_byte() {
    let v = vector();
    let owner = owner(&v);
    let did = did_doc(&v).id;
    let nk = hex32(&v.node_key_hex);
    let nonce: [u8; 24] = hex::decode(&v.body_nonce_hex).unwrap().try_into().unwrap();

    let body = seal_body(
        &nk,
        &did,
        &v.target,
        v.key_version,
        &serde_json::json!({"note": "hello"}),
        &nonce,
    )
    .unwrap();
    let e1 = owner_entry(
        EntrySpec {
            id: "gamma_00000000000000000000000001".into(),
            prev: String::new(),
            prevs: None,
            at: "2026-07-01T00:00:00Z".into(),
            kind: Kind::SectionAdd,
            target: None,
            payload: None,
            body_enc: Some(body),
        },
        &owner.content_sign,
    )
    .unwrap();
    assert_eq!(jcs::canonicalize(&e1).unwrap(), v.entry1_jcs);
    assert_eq!(e1.chain_hash().unwrap(), v.entry1_hash);

    let e2 = owner_entry(
        EntrySpec {
            id: "gamma_00000000000000000000000002".into(),
            prev: v.entry1_hash.clone(),
            prevs: None,
            at: "2026-07-01T00:05:00Z".into(),
            kind: Kind::Heartbeat,
            target: None,
            payload: Some(serde_json::json!({"seq": 1})),
            body_enc: None,
        },
        &owner.content_sign,
    )
    .unwrap();
    assert_eq!(jcs::canonicalize(&e2).unwrap(), v.entry2_jcs);
    assert_eq!(e2.chain_hash().unwrap(), v.entry2_hash);

    let mandate: Mandate = serde_json::from_str(&v.mandate_jcs).unwrap();
    let agent = SigningKey::from_bytes(&hex32(&v.agent_sk_hex));
    let e3 = delegated_entry(
        EntrySpec {
            id: "gamma_00000000000000000000000003".into(),
            prev: v.entry2_hash.clone(),
            prevs: None,
            at: "2026-07-02T00:00:00Z".into(),
            kind: Kind::Action,
            target: Some("x.gmail".into()),
            payload: Some(serde_json::json!({
                "action": "reply",
                "args_hash": format!("sha256:{}", aithos_core::gamma::sha256_hex(b"args")),
            })),
            body_enc: None,
        },
        vec![mandate.id.clone()],
        &agent,
    )
    .unwrap();
    assert_eq!(jcs::canonicalize(&e3).unwrap(), v.entry3_jcs);
    assert_eq!(e3.chain_hash().unwrap(), v.entry3_hash);
}

#[test]
fn mandate_rebuilds_and_chain_verifies() {
    let v = vector();
    let owner = owner(&v);
    let doc = did_doc(&v);
    let agent = SigningKey::from_bytes(&hex32(&v.agent_sk_hex));
    let expected: Mandate = serde_json::from_str(&v.mandate_jcs).unwrap();

    let rebuilt = Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: expected.id.clone(),
            subject: doc.id.clone(),
            grantee_id: "urn:aithos:agent:agent".into(),
            grantee_label: "agent".into(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![PerimeterEntry::parse("act.x.gmail.*").unwrap()],
            constraints: serde_json::json!({"max_actions": 3}),
            not_before: "2026-07-01T00:00:00Z".into(),
            not_after: "2026-08-01T00:00:00Z".into(),
            issued_at: "2026-07-01T00:00:00Z".into(),
            nonce: "00".repeat(16),
        },
    )
    .unwrap();
    assert_eq!(jcs::canonicalize(&rebuilt).unwrap(), v.mandate_jcs);

    let entries: Vec<Entry> = [&v.entry1_jcs, &v.entry2_jcs, &v.entry3_jcs]
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    verify_links(&entries).unwrap();
    assert_eq!(head(&entries).unwrap(), v.gamma_head);
    verify_owner_entry(&entries[0], &doc).unwrap();
    verify_owner_entry(&entries[1], &doc).unwrap();
    verify_delegated_entry(&entries[2], std::slice::from_ref(&expected), &doc).unwrap();
}

#[test]
fn bodies_open_only_under_the_right_key() {
    let v = vector();
    let doc = did_doc(&v);
    let entries: Vec<Entry> = [&v.entry1_jcs]
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let enc: &BodyEnc = entries[0].body_enc.as_ref().unwrap();

    let body = open_body(&hex32(&v.node_key_hex), &doc.id, &v.target, 1, enc).unwrap();
    assert_eq!(body.target, v.target);
    assert_eq!(body.payload, serde_json::json!({"note": "hello"}));

    let wrong = [7u8; 32];
    assert!(matches!(
        open_body(&wrong, &doc.id, &v.target, 1, enc),
        Err(Error::SealRejected(_))
    ));
}

#[test]
fn tampering_breaks_the_chain() {
    let v = vector();
    let mut entries: Vec<Entry> = [&v.entry1_jcs, &v.entry2_jcs, &v.entry3_jcs]
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();

    // Alter the middle entry: every downstream prev breaks.
    entries[1].payload = Some(serde_json::json!({"seq": 999}));
    assert!(matches!(
        verify_links(&entries),
        Err(Error::InvalidGammaChain(_))
    ));

    // Append with a wrong predecessor: rejected.
    let mut entries: Vec<Entry> = [&v.entry1_jcs, &v.entry2_jcs, &v.entry3_jcs]
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let mut rogue = entries[2].clone();
    rogue.id = "gamma_00000000000000000000000009".into();
    rogue.prev = v.entry1_hash.clone(); // not the head
    entries.push(rogue);
    assert!(matches!(
        verify_links(&entries),
        Err(Error::InvalidGammaChain(_))
    ));
}
