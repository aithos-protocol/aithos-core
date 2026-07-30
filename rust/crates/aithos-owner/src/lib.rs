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

use aithos_bundle::bundle::Bundle;
use aithos_bundle::entropy::EntropySource;
use aithos_bundle::Store;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;

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

pub(crate) fn owner_err(error: impl std::fmt::Display) -> OwnerError {
    OwnerError::Failed(error.to_string())
}

/// Derived owner keys for an enterprise-owned ethos (journal or context).
pub fn derived_owner(master: &[u8; 32], kind: &str, label: &str) -> OwnerKeys {
    OwnerKeys::genesis(&MasterSeed::from_bytes(aithos_core::derive::derive_key(
        &format!("aithos-gw/v1/{kind}/{label}"),
        master,
    )))
}

/// Derived succession key for the same ethos — the second genesis input.
pub fn derived_succession(master: &[u8; 32], kind: &str, label: &str) -> SigningKey {
    succession_from_entropy(aithos_core::derive::derive_key(
        &format!("aithos-gw/v1/{kind}/{label}/succession"),
        master,
    ))
}

/// Create a context Ethos owned by the enterprise (demo/dev path — real
/// contexts usually pre-exist with their own history).
pub fn owner_init_context<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    store: S,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    let owner = derived_owner(master, "context", label);
    let succession = derived_succession(master, "context", label);
    let bundle =
        Bundle::init(store, &owner, &succession.verifying_key(), ent, now).map_err(owner_err)?;
    Ok(bundle.did)
}

/// Owner revocation of one mandate on a context store (lot G6 scenario
/// surface; the M3 product surface `owner-revoke-mandate` subsumes it
/// later). One `revoke` entry — the runtime scan sees it on the very
/// next call, no restart.
pub fn owner_revoke_mandate_id<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    mandate_id: &str,
    reason: &str,
    store: S,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    bundle
        .log_revoke_owner(&owner, mandate_id, reason, now, ent)
        .map_err(owner_err)?;
    Ok(())
}

/// The journal's memory shelf: the circle folder `owner-init-journal`
/// prepares and the memory pen writes into (lot C2).
pub const MEMORY_FOLDER: &str = "memory";
/// The context's briefing shelf (lot K): the owner's directives live as
/// sections of a `briefing/` folder in the public and circle zones of
/// the CONTEXT ethos — `self` holds owner-only notes and never reaches
/// the agent (the briefing pen simply carries no self entry).
pub const BRIEFING_FOLDER: &str = "briefing";
/// The one briefing section per zone (v1): `owner-set-briefing` creates
/// it on first use and rewrites it afterwards — the directive has a
/// stable address, so a hot edit is served on the very next read.
pub const BRIEFING_SECTION: &str = "directives";

/// Owner-side read of one memory note body (sovereignty §3bis.3: the
/// journal is enterprise-owned — the owner audits its agent's memory
/// with its own derived keys, no pen involved).
pub fn owner_read_journal_note<S: OwnerStore>(
    master: &[u8; 32],
    agent_label: &str,
    store: S,
    name: &str,
) -> Result<String> {
    let owner = derived_owner(master, "journal", agent_label);
    let bundle = Bundle::open(store).map_err(owner_err)?;
    bundle
        .read_section(Zone::Circle, &format!("{MEMORY_FOLDER}/{name}"), &owner)
        .map_err(owner_err)
}
