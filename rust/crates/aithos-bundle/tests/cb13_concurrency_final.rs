//! CB13 two-parent draft.2 package, cold replay and order independence.

use aithos_bundle::manifest::{sha256_hex, Manifest, ManifestSigner};
use aithos_bundle::merge::{
    cold_merge_from_keyless_store, merge_draft2_package, resolve_draft2_package,
    verify_insertion_order_independence, Draft2MergePlan, Draft2ResolutionPlan,
};
use aithos_bundle::publication::{
    cold_verify_for_cas, import_keyless, KeylessPublicationPackage, PublicationMode,
    VerifiedPublication,
};
use aithos_bundle::{FsStore, MemStore, Store};
use aithos_core::carriers::{
    derive_changeset, K1cActor, K1cVerificationContext, CHANGESET_PROFILE,
};
use aithos_core::concurrency::{MergeAuthority, SemanticOccurrence};
use aithos_core::did::DidDocument;
use aithos_core::jcs;
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::wire;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
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

fn authorship(
    owner: &OwnerKeys,
    subject: &str,
    sid: &str,
    body: &[u8],
    operation_ref: Value,
    predecessors: &[Value],
) -> Value {
    let key = wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes());
    let mut document = serde_json::json!({
        "aithos-authorship-core": "1.0.0-draft.1",
        "subject": subject,
        "zone": "public",
        "sid": sid,
        "content_hash": format!("sha256:{}", sha256_hex(body)),
        "operation_ref": operation_ref,
        "edition": {
            "height": 3,
            "predecessors": predecessors,
        },
        "authorized_via": [],
        "key": key,
        "sig": "",
    });
    let mut unsigned = document.as_object().expect("authorship object").clone();
    unsigned.remove("sig");
    document["sig"] = Value::String(hex::encode(
        owner
            .root_sign
            .sign(&jcs::canonical_bytes(&Value::Object(unsigned)).expect("authorship JCS"))
            .to_bytes(),
    ));
    document
}

struct Fixture {
    owner: OwnerKeys,
    context: K1cVerificationContext,
    evidence: Value,
    parents: [String; 2],
    touched: BTreeSet<String>,
    occurrences: Vec<SemanticOccurrence>,
}

fn fixture(mode: &str) -> Fixture {
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes([0xb1; 32]));
    let succession = SigningKey::from_bytes(&[0xb2; 32]);
    let did = DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec!["file://cb13".into()],
        "gamma/2026-07.jsonl".into(),
    )
    .expect("DID");
    let did_bytes = jcs::canonical_bytes(&did).expect("DID JCS");
    let gamma = Vec::new();
    let genesis = Manifest::build(
        &owner.root_sign,
        1,
        String::new(),
        "2026-07-18T17:00:00Z".into(),
        BTreeMap::from([
            ("did.json".into(), sha256_hex(&did_bytes)),
            ("gamma/2026-07.jsonl".into(), sha256_hex(&gamma)),
        ]),
        BTreeMap::new(),
        BTreeMap::new(),
        String::new(),
        String::new(),
    )
    .expect("genesis manifest");
    let genesis_bytes = jcs::canonical_bytes(&genesis).expect("genesis JCS");
    let genesis_hash = genesis.chain_hash().expect("genesis hash");
    let parent_files = BTreeMap::from([
        ("did.json".into(), sha256_hex(&did_bytes)),
        ("gamma/2026-07.jsonl".into(), sha256_hex(&gamma)),
        ("manifests/1.json".into(), sha256_hex(&genesis_bytes)),
    ]);
    let parent_a = Manifest::build(
        &owner.root_sign,
        2,
        genesis_hash.clone(),
        "2026-07-18T17:01:00Z".into(),
        parent_files.clone(),
        BTreeMap::new(),
        BTreeMap::new(),
        String::new(),
        String::new(),
    )
    .expect("parent A");
    let parent_b = Manifest::build(
        &owner.root_sign,
        2,
        genesis_hash,
        "2026-07-18T17:01:01Z".into(),
        parent_files,
        BTreeMap::new(),
        BTreeMap::new(),
        String::new(),
        String::new(),
    )
    .expect("parent B");
    let mut parents_with_bytes = [
        (
            parent_a.chain_hash().expect("parent A hash"),
            jcs::canonical_bytes(&parent_a).expect("parent A JCS"),
        ),
        (
            parent_b.chain_hash().expect("parent B hash"),
            jcs::canonical_bytes(&parent_b).expect("parent B JCS"),
        ),
    ];
    parents_with_bytes.sort_by(|left, right| left.0.cmp(&right.0));
    let parents = [
        format!("sha256:{}", parents_with_bytes[0].0),
        format!("sha256:{}", parents_with_bytes[1].0),
    ];
    let predecessors = parents
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();

    let sid_left = "01J00000000000000000000131";
    let sid_right = "01J00000000000000000000132";
    let body_left = b"# left\n".to_vec();
    let body_right = b"# right\n".to_vec();
    let path_left = format!("public/sections/{sid_left}.md");
    let path_right = format!("public/sections/{sid_right}.md");
    let mut parent_store = BTreeMap::from([
        ("did.json".into(), did_bytes),
        ("gamma/2026-07.jsonl".into(), gamma),
        ("manifests/1.json".into(), genesis_bytes),
        ("manifests/2.json".into(), parents_with_bytes[0].1.clone()),
        (
            "manifests/2-alt.json".into(),
            parents_with_bytes[1].1.clone(),
        ),
    ]);
    let mut candidate_store = parent_store.clone();
    candidate_store.insert(path_left.clone(), body_left.clone());
    candidate_store.insert(path_right.clone(), body_right.clone());

    let root_key = wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes());
    let operation_rows = [
        (
            sid_left,
            "op_01K00000000000000000000131",
            body_left,
            path_left,
        ),
        (
            sid_right,
            "op_01K00000000000000000000132",
            body_right,
            path_right,
        ),
    ];
    let mut operation_facts = Vec::new();
    let mut operation_projections = Vec::new();
    let mut contained_operations = Vec::new();
    let mut change_causes = BTreeMap::new();
    let mut evidence_items = Vec::new();
    for (sid, occurrence, body, path) in operation_rows {
        let facts = serde_json::json!({
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
        let projection = serde_json::json!({
            "aithos-operation-core": "1.0.0-draft.1",
            "occurrence": occurrence,
            "subject": did.id,
            "at": "2026-07-18T17:02:00Z",
            "authority": {"actor": "owner"},
            "history_heads": predecessors,
            "operation": {
                "kind": "mutation",
                "facts_ref": facts_ref(&facts),
            }
        });
        let reference = operation_ref(&projection);
        evidence_items.push(serde_json::json!({
            "kind": "authorship",
            "document": authorship(
                &owner,
                &did.id,
                sid,
                &body,
                reference.clone(),
                &predecessors,
            ),
        }));
        change_causes.insert(path, reference.clone());
        contained_operations.push(reference);
        operation_projections.push(projection);
        operation_facts.push(facts);
    }
    evidence_items.sort_by_key(|item| jcs::canonical_bytes(item).expect("evidence item JCS"));

    let mut context = K1cVerificationContext {
        subject: did.id.clone(),
        actor: K1cActor::Owner {
            key: root_key.clone(),
        },
        height: 3,
        predecessors: predecessors.clone(),
        parent_store: std::mem::take(&mut parent_store),
        candidate_store,
        change_causes,
        contained_operations: contained_operations.clone(),
        operation_projections,
        operation_facts,
        authority_documents: Vec::new(),
        publication_projection: Value::Null,
        publication_facts: Value::Null,
        publication_ref: Value::Null,
        publication_at: "2026-07-18T17:03:00Z".into(),
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
        ),
    });
    let mut publication_body = serde_json::json!({
        "mode": mode,
        "height": 3,
        "predecessors": predecessors,
        "changeset_ref": changeset_ref,
        "contained_operations": contained_operations,
    });
    if mode == "resolution" {
        publication_body["winner"] = Value::String(parents[0].clone());
    }
    let publication_facts = serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "kind": "publication",
        "facts": publication_body,
    });
    let publication_projection = serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": if mode == "merge" {
            "op_01K00000000000000000000139"
        } else {
            "op_01K00000000000000000000138"
        },
        "subject": did.id,
        "at": "2026-07-18T17:03:00Z",
        "authority": {"actor": "owner"},
        "history_heads": predecessors,
        "operation": {
            "kind": "publication",
            "facts_ref": facts_ref(&publication_facts),
        }
    });
    context.publication_ref = operation_ref(&publication_projection);
    context.publication_projection = publication_projection;
    context.publication_facts = publication_facts;
    let evidence = serde_json::json!({
        "aithos-evidence-core": "1.0.0-draft.1",
        "items": evidence_items,
        "delegated_counts": context.delegated_counts,
    });
    Fixture {
        owner,
        context,
        evidence,
        parents,
        touched: BTreeSet::from([sid_left.to_owned(), sid_right.to_owned()]),
        occurrences: vec![
            SemanticOccurrence {
                operation_ref: "op_01K00000000000000000000131".into(),
                kind: "mutation".into(),
            },
            SemanticOccurrence {
                operation_ref: "op_01K00000000000000000000132".into(),
                kind: "mutation".into(),
            },
        ],
    }
}

fn fresh_root(label: &str) -> PathBuf {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "aithos-cb13-{label}-{}-{serial}",
        std::process::id()
    ))
}

fn verify_all_orders(package: &KeylessPublicationPackage) -> VerifiedPublication {
    let forward = package.objects().keys().cloned().collect::<Vec<_>>();
    let mut reverse = forward.clone();
    reverse.reverse();
    verify_insertion_order_independence(package.objects(), &[forward.clone(), reverse.clone()])
        .expect("object-map order independence");
    let expected = package.verify_for_cas().expect("producer CAS verdict");

    let mut memory = MemStore::default();
    import_keyless(&mut memory, package).expect("fresh MemStore import");
    cold_merge_from_keyless_store(&memory, package).expect("MemStore cold merge");
    assert_eq!(
        cold_verify_for_cas(&memory, package).expect("MemStore CAS verdict"),
        expected
    );

    let root = fresh_root("orders");
    std::fs::create_dir_all(&root).expect("create FsStore root");
    let mut store = FsStore::new(&root);
    store.begin_transaction().expect("begin reverse import");
    for path in reverse {
        store
            .put(path.as_str(), &package.objects()[&path])
            .expect("insert object");
    }
    store.commit_transaction().expect("commit reverse import");
    drop(store);
    let reopened = FsStore::new(&root);
    cold_merge_from_keyless_store(&reopened, package).expect("FsStore cold merge");
    assert_eq!(
        cold_verify_for_cas(&reopened, package).expect("FsStore CAS verdict"),
        expected
    );
    std::fs::remove_dir_all(root).expect("remove FsStore root");
    expected
}

#[test]
fn cb13_signed_draft2_merge_is_cold_and_insertion_order_independent() {
    let fixture = fixture("merge");
    let plan = Draft2MergePlan {
        parents: fixture.parents.clone(),
        left_changed_sids: BTreeSet::from(["01J00000000000000000000131".to_owned()]),
        right_changed_sids: BTreeSet::from(["01J00000000000000000000132".to_owned()]),
        deleted_sids: BTreeSet::new(),
        authority: MergeAuthority::Owner,
        left_occurrences: fixture.occurrences[..1].to_vec(),
        right_occurrences: fixture.occurrences[1..].to_vec(),
    };
    let package = merge_draft2_package(
        &plan,
        fixture.context,
        fixture.evidence,
        ManifestSigner::Root(&fixture.owner.root_sign),
        BTreeMap::new(),
    )
    .expect("draft2 merge package");
    assert_eq!(package.candidate().manifest.merges.len(), 2);
    assert!(package.candidate().manifest.resolves_fork.is_empty());
    let verdict = verify_all_orders(&package);
    assert_eq!(verdict.cas.mode, PublicationMode::Merge);
    assert_eq!(verdict.cas.expected_predecessors, plan.parents);
    assert_eq!(verdict.cas.resolution_winner, None);
    assert_eq!(verdict.cas.new_height, 3);
}

#[test]
fn cb13_resolution_authority_is_effect_free_and_cold_verifiable() {
    let fixture = fixture("resolution");
    let plan = Draft2ResolutionPlan {
        parents: fixture.parents.clone(),
        winner: fixture.parents[0].clone(),
        touched_sids: fixture.touched.clone(),
        authority: MergeAuthority::Owner,
        left_occurrences: fixture.occurrences[..1].to_vec(),
        right_occurrences: fixture.occurrences[1..].to_vec(),
    };
    let package = resolve_draft2_package(
        &plan,
        fixture.context,
        fixture.evidence,
        ManifestSigner::Root(&fixture.owner.root_sign),
        BTreeMap::new(),
    )
    .expect("draft2 resolution package");
    assert!(package.candidate().manifest.merges.is_empty());
    assert_eq!(
        package.candidate().manifest.resolves_fork,
        fixture.parents[0].trim_start_matches("sha256:")
    );
    let verdict = verify_all_orders(&package);
    assert_eq!(verdict.cas.mode, PublicationMode::Resolution);
    assert_eq!(verdict.cas.expected_predecessors, plan.parents);
    assert_eq!(verdict.cas.resolution_winner, Some(plan.winner.clone()));

    let outside = Draft2ResolutionPlan {
        parents: fixture.parents,
        winner: plan.winner,
        touched_sids: fixture.touched,
        authority: MergeAuthority::Grantee {
            chain_count: 1,
            covered_sids: BTreeSet::from(["01J00000000000000000000131".to_owned()]),
        },
        left_occurrences: plan.left_occurrences,
        right_occurrences: plan.right_occurrences,
    };
    assert!(aithos_core::concurrency::verify_fork_resolution(
        &outside.touched_sids,
        &outside.authority
    )
    .is_err());
}
