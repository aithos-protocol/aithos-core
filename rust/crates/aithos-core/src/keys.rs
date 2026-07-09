//! Owner identity and key genesis (spec §01.1, §01.2).

use crate::derive::{derive_key, CTX_CONTENT_SIGN, CTX_OWNER_KEX, CTX_ROOT_SIGN};
use crate::error::{Error, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// The owner's master seed `S` (§01.1) — the only thing to back up.
/// Never leaves the owner's devices; zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MasterSeed([u8; 32]);

impl MasterSeed {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::InvalidSeedLength(bytes.len()))?;
        Ok(Self(arr))
    }
}

/// Everything derived from `S` (§01.1) — recomputed on demand, never stored.
pub struct OwnerKeys {
    /// Controls the DID document; issues/revokes first-level mandates.
    /// Reserved for structural acts; never signs content.
    pub root_sign: SigningKey,
    /// The owner's single pen (§02.11): signs owner-authored gamma entries and
    /// content where the zone's policy says so. The audience lives in the
    /// signed payload ({zone, path}), never in the key.
    pub content_sign: SigningKey,
    /// Recipient key of the owner's line in every header (I3).
    pub owner_kex: StaticSecret,
}

impl OwnerKeys {
    /// Deterministic genesis (§01.1) — conformance vector A1.
    pub fn genesis(seed: &MasterSeed) -> Self {
        let s = &seed.0;
        OwnerKeys {
            root_sign: SigningKey::from_bytes(&derive_key(CTX_ROOT_SIGN, s)),
            content_sign: SigningKey::from_bytes(&derive_key(CTX_CONTENT_SIGN, s)),
            owner_kex: StaticSecret::from(derive_key(CTX_OWNER_KEX, s)),
        }
    }

    pub fn owner_kex_pub(&self) -> XPublicKey {
        XPublicKey::from(&self.owner_kex)
    }
}

/// Normative Ed25519 → X25519 conversion (§01.2, §04.1): the birational map
/// to Montgomery form, byte-identical to libsodium's
/// `crypto_sign_ed25519_pk_to_curve25519`. A mandate's `kex_pubkey` MUST
/// equal `ed2x(pubkey)` — a mismatch invalidates the mandate.
#[must_use]
pub fn ed2x(pubkey: &VerifyingKey) -> XPublicKey {
    XPublicKey::from(pubkey.to_montgomery().to_bytes())
}
