//! Conformance vector F3 — heartbeat window and freshness anchor
//! (spec 07.5, 07.7). Instants and verdicts computed independently with
//! Python datetime, cross-checking the pure Rust calendar math.

use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::gamma::{check_anchor, heartbeat_ok, Entry};
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
use ed25519_dalek::SigningKey;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct F3 {
    seed_hex: String,
    agent_sk_hex: String,
    beacon1_jcs: String,
    beacon2_jcs: String,
    heartbeat: Value,
    verdicts_after_beacon1: Value,
    verdict_after_beacon2: Value,
    freshness: String,
    anchor_verdicts: Value,
}

fn vector() -> F3 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/f3-gamma-liveness.json"
    )))
    .expect("valid vector json")
}

struct Fixture {
    beacon1: Entry,
    beacon2: Entry,
    mandate: Mandate,
    doc: DidDocument,
    agent: SigningKey,
    v: F3,
}

fn fixture() -> Fixture {
    let v = vector();
    let seed: [u8; 32] = hex::decode(&v.seed_hex).unwrap().try_into().unwrap();
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(seed));
    let succession = succession_from_entropy([9u8; 32]);
    let doc =
        DidDocument::build(&owner, &succession.verifying_key(), vec![], String::new()).unwrap();
    let agent = SigningKey::from_bytes(&hex::decode(&v.agent_sk_hex).unwrap().try_into().unwrap());
    let mandate = Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: "mandate_000000000000000000000000HB".into(),
            subject: doc.id.clone(),
            grantee_id: "urn:aithos:agent:agent".into(),
            grantee_label: "agent".into(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![PerimeterEntry::parse("act.x.gmail.*").unwrap()],
            constraints: serde_json::json!({"heartbeat": v.heartbeat}),
            not_before: "2026-07-01T00:00:00Z".into(),
            not_after: "2027-07-01T00:00:00Z".into(),
            issued_at: "2026-07-01T00:00:00Z".into(),
            nonce: "00".repeat(16),
        },
    )
    .unwrap();
    Fixture {
        beacon1: serde_json::from_str(&v.beacon1_jcs).unwrap(),
        beacon2: serde_json::from_str(&v.beacon2_jcs).unwrap(),
        mandate,
        doc,
        agent,
        v,
    }
}

#[test]
fn heartbeat_verdicts_match_python() {
    let f = fixture();
    let log = vec![f.beacon1.clone()];
    for (at, verdict) in f.v.verdicts_after_beacon1.as_object().unwrap() {
        let got = heartbeat_ok(&log, &f.mandate, at, &f.doc);
        match verdict.as_str().unwrap() {
            "valid" => got.unwrap_or_else(|e| panic!("{at} should be valid: {e}")),
            "GammaHeartbeatStale" => {
                assert!(
                    matches!(got, Err(Error::GammaHeartbeatStale(_))),
                    "{at} should be stale"
                );
            }
            other => panic!("unknown verdict {other}"),
        }
    }
}

#[test]
fn the_owners_return_resumes() {
    let f = fixture();
    let log = vec![f.beacon1.clone(), f.beacon2.clone()];
    for (at, verdict) in f.v.verdict_after_beacon2.as_object().unwrap() {
        assert_eq!(verdict.as_str().unwrap(), "valid");
        heartbeat_ok(&log, &f.mandate, at, &f.doc).unwrap();
    }
}

#[test]
fn a_forged_beacon_never_counts() {
    let f = fixture();
    // The head agent forges a "heartbeat" carrying the owner's #content
    // fragment but its own signature: it never verifies, so it never counts.
    let mut forged = f.beacon2.clone();
    forged.at = "2026-08-05T00:00:00Z".into();
    forged.signature.value = String::new();
    let mut unsigned = forged.clone();
    unsigned.signature.value = String::new();
    let bytes = aithos_core::jcs::canonical_bytes(&unsigned).unwrap();
    use ed25519_dalek::Signer;
    forged.signature.value = hex::encode(f.agent.sign(&bytes).to_bytes());

    let log = vec![f.beacon1.clone(), forged];
    // Day 34 after beacon1: stale despite the forged beacon of day 35.
    assert!(matches!(
        heartbeat_ok(&log, &f.mandate, "2026-08-04T00:00:01Z", &f.doc),
        Err(Error::GammaHeartbeatStale(_))
    ));
}

#[test]
fn anchor_verdicts_match_python() {
    let f = fixture();
    let log = vec![f.beacon1.clone()];
    let anchor = f.beacon1.chain_hash().unwrap();
    for (at, verdict) in f.v.anchor_verdicts.as_object().unwrap() {
        let got = check_anchor(&log, &anchor, &f.v.freshness, at);
        match verdict.as_str().unwrap() {
            "valid" => got.unwrap_or_else(|e| panic!("{at} should be valid: {e}")),
            "GammaStaleAnchor" => {
                assert!(
                    matches!(got, Err(Error::GammaStaleAnchor(_))),
                    "{at} should be stale"
                );
            }
            other => panic!("unknown verdict {other}"),
        }
    }
    // An anchor that is not on the log fails closed.
    assert!(matches!(
        check_anchor(
            &log,
            "sha256:deadbeef",
            &f.v.freshness,
            "2026-07-01T00:00:00Z"
        ),
        Err(Error::GammaStaleAnchor(_))
    ));
}
