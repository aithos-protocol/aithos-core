use aithos_core::carriers::{
    verify_k1c_carriers, verify_normal_edition_actor, K1cActor, K1cCarrierEnvelope,
    K1cVerificationContext,
};
use aithos_core::Error;
use serde_json::Value;
use std::collections::BTreeMap;

const VECTOR_BYTES: &[u8] = include_bytes!("../../../../vectors/cb2-draft2-carriers.json");

fn vector() -> Value {
    serde_json::from_slice(VECTOR_BYTES).expect("CB2 K1-C vector parses")
}

fn string_map(value: &Value) -> BTreeMap<String, Vec<u8>> {
    value
        .as_object()
        .expect("Store map")
        .iter()
        .map(|(path, bytes)| {
            (
                path.clone(),
                bytes.as_str().expect("stored bytes").as_bytes().to_vec(),
            )
        })
        .collect()
}

fn value_map(value: &Value) -> BTreeMap<String, Value> {
    value
        .as_object()
        .expect("value map")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn envelope(candidate: &Value) -> K1cCarrierEnvelope {
    let manifest = &candidate["manifest"];
    K1cCarrierEnvelope {
        changeset: candidate["changeset"].clone(),
        evidence: candidate["evidence"].clone(),
        operation_ref: manifest["operation_ref"].clone(),
        changeset_ref: manifest["changeset_ref"].clone(),
        evidence_ref: manifest["evidence_ref"].clone(),
        files: manifest["files"]
            .as_object()
            .expect("manifest files")
            .iter()
            .map(|(path, digest)| {
                (
                    path.clone(),
                    digest.as_str().expect("file digest").to_owned(),
                )
            })
            .collect(),
        sidecars: candidate["sidecars"]
            .as_object()
            .expect("candidate sidecars")
            .iter()
            .map(|(path, bytes)| {
                (
                    path.clone(),
                    bytes.as_str().expect("sidecar bytes").as_bytes().to_vec(),
                )
            })
            .collect(),
    }
}

fn context(vector: &Value) -> K1cVerificationContext {
    let positive = &vector["positive"];
    let context = &vector["context"];
    let required_receipts = positive["candidate"]["evidence"]["items"]
        .as_array()
        .expect("evidence items")
        .iter()
        .filter(|item| item["kind"] == "receipt")
        .map(|item| item["document"]["operation_ref"].clone())
        .collect();
    K1cVerificationContext {
        subject: context["subject"].as_str().expect("subject").to_owned(),
        actor: K1cActor::Grantee {
            key: context["grantee_key"]
                .as_str()
                .expect("actor key")
                .to_owned(),
            authority_chain: vec![context["authority_ref"].clone()],
        },
        height: context["height"].as_u64().expect("height"),
        predecessors: context["predecessors"]
            .as_array()
            .expect("predecessors")
            .clone(),
        parent_store: string_map(&context["store_before"]),
        candidate_store: string_map(&context["store_after"]),
        change_causes: value_map(&context["change_causes"]),
        contained_operations: positive["contained_operations"]
            .as_array()
            .expect("contained operations")
            .clone(),
        operation_projections: positive["operation_projections"]
            .as_array()
            .expect("operation projections")
            .clone(),
        operation_facts: positive["facts_documents"]
            .as_array()
            .expect("operation facts")
            .clone(),
        authority_documents: vec![positive["authority_certificate"]["document"].clone()],
        publication_projection: positive["publication"]["projection"].clone(),
        publication_facts: positive["publication"]["facts"].clone(),
        publication_ref: context["publication_ref"].clone(),
        publication_at: context["publication_at"]
            .as_str()
            .expect("publication at")
            .to_owned(),
        required_receipts,
        delegated_counts: context["delegated_counts"].clone(),
        gamma_source_head: context["source_head"]
            .as_str()
            .expect("source head")
            .to_owned(),
        gamma_request_digest: context["request_digest"]
            .as_str()
            .expect("request digest")
            .to_owned(),
        gamma_result: context["query_result"]
            .as_array()
            .expect("query result")
            .clone(),
        content_key: context["content_key"]
            .as_str()
            .expect("content key")
            .to_owned(),
        receipt_key: context["receipt_key"]
            .as_str()
            .expect("receipt key")
            .to_owned(),
    }
}

#[test]
fn cb11_k1c_positive_carriers_receive_one_pure_core_verdict() {
    let vector = vector();
    let verified = verify_k1c_carriers(
        &envelope(&vector["positive"]["candidate"]),
        &context(&vector),
    )
    .expect("coherent K1-C carriers verify");

    assert_eq!(verified.height(), 2);
    assert_eq!(verified.change_count(), 5);
    assert_eq!(verified.evidence_count(), 5);
}

#[test]
fn cb11_k1c_all_32_semantic_defects_are_typed_invalid_operation() {
    let vector = vector();
    let context = context(&vector);
    let cases = vector["negative_cases"]
        .as_array()
        .expect("negative cases")
        .iter()
        .filter(|case| case["must_fail"] == "InvalidOperation")
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 32);

    for case in cases {
        let id = case["id"].as_str().expect("negative id");
        let error = verify_k1c_carriers(&envelope(&case["candidate"]), &context).expect_err(id);
        assert!(
            matches!(error, Error::InvalidOperation(_)),
            "{id}: {error:?}"
        );
    }
}

#[test]
fn cb11_normal_edition_has_one_actor_and_at_most_one_complete_chain() {
    let vector = vector();
    let grantee_context = context(&vector);
    let authorities = grantee_context
        .operation_projections
        .iter()
        .map(|projection| projection["authority"].clone())
        .collect::<Vec<_>>();
    verify_normal_edition_actor(&grantee_context.actor, &authorities)
        .expect("one grantee and one complete chain");

    let owner = K1cActor::Owner {
        key: vector["context"]["content_key"]
            .as_str()
            .expect("owner public key")
            .to_owned(),
    };
    verify_normal_edition_actor(&owner, &[serde_json::json!({"actor": "owner"})])
        .expect("one local owner actor");

    let mut two_partial_chains = grantee_context.actor.clone();
    let K1cActor::Grantee {
        authority_chain, ..
    } = &mut two_partial_chains
    else {
        unreachable!("fixture is delegated");
    };
    authority_chain.push(serde_json::json!({
        "id": "mandate_01K00000000000000000000099",
        "certificate_digest":
            "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    }));
    assert!(matches!(
        verify_normal_edition_actor(&two_partial_chains, &authorities),
        Err(Error::InvalidOperation(_))
    ));
}
