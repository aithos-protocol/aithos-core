#![forbid(unsafe_code)]
//! WASM surface over `aithos-core`. Thin by design: no logic lives here,
//! only (de)serialization at the JS boundary. Packaged locally as
//! `@aithos/core` (wasm-pack); publishing is a separate, explicit decision.

use aithos_core::did::DidDocument;
use aithos_core::gamma::Entry;
use aithos_core::keys::{ed2x, MasterSeed, OwnerKeys};
use aithos_core::mandate::{
    verify_chain, verify_chain_revocable, Mandate, MandateSpec, PerimeterEntry,
};
use aithos_core::revocation::revocations;
use ed25519_dalek::{Signer as _, SigningKey};
use serde::Deserialize;
use serde_json::Value;
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
        "content_sign_pub": hex::encode(keys.content_sign.verifying_key().to_bytes()),
        "owner_kex_pub": hex::encode(keys.owner_kex_pub().to_bytes()),
    });
    Ok(out.to_string())
}

struct DelegateSeed<'a>(&'a mut [u8]);

impl Drop for DelegateSeed<'_> {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

fn signing_key(seed: &DelegateSeed<'_>) -> Result<SigningKey, JsError> {
    let seed: &[u8; 32] = seed
        .0
        .as_ref()
        .try_into()
        .map_err(|_| JsError::new("delegate seed must contain exactly 32 bytes"))?;
    Ok(SigningKey::from_bytes(seed))
}

/// Public identity of an already unlocked local delegate signer.
#[wasm_bindgen]
pub fn delegate_pubkey(seed: &mut [u8]) -> Result<String, JsError> {
    let seed = DelegateSeed(seed);
    let key = signing_key(&seed)?;
    Ok(aithos_core::wire::ed25519_pub_to_multibase(
        &key.verifying_key().to_bytes(),
    ))
}

/// Verify a complete public mandate chain at the caller-supplied instant.
/// When Gamma entries are supplied, revocations are applied as well.
#[wasm_bindgen]
pub fn verify_mandate_chain(
    chain_json: &str,
    did_json: &str,
    at: &str,
    gamma_entries_json: Option<String>,
) -> Result<(), JsError> {
    let chain: Vec<Mandate> =
        serde_json::from_str(chain_json).map_err(|_| JsError::new("mandate chain is malformed"))?;
    let did: DidDocument =
        serde_json::from_str(did_json).map_err(|_| JsError::new("DID document is malformed"))?;
    match gamma_entries_json {
        Some(entries) => {
            let entries: Vec<Entry> = serde_json::from_str(&entries)
                .map_err(|_| JsError::new("Gamma entries are malformed"))?;
            verify_chain_revocable(&chain, &did, at, &revocations(&entries))
                .map_err(|error| JsError::new(&error.to_string()))
        }
        None => verify_chain(&chain, &did, at).map_err(|error| JsError::new(&error.to_string())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionSubmandateRequest {
    id: String,
    subject: String,
    grantee_id: String,
    grantee_label: String,
    gateway_pub: String,
    gateway_kex_pub: String,
    session_pub: String,
    perimeter: Vec<String>,
    constraints: Value,
    not_before: String,
    not_after: String,
    issued_at: String,
    nonce: String,
}

/// Build and sign the strictly attenuated session leaf through Core.
/// Random ids and nonces are explicit caller inputs; no hidden randomness is
/// drawn and no seed is returned.
#[wasm_bindgen]
pub fn build_session_submandate(
    parent_json: &str,
    delegate_seed: &mut [u8],
    request_json: &str,
) -> Result<String, JsError> {
    let delegate_seed = DelegateSeed(delegate_seed);
    let key = signing_key(&delegate_seed)?;
    build_session_submandate_with_key(parent_json, &key, request_json)
}

fn build_session_submandate_with_key(
    parent_json: &str,
    key: &SigningKey,
    request_json: &str,
) -> Result<String, JsError> {
    let parent: Mandate = serde_json::from_str(parent_json)
        .map_err(|_| JsError::new("parent mandate is malformed"))?;
    let request: SessionSubmandateRequest = serde_json::from_str(request_json)
        .map_err(|_| JsError::new("session request is malformed"))?;
    if request.subject != parent.subject {
        return Err(JsError::new("session subject differs from its parent"));
    }
    if request.perimeter.iter().any(|entry| {
        PerimeterEntry::parse(entry)
            .is_ok_and(|entry| matches!(entry, PerimeterEntry::Issue { .. }))
    }) {
        return Err(JsError::new(
            "session leaves may not transmit issue authority",
        ));
    }
    let before = aithos_core::gamma::ts_epoch(&request.not_before)
        .map_err(|_| JsError::new("session not_before is malformed"))?;
    let after = aithos_core::gamma::ts_epoch(&request.not_after)
        .map_err(|_| JsError::new("session not_after is malformed"))?;
    if after <= before || after - before > 8 * 60 * 60 {
        return Err(JsError::new("session lifetime exceeds eight hours"));
    }
    let gateway_bytes = aithos_core::wire::multibase_to_ed25519_pub(&request.gateway_pub)
        .map_err(|_| JsError::new("gateway signing key is malformed"))?;
    let gateway = ed25519_dalek::VerifyingKey::from_bytes(&gateway_bytes)
        .map_err(|_| JsError::new("gateway signing key is malformed"))?;
    let gateway_kex = aithos_core::wire::multibase_to_x25519_pub(&request.gateway_kex_pub)
        .map_err(|_| JsError::new("gateway KEX key is malformed"))?;
    if gateway_kex != ed2x(&gateway).to_bytes() {
        return Err(JsError::new(
            "gateway KEX key is not bound to its signing key",
        ));
    }
    aithos_core::wire::multibase_to_ed25519_pub(&request.session_pub)
        .map_err(|_| JsError::new("session signing key is malformed"))?;
    let constraints = request
        .constraints
        .as_object()
        .cloned()
        .ok_or_else(|| JsError::new("session constraints must be an object"))?;
    let mut constraints = Value::Object(constraints);
    constraints["session_bind"] = Value::String(request.session_pub);
    let leaf = Mandate::build_sub(
        &parent,
        key,
        &MandateSpec {
            id: request.id,
            subject: request.subject,
            grantee_id: request.grantee_id,
            grantee_label: request.grantee_label,
            grantee_pub: &gateway,
            perimeter: request
                .perimeter
                .iter()
                .map(|entry| PerimeterEntry::parse(entry))
                .collect::<aithos_core::Result<Vec<_>>>()
                .map_err(|error| JsError::new(&error.to_string()))?,
            constraints,
            not_before: request.not_before,
            not_after: request.not_after,
            issued_at: request.issued_at,
            nonce: request.nonce,
        },
    )
    .map_err(|error| JsError::new(&error.to_string()))?;
    serde_json::to_string(&leaf).map_err(|_| JsError::new("leaf serialization failed"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CeremonyBindings {
    transaction_id: String,
    delegate_pub: String,
    client_id: String,
    redirect_uri: String,
    resource: String,
    code_challenge: String,
    scope: Option<String>,
    state_digest: Option<String>,
    gateway_pub: String,
    gateway_kex_pub: String,
    session_pub: String,
    nonce: String,
    #[serde(rename = "expires_at_epoch")]
    _expires_at_epoch: i64,
}

/// Construct the exact gateway challenge and its presentation digest in
/// Rust, so JavaScript never implements JCS or hashes a non-canonical leaf.
#[wasm_bindgen]
pub fn build_ceremony_challenge(
    bindings_json: &str,
    context: &str,
    parent_id: &str,
    leaf_json: &str,
    grant_json: &str,
) -> Result<String, JsError> {
    let bindings: CeremonyBindings = serde_json::from_str(bindings_json)
        .map_err(|_| JsError::new("ceremony bindings are malformed"))?;
    let leaf: Value =
        serde_json::from_str(leaf_json).map_err(|_| JsError::new("session leaf is malformed"))?;
    let leaf_jcs = serde_jcs::to_vec(&leaf)
        .map_err(|_| JsError::new("session leaf is not canonicalizable"))?;
    let grant: Value = serde_json::from_str(grant_json)
        .map_err(|_| JsError::new("delegated grant is malformed"))?;
    let grant_jcs = serde_jcs::to_vec(&grant)
        .map_err(|_| JsError::new("delegated grant is not canonicalizable"))?;
    let challenge = serde_json::json!({
        "v": 1,
        "transaction_id": bindings.transaction_id,
        "delegate_pub": bindings.delegate_pub,
        "client_id": bindings.client_id,
        "redirect_uri": bindings.redirect_uri,
        "resource": bindings.resource,
        "code_challenge": bindings.code_challenge,
        "scope": bindings.scope,
        "state_digest": bindings.state_digest,
        "gateway_pub": bindings.gateway_pub,
        "gateway_kex_pub": bindings.gateway_kex_pub,
        "session_pub": bindings.session_pub,
        "nonce": bindings.nonce,
        "context": context,
        "parent_id": parent_id,
        "leaf_digest": format!(
            "sha256:{}",
            aithos_core::gamma::sha256_hex(&leaf_jcs)
        ),
        "grant_digest": format!(
            "sha256:{}",
            aithos_core::gamma::sha256_hex(&grant_jcs)
        ),
    });
    let challenge_jcs = serde_jcs::to_vec(&challenge)
        .map_err(|_| JsError::new("ceremony challenge is not canonicalizable"))?;
    Ok(serde_json::json!({
        "digest": format!(
            "sha256:{}",
            aithos_core::gamma::sha256_hex(&challenge_jcs)
        ),
        "challenge": challenge,
    })
    .to_string())
}

/// Sign one gateway-prepared existing Gamma v1 `grant` entry. The delegate
/// seed stays in WASM custody; only the signed public entry is returned.
#[wasm_bindgen]
pub fn sign_delegated_grant(delegate_seed: &mut [u8], grant_json: &str) -> Result<String, JsError> {
    let delegate_seed = DelegateSeed(delegate_seed);
    let key = signing_key(&delegate_seed)?;
    sign_delegated_grant_with_key(&key, grant_json)
}

fn sign_delegated_grant_with_key(key: &SigningKey, grant_json: &str) -> Result<String, JsError> {
    let mut entry: Entry = serde_json::from_str(grant_json)
        .map_err(|_| JsError::new("delegated grant is malformed"))?;
    let public = aithos_core::wire::ed25519_pub_to_multibase(&key.verifying_key().to_bytes());
    if entry.kind != "grant"
        || entry.signature.alg != "ed25519"
        || entry.signature.key != public
        || !entry.signature.value.is_empty()
        || entry.authorized_by.is_none()
        || entry.authorized_via.as_ref().is_none_or(Vec::is_empty)
    {
        return Err(JsError::new(
            "delegated grant is not an unsigned entry for this signer",
        ));
    }
    entry
        .check_form()
        .map_err(|error| JsError::new(&error.to_string()))?;
    let canonical = serde_jcs::to_vec(&entry)
        .map_err(|_| JsError::new("delegated grant is not canonicalizable"))?;
    entry.signature.value = hex::encode(key.sign(&canonical).to_bytes());
    serde_json::to_string(&entry).map_err(|_| JsError::new("delegated grant serialization failed"))
}

/// Canonicalize and sign the complete WYSIWYS challenge. The returned proof
/// is exactly what `/ceremony/complete` accepts.
#[wasm_bindgen]
pub fn sign_ceremony_challenge(
    delegate_seed: &mut [u8],
    challenge_json: &str,
) -> Result<String, JsError> {
    let delegate_seed = DelegateSeed(delegate_seed);
    let key = signing_key(&delegate_seed)?;
    sign_ceremony_challenge_with_key(&key, challenge_json)
}

fn sign_ceremony_challenge_with_key(
    key: &SigningKey,
    challenge_json: &str,
) -> Result<String, JsError> {
    let challenge: Value = serde_json::from_str(challenge_json)
        .map_err(|_| JsError::new("ceremony challenge is malformed"))?;
    let canonical = serde_jcs::to_vec(&challenge)
        .map_err(|_| JsError::new("ceremony challenge is not canonicalizable"))?;
    let digest = format!("sha256:{}", aithos_core::gamma::sha256_hex(&canonical));
    let mut preimage = b"aithos-gateway/mcp-ceremony/v1\x00".to_vec();
    preimage.extend_from_slice(&canonical);
    Ok(serde_json::json!({
        "aithos-mcp-ceremony": "1.0.0",
        "digest": digest,
        "delegate_pub": aithos_core::wire::ed25519_pub_to_multibase(
            &key.verifying_key().to_bytes()
        ),
        "sig": hex::encode(key.sign(&preimage).to_bytes()),
    })
    .to_string())
}

/// One short-lived signer retained only inside WASM for a browser ceremony.
/// Construction zeroizes the caller's plaintext buffer; `free()` drops and
/// zeroizes the Ed25519 signing key when the ceremony finishes.
#[wasm_bindgen]
pub struct DelegateSigner {
    key: SigningKey,
}

#[wasm_bindgen]
impl DelegateSigner {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: &mut [u8]) -> Result<DelegateSigner, JsError> {
        let seed = DelegateSeed(seed);
        Ok(DelegateSigner {
            key: signing_key(&seed)?,
        })
    }

    pub fn public_key(&self) -> String {
        aithos_core::wire::ed25519_pub_to_multibase(&self.key.verifying_key().to_bytes())
    }

    pub fn build_session_submandate(
        &self,
        parent_json: &str,
        request_json: &str,
    ) -> Result<String, JsError> {
        build_session_submandate_with_key(parent_json, &self.key, request_json)
    }

    pub fn sign_ceremony_challenge(&self, challenge_json: &str) -> Result<String, JsError> {
        sign_ceremony_challenge_with_key(&self.key, challenge_json)
    }

    pub fn sign_delegated_grant(&self, grant_json: &str) -> Result<String, JsError> {
        sign_delegated_grant_with_key(&self.key, grant_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceremony_surface_is_deterministic_and_never_returns_a_seed() {
        let mut seed = [7u8; 32];
        let public = delegate_pubkey(&mut seed).unwrap();
        assert_eq!(seed, [0u8; 32]);
        let mut seed = [7u8; 32];
        let proof = sign_ceremony_challenge(&mut seed, r#"{"v":1,"nonce":"n"}"#).unwrap();
        assert_eq!(seed, [0u8; 32]);
        let proof: Value = serde_json::from_str(&proof).unwrap();
        assert_eq!(proof["delegate_pub"], public);
        assert!(proof.get("seed").is_none());
        assert_eq!(proof.as_object().unwrap().len(), 4);

        let vector: Value = serde_json::from_str(include_str!(
            "../../../../vectors/cb15-external-delegated-grant.json"
        ))
        .unwrap();
        let mut seed = [0x62u8; 32];
        let signed = sign_delegated_grant(
            &mut seed,
            &serde_json::to_string(&vector["positive"]["unsigned_entry"]).unwrap(),
        )
        .unwrap();
        assert_eq!(seed, [0u8; 32]);
        assert_eq!(
            serde_json::from_str::<Value>(&signed).unwrap(),
            vector["positive"]["signed_entry"]
        );
    }
}
