//! SPL-2 — la grammaire close des clés de store ne nomme aucun
//! consommateur.
//!
//! Oracle : `vectors/cb2-store-key-consumer-neutrality.json`. L'état
//! migré du consommateur vit sous le namespace `x/<id>/**` ordinaire ;
//! les anciennes clés nominatives (préfixe `gateway/`) sont rejetées.
//! Retrait seulement : aucune forme nouvelle de grammaire.

use aithos_bundle::validate_store_key;
use serde_json::Value;

const VECTOR_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-store-key-consumer-neutrality.json"
);

#[test]
fn the_store_key_grammar_names_no_consumer() {
    let vector: Value =
        serde_json::from_str(&std::fs::read_to_string(VECTOR_JSON).expect("vector readable"))
            .expect("vector parses");
    let cases = vector["cases"].as_array().expect("cases");
    assert!(!cases.is_empty(), "the oracle is non-empty");
    for case in cases {
        let value = case["value"].as_str().expect("value");
        assert_eq!(case["input_kind"], "store_key", "single-kind oracle");
        let accepted = validate_store_key(value).is_ok();
        assert_eq!(
            accepted,
            case["expected"] == "accepted",
            "case {}: {}",
            case["id"],
            value
        );
    }
}
