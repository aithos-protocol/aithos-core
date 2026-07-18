//! CB3 semantic transitions for the API-gated CB2 mandate contracts.
//!
//! Every assertion consumes the frozen independent CB2 oracle. The tests map
//! only to existing ledger IDs; they add no protocol behavior.

use aithos_core::error::Error;
use aithos_core::ids::Sid;
use aithos_core::mandate::{covers_section_op, Mandate, PerimeterEntry, SectionOp, Verb};
use aithos_core::path::Zone;
use serde_json::Value;

const VECTOR_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-mandate-contracts.json"
));

fn vector() -> Value {
    serde_json::from_str(VECTOR_JSON).expect("CB2 mandate vector parses")
}

fn named<'a>(items: &'a Value, name: &str) -> &'a Value {
    items
        .as_array()
        .expect("named collection is an array")
        .iter()
        .find(|case| case["case"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing vector case {name}"))
}

#[test]
fn cb3_id_selector_containment_reaches_the_semantic_oracle() {
    let vector = vector();
    for case in vector["id_selector"]["containment"]
        .as_array()
        .expect("containment matrix")
    {
        let parent = PerimeterEntry::parse(case["parent"].as_str().expect("parent entry"))
            .expect("parent selector reaches containment");
        let child = PerimeterEntry::parse(case["child"].as_str().expect("child entry"))
            .expect("child selector reaches containment");
        assert_eq!(
            parent.covers(&child),
            case["expected_covers"].as_bool().expect("oracle verdict"),
            "{}",
            case["case"].as_str().expect("case name")
        );
    }
}

#[test]
fn cb3_section_operation_supplies_its_sid_to_core() {
    let target = Sid::parse("00000000000000000000000002").expect("target SID");
    let sibling = Sid::parse("00000000000000000000000003").expect("sibling SID");
    let folders = [];
    let tags = [];
    let grant =
        PerimeterEntry::parse("read.circle#id=00000000000000000000000002").expect("id grant");

    let operation = |sid| SectionOp {
        verb: Verb::Read,
        zone: Zone::Circle,
        sid,
        folders: &folders,
        tags: &tags,
    };

    assert!(covers_section_op(
        std::slice::from_ref(&grant),
        &operation(target)
    ));
    assert!(!covers_section_op(
        std::slice::from_ref(&grant),
        &operation(sibling)
    ));
}

#[test]
fn cb3_raw_mandate_form_is_a_typed_error_before_signature_trust() {
    let vector = vector();
    for case_name in ["non-string nonce", "unsupported protocol version"] {
        let case = named(&vector["form_cases"], case_name);
        assert_eq!(case["expected_form_valid"].as_bool(), Some(false));
        let result = case["document_jcs"]
            .as_str()
            .expect("raw mandate JSON")
            .parse::<Mandate>();
        assert!(
            matches!(result, Err(Error::InvalidMandate(_))),
            "{case_name}: expected typed InvalidMandate, got {result:?}"
        );
    }
}
