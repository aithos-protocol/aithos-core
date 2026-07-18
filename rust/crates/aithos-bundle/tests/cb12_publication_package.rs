use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::grants::{GenericGrantRequest, GrantSelector};
use aithos_bundle::publication::{
    assemble_draft2_candidate, cold_verify, cold_verify_for_cas, export_keyless, import_keyless,
    package_with_objects, verify_draft2_candidate_value, PublicationMode,
};
use aithos_bundle::session::LocalSession;
use aithos_bundle::{FsStore, MemStore};
use aithos_core::carriers::{
    derive_changeset, K1cActor, K1cVerificationContext, CHANGESET_PROFILE,
};
use aithos_core::did::DidDocument;
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::Verb;
use aithos_core::path::Zone;
use aithos_core::wire;
use aithos_core::Error;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
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

fn grantee_signer(vector: &Value) -> SigningKey {
    let seed = hex::decode(
        vector["deterministic_private_seed_hex"]["grantee"]
            .as_str()
            .expect("private seed"),
    )
    .expect("seed hex");
    SigningKey::from_bytes(&seed.try_into().expect("32-byte signing seed"))
}

fn commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn sha256_text(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn operation_ref(projection: &Value) -> Value {
    serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": projection["occurrence"],
        "commitment": commitment(
            "aithos-core/v1/operation-commitment",
            &jcs::canonical_bytes(projection).expect("projection JCS")
        ),
    })
}

fn facts_ref(facts: &Value) -> Value {
    serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "digest": commitment(
            "aithos-core/v1/operation-facts",
            &jcs::canonical_bytes(facts).expect("facts JCS")
        ),
    })
}

fn owner_cold_fixture() -> (
    OwnerKeys,
    K1cVerificationContext,
    Value,
    BTreeMap<String, Vec<u8>>,
) {
    let owner = OwnerKeys::genesis(&MasterSeed::from_slice(&[0x91; 32]).expect("owner seed"));
    let succession = SigningKey::from_bytes(&[0x92; 32]);
    let did = DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec!["file://local".into()],
        "gamma/2026-07.jsonl".into(),
    )
    .expect("DID");
    let did_bytes = jcs::canonical_bytes(&did).expect("DID JCS");
    let parent = aithos_bundle::manifest::Manifest::build(
        &owner.root_sign,
        1,
        String::new(),
        "2026-07-18T15:00:00Z".into(),
        BTreeMap::from([("did.json".into(), hex::encode(Sha256::digest(&did_bytes)))]),
        BTreeMap::new(),
        BTreeMap::new(),
        String::new(),
        String::new(),
    )
    .expect("parent manifest");
    let parent_bytes = jcs::canonical_bytes(&parent).expect("parent JCS");
    let predecessor = format!("sha256:{}", parent.chain_hash().expect("parent hash"));
    let sid = "01J00000000000000000000091";
    let body_path = format!("public/sections/{sid}.md");
    let body = b"# Cold owner\n".to_vec();
    let gamma = b"".to_vec();
    let parent_store = BTreeMap::from([
        ("did.json".into(), did_bytes),
        ("gamma/2026-07.jsonl".into(), gamma.clone()),
        ("manifests/1.json".into(), parent_bytes),
    ]);
    let mut candidate_store = parent_store.clone();
    candidate_store.insert(body_path.clone(), body.clone());

    let mutation_facts = serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "kind": "mutation",
        "facts": {
            "domain": "ethos",
            "zone": "public",
            "dir": [],
            "sid": sid,
            "verb": "create",
            "before": {"state": "absent"},
            "after": {
                "state": "present",
                "state_ref": {
                    "aithos-state-fact-core": "1.0.0-draft.1",
                    "digest":
                        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                }
            }
        }
    });
    let mutation_projection = serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": "op_01K00000000000000000000091",
        "subject": did.id,
        "at": "2026-07-18T15:01:00Z",
        "authority": {"actor": "owner"},
        "history_heads": [predecessor],
        "operation": {
            "kind": "mutation",
            "facts_ref": facts_ref(&mutation_facts),
        }
    });
    let mutation_ref = operation_ref(&mutation_projection);
    let root_key = wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes());
    let mut context = K1cVerificationContext {
        subject: did.id.clone(),
        actor: K1cActor::Owner {
            key: root_key.clone(),
        },
        height: 2,
        predecessors: vec![Value::String(predecessor.clone())],
        parent_store,
        candidate_store,
        change_causes: BTreeMap::from([(body_path, mutation_ref.clone())]),
        contained_operations: vec![mutation_ref.clone()],
        operation_projections: vec![mutation_projection],
        operation_facts: vec![mutation_facts],
        authority_documents: Vec::new(),
        publication_projection: Value::Null,
        publication_facts: Value::Null,
        publication_ref: Value::Null,
        publication_at: "2026-07-18T15:02:00Z".into(),
        required_receipts: Vec::new(),
        delegated_counts: serde_json::json!({
            "aithos-delegated-counts-core": "1.0.0-draft.1",
            "root": "0000000000000000000000000000000000000000000000000000000000000000"
        }),
        gamma_source_head: String::new(),
        gamma_request_digest:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        gamma_result: Vec::new(),
        content_key: did.keys.content.clone(),
        receipt_key: did.keys.root.clone(),
    };
    let changeset = serde_json::to_value(derive_changeset(&context).expect("changeset"))
        .expect("changeset value");
    let changeset_ref = serde_json::json!({
        "aithos-changeset-core": CHANGESET_PROFILE,
        "digest": commitment(
            "aithos-core/v1/changeset",
            &jcs::canonical_bytes(&changeset).expect("changeset JCS")
        )
    });
    let publication_facts = serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "kind": "publication",
        "facts": {
            "mode": "normal",
            "height": 2,
            "predecessors": [predecessor],
            "changeset_ref": changeset_ref,
            "contained_operations": [mutation_ref],
        }
    });
    let publication_projection = serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": "op_01K00000000000000000000099",
        "subject": did.id,
        "at": "2026-07-18T15:02:00Z",
        "authority": {"actor": "owner"},
        "history_heads": context.predecessors,
        "operation": {
            "kind": "publication",
            "facts_ref": facts_ref(&publication_facts),
        }
    });
    context.publication_ref = operation_ref(&publication_projection);
    context.publication_projection = publication_projection;
    context.publication_facts = publication_facts;

    let mut authorship = serde_json::json!({
        "aithos-authorship-core": "1.0.0-draft.1",
        "subject": did.id,
        "zone": "public",
        "sid": sid,
        "content_hash": sha256_text(&body),
        "operation_ref": context.contained_operations[0],
        "edition": {
            "height": 2,
            "predecessors": context.predecessors,
        },
        "authorized_via": [],
        "key": root_key,
        "sig": "",
    });
    let mut unsigned = authorship.as_object().expect("authorship").clone();
    unsigned.remove("sig");
    authorship["sig"] = Value::String(hex::encode(
        owner
            .root_sign
            .sign(&jcs::canonical_bytes(&Value::Object(unsigned)).expect("authorship JCS"))
            .to_bytes(),
    ));
    let evidence = serde_json::json!({
        "aithos-evidence-core": "1.0.0-draft.1",
        "items": [{
            "kind": "authorship",
            "document": authorship,
        }],
        "delegated_counts": context.delegated_counts,
    });
    (owner, context, evidence, BTreeMap::new())
}

#[test]
fn cb12_bundle_assembles_the_exact_signed_draft2_candidate() {
    let vector = vector();
    let context = context(&vector);
    let signer = grantee_signer(&vector);
    let session = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let capability = session.manifest_capability();
    let candidate = session
        .assemble_draft2(
            &capability,
            &context,
            vector["positive"]["candidate"]["evidence"].clone(),
        )
        .expect("draft2 candidate assembles");

    assert_eq!(
        candidate.to_value().expect("candidate value"),
        vector["positive"]["candidate"]
    );
}

#[test]
fn cb12_bundle_preserves_all_37_closed_error_boundaries() {
    let vector = vector();
    let context = context(&vector);
    let cases = vector["negative_cases"].as_array().expect("negative cases");
    assert_eq!(cases.len(), 37);

    for case in cases {
        let id = case["id"].as_str().expect("negative id");
        let error = verify_draft2_candidate_value(&case["candidate"], &context).expect_err(id);
        match case["must_fail"].as_str() {
            Some("InvalidOperation") => {
                assert!(
                    matches!(error, Error::InvalidOperation(_)),
                    "{id}: {error:?}"
                );
            }
            Some("InvalidDidDocument") => {
                assert!(
                    matches!(error, Error::InvalidDidDocument(_)),
                    "{id}: {error:?}"
                );
            }
            other => panic!("{id}: unexpected error inventory {other:?}"),
        }
    }
}

#[test]
fn cb12_capabilities_are_class_bound_and_rejected_across_sessions() {
    let vector = vector();
    let context = context(&vector);
    let signer = grantee_signer(&vector);
    let first = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let second = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let first_manifest = first.manifest_capability();
    let first_gamma = first.gamma_capability();

    assert!(matches!(
        second.assemble_draft2(
            &first_manifest,
            &context,
            vector["positive"]["candidate"]["evidence"].clone(),
        ),
        Err(Error::InvalidSession(_))
    ));
    assert!(matches!(
        second.accepts_gamma_capability(&first_gamma),
        Err(Error::InvalidSession(_))
    ));

    let (owner, owner_context, _, _) = owner_cold_fixture();
    let owner_first = LocalSession::owner(owner_context.subject.clone(), &owner);
    let owner_second = LocalSession::owner(owner_context.subject, &owner);
    let audit = owner_first
        .audit_capability()
        .expect("owner audit capability");
    assert!(matches!(
        owner_second.accepts_audit_capability(&audit),
        Err(Error::InvalidSession(_))
    ));

    // The direct assembly API is deterministic too; the session only narrows
    // authority and capability use.
    let direct = assemble_draft2_candidate(
        &context,
        vector["positive"]["candidate"]["evidence"].clone(),
        aithos_bundle::manifest::ManifestSigner::Delegate {
            key_multibase: context.actor.public_key().to_owned(),
            sk: &signer,
        },
    )
    .expect("direct Bundle assembly");
    assert_eq!(
        direct.to_value().expect("candidate value"),
        vector["positive"]["candidate"]
    );
}

#[test]
fn cb12_owner_package_survives_fresh_mem_and_fs_cold_verification() {
    let (owner, context, evidence, extra) = owner_cold_fixture();
    let session = LocalSession::owner(context.subject.clone(), &owner);
    let manifest_capability = session.manifest_capability();
    let candidate = session
        .assemble_draft2(&manifest_capability, &context, evidence)
        .expect("owner draft2 candidate");
    let package = export_keyless(candidate, context, extra).expect("keyless export");
    let digest = package.digest().expect("package digest");
    assert_eq!(digest, package.digest().expect("stable package digest"));
    let producer_verdict = package.verify_for_cas().expect("producer CAS verdict");
    assert_eq!(producer_verdict.cas.subject, package.context().subject);
    assert_eq!(producer_verdict.cas.manifest_profile, "1.0.0-draft.2");
    assert_eq!(producer_verdict.cas.mode, PublicationMode::Normal);
    assert_eq!(producer_verdict.cas.new_height, 2);
    assert_eq!(producer_verdict.cas.expected_predecessors.len(), 1);
    assert!(producer_verdict
        .cas
        .new_manifest_head
        .starts_with("sha256:"));
    assert_eq!(producer_verdict.cas.package_digest, digest);
    assert_eq!(
        producer_verdict.cas.reachable_objects,
        package.objects().keys().cloned().collect::<Vec<_>>()
    );
    assert_eq!(producer_verdict.carriers.height(), 2);

    // Destroy every producer-side capability and private key holder before
    // either keyless verification phase.
    drop(manifest_capability);
    drop(session);
    drop(owner);

    let mut memory = MemStore::default();
    import_keyless(&mut memory, &package).expect("MemStore import");
    cold_verify(&memory, &package).expect("MemStore cold verification");
    assert_eq!(
        cold_verify_for_cas(&memory, &package).expect("MemStore CAS verdict"),
        producer_verdict
    );

    let fs_root = std::env::temp_dir().join(format!("aithos-cb12-cold-{}", std::process::id()));
    if fs_root.exists() {
        std::fs::remove_dir_all(&fs_root).expect("remove prior test root");
    }
    std::fs::create_dir_all(&fs_root).expect("create FsStore root");
    let mut fs = FsStore::new(&fs_root);
    import_keyless(&mut fs, &package).expect("FsStore import");
    drop(fs);
    let reopened = FsStore::new(&fs_root);
    cold_verify(&reopened, &package).expect("fresh FsStore cold verification");
    assert_eq!(
        cold_verify_for_cas(&reopened, &package).expect("fresh FsStore CAS verdict"),
        producer_verdict
    );

    for defect in [
        "missing-certificate",
        "substituted-certificate",
        "truncated-gamma",
        "wrong-parent",
        "missing-authorship",
        "unpinned-object",
    ] {
        let mut objects = package.objects().clone();
        match defect {
            "missing-certificate" => {
                objects.remove("did.json");
            }
            "substituted-certificate" => {
                objects.insert("did.json".into(), b"{}".to_vec());
            }
            "truncated-gamma" => {
                objects.insert("gamma/2026-07.jsonl".into(), b"truncated".to_vec());
            }
            "wrong-parent" => {
                objects.insert("manifests/1.json".into(), b"{}".to_vec());
            }
            "missing-authorship" => {
                let evidence_path = objects
                    .keys()
                    .find(|path| path.starts_with("evidence/"))
                    .expect("evidence path")
                    .clone();
                objects.insert(
                    evidence_path,
                    br#"{"aithos-evidence-core":"1.0.0-draft.1","delegated_counts":{"aithos-delegated-counts-core":"1.0.0-draft.1","root":"0000000000000000000000000000000000000000000000000000000000000000"},"items":[]}"#
                        .to_vec(),
                );
            }
            "unpinned-object" => {
                objects.insert("roots/public.json".into(), b"{}".to_vec());
            }
            _ => unreachable!(),
        }
        let defective = package_with_objects(&package, objects);
        let mut store = MemStore::default();
        import_keyless(&mut store, &defective).expect("defective import remains atomic");
        assert!(
            cold_verify(&store, &defective).is_err(),
            "{defect} must fail closed"
        );
    }
    std::fs::remove_dir_all(&fs_root).expect("clean FsStore test root");
}

#[test]
fn cb12_private_reads_resume_only_after_capabilities_are_reintroduced() {
    let seed = [0xa1; 32];
    let grantee_seed = [0xa3; 32];
    let owner = OwnerKeys::genesis(&MasterSeed::from_slice(&seed).expect("owner seed"));
    let grantee = SigningKey::from_bytes(&grantee_seed);
    let succession = succession_from_entropy([0xa2; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T16:00:00Z",
    )
    .expect("bundle init");
    bundle
        .section_add(
            &SectionSpec {
                zone: Zone::Circle,
                folder_path: "cold",
                name: "note",
                title: "note",
                tags: &[],
                body: "capability reintroduced",
                now: "2026-07-18T16:01:00Z",
            },
            &owner,
            &mut entropy,
        )
        .expect("protected section");
    bundle
        .publish(&owner, "2026-07-18T16:02:00Z")
        .expect("publication");
    let grant = bundle
        .grant_generic(
            &owner,
            "cold-reader",
            &grantee.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("cold".into()),
            )],
            "2026-07-18T16:02:30Z",
            "2026-07-19T16:02:30Z",
            0,
            "2026-07-18T16:02:30Z",
            &mut entropy,
        )
        .expect("grant protected read");
    let authority_chain = vec![grant.mandate];

    let producer = LocalSession::owner(bundle.did.clone(), &owner);
    let producer_body = producer.body_capability().expect("producer body cap");
    assert_eq!(
        producer
            .read_owner_section(&producer_body, &bundle, Zone::Circle, "cold/note")
            .expect("producer read"),
        "capability reintroduced"
    );
    drop(producer_body);
    drop(producer);
    drop(grantee);
    drop(owner);

    // Keyless verification is complete while no producer or private
    // capability exists.
    bundle.verify().expect("keyless bundle verification");

    let restored_owner = OwnerKeys::genesis(&MasterSeed::from_slice(&seed).expect("restored seed"));
    let restored = LocalSession::owner(bundle.did.clone(), &restored_owner);
    let restored_body = restored.body_capability().expect("restored body cap");
    assert_eq!(
        restored
            .read_owner_section(&restored_body, &bundle, Zone::Circle, "cold/note")
            .expect("restored read"),
        "capability reintroduced"
    );

    let restored_grantee = SigningKey::from_bytes(&grantee_seed);
    let grantee_session = LocalSession::grantee_from_mandates(
        bundle.did.clone(),
        &restored_grantee,
        &authority_chain,
    )
    .expect("restore grantee session");
    let grantee_body = grantee_session
        .body_capability()
        .expect("restored grantee body cap");
    assert_eq!(
        grantee_session
            .read_grantee_section(
                &grantee_body,
                &bundle,
                &authority_chain,
                Zone::Circle,
                "cold/note",
                "2026-07-18T16:03:00Z",
            )
            .expect("restored grantee read"),
        "capability reintroduced"
    );
}
