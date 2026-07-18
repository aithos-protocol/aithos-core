//! CB2 max_children versioning conformance consumer.
//!
//! The Python vector is the oracle. Byte/signature/Gamma replay tests are
//! green before production changes; the version-dispatch matrix and draft.2
//! builder tests stay deliberately red until the implementation supports the
//! two mandate profiles.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::did::{DidDocument, DidKeys, SignatureBlock, DID_VERSION};
use aithos_core::error::Error;
use aithos_core::gamma::{
    count_children, head, sha256_hex, verify_delegated_entry, verify_links, verify_owner_entry,
    Entry,
};
use aithos_core::jcs;
use aithos_core::keys::ed2x;
use aithos_core::mandate::{verify_chain, Mandate, MandateSpec};
use aithos_core::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::Value;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

const VECTOR_RAW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-max-children-versioning.json"
));
const EPLUS_RAW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/eplus-attenuation.json"
));
const VECTOR_SHA256: &str = "b0f49be51b9ed2097234ad161f11a1b0af546e6ec4f8a99e1cc43c83eef5b1ec";
const EPLUS_SHA256: &str = "9822d9da417487740b50efc1a760883addf8fffcaa0fa2008e029ab473d1db8c";
const DRAFT1: &str = "1.0.0-draft.1";
const DRAFT2: &str = "1.0.0-draft.2";

fn vector() -> Value {
    serde_json::from_str(VECTOR_RAW).expect("CB2 max_children vector parses")
}

fn b32(value: &Value) -> [u8; 32] {
    hex::decode(value.as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}

fn signing_key(v: &Value, field: &str) -> SigningKey {
    SigningKey::from_bytes(&b32(&v[field]))
}

fn certificate_jcs<'a>(v: &'a Value, name: &str) -> &'a str {
    v["certificates"][name]["jcs"].as_str().unwrap()
}

fn certificate(v: &Value, name: &str) -> Mandate {
    serde_json::from_str(certificate_jcs(v, name)).unwrap()
}

fn named_chain(v: &Value, names: &Value) -> Vec<Mandate> {
    names
        .as_array()
        .unwrap()
        .iter()
        .map(|name| certificate(v, name.as_str().unwrap()))
        .collect()
}

fn entries(section: &Value) -> Vec<Entry> {
    section["grant_entries_jcs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| serde_json::from_str(line.as_str().unwrap()).unwrap())
        .collect()
}

fn assert_gamma_hashes(section: &Value, parsed: &[Entry]) {
    let expected = section["grant_entry_hashes"].as_array().unwrap();
    assert_eq!(parsed.len(), expected.len());
    verify_links(parsed).expect("Gamma links verify");
    for (entry, hash) in parsed.iter().zip(expected) {
        assert_eq!(entry.chain_hash().unwrap(), hash.as_str().unwrap());
    }
    assert_eq!(
        head(parsed).unwrap(),
        section["gamma_head"].as_str().unwrap()
    );
}

/// The CB2 fixture deliberately uses independent fixed Ed25519 seeds instead
/// of OwnerKeys::genesis. Build the public DID document directly from those
/// seeds and sign its existing wire shape with the fixture root key.
fn did_document(v: &Value) -> DidDocument {
    let root = signing_key(v, "root_sk_hex");
    let content = signing_key(v, "content_sk_hex");
    let succession = SigningKey::from_bytes(&[9u8; 32]);
    let owner_kex = StaticSecret::from([7u8; 32]);
    let owner_kex_pub = XPublicKey::from(&owner_kex);
    let root_pub = root.verifying_key().to_bytes();

    let mut doc = DidDocument {
        version: DID_VERSION.to_owned(),
        bundle: vec![],
        id: wire::did_aithos(&root_pub),
        keys: DidKeys {
            content: wire::ed25519_pub_to_multibase(&content.verifying_key().to_bytes()),
            kex: wire::x25519_pub_to_multibase(&owner_kex_pub.to_bytes()),
            root: wire::ed25519_pub_to_multibase(&root_pub),
            succession: wire::ed25519_pub_to_multibase(&succession.verifying_key().to_bytes()),
        },
        revocations: String::new(),
        signature: SignatureBlock {
            alg: "ed25519".to_owned(),
            key: "#root".to_owned(),
            value: String::new(),
        },
    };
    doc.signature.value = hex::encode(root.sign(&jcs::canonical_bytes(&doc).unwrap()).to_bytes());
    assert_eq!(doc.id, v["did"].as_str().unwrap());
    doc.verify().expect("fixture DID document verifies");
    doc
}

fn verify_certificate_signature(mandate: &Mandate, key: &VerifyingKey) {
    let signature_bytes: [u8; 64] = hex::decode(&mandate.signature.value)
        .unwrap()
        .try_into()
        .unwrap();
    let signature = Signature::from_bytes(&signature_bytes);
    let mut unsigned = mandate.clone();
    unsigned.signature.value.clear();
    key.verify(&jcs::canonical_bytes(&unsigned).unwrap(), &signature)
        .expect("certificate signature verifies");
}

#[test]
fn cb2_historical_hashes_and_all_certificate_bytes_are_green() {
    let v = vector();
    assert_eq!(sha256_hex(VECTOR_RAW.as_bytes()), VECTOR_SHA256);
    assert_eq!(sha256_hex(EPLUS_RAW.as_bytes()), EPLUS_SHA256);
    assert_eq!(
        v["historical_eplus"]["sha256"].as_str().unwrap(),
        EPLUS_SHA256
    );

    let eplus: Value = serde_json::from_str(EPLUS_RAW).unwrap();
    let frozen = eplus["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case["family"] == "max_children" && case["case"] == "drop tolerated — per-level width"
        })
        .unwrap();
    assert_eq!(frozen["parent"], serde_json::json!({"max_children": 4}));
    assert_eq!(frozen["child"], serde_json::json!({}));
    assert_eq!(frozen["expected"], "valid");

    let records = v["certificates"].as_object().unwrap();
    assert_eq!(records.len(), 15);
    let mandates: BTreeMap<String, Mandate> = records
        .values()
        .map(|record| {
            let mandate: Mandate = serde_json::from_str(record["jcs"].as_str().unwrap()).unwrap();
            (mandate.id.clone(), mandate)
        })
        .collect();
    let root = signing_key(&v, "root_sk_hex").verifying_key();

    for (name, record) in records {
        let expected_jcs = record["jcs"].as_str().unwrap();
        let mandate: Mandate = serde_json::from_str(expected_jcs).unwrap();
        assert_eq!(
            jcs::canonicalize(&mandate).unwrap(),
            expected_jcs,
            "{name}: JCS"
        );
        assert_eq!(
            sha256_hex(expected_jcs.as_bytes()),
            record["sha256"].as_str().unwrap(),
            "{name}: JCS hash"
        );
        assert_eq!(
            mandate.signature.value,
            record["signature_hex"].as_str().unwrap(),
            "{name}: signature bytes"
        );
        assert_eq!(mandate.signature.alg, "ed25519", "{name}: algorithm");
        assert_eq!(mandate.subject, v["did"].as_str().unwrap());

        let verifier = match &mandate.parent {
            None => {
                assert_eq!(mandate.signature.key, "#root");
                root
            }
            Some(parent_id) => {
                let parent = mandates.get(parent_id).unwrap();
                assert_eq!(mandate.issued_by, parent.grantee.pubkey);
                assert_eq!(mandate.signature.key, parent.grantee.pubkey);
                parent.grantee_pub().unwrap()
            }
        };
        verify_certificate_signature(&mandate, &verifier);
        assert_eq!(
            mandate.grantee.kex_pubkey,
            wire::x25519_pub_to_multibase(&ed2x(&mandate.grantee_pub().unwrap()).to_bytes()),
            "{name}: Ed25519/X25519 binding"
        );
    }
}

fn assert_link_cases(owner: &str, case_ids: &[&str]) {
    let v = vector();
    let doc = did_document(&v);
    let at = v["verify_at"].as_str().unwrap();
    let mut mismatches = Vec::new();

    for requested_id in case_ids {
        let case = v["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == *requested_id)
            .unwrap_or_else(|| panic!("missing CB2 case {requested_id}"));
        let id = case["id"].as_str().unwrap();
        let parent = certificate(&v, case["parent"].as_str().unwrap());
        let child = certificate(&v, case["child"].as_str().unwrap());
        let accepted = match verify_chain(&[parent, child], &doc, at) {
            Ok(()) => true,
            Err(Error::InvalidMandate(_)) => false,
            Err(_) => panic!("{id}: rejection must use Error::InvalidMandate"),
        };
        let expected = match case["expected"].as_str().unwrap() {
            "valid" => true,
            "InvalidMandate" => false,
            other => panic!("{id}: unknown vector verdict {other}"),
        };
        if accepted != expected {
            mismatches.push(id);
        }
    }

    assert!(
        mismatches.is_empty(),
        "{owner} verify_chain mismatches: {mismatches:?}"
    );
}

/// PREEXISTING-GREEN: historical draft.1 and the numeric draft.2
/// equal/reduced/wider law already match the independent oracle.
#[test]
fn cb2_preexisting_green_historical_and_numeric_attenuation_cases() {
    assert_link_cases(
        "PREEXISTING-GREEN",
        &[
            "draft1_omission_historical",
            "draft2_equal",
            "draft2_reduced",
            "draft2_wider",
        ],
    );
}

/// CB5 RED: draft.2 omission is widening even when the child is a leaf.
#[test]
fn cb2_cb5_red_draft2_omission_cases() {
    assert_link_cases(
        "CB5",
        &["draft2_omission_delegating", "draft2_omission_leaf"],
    );
}

/// CB3 RED: a delegation chain must be version-homogeneous.
#[test]
fn cb2_cb3_red_mixed_version_links() {
    assert_link_cases("CB3", &["mixed_draft1_to_draft2", "mixed_draft2_to_draft1"]);
}

/// CB3 RED while the only builder profile is draft.1.
#[test]
fn cb2_cb3_red_root_builder_emits_the_draft2_vector_bytes() {
    let v = vector();
    let expected_name = "draft2_parent";
    let expected_jcs = certificate_jcs(&v, expected_name);
    let expected: Mandate = serde_json::from_str(expected_jcs).unwrap();
    let root = signing_key(&v, "root_sk_hex");
    let agent = signing_key(&v, "agent_sk_hex");

    let built = Mandate::build_root(
        &root,
        &MandateSpec {
            id: expected.id.clone(),
            subject: expected.subject.clone(),
            grantee_id: expected.grantee.id.clone(),
            grantee_label: expected.grantee.label.clone(),
            grantee_pub: &agent.verifying_key(),
            perimeter: expected.parsed_perimeter().unwrap(),
            constraints: expected.constraints.clone(),
            not_before: expected.not_before.clone(),
            not_after: expected.not_after.clone(),
            issued_at: expected.issued_at.clone(),
            nonce: expected.nonce.clone(),
        },
    )
    .unwrap();

    let mut mismatches = Vec::new();
    if built.version != expected.version {
        mismatches.push("version");
    }
    if jcs::canonicalize(&built).unwrap() != expected_jcs {
        mismatches.push("JCS");
    }
    assert!(
        mismatches.is_empty(),
        "draft.2 root builder mismatches: {mismatches:?}"
    );
}

/// CB3 RED while the sub-mandate builder still emits draft.1.
#[test]
fn cb2_cb3_red_sub_builder_emits_the_draft2_vector_bytes() {
    let v = vector();
    let parent = certificate(&v, "draft2_parent");
    let expected_jcs = certificate_jcs(&v, "draft2_reduced_leaf");
    let expected: Mandate = serde_json::from_str(expected_jcs).unwrap();
    let agent = signing_key(&v, "agent_sk_hex");
    let helper = signing_key(&v, "helper_sk_hex");

    let built = Mandate::build_sub(
        &parent,
        &agent,
        &MandateSpec {
            id: expected.id.clone(),
            subject: expected.subject.clone(),
            grantee_id: expected.grantee.id.clone(),
            grantee_label: expected.grantee.label.clone(),
            grantee_pub: &helper.verifying_key(),
            perimeter: expected.parsed_perimeter().unwrap(),
            constraints: expected.constraints.clone(),
            not_before: expected.not_before.clone(),
            not_after: expected.not_after.clone(),
            issued_at: expected.issued_at.clone(),
            nonce: expected.nonce.clone(),
        },
    )
    .unwrap();

    let mut mismatches = Vec::new();
    if built.version != expected.version {
        mismatches.push("version");
    }
    if jcs::canonicalize(&built).unwrap() != expected_jcs {
        mismatches.push("JCS");
    }
    assert!(
        mismatches.is_empty(),
        "draft.2 sub builder mismatches: {mismatches:?}"
    );
}

#[test]
fn cb2_direct_children_gamma_count_and_signatures_are_green() {
    let v = vector();
    let doc = did_document(&v);
    let section = &v["direct_children_only"];
    let parent = certificate(&v, section["parent_chain"][0].as_str().unwrap());
    let child = certificate(
        &v,
        section["child_chain"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_str()
            .unwrap(),
    );
    let grandchildren: Vec<Mandate> = section["grandchild_chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chain| {
            let names = chain.as_array().unwrap();
            certificate(&v, names.last().unwrap().as_str().unwrap())
        })
        .collect();

    verify_chain(
        std::slice::from_ref(&parent),
        &doc,
        v["verify_at"].as_str().unwrap(),
    )
    .expect("direct parent verifies");
    verify_chain(
        &[parent.clone(), child.clone()],
        &doc,
        v["verify_at"].as_str().unwrap(),
    )
    .expect("direct child verifies");
    for grandchild in &grandchildren {
        verify_chain(
            &[parent.clone(), child.clone(), grandchild.clone()],
            &doc,
            v["verify_at"].as_str().unwrap(),
        )
        .expect("grandchild chain verifies");
    }

    let parsed = entries(section);
    assert_eq!(parsed.len(), 5);
    assert_gamma_hashes(section, &parsed);
    verify_owner_entry(&parsed[0], &doc).expect("owner root grant signature");
    verify_delegated_entry(&parsed[1], std::slice::from_ref(&parent), &doc)
        .expect("direct-child grant signature");
    for entry in &parsed[2..] {
        verify_delegated_entry(entry, &[parent.clone(), child.clone()], &doc)
            .expect("grandchild grant signature");
    }

    assert_eq!(parsed[0].target.as_deref(), Some(parent.id.as_str()));
    assert_eq!(parsed[1].target.as_deref(), Some(child.id.as_str()));
    for (entry, grandchild) in parsed[2..].iter().zip(&grandchildren) {
        assert_eq!(entry.target.as_deref(), Some(grandchild.id.as_str()));
        assert_eq!(entry.authorized_by.as_deref(), Some(child.id.as_str()));
    }

    assert_eq!(parent.constraints["max_children"].as_u64(), Some(3));
    assert_eq!(child.constraints["max_children"].as_u64(), Some(3));
    assert_eq!(count_children(&parsed, &parent.id), 1);
    assert_eq!(count_children(&parsed, &child.id), 3);
    assert_eq!(
        section["direct_children_tallies"][&parent.id].as_u64(),
        Some(1)
    );
    assert_eq!(
        section["direct_children_tallies"][&child.id].as_u64(),
        Some(3)
    );

    let parent_progress: Vec<usize> = (0..3)
        .map(|index| count_children(&parsed[..3 + index], &parent.id))
        .collect();
    let child_progress: Vec<usize> = (0..3)
        .map(|index| count_children(&parsed[..3 + index], &child.id))
        .collect();
    assert_eq!(parent_progress, vec![1, 1, 1]);
    assert_eq!(child_progress, vec![1, 2, 3]);
    assert_eq!(
        serde_json::to_value(parent_progress).unwrap(),
        section["grandparent_tally_after_each_grandchild"]
    );
    assert_eq!(
        serde_json::to_value(child_progress).unwrap(),
        section["child_tally_after_each_grandchild"]
    );
    assert_eq!(
        section["expected"]["grandchildren_counted_against_parent"].as_u64(),
        Some(0)
    );
    assert_eq!(
        section["expected"]["all_grants_within_declared_caps"].as_bool(),
        Some(true)
    );
}

#[test]
fn cb2_migration_reissue_structure_and_gamma_are_green() {
    let v = vector();
    let doc = did_document(&v);
    let section = &v["migration"];
    let legacy = named_chain(&v, &section["legacy_chain"]);
    let reissued = named_chain(&v, &section["reissued_chain"]);
    let at = v["verify_at"].as_str().unwrap();

    verify_chain(&legacy, &doc, at).expect("historical draft.1 chain verifies");
    verify_chain(&reissued, &doc, at).expect("reissued draft.2 chain verifies");
    assert!(legacy.iter().all(|mandate| mandate.version == DRAFT1));
    assert!(reissued.iter().all(|mandate| mandate.version == DRAFT2));
    assert_eq!(legacy.len(), reissued.len());
    assert_eq!(legacy[0].grantee.pubkey, reissued[0].grantee.pubkey);
    assert_eq!(legacy[1].grantee.pubkey, reissued[1].grantee.pubkey);
    assert_eq!(reissued[1].parent.as_deref(), Some(reissued[0].id.as_str()));
    assert_ne!(legacy[0].signature.value, reissued[0].signature.value);
    assert_ne!(legacy[1].signature.value, reissued[1].signature.value);

    let legacy_ids: BTreeSet<&str> = legacy.iter().map(|mandate| mandate.id.as_str()).collect();
    let reissued_ids: BTreeSet<&str> = reissued.iter().map(|mandate| mandate.id.as_str()).collect();
    assert!(legacy_ids.is_disjoint(&reissued_ids));
    assert_eq!(section["same_authority_keys"].as_bool(), Some(true));
    assert_eq!(section["fresh_certificate_ids"].as_bool(), Some(true));
    assert_eq!(
        section["expected"]["historical_certificates_rewritten"].as_bool(),
        Some(false)
    );

    let parsed = entries(section);
    assert_eq!(parsed.len(), 2);
    assert_gamma_hashes(section, &parsed);
    verify_owner_entry(&parsed[0], &doc).expect("reissued root grant signature");
    verify_delegated_entry(&parsed[1], std::slice::from_ref(&reissued[0]), &doc)
        .expect("reissued child grant signature");
    assert_eq!(parsed[0].target.as_deref(), Some(reissued[0].id.as_str()));
    assert_eq!(parsed[1].target.as_deref(), Some(reissued[1].id.as_str()));
    assert_eq!(
        parsed[1].authorized_by.as_deref(),
        Some(reissued[0].id.as_str())
    );
    assert_eq!(count_children(&parsed, &reissued[0].id), 1);
}
