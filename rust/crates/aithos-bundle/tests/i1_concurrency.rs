//! Conformance vector I1 — concurrency (spec 02.6 + 07.6): deterministic
//! disjoint merge ordering, the two-predecessor merge entry, the merged
//! segment layout, recommitted gamma roots, the 3-way index merge by sid
//! and the same-sid conflict negative. Every expected value computed
//! independently in Python (vectors/gen-i.py, anchored on committed
//! B2 + H2 + F2).

use aithos_bundle::bundle::ZoneIndex;
use aithos_bundle::manifest::Manifest;
use aithos_bundle::merge::{merge_segment_lines, merge_zone_index};
use aithos_core::error::Error;
use aithos_core::gamma::{head, segment_root, verify_links, Entry};
use serde_json::Value;

fn vector() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/i1-concurrency.json"
    )))
    .expect("valid vector json")
}

fn tree(v: &Value) -> &Value {
    &v["tree"]
}

fn segment_lines(v: &Value) -> Vec<String> {
    tree(v)["merge"]["merged_segment"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_owned())
        .collect()
}

/// (low, high) branch keys by ascending edition hash — the ordering the
/// wire pins; the test must not assume which fixture branch drew the
/// lower hash.
fn low_high_branches(v: &Value) -> (&'static str, &'static str) {
    let ha = tree(v)["branch_a"]["edition_hash"].as_str().unwrap();
    let hb = tree(v)["branch_b"]["edition_hash"].as_str().unwrap();
    if ha < hb {
        ("branch_a", "branch_b")
    } else {
        ("branch_b", "branch_a")
    }
}

/// Parent ordering: ascending edition hash, `prev_hash` = the lowest,
/// `merges` both ascending — and our chain-hash convention reproduces the
/// Python hashes byte-for-byte (signature.value blanked, JCS).
#[test]
fn parent_ordering_matches_python() {
    let v = vector();
    for branch in ["branch_a", "branch_b"] {
        let m: Manifest = serde_json::from_value(tree(&v)[branch]["manifest"].clone()).unwrap();
        assert_eq!(
            m.chain_hash().unwrap(),
            tree(&v)[branch]["edition_hash"].as_str().unwrap(),
            "{branch} edition hash"
        );
    }
    let (ha, hb) = (
        tree(&v)["branch_a"]["edition_hash"].as_str().unwrap(),
        tree(&v)["branch_b"]["edition_hash"].as_str().unwrap(),
    );
    let mut ascending = vec![ha, hb];
    ascending.sort_unstable();
    let merges: Vec<&str> = tree(&v)["merge"]["merges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(merges, ascending, "merges list both parents ascending");
    assert_eq!(
        tree(&v)["merge"]["prev_hash_parent"].as_str().unwrap(),
        ascending[0],
        "prev_hash pins the lowest parent"
    );
}

/// The merge entry: well-formed (prevs discipline, payload mirror), and its
/// chain hash is the pinned gamma head.
#[test]
fn merge_entry_matches_python() {
    let v = vector();
    let entry_jcs = tree(&v)["merge"]["entry_jcs"].as_str().unwrap();
    let e: Entry = serde_json::from_str(entry_jcs).unwrap();
    e.check_form().expect("the merge entry is well-formed");
    assert_eq!(e.kind, "merge");
    assert_eq!(
        aithos_core::jcs::canonicalize(&e).unwrap(),
        entry_jcs,
        "round-trips to the exact vector bytes"
    );
    assert_eq!(
        e.chain_hash().unwrap(),
        tree(&v)["merge"]["gamma_head"].as_str().unwrap(),
        "the merge edition pins the merge entry"
    );
    let prevs = e.prevs.clone().unwrap();
    assert_eq!(prevs[0], e.prev, "prev repeats the low tip");
    let (lo, hi) = low_high_branches(&v);
    assert_eq!(
        prevs,
        vec![
            tree(&v)[lo]["head"].as_str().unwrap().to_owned(),
            tree(&v)[hi]["head"].as_str().unwrap().to_owned(),
        ],
        "prevs cite both sub-chain tips ordered like merges (low parent first)"
    );
}

/// The merged layout: shared prefix, sub-chain LOW, sub-chain HIGH, merge
/// entry — reproduced from the two branch segment files; recommitted root
/// and count land on the Python values; the whole chain verifies through
/// the join; dropping the join is an unresolved fork.
#[test]
fn merged_segment_matches_python() {
    let v = vector();
    let lines = segment_lines(&v);
    let n = lines.len();

    // Rebuild the two branch files: shared prefix + each branch's entry.
    let prefix: Vec<Vec<u8>> = lines[..n - 3]
        .iter()
        .map(|l| l.clone().into_bytes())
        .collect();
    let (lo_key, hi_key) = low_high_branches(&v);
    let lo_entry = tree(&v)[lo_key]["entry_jcs"].as_str().unwrap();
    let hi_entry = tree(&v)[hi_key]["entry_jcs"].as_str().unwrap();
    let mut lo = prefix.clone();
    lo.push(lo_entry.as_bytes().to_vec());
    let mut hi = prefix;
    hi.push(hi_entry.as_bytes().to_vec());

    let mut merged = merge_segment_lines(&lo, &hi);
    merged.push(
        tree(&v)["merge"]["entry_jcs"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec(),
    );
    let expected: Vec<Vec<u8>> = lines.iter().map(|l| l.clone().into_bytes()).collect();
    assert_eq!(merged, expected, "deterministic merged layout, exact bytes");

    let refs: Vec<&[u8]> = merged.iter().map(Vec::as_slice).collect();
    assert_eq!(
        hex::encode(segment_root(&refs)),
        tree(&v)["merge"]["merged_segment_root_hex"]
            .as_str()
            .unwrap(),
        "recommitted segment root"
    );
    assert_eq!(
        refs.len() as u64,
        tree(&v)["merge"]["merged_segment_n"].as_u64().unwrap(),
        "recommitted segment count"
    );

    let entries: Vec<Entry> = lines
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    verify_links(&entries).expect("the merged chain verifies through the join");
    assert_eq!(
        head(&entries).unwrap(),
        tree(&v)["merge"]["gamma_head"].as_str().unwrap()
    );

    // Withhold the join: two open tips = an unresolved fork, refused.
    let unjoined = &entries[..entries.len() - 1];
    assert!(matches!(
        verify_links(unjoined),
        Err(Error::InvalidGammaChain(_))
    ));
}

/// The 3-way index merge by sid: union, deletions hold, sid order — JCS
/// byte-identical to Python; the same sid changed on both sides is a fork.
#[test]
fn index_merge_matches_python() {
    let v = vector();
    let im = &tree(&v)["index_merge"];
    let base: ZoneIndex = serde_json::from_value(im["base"].clone()).unwrap();
    let a: ZoneIndex = serde_json::from_value(im["branch_a"].clone()).unwrap();
    let b: ZoneIndex = serde_json::from_value(im["branch_b"].clone()).unwrap();
    let merged = merge_zone_index(&base, &a, &b).unwrap();
    assert_eq!(
        aithos_core::jcs::canonicalize(&merged).unwrap(),
        im["merged_jcs"].as_str().unwrap(),
        "merged index bytes match Python"
    );

    // The negative: both branches retitle the SAME sid differently.
    let retitle = |idx: &ZoneIndex, title: &str| -> ZoneIndex {
        let mut out = idx.clone();
        out.sections[0].title = title.to_owned();
        out
    };
    let conflict = merge_zone_index(&base, &retitle(&base, "A!"), &retitle(&base, "B!"));
    match conflict {
        Err(Error::EditionFork(msg)) => {
            assert!(
                msg.contains(&base.sections[0].sid),
                "the fork names the conflicting sid"
            );
        }
        other => panic!("same-sid conflict must be a fork, got {other:?}"),
    }
}
