//! CB4 transitions for the seven API gates frozen by CB2.
//!
//! These tests consume only the independent CB2 vectors. The native leaf proof
//! domain below remains an explicit test fixture, not a promoted protocol wire.

use aithos_core::error::Error;
use aithos_core::ids::Sid;
use aithos_core::operation::{
    correlate_operation_references, verify_operation_facts, verify_operation_projection,
    verify_operation_reference, verify_session, verify_state_fact, MutationNode,
    OperationCorrelation, OperationFactsEvidence, OperationFactsInput, OperationProjectionEvidence,
    SessionEvidence, StateFactInput,
};
use aithos_core::path::Zone;
use serde_json::Value;

const MUTATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-mutation.json"
));
const READ: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-read.json"
));
const ACTION_INFERENCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-action-inference.json"
));
const STRUCTURAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-facts-structural.json"
));
const PROJECTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-operation-projection.json"
));
const SESSION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-session-proof.json"
));
const NATIVE_LEAF_TEST_DOMAIN: &[u8] = b"aithos-core/cb2/native-leaf-proof\0";

fn json(bytes: &str) -> Value {
    serde_json::from_str(bytes).expect("CB2 vector parses")
}

fn optional_object(value: &Value) -> Option<&Value> {
    value.is_object().then_some(value)
}

fn mutation_nodes(vector: &Value) -> Vec<MutationNode> {
    let sids = &vector["fixture_sids"];
    let sid = |name: &str| Sid::parse(sids[name].as_str().expect("fixture SID")).expect("SID");
    vec![
        MutationNode::new(sid("circle_root"), Zone::Circle, None),
        MutationNode::new(sid("circle_parent"), Zone::Circle, Some(sid("circle_root"))),
        MutationNode::new(sid("circle_destination"), Zone::Circle, None),
        MutationNode::new(
            sid("circle_destination_child"),
            Zone::Circle,
            Some(sid("circle_destination")),
        ),
        MutationNode::new(sid("self_root"), Zone::Self_, None),
        MutationNode::new(
            sid("ethos_target"),
            Zone::Circle,
            Some(sid("circle_parent")),
        ),
        MutationNode::new(
            sid("section_target"),
            Zone::Circle,
            Some(sid("circle_parent")),
        ),
        MutationNode::new(
            sid("folder_target"),
            Zone::Circle,
            Some(sid("circle_parent")),
        ),
        MutationNode::new(sid("folder_create"), Zone::Circle, None),
    ]
}

fn assert_operation_facts_error(result: aithos_core::Result<impl core::fmt::Debug>, id: &str) {
    assert!(
        matches!(result, Err(Error::InvalidOperationFacts(_))),
        "{id}: expected InvalidOperationFacts, got {result:?}"
    );
}

fn assert_state_fact_error(result: aithos_core::Result<impl core::fmt::Debug>, id: &str) {
    assert!(
        matches!(result, Err(Error::InvalidStateFact(_))),
        "{id}: expected InvalidStateFact, got {result:?}"
    );
}

#[test]
fn cb4_mutation_operation_facts_reach_the_typed_validator() {
    let vector = json(MUTATION);
    let nodes = mutation_nodes(&vector);
    let evidence = OperationFactsEvidence::Mutation {
        state_facts: &vector["states"],
        nodes: &nodes,
        vault_record_key: vector["vault_record_key"].as_str().expect("vault key"),
    };

    for case in vector["positive_cases"].as_array().expect("positive cases") {
        let verified = verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence,
        })
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        assert_eq!(verified.digest(), case["digest"].as_str().expect("digest"));
        assert_eq!(verified.kind(), "mutation");
    }

    for case in vector["negative_cases"]["operation_facts"]
        .as_array()
        .expect("operation negatives")
    {
        assert_operation_facts_error(
            verify_operation_facts(OperationFactsInput {
                document: &case["candidate"],
                facts_ref: optional_object(&case["facts_ref"]),
                evidence,
            }),
            case["id"].as_str().expect("case id"),
        );
    }
}

#[test]
fn cb4_state_facts_reach_the_typed_validator() {
    let vector = json(MUTATION);
    for fixture in vector["states"]
        .as_object()
        .expect("state fixtures")
        .values()
    {
        let expected_keys: Vec<String> = fixture["input_objects"]
            .as_array()
            .expect("state inputs")
            .iter()
            .map(|input| {
                input["key_commitment"]
                    .as_str()
                    .expect("key commitment")
                    .to_owned()
            })
            .collect();
        let verified = verify_state_fact(StateFactInput::Document {
            document: &fixture["document"],
            expected_key_commitments: Some(&expected_keys),
        })
        .expect("positive state fact");
        assert_eq!(
            verified.digest(),
            fixture["digest"].as_str().expect("digest")
        );
    }

    for case in vector["negative_cases"]["state_facts"]
        .as_array()
        .expect("state negatives")
    {
        let expected_keys: Option<Vec<String>> = case
            .get("expected_key_commitments")
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .map(|key| key.as_str().expect("expected key").to_owned())
                    .collect()
            });
        let input = match case["scope"].as_str().expect("state scope") {
            "logical_state" => StateFactInput::LogicalState {
                state: &case["candidate"],
                state_facts: None,
            },
            "state_document" => StateFactInput::Document {
                document: &case["candidate"],
                expected_key_commitments: expected_keys.as_deref(),
            },
            "state_reference" => StateFactInput::Reference {
                state: &case["candidate"]["logical_state"],
                document: &case["candidate"]["document"],
            },
            other => panic!("unknown state scope {other}"),
        };
        assert_state_fact_error(
            verify_state_fact(input),
            case["id"].as_str().expect("case id"),
        );
    }
}

#[test]
fn cb4_read_operation_facts_reach_the_typed_validator() {
    let vector = json(READ);
    for case in vector["positive_cases"].as_array().expect("positive cases") {
        let verified = verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::Read {
                context: &case["context"],
                fixtures: &vector["fixtures"],
            },
        })
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        assert_eq!(verified.digest(), case["digest"].as_str().expect("digest"));
        assert_eq!(verified.kind(), "read");
    }
    for case in vector["negative_cases"].as_array().expect("negatives") {
        assert_operation_facts_error(
            verify_operation_facts(OperationFactsInput {
                document: &case["candidate"],
                facts_ref: optional_object(&case["facts_ref"]),
                evidence: OperationFactsEvidence::Read {
                    context: &case["context"],
                    fixtures: &vector["fixtures"],
                },
            }),
            case["id"].as_str().expect("case id"),
        );
    }
}

#[test]
fn cb4_action_inference_facts_reach_the_typed_validator() {
    let vector = json(ACTION_INFERENCE);
    for case in vector["positive_cases"].as_array().expect("positive cases") {
        let verified = verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::ActionInference {
                context: &case["context"],
            },
        })
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        assert_eq!(verified.digest(), case["digest"].as_str().expect("digest"));
    }
    for case in vector["negative_cases"].as_array().expect("negatives") {
        assert_operation_facts_error(
            verify_operation_facts(OperationFactsInput {
                document: &case["candidate"],
                facts_ref: optional_object(&case["facts_ref"]),
                evidence: OperationFactsEvidence::ActionInference {
                    context: &case["context"],
                },
            }),
            case["id"].as_str().expect("case id"),
        );
    }
}

#[test]
fn cb4_structural_operation_facts_reach_the_typed_validator() {
    let vector = json(STRUCTURAL);
    for case in vector["positive_cases"].as_array().expect("positive cases") {
        let verified = verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::Structural {
                context: &case["context"],
            },
        })
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        assert_eq!(verified.digest(), case["digest"].as_str().expect("digest"));
    }
    for case in vector["negative_cases"].as_array().expect("negatives") {
        assert_operation_facts_error(
            verify_operation_facts(OperationFactsInput {
                document: &case["candidate"],
                facts_ref: optional_object(&case["facts_ref"]),
                evidence: OperationFactsEvidence::Structural {
                    context: &case["context"],
                },
            }),
            case["id"].as_str().expect("case id"),
        );
    }
}

fn projection_facts<'a>(mutation: &'a Value, structural: &'a Value) -> Vec<&'a Value> {
    mutation["positive_cases"]
        .as_array()
        .expect("mutation facts")
        .iter()
        .chain(
            structural["positive_cases"]
                .as_array()
                .expect("structural facts"),
        )
        .map(|case| &case["document"])
        .collect()
}

#[test]
fn cb4_projection_reference_and_correlation_reach_the_typed_validator() {
    let vector = json(PROJECTION);
    let mutation = json(MUTATION);
    let structural = json(STRUCTURAL);
    let facts = projection_facts(&mutation, &structural);
    let certificates = [&vector["fixtures"]["certificate"]];
    let evidence = OperationProjectionEvidence {
        facts_documents: &facts,
        certificates: &certificates,
    };

    for case in vector["positive_cases"].as_array().expect("positives") {
        let verified = verify_operation_projection(&case["projection"], evidence)
            .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        assert_eq!(verified.operation_ref(), &case["operation_ref"]);
        verify_operation_reference(&case["operation_ref"], &verified)
            .unwrap_or_else(|error| panic!("{} reference: {error}", case["id"]));
    }

    for case in vector["negative_projection_cases"]
        .as_array()
        .expect("projection negatives")
    {
        let result = verify_operation_projection(&case["candidate"], evidence);
        match case["must_fail"].as_str().expect("error kind") {
            "InvalidOperation" => assert!(
                matches!(result, Err(Error::InvalidOperation(_))),
                "{}: {result:?}",
                case["id"]
            ),
            "InvalidOperationFacts" => {
                assert_operation_facts_error(result, case["id"].as_str().expect("case id"))
            }
            other => panic!("unknown projection error {other}"),
        }
    }

    let positive = &vector["positive_cases"][1];
    let verified =
        verify_operation_projection(&positive["projection"], evidence).expect("valid projection");
    for case in vector["negative_reference_cases"]
        .as_array()
        .expect("reference negatives")
    {
        let result = verify_operation_reference(&case["candidate"], &verified);
        assert!(
            matches!(result, Err(Error::InvalidOperation(_))),
            "{}: {result:?}",
            case["id"]
        );
    }

    for case in vector["correlation_cases"]
        .as_array()
        .expect("correlation cases")
    {
        let result = correlate_operation_references(&case["first"], &case["second"]);
        if case["must_fail"] == "InvalidOperation" {
            assert!(
                matches!(result, Err(Error::InvalidOperation(_))),
                "{}: {result:?}",
                case["id"]
            );
        } else {
            let expected = match case["verdict"].as_str().expect("verdict") {
                "correlated" => OperationCorrelation::Correlated,
                "distinct" => OperationCorrelation::Distinct,
                other => panic!("unknown correlation verdict {other}"),
            };
            assert_eq!(result.expect("valid correlation"), expected);
        }
    }
}

fn session_evidence<'a>(candidate: &'a Value) -> SessionEvidence<'a> {
    SessionEvidence {
        mandate: &candidate["mandate"],
        certificate: &candidate["certificate"],
        projection: &candidate["operation_projection"],
        operation_ref: &candidate["operation_ref"],
        native_leaf_proof: candidate.get("native_leaf_proof_fixture"),
        native_leaf_domain: NATIVE_LEAF_TEST_DOMAIN,
        session_proof: candidate.get("session_proof"),
    }
}

#[test]
fn cb4_session_bundle_reaches_the_typed_validator() {
    let vector = json(SESSION);
    let verified = verify_session(session_evidence(&vector["positive"])).expect("valid SC1 bundle");
    assert_eq!(
        verified.operation_ref(),
        &vector["positive"]["operation_ref"]
    );

    for case in vector["negative_cases"].as_array().expect("negatives") {
        let result = verify_session(session_evidence(&case["candidate"]));
        assert!(
            matches!(result, Err(Error::InvalidSession(_))),
            "{}: {result:?}",
            case["id"]
        );
    }
}
