//! Conformance vector A1 — deterministic genesis (spec 01.1, 01.2).
//! Expected values in vectors/a1-genesis.json were generated independently
//! (Python blake3 + PyNaCl), so this test cross-checks two implementations.

use aithos_core::keys::{ed2x, MasterSeed, OwnerKeys};
use serde::Deserialize;

#[derive(Deserialize)]
struct A1 {
    seed_hex: String,
    root_sign_pub_hex: String,
    sphere_public_pub_hex: String,
    sphere_circle_pub_hex: String,
    sphere_self_pub_hex: String,
    owner_kex_pub_hex: String,
    root_sign_pub_ed2x_hex: String,
}

fn vector() -> A1 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/a1-genesis.json"
    ));
    serde_json::from_str(raw).expect("vector a1-genesis.json parses")
}

#[test]
fn a1_deterministic_genesis() {
    let v = vector();
    let seed_bytes = hex::decode(&v.seed_hex).unwrap();
    let seed = MasterSeed::from_slice(&seed_bytes).unwrap();
    let keys = OwnerKeys::genesis(&seed);

    assert_eq!(
        hex::encode(keys.root_sign.verifying_key().to_bytes()),
        v.root_sign_pub_hex,
        "root_sign"
    );
    assert_eq!(
        hex::encode(keys.sphere_public.verifying_key().to_bytes()),
        v.sphere_public_pub_hex,
        "sphere/public"
    );
    assert_eq!(
        hex::encode(keys.sphere_circle.verifying_key().to_bytes()),
        v.sphere_circle_pub_hex,
        "sphere/circle"
    );
    assert_eq!(
        hex::encode(keys.sphere_self.verifying_key().to_bytes()),
        v.sphere_self_pub_hex,
        "sphere/self"
    );
    assert_eq!(
        hex::encode(keys.owner_kex_pub().to_bytes()),
        v.owner_kex_pub_hex,
        "owner_kex"
    );
}

#[test]
fn a1_normative_ed2x_conversion() {
    let v = vector();
    let seed = MasterSeed::from_slice(&hex::decode(&v.seed_hex).unwrap()).unwrap();
    let keys = OwnerKeys::genesis(&seed);
    assert_eq!(
        hex::encode(ed2x(&keys.root_sign.verifying_key()).to_bytes()),
        v.root_sign_pub_ed2x_hex,
        "ed2x must match libsodium's crypto_sign_ed25519_pk_to_curve25519"
    );
}

#[test]
fn genesis_rejects_bad_seed_length() {
    assert!(MasterSeed::from_slice(&[0u8; 31]).is_err());
    assert!(MasterSeed::from_slice(&[0u8; 33]).is_err());
}
