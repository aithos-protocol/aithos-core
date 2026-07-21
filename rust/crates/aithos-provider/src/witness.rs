//! Witness checkpoints — annexe C, contrat C3 (lot P5).
//!
//! The witness signs **observations, never authority** (§4): it attests
//! having seen a `(did, edition_height, manifest_hash)`, it does not
//! validate a manifest. A checkpoint verifies alone (signature under a
//! published `witness_key`); the equivocation rule (C.4) makes two
//! incompatible checkpoints a **portable proof** — no store access needed.
//!
//! **P5 shape:** the signing key is `ECC_NIST_EDWARDS25519` in AWS KMS,
//! true sign-only (`ED25519_SHA_512`, `MessageType: RAW`, never the
//! prehashed `_PH_` mode) — the key is born in KMS and never leaves; each
//! signature is IAM-gated and CloudTrail-traced. That signer is a deploy
//! seam ([`WitnessSigner`]); every byte-level format here — checkpoint,
//! feed line, daily root, equivocation — is proven against the committed
//! `p4` vector with a local key, and the KMS-produced signatures are
//! byte-identical (pure Ed25519, 64 octets) to any verifier of the stack.

use std::collections::BTreeSet;

use aithos_core::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// The witness wire version (annexe C.1). `ed25519` is the only `alg` at
/// the registry in `draft.1`.
pub const WITNESS_WIRE_VERSION: &str = "1.0.0-draft.1";

const MK_LEAF: &[u8] = b"aithos-witness/v1/mk-leaf\x00";
const MK_NODE: &[u8] = b"aithos-witness/v1/mk-node\x00";

/// A signed checkpoint (annexe C.1). Field order under JCS: aithos-witness,
/// did, edition_height, gamma_head, manifest_hash, observed_at, signature,
/// witness_key — the exact `p4` layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    #[serde(rename = "aithos-witness")]
    pub version: String,
    pub did: String,
    pub edition_height: u64,
    pub manifest_hash: String,
    pub gamma_head: String,
    pub observed_at: String,
    pub witness_key: String,
    pub signature: WitnessSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessSignature {
    pub alg: String,
    pub value: String,
}

/// The daily aggregated root (annexe C.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyRoot {
    #[serde(rename = "aithos-witness-root")]
    pub version: String,
    pub date: String,
    pub root: String,
    pub n: u64,
    pub witness_key: String,
    pub signature: WitnessSignature,
}

/// The deploy seam: a sign-only witness key. The local impl (tests,
/// tooling) holds an Ed25519 key; the deployed impl calls AWS KMS. Both
/// yield the same 64-octet Ed25519 signature.
pub trait WitnessSigner: Send + Sync {
    /// The published `witness_key` (multibase Ed25519).
    fn witness_key(&self) -> String;
    /// Sign the canonical unsigned bytes (JCS with `value=""`).
    fn sign(&self, unsigned_jcs: &[u8]) -> String;
}

/// Local signer — **tooling and tests only**. Production signs in KMS
/// (the key never lands in a process).
pub struct LocalWitnessSigner {
    key: SigningKey,
}

impl LocalWitnessSigner {
    pub fn new(key: SigningKey) -> Self {
        Self { key }
    }
}

impl WitnessSigner for LocalWitnessSigner {
    fn witness_key(&self) -> String {
        wire::ed25519_pub_to_multibase(&self.key.verifying_key().to_bytes())
    }
    fn sign(&self, unsigned_jcs: &[u8]) -> String {
        hex::encode(self.key.sign(unsigned_jcs).to_bytes())
    }
}

/// Build and sign a checkpoint observing `(did, height, manifest_hash,
/// gamma_head)` at `observed_at`.
pub fn build_checkpoint(
    signer: &dyn WitnessSigner,
    did: &str,
    edition_height: u64,
    manifest_hash: &str,
    gamma_head: &str,
    observed_at: &str,
) -> Checkpoint {
    let mut checkpoint = Checkpoint {
        version: WITNESS_WIRE_VERSION.into(),
        did: did.to_owned(),
        edition_height,
        manifest_hash: manifest_hash.to_owned(),
        gamma_head: gamma_head.to_owned(),
        observed_at: observed_at.to_owned(),
        witness_key: signer.witness_key(),
        signature: WitnessSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let unsigned = serde_jcs::to_string(&checkpoint).expect("checkpoint serializable");
    checkpoint.signature.value = signer.sign(unsigned.as_bytes());
    checkpoint
}

/// The published registry of accepted witness keys
/// (`witness.aithos.fr/keys.json`): the set of `witness_key` multibase
/// strings a verifier will trust (annexe C.1/C.4). A one-key registry is
/// the common case; rotation publishes the new key beside the old.
pub type WitnessKeyRegistry = BTreeSet<String>;

/// Verify a checkpoint against the published registry (annexe C.4):
/// **both** the `witness_key` is in `registry` **and** the self-signature
/// checks under it. Fail-closed — a key outside the registry, a bad
/// signature, or a wrong `alg`/`version` all reject. This is the safe
/// default: it never trusts a key a checkpoint merely names.
pub fn verify_checkpoint(checkpoint: &Checkpoint, registry: &WitnessKeyRegistry) -> bool {
    registry.contains(&checkpoint.witness_key)
        && checkpoint.version == WITNESS_WIRE_VERSION
        && checkpoint.signature.alg == "ed25519"
        && verify_witness_doc(checkpoint, &checkpoint.witness_key, |c| {
            serde_jcs::to_string(c).ok()
        })
}

/// The exact feed line for a checkpoint: its signed JCS bytes (annexe C.3
/// — "exactement les octets JCS signés", rejouables tel quel).
pub fn feed_line(checkpoint: &Checkpoint) -> String {
    serde_jcs::to_string(checkpoint).expect("checkpoint serializable")
}

/// Build and sign the daily root over the day's feed lines (annexe C.3):
/// leaves = `H_leaf(JCS bytes)` over ALL lines of the UTC day (every DID),
/// **sorted by JCS byte order, deduplicated**; `mroot` is the left-heavy
/// balanced binary tree of §02.10 with the dedicated witness domains.
pub fn build_daily_root(signer: &dyn WitnessSigner, date: &str, day_lines: &[String]) -> DailyRoot {
    let mut lines: Vec<String> = day_lines.to_vec();
    lines.sort();
    lines.dedup();
    let leaves: Vec<[u8; 32]> = lines.iter().map(|l| h_leaf(l.as_bytes())).collect();
    let root = mroot(&leaves);
    let mut doc = DailyRoot {
        version: WITNESS_WIRE_VERSION.into(),
        date: date.to_owned(),
        root: hex::encode(root),
        n: lines.len() as u64,
        witness_key: signer.witness_key(),
        signature: WitnessSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let unsigned = serde_jcs::to_string(&doc).expect("root serializable");
    doc.signature.value = signer.sign(unsigned.as_bytes());
    doc
}

pub fn verify_daily_root(root: &DailyRoot, registry: &WitnessKeyRegistry) -> bool {
    registry.contains(&root.witness_key)
        && root.version == WITNESS_WIRE_VERSION
        && root.signature.alg == "ed25519"
        && verify_witness_doc(root, &root.witness_key, |r| serde_jcs::to_string(r).ok())
}

/// The published key registry document — `witness.aithos.fr/keys.json`
/// (annexe C.1: « un vérificateur accepte toute clé du registre publié
/// des clés témoin, signé par la clé sortante »). The annexe names the
/// file and its signer without fixing a format; this concrete shape is an
/// additive service-side definition (lot A / P5, consigné au handoff):
/// rotation publishes the NEW key inside `keys` while `witness_key` (the
/// signing key) stays the outgoing one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessKeys {
    #[serde(rename = "aithos-witness-keys")]
    pub version: String,
    pub keys: Vec<String>,
    pub witness_key: String,
    pub signature: WitnessSignature,
}

/// Build and sign the key registry document.
pub fn build_keys_doc(signer: &dyn WitnessSigner, keys: &[String]) -> WitnessKeys {
    let mut doc = WitnessKeys {
        version: WITNESS_WIRE_VERSION.into(),
        keys: keys.to_vec(),
        witness_key: signer.witness_key(),
        signature: WitnessSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let unsigned = serde_jcs::to_string(&doc).expect("keys doc serializable");
    doc.signature.value = signer.sign(unsigned.as_bytes());
    doc
}

/// Verify the key registry document: version and `alg` pinned, the
/// signing key listed in its own registry, self-signature valid.
pub fn verify_keys_doc(doc: &WitnessKeys) -> bool {
    doc.version == WITNESS_WIRE_VERSION
        && doc.signature.alg == "ed25519"
        && doc.keys.contains(&doc.witness_key)
        && verify_witness_doc(doc, &doc.witness_key, |d| serde_jcs::to_string(d).ok())
}

/// The equivocation rule (annexe C.4): two checkpoints, **both valid under
/// the published registry**, same `did`, same `edition_height`,
/// **different** `manifest_hash` = portable proof. Same `manifest_hash`
/// re-observed (heartbeat) is freshness; a different height is a chain,
/// not a fork. Requiring registry validity is what makes the proof
/// portable and unforgeable — two checkpoints signed by keys nobody
/// published are not evidence of anything.
pub fn is_equivocation(a: &Checkpoint, b: &Checkpoint, registry: &WitnessKeyRegistry) -> bool {
    verify_checkpoint(a, registry)
        && verify_checkpoint(b, registry)
        && a.did == b.did
        && a.edition_height == b.edition_height
        && a.manifest_hash != b.manifest_hash
}

// ------------------------------------------------------------- hashing

fn h_leaf(payload: &[u8]) -> [u8; 32] {
    let mut input = Vec::with_capacity(MK_LEAF.len() + payload.len());
    input.extend_from_slice(MK_LEAF);
    input.extend_from_slice(payload);
    *blake3::hash(&input).as_bytes()
}

fn h_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut input = Vec::with_capacity(MK_NODE.len() + 64);
    input.extend_from_slice(MK_NODE);
    input.extend_from_slice(left);
    input.extend_from_slice(right);
    *blake3::hash(&input).as_bytes()
}

/// Left-heavy balanced binary tree (§02.10): `mid = (n + 1) / 2` sends the
/// extra leaf left. Empty = 32 zero bytes; single leaf = itself.
fn mroot(hashes: &[[u8; 32]]) -> [u8; 32] {
    match hashes.len() {
        0 => [0u8; 32],
        1 => hashes[0],
        n => {
            let mid = n.div_ceil(2);
            h_node(&mroot(&hashes[..mid]), &mroot(&hashes[mid..]))
        }
    }
}

/// Shared §01.4 verify: a document self-verifies when the Ed25519
/// signature over its JCS-with-`value=""` checks under `key_mb`.
fn verify_witness_doc<T, F>(doc: &T, key_mb: &str, mut to_jcs: F) -> bool
where
    T: Serialize + Clone,
    F: FnMut(&serde_json::Value) -> Option<String>,
{
    let Ok(value) = serde_json::to_value(doc) else {
        return false;
    };
    let mut unsigned = value.clone();
    let sig_hex = unsigned["signature"]["value"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    unsigned["signature"]["value"] = serde_json::Value::String(String::new());
    let Some(unsigned_jcs) = to_jcs(&unsigned) else {
        return false;
    };
    let Ok(key_bytes) = wire::multibase_to_ed25519_pub(key_mb) else {
        return false;
    };
    let Ok(verifying) = VerifyingKey::from_bytes(&key_bytes) else {
        return false;
    };
    let Some(sig) = hex::decode(&sig_hex)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
    else {
        return false;
    };
    verifying
        .verify(unsigned_jcs.as_bytes(), &Signature::from_bytes(&sig))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_jcs_field_order_matches_the_committed_wire() {
        // Seed 0x77… is the p4 witness key; its multibase is the vector's.
        let signer = LocalWitnessSigner::new(SigningKey::from_bytes(&[0x77; 32]));
        assert_eq!(
            signer.witness_key(),
            "z6MkswFb62xmEDrqnknM3TP112AiH6A5YETp7gc2Qz4Wqkar"
        );
        let ck = build_checkpoint(
            &signer,
            "did:aithos:zX",
            2,
            "sha256:aa",
            "",
            "2026-07-16T11:35:00Z",
        );
        let jcs = feed_line(&ck);
        assert!(jcs.starts_with(r#"{"aithos-witness":"1.0.0-draft.1","did":"did:aithos:zX","edition_height":2,"gamma_head":"","manifest_hash":"sha256:aa","observed_at":"2026-07-16T11:35:00Z","signature":"#));
        assert!(
            jcs.ends_with(r#""witness_key":"z6MkswFb62xmEDrqnknM3TP112AiH6A5YETp7gc2Qz4Wqkar"}"#)
        );
        let registry = WitnessKeyRegistry::from([signer.witness_key()]);
        assert!(verify_checkpoint(&ck, &registry));
        // A key outside the published registry never verifies — even with
        // a perfect self-signature (the safe-default gate, annexe C.4).
        assert!(!verify_checkpoint(&ck, &WitnessKeyRegistry::new()));
    }

    #[test]
    fn empty_and_single_mroot_edges() {
        assert_eq!(mroot(&[]), [0u8; 32]);
        let one = h_leaf(b"x");
        assert_eq!(mroot(&[one]), one);
    }
}
