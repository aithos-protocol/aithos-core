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
    /// Flat file pins until step H replaces them with Merkle state roots.
    pub files: BTreeMap<String, String>,
    /// `sha256:<hex>` of the last gamma entry's JCS (§02.7); empty when the
    /// log is empty.
    #[serde(default)]
    pub gamma_head: String,
    pub signature: SignatureBlock,
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

    pub fn build(
        root_sign: &ed25519_dalek::SigningKey,
        height: u64,
        prev_hash: String,
        created_at: String,
        files: BTreeMap<String, String>,
        gamma_head: String,
    ) -> Result<Self> {
        let mut m = Manifest {
            version: CORE_VERSION.to_owned(),
            edition: Edition {
                height,
                prev_hash,
                created_at,
            },
            files,
            gamma_head,
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: "#root".to_owned(),
                value: String::new(),
            },
        };
        m.signature.value = hex::encode(root_sign.sign(&m.unsigned_jcs()?).to_bytes());
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
}
