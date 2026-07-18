//! CB6 typed Gamma-v2 admission, profile and occurrence contracts.

use aithos_core::gamma_v2::{
    verify_gamma_profile_transition, verify_gamma_v2_entry, GammaOccurrenceRegistry,
};
use aithos_core::Error;
use serde_json::Value;

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-gamma-v2-replay.json"
));

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 Gamma-v2 vector parses")
}

#[test]
fn cb6_gamma_v2_entries_reach_the_typed_admission_gate() {
    let vector = vector();
    for case in vector["kind_cases"].as_array().expect("kind cases") {
        let projection = (!case["projection"].is_null()).then_some(&case["projection"]);
        let verified = verify_gamma_v2_entry(&case["entry"], projection)
            .unwrap_or_else(|error| panic!("{}: {error}", case["kind"]));
        assert_eq!(verified.kind(), case["kind"].as_str().expect("kind"));
        assert_eq!(
            verified.operation_ref().is_some(),
            case["operation_ref_presence"] == "required"
        );
    }

    for case in vector["negative_entry_cases"]
        .as_array()
        .expect("entry negatives")
    {
        let projection = case["candidate"]
            .get("projection")
            .filter(|projection| !projection.is_null());
        match verify_gamma_v2_entry(&case["candidate"]["entry"], projection) {
            Err(Error::InvalidGammaEntry(_)) => {}
            other => panic!("{}: unexpected verdict {other:?}", case["id"]),
        }
    }
    for case in vector["negative_correlation_cases"]
        .as_array()
        .expect("correlation negatives")
    {
        match verify_gamma_v2_entry(
            &case["candidate"]["entry"],
            Some(&case["candidate"]["projection"]),
        ) {
            Err(Error::InvalidOperation(_)) => {}
            other => panic!("{}: unexpected verdict {other:?}", case["id"]),
        }
    }
}

#[test]
fn cb6_profile_edges_and_occurrences_reach_typed_state() {
    let vector = vector();
    for case in vector["monotonicity_cases"]
        .as_array()
        .expect("monotonicity cases")
    {
        assert_eq!(
            verify_gamma_profile_transition(
                case["parent_manifest"].as_str().expect("parent manifest"),
                case["parent_gamma"].as_str().expect("parent gamma"),
                case["child_manifest"].as_str().expect("child manifest"),
                case["child_gamma"].as_str().expect("child gamma"),
            )
            .is_ok(),
            case["expected_accepted"].as_bool().expect("edge verdict")
        );
    }

    let first = vector["kind_cases"]
        .as_array()
        .expect("kind cases")
        .iter()
        .find(|case| case["kind"] == "action")
        .expect("action case");
    let first = verify_gamma_v2_entry(&first["entry"], Some(&first["projection"]))
        .expect("seed action validates");
    let mut registry = GammaOccurrenceRegistry::default();
    registry.admit(&first).expect("seed occurrence is new");
    let before = registry.len();

    for case in vector["occurrence_cases"]
        .as_array()
        .expect("occurrence cases")
    {
        let result = registry.admit_reference(&case["operation_ref"]);
        match case["expected"]
            .as_str()
            .expect("expected occurrence verdict")
        {
            "refused-as-replay-before-tally" | "refused-as-equivocation-before-tally" => {
                assert!(matches!(result, Err(Error::InvalidOperation(_))));
                assert_eq!(registry.len(), before);
            }
            "accepted-as-distinct-occurrence" => {
                result.expect("distinct occurrence is admitted");
                assert_eq!(registry.len(), before + 1);
            }
            other => panic!("unknown occurrence verdict {other}"),
        }
    }
}
