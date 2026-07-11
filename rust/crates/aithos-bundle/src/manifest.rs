//! The signed manifest and the edition chain (spec §02.6).
//!
//! Until step H (Merkle state roots), the manifest pins files flat:
//! `files: {path -> sha256}` — the planned attachment point for the roots.

use aithos_core::did::{DidDocument, SignatureBlock};
use aithos_core::error::{Error, Result};
use aithos_core::jcs;
use aithos_core::wire;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CORE_VERSION: &str = "1.0.0-draft.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edition {
    pub height: u64,
    /// SHA-256 hex of the prior manifest's JCS with `signature.value=""`;
    /// empty for edition 1.
    pub prev_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "aithos-core")]
    pub version: String,
    pub edition: Edition,
    /// Flat file pins — kept BESIDE the Merkle roots (decided 2026-07-11):
    /// they still cover byte-rollback of sealed `self` blobs (§02.8).
    pub files: BTreeMap<String, String>,
    /// Merkle state roots (§02.10, pass H1): `public`/`circle`/`self`/
    /// `vault` → hex root. Empty on pre-H editions (absent from their
    /// signed bytes, so old chain hashes are untouched).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roots: BTreeMap<String, String>,
    /// Committed gamma segment roots (§07.10, pass H2): `YYYY-MM` →
    /// root+count, one per non-empty segment. Additive like `roots`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gamma_roots: BTreeMap<String, GammaSegmentRoot>,
    /// The counts-trie root (§07.10), hex — 32×0x00 when nothing was ever
    /// counted; absent only on pre-H2 editions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gamma_counts_root: String,
    /// `sha256:<hex>` of the last gamma entry's JCS (§02.7); empty when the
    /// log is empty.
    #[serde(default)]
    pub gamma_head: String,
    /// Disjoint-merge parents (§02.6, pass I): the two competing edition
    /// hashes, ascending — `edition.prev_hash` pins the first. Additive:
    /// absent from non-merge editions, pre-I chain hashes untouched.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<String>,
    /// Fork resolution (§02.6, pass I): the winning parent's edition hash —
    /// this edition's own `prev_hash` — named by the nearest common manager.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resolves_fork: String,
    /// Delegate-signed editions (§02.6): the full mandate chain ids, leaf
    /// last — mirrors gamma's `authorized_via`. Empty = root-signed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authorized_via: Vec<String>,
    pub signature: SignatureBlock,
}

/// One committed segment (§07.10): the chain-order root over the exact
/// line bytes, plus the entry count that leaves enumeration nowhere to hide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GammaSegmentRoot {
    pub root: String,
    pub n: u64,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

impl Manifest {
    fn unsigned_jcs(&self) -> Result<Vec<u8>> {
        let mut unsigned = self.clone();
        unsigned.signature.value = String::new();
        jcs::canonical_bytes(&unsigned)
    }

    /// The hash a successor pins as `prev_hash`.
    pub fn chain_hash(&self) -> Result<String> {
        Ok(sha256_hex(&self.unsigned_jcs()?))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        root_sign: &ed25519_dalek::SigningKey,
        height: u64,
        prev_hash: String,
        created_at: String,
        files: BTreeMap<String, String>,
        roots: BTreeMap<String, String>,
        gamma_roots: BTreeMap<String, GammaSegmentRoot>,
        gamma_counts_root: String,
        gamma_head: String,
    ) -> Result<Self> {
        Self::build_spec(
            ManifestSpec {
                height,
                prev_hash,
                created_at,
                files,
                roots,
                gamma_roots,
                gamma_counts_root,
                gamma_head,
                merges: Vec::new(),
                resolves_fork: String::new(),
                authorized_via: Vec::new(),
            },
            ManifestSigner::Root(root_sign),
        )
    }

    /// Build and sign a manifest from the full pass-I spec: plain editions,
    /// disjoint merges (`merges`) and fork resolutions (`resolves_fork`),
    /// root- or delegate-signed.
    pub fn build_spec(spec: ManifestSpec, signer: ManifestSigner<'_>) -> Result<Self> {
        let (key, sk) = match signer {
            ManifestSigner::Root(sk) => ("#root".to_owned(), sk),
            ManifestSigner::Delegate { key_multibase, sk } => (key_multibase, sk),
        };
        let mut m = Manifest {
            version: CORE_VERSION.to_owned(),
            edition: Edition {
                height: spec.height,
                prev_hash: spec.prev_hash,
                created_at: spec.created_at,
            },
            files: spec.files,
            roots: spec.roots,
            gamma_roots: spec.gamma_roots,
            gamma_counts_root: spec.gamma_counts_root,
            gamma_head: spec.gamma_head,
            merges: spec.merges,
            resolves_fork: spec.resolves_fork,
            authorized_via: spec.authorized_via,
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key,
                value: String::new(),
            },
        };
        m.signature.value = hex::encode(sk.sign(&m.unsigned_jcs()?).to_bytes());
        Ok(m)
    }

    /// Verify this manifest's signature against the DID document's root key.
    pub fn verify_signature(&self, did_doc: &DidDocument) -> Result<()> {
        let err = |m: &str| Error::InvalidDidDocument(format!("manifest: {m}"));
        if self.signature.key != "#root" {
            return Err(err("manifest must be root-signed"));
        }
        let root_bytes = wire::multibase_to_ed25519_pub(&did_doc.keys.root)?;
        let root = VerifyingKey::from_bytes(&root_bytes).map_err(|_| err("malformed root key"))?;
        let sig_bytes: [u8; 64] = hex::decode(&self.signature.value)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| err("bad signature encoding"))?;
        root.verify(&self.unsigned_jcs()?, &Signature::from_bytes(&sig_bytes))
            .map_err(|_| err("signature does not verify under the root key"))
    }

    /// Verify a delegate-signed manifest (§02.6): the signature key IS the
    /// leaf grantee key of the presented chain; chain validity and node
    /// authority are the CALLER's checks (they need the certs and the trees).
    pub fn verify_delegate_signature(&self, leaf: &aithos_core::mandate::Mandate) -> Result<()> {
        let err = |m: &str| Error::InvalidDidDocument(format!("manifest: {m}"));
        if self.authorized_via.is_empty() {
            return Err(err("delegate verification on a root-signed manifest"));
        }
        if self.authorized_via.last() != Some(&leaf.id) {
            return Err(err(
                "authorized_via leaf does not match the presented chain",
            ));
        }
        if self.signature.key != leaf.grantee.pubkey {
            return Err(err("manifest is not signed by the leaf grantee key"));
        }
        let sig_bytes: [u8; 64] = hex::decode(&self.signature.value)
            .ok()
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| err("bad signature encoding"))?;
        leaf.grantee_pub()?
            .verify(&self.unsigned_jcs()?, &Signature::from_bytes(&sig_bytes))
            .map_err(|_| err("signature does not verify under the grantee key"))
    }
}

/// Parameters of one signed edition (pass I) — `Manifest::build_spec`.
#[derive(Debug, Clone)]
pub struct ManifestSpec {
    pub height: u64,
    pub prev_hash: String,
    pub created_at: String,
    pub files: BTreeMap<String, String>,
    pub roots: BTreeMap<String, String>,
    pub gamma_roots: BTreeMap<String, GammaSegmentRoot>,
    pub gamma_counts_root: String,
    pub gamma_head: String,
    pub merges: Vec<String>,
    pub resolves_fork: String,
    pub authorized_via: Vec<String>,
}

/// Who signs an edition: the owner root, or a delegate under its chain.
pub enum ManifestSigner<'a> {
    Root(&'a ed25519_dalek::SigningKey),
    Delegate {
        /// The leaf grantee's Ed25519 public key, multibase — the manifest's
        /// `signature.key`.
        key_multibase: String,
        sk: &'a ed25519_dalek::SigningKey,
    },
}
