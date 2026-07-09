#![forbid(unsafe_code)]
//! WASM surface over `aithos-core`. Thin by design: no logic lives here,
//! only (de)serialization at the JS boundary. Published as `@aithos/core`.

use aithos_core::keys::{MasterSeed, OwnerKeys};
use wasm_bindgen::prelude::*;

/// Deterministic genesis (spec §01.1): derive the owner public keys from a
/// 32-byte master seed. Returns a JSON string of hex-encoded public keys.
/// The seed must come from the caller — WASM core never generates randomness.
#[wasm_bindgen]
pub fn genesis_pubkeys(seed: &[u8]) -> Result<String, JsError> {
    let seed = MasterSeed::from_slice(seed).map_err(|e| JsError::new(&e.to_string()))?;
    let keys = OwnerKeys::genesis(&seed);
    let out = serde_json::json!({
        "root_sign_pub": hex::encode(keys.root_sign.verifying_key().to_bytes()),
        "sphere_public_pub": hex::encode(keys.sphere_public.verifying_key().to_bytes()),
        "sphere_circle_pub": hex::encode(keys.sphere_circle.verifying_key().to_bytes()),
        "sphere_self_pub": hex::encode(keys.sphere_self.verifying_key().to_bytes()),
        "owner_kex_pub": hex::encode(keys.owner_kex_pub().to_bytes()),
    });
    Ok(out.to_string())
}
