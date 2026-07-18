//! Byte-exact replay of `vectors/p4-witness-checkpoint.json` against the
//! REAL witness code (annexe C, contrat C3).
//!
//! Every checkpoint, feed line and the daily root is REBUILT from its
//! fields with the vector's own witness key and must reproduce the
//! committed bytes exactly (signature included) — cross-checking our JCS,
//! our Ed25519 and our dedicated-domain mroot against the independent
//! Python generator. The equivocation rule (C.4) is replayed on the
//! committed pairs. The KMS signer is deploy-gated (P5 gate); the format
//! is proven here.

use aithos_provider::witness::{
    build_checkpoint, build_daily_root, feed_line, is_equivocation, verify_checkpoint,
    verify_daily_root, Checkpoint, DailyRoot, LocalWitnessSigner, WitnessKeyRegistry,
};
use ed25519_dalek::SigningKey;
use serde_json::Value;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");

fn p4() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!("{VECTORS}/p4-witness-checkpoint.json")).unwrap(),
    )
    .unwrap()
}

/// The published registry for the replay: the vector's single witness key.
fn registry(p4: &Value) -> WitnessKeyRegistry {
    WitnessKeyRegistry::from([p4["witness_key"].as_str().unwrap().to_owned()])
}

fn signer(p4: &Value) -> LocalWitnessSigner {
    let seed: [u8; 32] = hex::decode(p4["witness_sk_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    LocalWitnessSigner::new(SigningKey::from_bytes(&seed))
}

fn ck(jcs: &str) -> Checkpoint {
    serde_json::from_str(jcs).unwrap()
}

#[test]
fn every_committed_checkpoint_verifies_and_rebuilds_byte_exact() {
    let p4 = p4();
    let signer = signer(&p4);
    let reg = registry(&p4);

    for (name, jcs) in p4["checkpoints"].as_object().unwrap() {
        let jcs = jcs.as_str().unwrap();
        let committed = ck(jcs);
        // 1. Verifies under the published registry (key + self-signature).
        assert!(verify_checkpoint(&committed, &reg), "{name}: signature");
        // A checkpoint whose key is not published is not evidence.
        assert!(!verify_checkpoint(&committed, &WitnessKeyRegistry::new()));
        // 2. The feed line is the exact committed JCS bytes.
        assert_eq!(feed_line(&committed), jcs, "{name}: feed line drift");
        // 3. Rebuilding from the fields reproduces the bytes exactly —
        //    our JCS + Ed25519 == the independent Python generator.
        let rebuilt = build_checkpoint(
            &signer,
            &committed.did,
            committed.edition_height,
            &committed.manifest_hash,
            &committed.gamma_head,
            &committed.observed_at,
        );
        assert_eq!(feed_line(&rebuilt), jcs, "{name}: rebuild drift");
    }
}

#[test]
fn the_daily_root_rebuilds_byte_exact() {
    let p4 = p4();
    let signer = signer(&p4);
    let day_lines: Vec<String> = p4["feed"]["lines_2026-07-16"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_owned())
        .collect();

    // The committed leaf order is the sorted, deduped set.
    let mut sorted = day_lines.clone();
    sorted.sort();
    sorted.dedup();
    let committed_order: Vec<String> = p4["daily_root"]["leaf_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(sorted, committed_order, "leaf order (sorted JCS bytes)");

    let rebuilt = build_daily_root(&signer, "2026-07-16", &day_lines);
    let committed: DailyRoot = serde_json::from_value(p4["daily_root"]["doc"].clone()).unwrap();
    assert_eq!(
        rebuilt.root, committed.root,
        "mroot hex (dedicated domains)"
    );
    assert_eq!(rebuilt.n, committed.n, "n");
    assert_eq!(
        serde_jcs::to_string(&rebuilt).unwrap(),
        serde_jcs::to_string(&committed).unwrap(),
        "daily root bytes (signature included)"
    );
    assert!(
        verify_daily_root(&committed, &registry(&p4)),
        "committed root signature"
    );
}

#[test]
fn the_equivocation_rule_matches_annexe_c4() {
    let p4 = p4();
    let reg = registry(&p4);
    for case in p4["equivocation_cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let pair = case["pair"].as_array().unwrap();
        let a = ck(pair[0].as_str().unwrap());
        let b = ck(pair[1].as_str().unwrap());
        let want = case["expect"]["equivocation"].as_bool().unwrap();
        assert_eq!(is_equivocation(&a, &b, &reg), want, "{name}");
        // The rule is symmetric.
        assert_eq!(is_equivocation(&b, &a, &reg), want, "{name} (swapped)");
        // Outside the registry, the same pair is never portable proof.
        assert!(
            !is_equivocation(&a, &b, &WitnessKeyRegistry::new()),
            "{name} (no registry)"
        );
    }
}
