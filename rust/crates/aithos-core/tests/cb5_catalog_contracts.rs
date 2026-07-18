//! CB5c signed connector-catalog, owner approval, pin and class contracts.

use aithos_core::catalog::{
    catalog_action_permitted, verify_catalog_action_facts, verify_catalog_approval,
    verify_catalog_chain, verify_connector_catalog,
};
use aithos_core::Error;
use serde_json::Value;

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-connector-catalog.json"
));

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 catalog vector parses")
}

#[test]
fn cb5_catalog_documents_reach_the_typed_validator() {
    let vector = vector();
    let catalog = verify_connector_catalog(
        &vector["catalog"]["document"],
        vector["catalog"]["catalog_digest"]
            .as_str()
            .expect("catalog digest"),
    )
    .expect("positive catalog validates");
    verify_catalog_approval(
        &vector["approval"]["document"],
        vector["approval"]["approval_digest"]
            .as_str()
            .expect("approval digest"),
        &catalog,
        &vector["owner_did"]["document"],
    )
    .expect("positive owner approval validates");

    for case in vector["negative_catalog_cases"]
        .as_array()
        .expect("catalog negatives")
    {
        match verify_connector_catalog(
            &case["candidate"]["catalog"],
            case["candidate"]["claimed_digest"]
                .as_str()
                .unwrap_or_default(),
        ) {
            Err(Error::InvalidCatalog(_)) => {}
            other => panic!("{}: unexpected catalog verdict {other:?}", case["id"]),
        }
    }
    for case in vector["negative_approval_cases"]
        .as_array()
        .expect("approval negatives")
    {
        match verify_catalog_approval(
            &case["candidate"]["approval"],
            case["candidate"]["claimed_digest"]
                .as_str()
                .unwrap_or_default(),
            &catalog,
            &vector["owner_did"]["document"],
        ) {
            Err(Error::InvalidCatalog(_)) => {}
            other => panic!("{}: unexpected approval verdict {other:?}", case["id"]),
        }
    }
}

#[test]
fn cb5_catalog_pins_and_classes_reach_the_typed_validator() {
    let vector = vector();
    let catalog = verify_connector_catalog(
        &vector["catalog"]["document"],
        vector["catalog"]["catalog_digest"]
            .as_str()
            .expect("catalog digest"),
    )
    .expect("positive catalog validates");
    let approval = verify_catalog_approval(
        &vector["approval"]["document"],
        vector["approval"]["approval_digest"]
            .as_str()
            .expect("approval digest"),
        &catalog,
        &vector["owner_did"]["document"],
    )
    .expect("positive approval validates");
    verify_catalog_chain(
        &vector["draft3_chain"],
        &catalog,
        &approval,
        &vector["owner_did"]["document"],
    )
    .expect("positive catalog pin chain validates");
    let action = verify_catalog_action_facts(
        &vector["action_facts"]["facts"],
        &vector["catalog_pin"],
        &catalog,
        &approval,
        &vector["owner_did"]["document"],
    )
    .expect("positive catalog action facts validate");
    assert_eq!(action.class(), "act");

    for case in vector["class_cases"].as_array().expect("class cases") {
        assert_eq!(
            catalog_action_permitted(
                &catalog,
                case["action"].as_str().expect("action"),
                case["authority"].as_str().expect("authority"),
                case["owner_co_sign"].as_bool().expect("owner co-sign"),
            ),
            case["expected_authorized"].as_bool().expect("verdict"),
            "{}",
            case["action"]
        );
    }
    for case in vector["negative_chain_cases"]
        .as_array()
        .expect("chain negatives")
    {
        match verify_catalog_chain(
            &case["candidate"],
            &catalog,
            &approval,
            &vector["owner_did"]["document"],
        ) {
            Err(Error::InvalidMandate(_)) => {}
            other => panic!("{}: unexpected catalog-chain verdict {other:?}", case["id"]),
        }
    }
    for case in vector["negative_action_facts_cases"]
        .as_array()
        .expect("action-facts negatives")
    {
        match verify_catalog_action_facts(
            &case["candidate"],
            &vector["catalog_pin"],
            &catalog,
            &approval,
            &vector["owner_did"]["document"],
        ) {
            Err(Error::InvalidOperationFacts(_)) => {}
            other => panic!("{}: unexpected action-facts verdict {other:?}", case["id"]),
        }
    }
}
