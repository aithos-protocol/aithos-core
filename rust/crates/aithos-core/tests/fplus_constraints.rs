//! Conformance vector F+ — absolute windows, budget profiles, attestation
//! receipts (spec 04.10, 04.11). Verdicts computed independently with
//! Python datetime + PyNaCl.

use aithos_core::constraints::{
    check_budgets, entry_tokens, parse_budgets, tally_tokens, verify_receipt, Window,
};
use aithos_core::did::SignatureBlock;
use aithos_core::error::Error;
use aithos_core::gamma::{ts_epoch, Entry};
use serde_json::Value;

fn vector() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/fplus-constraints.json"
    )))
    .expect("valid vector json")
}

const MANDATE: &str = "mandate_00000000000000000000FPLUS";

fn entry(kind: &str, at: &str, payload: Value) -> Entry {
    Entry {
        v: 1,
        id: "gamma_00000000000000000000000FPP".into(),
        prev: String::new(),
        at: at.into(),
        kind: kind.into(),
        target: Some("x.llm".into()),
        authorized_by: Some(MANDATE.into()),
        authorized_via: Some(vec![MANDATE.into()]),
        payload: Some(payload),
        body_enc: None,
        signature: SignatureBlock {
            alg: "ed25519".into(),
            key: "unsigned-fixture".into(),
            value: String::new(),
        },
    }
}

fn ledger(v: &Value) -> Vec<Entry> {
    v["budgets"]["ledger"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let mut payload = e.clone();
            let kind = payload["kind"].as_str().unwrap().to_owned();
            payload.as_object_mut().unwrap().remove("kind");
            entry(&kind, "2026-07-03T10:00:00Z", payload)
        })
        .collect()
}

fn profiles(v: &Value) -> Vec<aithos_core::constraints::BudgetProfile> {
    parse_budgets(&serde_json::json!({"budgets": v["budgets"]["profiles"]}))
        .unwrap()
        .unwrap()
}

#[test]
fn window_verdicts_match_python() {
    let v = vector();
    let w = Window::from_json(&v["windows"]["window"]).unwrap();
    for (at, expected) in v["windows"]["verdicts"].as_object().unwrap() {
        assert_eq!(
            w.contains(ts_epoch(at).unwrap()),
            expected.as_bool().unwrap(),
            "window verdict at {at}"
        );
    }
    for (name, obj) in v["windows"]["bounded"].as_object().unwrap() {
        let mut spec = v["windows"]["window"].clone();
        for bound in ["until", "count"] {
            if let Some(x) = obj.get(bound) {
                spec[bound] = x.clone();
            }
        }
        let w = Window::from_json(&spec).unwrap();
        for (at, expected) in obj.as_object().unwrap() {
            if at == "until" || at == "count" {
                continue;
            }
            assert_eq!(
                w.contains(ts_epoch(at).unwrap()),
                expected.as_bool().unwrap(),
                "bounded {name} at {at}"
            );
        }
    }
}

#[test]
fn token_tallies_match_python() {
    let v = vector();
    let entries = ledger(&v);
    let consumed = tally_tokens(&entries, MANDATE, "gemma");
    assert_eq!(
        consumed,
        v["budgets"]["expected"]["gemma_tokens_consumed"]
            .as_u64()
            .unwrap()
    );
}

#[test]
fn budget_verdicts_match_python() {
    let v = vector();
    let entries = ledger(&v);
    let profiles = profiles(&v);
    let at_ok = "2026-07-02T15:00:00Z"; // inside the haiku Thursday window

    // Over budget vs headroom on gemma.
    let over = entry(
        "inference",
        at_ok,
        serde_json::json!({"budget_ref":"gemma","model":"gemma","tokens_in":5000,"tokens_out":0}),
    );
    assert!(matches!(
        check_budgets(&entries, &over, MANDATE, &profiles),
        Err(Error::GammaBudgetExhausted(_))
    ));
    let fits = entry(
        "inference",
        at_ok,
        serde_json::json!({"budget_ref":"gemma","model":"gemma","tokens_in":4000,"tokens_out":0}),
    );
    check_budgets(&entries, &fits, MANDATE, &profiles).unwrap();

    // Model not allowed on haiku.
    let wrong_model = entry(
        "action",
        at_ok,
        serde_json::json!({"budget_ref":"haiku","model":"gpt-oss","action":"reply","tokens":10}),
    );
    assert!(matches!(
        check_budgets(&entries, &wrong_model, MANDATE, &profiles),
        Err(Error::GammaBudgetExhausted(_))
    ));

    // Outside the haiku profile window.
    let off_window = entry(
        "action",
        "2026-07-03T14:30:00Z",
        serde_json::json!({"budget_ref":"haiku","model":"claude-haiku","action":"reply","tokens":10}),
    );
    assert!(matches!(
        check_budgets(&entries, &off_window, MANDATE, &profiles),
        Err(Error::GammaBudgetExhausted(_))
    ));

    // Unknown and missing budget_ref.
    let unknown = entry(
        "action",
        at_ok,
        serde_json::json!({"budget_ref":"grok-unlimited","action":"reply"}),
    );
    assert!(check_budgets(&entries, &unknown, MANDATE, &profiles).is_err());
    let missing = entry("action", at_ok, serde_json::json!({"action":"reply"}));
    assert!(check_budgets(&entries, &missing, MANDATE, &profiles).is_err());
}

#[test]
fn attestation_verdicts_match_python() {
    let v = vector();
    let att = &v["attestation"];
    let pub_hex = att["provider_pub_hex"].as_str().unwrap();
    let key_mb = aithos_core::wire::ed25519_pub_to_multibase(
        &hex::decode(pub_hex).unwrap().try_into().unwrap(),
    );
    let profile = parse_budgets(&serde_json::json!({"budgets": [{
        "id": "haiku", "require_attestation": true, "attestation_key": key_mb
    }]}))
    .unwrap()
    .unwrap()
    .remove(0);

    let good = entry(
        "action",
        "2026-07-02T15:00:00Z",
        serde_json::json!({
            "budget_ref": "haiku", "action": "reply", "model": "claude-haiku",
            "args_hash": att["args_hash"], "tokens": 1,
            "receipt": att["receipt"],
        }),
    );
    verify_receipt(&good, &profile).unwrap();
    // Attested tokens override the declaration in tallies.
    assert_eq!(
        entry_tokens(&good),
        att["expected"]["attested_tokens_override"]
            .as_u64()
            .unwrap()
    );

    // Wrong signer.
    let mut bad = good.clone();
    bad.payload.as_mut().unwrap()["receipt"]["sig"] =
        att["expected"]["wrong_signer_sig_hex"].clone();
    assert!(matches!(
        verify_receipt(&bad, &profile),
        Err(Error::InvalidGammaEntry(_))
    ));

    // Replay on another action's args.
    let mut replay = good.clone();
    replay.payload.as_mut().unwrap()["args_hash"] = serde_json::json!(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert!(matches!(
        verify_receipt(&replay, &profile),
        Err(Error::InvalidGammaEntry(_))
    ));
}
