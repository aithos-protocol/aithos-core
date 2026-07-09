//! DID document and identity-epoch transition (spec §01.4, §10.4).
//!
//! Signing convention (shared with the manifest, §02.6): signatures cover the
//! JCS of the document with `signature.value = ""`; `value` is hex Ed25519.

use crate::error::{Error, Result};
use crate::jcs;
use crate::keys::OwnerKeys;
use crate::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const DID_VERSION: &str = "1.0.0-draft.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidKeys {
    pub content: String,
    pub kex: String,
    pub root: String,
    pub succession: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureBlock {
    pub alg: String,
    pub key: String,
    pub value: String,
}

/// The published identity (spec §01.4). Fail-closed everywhere: any
/// inconsistency makes the whole document invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidDocument {
    #[serde(rename = "aithos-did-core")]
    pub version: String,
    pub bundle: Vec<String>,
    pub id: String,
    pub keys: DidKeys,
    pub revocations: String,
    pub signature: SignatureBlock,
}

fn signed_hex(key: &SigningKey, unsigned_jcs: &[u8]) -> String {
    hex::encode(key.sign(unsigned_jcs).to_bytes())
}

fn verify_hex(pubkey: &VerifyingKey, unsigned_jcs: &[u8], hex_sig: &str) -> bool {
    let Ok(bytes) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(sig_bytes) = <[u8; 64]>::try_from(bytes) else {
        return false;
    };
    pubkey
        .verify(unsigned_jcs, &Signature::from_bytes(&sig_bytes))
        .is_ok()
}

impl DidDocument {
    /// Build and root-sign the DID document.
    pub fn build(
        owner: &OwnerKeys,
        succession_pub: &VerifyingKey,
        bundle: Vec<String>,
        revocations: String,
    ) -> Result<Self> {
        let root_pub = owner.root_sign.verifying_key().to_bytes();
        let mut doc = DidDocument {
            version: DID_VERSION.to_owned(),
            bundle,
            id: wire::did_aithos(&root_pub),
            keys: DidKeys {
                content: wire::ed25519_pub_to_multibase(
                    &owner.content_sign.verifying_key().to_bytes(),
                ),
                kex: wire::x25519_pub_to_multibase(&owner.owner_kex_pub().to_bytes()),
                root: wire::ed25519_pub_to_multibase(&root_pub),
                succession: wire::ed25519_pub_to_multibase(&succession_pub.to_bytes()),
            },
            revocations,
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: "#root".to_owned(),
                value: String::new(),
            },
        };
        doc.signature.value = signed_hex(&owner.root_sign, &jcs::canonical_bytes(&doc)?);
        Ok(doc)
    }

    /// Fail-closed verification: id ↔ root binding, then root signature.
    pub fn verify(&self) -> Result<()> {
        let err = |m: &str| Error::InvalidDidDocument(m.to_owned());
        let root_bytes = wire::multibase_to_ed25519_pub(&self.keys.root)?;
        if self.id != wire::did_aithos(&root_bytes) {
            return Err(err("id does not match the root key"));
        }
        let root = VerifyingKey::from_bytes(&root_bytes).map_err(|_| err("malformed root key"))?;
        let mut unsigned = self.clone();
        unsigned.signature.value = String::new();
        if !verify_hex(
            &root,
            &jcs::canonical_bytes(&unsigned)?,
            &self.signature.value,
        ) {
            return Err(err("signature does not verify under the root key"));
        }
        Ok(())
    }

    pub fn succession_pub(&self) -> Result<VerifyingKey> {
        let bytes = wire::multibase_to_ed25519_pub(&self.keys.succession)?;
        VerifyingKey::from_bytes(&bytes)
            .map_err(|_| Error::InvalidDidDocument("malformed succession key".to_owned()))
    }
}

/// Identity-epoch transition (spec §01.4, §10.4): the only artifact the
/// succession key ever signs. Declares `next_did` as the successor of
/// `prev_did` after seed compromise or loss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochTransition {
    #[serde(rename = "aithos-epoch-core")]
    pub version: String,
    pub at: String,
    pub next_did: String,
    pub prev_did: String,
    pub signature: SignatureBlock,
}

impl EpochTransition {
    /// Sign a transition with an arbitrary key and fragment. The canonical,
    /// valid form uses the succession key (`#succession`); other signers exist
    /// so tests can prove they are rejected.
    pub fn sign_with(
        key: &SigningKey,
        fragment: &str,
        prev_did: String,
        next_did: String,
        at: String,
    ) -> Result<Self> {
        let mut tr = EpochTransition {
            version: DID_VERSION.to_owned(),
            at,
            next_did,
            prev_did,
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: fragment.to_owned(),
                value: String::new(),
            },
        };
        tr.signature.value = signed_hex(key, &jcs::canonical_bytes(&tr)?);
        Ok(tr)
    }

    /// Canonical form: signed by the succession key.
    pub fn sign(
        succession: &SigningKey,
        prev_did: String,
        next_did: String,
        at: String,
    ) -> Result<Self> {
        Self::sign_with(succession, "#succession", prev_did, next_did, at)
    }

    /// Fail-closed: only the PREVIOUS document's succession key may declare a
    /// successor. Anything else — including `#root` itself — is rejected.
    pub fn verify(&self, prev_doc: &DidDocument) -> Result<()> {
        let err = |m: &str| Error::InvalidEpochTransition(m.to_owned());
        prev_doc.verify()?;
        if self.prev_did != prev_doc.id {
            return Err(err("prev_did does not match the previous document"));
        }
        if self.signature.key != "#succession" {
            return Err(err("a transition must be signed by the succession key"));
        }
        let succession = prev_doc.succession_pub()?;
        let mut unsigned = self.clone();
        unsigned.signature.value = String::new();
        if !verify_hex(
            &succession,
            &jcs::canonical_bytes(&unsigned)?,
            &self.signature.value,
        ) {
            return Err(err("signature does not verify under the succession key"));
        }
        Ok(())
    }
}
