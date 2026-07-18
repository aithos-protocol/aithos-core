//! CB2 local concurrency/final-gate vector consumer.
//!
//! The tests independently reproduce the CB13 pure decision matrices without
//! adding production behavior or simulating Provider CAS.

use std::collections::{BTreeMap, BTreeSet};

use aithos_bundle::merge::{recompose_semantic_counts, verify_insertion_order_independence};
use aithos_core::concurrency::{
    verify_disjoint_merge, verify_fork_resolution, MergeAuthority, SemanticOccurrence,
};
use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};

const VECTOR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-concurrency-final.json"
));
const VECTOR_SHA256: &str = "6bf4dadde60b902ac114685c52abc1893f9eaa8b09dcc0bcd06876d31f6a83d1";

const I1_CONCURRENCY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/i1-concurrency.json"
));
const H2_GAMMA_ROOTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/h2-gamma-roots.json"
));
const DELEGATED_COUNTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-delegated-counts.json"
));
const GAMMA_V2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-gamma-v2-replay.json"
));
const DRAFT2_CARRIERS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-draft2-carriers.json"
));
const BUNDLE_BOUNDARIES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-boundaries.json"
));
const AUTHORITY_FLOWS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-authority-flows.json"
));
const STRUCTURE_VAULT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-structure-vault.json"
));

const MERGE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/merge.rs"));
const MANIFEST_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/manifest.rs"));

fn vector() -> Value {
    serde_json::from_slice(VECTOR_BYTES).expect("CB2 Bundle concurrency vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn state_digest(value: &Value) -> String {
    let canonical = jcs::canonicalize(value).expect("fixture state JCS");
    format!("sha256:{}", sha256_hex(canonical.as_bytes()))
}

fn merge_verdict(case: &Value) -> &'static str {
    let left: BTreeSet<_> = case["left_changed_sids"]
        .as_array()
        .expect("left SIDs")
        .iter()
        .map(|value| value.as_str().expect("SID"))
        .collect();
    let right: BTreeSet<_> = case["right_changed_sids"]
        .as_array()
        .expect("right SIDs")
        .iter()
        .map(|value| value.as_str().expect("SID"))
        .collect();
    if !left.is_disjoint(&right) {
        "conflict"
    } else if case["delete_wins"] == true {
        "accepted without resurrection"
    } else {
        "accepted"
    }
}

fn authority_verdict(case: &Value) -> &'static str {
    if case["actor"] == "owner" {
        return "accepted";
    }
    let changed: BTreeSet<_> = case["changed_sids"]
        .as_array()
        .expect("changed SIDs")
        .iter()
        .map(|value| value.as_str().expect("SID"))
        .collect();
    let chains = case["chains"].as_array().expect("authority chains");
    if chains.len() != 1 {
        return "refused";
    }
    let covered: BTreeSet<_> = chains[0]["covers"]
        .as_array()
        .expect("covered SIDs")
        .iter()
        .map(|value| value.as_str().expect("SID"))
        .collect();
    if changed == covered {
        "accepted"
    } else {
        "refused"
    }
}

#[test]
fn cb2_bundle_concurrency_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(VECTOR_BYTES), VECTOR_SHA256);
}

#[test]
fn cb2_bundle_concurrency_historical_hashes_preexisting_green() {
    let vector = vector();
    for (name, bytes) in [
        ("i1-concurrency.json", I1_CONCURRENCY),
        ("h2-gamma-roots.json", H2_GAMMA_ROOTS),
        ("cb2-delegated-counts.json", DELEGATED_COUNTS),
        ("cb2-gamma-v2-replay.json", GAMMA_V2),
        ("cb2-draft2-carriers.json", DRAFT2_CARRIERS),
        ("cb2-bundle-boundaries.json", BUNDLE_BOUNDARIES),
        ("cb2-bundle-authority-flows.json", AUTHORITY_FLOWS),
        ("cb2-bundle-structure-vault.json", STRUCTURE_VAULT),
    ] {
        assert_eq!(
            sha256_hex(bytes),
            vector["historical_vector_sha256"][name]
                .as_str()
                .expect("historical vector hash"),
            "{name}"
        );
    }
}

#[test]
fn cb2_bundle_merge_conflict_and_authority_decisions_preexisting_green() {
    let vector = vector();
    assert_eq!(
        state_digest(&vector["initial_state"]),
        vector["initial_state_digest"]
    );
    let merge = &vector["merge"];
    let cases = merge["cases"].as_array().expect("merge cases");
    assert_eq!(cases.len(), 5);
    for case in cases {
        assert_eq!(merge_verdict(case), case["expected"], "{}", case["id"]);
        let left = case["left_changed_sids"]
            .as_array()
            .expect("left SIDs")
            .iter()
            .map(|value| value.as_str().expect("SID").to_owned())
            .collect::<BTreeSet<_>>();
        let right = case["right_changed_sids"]
            .as_array()
            .expect("right SIDs")
            .iter()
            .map(|value| value.as_str().expect("SID").to_owned())
            .collect::<BTreeSet<_>>();
        let deleted = if case["delete_wins"] == true {
            left.clone()
        } else {
            BTreeSet::new()
        };
        let production = verify_disjoint_merge(&left, &right, &deleted, &MergeAuthority::Owner);
        assert_eq!(
            production.is_ok(),
            case["expected"] != "conflict",
            "{}",
            case["id"]
        );
        if case["expected"] == "conflict" {
            assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
        }
        if let Some(order) = case["merged_sid_order"].as_array() {
            let mut sorted = order.clone();
            sorted.sort_by_key(|value| value.as_str().expect("SID").to_owned());
            assert_eq!(&sorted, order);
        }
    }
    let authority = merge["authority_cases"]
        .as_array()
        .expect("merge authority cases");
    assert_eq!(authority.len(), 4);
    for case in authority {
        assert_eq!(authority_verdict(case), case["expected"], "{}", case["id"]);
        let changed = case["changed_sids"]
            .as_array()
            .expect("changed SIDs")
            .iter()
            .map(|value| value.as_str().expect("SID").to_owned())
            .collect::<BTreeSet<_>>();
        let chains = case["chains"].as_array().expect("chains");
        let authority = if case["actor"] == "owner" {
            MergeAuthority::Owner
        } else {
            MergeAuthority::Grantee {
                chain_count: chains.len(),
                covered_sids: chains
                    .iter()
                    .flat_map(|chain| {
                        chain["covers"]
                            .as_array()
                            .expect("coverage")
                            .iter()
                            .map(|value| value.as_str().expect("SID").to_owned())
                    })
                    .collect(),
            }
        };
        assert_eq!(
            verify_disjoint_merge(&changed, &BTreeSet::new(), &BTreeSet::new(), &authority).is_ok(),
            case["expected"] == "accepted",
            "{}",
            case["id"]
        );
        if case["expected"] == "accepted" {
            assert!(matches!(
                case["published_actor"].as_str(),
                Some("owner" | "grantee")
            ));
            assert!(case["published_chain_count"].as_u64().unwrap() <= 1);
        } else {
            assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
        }
    }
    assert_eq!(merge["network_participates"], false);
    assert_eq!(merge["provider_cas_participates"], false);
    assert_eq!(
        merge["merge_entry_is_not_an_extra_business_consumption"],
        true
    );
}

#[test]
fn cb2_bundle_resolution_and_counter_recomposition_preexisting_green() {
    let vector = vector();
    let resolution = &vector["resolution"];
    let cases = resolution["cases"].as_array().expect("resolution cases");
    assert_eq!(cases.len(), 5);
    for case in cases {
        let expected = match case["actor"].as_str() {
            Some("owner") => "accepted",
            Some("grantee")
                if case["covers_every_touched_sid"] == true && case["chain_count"] == 1 =>
            {
                "accepted"
            }
            Some("none") => "no canonical branch",
            _ => "refused",
        };
        assert_eq!(expected, case["expected"], "{}", case["id"]);
        if case["actor"] != "none" {
            let touched = BTreeSet::from(["sid-left".to_owned(), "sid-right".to_owned()]);
            let authority = if case["actor"] == "owner" {
                MergeAuthority::Owner
            } else {
                MergeAuthority::Grantee {
                    chain_count: case["chain_count"].as_u64().expect("chain count") as usize,
                    covered_sids: if case["covers_every_touched_sid"] == true {
                        touched.clone()
                    } else {
                        BTreeSet::from(["sid-left".to_owned()])
                    },
                }
            };
            assert_eq!(
                verify_fork_resolution(&touched, &authority).is_ok(),
                expected == "accepted",
                "{}",
                case["id"]
            );
        }
        if expected != "accepted" {
            assert_eq!(case["visible_state_digest"], vector["initial_state_digest"]);
        }
    }
    assert_eq!(resolution["losing_write_is_surfaced_not_replayed"], true);
    assert_eq!(resolution["refusal_changes_canonical_bytes"], false);

    let counters = &vector["counter_recomposition"];
    let mut unique = BTreeMap::new();
    for occurrence in counters["left_occurrences"]
        .as_array()
        .expect("left occurrences")
        .iter()
        .chain(
            counters["right_occurrences"]
                .as_array()
                .expect("right occurrences"),
        )
    {
        unique.insert(
            occurrence["operation_ref"].as_str().expect("operation ref"),
            occurrence["kind"].as_str().expect("kind"),
        );
    }
    let actions = unique.values().filter(|kind| **kind == "action").count();
    let mutations = unique.values().filter(|kind| **kind == "mutation").count();
    let children = unique.values().filter(|kind| **kind == "grant").count();
    assert_eq!(unique.len(), 7);
    assert_eq!(actions, 3);
    assert_eq!(mutations, 2);
    assert_eq!(children, 2);
    assert_eq!(
        counters["expected_counts"],
        serde_json::json!({
            "actions": actions,
            "mutations": mutations,
            "consumptions": unique.len(),
            "direct_children": children,
        })
    );
    let occurrences = |side: &str| {
        counters[side]
            .as_array()
            .expect("occurrences")
            .iter()
            .map(|occurrence| SemanticOccurrence {
                operation_ref: occurrence["operation_ref"]
                    .as_str()
                    .expect("operation ref")
                    .to_owned(),
                kind: occurrence["kind"].as_str().expect("kind").to_owned(),
            })
            .collect::<Vec<_>>()
    };
    let recomposed = recompose_semantic_counts(
        &occurrences("left_occurrences"),
        &occurrences("right_occurrences"),
    )
    .expect("production counter recomposition");
    assert_eq!(
        serde_json::to_value(recomposed).expect("counts value"),
        counters["expected_counts"]
    );
    assert_eq!(counters["shared_prefix_counted_once"], true);
    assert_eq!(counters["branch_occurrence_omitted"], false);
    assert_eq!(counters["branch_occurrence_double_counted"], false);
}

#[test]
fn cb2_bundle_fresh_store_order_and_api_inventory_preliminary() {
    let vector = vector();
    let fresh = &vector["fresh_store"];
    assert_eq!(
        state_digest(&fresh["objects"]),
        fresh["expected_cold_digest"]
    );
    let object_keys: BTreeSet<_> = fresh["objects"]
        .as_object()
        .expect("fresh-store objects")
        .keys()
        .map(String::as_str)
        .collect();
    let cases = fresh["insertion_order_cases"]
        .as_array()
        .expect("insertion-order cases");
    assert_eq!(cases.len(), 6);
    for case in cases {
        let order: BTreeSet<_> = case["insertion_order"]
            .as_array()
            .expect("insertion order")
            .iter()
            .map(|value| value.as_str().expect("object key"))
            .collect();
        assert_eq!(order, object_keys);
        assert_eq!(case["cold_digest"], fresh["expected_cold_digest"]);
        assert_eq!(case["expected"], "accepted");
    }
    assert_eq!(fresh["producer_destroyed_before_verify"], true);
    assert_eq!(fresh["private_capabilities_absent_during_verify"], true);
    assert_eq!(fresh["network_participates"], false);
    assert_eq!(fresh["provider_cas_participates"], false);

    let objects = fresh["objects"]
        .as_object()
        .expect("objects")
        .iter()
        .map(|(path, value)| {
            (
                path.clone(),
                value.as_str().expect("object bytes").as_bytes().to_vec(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let orders = cases
        .iter()
        .map(|case| {
            case["insertion_order"]
                .as_array()
                .expect("insertion order")
                .iter()
                .map(|path| path.as_str().expect("path").to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        verify_insertion_order_independence(&objects, &orders)
            .expect("production insertion-order proof"),
        fresh["expected_cold_digest"]
    );

    assert!(MERGE_SOURCE.contains("pub fn edition_merge"));
    assert!(MERGE_SOURCE.contains("pub fn resolve_fork"));
    assert!(MERGE_SOURCE.contains("Owner(&'a OwnerKeys)"));
    assert!(MERGE_SOURCE.contains("Delegate {"));
    assert!(MANIFEST_SOURCE.contains("pub resolves_fork: String"));
    assert!(MANIFEST_SOURCE.contains("pub authorized_via: Vec<String>"));
    for present in [
        "pub fn merge_draft2_package",
        "pub fn cold_merge_from_keyless_store",
        "pub fn recompose_semantic_counts",
        "pub fn verify_insertion_order_independence",
    ] {
        assert!(
            MERGE_SOURCE.contains(present) || MANIFEST_SOURCE.contains(present),
            "{present}"
        );
    }
}
