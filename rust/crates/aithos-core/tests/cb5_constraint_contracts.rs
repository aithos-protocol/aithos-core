//! CB5a operation-side constraint contracts.
//!
//! A root mandate may preserve an unknown extension for forward compatibility,
//! but a current verifier must never turn that preserved value into authority.

use aithos_core::constraints::verify_operation_constraints;
use aithos_core::Error;
use serde_json::Value;

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-mandate-contracts.json"
));

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 mandate vector parses")
}

fn root_case<'a>(vector: &'a Value, name: &str) -> &'a Value {
    vector["constraints"]["root_leaf_cases"]
        .as_array()
        .expect("root constraint cases")
        .iter()
        .find(|case| case["case"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing root constraint case {name}"))
}

fn constraints(case: &Value) -> Value {
    let document: Value = serde_json::from_str(case["document_jcs"].as_str().expect("mandate JCS"))
        .expect("mandate JSON");
    document["constraints"].clone()
}

#[test]
fn cb5_unknown_leaf_extensions_fail_closed_for_operations() {
    let vector = vector();
    let known = constraints(root_case(&vector, "known well-formed root constraint"));
    verify_operation_constraints(&known).expect("known constraints reach operation evaluation");

    let unknown = constraints(root_case(
        &vector,
        "unknown constraint on directly issued chain leaf",
    ));
    match verify_operation_constraints(&unknown) {
        Err(Error::InvalidMandate(_)) => {}
        Err(other) => panic!("expected Error::InvalidMandate, got {other:?}"),
        Ok(_) => panic!("an unknown preserved extension became operation authority"),
    }
}
