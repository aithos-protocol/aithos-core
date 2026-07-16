//! Conformance vector E+ — typed constraint attenuation per family at a
//! delegation link (spec 05.3 rule 3; M0 decision (c) 2026-07-16). Every
//! matrix verdict and the signed chain's bytes were generated independently
//! (Python sets/integers + blake3 + PyNaCl + base58).

use aithos_core::constraints::constraints_attenuate;
use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{verify_chain, Mandate, MandateSpec, PerimeterEntry};
use ed25519_dalek::SigningKey;
use serde_json::Value;

fn vector() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/eplus-attenuation.json"
    )))
    .expect("vector eplus-attenuation.json parses")
}

fn b32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}

/// The matrix cases avoid active_windows on purpose (F+ pins the interval
/// arithmetic), so any in-window pair works here.
const NB: &str = "2026-07-02T00:00:00Z";
const NA: &str = "2026-07-08T00:00:00Z";

#[test]
fn eplus_matrix_verdicts_match_python() {
    let v = vector();
    for case in v["matrix"].as_array().expect("matrix") {
        let family = case["family"].as_str().unwrap();
        let name = case["case"].as_str().unwrap();
        let verdict = constraints_attenuate(&case["parent"], &case["child"], NB, NA);
        match case["expected"].as_str().unwrap() {
            "valid" => {
                verdict.unwrap_or_else(|e| panic!("[{family}] {name}: expected valid, got {e}"))
            }
            "InvalidMandate" => {
                let err = verdict
                    .err()
                    .unwrap_or_else(|| panic!("[{family}] {name}: expected a rejection"));
                assert!(
                    matches!(err, Error::InvalidMandate(_)),
                    "[{family}] {name}: wrong error variant: {err}"
                );
            }
            other => panic!("unknown expected verdict {other}"),
        }
    }
}

#[test]
fn eplus_signed_chain_bytes_match_python() {
    let v = vector();
    let sc = &v["signed_chain"];
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(b32(sc["seed_hex"]
        .as_str()
        .unwrap())));
    let agent = SigningKey::from_bytes(&b32(sc["agent_sk_hex"].as_str().unwrap()));
    let helper = SigningKey::from_bytes(&b32(sc["helper_sk_hex"].as_str().unwrap()));
    let did = aithos_core::wire::did_aithos(&owner.root_sign.verifying_key().to_bytes());
    assert_eq!(did, sc["did"].as_str().unwrap(), "DID vs Python");

    let parent_json: Value = serde_json::from_str(sc["parent_jcs"].as_str().unwrap()).unwrap();
    let parent = Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: parent_json["id"].as_str().unwrap().to_owned(),
            subject: did.clone(),
            grantee_id: "urn:aithos:agent:agent".into(),
            grantee_label: "agent".into(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![
                PerimeterEntry::parse("act.x.gmail.*").unwrap(),
                PerimeterEntry::parse("issue#depth=1").unwrap(),
            ],
            constraints: parent_json["constraints"].clone(),
            not_before: parent_json["not_before"].as_str().unwrap().to_owned(),
            not_after: parent_json["not_after"].as_str().unwrap().to_owned(),
            issued_at: parent_json["issued_at"].as_str().unwrap().to_owned(),
            nonce: parent_json["nonce"].as_str().unwrap().to_owned(),
        },
    )
    .unwrap();
    assert_eq!(
        jcs::canonicalize(&parent).unwrap(),
        sc["parent_jcs"].as_str().unwrap(),
        "parent JCS vs Python"
    );
    assert_eq!(
        parent.signature.value,
        sc["parent_signature_hex"].as_str().unwrap(),
        "parent signature vs Python"
    );

    for (jcs_key, sig_key, cid) in [
        (
            "child_ok_jcs",
            "child_ok_signature_hex",
            "mandate_00000000000000000000000EPA",
        ),
        (
            "child_bad_jcs",
            "child_bad_signature_hex",
            "mandate_00000000000000000000000EPB",
        ),
    ] {
        let cj: Value = serde_json::from_str(sc[jcs_key].as_str().unwrap()).unwrap();
        let child = Mandate::build_sub(
            &parent,
            &agent,
            &MandateSpec {
                id: cid.to_owned(),
                subject: did.clone(),
                grantee_id: "urn:aithos:agent:helper".into(),
                grantee_label: "helper".into(),
                grantee_pub: &helper.verifying_key(),
                perimeter: vec![PerimeterEntry::parse("act.x.gmail.reply").unwrap()],
                constraints: cj["constraints"].clone(),
                not_before: cj["not_before"].as_str().unwrap().to_owned(),
                not_after: cj["not_after"].as_str().unwrap().to_owned(),
                issued_at: cj["issued_at"].as_str().unwrap().to_owned(),
                nonce: cj["nonce"].as_str().unwrap().to_owned(),
            },
        )
        .unwrap();
        assert_eq!(
            jcs::canonicalize(&child).unwrap(),
            sc[jcs_key].as_str().unwrap(),
            "{jcs_key} vs Python"
        );
        assert_eq!(
            child.signature.value,
            sc[sig_key].as_str().unwrap(),
            "{sig_key} vs Python"
        );
    }
}

#[test]
fn eplus_verifier_closes_the_hole() {
    let v = vector();
    let sc = &v["signed_chain"];
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(b32(sc["seed_hex"]
        .as_str()
        .unwrap())));
    let succ = succession_from_entropy(b32(sc["succession_entropy_hex"].as_str().unwrap()));
    let doc = DidDocument::build(
        &owner,
        &succ.verifying_key(),
        vec!["file://local".to_owned()],
        "gamma/gamma.jsonl".to_owned(),
    )
    .unwrap();
    let at = sc["at"].as_str().unwrap();
    let parent: Mandate = serde_json::from_str(sc["parent_jcs"].as_str().unwrap()).unwrap();
    let ok: Mandate = serde_json::from_str(sc["child_ok_jcs"].as_str().unwrap()).unwrap();
    let bad: Mandate = serde_json::from_str(sc["child_bad_jcs"].as_str().unwrap()).unwrap();

    verify_chain(std::slice::from_ref(&parent), &doc, at).expect("parent alone verifies");
    verify_chain(&[parent.clone(), ok], &doc, at).expect("tightened child verifies");
    // The E+ point: a cap-raising child is now rejected at the link.
    let refused = verify_chain(&[parent, bad], &doc, at).unwrap_err();
    assert!(
        matches!(&refused, Error::InvalidMandate(m) if m.contains("max_actions")),
        "the refusal names the widened family: {refused}"
    );
}
