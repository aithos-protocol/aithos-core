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

/// The only signature algorithm the identity layer speaks (§01.4).
pub const SIGNATURE_ALG: &str = "ed25519";
/// The only fragment that may sign a DID document (§01.4).
pub const ROOT_FRAGMENT: &str = "#root";
/// The only fragment that may sign an identity-epoch transition (§10.4).
pub const SUCCESSION_FRAGMENT: &str = "#succession";

/// The four published keys (§01.4). The schema is CLOSED: an unknown member
/// must not survive deserialization, because the verified JCS is rebuilt from
/// the typed value — a dropped member would be a signed-then-erased field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DidKeys {
    pub content: String,
    pub kex: String,
    pub root: String,
    pub succession: String,
}

/// Detached signature block, shared by every signed protocol object. Closed
/// for the same reason as [`DidKeys`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureBlock {
    pub alg: String,
    pub key: String,
    pub value: String,
}

/// The published identity (spec §01.4). Fail-closed everywhere: any
/// inconsistency makes the whole document invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

    /// Fail-closed verification (§01.4). A correct root signature is necessary
    /// but never sufficient: the declared version, the signature metadata and
    /// the four published keys are all validated BEFORE the signature, so a
    /// correctly signed but semantically malformed document is still refused.
    ///
    /// Checked, in order:
    /// 1. `aithos-did-core == DID_VERSION`;
    /// 2. `signature.alg == "ed25519"` and `signature.key == "#root"`;
    /// 3. `keys.root`, `keys.content`, `keys.succession` decode as Ed25519
    ///    public keys and `keys.kex` decodes as an X25519 public key, each
    ///    under its OWN multicodec — a key in the wrong codec is refused;
    /// 4. `id == did:aithos:<root>`;
    /// 5. the Ed25519 signature verifies under that same root key.
    ///
    /// Unknown wire members never reach this point: the schema is closed
    /// (`deny_unknown_fields`), so they are refused at deserialization
    /// instead of being silently dropped before the JCS is rebuilt.
    pub fn verify(&self) -> Result<()> {
        let err = |m: &str| Error::InvalidDidDocument(m.to_owned());
        if self.version != DID_VERSION {
            return Err(err("unsupported aithos-did-core version"));
        }
        if self.signature.alg != SIGNATURE_ALG {
            return Err(err("unsupported signature algorithm"));
        }
        if self.signature.key != ROOT_FRAGMENT {
            return Err(err("a DID document must be signed by #root"));
        }
        let root_bytes = wire::multibase_to_ed25519_pub(&self.keys.root)
            .map_err(|_| err("malformed root key"))?;
        let content_bytes = wire::multibase_to_ed25519_pub(&self.keys.content)
            .map_err(|_| err("malformed content key"))?;
        let succession_bytes = wire::multibase_to_ed25519_pub(&self.keys.succession)
            .map_err(|_| err("malformed succession key"))?;
        wire::multibase_to_x25519_pub(&self.keys.kex).map_err(|_| err("malformed kex key"))?;
        VerifyingKey::from_bytes(&content_bytes).map_err(|_| err("malformed content key"))?;
        VerifyingKey::from_bytes(&succession_bytes).map_err(|_| err("malformed succession key"))?;
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
#[serde(deny_unknown_fields)]
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

    /// Verify the transition **as a declaration only**: it says nothing about
    /// the successor document, which is not supplied here.
    ///
    /// Fail-closed on everything it CAN see: the previous document is fully
    /// verified, `prev_did` binds to it, the version and signature metadata
    /// are checked, `next_did` is a well-formed `did:aithos:` identifier
    /// distinct from `prev_did`, and only the previous document's succession
    /// key may sign — `#root` itself is rejected.
    ///
    /// A caller that must decide whether to ACCEPT a successor document has to
    /// call [`EpochTransition::verify_succession`] instead: a declaration
    /// alone can name a successor that is never presented, never verified, or
    /// different from the one the caller then installs.
    pub fn verify_declaration(&self, prev_doc: &DidDocument) -> Result<()> {
        let err = |m: &str| Error::InvalidEpochTransition(m.to_owned());
        prev_doc.verify()?;
        if self.version != DID_VERSION {
            return Err(err("unsupported aithos-epoch-core version"));
        }
        if self.signature.alg != SIGNATURE_ALG {
            return Err(err("unsupported signature algorithm"));
        }
        if self.signature.key != SUCCESSION_FRAGMENT {
            return Err(err("a transition must be signed by the succession key"));
        }
        if self.prev_did != prev_doc.id {
            return Err(err("prev_did does not match the previous document"));
        }
        let next_root = self
            .next_did
            .strip_prefix("did:aithos:")
            .ok_or_else(|| err("next_did is not a did:aithos identifier"))?;
        wire::multibase_to_ed25519_pub(next_root).map_err(|_| err("next_did is malformed"))?;
        if self.next_did == self.prev_did {
            return Err(err("next_did must differ from prev_did"));
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

    /// Fail-closed succession verification (§10.4) — the only verdict that
    /// authorizes replacing `prev_doc` with `next_doc`.
    ///
    /// Verifies the whole triple, not just the declaration: BOTH documents
    /// pass the strict [`DidDocument::verify`], the transition is a valid
    /// declaration under `prev_doc`, `next_did` binds to the document actually
    /// presented, and the two identities are distinct. Presenting any other
    /// successor document than the one named is rejected.
    pub fn verify_succession(&self, prev_doc: &DidDocument, next_doc: &DidDocument) -> Result<()> {
        let err = |m: &str| Error::InvalidEpochTransition(m.to_owned());
        self.verify_declaration(prev_doc)?;
        next_doc
            .verify()
            .map_err(|e| err(&format!("successor document is invalid: {e}")))?;
        if self.next_did != next_doc.id {
            return Err(err("next_did does not match the successor document"));
        }
        if prev_doc.id == next_doc.id {
            return Err(err("the successor must be a different identity"));
        }
        Ok(())
    }
}
