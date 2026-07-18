//! CB6 pure, prefix-sensitive Gamma replay contracts.

use std::collections::BTreeMap;

use aithos_core::did::DidDocument;
use aithos_core::gamma::Entry;
use aithos_core::gamma_replay::GammaReplayState;
use aithos_core::mandate::Mandate;
use aithos_core::Error;
use serde_json::Value;

const COEXISTENCE_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-version-coexistence.json"
));

fn fixture() -> Value {
    serde_json::from_str(COEXISTENCE_BYTES).expect("CB2 coexistence vector parses")
}

fn did(vector: &Value) -> DidDocument {
    serde_json::from_str(vector["did"]["jcs"].as_str().expect("DID JCS")).expect("typed DID")
}

fn mandates(vector: &Value, names: &[Value]) -> BTreeMap<String, Mandate> {
    names
        .iter()
        .map(|name| {
            let record = &vector["certificates"][name.as_str().expect("certificate name")];
            let mandate: Mandate =
                serde_json::from_str(record["jcs"].as_str().expect("mandate JCS"))
                    .expect("typed mandate");
            (mandate.id.clone(), mandate)
        })
        .collect()
}

fn entries(section: &Value) -> Vec<Entry> {
    section["gamma_jsonl"]
        .as_str()
        .expect("Gamma JSONL")
        .lines()
        .map(|line| serde_json::from_str(line).expect("typed Gamma entry"))
        .collect()
}

#[test]
fn cb6_append_and_cold_replay_share_one_prefix_sensitive_front_door() {
    let vector = fixture();
    let section = &vector["positive"];
    let did = did(&vector);
    let certificates = mandates(
        &vector,
        section["certificate_names"]
            .as_array()
            .expect("certificate names"),
    );
    let entries = entries(section);

    let mut append = GammaReplayState::new(did.clone(), certificates.clone());
    for entry in &entries {
        append
            .admit(entry)
            .unwrap_or_else(|error| panic!("append {}: {error}", entry.id));
    }
    append.finish().expect("append-time replay is complete");

    let mut cold = GammaReplayState::new(did, certificates);
    for entry in &entries {
        cold.admit(entry)
            .unwrap_or_else(|error| panic!("cold {}: {error}", entry.id));
    }
    cold.finish().expect("cold replay is complete");

    assert_eq!(append.accepted_len(), entries.len());
    assert_eq!(
        append.head().expect("append head"),
        cold.head().expect("cold head")
    );
    assert_eq!(append.counters(), cold.counters());
}

#[test]
fn cb6_rejection_does_not_advance_prefix_or_counters() {
    let vector = fixture();
    let section = &vector["positive"];
    let mut history = entries(section);
    let mut candidate = history.pop().expect("last candidate");
    candidate.signature.value = "00".repeat(64);

    let mut state = GammaReplayState::new(
        did(&vector),
        mandates(
            &vector,
            section["certificate_names"]
                .as_array()
                .expect("certificate names"),
        ),
    );
    for entry in &history {
        state.admit(entry).expect("accepted prefix");
    }
    let before_len = state.accepted_len();
    let before_head = state.head().expect("prefix head");
    let before_counters = state.counters().clone();

    assert!(matches!(
        state.admit(&candidate),
        Err(Error::InvalidGammaEntry(_))
    ));
    assert_eq!(state.accepted_len(), before_len);
    assert_eq!(state.head().expect("unchanged head"), before_head);
    assert_eq!(state.counters(), &before_counters);
}

#[test]
fn cb6_mixed_profile_chain_fails_at_the_candidate_prefix() {
    let vector = fixture();
    for section in vector["negative_cases"].as_array().expect("negative cases") {
        let mut state = GammaReplayState::new(
            did(&vector),
            mandates(
                &vector,
                section["certificate_names"]
                    .as_array()
                    .expect("certificate names"),
            ),
        );
        let mut rejected = false;
        for entry in entries(section) {
            let before = state.accepted_len();
            match state.admit(&entry) {
                Ok(()) => {}
                Err(Error::InvalidMandate(_)) => {
                    assert_eq!(state.accepted_len(), before);
                    rejected = true;
                    break;
                }
                Err(other) => panic!("{}: {other}", section["id"]),
            }
        }
        assert!(rejected, "{} was not rejected", section["id"]);
    }
}
