//! CB5b delegated counters, receipt-v2, matcher and session-meter contracts.

use aithos_core::constraints::verify_max_sessions;
use aithos_core::delegated_counts::{verify_delegated_count_mandates, verify_delegated_counts};
use aithos_core::receipts::{
    obligation_matches, verify_obligation, verify_obligation_chain, verify_r2_receipt,
    verify_u1_receipt,
};
use aithos_core::Error;
use serde_json::{json, Value};

const D7_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-delegated-counts.json"
));
const RECEIPTS_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-receipts.json"
));
const SESSION_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-session-proof.json"
));

fn parsed(bytes: &str) -> Value {
    serde_json::from_str(bytes).expect("CB2 vector parses")
}

fn expect_delegated_counts_error(result: aithos_core::Result<impl Sized>, case: &str) {
    match result {
        Err(Error::InvalidDelegatedCounts(_)) => {}
        Err(other) => panic!("{case}: expected InvalidDelegatedCounts, got {other:?}"),
        Ok(_) => panic!("{case}: malformed delegated counts were accepted"),
    }
}

fn expect_invalid_mandate(result: aithos_core::Result<impl Sized>, case: &str) {
    match result {
        Err(Error::InvalidMandate(_)) => {}
        Err(other) => panic!("{case}: expected InvalidMandate, got {other:?}"),
        Ok(_) => panic!("{case}: malformed mandate material was accepted"),
    }
}

#[test]
fn cb5_delegated_counts_reach_the_typed_validator() {
    let vector = parsed(D7_BYTES);
    let positive = &vector["positive"];
    let verified = verify_delegated_counts(
        &positive["delegated_counts"],
        &positive["leaves"],
        &positive["evidence_views"],
    )
    .expect("positive delegated counts validate");

    assert_eq!(verified.occurrences().len(), 14);
    assert_eq!(
        verified
            .counts_for("mandate_01J00000000000000000000020")
            .expect("root counter")
            .mutations(),
        2
    );
    assert_eq!(
        verified
            .counts_for("mandate_01J00000000000000000000020")
            .expect("root counter")
            .consumptions(),
        14
    );
    assert_eq!(
        verified
            .counts_for("mandate_01J00000000000000000000022")
            .expect("child counter")
            .consumptions(),
        1
    );

    for case in vector["negative_counter_cases"]
        .as_array()
        .expect("counter negatives")
    {
        let candidate = &case["candidate"];
        expect_delegated_counts_error(
            verify_delegated_counts(
                &candidate["delegated_counts"],
                &candidate["leaves"],
                &candidate["evidence_views"],
            ),
            case["id"].as_str().expect("case id"),
        );
    }
}

#[test]
fn cb5_delegated_count_mandates_reach_the_typed_validator() {
    let vector = parsed(D7_BYTES);
    verify_delegated_count_mandates(&vector["positive"]["mandates"])
        .expect("positive draft3 counter chain validates");
    for case in vector["negative_mandate_cases"]
        .as_array()
        .expect("mandate negatives")
    {
        expect_invalid_mandate(
            verify_delegated_count_mandates(&case["candidate"]),
            case["id"].as_str().expect("case id"),
        );
    }
}

#[test]
fn cb5_max_sessions_lifecycle_reaches_the_typed_validator() {
    let vector = parsed(SESSION_BYTES);
    assert_eq!(
        vector["inventory"]["max_sessions_lifecycle_is_out_of_scope"],
        true
    );
    let key = vector["positive"]["certificate"]["key"]
        .as_str()
        .expect("session key");
    let verified = verify_max_sessions(1, &[key]).expect("one active session fits the cap");
    assert_eq!(verified.active(), 1);

    match verify_max_sessions(0, &[key]) {
        Err(Error::InvalidSession(_)) => {}
        other => panic!("spent max_sessions must be InvalidSession, got {other:?}"),
    }
    match verify_max_sessions(2, &[key, key]) {
        Err(Error::InvalidSession(_)) => {}
        other => panic!("duplicate active session must be InvalidSession, got {other:?}"),
    }
}

#[test]
fn cb5_operation_bound_receipts_reach_typed_validators() {
    let vector = parsed(RECEIPTS_BYTES);
    let positives = &vector["positive_receipts"];
    let contexts = &vector["contexts"];
    let obligations = &vector["obligations"];

    for (record, context, profile, obligation) in [
        (
            &positives["r2_without_presented_digest"],
            &contexts["action"],
            "1.0.0-draft.2",
            &obligations["action"],
        ),
        (
            &positives["r2_with_presented_digest"],
            &contexts["action"],
            "1.0.0-draft.2",
            &obligations["action"],
        ),
        (
            &positives["r2_draft3_mutation"],
            &contexts["mutation-ethos-edit"],
            "1.0.0-draft.3",
            &obligations["mutation"],
        ),
    ] {
        verify_r2_receipt(
            &json!([record["receipt"].clone()]),
            context,
            profile,
            obligation,
        )
        .expect("positive R2 receipt validates");
    }

    let action = verify_u1_receipt(
        &json!([positives["u1_action"]["receipt"].clone()]),
        &contexts["action"],
        &vector["budget_profile"],
    )
    .expect("positive U1 action receipt validates");
    let inference = verify_u1_receipt(
        &json!([positives["u1_inference"]["receipt"].clone()]),
        &contexts["inference"],
        &vector["budget_profile"],
    )
    .expect("positive U1 inference receipt validates");
    assert_eq!(action.actual_tokens(), 8412);
    assert_eq!(inference.actual_tokens(), 1500);

    for case in vector["negative_r2_cases"]
        .as_array()
        .expect("R2 negatives")
    {
        match verify_r2_receipt(
            &case["candidate"],
            &contexts["action"],
            "1.0.0-draft.2",
            &obligations["action"],
        ) {
            Err(Error::GammaObligationUnsatisfied(_)) => {}
            other => panic!("{}: unexpected R2 verdict {other:?}", case["id"]),
        }
    }
    for case in vector["negative_u1_cases"]
        .as_array()
        .expect("U1 negatives")
    {
        let context = if case["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("inference-"))
        {
            &contexts["inference"]
        } else {
            &contexts["action"]
        };
        match verify_u1_receipt(&case["candidate"], context, &vector["budget_profile"]) {
            Err(Error::InvalidGammaEntry(_)) => {}
            other => panic!("{}: unexpected U1 verdict {other:?}", case["id"]),
        }
    }
}

#[test]
fn cb5_draft3_obligation_matchers_reach_the_typed_validator() {
    let vector = parsed(RECEIPTS_BYTES);
    for case in vector["matcher_cases"].as_array().expect("matcher cases") {
        let obligation = json!({
            "id": case["id"],
            "check": "human.approve",
            "attestor": [vector["public_keys"]["attestor_a"].clone()],
            "applies_to_operation": case["matcher"].clone(),
            "verdict": "approve",
        });
        let verified =
            verify_obligation("1.0.0-draft.3", &obligation).expect("matcher shape validates");
        assert_eq!(
            obligation_matches(
                &verified,
                &vector["contexts"][case["context"].as_str().expect("context id")],
            )
            .expect("context reconstructs"),
            case["expected_applicable"]
                .as_bool()
                .expect("matcher verdict"),
            "{}",
            case["id"]
        );
    }
    verify_obligation_chain(&vector["draft3_obligation_chain"])
        .expect("positive draft3 matcher chain validates");

    for case in vector["negative_matcher_cases"]
        .as_array()
        .expect("matcher negatives")
    {
        expect_invalid_mandate(
            verify_obligation(
                case["candidate"]["profile"]
                    .as_str()
                    .expect("candidate profile"),
                &case["candidate"]["obligation"],
            ),
            case["id"].as_str().expect("case id"),
        );
    }
    for case in vector["negative_matcher_chain_cases"]
        .as_array()
        .expect("matcher chain negatives")
    {
        expect_invalid_mandate(
            verify_obligation_chain(&case["candidate"]),
            case["id"].as_str().expect("case id"),
        );
    }
}
