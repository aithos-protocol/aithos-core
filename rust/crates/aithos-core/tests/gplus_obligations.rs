//! Conformance vector G+ — obligation receipts (spec 04.12): one wire
//! shape, guardrail / human approval / owner co_sign instances, replay
//! negatives, add-only attenuation. Every expected value computed
//! independently in Python (hashlib + PyNaCl + base58).

use aithos_core::constraints::{check_obligations, obligations_attenuate, parse_obligations};
use aithos_core::did::SignatureBlock;
use aithos_core::error::Error;
use aithos_core::gamma::Entry;
use serde_json::{json, Value};

fn vector() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/gplus-obligations.json"
    )))
    .expect("valid vector json")
}

fn entry(v: &Value, checks: Option<Value>) -> Entry {
    let e = &v["receipts"]["entry"];
    let mut payload = json!({
        "action": e["action"], "args_hash": e["args_hash"],
    });
    if let Some(c) = checks {
        payload["checks"] = json!([c]);
    }
    Entry {
        v: 1,
        id: "gamma_00000000000000000000000GPP".into(),
        prev: String::new(),
        at: e["at"].as_str().unwrap().into(),
        kind: "action".into(),
        target: Some("x.social".into()),
        authorized_by: Some(e["authorized_by"].as_str().unwrap().into()),
        authorized_via: Some(vec![e["authorized_by"].as_str().unwrap().into()]),
        payload: Some(payload),
        body_enc: None,
        signature: SignatureBlock {
            alg: "ed25519".into(),
            key: "unsigned-fixture".into(),
            value: String::new(),
        },
    }
}

/// Constraints carrying the vector's declared obligations.
fn constraints_with(v: &Value, which: &[&str]) -> Value {
    let obs: Vec<Value> = which
        .iter()
        .map(|k| v["receipts"]["obligations"][k].clone())
        .collect();
    json!({ "obligations": obs })
}

fn owner_key(v: &Value) -> String {
    v["keys"]["owner_content_pub_multibase"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn payload_jcs_matches_python() {
    let v = vector();
    for (name, ob_id, verdict) in [
        ("fresh_approval", "publish-approval", "approve"),
        ("guardrail_pass_aged_no_max_age", "pii-guard", "pass"),
    ] {
        let r = &v["receipts"][name];
        let check = &r["check"];
        let e = &v["receipts"]["entry"];
        let mut payload = json!({
            "obligation": ob_id,
            "mandate_id": e["authorized_by"],
            "action": e["action"],
            "args_hash": check["args_hash"],
            "verdict": verdict,
            "at": check["at"],
        });
        if let Some(d) = check.get("presented_digest") {
            payload["presented_digest"] = d.clone();
        }
        let bytes = aithos_core::jcs::canonical_bytes(&payload).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r["payload_jcs"].as_str().unwrap(),
            "JCS drift vs Python for {name}"
        );
    }
}

#[test]
fn valid_receipts_discharge() {
    let v = vector();
    for name in [
        "fresh_approval",
        "by_second_approver",
        "without_presented_digest",
        "ahead_of_entry_clock",
    ] {
        let e = entry(&v, Some(v["receipts"][name]["check"].clone()));
        check_obligations(&e, &constraints_with(&v, &["approval"]), &owner_key(&v))
            .unwrap_or_else(|x| panic!("{name} should discharge: {x}"));
    }
    let e = entry(
        &v,
        Some(v["receipts"]["guardrail_pass_aged_no_max_age"]["check"].clone()),
    );
    check_obligations(&e, &constraints_with(&v, &["guardrail"]), &owner_key(&v))
        .expect("aged guardrail pass without max_age discharges");
}

#[test]
fn co_sign_owner_desugars_to_the_same_wire() {
    let v = vector();
    let r = &v["receipts"]["co_sign_owner_send"];
    let mut e = entry(&v, Some(r["check"].clone()));
    e.target = Some("x.gmail".into());
    let p = e.payload.as_mut().unwrap();
    p["action"] = r["entry_action"].clone();
    p["args_hash"] = r["entry_args_hash"].clone();
    check_obligations(&e, &json!({"counter_sign": ["send"]}), &owner_key(&v))
        .expect("owner co_sign discharges counter_sign");
    // and without the receipt, the binding action is refused
    let mut bare = entry(&v, None);
    bare.target = Some("x.gmail".into());
    let p = bare.payload.as_mut().unwrap();
    p["action"] = r["entry_action"].clone();
    p["args_hash"] = r["entry_args_hash"].clone();
    let refused = check_obligations(&bare, &json!({"counter_sign": ["send"]}), &owner_key(&v));
    assert!(matches!(refused, Err(Error::GammaObligationUnsatisfied(_))));
}

#[test]
fn negatives_are_refused() {
    let v = vector();
    let cases: &[(&str, &[&str])] = &[
        ("stale_receipt", &["approval"]),
        ("guardrail_block_verdict", &["guardrail"]),
        ("bound_to_other_args", &["approval"]),
        ("sibling_mandate_id", &["approval"]),
        ("cross_action", &["approval"]),
        ("stranger_key", &["approval"]),
        ("presented_digest_swapped", &["approval"]),
    ];
    for (name, obs) in cases {
        let check = v["receipts"]["negatives"][*name]["check"].clone();
        let e = entry(&v, Some(check));
        let got = check_obligations(&e, &constraints_with(&v, obs), &owner_key(&v));
        assert!(
            matches!(got, Err(Error::GammaObligationUnsatisfied(_))),
            "{name}: expected GammaObligationUnsatisfied, got {got:?}"
        );
    }
    // missing receipt: no checks at all
    let e = entry(&v, None);
    let got = check_obligations(&e, &constraints_with(&v, &["approval"]), &owner_key(&v));
    assert!(matches!(got, Err(Error::GammaObligationUnsatisfied(_))));
}

#[test]
fn attenuation_is_add_only() {
    let v = vector();
    let parent = json!({ "obligations": v["attenuation"]["parent_obligations"] });
    for (name, expected_ok) in [
        ("child_adds", true),
        ("child_drops", false),
        ("child_loosens", false),
    ] {
        let child = json!({ "obligations": v["attenuation"][name]["obligations"] });
        let got = obligations_attenuate(&parent, &child);
        assert_eq!(
            got.is_ok(),
            expected_ok,
            "{name}: expected {} got {got:?}",
            v["attenuation"][name]["expected"]
        );
        if !expected_ok {
            assert!(matches!(got, Err(Error::InvalidMandate(_))));
        }
    }
}

#[test]
fn out_of_scope_actions_need_no_receipt() {
    let v = vector();
    // approval gates act.x.social.publish; a gmail send rides free
    let mut e = entry(&v, None);
    e.target = Some("x.gmail".into());
    e.payload.as_mut().unwrap()["action"] = json!("send");
    check_obligations(&e, &constraints_with(&v, &["approval"]), &owner_key(&v))
        .expect("out-of-scope action needs no receipt");
}

#[test]
fn desugared_obligations_parse() {
    let v = vector();
    let obs = parse_obligations(
        &json!({"counter_sign": ["send"], "binding": ["wire_transfer"]}),
        &owner_key(&v),
    )
    .unwrap();
    assert_eq!(obs.len(), 2);
    assert!(obs.iter().all(|o| o.id == "co_sign"));
    assert!(obs.iter().all(|o| o.verdict == "approve"));
    assert!(obs.iter().all(|o| o.max_age == Some(300)));
}
