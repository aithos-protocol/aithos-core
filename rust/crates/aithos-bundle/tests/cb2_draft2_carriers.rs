//! CB2 K1-C draft2 carrier vector consumer.
//!
//! Existing generic JCS, SHA-256, BLAKE3, multibase and Ed25519 primitives
//! reproduce the independent Python oracle. Typed Core carrier validation is
//! active; draft2 Bundle assembly/cold verification remains the explicit gate.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::{jcs, wire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-draft2-carriers.json"
));
const VECTOR_SHA256: &str = "2e75e9af30ba0207bd01a6f347cac1a263f816a7ae0fb3d583f75beabef2badc";

const BUNDLE_COEXISTENCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-version-coexistence.json"
));
const CONNECTOR_CATALOG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-connector-catalog.json"
));
const DELEGATED_COUNTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-delegated-counts.json"
));
const GAMMA_V2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-gamma-v2-replay.json"
));
const FACTS_ACTION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-action-inference.json"
));
const FACTS_MUTATION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
));
const FACTS_READ: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-read.json"
));
const FACTS_STRUCTURAL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-structural.json"
));
const OPERATION_PROJECTION: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));
const OPERATION_RECEIPTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-receipts.json"
));
const SESSION_PROOF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-session-proof.json"
));

const CORE_CARRIERS_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../aithos-core/src/carriers.rs"
));
const BUNDLE_MANIFEST_SOURCE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/manifest.rs"));
const BUNDLE_PUBLICATION_SOURCE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/publication.rs"));

fn vector() -> Value {
    serde_json::from_slice(VECTOR_BYTES).expect("CB2 draft2 carrier vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_text(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn object_keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("fixture object")
        .keys()
        .cloned()
        .collect()
}

fn verify_signature(key: &str, message: &[u8], signature: &str) {
    let key_bytes = wire::multibase_to_ed25519_pub(key).expect("fixture Ed25519 multibase");
    let key = VerifyingKey::from_bytes(&key_bytes).expect("fixture Ed25519 public key");
    let signature_bytes: [u8; 64] = hex::decode(signature)
        .expect("fixture signature hex")
        .try_into()
        .expect("64-byte fixture signature");
    key.verify(message, &Signature::from_bytes(&signature_bytes))
        .expect("fixture signature verifies");
}

fn without_top_member(value: &Value, member: &str) -> Value {
    let mut unsigned = value.clone();
    unsigned
        .as_object_mut()
        .expect("signed fixture object")
        .remove(member);
    unsigned
}

fn with_empty_signature_value(value: &Value) -> Value {
    let mut unsigned = value.clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    unsigned
}

fn evidence_item<'a>(evidence: &'a Value, kind: &str) -> &'a Value {
    evidence["items"]
        .as_array()
        .expect("evidence items")
        .iter()
        .find(|item| item["kind"].as_str() == Some(kind))
        .unwrap_or_else(|| panic!("missing evidence item {kind}"))
}

#[test]
fn cb2_k1c_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES), VECTOR_SHA256);
}

#[test]
fn cb2_k1c_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("cb2-bundle-version-coexistence.json", BUNDLE_COEXISTENCE),
        ("cb2-connector-catalog.json", CONNECTOR_CATALOG),
        ("cb2-delegated-counts.json", DELEGATED_COUNTS),
        ("cb2-gamma-v2-replay.json", GAMMA_V2),
        ("cb2-operation-facts-action-inference.json", FACTS_ACTION),
        ("cb2-operation-facts-mutation.json", FACTS_MUTATION),
        ("cb2-operation-facts-read.json", FACTS_READ),
        ("cb2-operation-facts-structural.json", FACTS_STRUCTURAL),
        ("cb2-operation-projection.json", OPERATION_PROJECTION),
        ("cb2-operation-receipts.json", OPERATION_RECEIPTS),
        ("cb2-session-proof.json", SESSION_PROOF),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            vector["historical_vector_sha256"][name]
                .as_str()
                .expect("historical vector hash"),
            "{name}"
        );
    }
    assert_eq!(
        vector["inventory"]["draft1_bytes_are_not_reinterpreted"],
        true
    );
}

#[test]
fn cb2_k1c_changeset_reference_path_and_file_bytes_preexisting_green() {
    let vector = vector();
    let positive = &vector["positive"];
    let changeset = &positive["changeset"];
    let document = &changeset["document"];
    let document_jcs = jcs::canonicalize(document).expect("changeset JCS");
    assert_eq!(
        document_jcs,
        changeset["document_jcs"]
            .as_str()
            .expect("oracle changeset JCS")
    );
    assert_eq!(
        commitment(
            vector["domains"]["changeset"]
                .as_str()
                .expect("changeset domain"),
            document_jcs.as_bytes()
        ),
        changeset["reference"]["digest"]
    );
    assert_eq!(
        changeset["path"],
        format!(
            "changesets/{}.json",
            changeset["reference"]["digest"]
                .as_str()
                .expect("changeset digest")
                .strip_prefix("sha256:")
                .expect("prefixed changeset digest")
        )
    );
    assert_eq!(
        sha256_hex(document_jcs.as_bytes()),
        changeset["file_sha256"]
    );

    assert_eq!(
        object_keys(document),
        [
            "aithos-changeset-core",
            "height",
            "predecessors",
            "operations",
            "changes",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    let operations = document["operations"].as_array().expect("operations");
    assert_eq!(operations.len(), 5);
    assert_eq!(
        operations,
        positive["contained_operations"].as_array().unwrap()
    );
    for (facts, reference) in positive["facts_documents"]
        .as_array()
        .expect("operation facts documents")
        .iter()
        .zip(
            positive["facts_refs"]
                .as_array()
                .expect("operation facts references"),
        )
    {
        assert_eq!(
            object_keys(facts),
            ["aithos-operation-facts-core", "kind", "facts"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        let facts_jcs = jcs::canonicalize(facts).expect("operation facts JCS");
        assert_eq!(
            reference["digest"],
            commitment(
                vector["domains"]["operation_facts"]
                    .as_str()
                    .expect("operation-facts domain"),
                facts_jcs.as_bytes()
            )
        );
    }
    let state_facts = positive["state_facts"].as_array().expect("state facts");
    assert_eq!(state_facts.len(), 2);
    for (index, state) in state_facts.iter().enumerate() {
        assert_eq!(
            object_keys(state),
            ["aithos-state-fact-core", "objects"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        let state_jcs = jcs::canonicalize(state).expect("state-fact JCS");
        assert_eq!(
            positive["facts_documents"][index]["facts"]["after"]["state_ref"]["digest"],
            commitment(
                vector["domains"]["state_fact"]
                    .as_str()
                    .expect("state-fact domain"),
                state_jcs.as_bytes()
            )
        );
    }
    assert_eq!(
        positive["facts_documents"][3]["facts"]["source_head"],
        positive["gamma_query"]["source_head"]
    );
    assert_eq!(
        positive["facts_documents"][3]["facts"]["request_digest"],
        positive["gamma_query"]["request_digest"]
    );
    for (projection, reference) in positive["operation_projections"]
        .as_array()
        .expect("operation projections")
        .iter()
        .zip(operations)
    {
        let projection_jcs = jcs::canonicalize(projection).expect("operation projection JCS");
        assert_eq!(reference["occurrence"], projection["occurrence"]);
        assert_eq!(
            reference["commitment"],
            commitment(
                vector["domains"]["operation"]
                    .as_str()
                    .expect("operation domain"),
                projection_jcs.as_bytes()
            )
        );
    }

    let store_after = vector["context"]["store_after"]
        .as_object()
        .expect("Store after map");
    let store_before = vector["context"]["store_before"]
        .as_object()
        .expect("Store before map");
    let expected_key_commitments = store_after
        .iter()
        .filter(|(key, value)| store_before.get(*key) != Some(*value))
        .map(|(key, _)| {
            commitment(
                vector["domains"]["state_key"]
                    .as_str()
                    .expect("state-key domain"),
                key.as_bytes(),
            )
        })
        .collect::<BTreeSet<_>>();
    let expected_last_writers = vector["context"]["change_causes"]
        .as_object()
        .expect("change causes")
        .iter()
        .map(|(key, reference)| {
            (
                commitment(
                    vector["domains"]["state_key"]
                        .as_str()
                        .expect("state-key domain"),
                    key.as_bytes(),
                ),
                reference.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let changes = document["changes"].as_array().expect("changes");
    assert_eq!(changes.len(), expected_key_commitments.len());
    assert_eq!(
        changes
            .iter()
            .map(|change| change["key_commitment"]
                .as_str()
                .expect("key commitment")
                .to_owned())
            .collect::<BTreeSet<_>>(),
        expected_key_commitments
    );
    for change in changes {
        assert_eq!(change["before"]["state"], "absent");
        assert_eq!(change["after"]["state"], "present");
        assert!(operations.contains(&change["operation_ref"]));
        assert_eq!(
            change["operation_ref"],
            expected_last_writers[change["key_commitment"]
                .as_str()
                .expect("change key commitment")]
        );
    }
    assert_eq!(
        vector["inventory"]["changeset_excludes_candidate_and_carrier_sidecars"],
        true
    );
}

#[test]
fn cb2_k1c_evidence_signatures_sorting_and_d7_preexisting_green() {
    let vector = vector();
    let positive = &vector["positive"];
    let evidence = &positive["evidence"];
    let document = &evidence["document"];
    let document_jcs = jcs::canonicalize(document).expect("evidence JCS");
    assert_eq!(
        document_jcs,
        evidence["document_jcs"]
            .as_str()
            .expect("oracle evidence JCS")
    );
    assert_eq!(
        commitment(
            vector["domains"]["evidence"]
                .as_str()
                .expect("evidence domain"),
            document_jcs.as_bytes()
        ),
        evidence["reference"]["digest"]
    );
    assert_eq!(
        evidence["path"],
        format!(
            "evidence/{}.json",
            evidence["reference"]["digest"]
                .as_str()
                .expect("evidence digest")
                .strip_prefix("sha256:")
                .expect("prefixed evidence digest")
        )
    );
    assert_eq!(sha256_hex(document_jcs.as_bytes()), evidence["file_sha256"]);

    let items = document["items"].as_array().expect("evidence items");
    assert_eq!(items.len(), 5);
    let item_jcs = items
        .iter()
        .map(|item| jcs::canonicalize(item).expect("evidence item JCS"))
        .collect::<Vec<_>>();
    let mut sorted = item_jcs.clone();
    sorted.sort();
    assert_eq!(item_jcs, sorted);
    assert_eq!(
        item_jcs.iter().collect::<BTreeSet<_>>().len(),
        item_jcs.len()
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item["kind"].as_str().expect("evidence kind"))
            .collect::<BTreeSet<_>>(),
        [
            "authorship",
            "catalog",
            "presentation",
            "receipt",
            "session"
        ]
        .into_iter()
        .collect()
    );

    let context = &vector["context"];
    let grantee_key = context["grantee_key"].as_str().expect("grantee key");
    let authority = &positive["authority_certificate"];
    let authority_document = &authority["document"];
    let authority_preimage = jcs::canonicalize(&with_empty_signature_value(authority_document))
        .expect("authority-certificate preimage");
    let root_key = context["subject"]
        .as_str()
        .expect("operation subject")
        .strip_prefix("did:aithos:")
        .expect("DID root key");
    verify_signature(
        root_key,
        authority_preimage.as_bytes(),
        authority_document["signature"]["value"]
            .as_str()
            .expect("authority signature"),
    );
    assert_eq!(
        authority["digest"],
        sha256_text(jcs::canonicalize(authority_document).unwrap().as_bytes())
    );
    assert_eq!(
        authority_document["constraints"]["catalog_pins"][0]["catalog_digest"],
        context["catalog_ref"]["catalog_digest"]
    );

    let authorship = &evidence_item(document, "authorship")["document"];
    let authorship_preimage =
        jcs::canonicalize(&without_top_member(authorship, "sig")).expect("authorship preimage");
    verify_signature(
        grantee_key,
        authorship_preimage.as_bytes(),
        authorship["sig"].as_str().expect("authorship signature"),
    );
    assert_eq!(
        authorship["content_hash"],
        sha256_text(
            context["public_body"]
                .as_str()
                .expect("public body")
                .as_bytes()
        )
    );

    let session = evidence_item(document, "session");
    let certificate = &session["certificate"];
    let certificate_preimage = jcs::canonicalize(&with_empty_signature_value(certificate))
        .expect("session-certificate preimage");
    verify_signature(
        grantee_key,
        certificate_preimage.as_bytes(),
        certificate["signature"]["value"]
            .as_str()
            .expect("certificate signature"),
    );
    assert_eq!(
        sha256_text(jcs::canonicalize(certificate).unwrap().as_bytes()),
        context["session_certificate_digest"]
    );
    assert_eq!(
        positive["operation_projections"][1]["authority"]["session"],
        serde_json::json!({
            "key": context["session_key"],
            "certificate_digest": context["session_certificate_digest"],
        })
    );
    let proof = &session["proof"];
    let proof_preimage =
        jcs::canonicalize(&without_top_member(proof, "sig")).expect("session-proof preimage");
    verify_signature(
        context["session_key"].as_str().expect("session key"),
        proof_preimage.as_bytes(),
        proof["sig"].as_str().expect("session proof signature"),
    );

    let receipt = &evidence_item(document, "receipt")["document"];
    let receipt_preimage =
        jcs::canonicalize(&without_top_member(receipt, "sig")).expect("receipt preimage");
    verify_signature(
        context["receipt_key"].as_str().expect("receipt key"),
        receipt_preimage.as_bytes(),
        receipt["sig"].as_str().expect("receipt signature"),
    );

    let catalog_item = evidence_item(document, "catalog");
    let catalog = &catalog_item["catalog"];
    let catalog_preimage =
        jcs::canonicalize(&with_empty_signature_value(catalog)).expect("catalog preimage");
    verify_signature(
        context["catalog_key"].as_str().expect("catalog key"),
        catalog_preimage.as_bytes(),
        catalog["signature"]["value"]
            .as_str()
            .expect("catalog signature"),
    );
    let approval = &catalog_item["approval"];
    let approval_preimage =
        jcs::canonicalize(&with_empty_signature_value(approval)).expect("approval preimage");
    verify_signature(
        context["content_key"].as_str().expect("content key"),
        approval_preimage.as_bytes(),
        approval["signature"]["value"]
            .as_str()
            .expect("approval signature"),
    );
    assert_eq!(
        approval["catalog_digest"],
        sha256_text(jcs::canonicalize(catalog).unwrap().as_bytes())
    );

    let presentation = &evidence_item(document, "presentation")["document"];
    let presentation_preimage =
        jcs::canonicalize(&without_top_member(presentation, "sig")).expect("presentation preimage");
    verify_signature(
        grantee_key,
        presentation_preimage.as_bytes(),
        presentation["sig"]
            .as_str()
            .expect("presentation signature"),
    );
    assert_eq!(presentation["entries"], positive["gamma_query"]["result"]);
    assert_eq!(
        presentation["request_digest"],
        commitment(
            vector["domains"]["gamma_request"]
                .as_str()
                .expect("Gamma request domain"),
            positive["gamma_query"]["canonical"]
                .as_str()
                .expect("canonical Gamma query")
                .as_bytes()
        )
    );

    let counts = &positive["delegated_counts_fixture"];
    let payload = hex::decode(
        counts["payload_hex"]
            .as_str()
            .expect("delegated-counts payload"),
    )
    .expect("delegated-counts payload hex");
    let mut leaf_preimage = b"aithos-core/v1/delegated-counts-leaf\0".to_vec();
    leaf_preimage.extend(payload);
    assert_eq!(
        hex::encode(blake3::hash(&leaf_preimage).as_bytes()),
        counts["leaf_hex"]
    );
    assert_eq!(document["delegated_counts"], counts["reference"]);
    assert_eq!(vector["inventory"]["evidence_grants_no_authority"], true);
}

#[test]
fn cb2_k1c_signed_manifest_negative_boundary_and_api_inventory_preliminary() {
    let vector = vector();
    let positive = &vector["positive"];
    let candidate = &positive["candidate"];
    let manifest = &candidate["manifest"];
    let manifest_jcs = jcs::canonicalize(manifest).expect("manifest JCS");
    assert_eq!(
        manifest_jcs,
        positive["manifest_jcs"]
            .as_str()
            .expect("oracle manifest JCS")
    );
    let preimage = jcs::canonicalize(&with_empty_signature_value(manifest))
        .expect("manifest signature preimage");
    assert_eq!(
        preimage,
        positive["manifest_preimage_jcs"]
            .as_str()
            .expect("oracle manifest preimage")
    );
    verify_signature(
        vector["context"]["grantee_key"]
            .as_str()
            .expect("manifest grantee key"),
        preimage.as_bytes(),
        manifest["signature"]["value"]
            .as_str()
            .expect("manifest signature"),
    );

    let publication = &positive["publication"];
    let projection_jcs =
        jcs::canonicalize(&publication["projection"]).expect("publication projection JCS");
    assert_eq!(
        projection_jcs,
        publication["projection_jcs"]
            .as_str()
            .expect("oracle publication projection")
    );
    assert_eq!(
        publication["operation_ref"]["commitment"],
        commitment(
            vector["domains"]["operation"]
                .as_str()
                .expect("operation domain"),
            projection_jcs.as_bytes()
        )
    );
    assert_eq!(manifest["operation_ref"], publication["operation_ref"]);
    assert_eq!(
        manifest["changeset_ref"],
        positive["changeset"]["reference"]
    );
    assert_eq!(manifest["evidence_ref"], positive["evidence"]["reference"]);

    let sidecars = candidate["sidecars"].as_object().expect("sidecar map");
    assert_eq!(sidecars.len(), 2);
    for (path, payload) in sidecars {
        assert_eq!(
            manifest["files"][path],
            sha256_hex(payload.as_str().expect("sidecar JCS").as_bytes()),
            "{path}"
        );
    }
    for (path, payload) in vector["context"]["store_after"]
        .as_object()
        .expect("Store after")
    {
        assert_eq!(
            manifest["files"][path],
            sha256_hex(payload.as_str().expect("stored bytes").as_bytes()),
            "{path}"
        );
    }

    let negatives = vector["negative_cases"].as_array().expect("negative cases");
    assert_eq!(negatives.len(), 37);
    assert_eq!(
        negatives
            .iter()
            .filter(|case| case["must_fail"] == "InvalidOperation")
            .count(),
        32
    );
    assert_eq!(
        negatives
            .iter()
            .filter(|case| case["must_fail"] == "InvalidDidDocument")
            .count(),
        5
    );
    assert_eq!(
        negatives
            .iter()
            .map(|case| case["id"].as_str().expect("negative id"))
            .collect::<BTreeSet<_>>()
            .len(),
        negatives.len()
    );
    assert!(negatives.iter().all(|case| case["candidate"].is_object()));

    for api in [
        "pub fn derive_changeset",
        "pub fn verify_k1c_carriers",
        "pub struct K1cVerificationContext",
        "pub enum EvidenceItem",
    ] {
        assert!(
            CORE_CARRIERS_SOURCE.contains(api),
            "typed K1-C Core API remains incomplete: {api}"
        );
    }
    for member in ["operation_ref", "changeset_ref", "evidence_ref"] {
        assert!(
            BUNDLE_MANIFEST_SOURCE.contains(&format!("pub {member}:")),
            "draft2 manifest member is missing: {member}"
        );
    }
    for api in [
        "pub fn assemble_draft2_candidate",
        "pub fn verify_draft2_candidate",
        "pub fn export_keyless",
        "pub fn cold_verify",
    ] {
        assert!(
            BUNDLE_PUBLICATION_SOURCE.contains(api),
            "draft2 Bundle API is missing: {api}"
        );
    }
}
