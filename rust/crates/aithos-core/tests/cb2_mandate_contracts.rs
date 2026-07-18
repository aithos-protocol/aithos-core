//! CB2 mandate contracts: executable consumers of the independent Python
//! oracle. RED tests use existing public APIs only; no future Rust API shape,
//! public reason code, or form-before-signature hook is invented here.

use aithos_core::constraints::validate_link_constraints;
use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{verify_chain, Mandate, PerimeterEntry};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CB2_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-mandate-contracts.json"
));
const E1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/e1-mandate.json"
));
const F1_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/f1-gamma-chain.json"
));
const CB2_SHA256: &str = "771eef3b92314a5cc6a37882a35cc81cbbb2b4e0d4976c4d555a10ba05cf1e3e";
const F1_MANDATE_SHA256: &str = "daa2fa399ab7ead621a0569942712c2939fd144ac0940deca5e463804810d90f";
const AT: &str = "2026-07-02T00:00:00Z";
const FORM_CASE_NAMES: [&str; 31] = [
    "historical F1 action perimeter",
    "supported draft.1 root",
    "supported draft.2 root",
    "supported punctuation nonce",
    "supported fractional Zulu timestamps",
    "unsupported protocol version",
    "signature algorithm other than ed25519",
    "root announced signer key differs from issuer",
    "child announced signer key differs from issuer",
    "malformed mandate identifier",
    "malformed subject identifier",
    "child subject changes along the chain",
    "root carries a parent identifier",
    "child parent identifier differs from presented parent",
    "child issued_by differs from parent grantee",
    "malformed grantee signing key",
    "grantee kex key does not match signing key",
    "empty nonce",
    "non-string nonce",
    "timestamp uses an offset instead of Zulu",
    "timestamp is not a calendar instant",
    "validity window is inverted",
    "sub-microsecond validity window is inverted",
    "issue depth zero",
    "duplicate dir selector",
    "duplicate tag selector",
    "duplicate id selector",
    "id mixed with dir",
    "id mixed with tag",
    "dir mixed with id",
    "tag mixed with id",
];
const ID_CONTAINMENT_PARSE_GATES: [&str; 7] = [
    "whole zone covers exact id",
    "identical id covers itself",
    "different id is not covered",
    "id does not cover whole zone",
    "dir never covers id",
    "tag never covers id",
    "other zone does not cover id",
];

fn vector() -> Value {
    serde_json::from_str(CB2_BYTES).expect("CB2 mandate vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn hex32(value: &str) -> [u8; 32] {
    hex::decode(value)
        .expect("hex fixture")
        .try_into()
        .expect("32-byte fixture")
}

fn named<'a>(items: &'a Value, name: &str) -> &'a Value {
    items
        .as_array()
        .expect("named collection is an array")
        .iter()
        .find(|case| case["case"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing vector case {name}"))
}

fn did_document(vector: &Value) -> DidDocument {
    serde_json::from_str(
        vector["signed_fixtures"]["did_document_jcs"]
            .as_str()
            .expect("DID fixture JCS"),
    )
    .expect("DID fixture parses")
}

fn mandate_from_jcs(value: &Value) -> Mandate {
    serde_json::from_str(value.as_str().expect("mandate JCS string")).expect("mandate parses")
}

fn f1_did_document() -> DidDocument {
    let f1: Value = serde_json::from_str(F1_BYTES).expect("F1 parses");
    let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(hex32(
        f1["seed_hex"].as_str().expect("F1 seed"),
    )));
    let succession = succession_from_entropy([9; 32]);
    DidDocument::build(
        &owner,
        &succession.verifying_key(),
        Vec::new(),
        String::new(),
    )
    .expect("F1 DID builds")
}

fn expect_invalid_mandate<T>(result: Result<T, Error>, case: &str) {
    match result {
        Err(Error::InvalidMandate(_)) => {}
        Err(other) => panic!("{case}: expected Error::InvalidMandate, got {other:?}"),
        Ok(_) => panic!("{case}: expected Error::InvalidMandate, got Ok"),
    }
}

fn assert_root_constraint_case_rejected(case_name: &str) {
    let vector = vector();
    let case = named(&vector["constraints"]["root_leaf_cases"], case_name);
    assert_eq!(case["expected_certificate_valid"].as_bool(), Some(false));
    let mandate = mandate_from_jcs(&case["document_jcs"]);
    expect_invalid_mandate(
        verify_chain(&[mandate], &did_document(&vector), AT),
        case_name,
    );
}

fn form_case_chain(vector: &Value, case: &Value) -> Result<Vec<Mandate>, serde_json::Error> {
    let document = serde_json::from_str(
        case["document_jcs"]
            .as_str()
            .expect("form-case document JCS"),
    )?;
    match case["role"].as_str().expect("form-case role") {
        "root" => Ok(vec![document]),
        "child" => {
            let fixture = match case["parent_fixture"]
                .as_str()
                .expect("child parent fixture")
            {
                "root_draft1" => "root_draft1_jcs",
                "root_draft2" => "root_draft2_jcs",
                "root_draft2_form_no_id" => "root_draft2_form_no_id_jcs",
                other => panic!("unknown parent fixture {other}"),
            };
            let parent = serde_json::from_str(
                vector["signed_fixtures"][fixture]
                    .as_str()
                    .expect("parent fixture JCS"),
            )?;
            Ok(vec![parent, document])
        }
        other => panic!("unknown form-case role {other}"),
    }
}

fn form_case_did(vector: &Value, case_name: &str) -> DidDocument {
    if case_name == "historical F1 action perimeter" {
        f1_did_document()
    } else {
        did_document(vector)
    }
}

fn assert_t3_form_case_accepted(case_name: &str) {
    let vector = vector();
    let case = named(&vector["form_cases"], case_name);
    assert_eq!(case["expected_form_valid"].as_bool(), Some(true));
    let document_jcs = case["document_jcs"].as_str().expect("form-case JCS");
    let document: Value = serde_json::from_str(document_jcs).expect("form-case JSON");
    assert_eq!(
        jcs::canonicalize(&document).expect("form-case canonicalizes"),
        document_jcs
    );
    let chain = form_case_chain(&vector, case).expect("positive form case reaches verifier");
    verify_chain(&chain, &form_case_did(&vector, case_name), AT).unwrap_or_else(|error| {
        panic!("{case_name}: expected valid form and signature: {error:?}")
    });
}

fn assert_t3_form_case_rejected(case_name: &str) {
    let vector = vector();
    let case = named(&vector["form_cases"], case_name);
    assert_eq!(case["expected_form_valid"].as_bool(), Some(false));
    let chain = form_case_chain(&vector, case).expect("negative form case reaches verifier");
    expect_invalid_mandate(
        verify_chain(&chain, &form_case_did(&vector, case_name), AT),
        case_name,
    );
}

fn assert_invalid_selector_entry_rejected(index: usize, case_name: &str) {
    let vector = vector();
    let case = &vector["id_selector"]["invalid_entries"][index];
    assert_eq!(case["expected_parse_valid"].as_bool(), Some(false));
    expect_invalid_mandate(
        PerimeterEntry::parse(case["entry"].as_str().expect("invalid perimeter entry")),
        case_name,
    );
}

macro_rules! t3_form_acceptance_test {
    ($test_name:ident, $case_name:literal) => {
        #[test]
        fn $test_name() {
            assert_t3_form_case_accepted($case_name);
        }
    };
}

macro_rules! t3_form_rejection_test {
    ($test_name:ident, $case_name:literal) => {
        #[test]
        fn $test_name() {
            assert_t3_form_case_rejected($case_name);
        }
    };
}

#[test]
fn cb2_vector_hash_is_stable_preexisting_green() {
    assert_eq!(sha256_hex(CB2_BYTES.as_bytes()), CB2_SHA256);
}

#[test]
fn cb2_historical_e1_f1_hash_and_jcs_preexisting_green() {
    let vector = vector();
    let e1: Value = serde_json::from_str(E1_BYTES).expect("E1 parses");
    let e1_jcs = e1["mandate_jcs"].as_str().expect("E1 mandate JCS");
    assert_eq!(
        sha256_hex(e1_jcs.as_bytes()),
        vector["historical_without_id_selector"]["sha256"]
            .as_str()
            .expect("E1 frozen hash")
    );
    let e1_value: Value = serde_json::from_str(e1_jcs).expect("E1 mandate parses");
    assert_eq!(jcs::canonicalize(&e1_value).expect("E1 JCS"), e1_jcs);

    let f1: Value = serde_json::from_str(F1_BYTES).expect("F1 parses");
    let f1_jcs = f1["mandate_jcs"].as_str().expect("F1 mandate JCS");
    assert_eq!(sha256_hex(f1_jcs.as_bytes()), F1_MANDATE_SHA256);
    let f1_value: Value = serde_json::from_str(f1_jcs).expect("F1 mandate parses");
    assert_eq!(jcs::canonicalize(&f1_value).expect("F1 JCS"), f1_jcs);
    assert_eq!(
        named(&vector["form_cases"], "historical F1 action perimeter")["document_jcs"].as_str(),
        Some(f1_jcs)
    );
}

#[test]
fn cb2_id_mandate_jcs_preexisting_green() {
    let vector = vector();
    let mandate_jcs = vector["id_selector"]["canonical_mandate_jcs"]
        .as_str()
        .expect("id= mandate JCS");
    let mandate: Mandate = serde_json::from_str(mandate_jcs).expect("id= mandate parses");
    assert_eq!(
        jcs::canonicalize(&mandate).expect("id= mandate JCS"),
        mandate_jcs
    );
    assert_eq!(
        sha256_hex(mandate_jcs.as_bytes()),
        vector["id_selector"]["canonical_mandate_sha256"]
            .as_str()
            .expect("id= mandate hash")
    );
}

#[test]
fn cb2_id_selector_parse_and_roundtrip_red() {
    let vector = vector();
    let cases = vector["id_selector"]["roundtrips"]
        .as_array()
        .expect("id= roundtrips");
    assert_eq!(cases.len(), 2);
    let mut parse_blocked = Vec::new();
    for case in cases {
        let entry = case["entry"].as_str().expect("id= entry");
        match PerimeterEntry::parse(entry) {
            Ok(parsed) => assert_eq!(
                parsed.to_entry_string(),
                case["roundtrip"].as_str().expect("roundtrip")
            ),
            Err(_) => parse_blocked.push(entry),
        }
    }
    assert!(
        parse_blocked.is_empty(),
        "CB2 RED: valid id= entries rejected before round-trip: {}",
        parse_blocked.join(", ")
    );
}

#[test]
fn cb2_id_selector_containment_parse_gate_inventory_preliminary() {
    let vector = vector();
    assert_eq!(
        vector["id_selector"]["containment"]
            .as_array()
            .expect("containment cases")
            .len(),
        ID_CONTAINMENT_PARSE_GATES.len() + 1
    );
    let mut parse_blocked = Vec::new();
    for case_name in ID_CONTAINMENT_PARSE_GATES {
        let case = named(&vector["id_selector"]["containment"], case_name);
        assert!(case["expected_covers"].is_boolean());
        let parent = PerimeterEntry::parse(case["parent"].as_str().expect("parent entry"));
        let child = PerimeterEntry::parse(case["child"].as_str().expect("child entry"));
        if parent.is_err() || child.is_err() {
            parse_blocked.push(case_name);
        }
    }
    assert_eq!(parse_blocked, ID_CONTAINMENT_PARSE_GATES);
}

#[test]
fn cb2_dir_selector_containment_after_address_change_preexisting_green() {
    let vector = vector();
    let case = named(
        &vector["id_selector"]["containment"],
        "dir containment follows the terminal node after an address change",
    );
    let parent = PerimeterEntry::parse(case["parent"].as_str().expect("parent entry"))
        .expect("parent parses");
    let child =
        PerimeterEntry::parse(case["child"].as_str().expect("child entry")).expect("child parses");
    assert_eq!(
        parent.covers(&child),
        case["expected_covers"].as_bool().expect("covers verdict")
    );
}

#[test]
fn cb2_duplicate_dir_invalid_entry_rejected_red() {
    assert_invalid_selector_entry_rejected(0, "duplicate dir selector");
}

#[test]
fn cb2_duplicate_tag_invalid_entry_rejected_red() {
    assert_invalid_selector_entry_rejected(1, "duplicate tag selector");
}

#[test]
fn cb2_id_mixed_and_duplicate_invalid_entries_parse_gate_preliminary() {
    let vector = vector();
    assert_eq!(
        vector["id_selector"]["invalid_entries"]
            .as_array()
            .expect("invalid entries")
            .len(),
        7
    );
    for (index, case_name) in [
        (2, "duplicate id selector"),
        (3, "id mixed with dir"),
        (4, "id mixed with tag"),
        (5, "dir mixed with id"),
        (6, "tag mixed with id"),
    ] {
        assert_invalid_selector_entry_rejected(index, case_name);
    }
}

#[test]
fn cb2_delete_covers_read_red() {
    let vector = vector();
    let case = vector["verb_lattice"]
        .as_array()
        .expect("verb lattice")
        .iter()
        .find(|case| {
            case["grant"].as_str() == Some("delete") && case["required"].as_str() == Some("read")
        })
        .expect("delete -> read case");
    assert_eq!(case["expected_covers"].as_bool(), Some(true));
    let grant = PerimeterEntry::parse("delete.circle").expect("delete grant parses");
    let required = PerimeterEntry::parse("read.circle").expect("read requirement parses");
    assert!(grant.covers(&required), "CB2 RED: delete must cover read");
}

#[test]
fn cb2_known_link_constraint_shapes_well_formed_preexisting_green() {
    let vector = vector();
    let case = named(
        &vector["constraints"]["known_shape_matrix"],
        "all known families well-formed",
    );
    assert_eq!(case["expected_shape_valid"].as_bool(), Some(true));
    validate_link_constraints(&case["constraints"]).expect("known constraint shapes are valid");
}

#[test]
fn cb2_known_link_constraint_other_malformed_forms_preexisting_green() {
    let vector = vector();
    let qualified_red = [
        "malformed active_windows",
        "fractional active_window requires a future version",
        "malformed obligations",
    ];
    for case in vector["constraints"]["known_shape_matrix"]
        .as_array()
        .expect("known constraint matrix")
        .iter()
        .filter(|case| case["expected_shape_valid"].as_bool() == Some(false))
    {
        let name = case["case"].as_str().expect("constraint case name");
        if qualified_red.contains(&name) {
            continue;
        }
        expect_invalid_mandate(validate_link_constraints(&case["constraints"]), name);
    }
}

#[test]
fn cb2_malformed_obligation_shape_rejected_red() {
    let vector = vector();
    let case = named(
        &vector["constraints"]["known_shape_matrix"],
        "malformed obligations",
    );
    expect_invalid_mandate(
        validate_link_constraints(&case["constraints"]),
        "malformed obligations",
    );
}

#[test]
fn cb2_malformed_active_window_duration_rejected_red() {
    let vector = vector();
    let case = named(
        &vector["constraints"]["known_shape_matrix"],
        "malformed active_windows",
    );
    expect_invalid_mandate(
        validate_link_constraints(&case["constraints"]),
        "malformed active_windows",
    );
}

#[test]
fn cb2_fractional_active_window_rejected_red() {
    let vector = vector();
    let case = named(
        &vector["constraints"]["known_shape_matrix"],
        "fractional active_window requires a future version",
    );
    expect_invalid_mandate(
        validate_link_constraints(&case["constraints"]),
        "fractional active_window requires a future version",
    );
}

#[test]
fn cb2_root_known_well_formed_constraint_preexisting_green() {
    let vector = vector();
    assert_eq!(
        vector["constraints"]["root_leaf_cases"]
            .as_array()
            .expect("root-leaf cases")
            .len(),
        6
    );
    let case = named(
        &vector["constraints"]["root_leaf_cases"],
        "known well-formed root constraint",
    );
    assert_eq!(case["expected_certificate_valid"].as_bool(), Some(true));
    let mandate = mandate_from_jcs(&case["document_jcs"]);
    verify_chain(&[mandate], &did_document(&vector), AT)
        .expect("known well-formed root constraint verifies");
}

#[test]
fn cb2_root_malformed_max_actions_rejected_red() {
    assert_root_constraint_case_rejected("known malformed root constraint");
}

#[test]
fn cb2_root_malformed_max_children_rejected_red() {
    assert_root_constraint_case_rejected("known malformed root max_children");
}

#[test]
fn cb2_root_malformed_log_reads_rejected_red() {
    assert_root_constraint_case_rejected("known malformed root log_reads");
}

#[test]
fn cb2_root_malformed_domains_rejected_red() {
    assert_root_constraint_case_rejected("known malformed root domains");
}

#[test]
fn cb2_root_unknown_leaf_structural_preservation_preexisting_green() {
    let vector = vector();
    let case = named(
        &vector["constraints"]["root_leaf_cases"],
        "unknown constraint on directly issued chain leaf",
    );
    assert_eq!(case["expected_certificate_valid"].as_bool(), Some(true));
    let mandate = mandate_from_jcs(&case["document_jcs"]);
    verify_chain(std::slice::from_ref(&mandate), &did_document(&vector), AT)
        .expect("unknown root-leaf extension remains structurally valid");
    // Structural preservation is the only claim here. Operation-level
    // "Never implicit Allow" remains API-gated for CB5.
    assert_eq!(
        jcs::canonicalize(&mandate.constraints).expect("preserved constraints JCS"),
        case["preserved_constraints_jcs"]
            .as_str()
            .expect("preserved constraints")
    );
}

#[test]
fn cb2_unknown_constraints_on_links_fail_closed_preexisting_green() {
    let vector = vector();
    for case in vector["constraints"]["link_cases"]
        .as_array()
        .expect("unknown link cases")
    {
        assert_eq!(case["expected_chain_valid"].as_bool(), Some(false));
        let chain: Vec<Mandate> = case["chain_jcs"]
            .as_array()
            .expect("chain JCS")
            .iter()
            .map(mandate_from_jcs)
            .collect();
        expect_invalid_mandate(
            verify_chain(&chain, &did_document(&vector), AT),
            case["case"].as_str().expect("unknown link case"),
        );
    }
}

#[test]
fn cb2_t3_form_case_inventory_is_exhaustive() {
    let vector = vector();
    let actual: Vec<&str> = vector["form_cases"]
        .as_array()
        .expect("form cases")
        .iter()
        .map(|case| case["case"].as_str().expect("form-case name"))
        .collect();
    assert_eq!(actual, FORM_CASE_NAMES);
}

t3_form_acceptance_test!(
    cb2_t3_historical_f1_form_and_signature_preexisting_green,
    "historical F1 action perimeter"
);
t3_form_acceptance_test!(
    cb2_t3_supported_draft1_root_and_signature_preexisting_green,
    "supported draft.1 root"
);
t3_form_acceptance_test!(
    cb2_t3_supported_draft2_root_and_signature_preexisting_green,
    "supported draft.2 root"
);
t3_form_acceptance_test!(
    cb2_t3_punctuation_nonce_and_signature_preexisting_green,
    "supported punctuation nonce"
);
t3_form_acceptance_test!(
    cb2_t3_fractional_zulu_timestamps_and_signature_preexisting_green,
    "supported fractional Zulu timestamps"
);

t3_form_rejection_test!(
    cb2_t3_unsupported_protocol_version_rejected_red,
    "unsupported protocol version"
);
t3_form_rejection_test!(
    cb2_t3_signature_algorithm_rejected_red,
    "signature algorithm other than ed25519"
);
t3_form_rejection_test!(
    cb2_t3_root_announced_signer_key_rejected_red,
    "root announced signer key differs from issuer"
);
t3_form_rejection_test!(
    cb2_t3_child_announced_signer_key_rejected_red,
    "child announced signer key differs from issuer"
);
t3_form_rejection_test!(
    cb2_t3_malformed_mandate_identifier_rejected_red,
    "malformed mandate identifier"
);
t3_form_rejection_test!(
    cb2_t3_malformed_subject_identifier_rejected_preexisting_green,
    "malformed subject identifier"
);
t3_form_rejection_test!(
    cb2_t3_child_subject_change_rejected_preexisting_green,
    "child subject changes along the chain"
);
t3_form_rejection_test!(
    cb2_t3_root_parent_identifier_rejected_preexisting_green,
    "root carries a parent identifier"
);
t3_form_rejection_test!(
    cb2_t3_child_parent_mismatch_rejected_preexisting_green,
    "child parent identifier differs from presented parent"
);
t3_form_rejection_test!(
    cb2_t3_child_issuer_mismatch_rejected_preexisting_green,
    "child issued_by differs from parent grantee"
);
t3_form_rejection_test!(
    cb2_t3_malformed_grantee_signing_key_rejected_red,
    "malformed grantee signing key"
);
t3_form_rejection_test!(
    cb2_t3_grantee_kex_mismatch_rejected_preexisting_green,
    "grantee kex key does not match signing key"
);
t3_form_rejection_test!(cb2_t3_empty_nonce_rejected_red, "empty nonce");

#[test]
fn cb2_t3_non_string_nonce_serde_gate_preliminary() {
    let vector = vector();
    let case = named(&vector["form_cases"], "non-string nonce");
    assert_eq!(case["expected_form_valid"].as_bool(), Some(false));
    let document_jcs = case["document_jcs"].as_str().expect("form-case JCS");
    let document: Value = serde_json::from_str(document_jcs).expect("raw form-case JSON");
    assert!(!document["nonce"].is_string());
    assert!(
        serde_json::from_str::<Mandate>(document_jcs).is_err(),
        "CB2 API-GATE-PRELIMINARY: non-string nonce unexpectedly reaches verify_chain"
    );
}

t3_form_rejection_test!(
    cb2_t3_offset_timestamp_rejected_red,
    "timestamp uses an offset instead of Zulu"
);
t3_form_rejection_test!(
    cb2_t3_invalid_calendar_timestamp_rejected_red,
    "timestamp is not a calendar instant"
);
t3_form_rejection_test!(
    cb2_t3_inverted_validity_window_rejected_preexisting_green,
    "validity window is inverted"
);
t3_form_rejection_test!(
    cb2_t3_submicrosecond_inverted_window_rejected_preexisting_green,
    "sub-microsecond validity window is inverted"
);
t3_form_rejection_test!(cb2_t3_issue_depth_zero_rejected_red, "issue depth zero");
t3_form_rejection_test!(
    cb2_t3_duplicate_dir_selector_rejected_red,
    "duplicate dir selector"
);
t3_form_rejection_test!(
    cb2_t3_duplicate_tag_selector_rejected_red,
    "duplicate tag selector"
);
t3_form_rejection_test!(
    cb2_t3_duplicate_id_selector_rejected_red,
    "duplicate id selector"
);
t3_form_rejection_test!(cb2_t3_id_mixed_with_dir_rejected_red, "id mixed with dir");
t3_form_rejection_test!(cb2_t3_id_mixed_with_tag_rejected_red, "id mixed with tag");
t3_form_rejection_test!(cb2_t3_dir_mixed_with_id_rejected_red, "dir mixed with id");
t3_form_rejection_test!(cb2_t3_tag_mixed_with_id_rejected_red, "tag mixed with id");
