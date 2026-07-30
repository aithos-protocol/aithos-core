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
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
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

/// Owner-side section write (lot G6 provisioning surface, the generic
/// sibling of `owner_set_briefing`): ensure the folder chain, add ONE
/// fresh section — title = the last path segment, the human label the
/// clear index shows. Demos and harnesses fill zones with it.
#[allow(clippy::too_many_arguments)]
pub fn owner_add_section<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    zone: &str,
    path: &str,
    text: &str,
    store: S,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let zone = match zone {
        "public" => Zone::Public,
        "circle" => Zone::Circle,
        "self" => Zone::Self_,
        other => {
            return Err(OwnerError::Rejected(format!(
                "zone must be public, circle or self, not `{other}`"
            )))
        }
    };
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let (folder_path, name) = match path.rsplit_once('/') {
        Some((dir, name)) => (dir, name),
        None => ("", path),
    };
    if !folder_path.is_empty() {
        bundle
            .ensure_folder(zone, folder_path, &owner, ent)
            .map_err(owner_err)?;
        bundle.publish(&owner, now).map_err(owner_err)?;
    }
    bundle
        .section_add(
            &aithos_bundle::bundle::SectionSpec {
                zone,
                folder_path,
                name,
                title: name,
                tags: &[],
                body: text,
                now,
            },
            &owner,
            ent,
        )
        .map_err(owner_err)
}

/// Deliver the circle zone line to ONE recipient key (§04.3 — the line
/// is the physics half of a pen). Generic on purpose: the briefing pen
/// delivers to the agent key, a delegated session pen delivers to the
/// GATEWAY key (the session leaf grantee), and the auditor gets its
/// copy when present — issuance appends the needed lines, the
/// certificate half travels separately.
pub fn owner_deliver_circle_line<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    recipient_pub_mb: &str,
    store: S,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let owner = derived_owner(master, "context", label);
    let recipient_pub = decode_pub(recipient_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let _ = now;
    bundle
        .deliver_zone_line(&owner, &recipient_pub, Zone::Circle, "", None, ent)
        .map_err(owner_err)?;
    Ok(())
}

pub fn decode_pub(multibase: &str) -> Result<ed25519_dalek::VerifyingKey> {
    let bytes = aithos_core::wire::multibase_to_ed25519_pub(multibase).map_err(owner_err)?;
    ed25519_dalek::VerifyingKey::from_bytes(&bytes)
        .map_err(|e| OwnerError::Failed(format!("bad agent public key: {e}")))
}

/// The validity window of the mandates minted at onboarding. Computed by
/// the surface (binary or test) — T stays injected, the bridge does no
/// clock arithmetic.
pub struct MandateWindow {
    pub not_before: String,
    pub not_after: String,
}

/// One narrowly scoped control-plane delegate. The seed is handed to the
/// enterprise client once; the gateway persists only the signed mandate.
pub struct ConnectorConfigGrant {
    pub mandate: String,
    pub seed_hex: String,
}

/// Where mandate certificates live in the store.
pub fn cert_path(id: &str) -> String {
    format!("certs/{id}.json")
}

/// No constraints — the shape most mints use.
pub fn no_constraints() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// Mint one root mandate: `ops` are perimeter entry strings. Every
/// caller passes its constraints explicitly (empty object = none).
#[allow(clippy::too_many_arguments)]
pub fn mint<S: Store>(
    owner: &OwnerKeys,
    bundle: &Bundle<S>,
    ent: &mut dyn EntropySource,
    label: &str,
    grantee_pub: &ed25519_dalek::VerifyingKey,
    ops: &[String],
    constraints: serde_json::Value,
    window: &MandateWindow,
    now: &str,
) -> Result<Mandate> {
    let perimeter = ops
        .iter()
        .map(|op| PerimeterEntry::parse(op).map_err(owner_err))
        .collect::<Result<Vec<_>>>()?;
    mint_entries(
        owner,
        bundle,
        ent,
        label,
        grantee_pub,
        perimeter,
        constraints,
        window,
        now,
    )
}

/// Mint one root mandate from pre-built perimeter entries — what the
/// memory pen uses (its Ethos entry carries resolved folder sids, not a
/// parseable string).
#[allow(clippy::too_many_arguments)]
pub fn mint_entries<S: Store>(
    owner: &OwnerKeys,
    bundle: &Bundle<S>,
    ent: &mut dyn EntropySource,
    label: &str,
    grantee_pub: &ed25519_dalek::VerifyingKey,
    perimeter: Vec<PerimeterEntry>,
    constraints: serde_json::Value,
    window: &MandateWindow,
    now: &str,
) -> Result<Mandate> {
    let id = format!(
        "mandate_{}",
        aithos_core::ids::Sid(ulid::Ulid::from(u128::from_be_bytes(ent.e16())))
    );
    Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id,
            subject: bundle.did.clone(),
            grantee_id: format!("urn:aithos:agent:{label}"),
            grantee_label: label.to_owned(),
            grantee_pub,
            perimeter,
            constraints,
            not_before: window.not_before.clone(),
            not_after: window.not_after.clone(),
            issued_at: now.to_owned(),
            nonce: hex::encode(ent.e16()),
        },
    )
    .map_err(owner_err)
}

/// Mint an exact `act.x.<connector>.config` delegate for the signed control
/// plane. This consumes Core's existing perimeter grammar and grant log; it
/// does not create a gateway-local authority dialect.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_connector_config<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    connector: &str,
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<ConnectorConfigGrant> {
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let seed = ent.e32();
    let signer = SigningKey::from_bytes(&seed);
    let mandate = mint(
        &owner,
        &bundle,
        ent,
        "connector-config",
        &signer.verifying_key(),
        &[format!("act.x.{connector}.config")],
        no_constraints(),
        window,
        now,
    )?;
    bundle
        .store
        .put(
            &cert_path(&mandate.id),
            &serde_json::to_vec_pretty(&mandate).map_err(owner_err)?,
        )
        .map_err(owner_err)?;
    bundle
        .log_owner_grant(&owner, &mandate.id, now, ent)
        .map_err(owner_err)?;
    Ok(ConnectorConfigGrant {
        mandate: mandate.id,
        seed_hex: hex::encode(seed),
    })
}
