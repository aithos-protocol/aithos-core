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
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry, Verb};
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

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

/// A plain filesystem bundle store is a protocol store: it never carried
/// the pre-SPL-2 `gateway/state.json` key (that deployment shape is the
/// gateway's `GatewayStore`), so the trait default `legacy_state_bytes()
/// -> None` is exactly right. Declared here — the orphan rule forbids the
/// CLI from implementing this local trait on `aithos-bundle`'s type
/// (lot SPL-5, consigned in the chantier document).
impl OwnerStore for aithos_bundle::FsStore {}

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

/// Where the bridge keeps its non-secret runtime state in the store —
/// under the vault namespace of the node its own governance mandate
/// covers (`act.x.gateway.*` → `/x/gateway`, spec §08). Migrated from
/// [`LEGACY_STATE_PATH`] at context open (SPL-2); custody is unchanged:
/// the state stays pod-local (sidecar / primary fs), never replicated.
pub const STATE_PATH: &str = "x/gateway/state.json";
/// Pre-SPL-2 address of the bridge state. Read once at context open to
/// rewrite the bytes under [`STATE_PATH`]; never deleted, never written
/// again.
pub const LEGACY_STATE_PATH: &str = "gateway/state.json";
/// The one budget profile id the gateway cites on inference entries —
/// the same id `owner-init-journal --token-budget` writes into the
/// inference mandate (v1: one profile, one tap).
pub const LLM_BUDGET_REF: &str = "llm";

/// Non-secret state persisted at equip time, reloaded by `open`.
#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeState {
    pub agent_mandate: String,
    pub gateway_mandate: String,
    /// Absent on ethos where no audit grant was made (e.g. journals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auditor_mandate: Option<String>,
    /// The budgeted inference pen (journals only, Phase C) — absent on
    /// contexts and on journals provisioned without a token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_mandate: Option<String>,
    /// The memory pen (journals only, lot C2): the append mandate on
    /// `circle:memory/` — absent on contexts and on journals provisioned
    /// before this lot (their journal tools refuse fail-closed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mandate: Option<String>,
    /// The briefing pen (contexts only, lot K): the READ mandate on the
    /// `briefing/` folders of the public and circle zones, granted by
    /// `owner-grant-briefing` — a separate owner gesture, orthogonal to
    /// server enrollment (re-enrollment preserves it). Absent = this
    /// context serves no directives (mute surface, fail-closed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub briefing_mandate: Option<String>,
}

/// What equipping an ethos hands back — identifiers only, never seeds
/// (the auditor seed is the one exception, printed once by the caller).
#[derive(Debug, Clone)]
pub struct EquipOutcome {
    pub ethos_did: String,
    pub agent_mandate: String,
    pub gateway_mandate: String,
    pub auditor_mandate: Option<String>,
    pub auditor_seed_hex: Option<String>,
    /// The budgeted inference pen (journals provisioned with a token
    /// budget only, Phase C).
    pub inference_mandate: Option<String>,
    /// The memory pen (journals only, lot C2): append on the journal's
    /// `circle:memory/` shelf.
    pub memory_mandate: Option<String>,
}

/// Read the bridge state, migrating the pre-SPL-2 key on first touch:
/// when [`STATE_PATH`] is absent and [`LEGACY_STATE_PATH`] present, the
/// bytes are copied verbatim under the new key — the legacy object is
/// never deleted — then read back from the new key.
pub fn read_state_migrating<S: OwnerStore>(bundle: &mut Bundle<S>) -> Result<BridgeState> {
    if bundle.store.get(STATE_PATH).map_err(owner_err)?.is_none() {
        // The legacy key left the canonical grammar with SPL-2 — the read
        // is a raw pod-territory access, never a Store::get.
        if let Some(legacy) = bundle.store.legacy_state_bytes().map_err(owner_err)? {
            bundle.store.put(STATE_PATH, &legacy).map_err(owner_err)?;
        }
    }
    read_json(bundle, STATE_PATH)
}

/// Shared equip path: mint the mandates towards the agent/gateway PUBLIC
/// keys, log every grant (issuance is never silent), persist certs+state.
#[allow(clippy::too_many_arguments)]
pub fn equip<S: OwnerStore>(
    mut bundle: Bundle<S>,
    owner: &OwnerKeys,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    agent_ops: &[String],
    with_auditor: bool,
    token_budget: Option<u64>,
    memory_folder: Option<&str>,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    let agent_pub = decode_pub(agent_pub_mb)?;
    let gateway_pub = decode_pub(gateway_pub_mb)?;

    let agent_mandate = mint(
        owner,
        &bundle,
        ent,
        "agent",
        &agent_pub,
        agent_ops,
        no_constraints(),
        window,
        now,
    )?;
    let gateway_mandate = mint(
        owner,
        &bundle,
        ent,
        "gateway",
        &gateway_pub,
        &["act.x.gateway.*".to_owned()],
        no_constraints(),
        window,
        now,
    )?;
    let (auditor_mandate, auditor_seed_hex) = if with_auditor {
        let seed = ent.e32();
        let sk = SigningKey::from_bytes(&seed);
        // The context auditor replays acts AND journalized reads (lot K
        // briefing entries are `ethos.read`) — two scoped entries, each
        // query still names ONE kind and anything wider stays refused by
        // the certificate half (§07.8). The mono `onboard` auditor keeps
        // its historic act-only scope (gateway-audit contract).
        let m = mint(
            owner,
            &bundle,
            ent,
            "auditor",
            &sk.verifying_key(),
            &[
                "read.gamma#kind=action".to_owned(),
                "read.gamma#kind=ethos.read".to_owned(),
            ],
            no_constraints(),
            window,
            now,
        )?;
        (Some(m), Some(hex::encode(seed)))
    } else {
        (None, None)
    };
    // The inference pen: SAME grantee key, its OWN mandate — budgets are
    // profile constraints checked on every entry citing them, so parking
    // the token budget here keeps the xref pen budget-free.
    let inference_mandate = match token_budget {
        Some(budget) => Some(mint(
            owner,
            &bundle,
            ent,
            "inference",
            &agent_pub,
            &["act.x.llm.*".to_owned()],
            serde_json::json!({
                "budgets": [{ "id": LLM_BUDGET_REF, "token_budget": budget }]
            }),
            window,
            now,
        )?),
        None => None,
    };

    // The memory pen (lot C2): the append mandate on the journal's
    // memory shelf, next to — never inside — the xref pen. The
    // certificate half is the Ethos perimeter entry (§04.2 lattice:
    // append creates and reads, never rewrites nor deletes); the
    // physical half is the shelf's header line, delivered to the SAME
    // agent key (§04.3 — the line is the pen).
    let memory_mandate = match memory_folder {
        Some(folder) => {
            bundle
                .deliver_zone_line(owner, &agent_pub, Zone::Circle, folder, None, ent)
                .map_err(owner_err)?;
            let dir = bundle
                .resolve_folder(Zone::Circle, folder)
                .map_err(owner_err)?;
            Some(mint_entries(
                owner,
                &bundle,
                ent,
                "memory",
                &agent_pub,
                vec![PerimeterEntry::Ethos {
                    verb: Verb::Append,
                    zone: Zone::Circle,
                    dir,
                    tag: None,
                }],
                no_constraints(),
                window,
                now,
            )?)
        }
        None => None,
    };

    let mut all = vec![&agent_mandate, &gateway_mandate];
    if let Some(m) = &auditor_mandate {
        all.push(m);
    }
    if let Some(m) = &inference_mandate {
        all.push(m);
    }
    if let Some(m) = &memory_mandate {
        all.push(m);
    }
    for m in all {
        bundle
            .store
            .put(
                &cert_path(&m.id),
                &serde_json::to_vec_pretty(m).map_err(owner_err)?,
            )
            .map_err(owner_err)?;
        bundle
            .log_owner_grant(owner, &m.id, now, ent)
            .map_err(owner_err)?;
    }
    let state = BridgeState {
        agent_mandate: agent_mandate.id.clone(),
        gateway_mandate: gateway_mandate.id.clone(),
        auditor_mandate: auditor_mandate.as_ref().map(|m| m.id.clone()),
        inference_mandate: inference_mandate.as_ref().map(|m| m.id.clone()),
        memory_mandate: memory_mandate.as_ref().map(|m| m.id.clone()),
        briefing_mandate: None,
    };
    bundle
        .store
        .put(
            STATE_PATH,
            &serde_json::to_vec_pretty(&state).map_err(owner_err)?,
        )
        .map_err(owner_err)?;

    Ok(EquipOutcome {
        ethos_did: bundle.did.clone(),
        agent_mandate: agent_mandate.id,
        gateway_mandate: gateway_mandate.id,
        auditor_mandate: auditor_mandate.map(|m| m.id),
        auditor_seed_hex,
        inference_mandate: inference_mandate.map(|m| m.id),
        memory_mandate: memory_mandate.map(|m| m.id),
    })
}

/// Equip a context ethos: xref pen + gateway pen (+ auditor). The read
/// ops arrive already mapped onto the mandate grammar — the tool-naming
/// convention (MCP flattening) is the caller's domain, not the ceremony's.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_context_ops<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    read_ops: &[String],
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    let owner = derived_owner(master, "context", label);
    let bundle = Bundle::open(store).map_err(owner_err)?;
    equip(
        bundle,
        &owner,
        agent_pub_mb,
        gateway_pub_mb,
        read_ops,
        true,
        None,
        None,
        window,
        now,
        ent,
    )
}

pub fn read_json<S: Store, T: serde::de::DeserializeOwned>(
    bundle: &Bundle<S>,
    path: &str,
) -> Result<T> {
    let bytes = bundle
        .store
        .get(path)
        .map_err(owner_err)?
        .ok_or_else(|| OwnerError::Failed(format!("missing {path}")))?;
    serde_json::from_slice(&bytes).map_err(owner_err)
}

/// Create the agent's journal: an isolated Ethos owned by the enterprise.
/// The agent's key gets the xref pen (`act.x.xref.*`), the gateway its
/// governance pen (`act.x.gateway.*`); both grants are logged — that IS
/// the journal's « mandate received » record. The agent's key ALSO gets
/// the MEMORY pen (lot C2): a separate `append` mandate on the
/// `circle:memory/` shelf this function prepares (folder + publish,
/// mirroring the pass-L given) — one pen per usage, independently
/// revocable. With `token_budget`, a budgeted inference pen joins them
/// (Phase C): a separate mandate carrying `budgets: [{id: "llm",
/// token_budget}]` — separate on purpose, so the xref pen never has to
/// cite a budget.
#[allow(clippy::too_many_arguments)]
pub fn owner_init_journal<S: OwnerStore>(
    master: &[u8; 32],
    agent_label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    token_budget: Option<u64>,
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    let owner = derived_owner(master, "journal", agent_label);
    let succession = derived_succession(master, "journal", agent_label);
    let mut bundle =
        Bundle::init(store, &owner, &succession.verifying_key(), ent, now).map_err(owner_err)?;
    // The memory shelf: an owner-prepared circle folder the memory pen
    // will write into. An append perimeter grows content, never the
    // tree shape — the folder must pre-exist.
    bundle
        .ensure_folder(Zone::Circle, MEMORY_FOLDER, &owner, ent)
        .map_err(owner_err)?;
    bundle.publish(&owner, now).map_err(owner_err)?;
    equip(
        bundle,
        &owner,
        agent_pub_mb,
        gateway_pub_mb,
        &["act.x.xref.*".to_owned()],
        false,
        token_budget,
        Some(MEMORY_FOLDER),
        window,
        now,
        ent,
    )
}

/// The connector namespace for MCP tools in the mandate grammar.
/// One place to change if the core grammar evolves.
pub const MCP_CONNECTOR: &str = "mcp";

/// The action name a tool maps to in the mandate grammar. The grammar
/// splits `act.x.<connector>.<action>` at the LAST dot, so dotted MCP
/// tool names ("user.read") cannot be actions verbatim: dots become
/// underscores. The raw tool name still travels in the clear payload of
/// every logged act. Collisions ("user.read" vs "user_read") are
/// rejected at config time — never aliased silently.
pub fn action_name(tool: &str) -> String {
    tool.replace('.', "_")
}

/// The op string an MCP tool call maps to (`act.x.mcp.<action>`).
pub fn op_for_tool(tool: &str) -> String {
    format!("act.x.{MCP_CONNECTOR}.{}", action_name(tool))
}

/// Equip a context ethos from tool names: maps each MCP tool onto the
/// mandate grammar ([`op_for_tool`]) then runs the ceremony.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_context<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    read_tools: &[String],
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    let read_ops: Vec<String> = read_tools.iter().map(|t| op_for_tool(t)).collect();
    owner_grant_context_ops(
        master,
        label,
        agent_pub_mb,
        gateway_pub_mb,
        &read_ops,
        store,
        window,
        now,
        ent,
    )
}

/// Enrol one person public key as a session issuer for an existing context.
/// The enterprise keeps the owner key; the gateway receives only this signed
/// root mandate. The delegate may attenuate the listed actions exactly one
/// level, with at most three simultaneously active MCP sessions.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_session_delegate<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    delegate_pub_mb: &str,
    gateway_audience: &str,
    tools: &[String],
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let delegate_pub = decode_pub(delegate_pub_mb)?;
    // Each granted line is either a raw perimeter entry (act.…, or an
    // ethos entry like `read.public` — the zone rights a delegated
    // session may carry, lot 1) or a bare tool name projected onto the
    // gateway's own connector. `self` is refused at the gesture: never
    // delegable until the delegated self-resolution core lot.
    let mut perimeter = Vec::new();
    for tool in tools {
        let entry = if tool.starts_with("act.") {
            PerimeterEntry::parse(tool).map_err(owner_err)?
        } else if let Ok(parsed) = PerimeterEntry::parse(tool) {
            parsed
        } else {
            PerimeterEntry::parse(&op_for_tool(tool)).map_err(owner_err)?
        };
        if matches!(
            &entry,
            PerimeterEntry::Ethos {
                zone: Zone::Self_,
                ..
            } | PerimeterEntry::EthosId {
                zone: Zone::Self_,
                ..
            }
        ) {
            return Err(OwnerError::Rejected(
                "zone `self` is refused in a session delegate: it is never delegable — the delegated self-resolution is its own core lot"
                    .into(),
            ));
        }
        perimeter.push(entry);
    }
    perimeter.push(PerimeterEntry::Issue { depth: 1 });
    let mandate = mint_entries(
        &owner,
        &bundle,
        ent,
        "session-delegate",
        &delegate_pub,
        perimeter,
        serde_json::json!({
            "max_sessions": 3,
            "purpose": gateway_audience,
        }),
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
    Ok(mandate.id)
}

/// Grant the briefing pen on a context (lot K, the minimal seam): the
/// owner prepares the `briefing/` folders in the public and circle
/// zones, delivers their zone lines to the agent's PUBLIC key (§04.3 —
/// the line is the pen's physics half) and mints ONE read mandate
/// covering both dirs (the certificate half). A separate owner gesture,
/// deliberately: one pen per usage, independently revocable, and the
/// existing tool equipment (grants, counts, re-enrollment) is never
/// touched. The `self` zone gets no line and no perimeter entry — it is
/// structurally out of the agent's reach. Requires prior equipment
/// (`owner-grant-context` / `owner-enroll-server`): the pen extends a
/// provisioned context, it never creates one.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_briefing<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    let owner = derived_owner(master, "context", label);
    let agent_pub = decode_pub(agent_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let mut state: BridgeState = read_state_migrating(&mut bundle)?;
    let expected_agent = &state.agent_mandate;
    let agent_cert: Mandate = read_json(&bundle, &cert_path(expected_agent))?;
    if agent_cert.grantee_pub().map_err(owner_err)? != agent_pub {
        return Err(OwnerError::Rejected(
            "the briefing pen must go to the equipped agent public key".into(),
        ));
    }
    // The shelves: owner-prepared folders in both served zones. A read
    // perimeter serves content, never grows the tree — they must exist.
    bundle
        .ensure_folder(Zone::Public, BRIEFING_FOLDER, &owner, ent)
        .map_err(owner_err)?;
    bundle
        .ensure_folder(Zone::Circle, BRIEFING_FOLDER, &owner, ent)
        .map_err(owner_err)?;
    bundle.publish(&owner, now).map_err(owner_err)?;
    // The physics half exists for CIRCLE only: public is clear by
    // design (§02.1 — no zone key, no header line to deliver), so the
    // circle dir gets the sealed line and the certificate names both
    // zones (the public entry documents the granted read even though no
    // key gates it).
    bundle
        .deliver_zone_line(&owner, &agent_pub, Zone::Circle, BRIEFING_FOLDER, None, ent)
        .map_err(owner_err)?;
    // The context AUDITOR gets the same circle line: the journalized
    // briefing reads seal their bodies under the section keys of this
    // dir (§07.9.2), and a gamma query only serves sealed entries the
    // querier can physically open — the auditor mandated on
    // `kind=ethos.read` needs the keys to replay its own slice. The
    // owner accepts what this implies: the context auditor can read the
    // circle directives it audits the reads of.
    if let Some(auditor_id) = &state.auditor_mandate {
        let auditor_cert: Mandate = read_json(&bundle, &cert_path(auditor_id))?;
        let auditor_pub = auditor_cert.grantee_pub().map_err(owner_err)?;
        bundle
            .deliver_zone_line(
                &owner,
                &auditor_pub,
                Zone::Circle,
                BRIEFING_FOLDER,
                None,
                ent,
            )
            .map_err(owner_err)?;
    }
    let mut perimeter = Vec::new();
    for zone in [Zone::Public, Zone::Circle] {
        let dir = bundle
            .resolve_folder(zone, BRIEFING_FOLDER)
            .map_err(owner_err)?;
        perimeter.push(PerimeterEntry::Ethos {
            verb: Verb::Read,
            zone,
            dir,
            tag: None,
        });
    }
    let mandate = mint_entries(
        &owner,
        &bundle,
        ent,
        "briefing",
        &agent_pub,
        perimeter,
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
    state.briefing_mandate = Some(mandate.id.clone());
    bundle
        .store
        .put(
            STATE_PATH,
            &serde_json::to_vec_pretty(&state).map_err(owner_err)?,
        )
        .map_err(owner_err)?;
    Ok(mandate.id)
}

/// Write or update one zone's directive (lot K owner tooling): creation
/// on first use, in-place rewrite afterwards — the very next
/// `briefing.read` serves the new text, no restart. `self` is accepted
/// as a target (owner-only notes live there) but is NEVER served: the
/// runtime holds no self line and lists no self index. v1 limits,
/// documented: one section per zone (`briefing/directives`), and the
/// owner-side rewrite is circle-only (the core's `section_rewrite`
/// pass) — a public or self directive is written once.
#[allow(clippy::too_many_arguments)]
pub fn owner_set_briefing<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    zone: &str,
    title: &str,
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
                "briefing zone must be public, circle or self, not `{other}`"
            )))
        }
    };
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    bundle
        .ensure_folder(zone, BRIEFING_FOLDER, &owner, ent)
        .map_err(owner_err)?;
    let path = format!("{BRIEFING_FOLDER}/{BRIEFING_SECTION}");
    let exists = bundle.read_section(zone, &path, &owner).is_ok();
    if !exists {
        bundle
            .section_add(
                &aithos_bundle::bundle::SectionSpec {
                    zone,
                    folder_path: BRIEFING_FOLDER,
                    name: BRIEFING_SECTION,
                    title,
                    tags: &[],
                    body: text,
                    now,
                },
                &owner,
                ent,
            )
            .map_err(owner_err)?;
        return bundle.publish(&owner, now).map_err(owner_err);
    }
    if zone != Zone::Circle {
        return Err(OwnerError::Rejected(format!(
            "rewriting a `{}` directive is circle-only in v1 — the {} zone directive is written once",
            zone.as_str(),
            zone.as_str()
        )));
    }
    bundle
        .section_rewrite(zone, &path, text, &owner, now, ent)
        .map_err(owner_err)?;
    bundle.publish(&owner, now).map_err(owner_err)
}

/// Mint the v1 ethos-read pen (lot G6, decided 2026-07-16): a plain
/// owner mandate covering `read.<zone>` for the asked zones, the circle
/// line delivered to the agent AND to the context auditor (the lot-K
/// implication, assumed: the auditor mandated on `kind=ethos.read` can
/// replay what it audits), the grant journalized. NEVER a toggle and
/// NEVER a state field: the runtime discovers the pen — like any other
/// chain to the agent key — by scanning the certificates, so any other
/// emission path (G8.c, a delegate) lights the same surface. `self` is
/// refused while the delegated self resolution is its own core lot.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_ethos_read<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    zones: &[String],
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    if zones.is_empty() {
        return Err(OwnerError::Rejected(
            "owner-grant-ethos-read needs at least one zone (public, circle)".into(),
        ));
    }
    let mut perimeter = Vec::new();
    let mut wants_circle = false;
    for zone in zones {
        match zone.as_str() {
            "public" => perimeter.push(PerimeterEntry::Ethos {
                verb: Verb::Read,
                zone: Zone::Public,
                dir: Vec::new(),
                tag: None,
            }),
            "circle" => {
                wants_circle = true;
                perimeter.push(PerimeterEntry::Ethos {
                    verb: Verb::Read,
                    zone: Zone::Circle,
                    dir: Vec::new(),
                    tag: None,
                });
            }
            "self" => {
                return Err(OwnerError::Rejected(
                    "zone `self` is refused: read.self is never granted by default, and \
                     serving it awaits the delegated self-resolution core lot (vectors-first)"
                        .into(),
                ))
            }
            other => {
                return Err(OwnerError::Rejected(format!(
                    "unknown zone `{other}` (public, circle)"
                )))
            }
        }
    }
    let owner = derived_owner(master, "context", label);
    let agent_pub = decode_pub(agent_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let state: BridgeState = read_state_migrating(&mut bundle)?;
    let agent_cert: Mandate = read_json(&bundle, &cert_path(&state.agent_mandate))?;
    if agent_cert.grantee_pub().map_err(owner_err)? != agent_pub {
        return Err(OwnerError::Rejected(
            "the ethos-read pen must go to the equipped agent public key".into(),
        ));
    }
    if wants_circle {
        bundle
            .deliver_zone_line(&owner, &agent_pub, Zone::Circle, "", None, ent)
            .map_err(owner_err)?;
        if let Some(auditor_id) = &state.auditor_mandate {
            let auditor_cert: Mandate = read_json(&bundle, &cert_path(auditor_id))?;
            let auditor_pub = auditor_cert.grantee_pub().map_err(owner_err)?;
            bundle
                .deliver_zone_line(&owner, &auditor_pub, Zone::Circle, "", None, ent)
                .map_err(owner_err)?;
        }
    }
    let mandate = mint_entries(
        &owner,
        &bundle,
        ent,
        "ethos-read",
        &agent_pub,
        perimeter,
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
    Ok(mandate.id)
}

/// Dev/harness emission path pending the G8.c product surface: the
/// owner mints a `read.circle` + `issue#depth=1` mandate to a FRESH
/// delegate key (born from the injected entropy), the delegate
/// immediately sub-mints `read.circle` to the agent key, both
/// certificates land in the store, the agent's circle line is
/// delivered (§04.3 — issuance appends the needed lines) and both
/// grants are journalized. Exercises the REAL sub-mandate path: the
/// runtime scan must light the surface from this chain exactly as from
/// an owner-minted pen.
#[allow(clippy::too_many_arguments)]
pub fn owner_issue_ethos_read_subchain<S: OwnerStore>(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    store: S,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<(String, String)> {
    let owner = derived_owner(master, "context", label);
    let agent_pub = decode_pub(agent_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(owner_err)?;
    let delegate_sk = SigningKey::from_bytes(&ent.e32());
    let circle_read = || PerimeterEntry::Ethos {
        verb: Verb::Read,
        zone: Zone::Circle,
        dir: Vec::new(),
        tag: None,
    };
    let issue = PerimeterEntry::parse("issue#depth=1").map_err(owner_err)?;
    let parent = mint_entries(
        &owner,
        &bundle,
        ent,
        "ethos-delegate",
        &delegate_sk.verifying_key(),
        vec![circle_read(), issue],
        no_constraints(),
        window,
        now,
    )?;
    let sub = Mandate::build_sub(
        &parent,
        &delegate_sk,
        &MandateSpec {
            id: format!(
                "mandate_{}",
                aithos_core::ids::Sid(ulid::Ulid::from(u128::from_be_bytes(ent.e16())))
            ),
            subject: bundle.did.clone(),
            grantee_id: "urn:aithos:agent:ethos-read-sub".to_owned(),
            grantee_label: "ethos-read-sub".to_owned(),
            grantee_pub: &agent_pub,
            perimeter: vec![circle_read()],
            constraints: no_constraints(),
            not_before: window.not_before.clone(),
            not_after: window.not_after.clone(),
            issued_at: now.to_owned(),
            nonce: hex::encode(ent.e16()),
        },
    )
    .map_err(owner_err)?;
    for mandate in [&parent, &sub] {
        bundle
            .store
            .put(
                &cert_path(&mandate.id),
                &serde_json::to_vec_pretty(mandate).map_err(owner_err)?,
            )
            .map_err(owner_err)?;
        bundle
            .log_owner_grant(&owner, &mandate.id, now, ent)
            .map_err(owner_err)?;
    }
    bundle
        .deliver_zone_line(&owner, &agent_pub, Zone::Circle, "", None, ent)
        .map_err(owner_err)?;
    Ok((parent.id.clone(), sub.id.clone()))
}

/// A clear, serialisable view of one gamma entry — what steps and
/// exports consume, without leaking core types across the bridge.
#[derive(Debug, Clone, Serialize)]
pub struct EntryView {
    pub id: String,
    pub at: String,
    pub kind: String,
    pub target: Option<String>,
    pub authorized_via: Option<Vec<String>>,
    pub payload: Option<serde_json::Value>,
}

pub fn view(e: &aithos_core::gamma::Entry) -> EntryView {
    EntryView {
        id: e.id.clone(),
        at: e.at.clone(),
        kind: e.kind.clone(),
        target: e.target.clone(),
        authorized_via: e.authorized_via.clone(),
        payload: e.payload.clone(),
    }
}

/// Owner/test-side view of any ethos gamma (opens the store read-only).
pub fn gamma_view<S: OwnerStore>(store: S) -> Result<Vec<EntryView>> {
    let bundle = Bundle::open(store).map_err(owner_err)?;
    Ok(bundle
        .gamma_entries()
        .map_err(owner_err)?
        .iter()
        .map(view)
        .collect())
}

/// A clear, serialisable view of one memory note — what the journal
/// tools hand back. `text` rides on opened hits only: the index
/// skeleton (name, title, tags) is clear, the body stays sealed until
/// a covered read opens it.
#[derive(Debug, Clone, Serialize)]
pub struct NoteView {
    pub name: String,
    pub title: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// One clear index row of the memory shelf (skeleton data — no body).
pub struct MemoryRow {
    pub name: String,
    pub title: String,
    pub tags: Vec<String>,
}

/// One zone folder's clear index rows, oldest first, optionally filtered
/// by a case-insensitive `query` over name/title/tags and an exact
/// `tag`. This reads the SKELETON the readability frontier already
/// grants whoever holds the files — no body is touched here. A folder
/// that does not exist yields no rows (nothing was ever shelved there).
pub fn zone_rows<S: Store>(
    bundle: &Bundle<S>,
    zone: Zone,
    folder: &str,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<MemoryRow>> {
    let Ok(folders) = bundle.resolve_folder(zone, folder) else {
        return Ok(Vec::new());
    };
    let folder_sid = folders.last().map(ToString::to_string);
    let index: serde_json::Value = read_json(bundle, &format!("e/{}/index.json", zone.as_str()))?;
    let needle = query.map(str::to_lowercase);
    let mut rows = Vec::new();
    for row in index["sections"].as_array().into_iter().flatten() {
        if row["folder_sid"].as_str().map(str::to_owned) != folder_sid {
            continue;
        }
        let name = row["name"].as_str().unwrap_or_default().to_owned();
        let title = row["title"].as_str().unwrap_or_default().to_owned();
        let tags: Vec<String> = row["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(q) = &needle {
            let hay = format!(
                "{}\u{0}{}\u{0}{}",
                name.to_lowercase(),
                title.to_lowercase(),
                tags.join("\u{0}").to_lowercase()
            );
            if !hay.contains(q.as_str()) {
                continue;
            }
        }
        if let Some(t) = tag {
            if !tags.iter().any(|x| x == t) {
                continue;
            }
        }
        rows.push(MemoryRow { name, title, tags });
    }
    Ok(rows)
}

/// The memory shelf's clear index rows, oldest first — see [`zone_rows`].
pub fn memory_rows<S: Store>(
    bundle: &Bundle<S>,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<MemoryRow>> {
    zone_rows(bundle, Zone::Circle, MEMORY_FOLDER, query, tag)
}

/// Owner/ops-side view of a journal's memory shelf: the CLEAR index
/// skeleton (names, titles, tags — never a body), oldest first. What an
/// operator or test lists before opening a note with the owner keys.
pub fn journal_notes_view<S: OwnerStore>(store: S) -> Result<Vec<NoteView>> {
    let bundle = Bundle::open(store).map_err(owner_err)?;
    Ok(memory_rows(&bundle, None, None)?
        .into_iter()
        .map(|r| NoteView {
            name: r.name,
            title: r.title,
            tags: r.tags,
            text: None,
        })
        .collect())
}

/// The grantee public key (multibase) named by a stored certificate.
pub fn cert_grantee_pub<S: OwnerStore>(store: S, mandate_id: &str) -> Result<String> {
    let bundle = Bundle::open(store).map_err(owner_err)?;
    let m: Mandate = read_json(&bundle, &cert_path(mandate_id))?;
    Ok(m.grantee.pubkey)
}

/// The constraints block carried by a stored certificate (owner/test-side
/// assertions — e.g. the token budget on the inference pen).
pub fn cert_constraints<S: OwnerStore>(store: S, mandate_id: &str) -> Result<serde_json::Value> {
    let bundle = Bundle::open(store).map_err(owner_err)?;
    let m: Mandate = read_json(&bundle, &cert_path(mandate_id))?;
    Ok(m.constraints)
}

/// Canonical perimeter strings carried by a stored certificate.
pub fn cert_perimeter<S: OwnerStore>(store: S, mandate_id: &str) -> Result<Vec<String>> {
    let bundle = Bundle::open(store).map_err(owner_err)?;
    let mandate: Mandate = read_json(&bundle, &cert_path(mandate_id))?;
    Ok(mandate.perimeter)
}
