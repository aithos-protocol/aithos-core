#![forbid(unsafe_code)]
//! aithos-owner — the owner-side ceremonies of the protocol.
//!
//! Extracted from the gateway's `core_bridge` owner block at lot SPL-4 of
//! the split chantier: mandate minting, context/journal equipment and the
//! owner read surface, generic over any [`aithos_bundle::Store`]. The CLI
//! and the gateway both consume these ceremonies; neither redefines them.
//! Hub-manifest enrollment and tool-policy previews stay gateway-side
//! (famille B): `ApprovedManifest` and the tool policy are gateway domain,
//! not protocol.

use std::io;

use aithos_bundle::Store;

/// Every way an owner ceremony can refuse or fail. Mirrors the callers'
/// fail-closed taxonomy: [`OwnerError::Rejected`] converts to their
/// config-rejection variant, [`OwnerError::Failed`] to their bridge
/// failure — messages ride unchanged.
#[derive(Debug, thiserror::Error)]
pub enum OwnerError {
    /// Input, label or store content is malformed or ambiguous — fail
    /// closed before any write.
    #[error("config rejected: {0}")]
    Rejected(String),
    /// Store or protocol failure that is not a policy rejection.
    #[error("core bridge failed: {0}")]
    Failed(String),
}

/// Ceremony-wide result type.
pub type Result<T> = std::result::Result<T, OwnerError>;

/// The store a ceremony opens. [`OwnerStore::legacy_state_bytes`] lets a
/// deployment-specific store surface a pre-SPL-2 bridge state for the
/// key migration (the legacy key is outside the canonical grammar, so it
/// cannot ride [`Store::get`]); plain protocol stores have none.
pub trait OwnerStore: Store {
    /// Raw bytes of the pre-migration bridge state, when the deployment
    /// has one. Never deleted by callers; read once to rewrite under the
    /// canonical key.
    fn legacy_state_bytes(&self) -> io::Result<Option<Vec<u8>>> {
        Ok(None)
    }
}
