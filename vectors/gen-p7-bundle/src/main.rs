//! Emits the bundle-exported keyless publication packages consumed by
//! gen-p7.py / gen-p8.py (piste P, gate contrat P2).
//!
//! Anchored on the committed vectors before emitting anything:
//!   - a1-genesis.json: the owner keys re-derived from the committed seed
//!     must match byte for byte;
//!   - p1-store-envelope.json: the DID document rebuilt through
//!     `DidDocument::build` must be byte-identical to the committed
//!     `did_json_jcs` (identity anchor: p7 lives on p1's DID).
//!
//! Every package goes through the real façade: `assemble_draft2_candidate`
//! (which runs the Core K1-C verdict), `export_keyless` (which re-verifies
//! public-only), then `verify_for_cas()` for the typed CAS facts. Nothing
//! here decides anything — the bundle is the oracle.
//!
//! Deterministic by construction: fixed seeds, fixed instants, fixed sids.
//! Usage: cargo run --release -- <vectors-dir>   (emits JSON on stdout)

use aithos_bundle::manifest::{sha256_hex, Manifest, ManifestSigner};
use aithos_bundle::publication::{
    assemble_draft2_candidate, export_keyless, Draft2Candidate, KeylessPublicationPackage,
    PublicationCasFacts, PublicationMode,
};
use aithos_core::carriers::{derive_changeset, K1cActor, K1cVerificationContext, CHANGESET_PROFILE};
use aithos_core::did::DidDocument;
use aithos_core::jcs;
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::wire;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const STORE_HOST: &str = "store.aithos.fr";
const TENANT: &str = "acme";

fn jcs_string(value: &Value) -> String {
    String::from_utf8(jcs::canonical_bytes(value).expect("JCS")).expect("utf8")
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
    json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": projection["occurrence"],
        "commitment": commitment(
            "aithos-core/v1/operation-commitment",
            &jcs::canonical_bytes(projection).expect("projection JCS")
        ),
    })
}

fn facts_ref(facts: &Value) -> Value {
    json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "digest": commitment(
            "aithos-core/v1/operation-facts",
            &jcs::canonical_bytes(facts).expect("facts JCS")
        ),
    })
}

/// One owner mutation (create) of `path` carrying `body`, projected at `at`
/// with deterministic ids — the cb12 owner recipe, verbatim shapes.
struct OwnerMutation {
    facts: Value,
    projection: Value,
    reference: Value,
}

#[allow(clippy::too_many_arguments)]
fn owner_mutation(
    did: &str,
    zone: &str,
    sid: &str,
    occurrence: &str,
    at: &str,
    predecessors: &[String],
    state_digest: &str,
) -> OwnerMutation {
    let facts = json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "kind": "mutation",
        "facts": {
            "domain": "ethos",
            "zone": zone,
            "dir": [],
            "sid": sid,
            "verb": "create",
            "before": {"state": "absent"},
            "after": {
                "state": "present",
                "state_ref": {
                    "aithos-state-fact-core": "1.0.0-draft.1",
                    "digest": state_digest,
                }
            }
        }
    });
    let projection = json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": occurrence,
        "subject": did,
        "at": at,
        "authority": {"actor": "owner"},
        "history_heads": predecessors,
        "operation": {
            "kind": "mutation",
            "facts_ref": facts_ref(&facts),
        }
    });
    let reference = operation_ref(&projection);
    OwnerMutation {
        facts,
        projection,
        reference,
    }
}

struct BuiltPackage {
    package: KeylessPublicationPackage,
    facts: PublicationCasFacts,
}

/// Assemble + export + verify one owner publication over `parent_store`.
/// `mode_facts` carries the publication facts body (normal or merge).
#[allow(clippy::too_many_arguments)]
fn owner_package(
    owner: &OwnerKeys,
    did: &str,
    height: u64,
    predecessors: Vec<String>,
    parent_store: BTreeMap<String, Vec<u8>>,
    candidate_store: BTreeMap<String, Vec<u8>>,
    mutations: Vec<(String, OwnerMutation)>,
    publication_occurrence: &str,
    publication_at: &str,
    mode: &str,
    gamma_source_head: &str,
    did_doc: &DidDocument,
) -> BuiltPackage {
    let root_key = wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes());
    let contained: Vec<Value> = mutations.iter().map(|(_, m)| m.reference.clone()).collect();
    let mut context = K1cVerificationContext {
        subject: did.to_owned(),
        actor: K1cActor::Owner {
            key: root_key.clone(),
        },
        height,
        predecessors: predecessors.iter().cloned().map(Value::String).collect(),
        sparse_parent_manifest: None,
        parent_store,
        candidate_store,
        change_causes: mutations
            .iter()
            .map(|(path, m)| (path.clone(), m.reference.clone()))
            .collect(),
        contained_operations: contained.clone(),
        operation_projections: mutations.iter().map(|(_, m)| m.projection.clone()).collect(),
        operation_facts: mutations.iter().map(|(_, m)| m.facts.clone()).collect(),
        authority_documents: Vec::new(),
        publication_projection: Value::Null,
        publication_facts: Value::Null,
        publication_ref: Value::Null,
        publication_at: publication_at.to_owned(),
        required_receipts: Vec::new(),
        delegated_counts: json!({
            "aithos-delegated-counts-core": "1.0.0-draft.1",
            "root": "0000000000000000000000000000000000000000000000000000000000000000"
        }),
        gamma_source_head: gamma_source_head.to_owned(),
        gamma_request_digest:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        gamma_result: Vec::new(),
        content_key: did_doc.keys.content.clone(),
        receipt_key: did_doc.keys.root.clone(),
    };
    let changeset =
        serde_json::to_value(derive_changeset(&context).expect("changeset")).expect("value");
    let changeset_ref = json!({
        "aithos-changeset-core": CHANGESET_PROFILE,
        "digest": commitment(
            "aithos-core/v1/changeset",
            &jcs::canonical_bytes(&changeset).expect("changeset JCS")
        )
    });
    let mut facts_body = json!({
        "mode": mode,
        "height": height,
        "predecessors": predecessors,
        "changeset_ref": changeset_ref,
        "contained_operations": contained,
    });
    if mode == "merge" {
        // No extra field: merge names both predecessors, nothing more.
        let _ = &mut facts_body;
    }
    let publication_facts = json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "kind": "publication",
        "facts": facts_body,
    });
    let publication_projection = json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": publication_occurrence,
        "subject": did,
        "at": publication_at,
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

    // Authorship evidence for every mutated path (owner-signed, cb12 shape).
    let items: Vec<Value> = mutations
        .iter()
        .filter(|(_, m)| m.facts["facts"]["zone"] == "public")
        .map(|(path, m)| {
            let zone = m.facts["facts"]["zone"].as_str().expect("zone");
            let sid = m.facts["facts"]["sid"].as_str().expect("sid");
            let body = context.candidate_store.get(path).expect("mutated body");
            let mut authorship = json!({
                "aithos-authorship-core": "1.0.0-draft.1",
                "subject": did,
                "zone": zone,
                "sid": sid,
                "content_hash": sha256_text(body),
                "operation_ref": m.reference,
                "edition": {
                    "height": height,
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
                    .sign(&jcs::canonical_bytes(&Value::Object(unsigned)).expect("JCS"))
                    .to_bytes(),
            ));
            json!({"kind": "authorship", "document": authorship})
        })
        .collect();
    let evidence = json!({
        "aithos-evidence-core": "1.0.0-draft.1",
        "items": items,
        "delegated_counts": context.delegated_counts,
    });

    let candidate = assemble_draft2_candidate(
        &context,
        evidence,
        ManifestSigner::Root(&owner.root_sign),
    )
    .expect("owner draft2 candidate assembles");
    let package = export_keyless(candidate, context, BTreeMap::new()).expect("keyless export");
    let facts = package.verify_for_cas().expect("verify_for_cas").cas;
    BuiltPackage { package, facts }
}

fn emit_package(built: &BuiltPackage) -> Value {
    let package = &built.package;
    let facts = &built.facts;
    let objects: serde_json::Map<String, Value> = package
        .objects()
        .iter()
        .map(|(path, bytes)| {
            let value = match std::str::from_utf8(bytes) {
                Ok(text) => json!({"utf8": text}),
                Err(_) => json!({"hex": hex::encode(bytes)}),
            };
            (path.clone(), value)
        })
        .collect();
    let manifest_value = serde_json::to_value(&package.candidate().manifest).expect("manifest");
    json!({
        "candidate": package.candidate().to_value().expect("candidate"),
        "context": serde_json::to_value(package.context()).expect("context"),
        "objects": objects,
        "manifest_jcs": jcs_string(&manifest_value),
        "cas_facts": {
            "subject": facts.subject,
            "manifest_profile": facts.manifest_profile,
            "mode": match facts.mode {
                PublicationMode::Normal => "normal",
                PublicationMode::Merge => "merge",
                PublicationMode::Resolution => "resolution",
            },
            "new_height": facts.new_height,
            "expected_predecessors": facts.expected_predecessors,
            "resolution_winner": facts.resolution_winner,
            "source_gamma_head": facts.source_gamma_head,
            "new_manifest_head": facts.new_manifest_head,
            "new_gamma_head": facts.new_gamma_head,
            "roots": facts.roots,
            "gamma_roots": serde_json::to_value(&facts.gamma_roots).expect("gamma roots"),
            "gamma_counts_root": facts.gamma_counts_root,
            "reachable_objects": facts.reachable_objects,
            "package_digest": facts.package_digest,
        },
    })
}

fn main() {
    let vectors_dir = std::env::args().nth(1).unwrap_or_else(|| "..".to_owned());
    let read = |name: &str| -> Value {
        let path = format!("{vectors_dir}/{name}");
        serde_json::from_str(&std::fs::read_to_string(&path).expect(&path)).expect("json")
    };

    // ---------------------------------------------------------- anchors
    let a1 = read("a1-genesis.json");
    let seed_bytes = hex::decode(a1["seed_hex"].as_str().expect("a1 seed")).expect("hex");
    let owner = OwnerKeys::genesis(&MasterSeed::from_slice(&seed_bytes).expect("seed"));
    assert_eq!(
        hex::encode(owner.root_sign.verifying_key().to_bytes()),
        a1["root_sign_pub_hex"].as_str().expect("a1 root"),
        "A1 root pub drift"
    );
    assert_eq!(
        hex::encode(owner.content_sign.verifying_key().to_bytes()),
        a1["content_sign_pub_hex"].as_str().expect("a1 content"),
        "A1 content pub drift"
    );
    let did = wire::did_aithos(&owner.root_sign.verifying_key().to_bytes());
    assert_eq!(did, a1["did"].as_str().expect("a1 did"), "A1 did drift");

    // The identity anchor: rebuild p1's did.json through the bundle/core API
    // and demand byte identity with the committed vector.
    let p1 = read("p1-store-envelope.json");
    let succession = SigningKey::from_bytes(&[0xaa; 32]);
    let did_doc = DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec![format!("https://{STORE_HOST}/t/{TENANT}/{did}")],
        "gamma/gamma.jsonl".into(),
    )
    .expect("DID document");
    let did_json = String::from_utf8(jcs::canonical_bytes(&did_doc).expect("DID JCS")).unwrap();
    assert_eq!(
        did_json,
        p1["did_json_jcs"].as_str().expect("p1 did json"),
        "p1 did.json drift: the rebuilt DID document is not byte-identical"
    );
    let did_bytes = did_json.clone().into_bytes();

    // ---------------------------------------------------- draft.1 genesis
    // The height-1 edition every p7 chain hangs from — a REAL bundle
    // manifest (draft.1: no carriers), root-signed.
    let m1 = Manifest::build(
        &owner.root_sign,
        1,
        String::new(),
        "2026-07-19T10:00:00Z".into(),
        BTreeMap::from([("did.json".into(), sha256_hex(&did_bytes))]),
        BTreeMap::new(),
        BTreeMap::new(),
        String::new(),
        String::new(),
    )
    .expect("m1");
    let m1_value = serde_json::to_value(&m1).expect("m1 value");
    let m1_jcs = jcs_string(&m1_value);
    let c1 = m1.chain_hash().expect("m1 chain hash");
    let predecessor = format!("sha256:{c1}");

    let gamma_seg = "gamma/2026-07.jsonl";
    let parent_store: BTreeMap<String, Vec<u8>> = BTreeMap::from([
        ("did.json".into(), did_bytes.clone()),
        (gamma_seg.to_owned(), Vec::new()),
        ("manifests/1.json".into(), m1_jcs.clone().into_bytes()),
    ]);

    // ------------------------------------------------- package A (h2, owner)
    let sid_a = "01000000000000000000000P71";
    let body_a = b"# p7 hello\n".to_vec();
    let path_a = format!("public/sections/{sid_a}.md");
    let mut store_a = parent_store.clone();
    store_a.insert(path_a.clone(), body_a);
    let mut_a = owner_mutation(
        &did,
        "public",
        sid_a,
        "op_01000000000000000000000P71",
        "2026-07-19T10:01:00Z",
        std::slice::from_ref(&predecessor),
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    );
    let pkg_a = owner_package(
        &owner,
        &did,
        2,
        vec![predecessor.clone()],
        parent_store.clone(),
        store_a,
        vec![(path_a, mut_a)],
        "op_01000000000000000000000P79",
        "2026-07-19T10:02:00Z",
        "normal",
        "",
        &did_doc,
    );

    // ------------------------------------------------ package B (h2, twin)
    let sid_b = "01000000000000000000000P72";
    let body_b = b"# p7 fork\n".to_vec();
    let path_b = format!("public/sections/{sid_b}.md");
    let mut store_b = parent_store.clone();
    store_b.insert(path_b.clone(), body_b);
    let mut_b = owner_mutation(
        &did,
        "public",
        sid_b,
        "op_01000000000000000000000P72",
        "2026-07-19T10:01:30Z",
        std::slice::from_ref(&predecessor),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    );
    let pkg_b = owner_package(
        &owner,
        &did,
        2,
        vec![predecessor.clone()],
        parent_store.clone(),
        store_b,
        vec![(path_b, mut_b)],
        "op_01000000000000000000000P7A",
        "2026-07-19T10:02:30Z",
        "normal",
        "",
        &did_doc,
    );

    // -------------------------------------------------- package M (h3, merge)
    // The two competing h2 heads, ascending; the store that serialized the
    // SMALLER hash is the one this merge lands on (prev_hash pins sorted[0],
    // and annexe A.4 demands prev_hash == stored head).
    let head_a = pkg_a.facts.new_manifest_head.clone();
    let head_b = pkg_b.facts.new_manifest_head.clone();
    let (first, second) = if head_a < head_b {
        (pkg_a.package.clone(), pkg_b.package.clone())
    } else {
        (pkg_b.package.clone(), pkg_a.package.clone())
    };
    let mut merge_preds: Vec<String> = vec![
        first.candidate().manifest.chain_hash().map(|h| format!("sha256:{h}")).expect("hash"),
        second.candidate().manifest.chain_hash().map(|h| format!("sha256:{h}")).expect("hash"),
    ];
    merge_preds.sort();

    // Parent state = the winning (first) branch's full store, tip archived.
    // The merge candidate carries NO new operation: it is the pure merge
    // marker naming both predecessors — content reconciliation semantics
    // stay client-side (the store never arbitrates; §02.6 is not re-modeled
    // here). The bundle verdict is the oracle for whether this is a valid
    // publication shape.
    let mut merge_parent: BTreeMap<String, Vec<u8>> = first.objects().clone();
    merge_parent.remove("manifest.json"); // the mutable tip is not a pinned parent object
    let _ = &second;
    let sid_m = "01000000000000000000000P73";
    let path_m = format!("public/sections/{sid_m}.md");
    let mut merge_store = merge_parent.clone();
    merge_store.insert(path_m.clone(), b"# p7 merge note\n".to_vec());
    let mut_m = owner_mutation(
        &did,
        "public",
        sid_m,
        "op_01000000000000000000000P73",
        "2026-07-19T10:03:00Z",
        &merge_preds,
        "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    );
    let pkg_m = owner_package(
        &owner,
        &did,
        3,
        merge_preds.clone(),
        merge_parent,
        merge_store,
        vec![(path_m, mut_m)],
        "op_01000000000000000000000P7B",
        "2026-07-19T10:04:00Z",
        "merge",
        "",
        &did_doc,
    );

    // -------------------------------------- package D (delegated, cb2 anchor)
    // The committed CB2 draft.2 candidate re-exported keyless: a REAL
    // grantee-signed publication, byte-anchored on the frozen vector.
    let cb2 = read("cb2-draft2-carriers.json");
    let delegated = build_delegated(&cb2);

    // ---------------------------------------------------- package P (p8)
    let sid_pub = "01000000000000000000000P81";
    let sid_circle = "01000000000000000000000P82";
    let path_pub = format!("public/sections/{sid_pub}.md");
    let path_circle = format!("circle/blobs/{sid_circle}.json");
    let body_pub = b"# p8 cold note\n".to_vec();
    // Opaque ciphertext-shaped filler: deterministic, never a secret.
    let body_circle: Vec<u8> = {
        let mut h = Sha256::new();
        h.update(b"aithos-p8/opaque-blob");
        let sealed = hex::encode(h.finalize());
        format!("{{\"sealed\":\"{sealed}\"}}").into_bytes()
    };
    let mut store_p = parent_store.clone();
    store_p.insert(path_pub.clone(), body_pub);
    store_p.insert(path_circle.clone(), body_circle);
    let mut_p1 = owner_mutation(
        &did,
        "public",
        sid_pub,
        "op_01000000000000000000000P81",
        "2026-07-19T10:05:00Z",
        std::slice::from_ref(&predecessor),
        "sha256:4444444444444444444444444444444444444444444444444444444444444444",
    );
    let mut_p2 = owner_mutation(
        &did,
        "circle",
        sid_circle,
        "op_01000000000000000000000P82",
        "2026-07-19T10:05:30Z",
        std::slice::from_ref(&predecessor),
        "sha256:5555555555555555555555555555555555555555555555555555555555555555",
    );
    let pkg_p = owner_package(
        &owner,
        &did,
        2,
        vec![predecessor.clone()],
        parent_store,
        store_p,
        vec![(path_pub, mut_p1), (path_circle, mut_p2)],
        "op_01000000000000000000000P89",
        "2026-07-19T10:06:00Z",
        "normal",
        "",
        &did_doc,
    );

    // ------------------------------------------------------------- emit
    let out = json!({
        "generator": "gen-p7-bundle (aithos-bundle keyless façade; no re-invented crypto)",
        "anchors": {
            "did": did,
            "did_json_jcs": did_json,
            "p1_did_json_matches": true,
        },
        "m1": {"jcs": m1_jcs, "chain_hash": c1},
        "packages": {
            "a_h2": emit_package(&pkg_a),
            "b_h2_twin": emit_package(&pkg_b),
            "m_h3_merge": emit_package(&pkg_m),
            "delegated_cb2": delegated,
            "p8_cold": emit_package(&pkg_p),
        },
    });
    println!("{}", serde_json::to_string_pretty(&out).expect("emit"));
}

/// Rebuild the committed CB2 K1-C context (the cb12 recipe, verbatim) and
/// re-export its candidate as a keyless package.
fn build_delegated(vector: &Value) -> Value {
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
    let positive = &vector["positive"];
    let context_v = &vector["context"];
    let required_receipts = positive["candidate"]["evidence"]["items"]
        .as_array()
        .expect("evidence items")
        .iter()
        .filter(|item| item["kind"] == "receipt")
        .map(|item| item["document"]["operation_ref"].clone())
        .collect();
    let context = K1cVerificationContext {
        subject: context_v["subject"].as_str().expect("subject").to_owned(),
        actor: K1cActor::Grantee {
            key: context_v["grantee_key"].as_str().expect("key").to_owned(),
            authority_chain: vec![context_v["authority_ref"].clone()],
        },
        height: context_v["height"].as_u64().expect("height"),
        predecessors: context_v["predecessors"].as_array().expect("preds").clone(),
        sparse_parent_manifest: None,
        parent_store: string_map(&context_v["store_before"]),
        candidate_store: string_map(&context_v["store_after"]),
        change_causes: value_map(&context_v["change_causes"]),
        contained_operations: positive["contained_operations"]
            .as_array()
            .expect("ops")
            .clone(),
        operation_projections: positive["operation_projections"]
            .as_array()
            .expect("projections")
            .clone(),
        operation_facts: positive["facts_documents"].as_array().expect("facts").clone(),
        authority_documents: vec![positive["authority_certificate"]["document"].clone()],
        publication_projection: positive["publication"]["projection"].clone(),
        publication_facts: positive["publication"]["facts"].clone(),
        publication_ref: context_v["publication_ref"].clone(),
        publication_at: context_v["publication_at"].as_str().expect("at").to_owned(),
        required_receipts,
        delegated_counts: context_v["delegated_counts"].clone(),
        gamma_source_head: context_v["source_head"].as_str().expect("head").to_owned(),
        gamma_request_digest: context_v["request_digest"]
            .as_str()
            .expect("digest")
            .to_owned(),
        gamma_result: context_v["query_result"].as_array().expect("result").clone(),
        content_key: context_v["content_key"].as_str().expect("content").to_owned(),
        receipt_key: context_v["receipt_key"].as_str().expect("receipt").to_owned(),
    };
    let candidate =
        Draft2Candidate::from_value(&positive["candidate"]).expect("cb2 candidate parses");
    let package =
        export_keyless(candidate, context, BTreeMap::new()).expect("cb2 keyless export");
    let facts = package.verify_for_cas().expect("cb2 verify_for_cas").cas;
    emit_package(&BuiltPackage { package, facts })
}
