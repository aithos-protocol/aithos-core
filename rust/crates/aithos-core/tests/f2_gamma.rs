//! Conformance vector F2 — the agentic meter (spec 07.4): subtree counts,
//! windows, per-action rate, children, unlogged grants. Expected counts and
//! verdicts computed independently in Python.

use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::gamma::{
    check_action_append, check_grant_append, count_actions, count_children, grant_logged, ts_epoch,
    Entry,
};
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::Mandate;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct F2 {
    seed_hex: String,
    root_mandate_jcs: String,
    leaf_mandate_jcs: String,
    ghost_mandate_jcs: String,
    entries_jcs: Vec<String>,
    expected: Value,
}

fn vector() -> F2 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/f2-gamma-counting.json"
    )))
    .expect("valid vector json")
}

struct Fixture {
    root: Mandate,
    leaf: Mandate,
    ghost: Mandate,
    entries: Vec<Entry>,
    expected: Value,
    doc: DidDocument,
}

fn fixture() -> Fixture {
    let v = vector();
    let seed: [u8; 32] = hex::decode(&v.seed_hex).unwrap().try_into().unwrap();
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(seed));
    let succession = succession_from_entropy([9u8; 32]);
    Fixture {
        root: serde_json::from_str(&v.root_mandate_jcs).unwrap(),
        leaf: serde_json::from_str(&v.leaf_mandate_jcs).unwrap(),
        ghost: serde_json::from_str(&v.ghost_mandate_jcs).unwrap(),
        entries: v
            .entries_jcs
            .iter()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect(),
        expected: v.expected,
        doc: DidDocument::build(&owner, &succession.verifying_key(), vec![], String::new())
            .unwrap(),
    }
}

fn expect_n(f: &Fixture, key: &str) -> usize {
    f.expected[key].as_u64().unwrap() as usize
}

/// A candidate action entry: counting inspects fields, not signatures.
fn candidate(at: &str, via: &[&Mandate], action: &str) -> Entry {
    Entry {
        v: 1,
        id: "gamma_000000000000000000000000ZZ".into(),
        prev: String::new(),
        at: at.into(),
        kind: "action".into(),
        target: Some("x.gmail".into()),
        authorized_by: Some(via.last().unwrap().id.clone()),
        authorized_via: Some(via.iter().map(|m| m.id.clone()).collect()),
        payload: Some(serde_json::json!({"action": action})),
        body_enc: None,
        signature: aithos_core::did::SignatureBlock {
            alg: "ed25519".into(),
            key: "unsigned-fixture".into(),
            value: String::new(),
        },
    }
}

#[test]
fn subtree_counts_match_python() {
    let f = fixture();
    assert_eq!(
        count_actions(&f.entries, &f.root.id, None, None),
        expect_n(&f, "actions_via_root")
    );
    assert_eq!(
        count_actions(&f.entries, &f.leaf.id, None, None),
        expect_n(&f, "actions_via_leaf")
    );
    assert_eq!(
        count_children(&f.entries, &f.root.id),
        expect_n(&f, "children_of_root")
    );
    assert!(grant_logged(&f.entries, &f.leaf.id));
    assert!(!grant_logged(&f.entries, &f.ghost.id));
}

#[test]
fn windowed_counts_match_python() {
    let f = fixture();
    let day = 86_400;
    for (key, window, action) in [
        ("root_window_24h_at_2026-07-02T23:59:59Z", day, None),
        ("root_window_24h_at_2026-07-03T23:59:59Z", day, None),
        (
            "root_replies_window_72h_at_2026-07-03T00:00:00Z",
            3 * day,
            Some("reply"),
        ),
    ] {
        let at = key.rsplit("_at_").next().unwrap();
        assert_eq!(
            count_actions(
                &f.entries,
                &f.root.id,
                action,
                Some((window, ts_epoch(at).unwrap()))
            ),
            expect_n(&f, key),
            "window count {key}"
        );
    }
}

#[test]
fn spent_budgets_fail_closed() {
    let f = fixture();
    // max_actions 3 on root is spent — the 4th action fails via either key.
    let c = candidate("2026-07-10T00:00:00Z", &[&f.root], "label");
    assert!(matches!(
        check_action_append(&f.entries, &c, std::slice::from_ref(&f.root), &f.doc),
        Err(Error::GammaBudgetExhausted(_))
    ));
    let c = candidate("2026-07-10T00:00:00Z", &[&f.root, &f.leaf], "reply");
    assert!(matches!(
        check_action_append(&f.entries, &c, &[f.root.clone(), f.leaf.clone()], &f.doc),
        Err(Error::GammaBudgetExhausted(_))
    ));
    // max_children 1 on root is spent.
    assert!(matches!(
        check_grant_append(&f.entries, &f.root),
        Err(Error::GammaBudgetExhausted(_))
    ));
}

#[test]
fn windowed_budgets_replenish() {
    let f = fixture();
    // Drop the third root-subtree action so only the window logic decides.
    let trimmed: Vec<Entry> = f.entries[..3].to_vec();
    // Two actions sit on 2026-07-02 (01:00, 02:00). Inside the 24h window a
    // third is one too many; a day later the window is clear again.
    let same_day = candidate("2026-07-02T20:00:00Z", &[&f.root], "label");
    assert!(matches!(
        check_action_append(&trimmed, &same_day, std::slice::from_ref(&f.root), &f.doc),
        Err(Error::GammaBudgetExhausted(_))
    ));
    let next_day = candidate("2026-07-04T02:00:01Z", &[&f.root], "label");
    check_action_append(&trimmed, &next_day, std::slice::from_ref(&f.root), &f.doc).unwrap();
}

#[test]
fn per_action_rate_counts_only_its_kind() {
    let f = fixture();
    let trimmed: Vec<Entry> = f.entries[..3].to_vec(); // two replies logged
                                                       // At 07-03T03:00 both logged actions fall outside the 24h
                                                       // max_actions_per window (T-24h, T]; only the 72h reply rate refuses.
    let third_reply = candidate("2026-07-03T03:00:00Z", &[&f.root], "reply");
    assert!(matches!(
        check_action_append(
            &trimmed,
            &third_reply,
            std::slice::from_ref(&f.root),
            &f.doc
        ),
        Err(Error::GammaBudgetExhausted(_))
    ));
    // Another action kind passes the rate limit (but must clear the 24h
    // max_actions_per window — pick an instant outside it).
    let label = candidate("2026-07-03T02:00:01Z", &[&f.root], "label");
    check_action_append(&trimmed, &label, std::slice::from_ref(&f.root), &f.doc).unwrap();
}

#[test]
fn unlogged_grant_kills_the_chain() {
    let f = fixture();
    let c = candidate("2026-07-02T03:00:00Z", &[&f.root, &f.ghost], "reply");
    assert!(matches!(
        check_action_append(&f.entries, &c, &[f.root.clone(), f.ghost.clone()], &f.doc),
        Err(Error::GammaGrantNotLogged(_))
    ));
}
