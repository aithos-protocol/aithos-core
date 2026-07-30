//! The one thin layer between the gateway and the trust engine.
//!
//! **Only this module (and `store_adapter`) imports `aithos-core` /
//! `aithos-bundle`.** The rest of the gateway speaks in tool names and
//! outcomes; this bridge translates those into mandate verification,
//! gamma appends (kind imposed by the operation, never by the caller)
//! and scoped audit reads (`read.gamma`). When the core's API moves,
//! this file absorbs the change.
//!
//! Identity model (audit MVP): the owner (the enterprise) grants THREE
//! mandates at onboarding —
//! - the **agent** mandate: one `act.x.mcp.<tool>` entry per read tool;
//! - the **gateway** mandate: `act.x.gateway.*` — refusals are not acts
//!   of the agent (the agent did not act, that is the point) but
//!   governance acts of the gateway's own identity;
//! - the **auditor** mandate: `read.gamma#kind=action`, the scoped
//!   certificate a third party audits with.
//!
//! Enforcement is double-walled: `authorize` is the polite pre-check
//! (clean refusal before anything is relayed), and `log_action` re-runs
//! the full verification (chain, revocations, budgets) at append time —
//! the bundle itself refuses to log an uncovered act.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};
use serde::{Deserialize, Serialize};

use aithos_bundle::bundle::Bundle;
use aithos_bundle::log::{ActionSpec, InferenceSpec, LogFilter};
use aithos_bundle::remote::{KeySigner, RemoteStore};
use aithos_bundle::Store;
use aithos_core::constraints::verify_max_sessions;
use aithos_core::did::DidDocument;
use aithos_core::gamma::grant_logged;
use aithos_core::header::{Header, Recipient};
use aithos_core::ids::Sid;
use aithos_core::keys::{ed2x, grantee_kex_secret, succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{
    covers_act, covers_op, covers_section_op, verify_chain, verify_chain_revocable, verify_op,
    ActOp, GammaQuery, Mandate, MandateSpec, Op, PerimeterEntry, SectionOp, Verb,
};
use aithos_core::operation::{
    verify_delegated_session as verify_delegated_session_core, verify_session,
    DelegatedSessionEvidence as CoreDelegatedSessionEvidence, SessionEvidence,
};
use aithos_core::path::Zone;
use aithos_core::revocation::{chain_revoked_at, revocations, Revocation};

use crate::config::{ContextTools, GatewayConfig, ToolAccess};
use crate::hub::{validate_approved, ApprovedManifest, ApprovedTool, ProposedManifest};
use crate::keyholder::Keyholder;
use crate::policy::{hub_op_for_tool, op_for_tool, Policy};
use crate::store_adapter::{replicate_owner_history, GatewayStore, OwnerReplicationReport};
use crate::{GatewayError, Result};

/// Closed, non-secret join material for guarded connector decisions/effects.
pub struct ConnectorEffectProof<'a> {
    pub event: &'a str,
    pub approval_id: &'a str,
    pub payload_digest: &'a str,
    pub message_id: Option<&'a str>,
}

/// Entropy seam, re-exported so surfaces (binary, tests) never import
/// the bundle directly: the bridge is the only door to the core.
pub use aithos_bundle::entropy::{EntropySource, OsEntropy, SeqEntropy};
/// Raw store trait, re-exported through the same single door — what
/// owner/test-side surgery uses to read or doctor a store it holds.
pub use aithos_bundle::Store as RawStore;

mod control;
pub(crate) use control::render_rfc3339z;
pub use control::{
    prepare_control_envelope, valid_control_gamma_kind, ControlAccess, ControlAuthError,
    ControlContextProof, ControlHeadsProof, ControlPage, ControlPrincipal, ControlProofReader,
    ControlRawArtifact, PreparedControlEnvelope,
};

/// Helpers du runtime et helpers partagés runtime/cérémonies,
/// isolés du bloc owner (lot SPL-3 du chantier split). Les chemins
/// publics historiques sont préservés par les re-exports ci-dessous.
mod shared;

pub use shared::{
    agent_kex_pub_multibase, agent_pub_multibase, approved_manifest_catalog_digest,
    enforce_max_sessions, gateway_acme_authorization_header, gateway_kex_pub_multibase,
    gateway_pub_multibase, gateway_tunnel_registration_line, manifest_tool_pin,
    proposed_manifest_catalog_digest, verify_delegated_chain_session, verify_delegated_session,
};
pub(crate) use shared::{
    bridge_err, cert_path, commitment_of, constraints_bind_resource,
    enrollment_chain_is_direct_owner, ethos_row_is_covered, hash_of, hub_manifest_paths,
    memory_rows, merge_server_pins, mint, no_constraints, public_read_current, read_denied_op,
    read_json, read_state_migrating, validate_runtime_tool, view, write_denied, write_denied_op,
    zone_all_rows, zone_rows,
};

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
/// The one budget profile id the gateway cites on inference entries —
/// the same id `owner-init-journal --token-budget` writes into the
/// inference mandate (v1: one profile, one tap).
pub const LLM_BUDGET_REF: &str = "llm";

/// Native proof domain fixed by the historical CB2 session vector.
pub const MCP_SESSION_NATIVE_PROOF_DOMAIN: &[u8] = b"aithos-core/cb2/native-leaf-proof\x00";

/// Closed input passed through the gateway's only Core seam. Keeping Values
/// here is intentional: Core owns and verifies every exact wire shape.
pub struct DelegatedSessionEvidence<'a> {
    pub mandate: &'a serde_json::Value,
    pub certificate: &'a serde_json::Value,
    pub projection: &'a serde_json::Value,
    pub operation_ref: &'a serde_json::Value,
    pub native_leaf_proof: Option<&'a serde_json::Value>,
    pub session_proof: Option<&'a serde_json::Value>,
}

pub struct DelegatedChainSessionEvidence<'a> {
    pub chain: &'a [Mandate],
    pub did: &'a DidDocument,
    pub at: &'a str,
    pub revocations: &'a [Revocation],
    pub mandate: &'a serde_json::Value,
    pub certificate: &'a serde_json::Value,
    pub projection: &'a serde_json::Value,
    pub operation_ref: &'a serde_json::Value,
    pub native_leaf_proof: &'a serde_json::Value,
    pub session_proof: &'a serde_json::Value,
}

/// Vault record name for one approved hub manifest. The parent
/// `/x/<server>` header is pinned by the bundle's vault root.
const HUB_MANIFEST_FILE: &str = "manifest.enc";
/// Non-secret state persisted at equip time, reloaded by `open`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BridgeState {
    agent_mandate: String,
    gateway_mandate: String,
    /// Absent on ethos where no audit grant was made (e.g. journals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auditor_mandate: Option<String>,
    /// The budgeted inference pen (journals only, Phase C) — absent on
    /// contexts and on journals provisioned without a token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inference_mandate: Option<String>,
    /// The memory pen (journals only, lot C2): the append mandate on
    /// `circle:memory/` — absent on contexts and on journals provisioned
    /// before this lot (their journal tools refuse fail-closed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memory_mandate: Option<String>,
    /// The briefing pen (contexts only, lot K): the READ mandate on the
    /// `briefing/` folders of the public and circle zones, granted by
    /// `owner-grant-briefing` — a separate owner gesture, orthogonal to
    /// server enrollment (re-enrollment preserves it). Absent = this
    /// context serves no directives (mute surface, fail-closed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    briefing_mandate: Option<String>,
}

/// What onboarding hands back to the operator. Secrets appear ONCE here
/// (to be stored cold / handed to the auditor) and are never persisted
/// by the gateway runtime.
pub struct OnboardOutcome {
    /// The owner's DID (the enterprise identity anchoring the ethos).
    pub owner_did: String,
    /// Owner master seed, hex — STORE COLD, shown once.
    pub owner_seed_hex: String,
    /// Succession secret, hex — STORE COLD (paper/HSM), shown once.
    pub succession_secret_hex: String,
    /// The auditor's signing seed, hex — hand to the auditor out of band.
    pub auditor_seed_hex: String,
    /// Mandate ids (also persisted in the bridge state).
    pub agent_mandate: String,
    pub gateway_mandate: String,
    pub auditor_mandate: String,
    /// The agent-facing endpoint to configure in the agent runtime.
    pub agent_endpoint: String,
}

impl std::fmt::Debug for OnboardOutcome {
    /// Never print seed material through Debug.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnboardOutcome")
            .field("owner_did", &self.owner_did)
            .field("agent_mandate", &self.agent_mandate)
            .field("gateway_mandate", &self.gateway_mandate)
            .field("auditor_mandate", &self.auditor_mandate)
            .finish_non_exhaustive()
    }
}

/// The validity window of the mandates minted at onboarding. Computed by
/// the surface (binary or test) — T stays injected, the bridge does no
/// clock arithmetic.
pub struct MandateWindow {
    pub not_before: String,
    pub not_after: String,
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

/// One served directive (lot K): the owner's exact text, the zone it
/// came from named next to it — what `briefing.read` hands the agent.
#[derive(Debug, Clone, Serialize)]
pub struct BriefingItem {
    pub zone: String,
    pub title: String,
    pub text: String,
}

/// Live bridge: the ethos, the mandate chains and the keyholder,
/// assembled and ready to authorise, log and export. The keyholder is
/// shared (`Arc`): one runner identity signs into N ethos at once
/// (multi-context runtime) while custody semantics stay intact — the
/// seeds zeroise when the last bridge drops and are never serialised.
pub struct Bridge {
    bundle: Bundle<GatewayStore>,
    keyholder: Arc<Keyholder>,
    agent_chain: Vec<Mandate>,
    gateway_chain: Vec<Mandate>,
    auditor_mandate: Option<Mandate>,
    inference_chain: Option<Vec<Mandate>>,
    memory_chain: Option<Vec<Mandate>>,
    briefing_chain: Option<Vec<Mandate>>,
    entropy: Box<dyn EntropySource + Send>,
}

impl Bridge {
    // ------------------------------------------------------------ onboard

    /// One command: initialise the ethos, mint the identities, grant the
    /// read-only agent mandate derived from the tool map, the gateway's
    /// own governance mandate, and the scoped auditor mandate. Every
    /// grant is logged (issuance is never silent, spec 07.4).
    pub fn onboard(
        cfg: &GatewayConfig,
        store: GatewayStore,
        keyholder: Keyholder,
        mut entropy: Box<dyn EntropySource + Send>,
        window: &MandateWindow,
        now: &str,
    ) -> Result<(Self, OnboardOutcome)> {
        let keyholder = Arc::new(keyholder);
        let ent = entropy.as_mut();

        // The enterprise identity. Seeds surface once in the outcome and
        // are never kept by the runtime.
        let owner_seed = ent.e32();
        let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(owner_seed));
        let succession_entropy = ent.e32();
        let succession = succession_from_entropy(succession_entropy);

        let mut bundle = Bundle::init(store, &owner, &succession.verifying_key(), ent, now)
            .map_err(bridge_err)?;

        // Agent mandate: exactly the read tools, nothing else.
        let read_ops: Vec<String> = cfg
            .tools
            .iter()
            .filter(|(_, a)| **a == crate::config::ToolAccess::Read)
            .map(|(t, _)| op_for_tool(t))
            .collect();
        let agent_sk = SigningKey::from_bytes(keyholder.agent_seed());
        let agent_mandate = mint(
            &owner,
            &bundle,
            ent,
            "agent",
            &agent_sk.verifying_key(),
            &read_ops,
            no_constraints(),
            window,
            now,
        )?;

        // Gateway mandate: governance acts (refusals) under its own key.
        let gateway_sk = SigningKey::from_bytes(keyholder.gateway_seed());
        let gateway_mandate = mint(
            &owner,
            &bundle,
            ent,
            "gateway",
            &gateway_sk.verifying_key(),
            &["act.x.gateway.*".to_owned()],
            no_constraints(),
            window,
            now,
        )?;

        // Auditor mandate: read.gamma scoped to act entries.
        let auditor_seed = ent.e32();
        let auditor_sk = SigningKey::from_bytes(&auditor_seed);
        let auditor_mandate = mint(
            &owner,
            &bundle,
            ent,
            "auditor",
            &auditor_sk.verifying_key(),
            &["read.gamma#kind=action".to_owned()],
            no_constraints(),
            window,
            now,
        )?;

        for m in [&agent_mandate, &gateway_mandate, &auditor_mandate] {
            bundle
                .store
                .put(
                    &cert_path(&m.id),
                    &serde_json::to_vec_pretty(m).map_err(bridge_err)?,
                )
                .map_err(bridge_err)?;
            bundle
                .log_owner_grant(&owner, &m.id, now, ent)
                .map_err(bridge_err)?;
        }

        let state = BridgeState {
            agent_mandate: agent_mandate.id.clone(),
            gateway_mandate: gateway_mandate.id.clone(),
            auditor_mandate: Some(auditor_mandate.id.clone()),
            inference_mandate: None,
            memory_mandate: None,
            briefing_mandate: None,
        };
        bundle
            .store
            .put(
                STATE_PATH,
                &serde_json::to_vec_pretty(&state).map_err(bridge_err)?,
            )
            .map_err(bridge_err)?;
        let outcome = OnboardOutcome {
            owner_did: bundle.did.clone(),
            owner_seed_hex: hex::encode(owner_seed),
            succession_secret_hex: hex::encode(succession_entropy),
            auditor_seed_hex: hex::encode(auditor_seed),
            agent_mandate: agent_mandate.id.clone(),
            gateway_mandate: gateway_mandate.id.clone(),
            auditor_mandate: auditor_mandate.id.clone(),
            agent_endpoint: format!("http://{}/mcp", cfg.listen),
        };
        let bridge = Self {
            bundle,
            keyholder,
            agent_chain: vec![agent_mandate],
            gateway_chain: vec![gateway_mandate],
            auditor_mandate: Some(auditor_mandate),
            inference_chain: None,
            memory_chain: None,
            briefing_chain: None,
            entropy,
        };
        Ok((bridge, outcome))
    }

    /// Reload a bridge equipped earlier (the `run` and `audit-export`
    /// paths): state and certs come from the store; the seeds come from
    /// the RUNNER's identity file (`Keyholder::load`), never the store.
    /// Takes the keyholder shared so a multi-context runner opens N
    /// bridges over its ONE identity.
    pub fn open(
        store: GatewayStore,
        keyholder: Arc<Keyholder>,
        entropy: Box<dyn EntropySource + Send>,
    ) -> Result<Self> {
        let mut bundle = Bundle::open(store).map_err(bridge_err)?;
        let state: BridgeState = read_state_migrating(&mut bundle)?;
        let agent = read_json(&bundle, &cert_path(&state.agent_mandate))?;
        let gateway = read_json(&bundle, &cert_path(&state.gateway_mandate))?;
        let auditor_mandate = match &state.auditor_mandate {
            Some(id) => Some(read_json(&bundle, &cert_path(id))?),
            None => None,
        };
        let inference_chain = match &state.inference_mandate {
            Some(id) => Some(vec![read_json(&bundle, &cert_path(id))?]),
            None => None,
        };
        let memory_chain = match &state.memory_mandate {
            Some(id) => Some(vec![read_json(&bundle, &cert_path(id))?]),
            None => None,
        };
        let briefing_chain = match &state.briefing_mandate {
            Some(id) => Some(vec![read_json(&bundle, &cert_path(id))?]),
            None => None,
        };
        Ok(Self {
            bundle,
            keyholder,
            agent_chain: vec![agent],
            gateway_chain: vec![gateway],
            auditor_mandate,
            inference_chain,
            memory_chain,
            briefing_chain,
            entropy,
        })
    }

    /// Open and validate one owner-approved hub manifest through the
    /// gateway recipient line. The runner never needs the owner seed.
    pub fn read_hub_manifest(&self, server: &str) -> Result<ApprovedManifest> {
        let (header_path, manifest_path) = hub_manifest_paths(server);
        let header: Header = read_json(&self.bundle, &header_path)?;
        header.validate().map_err(bridge_err)?;
        let expected_node = format!("/x/{server}");
        if header.node != expected_node {
            return Err(GatewayError::BridgeFailed(format!(
                "hub manifest header targets `{}`, expected `{expected_node}`",
                header.node
            )));
        }
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let gateway_kex = grantee_kex_secret(&gateway_sk);
        let gateway_kid = gateway_pub_multibase(&self.keyholder);
        let (version, key) = header
            .open_latest(&self.bundle.did, &gateway_kid, &gateway_kex)
            .map_err(bridge_err)?;
        let sealed = self
            .bundle
            .store
            .get(&manifest_path)
            .map_err(bridge_err)?
            .ok_or_else(|| GatewayError::BridgeFailed(format!("missing {manifest_path}")))?;
        if sealed.len() < 24 {
            return Err(GatewayError::BridgeFailed(format!(
                "truncated {manifest_path}"
            )));
        }
        let nonce: [u8; 24] = sealed[..24].try_into().expect("length checked");
        let aad = aithos_core::seal::blob_aad(&self.bundle.did, &expected_node, version);
        let plain =
            aithos_core::seal::blob_open(&key, &sealed[24..], &nonce, &aad).map_err(bridge_err)?;
        let manifest: ApprovedManifest = serde_json::from_slice(&plain).map_err(bridge_err)?;
        validate_approved(&manifest)?;
        if manifest.server != server {
            return Err(GatewayError::BridgeFailed(format!(
                "manifest server is `{}`, expected `{server}`",
                manifest.server
            )));
        }
        Ok(manifest)
    }

    /// Re-open the approved connector through the existing sealed H3 reader,
    /// after semantic Gamma replay verifies. A full edition verification is
    /// deliberately not run here: certificates and Gamma entries are valid
    /// append-only post-publication state and therefore appear as unpinned
    /// files to an old edition snapshot. The H3 header/AEAD/manifest validator
    /// remains the approval oracle; normal tools/list remains memory-only.
    pub fn verified_hub_manifest(&self, server: &str) -> Result<ApprovedManifest> {
        self.bundle.gamma_verify().map_err(bridge_err)?;
        self.read_hub_manifest(server)
    }

    /// Pull an owner-published sealed connector binding into a replicated
    /// context. Remote contexts already read through the Provider and local
    /// contexts have no remote source, so their store adapters make this a
    /// no-op.
    pub fn refresh_hub_manifest(&self, server: &str) -> Result<()> {
        self.bundle
            .store
            .refresh_connector_binding(server)
            .map_err(bridge_err)
    }

    pub fn refresh_session_publications(&self) -> Result<()> {
        self.bundle
            .store
            .refresh_session_publications()
            .map_err(bridge_err)
    }

    // ------------------------------------------------------------ policy

    /// Is this tool covered by the agent's mandate at `now`? The polite
    /// pre-check — `record_act` re-verifies everything at append time.
    pub fn authorize(&self, tool: &str, now: &str) -> Result<()> {
        self.authorize_action(
            crate::policy::MCP_CONNECTOR,
            &crate::policy::action_name(tool),
            &op_for_tool(tool),
            now,
        )
    }

    fn authorize_action(&self, connector: &str, action: &str, op: &str, now: &str) -> Result<()> {
        let doc = self.did_doc()?;
        verify_chain(&self.agent_chain, &doc, now).map_err(|e| GatewayError::MandateDenied {
            op: op.to_owned(),
            reason: e.to_string(),
        })?;
        let covered = self
            .bundle
            .action_covered(&self.agent_chain, connector, action)
            .map_err(bridge_err)?;
        if covered {
            Ok(())
        } else {
            Err(GatewayError::MandateDenied {
                op: op.to_owned(),
                reason: "outside the granted perimeter".to_owned(),
            })
        }
    }

    pub fn authorize_hub(&self, server: &str, raw_tool: &str, now: &str) -> Result<()> {
        self.authorize_action(
            server,
            &crate::policy::action_name(raw_tool),
            &hub_op_for_tool(server, raw_tool),
            now,
        )
    }

    // ------------------------------------------------------------- gamma

    /// Append one act entry for an authorised call, signed by the
    /// gateway-held agent key via the agent's mandate chain. The kind is
    /// imposed by the operation (`action`), never by the caller; the
    /// arguments enter as a hash, never in clear.
    pub fn record_act(
        &mut self,
        tool: &str,
        args: &serde_json::Value,
        now: &str,
    ) -> Result<String> {
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let args_hash = hash_of(args)?;
        // The action is the flattened tool name (grammar constraint); the
        // raw tool name stays readable in the clear payload for auditors.
        let entry = self
            .bundle
            .log_action(
                &self.agent_chain,
                &agent_sk,
                &ActionSpec {
                    connector: crate::policy::MCP_CONNECTOR,
                    action: &crate::policy::action_name(tool),
                    args_hash: &args_hash,
                    now,
                    budget: Some(serde_json::json!({ "tool": tool })),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
        Ok(entry.id)
    }

    pub fn record_hub_act(
        &mut self,
        exposed_tool: &str,
        server: &str,
        raw_tool: &str,
        args: &serde_json::Value,
        now: &str,
    ) -> Result<String> {
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let args_hash = hash_of(args)?;
        let entry = self
            .bundle
            .log_action(
                &self.agent_chain,
                &agent_sk,
                &ActionSpec {
                    connector: server,
                    action: &crate::policy::action_name(raw_tool),
                    args_hash: &args_hash,
                    now,
                    budget: Some(serde_json::json!({
                        "tool": exposed_tool,
                        "server": server,
                        "upstream_tool": raw_tool
                    })),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
        Ok(entry.id)
    }

    /// Append one refusal entry: a governance act of the gateway's own
    /// identity, under the gateway's own mandate. The refused tool and
    /// the reason code stay readable in the clear payload — that is what
    /// the audit sells.
    pub fn record_refusal(&mut self, tool: &str, reason: &str, now: &str) -> Result<String> {
        self.record_refusal_entry(tool, reason, None, now)
    }

    /// A refusal carrying its pedagogical detail in clear (lot D): used
    /// for bound violations ONLY — the detail is exactly the owner's
    /// sealed rule plus the offending values, already served to the
    /// agent, no secret (decision 2026-07-15 n°1). Other refusal kinds
    /// keep the bare code: their messages are not structurally
    /// guaranteed leak-free the way bound refusals are.
    pub fn record_refusal_detailed(
        &mut self,
        tool: &str,
        reason: &str,
        detail: &str,
        now: &str,
    ) -> Result<String> {
        self.record_refusal_entry(tool, reason, Some(detail), now)
    }

    fn record_refusal_entry(
        &mut self,
        tool: &str,
        reason: &str,
        detail: Option<&str>,
        now: &str,
    ) -> Result<String> {
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let mut detail_json = serde_json::json!({ "tool": tool, "reason": reason });
        if let Some(text) = detail {
            detail_json["detail"] = serde_json::Value::String(text.to_owned());
        }
        let detail = detail_json;
        let args_hash = hash_of(&detail)?;
        let entry = self
            .bundle
            .log_action(
                &self.gateway_chain,
                &gateway_sk,
                &ActionSpec {
                    connector: "gateway",
                    action: "refuse",
                    args_hash: &args_hash,
                    now,
                    budget: Some(detail),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
        Ok(entry.id)
    }

    /// Append one OAuth-issuance governance entry (lot G3): a gateway
    /// act under its own mandate (`act.x.gateway.oauth_issue`), naming
    /// the client in the clear payload — NEVER the token, code or any
    /// secret. This is how "an issuance is an act, not a silent event"
    /// (I5) is kept: the auditor sees that a session was granted, to
    /// which client, when, without ever seeing what was handed over.
    pub fn record_oauth_issue(&mut self, client_id: &str, now: &str) -> Result<String> {
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let detail = serde_json::json!({ "client_id": client_id, "event": "oauth.issue" });
        let args_hash = hash_of(&detail)?;
        let entry = self
            .bundle
            .log_action(
                &self.gateway_chain,
                &gateway_sk,
                &ActionSpec {
                    connector: "gateway",
                    action: "oauth_issue",
                    args_hash: &args_hash,
                    now,
                    budget: Some(detail),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
        Ok(entry.id)
    }

    /// One non-secret gateway governance intent for the local connector
    /// binding registry. This reuses the existing `action` Gamma kind and
    /// the gateway's `act.x.gateway.*` mandate; it does not mint a new
    /// protocol operation or pretend the sidecar itself is a Core proof.
    pub fn record_connector_config(
        &mut self,
        context: &str,
        connector: &str,
        event: &str,
        now: &str,
    ) -> Result<String> {
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let detail = serde_json::json!({
            "context": context,
            "connector": connector,
            "event": event,
        });
        let args_hash = hash_of(&detail)?;
        let entry = self
            .bundle
            .log_action(
                &self.gateway_chain,
                &gateway_sk,
                &ActionSpec {
                    connector: "gateway",
                    action: "connector_config",
                    args_hash: &args_hash,
                    now,
                    budget: Some(detail),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|error| GatewayError::LogAppendRefused(error.to_string()))?;
        Ok(entry.id)
    }

    /// Non-secret proof link for a guarded approval/effect. The payload body,
    /// recipients and OAuth material never enter Gamma; the immutable digest
    /// and provider message id are sufficient to join decision and outcome.
    pub fn record_connector_effect(
        &mut self,
        context: &str,
        connector: &str,
        effect: &ConnectorEffectProof<'_>,
        now: &str,
    ) -> Result<String> {
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let detail = serde_json::json!({
            "context": context,
            "connector": connector,
            "event": effect.event,
            "approval_id": effect.approval_id,
            "payload_digest": effect.payload_digest,
            "message_id": effect.message_id,
        });
        let args_hash = hash_of(&detail)?;
        let entry = self
            .bundle
            .log_action(
                &self.gateway_chain,
                &gateway_sk,
                &ActionSpec {
                    connector: "gateway",
                    action: "connector_effect",
                    args_hash: &args_hash,
                    now,
                    budget: Some(detail),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|error| GatewayError::LogAppendRefused(error.to_string()))?;
        Ok(entry.id)
    }

    /// Append one cross-reference to THIS ethos (the agent's journal),
    /// mirroring an act recorded in a context gamma (§3bis.5/.7): the
    /// journal is the per-agent index, the context entry stays the only
    /// proof. Signed by the agent key under the journal's agent chain
    /// (the xref pen, `act.x.xref.*`); the join key `(ethos_did,
    /// entry_id)` rides clear in the payload.
    pub fn record_xref(
        &mut self,
        tool: &str,
        ethos_did: &str,
        entry_id: &str,
        now: &str,
    ) -> Result<String> {
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let join = serde_json::json!({
            "ethos_did": ethos_did,
            "entry_id": entry_id,
            "tool": tool,
        });
        let args_hash = hash_of(&join)?;
        let entry = self
            .bundle
            .log_action(
                &self.agent_chain,
                &agent_sk,
                &ActionSpec {
                    connector: "xref",
                    action: "ref",
                    args_hash: &args_hash,
                    now,
                    budget: Some(join),
                    sealed_args: None,
                },
                self.entropy.as_mut(),
            )
            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
        Ok(entry.id)
    }

    // --------------------------------------------------------- inference

    /// Is there tap left before touching the provider? Fail-closed: no
    /// inference pen → no LLM at all; a valid pen with its token budget
    /// spent refuses BEFORE any provider round-trip. The real gate stays
    /// at append time (`record_inference` re-runs the full budget check
    /// with the actual usage) — this is the polite pre-check.
    pub fn inference_headroom(&self, now: &str) -> Result<()> {
        let denied = |reason: String| GatewayError::MandateDenied {
            op: "inference".to_owned(),
            reason,
        };
        let chain = self
            .inference_chain
            .as_ref()
            .ok_or_else(|| denied("no inference pen granted on this journal".into()))?;
        let doc = self.did_doc()?;
        verify_chain(chain, &doc, now).map_err(|e| denied(e.to_string()))?;
        let leaf = chain.last().expect("non-empty chain");
        if let Some(profiles) =
            aithos_core::constraints::parse_budgets(&leaf.constraints).map_err(bridge_err)?
        {
            if let Some(profile) = profiles.iter().find(|p| p.id == LLM_BUDGET_REF) {
                if let Some(budget) = profile.token_budget {
                    let entries = self.bundle.gamma_entries().map_err(bridge_err)?;
                    let spent =
                        aithos_core::constraints::tally_tokens(&entries, &leaf.id, LLM_BUDGET_REF);
                    if spent >= budget {
                        return Err(denied(format!("token budget exhausted ({spent}/{budget})")));
                    }
                }
            }
        }
        Ok(())
    }

    /// Append one `inference` entry under the budgeted pen: metadata
    /// only (provider, model, the REAL token usage), never the prompt.
    /// The bundle re-checks the token budget at append — an inference
    /// that cannot be metered is refused, and the caller must withhold
    /// the completion.
    pub fn record_inference(
        &mut self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        now: &str,
    ) -> Result<String> {
        let chain = self
            .inference_chain
            .as_ref()
            .ok_or_else(|| GatewayError::MandateDenied {
                op: "inference".to_owned(),
                reason: "no inference pen granted on this journal".into(),
            })?;
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let entry = self
            .bundle
            .log_inference(
                chain,
                &agent_sk,
                &InferenceSpec {
                    provider,
                    model,
                    tokens_in,
                    tokens_out,
                    budget_ref: Some(LLM_BUDGET_REF),
                    now,
                },
                self.entropy.as_mut(),
            )
            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
        Ok(entry.id)
    }

    // ---------------------------------------------------- delegated writes

    /// Create a section in the ethos under the agent's own mandate chain
    /// (spec 04.2 `append`, 07.2): the bundle verifies chain, window,
    /// revocations and the verb lattice at append time, seals the blob
    /// with the agent's granted line, and logs the delegated
    /// `section.add`. Fails closed when the agent mandate carries no
    /// covering write perimeter.
    #[allow(clippy::too_many_arguments)]
    pub fn record_section_add(
        &mut self,
        folder_path: &str,
        name: &str,
        title: &str,
        tags: &[String],
        body: &str,
        now: &str,
    ) -> Result<()> {
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        self.bundle
            .section_add_as_agent(
                &self.agent_chain,
                &agent_sk,
                &aithos_bundle::bundle::SectionSpec {
                    zone: aithos_core::path::Zone::Circle,
                    folder_path,
                    name,
                    title,
                    tags,
                    body,
                    now,
                },
                self.entropy.as_mut(),
            )
            .map_err(write_denied)
    }

    /// Rewrite an existing section under the agent's chain (spec 04.2
    /// `edit`): delegated `section.modify` logged, same double wall.
    pub fn record_section_rewrite(
        &mut self,
        display_path: &str,
        body: &str,
        now: &str,
    ) -> Result<()> {
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        self.bundle
            .section_rewrite_as_agent(
                &self.agent_chain,
                &agent_sk,
                aithos_core::path::Zone::Circle,
                display_path,
                body,
                now,
                self.entropy.as_mut(),
            )
            .map_err(write_denied)
    }

    /// Delete a section under the agent's chain (spec 04.2 `delete`):
    /// delegated `section.delete` logged; the sealed blob stays, erasure
    /// is cryptographic (spec 06).
    pub fn record_section_delete(&mut self, display_path: &str, now: &str) -> Result<()> {
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        self.bundle
            .section_delete_as_agent(
                &self.agent_chain,
                &agent_sk,
                aithos_core::path::Zone::Circle,
                display_path,
                now,
                self.entropy.as_mut(),
            )
            .map_err(write_denied)
    }

    // ------------------------------------------------ journal memory (C2)

    /// The memory pen, fail-closed: journals provisioned before lot C2
    /// carry no pen, and every journal tool refuses — the LLM-tap
    /// precedent (no pen, no tool).
    fn memory_pen(&self, op: &str) -> Result<Vec<Mandate>> {
        self.memory_chain
            .as_ref()
            .cloned()
            .ok_or_else(|| GatewayError::MandateDenied {
                op: op.to_owned(),
                reason: "no memory pen granted on this journal".into(),
            })
    }

    /// Consolidate one memory note: ONE fresh sealed section in
    /// `circle:memory/` under the memory pen (decided 2026-07-12 — a
    /// note is a section, never a gamma payload). The bundle verifies
    /// chain, window, revocations and the append verb at write time and
    /// logs the delegated `section.add` (sealed body, §07.2); the
    /// technical name is unique, the human label rides in title and
    /// tags, clear in the zone index.
    pub fn journal_write(
        &mut self,
        title: &str,
        tags: &[String],
        text: &str,
        now: &str,
    ) -> Result<NoteView> {
        let chain = self.memory_pen("journal.write")?;
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let name = format!("n-{}", hex::encode(self.entropy.as_mut().e16()));
        self.bundle
            .section_add_as_agent(
                &chain,
                &agent_sk,
                &aithos_bundle::bundle::SectionSpec {
                    zone: Zone::Circle,
                    folder_path: MEMORY_FOLDER,
                    name: &name,
                    title,
                    tags,
                    body: text,
                    now,
                },
                self.entropy.as_mut(),
            )
            .map_err(write_denied)?;
        Ok(NoteView {
            name,
            title: title.to_owned(),
            tags: tags.to_vec(),
            text: None,
        })
    }

    /// Recall memory notes. The match runs on the CLEAR zone index only
    /// (name, title, tags — the readability frontier: the gateway holds
    /// the files), newest first; the sealed bodies are opened for the
    /// returned hits ONLY, under the same pen (append implies read,
    /// §04.2), and EVERY opened body is one journalized `ethos.read`
    /// (§07.9.2). An open that cannot be journalized fails the whole
    /// recall — no unlogged read ever leaves the gateway.
    pub fn journal_search(
        &mut self,
        query: Option<&str>,
        tag: Option<&str>,
        limit: usize,
        now: &str,
    ) -> Result<Vec<NoteView>> {
        let chain = self.memory_pen("journal.search")?;
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let matches = memory_rows(&self.bundle, query, tag)?;
        let mut hits = Vec::new();
        for row in matches.into_iter().rev().take(limit) {
            let path = format!("{MEMORY_FOLDER}/{}", row.name);
            let text = self
                .bundle
                .read_section_as_agent(&chain, &agent_sk, Zone::Circle, &path, now)
                .map_err(read_denied_op("journal.search"))?;
            self.bundle
                .log_read_as_agent(
                    &chain,
                    &agent_sk,
                    Zone::Circle,
                    &path,
                    now,
                    self.entropy.as_mut(),
                )
                .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
            hits.push(NoteView {
                name: row.name,
                title: row.title,
                tags: row.tags,
                text: Some(text),
            });
        }
        Ok(hits)
    }

    // ---------------------------------------------------- briefing (lot K)

    /// Does this context have anything to brief? True when the briefing
    /// pen is granted AND a granted zone (public or circle — never self)
    /// holds at least one directive. Index-only: no body is opened, no
    /// entry is journalized — this is the conditional-surface probe the
    /// router runs on `initialize` and `tools/list`. Every failure reads
    /// as "nothing to say" (mute surface, fail-closed); the read path
    /// errors loudly when actually called.
    pub fn briefing_available(&self) -> bool {
        if self.briefing_chain.is_none() {
            return false;
        }
        [Zone::Public, Zone::Circle].iter().any(|zone| {
            zone_rows(&self.bundle, *zone, BRIEFING_FOLDER, None, None)
                .map(|rows| !rows.is_empty())
                .unwrap_or(false)
        })
    }

    /// Serve the owner's directives, exact text, zone named on each —
    /// public first, then circle; `self` is structurally out of reach
    /// (the pen carries no self entry and no self index is ever listed).
    /// Every served CIRCLE section is one journalized `ethos.read` under
    /// the briefing pen (§07.9.2, the C2 precedent): a sealed read that
    /// cannot be journalized fails the whole briefing. Public sections
    /// are the readability frontier (§02.1 — clear, keyless, hash-
    /// pinned): no key opens them, so no sealed read entry exists for
    /// them in v1; the demo's directives live in circle, on the record.
    pub fn briefing_read(&mut self, now: &str) -> Result<Vec<BriefingItem>> {
        let chain =
            self.briefing_chain
                .as_ref()
                .cloned()
                .ok_or_else(|| GatewayError::MandateDenied {
                    op: "briefing.read".to_owned(),
                    reason: "no briefing pen granted on this context".into(),
                })?;
        let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
        let mut items = Vec::new();
        for zone in [Zone::Public, Zone::Circle] {
            let rows = zone_rows(&self.bundle, zone, BRIEFING_FOLDER, None, None)?;
            for row in rows {
                let path = format!("{BRIEFING_FOLDER}/{}", row.name);
                let text = match zone {
                    // Public bodies are clear by design (§02.1): the
                    // keyless read is the honest one — the hash pin in
                    // the index still authenticates the text.
                    Zone::Public => Bundle::public_read(&self.bundle.store, &path)
                        .map_err(read_denied_op("briefing.read"))?,
                    // Circle bodies are sealed: the full §04.5 verifier
                    // runs (certificate half), the delivered zone line
                    // opens the blob (physics half), and the read goes
                    // on the record under the same pen.
                    _ => {
                        let text = self
                            .bundle
                            .read_section_as_agent(&chain, &agent_sk, zone, &path, now)
                            .map_err(read_denied_op("briefing.read"))?;
                        self.bundle
                            .log_read_as_agent(
                                &chain,
                                &agent_sk,
                                zone,
                                &path,
                                now,
                                self.entropy.as_mut(),
                            )
                            .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
                        text
                    }
                };
                items.push(BriefingItem {
                    zone: zone.as_str().to_owned(),
                    title: row.title,
                    text,
                });
            }
        }
        Ok(items)
    }

    // ---------------------------------------------------- ethos read (lot G6)

    /// Every valid chain to the agent key carrying at least one ethos
    /// perimeter entry, reconstructed from the certificates the store
    /// holds — whatever gesture minted them (owner CLI today, a
    /// delegate's sub-mandate, the G8.c emission surface tomorrow).
    /// Recomputed per call, never cached, never a state field (decided
    /// 2026-07-16: the surface DERIVES from the mandates) — a fresh
    /// grant appears on the very next call and a revocation drops out
    /// the same way. Deterministic order (leaf mandate id); malformed,
    /// dangling, expired or revoked chains drop out silently here (the
    /// read path errors loudly).
    fn agent_read_chains(&self, now: &str) -> Vec<Vec<Mandate>> {
        let Ok(doc) = self.did_doc() else {
            return Vec::new();
        };
        let Ok(entries) = self.bundle.gamma_entries() else {
            return Vec::new();
        };
        let revs = revocations(&entries);
        let agent_pub = agent_pub_multibase(&self.keyholder);
        let mut chains: Vec<Vec<Mandate>> = self
            .walk_agent_cert_chains(&agent_pub)
            .into_iter()
            .filter(|chain| verify_chain_revocable(chain, &doc, now, &revs).is_ok())
            .collect();
        chains.sort_by_key(|chain| chain.last().map(|m| m.id.clone()));
        chains
    }

    /// The raw certificate walk under [`agent_read_chains`]: every
    /// reconstructable chain to the agent key carrying an ethos entry,
    /// UNVERIFIED — the callers apply the verification tier they need
    /// (full revocable for serving; signature-only for the pedagogical
    /// "your chain was revoked" refusal).
    fn walk_agent_cert_chains(&self, agent_pub: &str) -> Vec<Vec<Mandate>> {
        self.walk_cert_chains(agent_pub)
            .into_iter()
            .filter(|chain| {
                chain.last().is_some_and(|leaf| {
                    leaf.parsed_perimeter().is_ok_and(|entries| {
                        entries
                            .iter()
                            .any(|entry| matches!(entry, PerimeterEntry::Ethos { .. }))
                    })
                })
            })
            .collect()
    }

    fn walk_cert_chains(&self, grantee_pub: &str) -> Vec<Vec<Mandate>> {
        self.walk_cert_chains_censused(grantee_pub).0
    }

    /// The same walk, plus a census of what this view held and what the
    /// walk dropped in silence. Diagnostic material only: no verdict, no
    /// public response and no ordering reads the census, and
    /// `walk_cert_chains` discards it outright.
    ///
    /// Added 2026-07-25: an empty result was indistinguishable between
    /// "no certificate in this view", "certificate present but its chain
    /// is unresolvable here" and "certificate published for another key"
    /// — three different defective stages behind one silent zero.
    fn walk_cert_chains_censused(&self, grantee_pub: &str) -> (Vec<Vec<Mandate>>, CertWalkCensus) {
        let mut census = CertWalkCensus::default();
        let Ok(paths) = self.bundle.store.list("certs/") else {
            return (Vec::new(), census);
        };
        census.listed = paths.len();
        let mut by_id: BTreeMap<String, Mandate> = BTreeMap::new();
        for path in paths {
            let Ok(Some(bytes)) = self.bundle.store.get(&path) else {
                continue;
            };
            let Ok(mandate) = serde_json::from_slice::<Mandate>(&bytes) else {
                continue;
            };
            by_id.insert(mandate.id.clone(), mandate);
        }
        census.parsed = by_id.len();
        let mut chains: Vec<Vec<Mandate>> = Vec::new();
        let mut grantees: BTreeSet<String> = BTreeSet::new();
        for leaf in by_id.values() {
            grantees.insert(leaf.grantee.pubkey.clone());
            if leaf.grantee.pubkey != grantee_pub {
                continue;
            }
            census.leaves_for_grantee += 1;
            let mut chain = vec![leaf.clone()];
            let mut cursor = leaf;
            let mut resolvable = true;
            while let Some(parent_id) = &cursor.parent {
                match by_id.get(parent_id) {
                    Some(parent) => {
                        chain.insert(0, parent.clone());
                        cursor = parent;
                    }
                    None => {
                        resolvable = false;
                        if census.unresolvable_samples.len() < CERT_WALK_CENSUS_SAMPLES {
                            census
                                .unresolvable_samples
                                .push(format!("{}->{}", leaf.id, parent_id));
                        }
                        break;
                    }
                }
            }
            if resolvable {
                chains.push(chain);
            } else {
                census.unresolvable += 1;
            }
        }
        census.grantee_samples = grantees
            .into_iter()
            .take(CERT_WALK_CENSUS_SAMPLES)
            .collect();
        (chains, census)
    }

    /// The pedagogical revocation probe (cold path, refusals only): a
    /// chain that still SIGNS as valid and covers a circle read of this
    /// path, but fails the revocable verification — the refusal then
    /// names the revoked mandate instead of a generic coverage gap.
    fn revoked_covering_read(&self, path: &str, now: &str) -> Option<String> {
        let doc = self.did_doc().ok()?;
        let entries = self.bundle.gamma_entries().ok()?;
        let revs = revocations(&entries);
        let agent_pub = agent_pub_multibase(&self.keyholder);
        let row = zone_all_rows(&self.bundle, Zone::Circle)
            .into_iter()
            .find(|row| row.path == path)?;
        for chain in self.walk_agent_cert_chains(&agent_pub) {
            if verify_chain(&chain, &doc, now).is_err() {
                continue;
            }
            let op = Op {
                verb: Verb::Read,
                zone: Zone::Circle,
                folders: &row.folders,
                tags: &row.tags,
            };
            if verify_op(&chain, &doc, now, &op).is_err() {
                continue;
            }
            if let Err(refusal) = verify_chain_revocable(&chain, &doc, now, &revs) {
                return Some(refusal.to_string());
            }
        }
        None
    }

    /// The zones this context serves right now (lot G6): public appears
    /// when it holds content — the readability frontier (§02.1), clear
    /// by design, no coverage needed (decided 2026-07-16: any connected
    /// session is informed); circle appears when at least one scanned
    /// chain covers at least one existing row. `self` never appears in
    /// v1 (sealed structure — the delegated resolution is its own core
    /// lot). Index-only: nothing opens, nothing is journalized.
    pub fn ethos_surface(&self, now: &str) -> Vec<String> {
        let mut zones = Vec::new();
        if !zone_all_rows(&self.bundle, Zone::Public).is_empty() {
            zones.push("public".to_owned());
        }
        if !self.covered_circle_rows(now).is_empty() {
            zones.push("circle".to_owned());
        }
        zones
    }

    /// The circle rows at least one valid chain covers, each paired
    /// with the first covering chain (deterministic order — the chain
    /// an actual read would cite).
    fn covered_circle_rows(&self, now: &str) -> Vec<(EthosRow, Vec<Mandate>)> {
        let chains = self.agent_read_chains(now);
        if chains.is_empty() {
            return Vec::new();
        }
        let Ok(doc) = self.did_doc() else {
            return Vec::new();
        };
        let mut covered = Vec::new();
        for row in zone_all_rows(&self.bundle, Zone::Circle) {
            let op = Op {
                verb: Verb::Read,
                zone: Zone::Circle,
                folders: &row.folders,
                tags: &row.tags,
            };
            if let Some(chain) = chains
                .iter()
                .find(|chain| verify_op(chain, &doc, now, &op).is_ok())
            {
                covered.push((row, chain.clone()));
            }
        }
        covered
    }

    /// The covered skeleton (ethos.list): public rows plus covered
    /// circle rows — paths, titles and tags, never a body, never a
    /// gamma entry.
    pub fn ethos_list(&self, now: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        for row in zone_all_rows(&self.bundle, Zone::Public) {
            out.push(serde_json::json!({
                "zone": "public", "path": row.path, "title": row.title, "tags": row.tags,
            }));
        }
        for (row, _) in self.covered_circle_rows(now) {
            out.push(serde_json::json!({
                "zone": "circle", "path": row.path, "title": row.title, "tags": row.tags,
            }));
        }
        out
    }

    /// One section body (ethos.read). public: keyless and unjournalized
    /// — the readability frontier (§02.1, decided 2026-07-16). circle:
    /// under the first covering chain whose lines open the body; every
    /// open is ONE `ethos.read` entry under the chain that read, and an
    /// unjournalizable read fails the whole call (the C2 precedent).
    /// self: refused in v1 — never served by default (GAPS §4.2), and
    /// the delegated self resolution is its own core lot.
    pub fn ethos_read_section(&mut self, zone: &str, path: &str, now: &str) -> Result<String> {
        match zone {
            "public" => Bundle::public_read(&self.bundle.store, path).map_err(|_| {
                GatewayError::RequestRejected(format!("ethos.read: no public section at `{path}`"))
            }),
            "circle" => {
                let chains = self.agent_read_chains(now);
                if chains.is_empty() {
                    if let Some(revoked) = self.revoked_covering_read(path, now) {
                        return Err(GatewayError::MandateDenied {
                            op: "ethos.read".to_owned(),
                            reason: format!(
                                "the chain that covered this read is no longer valid: {revoked}"
                            ),
                        });
                    }
                    return Err(GatewayError::MandateDenied {
                        op: "ethos.read".to_owned(),
                        reason: "no valid chain covers the `read.circle` perimeter of this context — ask the owner for an ethos-read grant"
                            .to_owned(),
                    });
                }
                let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
                let mut denial: Option<GatewayError> = None;
                let mut opened: Option<(String, Vec<Mandate>)> = None;
                for chain in &chains {
                    match self.bundle.read_section_as_agent(
                        chain,
                        &agent_sk,
                        Zone::Circle,
                        path,
                        now,
                    ) {
                        Ok(text) => {
                            opened = Some((text, chain.clone()));
                            break;
                        }
                        Err(e) => denial = Some(read_denied_op("ethos.read")(e)),
                    }
                }
                let Some((text, chain)) = opened else {
                    if let Some(revoked) = self.revoked_covering_read(path, now) {
                        return Err(GatewayError::MandateDenied {
                            op: "ethos.read".to_owned(),
                            reason: format!(
                                "the chain that covered this read is no longer valid: {revoked}"
                            ),
                        });
                    }
                    return Err(match denial {
                        Some(GatewayError::MandateDenied { op, reason }) => {
                            GatewayError::MandateDenied {
                                op,
                                reason: format!(
                                    "the `read.circle` perimeter does not cover this call: {reason}"
                                ),
                            }
                        }
                        Some(other) => other,
                        None => GatewayError::BridgeFailed(
                            "no chain answered the circle read".to_owned(),
                        ),
                    });
                };
                self.bundle
                    .log_read_as_agent(
                        &chain,
                        &agent_sk,
                        Zone::Circle,
                        path,
                        now,
                        self.entropy.as_mut(),
                    )
                    .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
                Ok(text)
            }
            "self" => Err(GatewayError::MandateDenied {
                op: "ethos.read".to_owned(),
                reason: "the `read.self` perimeter is never served by default — an explicit self grant awaits the delegated self-resolution core lot"
                    .to_owned(),
            }),
            other => Err(GatewayError::RequestRejected(format!(
                "ethos.read: zone must be public, circle or self, not `{other}`"
            ))),
        }
    }

    /// The starting pack (ethos.context, decided 2026-07-16): the
    /// briefing directives (their lot-K record preserved — circle
    /// directives read on the record), the public bodies (clear,
    /// costless), and the covered sealed index (titles and paths, no
    /// body, no entry). The map, not the vault.
    pub fn ethos_context_pack(&mut self, now: &str) -> Result<serde_json::Value> {
        let directives = if self.briefing_available() {
            Some(self.briefing_read(now)?)
        } else {
            None
        };
        let mut public = Vec::new();
        for row in zone_all_rows(&self.bundle, Zone::Public) {
            let text = Bundle::public_read(&self.bundle.store, &row.path)
                .map_err(read_denied_op("ethos.context"))?;
            public.push(serde_json::json!({
                "path": row.path, "title": row.title, "text": text,
            }));
        }
        let index: Vec<serde_json::Value> = self
            .covered_circle_rows(now)
            .into_iter()
            .map(|(row, _)| {
                serde_json::json!({
                    "zone": "circle", "path": row.path, "title": row.title, "tags": row.tags,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "directives": directives,
            "public": public,
            "index": index,
        }))
    }

    // ---------------------------------------- delegated ethos reads (lot 1)
    //
    // The delegated surface IS the mandate (§3 target table of the
    // 2026-07-24 handoff): a session chain serves a zone only when its
    // leaf covers a `read` op on at least one existing row — public
    // included. The owner-agent frontier default ("any connected
    // session is informed") does NOT apply to a delegated session:
    // no covering entry, no surface, fail closed.

    /// The zones this (already chain-verified) delegated session serves
    /// right now. Certificate half only — the physics half (lines) is
    /// tested by the read itself, like the agent surface does.
    pub fn ethos_surface_for_chain(&self, chain: &[Mandate], now: &str) -> Vec<String> {
        let Ok(doc) = self.did_doc() else {
            return Vec::new();
        };
        let mut zones = Vec::new();
        for zone in [Zone::Public, Zone::Circle] {
            let covered = zone_all_rows(&self.bundle, zone)
                .into_iter()
                .any(|row| ethos_row_is_covered(chain, &doc, now, zone, &row));
            if covered {
                zones.push(zone.as_str().to_owned());
            }
        }
        zones
    }

    /// The covered skeleton for one delegated session (ethos.list):
    /// public and circle rows the session leaf covers — paths, titles
    /// and tags, never a body, never a gamma entry. `self` never
    /// appears (sealed structure — its delegated resolution is its own
    /// core lot).
    pub fn ethos_list_for_chain(&self, chain: &[Mandate], now: &str) -> Vec<serde_json::Value> {
        let Ok(doc) = self.did_doc() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for zone in [Zone::Public, Zone::Circle] {
            for row in zone_all_rows(&self.bundle, zone) {
                if ethos_row_is_covered(chain, &doc, now, zone, &row) {
                    out.push(serde_json::json!({
                        "zone": zone.as_str(), "path": row.path,
                        "title": row.title, "tags": row.tags,
                    }));
                }
            }
        }
        out
    }

    /// One section body under the SESSION chain (delegated ethos.read).
    /// public: covered-by-certificate first (no frontier default in a
    /// delegated session), then the clear read. circle: the whole §04.5
    /// walk under the session chain with the gateway key (the session
    /// leaf grantee); every open is ONE journalized `ethos.read` under
    /// the chain that read, and an unjournalizable read fails the whole
    /// call (the C2 precedent). self: refused.
    pub fn ethos_read_section_for_chain(
        &mut self,
        chain: &[Mandate],
        zone: &str,
        path: &str,
        now: &str,
    ) -> Result<String> {
        match zone {
            "public" => {
                let doc = self.did_doc()?;
                let row = zone_all_rows(&self.bundle, Zone::Public)
                    .into_iter()
                    .find(|row| row.path == path)
                    .ok_or_else(|| {
                        GatewayError::RequestRejected(format!(
                            "ethos.read: no public section at `{path}`"
                        ))
                    })?;
                if !ethos_row_is_covered(chain, &doc, now, Zone::Public, &row) {
                    return Err(GatewayError::MandateDenied {
                        op: "ethos.read".to_owned(),
                        reason: "the delegated session does not cover `read.public` on this path"
                            .to_owned(),
                    });
                }
                public_read_current(&self.bundle, path).map_err(|_| {
                    GatewayError::RequestRejected(format!(
                        "ethos.read: no public section at `{path}`"
                    ))
                })
            }
            "circle" => {
                let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
                let agent_sk = SigningKey::from_bytes(self.keyholder.agent_seed());
                // Physics candidates, in preference order: a line
                // delivered to the session leaf grantee (the gateway
                // key), else the custodian agent's own line — the
                // gateway holds both keys, and the authority cited is
                // the session chain either way.
                let physics = [
                    (
                        gateway_pub_multibase(&self.keyholder),
                        grantee_kex_secret(&gateway_sk),
                    ),
                    (
                        agent_pub_multibase(&self.keyholder),
                        grantee_kex_secret(&agent_sk),
                    ),
                ];
                let text = self
                    .bundle
                    .read_section_as_delegated_session(
                        chain,
                        &gateway_sk,
                        &physics,
                        Zone::Circle,
                        path,
                        now,
                    )
                    .map_err(read_denied_op("ethos.read"))?;
                self.bundle
                    .log_read_as_agent(
                        chain,
                        &gateway_sk,
                        Zone::Circle,
                        path,
                        now,
                        self.entropy.as_mut(),
                    )
                    .map_err(|e| GatewayError::LogAppendRefused(e.to_string()))?;
                Ok(text)
            }
            "self" => Err(GatewayError::MandateDenied {
                op: "ethos.read".to_owned(),
                reason: "the `read.self` perimeter is never served by default — an explicit self grant awaits the delegated self-resolution core lot"
                    .to_owned(),
            }),
            other => Err(GatewayError::RequestRejected(format!(
                "ethos.read: zone must be public, circle or self, not `{other}`"
            ))),
        }
    }

    /// The starting pack for one delegated session (ethos.context): the
    /// covered public bodies and the covered sealed index. NO
    /// directives here — the briefing keeps its own explicitly-mandated
    /// tool (`briefing.read`, lot K), never folded into a zone grant.
    pub fn ethos_context_pack_for_chain(
        &mut self,
        chain: &[Mandate],
        now: &str,
    ) -> Result<serde_json::Value> {
        let doc = self.did_doc()?;
        let mut public = Vec::new();
        for row in zone_all_rows(&self.bundle, Zone::Public) {
            if ethos_row_is_covered(chain, &doc, now, Zone::Public, &row) {
                let text = public_read_current(&self.bundle, &row.path)
                    .map_err(read_denied_op("ethos.context"))?;
                public.push(serde_json::json!({
                    "path": row.path, "title": row.title, "text": text,
                }));
            }
        }
        let mut index = Vec::new();
        for row in zone_all_rows(&self.bundle, Zone::Circle) {
            if ethos_row_is_covered(chain, &doc, now, Zone::Circle, &row) {
                index.push(serde_json::json!({
                    "zone": "circle", "path": row.path, "title": row.title, "tags": row.tags,
                }));
            }
        }
        Ok(serde_json::json!({
            "directives": serde_json::Value::Null,
            "public": public,
            "index": index,
        }))
    }

    // ------------------------------------- delegated ethos writes (lot 4)
    //
    // Explicit tools, one Core verb each (D6, plan 2026-07-24):
    // `ethos.create` needs `append`, `ethos.edit` needs `edit` (append
    // covers it — §04.2), `ethos.delete` needs `delete` (write covers
    // it). Circle only: the bundle's delegated-write pass is circle
    // scoped, and this wrapper refuses the rest instead of widening.
    // Every mutation is a log citizen signed by the session leaf key —
    // the primitives refuse an unjournalizable mutation outright.

    /// The clear digest of one section row (no body, no gamma entry) —
    /// the D8 concurrency precondition (`expected_digest`) compares
    /// against THIS before any delegated edit or delete.
    pub fn ethos_section_digest(&self, zone: Zone, path: &str) -> Result<String> {
        Self::ethos_section_digest_in(&self.bundle, zone, path)
    }

    fn ethos_section_digest_in(
        bundle: &Bundle<GatewayStore>,
        zone: Zone,
        path: &str,
    ) -> Result<String> {
        let (row, _) = bundle
            .resolve_clear(zone, path)
            .map_err(|_| GatewayError::RequestRejected(format!("no section at `{path}`")))?;
        Ok(row.blob_sha)
    }

    /// Open a mutation-scoped bundle. Provider-primary contexts use the
    /// already-verified session chain and gateway leaf key for every wire
    /// request; the permanent agent reader remains unchanged.
    fn delegated_write_bundle(&self, chain: &[Mandate], op: &str) -> Result<Bundle<GatewayStore>> {
        let leaf = chain.last().ok_or_else(|| GatewayError::MandateDenied {
            op: op.to_owned(),
            reason: "the delegated session chain is empty".to_owned(),
        })?;
        let gateway_pub = gateway_pub_multibase(&self.keyholder);
        if leaf.grantee.pubkey != gateway_pub {
            return Err(GatewayError::MandateDenied {
                op: op.to_owned(),
                reason: "the delegated session leaf is not bound to this gateway key".to_owned(),
            });
        }
        let mandate = chain.iter().map(|item| item.id.clone()).collect();
        let store =
            self.bundle
                .store
                .for_delegated_write(&self.keyholder, mandate, Box::new(OsEntropy))?;
        Bundle::open(store).map_err(bridge_err)
    }

    /// Build the independently cold-verified snapshot consumed by
    /// `aithos-client`. The request-scoped store carries the exact delegated
    /// chain, while the Gateway sidecar contributes only its local artifacts.
    fn ethos_client_snapshot_for_chain(
        &self,
        chain: &[Mandate],
    ) -> Result<aithos_client::VerifiedSnapshot> {
        let bundle = self.delegated_write_bundle(chain, "ethos.client.snapshot")?;
        let paths = bundle.store.list("").map_err(bridge_err)?;
        let mut artifacts = Vec::with_capacity(paths.len());
        for path in paths {
            if let Some(bytes) = bundle.store.get(&path).map_err(bridge_err)? {
                artifacts.push((path, bytes));
            }
        }
        aithos_client::ArtifactSnapshot::try_from_iter(artifacts)
            .and_then(aithos_client::ArtifactSnapshot::cold_verify)
            .map_err(|error| {
                GatewayError::BridgeFailed(format!("aithos-client snapshot refused: {error}"))
            })
    }

    /// Download only the proofs, circle skeleton and optional target blob
    /// required by one delegated mutation. Provider list results are path
    /// metadata; unrelated blob bodies are never fetched.
    fn ethos_client_working_set_for_chain(
        &self,
        chain: &[Mandate],
        intent: &aithos_client::MutationIntent,
        _now: &str,
    ) -> Result<aithos_client::VerifiedWorkingSet> {
        let bundle = self.delegated_write_bundle(chain, "ethos.client.working_set")?;
        let mut selected = bundle
            .store
            .list("")
            .map_err(bridge_err)?
            .into_iter()
            .filter(|path| {
                path == "manifest.json"
                    || path == "did.json"
                    || path.starts_with("certs/")
                    || path.starts_with("gamma/")
                    || (path.starts_with("e/circle/") && !path.starts_with("e/circle/blobs/"))
            })
            .collect::<BTreeSet<_>>();
        match intent {
            aithos_client::MutationIntent::Edit {
                zone: Zone::Circle,
                path,
                ..
            }
            | aithos_client::MutationIntent::Delete {
                zone: Zone::Circle,
                path,
                ..
            } => {
                let (row, _) = bundle
                    .resolve_clear(Zone::Circle, path)
                    .map_err(write_denied_op("ethos.client.working_set"))?;
                selected.insert(format!("e/circle/blobs/{}.enc", row.sid));
            }
            aithos_client::MutationIntent::Create {
                zone: Zone::Circle, ..
            } => {}
            _ => {
                return Err(GatewayError::MandateDenied {
                    op: "ethos.client.working_set".to_owned(),
                    reason: "the Client working-set canary is limited to circle".to_owned(),
                });
            }
        }
        let mut artifacts = Vec::with_capacity(selected.len());
        for path in selected {
            let bytes = bundle
                .store
                .get(&path)
                .map_err(bridge_err)?
                .ok_or_else(|| {
                    GatewayError::BridgeFailed(format!(
                        "aithos-client working-set artifact disappeared: {path}"
                    ))
                })?;
            artifacts.push((path, bytes));
        }
        let manifest: aithos_bundle::manifest::Manifest = serde_json::from_slice(
            artifacts
                .iter()
                .find_map(|(path, bytes)| (path == "manifest.json").then_some(bytes))
                .ok_or_else(|| {
                    GatewayError::BridgeFailed(
                        "aithos-client working-set manifest is unavailable".into(),
                    )
                })?,
        )
        .map_err(bridge_err)?;
        let head = format!("sha256:{}", manifest.chain_hash().map_err(bridge_err)?);
        aithos_client::ArtifactSnapshot::try_from_iter(artifacts)
            .and_then(|snapshot| snapshot.verify_circle_mutation_working_set(&head, chain, intent))
            .map_err(|error| {
                GatewayError::BridgeFailed(format!("aithos-client working-set refused: {error}"))
            })
    }

    fn prepare_ethos_client_mutation_for_chain(
        &mut self,
        chain: &[Mandate],
        intent: aithos_client::MutationIntent,
        response_context: &str,
        response_zone: &str,
        response_path: &str,
        now: &str,
    ) -> Result<crate::ethos_backend::PreparedEthosMutation> {
        let (provider_url, tenant, did) =
            self.bundle.store.provider_coordinates().ok_or_else(|| {
                GatewayError::RequestRejected(
                    "aithos-client Provider mutations require a provider-primary Ethos".into(),
                )
            })?;
        let working_set = self.ethos_client_working_set_for_chain(chain, &intent, now)?;
        let publication_entropy =
            aithos_client::PublicationEntropy::new(self.entropy.e16(), self.entropy.e16());
        let transport = crate::ethos_backend::ProviderTransport::new(&provider_url)?;
        let host = transport.envelope_host().to_owned();
        let keyholder = Arc::clone(&self.keyholder);
        let deleting = matches!(intent, aithos_client::MutationIntent::Delete { .. });
        let plan = keyholder
            .with_ethos_client_grantee(|client| {
                aithos_client::PublicationPlan::build_grantee_from_working_set(
                    client,
                    chain,
                    working_set,
                    intent,
                    publication_entropy,
                )
            })
            .map_err(|error| {
                if std::env::var("AITHOS_ETHOS_DIAGNOSTICS").as_deref() == Ok("protocol") {
                    eprintln!(
                        "aithos_gateway_ethos_protocol_diagnostic: mutation planning: {error:?}"
                    );
                }
                GatewayError::BridgeFailed(format!(
                    "aithos-client mutation planning refused: {error}"
                ))
            })?;
        if plan.did() != did {
            return Err(GatewayError::BridgeFailed(
                "aithos-client mutation subject drifted".into(),
            ));
        }
        let digest = (!deleting)
            .then(|| plan.circle_section_digest(response_path))
            .transpose()
            .map_err(|error| {
                GatewayError::BridgeFailed(format!(
                    "aithos-client mutation result refused: {error}"
                ))
            })?;
        let nonces = (0..plan.upload_order().len())
            .map(|_| self.entropy.e16())
            .collect::<Vec<_>>();
        let envelopes = keyholder
            .with_ethos_client_grantee(|client| {
                plan.upload_order()
                    .iter()
                    .enumerate()
                    .map(|(index, artifact)| {
                        aithos_client::ProviderEnvelopePlan::for_grantee_working_set_publication(
                            client,
                            &plan,
                            aithos_client::ProviderUploadIntent::new(
                                &host,
                                &tenant,
                                artifact,
                                now,
                                nonces[index],
                            ),
                        )
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .map_err(|error| {
                GatewayError::BridgeFailed(format!(
                    "aithos-client Provider envelopes refused: {error}"
                ))
            })?;
        let commit_nonce = self.entropy.e16();
        let commit_probe = keyholder
            .with_ethos_client_grantee(|client| {
                aithos_client::ProviderReadEnvelopePlan::for_grantee(
                    client,
                    chain,
                    aithos_client::ProviderReadIntent::new(
                        &host,
                        &tenant,
                        plan.did(),
                        aithos_client::ProviderReadTarget::Heads,
                        now,
                        commit_nonce,
                    ),
                )
            })
            .map_err(|error| {
                GatewayError::BridgeFailed(format!("aithos-client commit probe refused: {error}"))
            })?;
        let expected_new_head = plan.new_head().to_owned();
        let context = response_context.to_owned();
        let zone = response_zone.to_owned();
        let path = response_path.to_owned();
        let response = if deleting {
            serde_json::json!({
                "context": context,
                "zone": zone,
                "path": path,
                "deleted": true,
            })
        } else {
            serde_json::json!({
                "context": context,
                "zone": zone,
                "path": path,
                "digest": digest,
            })
        };
        crate::ethos_backend::PreparedEthosMutation::new(
            transport,
            envelopes,
            commit_probe,
            expected_new_head,
            response,
        )
    }

    /// Fail on the certificate half before opening a Provider client under
    /// the delegated signer. A read-only session cannot necessarily read the
    /// remote gamma/index under its own chain, but it must still receive the
    /// stable mandate denial produced by Core rather than a transport 403.
    ///
    /// Folder SIDs and tags are resolved through the long-lived read client,
    /// so the preflight checks the exact same target as the bundle mutation.
    /// The mutation rechecks this authorization inside the scoped bundle.
    #[allow(clippy::too_many_arguments)]
    fn verify_delegated_write_target(
        &self,
        chain: &[Mandate],
        verb: Verb,
        zone: Zone,
        folders: &[Sid],
        tags: &[String],
        now: &str,
        op: &'static str,
    ) -> Result<()> {
        let doc = self.did_doc()?;
        let revocations = self
            .bundle
            .active_revocations()
            .map_err(write_denied_op(op))?;
        verify_chain_revocable(chain, &doc, now, &revocations).map_err(write_denied_op(op))?;
        let leaf = chain.last().ok_or_else(|| GatewayError::MandateDenied {
            op: op.to_owned(),
            reason: "empty chain".to_owned(),
        })?;
        let target = Op {
            verb,
            zone,
            folders,
            tags,
        };
        if !covers_op(
            &leaf.parsed_perimeter().map_err(write_denied_op(op))?,
            &target,
        ) {
            return Err(GatewayError::MandateDenied {
                op: op.to_owned(),
                reason: format!("{}: write not covered by the leaf perimeter", leaf.id),
            });
        }
        Ok(())
    }

    /// The delegated write surface of one session chain: which mutation
    /// verbs the leaf covers on the circle zone root. `(create/edit,
    /// delete)` — public and self serve no mutation surface in this
    /// pass, fail closed.
    pub fn ethos_write_surface_for_chain(&self, chain: &[Mandate], now: &str) -> (bool, bool) {
        let Ok(doc) = self.did_doc() else {
            return (false, false);
        };
        let covered = |verb: Verb| {
            let op = Op {
                verb,
                zone: Zone::Circle,
                folders: &[],
                tags: &[],
            };
            verify_op(chain, &doc, now, &op).is_ok()
        };
        (covered(Verb::Append), covered(Verb::Delete))
    }

    fn delegated_write_zone(zone: &str, op: &str) -> Result<Zone> {
        match zone {
            "circle" => Ok(Zone::Circle),
            "public" => Err(GatewayError::MandateDenied {
                op: op.to_owned(),
                reason:
                    "delegated public mutations await their own core lot — circle only this pass"
                        .to_owned(),
            }),
            "self" => Err(GatewayError::MandateDenied {
                op: op.to_owned(),
                reason: "the `self` zone is never mutable by delegation".to_owned(),
            }),
            other => Err(GatewayError::RequestRejected(format!(
                "{op}: zone must be public, circle or self, not `{other}`"
            ))),
        }
    }

    /// Delegated section creation under the session chain (ethos.create).
    #[allow(clippy::too_many_arguments)]
    pub fn ethos_create_for_chain(
        &mut self,
        chain: &[Mandate],
        zone: &str,
        folder: &str,
        name: &str,
        title: &str,
        tags: &[String],
        body: &str,
        now: &str,
    ) -> Result<String> {
        let zone = Self::delegated_write_zone(zone, "ethos.create")?;
        let folders = self
            .bundle
            .resolve_folder(zone, folder)
            .map_err(write_denied_op("ethos.create"))?;
        self.verify_delegated_write_target(
            chain,
            Verb::Append,
            zone,
            &folders,
            tags,
            now,
            "ethos.create",
        )?;
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let mut bundle = self.delegated_write_bundle(chain, "ethos.create")?;
        bundle
            .section_add_as_agent(
                chain,
                &gateway_sk,
                &aithos_bundle::bundle::SectionSpec {
                    zone,
                    folder_path: folder,
                    name,
                    title,
                    tags,
                    body,
                    now,
                },
                self.entropy.as_mut(),
            )
            .map_err(write_denied_op("ethos.create"))?;
        let path = if folder.is_empty() {
            name.to_owned()
        } else {
            format!("{folder}/{name}")
        };
        Self::ethos_section_digest_in(&bundle, zone, &path)
    }

    /// Delegated rewrite under the session chain (ethos.edit). The D8
    /// precondition is REQUIRED: a rewrite whose `expected_digest` does
    /// not match the current row refuses before anything is sealed —
    /// no silent overwrite of a concurrent change.
    pub fn ethos_edit_for_chain(
        &mut self,
        chain: &[Mandate],
        zone: &str,
        path: &str,
        body: &str,
        expected_digest: &str,
        now: &str,
    ) -> Result<String> {
        let zone = Self::delegated_write_zone(zone, "ethos.edit")?;
        let (row, folders) = self
            .bundle
            .resolve_clear(zone, path)
            .map_err(|_| GatewayError::RequestRejected(format!("no section at `{path}`")))?;
        self.verify_delegated_write_target(
            chain,
            Verb::Edit,
            zone,
            &folders,
            &row.tags,
            now,
            "ethos.edit",
        )?;
        let current = row.blob_sha;
        if current != expected_digest {
            return Err(GatewayError::MandateDenied {
                op: "ethos.edit".to_owned(),
                reason: format!(
                    "stale precondition: the section changed since it was read (current digest `{current}`)"
                ),
            });
        }
        let mut bundle = self.delegated_write_bundle(chain, "ethos.edit")?;
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        bundle
            .section_rewrite_as_agent(
                chain,
                &gateway_sk,
                zone,
                path,
                body,
                now,
                self.entropy.as_mut(),
            )
            .map_err(write_denied_op("ethos.edit"))?;
        Self::ethos_section_digest_in(&bundle, zone, path)
    }

    /// Delegated deletion under the session chain (ethos.delete). The
    /// D8 precondition is optional here, enforced when present.
    pub fn ethos_delete_for_chain(
        &mut self,
        chain: &[Mandate],
        zone: &str,
        path: &str,
        expected_digest: Option<&str>,
        now: &str,
    ) -> Result<()> {
        let zone = Self::delegated_write_zone(zone, "ethos.delete")?;
        let (row, folders) = self
            .bundle
            .resolve_clear(zone, path)
            .map_err(write_denied_op("ethos.delete"))?;
        self.verify_delegated_write_target(
            chain,
            Verb::Delete,
            zone,
            &folders,
            &row.tags,
            now,
            "ethos.delete",
        )?;
        if let Some(expected) = expected_digest {
            let current = row.blob_sha;
            if current != expected {
                return Err(GatewayError::MandateDenied {
                    op: "ethos.delete".to_owned(),
                    reason: format!(
                        "stale precondition: the section changed since it was read (current digest `{current}`)"
                    ),
                });
            }
        }
        let mut bundle = self.delegated_write_bundle(chain, "ethos.delete")?;
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        bundle
            .section_delete_as_agent(chain, &gateway_sk, zone, path, now, self.entropy.as_mut())
            .map_err(write_denied_op("ethos.delete"))
    }

    // ------------------------------------------------------------- audit

    /// Run the auditor's scoped query with the auditor's own key and
    /// mandate, and serialise the slice. A query outside the granted
    /// perimeter is refused by the certificate half (§07.8).
    pub fn export_audit(
        &self,
        auditor_seed: &[u8; 32],
        kind: Option<&str>,
        now: &str,
    ) -> Result<String> {
        let auditor_sk = SigningKey::from_bytes(auditor_seed);
        let auditor_mandate = self
            .auditor_mandate
            .as_ref()
            .ok_or_else(|| GatewayError::AuditDenied("no audit grant on this ethos".into()))?;
        let chain = vec![auditor_mandate.clone()];
        let query = GammaQuery {
            kind: kind.map(str::to_owned),
            ..GammaQuery::default()
        };
        let filter = LogFilter {
            kind: kind.map(str::to_owned),
            ..LogFilter::default()
        };
        let hits = self
            .bundle
            .log_query_as_agent(&chain, &auditor_sk, &query, &filter, now)
            .map_err(|e| GatewayError::AuditDenied(e.to_string()))?;
        let entries: Vec<EntryView> = hits.iter().map(|h| view(&h.entry)).collect();
        let export = serde_json::json!({
            "exported_at": now,
            "mandate": auditor_mandate.id,
            "scope": { "kind": kind },
            "entries": entries,
        });
        serde_json::to_string_pretty(&export).map_err(bridge_err)
    }

    // ------------------------------------------------------------ queries

    /// Clear view of the whole log (owner-side surface, test assertions).
    pub fn entries(&self) -> Result<Vec<EntryView>> {
        Ok(self
            .bundle
            .gamma_entries()
            .map_err(bridge_err)?
            .iter()
            .map(view)
            .collect())
    }

    /// Offline verification of the whole gamma chain (hashes, signatures,
    /// authorities). Completeness *proofs* land with H (Merkle roots).
    pub fn verify_log(&self) -> Result<()> {
        self.bundle.gamma_verify().map_err(bridge_err)
    }

    /// The DID of the ethos this bridge serves (xref payloads, joins).
    pub fn ethos_did(&self) -> &str {
        &self.bundle.did
    }

    /// Validate one remotely published Ethos before it can enter the hot
    /// runner catalogue. This is deliberately a Core-side verdict: the HTTP
    /// request contributes identifiers only, never trust or provider
    /// coordinates.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_hot_enrollment(
        &self,
        expected_did: &str,
        agent_mandate: &str,
        gateway_mandate: &str,
        agent_signing: &str,
        agent_kex: &str,
        gateway_signing: &str,
        gateway_kex: &str,
        audience: &str,
        required_agent_actions: &[String],
        required_gateway_actions: &[String],
        now: &str,
    ) -> Result<()> {
        let rejected = |reason: &str| GatewayError::MandateDenied {
            op: "ethos.enroll".to_owned(),
            reason: reason.to_owned(),
        };
        if self.bundle.did != expected_did
            || self.agent_mandate_id() != agent_mandate
            || self.gateway_mandate_id() != gateway_mandate
            || agent_mandate == gateway_mandate
        {
            return Err(rejected("published identity or mandate selection differs"));
        }
        self.bundle
            .gamma_verify()
            .map_err(|_| rejected("published Gamma history is invalid"))?;
        let doc = self
            .did_doc()
            .map_err(|_| rejected("published DID document is invalid"))?;
        if doc.id != expected_did || doc.verify().is_err() {
            return Err(rejected("published DID document is invalid"));
        }
        let entries = self
            .bundle
            .gamma_entries()
            .map_err(|_| rejected("published Gamma history is invalid"))?;
        let revs = revocations(&entries);
        for chain in [&self.agent_chain, &self.gateway_chain] {
            if !enrollment_chain_is_direct_owner(chain) {
                return Err(rejected("enrollment requires a direct Owner root mandate"));
            }
            verify_chain_revocable(chain, &doc, now, &revs)
                .map_err(|_| rejected("published mandate is inactive or revoked"))?;
            if chain.iter().any(|mandate| {
                mandate.subject != expected_did || !grant_logged(&entries, &mandate.id)
            }) {
                return Err(rejected("published mandate is not logged under this Ethos"));
            }
            let leaf = chain
                .last()
                .ok_or_else(|| rejected("published mandate chain is empty"))?;
            if leaf
                .parsed_perimeter()
                .map_err(|_| rejected("published mandate perimeter is invalid"))?
                .iter()
                .any(|entry| matches!(entry, PerimeterEntry::Issue { .. }))
            {
                return Err(rejected(
                    "enrollment mandates may not carry issue authority",
                ));
            }
            let constraints_are_bound = leaf.constraints.as_object().is_some_and(|constraints| {
                constraints.is_empty()
                    || (constraints.len() == 1
                        && constraints
                            .get("purpose")
                            .and_then(serde_json::Value::as_str)
                            == Some(audience))
            });
            if !constraints_are_bound {
                return Err(rejected(
                    "enrollment mandate constraints are not empty or audience-bound",
                ));
            }
        }
        let agent = self
            .agent_chain
            .last()
            .ok_or_else(|| rejected("published agent mandate chain is empty"))?;
        if agent.perimeter != required_agent_actions {
            return Err(rejected(
                "agent mandate perimeter differs from the exact enrollment rights",
            ));
        }
        if agent.grantee.pubkey != agent_signing || agent.grantee.kex_pubkey != agent_kex {
            return Err(rejected(
                "agent mandate is equipped to another gateway identity",
            ));
        }
        let gateway = self
            .gateway_chain
            .last()
            .ok_or_else(|| rejected("published gateway mandate chain is empty"))?;
        if gateway.perimeter != required_gateway_actions {
            return Err(rejected(
                "governance mandate perimeter differs from the exact enrollment rights",
            ));
        }
        if gateway.grantee.pubkey != gateway_signing || gateway.grantee.kex_pubkey != gateway_kex {
            return Err(rejected(
                "governance mandate is equipped to another gateway identity",
            ));
        }
        for required in required_agent_actions {
            let action = required
                .strip_prefix("act.x.gateway.")
                .ok_or_else(|| rejected("gateway required-action configuration is invalid"))?;
            if !self
                .bundle
                .action_covered(&self.agent_chain, "gateway", action)
                .map_err(|_| rejected("agent mandate perimeter is invalid"))?
            {
                return Err(rejected("agent mandate misses a required gateway action"));
            }
        }
        for required in required_gateway_actions {
            let action = required
                .strip_prefix("act.x.gateway.")
                .ok_or_else(|| rejected("gateway required-action configuration is invalid"))?;
            if !self
                .bundle
                .action_covered(&self.gateway_chain, "gateway", action)
                .map_err(|_| rejected("governance mandate perimeter is invalid"))?
            {
                return Err(rejected(
                    "governance mandate misses a required gateway action",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn store_clone(&self) -> GatewayStore {
        self.bundle.store.clone()
    }

    /// The mandate id the agent acts under (test assertions).
    pub fn agent_mandate_id(&self) -> &str {
        &self.agent_chain[0].id
    }

    /// The mandate id the gateway refuses under (test assertions).
    pub fn gateway_mandate_id(&self) -> &str {
        &self.gateway_chain[0].id
    }

    /// The `not_after` of the agent chain IF it verifies at `now` (lot
    /// G3): the ceiling an OAuth token bound to this context's agent
    /// authority must not outlive. `None` when the chain is expired or
    /// otherwise invalid — the pre-G4 stand-in for "the bound authority
    /// is gone, redo the ceremony". Read-only; G4/G5 replace the binding
    /// with the session sub-mandate's `not_after` through the same seam.
    pub fn agent_authority_ceiling(&self, now: &str) -> Option<String> {
        let doc = self.did_doc().ok()?;
        verify_chain(&self.agent_chain, &doc, now).ok()?;
        self.agent_chain.last().map(|m| m.not_after.clone())
    }

    fn did_doc(&self) -> Result<DidDocument> {
        read_json(&self.bundle, "did.json")
    }
}

// ------------------------------------------------------ multi-Ethos runner

/// One provisioned context at runtime: its routing tool map and its
/// live bridge into the context ethos.
pub struct ContextRuntime {
    pub policy: Policy,
    pub bridge: Bridge,
}

#[derive(Debug, Clone, Serialize)]
pub struct EligibleSessionParent {
    pub context: String,
    pub parent_id: String,
    pub subject: String,
    pub not_before: String,
    pub not_after: String,
    pub perimeter: Vec<String>,
    pub session_perimeter: Vec<String>,
    pub constraints: serde_json::Value,
    pub chain: Vec<serde_json::Value>,
    pub did: serde_json::Value,
    pub revocations: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionAuthority {
    pub context: String,
    pub subject: String,
    pub parent_id: String,
    pub leaf_id: String,
    pub not_before: String,
    pub not_after: String,
    pub session_pub: String,
    pub chain: Vec<serde_json::Value>,
    pub leaf: serde_json::Value,
    pub certificate: serde_json::Value,
}

pub struct PreparedSessionOperation {
    pub chain: Vec<Mandate>,
    pub did: DidDocument,
    pub revocations: Vec<Revocation>,
    pub mandate: serde_json::Value,
    pub certificate: serde_json::Value,
    pub certificate_digest: String,
    pub projection: serde_json::Value,
    pub operation_ref: serde_json::Value,
    pub native_leaf_proof: serde_json::Value,
}

#[derive(Debug, Clone)]
struct HubRuntimeTool {
    context: String,
    server: String,
    raw_tool: String,
    description: Option<String>,
    input_schema: serde_json::Value,
    pin_sha256: String,
    #[allow(dead_code)]
    access: ToolAccess,
    /// The effective grant decision (lot W): drives the exposed
    /// `tools/list`. Refusals still flow from the mandate itself.
    granted: bool,
    /// Owner-approved argument bounds (lot P), from the sealed manifest.
    bounds: Vec<crate::hub::ArgumentBound>,
}

/// How many samples a census keeps. Diagnostic lines stay bounded: a
/// context holding thousands of certificates must not turn one opt-in
/// eligibility probe into an unbounded terminal dump.
const CERT_WALK_CENSUS_SAMPLES: usize = 8;

/// What a certificate walk saw, for the opt-in G4 diagnostic only.
/// Public keys and mandate ids are already public surface; no payload,
/// no secret and no private key ever enters this structure.
#[derive(Debug, Default, Clone)]
struct CertWalkCensus {
    /// Paths the store listed under `certs/` (the view's raw size).
    listed: usize,
    /// Of those, how many parsed as a mandate.
    parsed: usize,
    /// Leaves whose `grantee.pubkey` is exactly the probed signer.
    leaves_for_grantee: usize,
    /// Leaves for that signer whose parent link left this view.
    unresolvable: usize,
    /// Bounded `leaf->missing_parent` samples for the dropped chains.
    unresolvable_samples: Vec<String>,
    /// Bounded sample of the distinct grantee keys actually present.
    grantee_samples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubRelayTarget {
    pub server: String,
    pub raw_tool: String,
    /// Hot routes must resolve only through the dynamic registry. This bit
    /// prevents a removed/poisoned connector entry from falling through to
    /// a same-name static template or context upstream.
    pub hot: bool,
}

/// The multi-Ethos runtime (Phase B lot 3): N context bridges plus the
/// agent's journal bridge, all signing with the runner's ONE identity.
/// An act writes twice — the context gamma (the proof) and a journal
/// xref (the per-agent index); a refusal writes to the journal always,
/// and to the context too when the tool names one (§3bis.8).
pub struct Runner {
    contexts: BTreeMap<String, ContextRuntime>,
    journal: Bridge,
    hub_tools: BTreeMap<String, HubRuntimeTool>,
    hub_server_pins: BTreeMap<String, BTreeMap<String, String>>,
    hub_drift: BTreeMap<String, String>,
    hot_tools: BTreeMap<String, HubRuntimeTool>,
    /// One hot descriptor can be referenced by several Ethos contexts.
    /// The descriptor/pin stays global while availability remains scoped by
    /// the owner-published binding present in each context.
    hot_tool_contexts: BTreeMap<String, BTreeSet<String>>,
    hot_server_pins: BTreeMap<String, BTreeMap<String, String>>,
}

impl Runner {
    /// Assemble from pre-built parts (the acceptance tests' door).
    pub fn from_parts(contexts: BTreeMap<String, ContextRuntime>, journal: Bridge) -> Self {
        Self {
            contexts,
            journal,
            hub_tools: BTreeMap::new(),
            hub_server_pins: BTreeMap::new(),
            hub_drift: BTreeMap::new(),
            hot_tools: BTreeMap::new(),
            hot_tool_contexts: BTreeMap::new(),
            hot_server_pins: BTreeMap::new(),
        }
    }

    pub(crate) fn hot_context_conflicts(&self, name: &str, did: &str) -> bool {
        self.contexts.contains_key(name)
            || self
                .contexts
                .values()
                .any(|runtime| runtime.bridge.ethos_did() == did)
    }

    /// Insert a context whose remote proofs have already passed
    /// `Bridge::validate_hot_enrollment`. Runtime policy starts empty:
    /// connector equipment is a later owner-governed hot operation.
    pub(crate) fn insert_hot_context(
        &mut self,
        name: String,
        bridge: Bridge,
    ) -> Result<GatewayStore> {
        if self.hot_context_conflicts(&name, bridge.ethos_did()) {
            return Err(GatewayError::RequestRejected("context_conflict".into()));
        }
        let store = bridge.store_clone();
        self.contexts.insert(
            name,
            ContextRuntime {
                policy: Policy::new(BTreeMap::new()),
                bridge,
            },
        );
        Ok(store)
    }

    pub fn gateway_public_key(&self) -> String {
        gateway_pub_multibase(&self.journal.keyholder)
    }

    pub fn gateway_kex_public_key(&self) -> String {
        gateway_kex_pub_multibase(&self.journal.keyholder)
    }

    pub(crate) fn context_is_provider_primary(&self, context: &str) -> bool {
        self.context(context)
            .ok()
            .and_then(|runtime| runtime.bridge.bundle.store.provider_coordinates())
            .is_some()
    }

    /// Discover only fresh, revocation-checked chains whose current leaf is
    /// held by `delegate_pub` and carries an issue right. The OAuth request
    /// contributes no context or mandate selector.
    pub fn eligible_session_parents(
        &mut self,
        delegate_pub: &str,
        resource: &str,
        now: &str,
    ) -> Vec<EligibleSessionParent> {
        let diagnostics = std::env::var("AITHOS_ETHOS_DIAGNOSTICS").as_deref() == Ok("protocol");
        let mut eligible = Vec::new();
        for (context, runtime) in &mut self.contexts {
            if let Err(refresh_error) = runtime.bridge.refresh_session_publications() {
                // TEMPORARY DIAGNOSTIC (2026-07-24): surface why a context is
                // skipped during ceremony eligibility — remove after debugging.
                eprintln!(
                    "[eligible] context `{context}` refresh_session_publications FAILED: {refresh_error:?}"
                );
                continue;
            }
            let Ok(doc) = runtime.bridge.did_doc() else {
                continue;
            };
            let Ok(entries) = runtime.bridge.bundle.gamma_entries() else {
                continue;
            };
            let revs = revocations(&entries);
            let Ok(did) = serde_json::to_value(&doc) else {
                continue;
            };
            let revocation_entries = entries
                .iter()
                .filter(|entry| entry.kind == "revoke")
                .filter_map(|entry| serde_json::to_value(entry).ok())
                .collect::<Vec<_>>();
            let (chains, census) = runtime.bridge.walk_cert_chains_censused(delegate_pub);
            if diagnostics {
                eprintln!(
                    "aithos_gateway_g4_diagnostic: context={context} signer={delegate_pub} certs_listed={} certs_parsed={} leaves_for_signer={} unresolvable_chains={} reconstructed_chains={}",
                    census.listed,
                    census.parsed,
                    census.leaves_for_grantee,
                    census.unresolvable,
                    chains.len()
                );
                for sample in &census.unresolvable_samples {
                    eprintln!(
                        "aithos_gateway_g4_diagnostic: context={context} rejected=unresolvable_parent chain={sample}"
                    );
                }
                if census.leaves_for_grantee == 0 {
                    eprintln!(
                        "aithos_gateway_g4_diagnostic: context={context} rejected=no_leaf_for_signer grantees=[{}]",
                        census.grantee_samples.join(" ")
                    );
                }
            }
            for chain in chains {
                let parent_id = chain
                    .last()
                    .map(|parent| parent.id.as_str())
                    .unwrap_or("<empty>");
                if let Err(error) = verify_chain_revocable(&chain, &doc, now, &revs) {
                    if diagnostics {
                        eprintln!(
                            "aithos_gateway_g4_diagnostic: context={context} parent={parent_id} rejected=chain error={error}"
                        );
                    }
                    continue;
                }
                let Some(parent) = chain.last() else {
                    continue;
                };
                if !constraints_bind_resource(&parent.constraints, resource) {
                    if diagnostics {
                        eprintln!(
                            "aithos_gateway_g4_diagnostic: context={context} parent={parent_id} rejected=resource expected={resource}"
                        );
                    }
                    continue;
                }
                let Ok(parent_perimeter) = parent.parsed_perimeter() else {
                    if diagnostics {
                        eprintln!(
                            "aithos_gateway_g4_diagnostic: context={context} parent={parent_id} rejected=perimeter"
                        );
                    }
                    continue;
                };
                let can_issue = parent_perimeter
                    .iter()
                    .any(|entry| matches!(entry, PerimeterEntry::Issue { depth } if *depth > 0));
                if !can_issue {
                    if diagnostics {
                        eprintln!(
                            "aithos_gateway_g4_diagnostic: context={context} parent={parent_id} rejected=issue"
                        );
                    }
                    continue;
                }
                let session_perimeter = parent_perimeter
                    .iter()
                    .filter(|entry| !matches!(entry, PerimeterEntry::Issue { .. }))
                    .map(PerimeterEntry::to_entry_string)
                    .collect();
                let Ok(public_chain) = chain
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<std::result::Result<Vec<_>, _>>()
                else {
                    continue;
                };
                eligible.push(EligibleSessionParent {
                    context: context.clone(),
                    parent_id: parent.id.clone(),
                    subject: parent.subject.clone(),
                    not_before: parent.not_before.clone(),
                    not_after: parent.not_after.clone(),
                    perimeter: parent.perimeter.clone(),
                    session_perimeter,
                    constraints: parent.constraints.clone(),
                    chain: public_chain,
                    did: did.clone(),
                    revocations: revocation_entries.clone(),
                });
            }
        }
        if diagnostics && eligible.is_empty() {
            eprintln!(
                "aithos_gateway_g4_diagnostic: signer={delegate_pub} resource={resource} eligible=0"
            );
        }
        eligible.sort_by(|left, right| {
            (&left.context, &left.parent_id).cmp(&(&right.context, &right.parent_id))
        });
        eligible
    }

    fn verified_session_chain(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<(Vec<Mandate>, DidDocument, Vec<Revocation>)> {
        let runtime = self.context(context)?;
        let did = runtime.bridge.did_doc()?;
        let entries = runtime.bridge.bundle.gamma_entries().map_err(bridge_err)?;
        let revocations = revocations(&entries);
        let gateway_pub = gateway_pub_multibase(&runtime.bridge.keyholder);
        // The OAuth session already pins the exact leaf id. Resolve that
        // immutable certificate and follow its parent links directly instead
        // of listing and downloading every certificate in the Ethos on every
        // MCP call. Live revocations are still read and checked below.
        const MAX_SESSION_CHAIN_DEPTH: usize = 64;
        let unavailable = || GatewayError::MandateDenied {
            op: "delegated_session".into(),
            reason: "the delegated session leaf is unavailable".into(),
        };
        let mut reversed = Vec::new();
        let mut seen = BTreeSet::new();
        let mut cursor = Some(leaf_id.to_owned());
        while let Some(id) = cursor {
            if reversed.len() >= MAX_SESSION_CHAIN_DEPTH || !seen.insert(id.clone()) {
                return Err(GatewayError::MandateDenied {
                    op: "delegated_session".into(),
                    reason: "the delegated session certificate chain is cyclic or too deep".into(),
                });
            }
            let mandate: Mandate =
                read_json(&runtime.bridge.bundle, &cert_path(&id)).map_err(|_| unavailable())?;
            if mandate.id != id {
                return Err(unavailable());
            }
            cursor = mandate.parent.clone();
            reversed.push(mandate);
        }
        reversed.reverse();
        let chain = reversed;
        if chain
            .last()
            .is_none_or(|leaf| leaf.id != leaf_id || leaf.grantee.pubkey != gateway_pub)
        {
            return Err(GatewayError::MandateDenied {
                op: "delegated_session".into(),
                reason: "the delegated session leaf authority is unavailable".into(),
            });
        }
        verify_chain_revocable(&chain, &did, now, &revocations).map_err(|error| {
            GatewayError::MandateDenied {
                op: "delegated_session".into(),
                reason: error.to_string(),
            }
        })?;
        let leaf = chain.last().expect("non-empty verified chain");
        if leaf
            .constraints
            .get("session_bind")
            .and_then(serde_json::Value::as_str)
            != Some(session_pub)
            || serde_json::to_value(leaf).map_err(bridge_err)? != *expected_leaf
        {
            return Err(GatewayError::MandateDenied {
                op: "delegated_session".into(),
                reason: "the live delegated leaf differs from the OAuth session".into(),
            });
        }
        Ok((chain, did, revocations))
    }

    pub fn validate_bearer_session(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<()> {
        self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)
            .map(|_| ())
    }

    fn session_tool_parts(&self, tool: &str) -> (String, String) {
        self.runtime_hub_tool(tool).map_or_else(
            || {
                (
                    crate::policy::MCP_CONNECTOR.to_owned(),
                    crate::policy::action_name(tool),
                )
            },
            |hub| {
                (
                    hub.server.clone(),
                    crate::policy::action_name(&hub.raw_tool),
                )
            },
        )
    }

    pub fn listed_tools_for_session(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let runtime = self.context(context)?;
        Ok(self
            .listed_tools()
            .into_iter()
            .filter(|descriptor| {
                let Some(tool) = descriptor.get("name").and_then(serde_json::Value::as_str) else {
                    return false;
                };
                if !self.tool_available_in_context(context, tool) {
                    return false;
                }
                let (connector, action) = self.session_tool_parts(tool);
                runtime
                    .bridge
                    .bundle
                    .action_covered(&chain, &connector, &action)
                    .unwrap_or(false)
            })
            .collect())
    }

    pub fn session_covers_tool(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        tool: &str,
        now: &str,
    ) -> Result<bool> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let runtime = self.context(context)?;
        let (connector, action) = self.session_tool_parts(tool);
        runtime
            .bridge
            .bundle
            .action_covered(&chain, &connector, &action)
            .map_err(bridge_err)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_session_operation(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        certificate: &serde_json::Value,
        tool: &str,
        args: &serde_json::Value,
        now: &str,
    ) -> Result<PreparedSessionOperation> {
        let (chain, did, revocations) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let (connector, action) = self.session_tool_parts(tool);
        let runtime = self.context_mut(context)?;
        if !runtime
            .bridge
            .bundle
            .action_covered(&chain, &connector, &action)
            .map_err(bridge_err)?
        {
            return Err(GatewayError::MandateDenied {
                op: hub_op_for_tool(&connector, &action),
                reason: format!("exposed tool `{tool}` is outside the delegated session"),
            });
        }
        let history_heads = runtime
            .bridge
            .bundle
            .gamma_entries()
            .map_err(bridge_err)?
            .last()
            .map(|entry| entry.chain_hash().map_err(bridge_err))
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>();
        let occurrence = format!(
            "op_{}",
            Sid(ulid::Ulid::from(u128::from_be_bytes(
                runtime.bridge.entropy.as_mut().e16()
            )))
        );
        let args_hash = hash_of(args)?;
        let facts = serde_json::json!({
            "v": 1,
            "context": context,
            "tool": tool,
            "connector": connector,
            "action": action,
            "args_hash": args_hash,
        });
        let facts_digest = commitment_of("aithos-core/v1/operation-facts", &facts)?;
        let mandate =
            serde_json::to_value(chain.last().expect("verified chain")).map_err(bridge_err)?;
        let certificate_digest = hash_of(certificate)?;
        let projection = serde_json::json!({
            "aithos-operation-core": "1.0.0-draft.1",
            "occurrence": occurrence,
            "subject": chain.last().expect("verified chain").subject,
            "at": now,
            "history_heads": history_heads,
            "authority": {
                "actor": "grantee",
                "key": chain.last().expect("verified chain").grantee.pubkey,
                "authorized_by": leaf_id,
                "authorized_via": [{
                    "id": leaf_id,
                    "certificate_digest": hash_of(&mandate)?,
                }],
                "session": {
                    "key": session_pub,
                    "certificate_digest": certificate_digest,
                },
            },
            "operation": {
                "kind": "action",
                "facts_ref": {
                    "aithos-operation-facts-core": "1.0.0-draft.1",
                    "digest": facts_digest,
                },
            },
        });
        let operation_ref = serde_json::json!({
            "aithos-operation-core": "1.0.0-draft.1",
            "occurrence": projection["occurrence"],
            "commitment": commitment_of("aithos-core/v1/operation-commitment", &projection)?,
        });
        let mut native_message = MCP_SESSION_NATIVE_PROOF_DOMAIN.to_vec();
        native_message.extend_from_slice(
            &aithos_core::jcs::canonical_bytes(&operation_ref).map_err(bridge_err)?,
        );
        let gateway = SigningKey::from_bytes(runtime.bridge.keyholder.gateway_seed());
        let native_leaf_proof = serde_json::json!({
            "key": chain.last().expect("verified chain").grantee.pubkey,
            "sig": hex::encode(gateway.sign(&native_message).to_bytes()),
        });
        Ok(PreparedSessionOperation {
            chain,
            did,
            revocations,
            mandate,
            certificate: certificate.clone(),
            certificate_digest,
            projection,
            operation_ref,
            native_leaf_proof,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn validated_session_leaf(
        &self,
        context: &str,
        parent_id: &str,
        delegate_pub: &str,
        gateway_pub: &str,
        gateway_kex_pub: &str,
        session_pub: &str,
        resource: &str,
        leaf_value: &serde_json::Value,
        now: &str,
    ) -> Result<(Vec<Mandate>, Mandate)> {
        let runtime = self.context(context)?;
        let doc = runtime.bridge.did_doc()?;
        let entries = runtime.bridge.bundle.gamma_entries().map_err(bridge_err)?;
        let revs = revocations(&entries);
        let parent_chain = runtime
            .bridge
            .walk_cert_chains(delegate_pub)
            .into_iter()
            .find(|chain| chain.last().is_some_and(|leaf| leaf.id == parent_id))
            .ok_or_else(|| GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "selected parent is not held by the delegate key".into(),
            })?;
        verify_chain_revocable(&parent_chain, &doc, now, &revs).map_err(|error| {
            GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: error.to_string(),
            }
        })?;
        let parent = parent_chain.last().expect("non-empty chain");
        if !constraints_bind_resource(&parent.constraints, resource) {
            return Err(GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "selected parent is bound to another gateway audience".into(),
            });
        }
        let parent_can_issue = parent.parsed_perimeter().is_ok_and(|entries| {
            entries
                .iter()
                .any(|entry| matches!(entry, PerimeterEntry::Issue { depth } if *depth > 0))
        });
        if !parent_can_issue {
            return Err(GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "selected parent grants no issue authority".into(),
            });
        }
        let leaf: Mandate = serde_json::from_value(leaf_value.clone()).map_err(|_| {
            GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "session leaf is malformed".into(),
            }
        })?;
        if leaf.parent.as_deref() != Some(parent_id)
            || leaf.issued_by != delegate_pub
            || leaf.grantee.pubkey != gateway_pub
            || leaf.grantee.kex_pubkey != gateway_kex_pub
        {
            return Err(GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "session leaf authority binding mismatch".into(),
            });
        }
        if leaf
            .constraints
            .get("session_bind")
            .and_then(serde_json::Value::as_str)
            != Some(session_pub)
        {
            return Err(GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "session leaf session_bind mismatch".into(),
            });
        }
        if leaf.parsed_perimeter().is_err()
            || leaf.parsed_perimeter().is_ok_and(|entries| {
                entries
                    .iter()
                    .any(|entry| matches!(entry, PerimeterEntry::Issue { .. }))
            })
        {
            return Err(GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "session leaf may not transmit issue authority".into(),
            });
        }
        let before = aithos_core::gamma::ts_epoch(&leaf.not_before).map_err(bridge_err)?;
        let after = aithos_core::gamma::ts_epoch(&leaf.not_after).map_err(bridge_err)?;
        if after <= before || after - before > 8 * 60 * 60 {
            return Err(GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "session leaf lifetime exceeds eight hours".into(),
            });
        }
        let mut session_chain = parent_chain.clone();
        session_chain.push(leaf.clone());
        verify_chain_revocable(&session_chain, &doc, now, &revs).map_err(|error| {
            GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: error.to_string(),
            }
        })?;
        Ok((parent_chain, leaf))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_session_grant(
        &mut self,
        context: &str,
        parent_id: &str,
        delegate_pub: &str,
        gateway_pub: &str,
        gateway_kex_pub: &str,
        session_pub: &str,
        resource: &str,
        leaf_value: &serde_json::Value,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (parent_chain, leaf) = self.validated_session_leaf(
            context,
            parent_id,
            delegate_pub,
            gateway_pub,
            gateway_kex_pub,
            session_pub,
            resource,
            leaf_value,
            now,
        )?;
        let runtime = self.context_mut(context)?;
        let entry = runtime
            .bridge
            .bundle
            .prepare_external_delegated_grant(&parent_chain, &leaf, runtime.bridge.entropy.as_mut())
            .map_err(bridge_err)?;
        serde_json::to_value(entry).map_err(bridge_err)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn activate_session_leaf(
        &mut self,
        context: &str,
        parent_id: &str,
        delegate_pub: &str,
        gateway_pub: &str,
        gateway_kex_pub: &str,
        session_pub: &str,
        resource: &str,
        leaf_value: &serde_json::Value,
        grant_value: &serde_json::Value,
        now: &str,
    ) -> Result<SessionAuthority> {
        let (mut parent_chain, leaf) = self.validated_session_leaf(
            context,
            parent_id,
            delegate_pub,
            gateway_pub,
            gateway_kex_pub,
            session_pub,
            resource,
            leaf_value,
            now,
        )?;
        let grant: aithos_core::gamma::Entry = serde_json::from_value(grant_value.clone())
            .map_err(|_| GatewayError::MandateDenied {
                op: "session.issue".into(),
                reason: "delegated Gamma grant is malformed".into(),
            })?;
        let runtime = self.context_mut(context)?;
        let doc = runtime.bridge.did_doc()?;
        let entries = runtime.bridge.bundle.gamma_entries().map_err(bridge_err)?;
        let revs = revocations(&entries);
        Bundle::<GatewayStore>::verify_external_delegated_grant(
            &grant,
            &parent_chain,
            &leaf,
            &doc,
            &revs,
            &entries,
        )
        .map_err(bridge_err)?;
        parent_chain.push(leaf.clone());

        let mut certificate = serde_json::json!({
            "aithos-session-core": "1.0.0-draft.1",
            "subject": leaf.subject.clone(),
            "mandate_id": leaf.id.clone(),
            "key": session_pub,
            "not_before": leaf.not_before.clone(),
            "not_after": leaf.not_after.clone(),
            "signature": {
                "alg": "ed25519",
                "key": gateway_pub,
                "value": "",
            },
        });
        let preimage = serde_jcs::to_vec(&certificate).map_err(bridge_err)?;
        let gateway_signing = SigningKey::from_bytes(runtime.bridge.keyholder.gateway_seed());
        use ed25519_dalek::Signer as _;
        certificate["signature"]["value"] =
            serde_json::Value::String(hex::encode(gateway_signing.sign(&preimage).to_bytes()));
        runtime
            .bridge
            .bundle
            .store
            .put(
                &cert_path(&leaf.id),
                &serde_json::to_vec_pretty(&leaf).map_err(bridge_err)?,
            )
            .map_err(bridge_err)?;
        runtime
            .bridge
            .bundle
            .append_external_delegated_grant(&parent_chain[..parent_chain.len() - 1], &leaf, &grant)
            .map_err(bridge_err)?;
        let public_chain = parent_chain
            .iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(bridge_err)?;
        Ok(SessionAuthority {
            context: context.to_owned(),
            subject: leaf.subject.clone(),
            parent_id: parent_id.to_owned(),
            leaf_id: leaf.id.clone(),
            not_before: leaf.not_before.clone(),
            not_after: leaf.not_after.clone(),
            session_pub: session_pub.to_owned(),
            chain: public_chain,
            leaf: serde_json::to_value(&leaf).map_err(bridge_err)?,
            certificate,
        })
    }

    /// Open every context and the journal declared by a multi-context
    /// config (the binary's `run` path). The runner identity is shared:
    /// one keyholder, N bridges. Entropy stays injected — the factory is
    /// called once per bridge.
    pub fn open(
        cfg: &GatewayConfig,
        keyholder: Keyholder,
        entropy: impl FnMut() -> Box<dyn EntropySource + Send>,
    ) -> Result<Self> {
        Self::open_shared(cfg, Arc::new(keyholder), entropy)
    }

    /// Open a runner while the binary retains a clone of the same bounded
    /// identity for G1 registration and B.5. This does not expose a signer:
    /// every external signature still passes through a fixed bridge method.
    pub fn open_shared(
        cfg: &GatewayConfig,
        keyholder: Arc<Keyholder>,
        mut entropy: impl FnMut() -> Box<dyn EntropySource + Send>,
    ) -> Result<Self> {
        let contexts_cfg = cfg.contexts.as_ref().ok_or_else(|| {
            GatewayError::ConfigRejected("the multi-context runner needs `contexts`".into())
        })?;
        let journal_cfg = cfg.journal.as_ref().ok_or_else(|| {
            GatewayError::ConfigRejected("the multi-context runner needs `journal`".into())
        })?;
        let mut contexts = BTreeMap::new();
        let mut hub_tools = BTreeMap::new();
        let mut hub_server_pins: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for ctx in contexts_cfg {
            // P3: remote/replicated stores sign their envelopes with the
            // runner identity (arbitrage ② — the keyholder seam).
            let store =
                GatewayStore::from_config_with_identity(&ctx.store, &keyholder, &mut entropy)?;
            let bridge = Bridge::open(store, Arc::clone(&keyholder), entropy())?;
            let policy = match &ctx.tools {
                ContextTools::Legacy(tools) => Policy::new(tools.clone()),
                ContextTools::Hub(refs) => {
                    let mut policy_map = BTreeMap::new();
                    let mut manifests = BTreeMap::new();
                    for (exposed, reference) in refs {
                        if !manifests.contains_key(&reference.server) {
                            manifests.insert(
                                reference.server.clone(),
                                bridge.read_hub_manifest(&reference.server)?,
                            );
                        }
                        let manifest = manifests.get(&reference.server).expect("inserted above");
                        merge_server_pins(&mut hub_server_pins, manifest)?;
                        let approved = manifest
                            .tools
                            .iter()
                            .find(|tool| tool.name == reference.tool)
                            .ok_or_else(|| {
                                GatewayError::ConfigRejected(format!(
                                    "context `{}` references `{}/{}` absent from its approved manifest",
                                    ctx.name, reference.server, reference.tool
                                ))
                            })?;
                        validate_runtime_tool(&ctx.name, exposed, reference, approved)?;
                        policy_map.insert(exposed.clone(), reference.access);
                        hub_tools.insert(
                            exposed.clone(),
                            HubRuntimeTool {
                                context: ctx.name.clone(),
                                server: reference.server.clone(),
                                raw_tool: reference.tool.clone(),
                                description: approved.description.clone(),
                                input_schema: approved.input_schema.clone(),
                                pin_sha256: approved.pin_sha256.clone(),
                                access: reference.access,
                                granted: approved.is_granted(),
                                bounds: approved.bounds.clone(),
                            },
                        );
                    }
                    Policy::new(policy_map)
                }
            };
            contexts.insert(ctx.name.clone(), ContextRuntime { policy, bridge });
        }
        let journal_store =
            GatewayStore::from_config_with_identity(&journal_cfg.store, &keyholder, &mut entropy)?;
        let journal = Bridge::open(journal_store, keyholder, entropy())?;
        Ok(Self {
            contexts,
            journal,
            hub_tools,
            hub_server_pins,
            hub_drift: BTreeMap::new(),
            hot_tools: BTreeMap::new(),
            hot_tool_contexts: BTreeMap::new(),
            hot_server_pins: BTreeMap::new(),
        })
    }

    fn runtime_hub_tool(&self, tool: &str) -> Option<&HubRuntimeTool> {
        self.hot_tools
            .get(tool)
            .or_else(|| self.hub_tools.get(tool))
    }

    /// Resolve and verify one already-sealed connector approval in exactly
    /// the context named by the signed control principal.
    pub fn approved_connector(
        &self,
        context: &str,
        connector: &str,
    ) -> Result<(ApprovedManifest, String)> {
        let manifest = self
            .context(context)?
            .bridge
            .verified_hub_manifest(connector)?;
        let digest = approved_manifest_catalog_digest(&manifest)?;
        Ok((manifest, digest))
    }

    /// Make the latest Provider-published binding visible in a replicated
    /// context before the normal sealed-manifest verifier is consulted.
    pub fn refresh_approved_connector(&self, context: &str, connector: &str) -> Result<()> {
        self.context(context)?
            .bridge
            .refresh_hub_manifest(connector)
    }

    /// Validate a complete hot runtime view without mutating it. The
    /// caller persists the complete registry only after this passes.
    pub fn validate_hot_connector(
        &self,
        context: &str,
        connector: &str,
        manifest: &ApprovedManifest,
    ) -> Result<()> {
        self.context(context)?;
        validate_approved(manifest)?;
        if manifest.server != connector {
            return Err(GatewayError::ConfigRejected(
                "approved connector id does not match its sealed manifest".into(),
            ));
        }
        if self.hub_server_pins.contains_key(connector) {
            return Err(GatewayError::ConfigRejected(format!(
                "connector `{connector}` is already statically routed"
            )));
        }
        for tool in &manifest.tools {
            if let Some(existing) = self.hub_tools.get(&tool.exposed_name) {
                return Err(GatewayError::ConfigRejected(format!(
                    "hot connector tool `{}` collides with static route `{}/{}`",
                    tool.exposed_name, existing.server, existing.raw_tool
                )));
            }
            if let Some(existing) = self.hot_tools.get(&tool.exposed_name) {
                if existing.server != connector {
                    return Err(GatewayError::ConfigRejected(format!(
                        "hot connector tool `{}` collides with connector `{}`",
                        tool.exposed_name, existing.server
                    )));
                }
            }
        }
        Ok(())
    }

    /// Replace one connector's in-memory tool/pin view while the caller
    /// holds the runner mutex. Every descriptor comes from the sealed
    /// approval; live discovery contributes only a comparison verdict.
    pub fn install_hot_connector(
        &mut self,
        context: &str,
        connector: &str,
        manifest: &ApprovedManifest,
    ) -> Result<()> {
        self.clear_hot_connector_routes(connector);
        self.install_hot_connector_context(context, connector, manifest)
    }

    fn install_hot_connector_context(
        &mut self,
        context: &str,
        connector: &str,
        manifest: &ApprovedManifest,
    ) -> Result<()> {
        self.validate_hot_connector(context, connector, manifest)?;
        for approved in &manifest.tools {
            self.hot_tools
                .entry(approved.exposed_name.clone())
                .or_insert_with(|| HubRuntimeTool {
                    context: context.to_owned(),
                    server: connector.to_owned(),
                    raw_tool: approved.name.clone(),
                    description: approved.description.clone(),
                    input_schema: approved.input_schema.clone(),
                    pin_sha256: approved.pin_sha256.clone(),
                    access: approved.risk_class,
                    granted: approved.is_granted(),
                    bounds: approved.bounds.clone(),
                });
            self.hot_tool_contexts
                .entry(approved.exposed_name.clone())
                .or_default()
                .insert(context.to_owned());
        }
        self.hot_server_pins.insert(
            connector.to_owned(),
            manifest
                .tools
                .iter()
                .map(|tool| (tool.name.clone(), tool.pin_sha256.clone()))
                .collect(),
        );
        self.clear_manifest_drift(connector);
        Ok(())
    }

    /// Rebuild one hot connector's context projection from the canonical
    /// bindings currently present in every loaded Ethos. The control
    /// context that activated the connector is required; other contexts are
    /// added only when their independently verified binding has the exact
    /// same catalogue digest.
    pub fn reconcile_bound_hot_connector(
        &mut self,
        required_context: &str,
        connector: &str,
        required_manifest: &ApprovedManifest,
        expected_digest: &str,
    ) -> Result<usize> {
        self.validate_hot_connector(required_context, connector, required_manifest)?;
        self.clear_hot_connector_routes(connector);
        self.install_hot_connector_context(required_context, connector, required_manifest)?;
        let mut installed = 1usize;
        let contexts = self.contexts.keys().cloned().collect::<Vec<_>>();
        for context in contexts {
            if context == required_context {
                continue;
            }
            if self
                .refresh_approved_connector(&context, connector)
                .is_err()
            {
                continue;
            }
            let Ok((manifest, digest)) = self.approved_connector(&context, connector) else {
                continue;
            };
            if digest != expected_digest {
                continue;
            }
            self.install_hot_connector_context(&context, connector, &manifest)?;
            installed += 1;
        }
        Ok(installed)
    }

    fn clear_hot_connector_routes(&mut self, connector: &str) {
        let names = self
            .hot_tools
            .iter()
            .filter(|(_, tool)| tool.server == connector)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in names {
            self.hot_tools.remove(&name);
            self.hot_tool_contexts.remove(&name);
        }
        self.hot_server_pins.remove(connector);
    }

    pub fn remove_hot_connector(&mut self, connector: &str) {
        self.clear_hot_connector_routes(connector);
        self.hub_drift.remove(connector);
    }

    /// Log-before-sidecar for stage/secret/OAuth/activation lifecycle
    /// changes. Details are finite non-secret labels only.
    pub fn record_connector_config(
        &mut self,
        context: &str,
        connector: &str,
        event: &str,
        now: &str,
    ) -> Result<String> {
        self.journal
            .record_connector_config(context, connector, event, now)
    }

    pub fn record_connector_effect(
        &mut self,
        context: &str,
        connector: &str,
        effect: &ConnectorEffectProof<'_>,
        now: &str,
    ) -> Result<String> {
        self.journal
            .record_connector_effect(context, connector, effect, now)
    }

    /// The OAuth authority ceiling (lot G3): the latest `not_after`
    /// among the runner's valid agent chains at `now`, or `None` when no
    /// context grants the agent a live authority. This is the injectable
    /// binding the AS caps tokens by pre-G4 — a session lives while some
    /// authority backs it; each ACT stays gated per context by the
    /// existing pipeline. G4/G5 swap this for the session sub-mandate's
    /// `not_after` without touching the AS.
    pub fn agent_authority_ceiling(&self, now: &str) -> Option<String> {
        self.contexts
            .values()
            .filter_map(|c| c.bridge.agent_authority_ceiling(now))
            .max()
    }

    /// The context whose tool map names this tool (read or write).
    /// Unambiguous by construction: config v2 rejects cross-context
    /// collisions. Unknown everywhere → `None` (default-deny).
    pub fn resolve(&self, tool: &str) -> Option<&str> {
        if let Some(runtime) = self.hot_tools.get(tool) {
            return Some(runtime.context.as_str());
        }
        self.contexts
            .iter()
            .find(|(_, c)| c.policy.is_mapped(tool))
            .map(|(name, _)| name.as_str())
    }

    /// Whether a tool is actually mapped inside one specific Ethos context.
    /// This is the delegated-session resolver: unlike the legacy aggregate
    /// resolver it can represent the same hot connector in several Ethos.
    pub fn tool_available_in_context(&self, context: &str, tool: &str) -> bool {
        if self
            .hot_tool_contexts
            .get(tool)
            .is_some_and(|contexts| contexts.contains(context))
        {
            return true;
        }
        if self
            .hub_tools
            .get(tool)
            .is_some_and(|hub| hub.context == context)
        {
            return true;
        }
        self.contexts
            .get(context)
            .is_some_and(|runtime| runtime.policy.is_mapped(tool))
    }

    /// Every mapped tool name across all contexts (deterministic order) —
    /// what the router's aggregated `tools/list` advertises.
    pub fn mapped_tools(&self) -> Vec<String> {
        self.contexts
            .values()
            .flat_map(|c| c.policy.tools().map(str::to_owned))
            .collect()
    }

    /// Agent-visible descriptors. Hub mode exposes GRANTED pins only
    /// (lot W: reads and writes alike — the decision, not the class);
    /// legacy mode preserves the v2 names-only surface.
    pub fn listed_tools(&self) -> Vec<serde_json::Value> {
        if self.hub_tools.is_empty() && self.hot_tools.is_empty() {
            return self
                .mapped_tools()
                .into_iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "inputSchema": { "type": "object" }
                    })
                })
                .collect();
        }
        self.hub_tools
            .iter()
            .chain(self.hot_tools.iter())
            .filter(|(_, tool)| tool.granted)
            .map(|(name, tool)| {
                let mut descriptor = serde_json::json!({
                    "name": name,
                    "inputSchema": tool.input_schema
                });
                if let Some(description) = &tool.description {
                    descriptor["description"] = serde_json::Value::String(description.clone());
                }
                descriptor
            })
            .collect()
    }

    pub fn relay_target(&self, ctx: &str, tool: &str) -> Result<HubRelayTarget> {
        if let Some(hub) = self.hot_tools.get(tool) {
            if !self
                .hot_tool_contexts
                .get(tool)
                .is_some_and(|contexts| contexts.contains(ctx))
            {
                return Err(GatewayError::ConfigRejected(format!(
                    "hot route `{tool}` has no owner binding in context `{ctx}`"
                )));
            }
            return Ok(HubRelayTarget {
                server: hub.server.clone(),
                raw_tool: hub.raw_tool.clone(),
                hot: true,
            });
        }
        if let Some(hub) = self.hub_tools.get(tool) {
            if hub.context != ctx {
                return Err(GatewayError::ConfigRejected(format!(
                    "hub route `{tool}` resolved to `{ctx}`, pin belongs to `{}`",
                    hub.context
                )));
            }
            return Ok(HubRelayTarget {
                server: hub.server.clone(),
                raw_tool: hub.raw_tool.clone(),
                hot: false,
            });
        }
        Ok(HubRelayTarget {
            server: ctx.to_owned(),
            raw_tool: tool.to_owned(),
            hot: false,
        })
    }

    pub fn is_hot_server(&self, server: &str) -> bool {
        self.hot_server_pins.contains_key(server)
    }

    pub fn server_pins(&self, server: &str) -> Option<BTreeMap<String, String>> {
        self.hot_server_pins
            .get(server)
            .or_else(|| self.hub_server_pins.get(server))
            .cloned()
    }

    pub fn hub_servers(&self) -> Vec<String> {
        self.hub_server_pins
            .keys()
            .chain(self.hot_server_pins.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn mark_manifest_drift(&mut self, server: &str, reason: String) {
        self.hub_drift.insert(server.to_owned(), reason);
    }

    pub fn clear_manifest_drift(&mut self, server: &str) {
        self.hub_drift.remove(server);
    }

    pub fn manifest_drift_for(&self, tool: &str) -> Option<GatewayError> {
        let hub = self.runtime_hub_tool(tool)?;
        match manifest_tool_pin(&hub.raw_tool, hub.description.as_deref(), &hub.input_schema) {
            Ok(pin) if pin == hub.pin_sha256 => {}
            Ok(_) => {
                return Some(GatewayError::ManifestDrift {
                    server: hub.server.clone(),
                    reason: format!("local pin for `{}` is inconsistent", hub.raw_tool),
                })
            }
            Err(error) => {
                return Some(GatewayError::ManifestDrift {
                    server: hub.server.clone(),
                    reason: error.to_string(),
                })
            }
        }
        self.hub_drift
            .get(&hub.server)
            .map(|reason| GatewayError::ManifestDrift {
                server: hub.server.clone(),
                reason: reason.clone(),
            })
    }

    /// The owner-approved argument bounds of one exposed tool (lot P):
    /// checked by the router AFTER the mandate said yes and BEFORE the
    /// act is logged. A violation refuses the whole call — the gateway
    /// never rewrites arguments. Non-hub tools carry no bounds.
    pub fn check_bounds(&self, tool: &str, args: &serde_json::Value) -> Result<()> {
        let Some(hub) = self.runtime_hub_tool(tool) else {
            return Ok(());
        };
        for bound in &hub.bounds {
            // Shape first, from the PINNED schema: a bounded field the
            // approved schema types as an array must arrive as one —
            // never coerced, never guessed (the `action`-style scalar
            // whitelists stay scalars because their schema says so).
            let field = bound.field();
            let pinned_type = hub
                .input_schema
                .pointer(&format!("/properties/{field}/type"))
                .and_then(serde_json::Value::as_str);
            if pinned_type == Some("array") {
                if let Some(value) = args.get(field) {
                    if !value.is_array() {
                        return Err(GatewayError::BoundViolated(format!(
                            "`{}.{field}` — must be an array of strings per the pinned schema",
                            hub.raw_tool
                        )));
                    }
                }
            }
            bound.check(&hub.raw_tool, args)?;
        }
        Ok(())
    }

    /// Pre-check on the resolved context: does its mandate cover the
    /// tool at `now`? (`record_act_with_xref` re-verifies at append.)
    pub fn authorize(&self, ctx: &str, tool: &str, now: &str) -> Result<()> {
        if let Some(hub) = self.runtime_hub_tool(tool) {
            return self
                .context(ctx)?
                .bridge
                .authorize_hub(&hub.server, &hub.raw_tool, now)
                .map_err(|error| match error {
                    GatewayError::MandateDenied { op, reason } => GatewayError::MandateDenied {
                        op,
                        reason: format!("exposed tool `{tool}`: {reason}"),
                    },
                    other => other,
                });
        }
        self.context(ctx)?.bridge.authorize(tool, now)
    }

    /// Log-before-relay, twice: the authoritative act in the context
    /// gamma, then its xref mirror in the journal. Any failed append
    /// refuses the call — an act that cannot be indexed does not happen
    /// (the already-appended context entry stands, append-only; the
    /// caller's refusal then closes the story). Returns the CONTEXT
    /// entry id (the proof).
    pub fn record_act_with_xref(
        &mut self,
        ctx: &str,
        tool: &str,
        args: &serde_json::Value,
        now: &str,
    ) -> Result<String> {
        let hub = self.runtime_hub_tool(tool).cloned();
        let context = self.context_mut(ctx)?;
        let entry_id = match hub {
            Some(hub) => {
                context
                    .bridge
                    .record_hub_act(tool, &hub.server, &hub.raw_tool, args, now)?
            }
            None => context.bridge.record_act(tool, args, now)?,
        };
        let ethos_did = context.bridge.ethos_did().to_owned();
        self.journal.record_xref(tool, &ethos_did, &entry_id, now)?;
        Ok(entry_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_session_act_with_xref(
        &mut self,
        ctx: &str,
        tool: &str,
        args: &serde_json::Value,
        chain: &[Mandate],
        session_pub: &str,
        certificate_digest: &str,
        operation_ref: &serde_json::Value,
        now: &str,
    ) -> Result<String> {
        let hub = self.runtime_hub_tool(tool).cloned();
        let context = self.context_mut(ctx)?;
        let (connector, action, mut detail) = match hub {
            Some(hub) => (
                hub.server.clone(),
                crate::policy::action_name(&hub.raw_tool),
                serde_json::json!({
                    "tool": tool,
                    "server": hub.server,
                    "upstream_tool": hub.raw_tool,
                }),
            ),
            None => (
                crate::policy::MCP_CONNECTOR.to_owned(),
                crate::policy::action_name(tool),
                serde_json::json!({ "tool": tool }),
            ),
        };
        detail["session"] = serde_json::json!({
            "key": session_pub,
            "mandate_id": chain.last().map(|leaf| leaf.id.as_str()),
            "certificate_digest": certificate_digest,
        });
        detail["operation_ref"] = operation_ref.clone();
        let gateway = SigningKey::from_bytes(context.bridge.keyholder.gateway_seed());
        let args_hash = hash_of(args)?;
        let entry = context
            .bridge
            .bundle
            .log_action(
                chain,
                &gateway,
                &ActionSpec {
                    connector: &connector,
                    action: &action,
                    args_hash: &args_hash,
                    now,
                    budget: Some(detail),
                    sealed_args: None,
                },
                context.bridge.entropy.as_mut(),
            )
            .map_err(|error| GatewayError::LogAppendRefused(error.to_string()))?;
        let entry_id = entry.id;
        let ethos_did = context.bridge.ethos_did().to_owned();
        self.journal.record_xref(tool, &ethos_did, &entry_id, now)?;
        Ok(entry_id)
    }

    /// Pre-check of the journal's inference tap (`proxy_llm`, before the
    /// provider is touched). See [`Bridge::inference_headroom`].
    pub fn inference_headroom(&self, now: &str) -> Result<()> {
        self.journal.inference_headroom(now)
    }

    /// One metered `inference` entry in the agent's journal — the
    /// per-call record of the agent's own life with the LLM. See
    /// [`Bridge::record_inference`].
    pub fn record_inference(
        &mut self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        now: &str,
    ) -> Result<String> {
        self.journal
            .record_inference(provider, model, tokens_in, tokens_out, now)
    }

    /// Consolidate one memory note into the agent's journal (lot C2).
    /// See [`Bridge::journal_write`].
    pub fn journal_write(
        &mut self,
        title: &str,
        tags: &[String],
        text: &str,
        now: &str,
    ) -> Result<NoteView> {
        self.journal.journal_write(title, tags, text, now)
    }

    /// Recall memory notes from the agent's journal (lot C2). See
    /// [`Bridge::journal_search`].
    pub fn journal_search(
        &mut self,
        query: Option<&str>,
        tag: Option<&str>,
        limit: usize,
        now: &str,
    ) -> Result<Vec<NoteView>> {
        self.journal.journal_search(query, tag, limit, now)
    }

    /// Is there anything to brief anywhere? The conditional-surface
    /// probe (lot K): true when at least one context holds a granted,
    /// non-empty briefing zone. Recomputed on every `initialize` and
    /// `tools/list` — a hot owner edit flips the surface without any
    /// restart. Index-only, no journal entry.
    pub fn briefing_available(&self) -> bool {
        self.contexts
            .values()
            .any(|context| context.bridge.briefing_available())
    }

    pub fn briefing_available_for(&self, context: &str) -> bool {
        self.contexts
            .get(context)
            .is_some_and(|runtime| runtime.bridge.briefing_available())
    }

    /// Serve the owner's directives across the granted contexts —
    /// `context: None` serves them all, labeled by context name, the
    /// zone named on each directive; a named context serves that one
    /// only. Every served section is one journalized read in ITS
    /// context's gamma (see [`Bridge::briefing_read`]). Nothing to say
    /// anywhere → the call refuses: a mute surface has no callable tool.
    pub fn briefing_read(&mut self, context: Option<&str>, now: &str) -> Result<serde_json::Value> {
        let names: Vec<String> = match context {
            Some(name) => {
                if !self.contexts.contains_key(name) {
                    return Err(GatewayError::RequestRejected(format!(
                        "briefing.read: unknown context `{name}`"
                    )));
                }
                vec![name.to_owned()]
            }
            None => self.contexts.keys().cloned().collect(),
        };
        let mut served = Vec::new();
        for name in names {
            let bridge = &mut self.context_mut(&name)?.bridge;
            if !bridge.briefing_available() {
                continue;
            }
            let items = bridge.briefing_read(now)?;
            if !items.is_empty() {
                served.push(serde_json::json!({
                    "context": name,
                    "directives": items,
                }));
            }
        }
        if served.is_empty() {
            return Err(GatewayError::RequestRejected(
                "briefing.read: no directives in any granted briefing zone".into(),
            ));
        }
        Ok(serde_json::json!({ "contexts": served }))
    }

    /// The ethos surface per context (lot G6): context name → served
    /// zones; an empty map is a mute surface. Recomputed on every
    /// initialize and tools/list — a fresh grant lights it and a
    /// revocation drops it, hot, no restart.
    pub fn ethos_surface(&self, now: &str) -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        for (name, context) in &self.contexts {
            let zones = context.bridge.ethos_surface(now);
            if !zones.is_empty() {
                out.insert(name.clone(), zones);
            }
        }
        out
    }

    /// The covered skeleton across contexts (`context: None` = all).
    /// See [`Bridge::ethos_list`]. Mute contexts simply do not appear.
    pub fn ethos_list(&self, context: Option<&str>, now: &str) -> Result<serde_json::Value> {
        let names = self.ethos_target_contexts(context)?;
        let mut served = Vec::new();
        for name in names {
            let entries = self.context(&name)?.bridge.ethos_list(now);
            if !entries.is_empty() {
                served.push(serde_json::json!({ "context": name, "entries": entries }));
            }
        }
        Ok(serde_json::json!({ "contexts": served }))
    }

    /// One section body under the covering chain of ITS context. See
    /// [`Bridge::ethos_read_section`].
    pub fn ethos_read(
        &mut self,
        context: Option<&str>,
        zone: &str,
        path: &str,
        now: &str,
    ) -> Result<serde_json::Value> {
        let name = self.ethos_single_context(context)?;
        let text = self
            .context_mut(&name)?
            .bridge
            .ethos_read_section(zone, path, now)?;
        Ok(serde_json::json!({
            "context": name, "zone": zone, "path": path, "text": text,
        }))
    }

    /// The starting pack across contexts (`context: None` = all). See
    /// [`Bridge::ethos_context_pack`].
    pub fn ethos_context_pack(
        &mut self,
        context: Option<&str>,
        now: &str,
    ) -> Result<serde_json::Value> {
        let names = self.ethos_target_contexts(context)?;
        let mut served = Vec::new();
        for name in names {
            let pack = self.context_mut(&name)?.bridge.ethos_context_pack(now)?;
            served.push(serde_json::json!({ "context": name, "pack": pack }));
        }
        Ok(serde_json::json!({ "contexts": served }))
    }

    // ------------------------------------ delegated ethos surface (lot 1)

    /// The zones one delegated session serves in ITS context — never
    /// another context, never a frontier default. Chain re-verified
    /// fresh (revocations included): a revocation drops the surface on
    /// the very next call, no restart.
    pub fn ethos_surface_for_session(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<Vec<String>> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        Ok(self
            .context(context)?
            .bridge
            .ethos_surface_for_chain(&chain, now))
    }

    /// The covered skeleton for one delegated session — same response
    /// shape as [`Runner::ethos_list`], restricted to the session's own
    /// context and chain.
    pub fn ethos_list_for_session(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let entries = self
            .context(context)?
            .bridge
            .ethos_list_for_chain(&chain, now);
        let mut served = Vec::new();
        if !entries.is_empty() {
            served.push(serde_json::json!({ "context": context, "entries": entries }));
        }
        Ok(serde_json::json!({ "contexts": served }))
    }

    /// One section body under the session chain — same response shape
    /// as [`Runner::ethos_read`], the session's own context imposed.
    #[allow(clippy::too_many_arguments)]
    pub fn ethos_read_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        path: &str,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let bridge = &mut self.context_mut(context)?.bridge;
        let text = bridge.ethos_read_section_for_chain(&chain, zone, path, now)?;
        // The row digest rides along (D8): an `ethos.edit` cites it as
        // its `expected_digest` so concurrent changes refuse cleanly.
        let digest = match zone {
            "public" => bridge.ethos_section_digest(Zone::Public, path).ok(),
            "circle" => bridge.ethos_section_digest(Zone::Circle, path).ok(),
            _ => None,
        };
        Ok(serde_json::json!({
            "context": context, "zone": zone, "path": path, "text": text,
            "digest": digest,
        }))
    }

    /// Side-effect-free `aithos-client` observation used by the shadow gate.
    /// It performs its own cold verification, session proof, chain
    /// verification, authorization and content opening, but writes no Gamma
    /// entry and therefore cannot replace the serving path yet.
    #[allow(clippy::too_many_arguments)]
    pub fn ethos_client_read_probe_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        path: &str,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let zone = Zone::parse(zone).map_err(bridge_err)?;
        let runtime = self.context_mut(context)?;
        let snapshot = runtime.bridge.ethos_client_snapshot_for_chain(&chain)?;
        let nonce = runtime.bridge.entropy.e32();
        let keyholder = Arc::clone(&runtime.bridge.keyholder);
        let read = keyholder
            .with_ethos_client_session(snapshot, chain, now, nonce, |session| {
                session.read_content(
                    zone,
                    path,
                    aithos_client::AuthorizationContext::new(now),
                    aithos_client::ReadLimits::default(),
                )
            })
            .map_err(|error| {
                GatewayError::BridgeFailed(format!("aithos-client read refused: {error}"))
            })?;
        Ok(serde_json::json!({
            "context": context,
            "zone": zone.as_str(),
            "path": read.item().path(),
            "text": read.body(),
            "edition_height": read.edition_height(),
            "edition_chain_hash": read.edition_chain_hash(),
        }))
    }

    /// Side-effect-free `aithos-client` list observation for the same shadow
    /// comparison as [`Runner::ethos_client_read_probe_for_session`].
    pub fn ethos_client_list_probe_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let runtime = self.context_mut(context)?;
        let zones = runtime.bridge.ethos_surface_for_chain(&chain, now);
        let snapshot = runtime.bridge.ethos_client_snapshot_for_chain(&chain)?;
        let nonce = runtime.bridge.entropy.e32();
        let keyholder = Arc::clone(&runtime.bridge.keyholder);
        let entries = keyholder
            .with_ethos_client_session(snapshot, chain, now, nonce, |session| {
                let mut entries = Vec::new();
                for zone in &zones {
                    let zone = Zone::parse(zone)
                        .map_err(|_| aithos_client::ClientError::ZoneNotAllowed)?;
                    for item in session.list_content(
                        zone,
                        aithos_client::AuthorizationContext::new(now),
                        aithos_client::ReadLimits::default(),
                    )? {
                        entries.push(serde_json::json!({
                            "zone": zone.as_str(),
                            "path": item.path(),
                            "kind": match item.kind() {
                                aithos_client::ContentKind::Folder => "folder",
                                aithos_client::ContentKind::Section => "section",
                            },
                            "title": item.title(),
                            "tags": item.tags(),
                        }));
                    }
                }
                Ok(entries)
            })
            .map_err(|error| {
                GatewayError::BridgeFailed(format!("aithos-client list refused: {error}"))
            })?;
        Ok(serde_json::json!({ "context": context, "entries": entries }))
    }

    /// The starting pack for one delegated session — same response
    /// shape as [`Runner::ethos_context_pack`], restricted to the
    /// session's own context and chain, directives excluded (the
    /// briefing keeps its own explicitly-mandated tool).
    pub fn ethos_context_pack_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let pack = self
            .context_mut(context)?
            .bridge
            .ethos_context_pack_for_chain(&chain, now)?;
        Ok(serde_json::json!({
            "contexts": [ { "context": context, "pack": pack } ],
        }))
    }

    /// The delegated write surface of one session: `(create/edit,
    /// delete)` on the session's own context, chain re-verified fresh.
    pub fn ethos_write_surface_for_session(
        &self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        now: &str,
    ) -> Result<(bool, bool)> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        Ok(self
            .context(context)?
            .bridge
            .ethos_write_surface_for_chain(&chain, now))
    }

    /// Delegated `ethos.create` — returns the created section's digest.
    #[allow(clippy::too_many_arguments)]
    pub fn ethos_create_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        folder: &str,
        name: &str,
        title: &str,
        tags: &[String],
        body: &str,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let digest = self
            .context_mut(context)?
            .bridge
            .ethos_create_for_chain(&chain, zone, folder, name, title, tags, body, now)?;
        let path = if folder.is_empty() {
            name.to_owned()
        } else {
            format!("{folder}/{name}")
        };
        Ok(serde_json::json!({
            "context": context, "zone": zone, "path": path, "digest": digest,
        }))
    }

    /// Delegated `ethos.edit` — the required `expected_digest` is the D8
    /// concurrency precondition; returns the new digest.
    #[allow(clippy::too_many_arguments)]
    pub fn ethos_edit_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        path: &str,
        body: &str,
        expected_digest: &str,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let digest = self.context_mut(context)?.bridge.ethos_edit_for_chain(
            &chain,
            zone,
            path,
            body,
            expected_digest,
            now,
        )?;
        Ok(serde_json::json!({
            "context": context, "zone": zone, "path": path, "digest": digest,
        }))
    }

    /// Delegated `ethos.delete` — `expected_digest` enforced when given.
    #[allow(clippy::too_many_arguments)]
    pub fn ethos_delete_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        path: &str,
        expected_digest: Option<&str>,
        now: &str,
    ) -> Result<serde_json::Value> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        self.context_mut(context)?.bridge.ethos_delete_for_chain(
            &chain,
            zone,
            path,
            expected_digest,
            now,
        )?;
        Ok(serde_json::json!({
            "context": context, "zone": zone, "path": path, "deleted": true,
        }))
    }

    /// Prepare one operation-scoped `aithos-client` create without performing
    /// any Provider I/O. The caller may therefore release the Runner lock
    /// before executing the closed signed envelopes.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_ethos_client_create_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        folder: &str,
        name: &str,
        body: &str,
        now: &str,
    ) -> Result<crate::ethos_backend::PreparedEthosMutation> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let zone = Bridge::delegated_write_zone(zone, "ethos.create")?;
        if zone != Zone::Circle {
            return Err(GatewayError::MandateDenied {
                op: "ethos.create".to_owned(),
                reason: "the Client working-set canary is limited to circle".to_owned(),
            });
        }
        let path = if folder.is_empty() {
            name.to_owned()
        } else {
            format!("{folder}/{name}")
        };
        let intent = aithos_client::MutationIntent::Create {
            zone,
            path: path.clone(),
            body: body.to_owned(),
            at: now.to_owned(),
        };
        self.context_mut(context)?
            .bridge
            .prepare_ethos_client_mutation_for_chain(
                &chain,
                intent,
                context,
                zone.as_str(),
                &path,
                now,
            )
    }

    /// Prepare one operation-scoped edit. D8 is checked against the current
    /// clear row before planning; Provider CAS remains the final race guard.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_ethos_client_edit_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        path: &str,
        body: &str,
        expected_digest: &str,
        now: &str,
    ) -> Result<crate::ethos_backend::PreparedEthosMutation> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let zone = Bridge::delegated_write_zone(zone, "ethos.edit")?;
        if zone != Zone::Circle {
            return Err(GatewayError::MandateDenied {
                op: "ethos.edit".to_owned(),
                reason: "the Client working-set canary is limited to circle".to_owned(),
            });
        }
        let current = self
            .context(context)?
            .bridge
            .ethos_section_digest(zone, path)?;
        if current != expected_digest {
            return Err(GatewayError::MandateDenied {
                op: "ethos.edit".to_owned(),
                reason: format!(
                    "stale precondition: the section changed since it was read (current digest `{current}`)"
                ),
            });
        }
        let intent = aithos_client::MutationIntent::Edit {
            zone,
            path: path.to_owned(),
            body: body.to_owned(),
            at: now.to_owned(),
        };
        self.context_mut(context)?
            .bridge
            .prepare_ethos_client_mutation_for_chain(
                &chain,
                intent,
                context,
                zone.as_str(),
                path,
                now,
            )
    }

    /// Prepare one operation-scoped delete. The optional D8 digest retains
    /// the historical API; Provider CAS still refuses concurrent editions.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_ethos_client_delete_for_session(
        &mut self,
        context: &str,
        leaf_id: &str,
        session_pub: &str,
        expected_leaf: &serde_json::Value,
        zone: &str,
        path: &str,
        expected_digest: Option<&str>,
        now: &str,
    ) -> Result<crate::ethos_backend::PreparedEthosMutation> {
        let (chain, _, _) =
            self.verified_session_chain(context, leaf_id, session_pub, expected_leaf, now)?;
        let zone = Bridge::delegated_write_zone(zone, "ethos.delete")?;
        if zone != Zone::Circle {
            return Err(GatewayError::MandateDenied {
                op: "ethos.delete".to_owned(),
                reason: "the Client working-set canary is limited to circle".to_owned(),
            });
        }
        if let Some(expected) = expected_digest {
            let current = self
                .context(context)?
                .bridge
                .ethos_section_digest(zone, path)?;
            if current != expected {
                return Err(GatewayError::MandateDenied {
                    op: "ethos.delete".to_owned(),
                    reason: format!(
                        "stale precondition: the section changed since it was read (current digest `{current}`)"
                    ),
                });
            }
        }
        let intent = aithos_client::MutationIntent::Delete {
            zone,
            path: path.to_owned(),
            at: now.to_owned(),
        };
        self.context_mut(context)?
            .bridge
            .prepare_ethos_client_mutation_for_chain(
                &chain,
                intent,
                context,
                zone.as_str(),
                path,
                now,
            )
    }

    fn ethos_target_contexts(&self, context: Option<&str>) -> Result<Vec<String>> {
        match context {
            Some(name) => {
                if !self.contexts.contains_key(name) {
                    return Err(GatewayError::RequestRejected(format!(
                        "unknown context `{name}`"
                    )));
                }
                Ok(vec![name.to_owned()])
            }
            None => Ok(self.contexts.keys().cloned().collect()),
        }
    }

    /// ethos.read targets exactly one context: the named one, or the
    /// only one configured — an ambiguous call is refused naming the
    /// candidates (pedagogical, never a guess).
    fn ethos_single_context(&self, context: Option<&str>) -> Result<String> {
        match context {
            Some(name) => {
                if !self.contexts.contains_key(name) {
                    return Err(GatewayError::RequestRejected(format!(
                        "unknown context `{name}`"
                    )));
                }
                Ok(name.to_owned())
            }
            None => {
                if self.contexts.len() == 1 {
                    Ok(self.contexts.keys().next().expect("one context").clone())
                } else {
                    Err(GatewayError::RequestRejected(format!(
                        "ethos.read: name the context — one of {}",
                        self.contexts
                            .keys()
                            .map(|n| format!("`{n}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )))
                }
            }
        }
    }

    /// Refusal routing, decided §3bis.8: the journal gets EVERY refusal
    /// (it is the agent's story); the context gets it too when the tool
    /// maps to one (its auditor must see attempts against its
    /// perimeter). Best-effort appends: even a failed refusal log never
    /// un-refuses the call — the caller returns the error regardless.
    pub fn record_refusal(&mut self, ctx: Option<&str>, tool: &str, reason: &str, now: &str) {
        let _ = self.journal.record_refusal(tool, reason, now);
        if let Some(c) = ctx.and_then(|name| self.contexts.get_mut(name)) {
            let _ = c.bridge.record_refusal(tool, reason, now);
        }
    }

    /// The bound-violation refusal (lot D): same §3bis.8 routing, plus
    /// the pedagogical detail in the clear payload — field, offending
    /// values and approved rule, exactly what the agent was told and
    /// what beat 8 replays. Bound refusals only: that message is the
    /// owner's own sealed policy, structurally secret-free.
    pub fn record_bound_refusal(
        &mut self,
        ctx: Option<&str>,
        tool: &str,
        deny: &GatewayError,
        now: &str,
    ) {
        let code = deny.refusal_code();
        let detail = deny.to_string();
        let _ = self
            .journal
            .record_refusal_detailed(tool, code, &detail, now);
        if let Some(c) = ctx.and_then(|name| self.contexts.get_mut(name)) {
            let _ = c.bridge.record_refusal_detailed(tool, code, &detail, now);
        }
    }

    /// Journalize one OAuth token issuance (lot G3): a governance act of
    /// the gateway's own identity in the agent's journal — issuance is
    /// never silent (I5), and the entry names the client but carries NO
    /// token, code or secret. Best-effort like the refusal log: a failed
    /// append never un-mints a token already returned, but the store
    /// almost never refuses a governance act the gateway mandate covers.
    pub fn record_oauth_issue(&mut self, client_id: &str, now: &str) {
        let _ = self.journal.record_oauth_issue(client_id, now);
    }

    fn context(&self, name: &str) -> Result<&ContextRuntime> {
        self.contexts
            .get(name)
            .ok_or_else(|| GatewayError::RequestRejected(format!("unknown context `{name}`")))
    }

    fn context_mut(&mut self, name: &str) -> Result<&mut ContextRuntime> {
        self.contexts
            .get_mut(name)
            .ok_or_else(|| GatewayError::RequestRejected(format!("unknown context `{name}`")))
    }
}

// ---------------------------------------------------------- owner tooling
// Runs where the enterprise master seed lives — NEVER inside the runner.
// One master anchors everything (decisions §6/§9): the owner keys of
// journals and contexts derive by label; agent keys never derive.

/// Derived owner keys for an enterprise-owned ethos (journal or context).
fn derived_owner(master: &[u8; 32], kind: &str, label: &str) -> OwnerKeys {
    OwnerKeys::genesis(&MasterSeed::from_bytes(aithos_core::derive::derive_key(
        &format!("aithos-gw/v1/{kind}/{label}"),
        master,
    )))
}

/// Owner-side promotion of the P3 history-seeding seam. The master seed
/// stays on the operator machine; only signed A.2 requests leave it.
/// `kind` is deliberately closed to the two derivation domains created
/// by this gateway's owner tooling.
#[allow(clippy::too_many_arguments)]
pub fn owner_replicate_history_to_remote(
    master: &[u8; 32],
    kind: &str,
    label: &str,
    local_root: &std::path::Path,
    url: &str,
    tenant: &str,
    now: Arc<dyn Fn() -> String + Send + Sync>,
    entropy: Box<dyn EntropySource + Send>,
) -> Result<(String, OwnerReplicationReport)> {
    if !matches!(kind, "journal" | "context") {
        return Err(GatewayError::ConfigRejected(
            "owner replication kind must be `journal` or `context`".into(),
        ));
    }
    let owner = derived_owner(master, kind, label);
    let did = aithos_core::wire::did_aithos(&owner.root_sign.verifying_key().to_bytes());
    let primary = aithos_bundle::FsStore::new(local_root.to_path_buf());
    let did_doc = primary
        .get("did.json")
        .map_err(|e| GatewayError::BridgeFailed(e.to_string()))?
        .ok_or_else(|| GatewayError::ConfigRejected("local store has no did.json".into()))?;
    let local_did = serde_json::from_slice::<serde_json::Value>(&did_doc)
        .ok()
        .and_then(|doc| doc.get("id").and_then(|id| id.as_str()).map(str::to_owned))
        .ok_or_else(|| GatewayError::ConfigRejected("local did.json has no string id".into()))?;
    if local_did != did {
        return Err(GatewayError::ConfigRejected(format!(
            "local DID `{local_did}` does not match {kind} label `{label}`"
        )));
    }
    let signer = Arc::new(KeySigner::owner("#root", owner.root_sign.clone()));
    let mut remote = RemoteStore::new(url, tenant, &did, signer, now, entropy)
        .map_err(|e| GatewayError::ConfigRejected(format!("remote store: {e}")))?;
    let report = replicate_owner_history(local_root, &mut remote)
        .map_err(|e| GatewayError::BridgeFailed(e.to_string()))?;
    Ok((did, report))
}

fn derived_succession(master: &[u8; 32], kind: &str, label: &str) -> SigningKey {
    succession_from_entropy(aithos_core::derive::derive_key(
        &format!("aithos-gw/v1/{kind}/{label}/succession"),
        master,
    ))
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

/// One narrowly scoped control-plane delegate. The seed is handed to the
/// enterprise client once; the gateway persists only the signed mandate.
pub struct ConnectorConfigGrant {
    pub mandate: String,
    pub seed_hex: String,
}

/// Governed replacement result: fresh active equipment plus the prior
/// certificates that were politically revoked in the same owner gesture.
#[derive(Debug, Clone)]
pub struct ReenrollOutcome {
    pub equipment: EquipOutcome,
    pub revoked_mandates: Vec<String>,
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
pub fn owner_init_journal(
    master: &[u8; 32],
    agent_label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    token_budget: Option<u64>,
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    let owner = derived_owner(master, "journal", agent_label);
    let succession = derived_succession(master, "journal", agent_label);
    let mut bundle =
        Bundle::init(store, &owner, &succession.verifying_key(), ent, now).map_err(bridge_err)?;
    // The memory shelf: an owner-prepared circle folder the memory pen
    // will write into. An append perimeter grows content, never the
    // tree shape — the folder must pre-exist.
    bundle
        .ensure_folder(Zone::Circle, MEMORY_FOLDER, &owner, ent)
        .map_err(bridge_err)?;
    bundle.publish(&owner, now).map_err(bridge_err)?;
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

/// Create a context Ethos owned by the enterprise (demo/dev path — real
/// contexts usually pre-exist with their own history).
pub fn owner_init_context(
    master: &[u8; 32],
    label: &str,
    store: GatewayStore,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    let owner = derived_owner(master, "context", label);
    let succession = derived_succession(master, "context", label);
    let bundle =
        Bundle::init(store, &owner, &succession.verifying_key(), ent, now).map_err(bridge_err)?;
    Ok(bundle.did)
}

/// Grant a context to the agent's PUBLIC key: read mandate on the listed
/// tools, governance mandate for the gateway, scoped audit mandate.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_context(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    read_tools: &[String],
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    let owner = derived_owner(master, "context", label);
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    let read_ops: Vec<String> = read_tools.iter().map(|t| op_for_tool(t)).collect();
    equip(
        bundle,
        &owner,
        agent_pub_mb,
        gateway_pub_mb,
        &read_ops,
        true,
        None,
        None,
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
pub fn owner_grant_session_delegate(
    master: &[u8; 32],
    label: &str,
    delegate_pub_mb: &str,
    gateway_audience: &str,
    tools: &[String],
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let delegate_pub = decode_pub(delegate_pub_mb)?;
    // Each granted line is either a raw perimeter entry (act.…, or an
    // ethos entry like `read.public` — the zone rights a delegated
    // session may carry, lot 1) or a bare tool name projected onto the
    // gateway's own connector. `self` is refused at the gesture: never
    // delegable until the delegated self-resolution core lot.
    let mut perimeter = Vec::new();
    for tool in tools {
        let entry = if tool.starts_with("act.") {
            PerimeterEntry::parse(tool).map_err(bridge_err)?
        } else if let Ok(parsed) = PerimeterEntry::parse(tool) {
            parsed
        } else {
            PerimeterEntry::parse(&op_for_tool(tool)).map_err(bridge_err)?
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
            return Err(GatewayError::ConfigRejected(
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
            &serde_json::to_vec_pretty(&mandate).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;
    bundle
        .log_owner_grant(&owner, &mandate.id, now, ent)
        .map_err(bridge_err)?;
    Ok(mandate.id)
}

/// Mint an exact `act.x.<connector>.config` delegate for the signed control
/// plane. This consumes Core's existing perimeter grammar and grant log; it
/// does not create a gateway-local authority dialect.
#[allow(clippy::too_many_arguments)]
pub fn owner_grant_connector_config(
    master: &[u8; 32],
    label: &str,
    connector: &str,
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<ConnectorConfigGrant> {
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
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
            &serde_json::to_vec_pretty(&mandate).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;
    bundle
        .log_owner_grant(&owner, &mandate.id, now, ent)
        .map_err(bridge_err)?;
    Ok(ConnectorConfigGrant {
        mandate: mandate.id,
        seed_hex: hex::encode(seed),
    })
}

/// Discover/approval has already produced a validated manifest. Pin it
/// sealed under `/x/<server>` with owner + gateway header lines, publish
/// that vault header, then mint the read-class tool mandate through the
/// same equip path as `owner_grant_context`.
#[allow(clippy::too_many_arguments)]
pub fn owner_enroll_server(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    manifest: &ApprovedManifest,
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    owner_enroll_servers(
        master,
        label,
        agent_pub_mb,
        gateway_pub_mb,
        std::slice::from_ref(manifest),
        store,
        window,
        now,
        ent,
    )
}

/// Enroll N servers into ONE context under ONE owner gesture (lot D):
/// each approved manifest is pinned sealed under its own `/x/<server>`,
/// then a SINGLE agent mandate covers the union of the granted tools —
/// « un seul mandat agent couvre les outils grantés », the demo's
/// provisioning shape. All-or-nothing on validation: a duplicate server
/// in the batch, an already-enrolled server or an invalid manifest
/// refuses the whole gesture before anything is pinned.
#[allow(clippy::too_many_arguments)]
pub fn owner_enroll_servers(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    manifests: &[ApprovedManifest],
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<EquipOutcome> {
    if manifests.is_empty() {
        return Err(GatewayError::ConfigRejected(
            "enrollment needs at least one approved manifest".into(),
        ));
    }
    let mut servers = std::collections::BTreeSet::new();
    for manifest in manifests {
        validate_approved(manifest)?;
        if !servers.insert(manifest.server.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "the batch enrolls server `{}` twice",
                manifest.server
            )));
        }
    }
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    for manifest in manifests {
        let (_, manifest_path) = hub_manifest_paths(&manifest.server);
        if bundle
            .store
            .get(&manifest_path)
            .map_err(bridge_err)?
            .is_some()
        {
            return Err(GatewayError::ConfigRejected(format!(
                "server `{}` is already enrolled; use governed re-enrollment",
                manifest.server
            )));
        }
    }
    for manifest in manifests {
        pin_hub_manifest(&mut bundle, &owner, gateway_pub_mb, manifest, now, ent)?;
    }
    // The mandate covers the GRANTED tools — the decision, not the
    // class (lot W): a granted write is covered, a denied read is not —
    // across every server of the batch.
    let granted_ops: Vec<String> = manifests
        .iter()
        .flat_map(|manifest| {
            manifest
                .tools
                .iter()
                .filter(|tool| tool.is_granted())
                .map(|tool| hub_op_for_tool(&manifest.server, &tool.name))
        })
        .collect();
    equip(
        bundle,
        &owner,
        agent_pub_mb,
        gateway_pub_mb,
        &granted_ops,
        true,
        None,
        None,
        window,
        now,
        ent,
    )
}

/// Replace an existing server pin for the SAME agent key. The owner
/// seals the newly approved manifest, issues fresh equipment, then
/// appends revocations for the superseded agent, gateway and auditor
/// mandates. Old certificates remain as immutable audit evidence.
#[allow(clippy::too_many_arguments)]
pub fn owner_reenroll_server(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    gateway_pub_mb: &str,
    manifest: &ApprovedManifest,
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<ReenrollOutcome> {
    validate_approved(manifest)?;
    let owner = derived_owner(master, "context", label);
    let reopen = store.clone();
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let state: BridgeState = read_state_migrating(&mut bundle)?;
    let old_agent: Mandate = read_json(&bundle, &cert_path(&state.agent_mandate))?;
    let expected_agent = decode_pub(agent_pub_mb)?;
    if old_agent.grantee_pub().map_err(bridge_err)? != expected_agent {
        return Err(GatewayError::ConfigRejected(
            "re-enrollment must keep the same agent public key".into(),
        ));
    }
    let (_, manifest_path) = hub_manifest_paths(&manifest.server);
    if bundle
        .store
        .get(&manifest_path)
        .map_err(bridge_err)?
        .is_none()
    {
        return Err(GatewayError::ConfigRejected(format!(
            "server `{}` is not enrolled",
            manifest.server
        )));
    }
    replace_hub_manifest(&mut bundle, &owner, manifest, ent)?;
    let granted_ops: Vec<String> = manifest
        .tools
        .iter()
        .filter(|tool| tool.is_granted())
        .map(|tool| hub_op_for_tool(&manifest.server, &tool.name))
        .collect();
    let mut revoked_mandates = vec![state.agent_mandate.clone(), state.gateway_mandate.clone()];
    revoked_mandates.extend(state.auditor_mandate.clone());
    let equipment = equip(
        bundle,
        &owner,
        agent_pub_mb,
        gateway_pub_mb,
        &granted_ops,
        true,
        None,
        None,
        window,
        now,
        ent,
    )?;
    let mut bundle = Bundle::open(reopen).map_err(bridge_err)?;
    // The briefing pen (lot K) is orthogonal equipment: re-enrolling a
    // server replaces the TOOL mandates, never the owner's directive
    // channel — the pen survives the pin swap, unrevoked.
    if state.briefing_mandate.is_some() {
        let mut fresh: BridgeState = read_state_migrating(&mut bundle)?;
        fresh.briefing_mandate = state.briefing_mandate.clone();
        bundle
            .store
            .put(
                STATE_PATH,
                &serde_json::to_vec_pretty(&fresh).map_err(bridge_err)?,
            )
            .map_err(bridge_err)?;
    }
    for mandate in &revoked_mandates {
        bundle
            .log_revoke_owner(
                &owner,
                mandate,
                "superseded by governed server re-enrollment",
                now,
                ent,
            )
            .map_err(bridge_err)?;
    }
    Ok(ReenrollOutcome {
        equipment,
        revoked_mandates,
    })
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
pub fn owner_grant_briefing(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    let owner = derived_owner(master, "context", label);
    let agent_pub = decode_pub(agent_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let mut state: BridgeState = read_state_migrating(&mut bundle)?;
    let expected_agent = &state.agent_mandate;
    let agent_cert: Mandate = read_json(&bundle, &cert_path(expected_agent))?;
    if agent_cert.grantee_pub().map_err(bridge_err)? != agent_pub {
        return Err(GatewayError::ConfigRejected(
            "the briefing pen must go to the equipped agent public key".into(),
        ));
    }
    // The shelves: owner-prepared folders in both served zones. A read
    // perimeter serves content, never grows the tree — they must exist.
    bundle
        .ensure_folder(Zone::Public, BRIEFING_FOLDER, &owner, ent)
        .map_err(bridge_err)?;
    bundle
        .ensure_folder(Zone::Circle, BRIEFING_FOLDER, &owner, ent)
        .map_err(bridge_err)?;
    bundle.publish(&owner, now).map_err(bridge_err)?;
    // The physics half exists for CIRCLE only: public is clear by
    // design (§02.1 — no zone key, no header line to deliver), so the
    // circle dir gets the sealed line and the certificate names both
    // zones (the public entry documents the granted read even though no
    // key gates it).
    bundle
        .deliver_zone_line(&owner, &agent_pub, Zone::Circle, BRIEFING_FOLDER, None, ent)
        .map_err(bridge_err)?;
    // The context AUDITOR gets the same circle line: the journalized
    // briefing reads seal their bodies under the section keys of this
    // dir (§07.9.2), and a gamma query only serves sealed entries the
    // querier can physically open — the auditor mandated on
    // `kind=ethos.read` needs the keys to replay its own slice. The
    // owner accepts what this implies: the context auditor can read the
    // circle directives it audits the reads of.
    if let Some(auditor_id) = &state.auditor_mandate {
        let auditor_cert: Mandate = read_json(&bundle, &cert_path(auditor_id))?;
        let auditor_pub = auditor_cert.grantee_pub().map_err(bridge_err)?;
        bundle
            .deliver_zone_line(
                &owner,
                &auditor_pub,
                Zone::Circle,
                BRIEFING_FOLDER,
                None,
                ent,
            )
            .map_err(bridge_err)?;
    }
    let mut perimeter = Vec::new();
    for zone in [Zone::Public, Zone::Circle] {
        let dir = bundle
            .resolve_folder(zone, BRIEFING_FOLDER)
            .map_err(bridge_err)?;
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
            &serde_json::to_vec_pretty(&mandate).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;
    bundle
        .log_owner_grant(&owner, &mandate.id, now, ent)
        .map_err(bridge_err)?;
    state.briefing_mandate = Some(mandate.id.clone());
    bundle
        .store
        .put(
            STATE_PATH,
            &serde_json::to_vec_pretty(&state).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;
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
pub fn owner_set_briefing(
    master: &[u8; 32],
    label: &str,
    zone: &str,
    title: &str,
    text: &str,
    store: GatewayStore,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let zone = match zone {
        "public" => Zone::Public,
        "circle" => Zone::Circle,
        "self" => Zone::Self_,
        other => {
            return Err(GatewayError::ConfigRejected(format!(
                "briefing zone must be public, circle or self, not `{other}`"
            )))
        }
    };
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    bundle
        .ensure_folder(zone, BRIEFING_FOLDER, &owner, ent)
        .map_err(bridge_err)?;
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
            .map_err(bridge_err)?;
        return bundle.publish(&owner, now).map_err(bridge_err);
    }
    if zone != Zone::Circle {
        return Err(GatewayError::ConfigRejected(format!(
            "rewriting a `{}` directive is circle-only in v1 — the {} zone directive is written once",
            zone.as_str(),
            zone.as_str()
        )));
    }
    bundle
        .section_rewrite(zone, &path, text, &owner, now, ent)
        .map_err(bridge_err)?;
    bundle.publish(&owner, now).map_err(bridge_err)
}

fn replace_hub_manifest(
    bundle: &mut Bundle<GatewayStore>,
    owner: &OwnerKeys,
    manifest: &ApprovedManifest,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let (header_path, manifest_path) = hub_manifest_paths(&manifest.server);
    let header: Header = read_json(bundle, &header_path)?;
    header.validate().map_err(bridge_err)?;
    let expected_node = format!("/x/{}", manifest.server);
    if header.node != expected_node {
        return Err(GatewayError::BridgeFailed(format!(
            "hub manifest header targets `{}`, expected `{expected_node}`",
            header.node
        )));
    }
    let (version, key) = header
        .open_latest(&bundle.did, "owner-kex", &owner.owner_kex)
        .map_err(bridge_err)?;
    let plain = aithos_core::jcs::canonical_bytes(manifest).map_err(bridge_err)?;
    let nonce = ent.e24();
    let aad = aithos_core::seal::blob_aad(&bundle.did, &expected_node, version);
    let cipher = aithos_core::seal::blob_seal(&key, &plain, &nonce, &aad);
    let mut sealed = Vec::with_capacity(nonce.len() + cipher.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&cipher);
    bundle
        .store
        .put(&manifest_path, &sealed)
        .map_err(bridge_err)
}

fn pin_hub_manifest(
    bundle: &mut Bundle<GatewayStore>,
    owner: &OwnerKeys,
    gateway_pub_mb: &str,
    manifest: &ApprovedManifest,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let gateway_pub = decode_pub(gateway_pub_mb)?;
    let gateway_recipient = Recipient {
        to: gateway_pub_mb.to_owned(),
        kid: gateway_pub_mb.to_owned(),
        pubkey: ed2x(&gateway_pub),
    };
    let node = format!("/x/{}", manifest.server);
    let key = bundle
        .audit_key_owner(owner, &manifest.server)
        .map_err(bridge_err)?;
    let header = Header::build(
        &bundle.did,
        &node,
        &key,
        &[Recipient::owner(owner.owner_kex_pub()), gateway_recipient],
        &[ent.e32(), ent.e32()],
        &[ent.e24(), ent.e24()],
    )
    .map_err(bridge_err)?;
    let (header_path, manifest_path) = hub_manifest_paths(&manifest.server);
    bundle
        .store
        .put(
            &header_path,
            &serde_json::to_vec_pretty(&header).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;

    let plain = aithos_core::jcs::canonical_bytes(manifest).map_err(bridge_err)?;
    let nonce = ent.e24();
    let aad = aithos_core::seal::blob_aad(&bundle.did, &node, 1);
    let cipher = aithos_core::seal::blob_seal(&key, &plain, &nonce, &aad);
    let mut sealed = Vec::with_capacity(nonce.len() + cipher.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&cipher);
    bundle
        .store
        .put(&manifest_path, &sealed)
        .map_err(bridge_err)?;
    // The vault state root commits the new /x/<server> header before
    // any grant is issued. The sealed blob is authenticated by that DK.
    bundle.publish(owner, now).map_err(bridge_err)
}

/// Owner-side proof/read helper used by enrollment tests and tooling.
pub fn owner_read_hub_manifest(
    master: &[u8; 32],
    label: &str,
    server: &str,
    store: GatewayStore,
) -> Result<ApprovedManifest> {
    if !crate::policy::valid_server_name(server) {
        return Err(GatewayError::ConfigRejected(format!(
            "invalid hub server name `{server}`"
        )));
    }
    let owner = derived_owner(master, "context", label);
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    let (header_path, manifest_path) = hub_manifest_paths(server);
    let header: Header = read_json(&bundle, &header_path)?;
    header.validate().map_err(bridge_err)?;
    let expected_node = format!("/x/{server}");
    if header.node != expected_node {
        return Err(GatewayError::BridgeFailed(format!(
            "hub manifest header targets `{}`, expected `{expected_node}`",
            header.node
        )));
    }
    let (version, key) = header
        .open_latest(&bundle.did, "owner-kex", &owner.owner_kex)
        .map_err(bridge_err)?;
    let sealed = bundle
        .store
        .get(&manifest_path)
        .map_err(bridge_err)?
        .ok_or_else(|| GatewayError::BridgeFailed(format!("missing {manifest_path}")))?;
    if sealed.len() < 24 {
        return Err(GatewayError::BridgeFailed(format!(
            "truncated {manifest_path}"
        )));
    }
    let nonce: [u8; 24] = sealed[..24].try_into().expect("length checked");
    let aad = aithos_core::seal::blob_aad(&bundle.did, &expected_node, version);
    let plain =
        aithos_core::seal::blob_open(&key, &sealed[24..], &nonce, &aad).map_err(bridge_err)?;
    let manifest: ApprovedManifest = serde_json::from_slice(&plain).map_err(bridge_err)?;
    validate_approved(&manifest)?;
    Ok(manifest)
}

/// Shared equip path: mint the mandates towards the agent/gateway PUBLIC
/// keys, log every grant (issuance is never silent), persist certs+state.
#[allow(clippy::too_many_arguments)]
fn equip(
    mut bundle: Bundle<GatewayStore>,
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
                .map_err(bridge_err)?;
            let dir = bundle
                .resolve_folder(Zone::Circle, folder)
                .map_err(bridge_err)?;
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
                &serde_json::to_vec_pretty(m).map_err(bridge_err)?,
            )
            .map_err(bridge_err)?;
        bundle
            .log_owner_grant(owner, &m.id, now, ent)
            .map_err(bridge_err)?;
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
            &serde_json::to_vec_pretty(&state).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;

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

fn decode_pub(multibase: &str) -> Result<ed25519_dalek::VerifyingKey> {
    let bytes = aithos_core::wire::multibase_to_ed25519_pub(multibase).map_err(bridge_err)?;
    ed25519_dalek::VerifyingKey::from_bytes(&bytes)
        .map_err(|e| GatewayError::BridgeFailed(format!("bad agent public key: {e}")))
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
pub fn owner_grant_ethos_read(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    zones: &[String],
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<String> {
    if zones.is_empty() {
        return Err(GatewayError::ConfigRejected(
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
                return Err(GatewayError::ConfigRejected(
                    "zone `self` is refused: read.self is never granted by default, and \
                     serving it awaits the delegated self-resolution core lot (vectors-first)"
                        .into(),
                ))
            }
            other => {
                return Err(GatewayError::ConfigRejected(format!(
                    "unknown zone `{other}` (public, circle)"
                )))
            }
        }
    }
    let owner = derived_owner(master, "context", label);
    let agent_pub = decode_pub(agent_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let state: BridgeState = read_state_migrating(&mut bundle)?;
    let agent_cert: Mandate = read_json(&bundle, &cert_path(&state.agent_mandate))?;
    if agent_cert.grantee_pub().map_err(bridge_err)? != agent_pub {
        return Err(GatewayError::ConfigRejected(
            "the ethos-read pen must go to the equipped agent public key".into(),
        ));
    }
    if wants_circle {
        bundle
            .deliver_zone_line(&owner, &agent_pub, Zone::Circle, "", None, ent)
            .map_err(bridge_err)?;
        if let Some(auditor_id) = &state.auditor_mandate {
            let auditor_cert: Mandate = read_json(&bundle, &cert_path(auditor_id))?;
            let auditor_pub = auditor_cert.grantee_pub().map_err(bridge_err)?;
            bundle
                .deliver_zone_line(&owner, &auditor_pub, Zone::Circle, "", None, ent)
                .map_err(bridge_err)?;
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
            &serde_json::to_vec_pretty(&mandate).map_err(bridge_err)?,
        )
        .map_err(bridge_err)?;
    bundle
        .log_owner_grant(&owner, &mandate.id, now, ent)
        .map_err(bridge_err)?;
    Ok(mandate.id)
}

/// Deliver the circle zone line to ONE recipient key (§04.3 — the line
/// is the physics half of a pen). Generic on purpose: the briefing pen
/// delivers to the agent key, a delegated session pen delivers to the
/// GATEWAY key (the session leaf grantee), and the auditor gets its
/// copy when present — issuance appends the needed lines, the
/// certificate half travels separately.
pub fn owner_deliver_circle_line(
    master: &[u8; 32],
    label: &str,
    recipient_pub_mb: &str,
    store: GatewayStore,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let owner = derived_owner(master, "context", label);
    let recipient_pub = decode_pub(recipient_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let _ = now;
    bundle
        .deliver_zone_line(&owner, &recipient_pub, Zone::Circle, "", None, ent)
        .map_err(bridge_err)?;
    Ok(())
}

/// Owner revocation of one mandate on a context store (lot G6 scenario
/// surface; the M3 product surface `owner-revoke-mandate` subsumes it
/// later). One `revoke` entry — the runtime scan sees it on the very
/// next call, no restart.
pub fn owner_revoke_mandate_id(
    master: &[u8; 32],
    label: &str,
    mandate_id: &str,
    reason: &str,
    store: GatewayStore,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    bundle
        .log_revoke_owner(&owner, mandate_id, reason, now, ent)
        .map_err(bridge_err)?;
    Ok(())
}

/// Owner-side section write (lot G6 provisioning surface, the generic
/// sibling of `owner_set_briefing`): ensure the folder chain, add ONE
/// fresh section — title = the last path segment, the human label the
/// clear index shows. Demos and harnesses fill zones with it.
#[allow(clippy::too_many_arguments)]
pub fn owner_add_section(
    master: &[u8; 32],
    label: &str,
    zone: &str,
    path: &str,
    text: &str,
    store: GatewayStore,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<()> {
    let zone = match zone {
        "public" => Zone::Public,
        "circle" => Zone::Circle,
        "self" => Zone::Self_,
        other => {
            return Err(GatewayError::ConfigRejected(format!(
                "zone must be public, circle or self, not `{other}`"
            )))
        }
    };
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let (folder_path, name) = match path.rsplit_once('/') {
        Some((dir, name)) => (dir, name),
        None => ("", path),
    };
    if !folder_path.is_empty() {
        bundle
            .ensure_folder(zone, folder_path, &owner, ent)
            .map_err(bridge_err)?;
        bundle.publish(&owner, now).map_err(bridge_err)?;
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
        .map_err(bridge_err)
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
pub fn owner_issue_ethos_read_subchain(
    master: &[u8; 32],
    label: &str,
    agent_pub_mb: &str,
    store: GatewayStore,
    window: &MandateWindow,
    now: &str,
    ent: &mut dyn EntropySource,
) -> Result<(String, String)> {
    let owner = derived_owner(master, "context", label);
    let agent_pub = decode_pub(agent_pub_mb)?;
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
    let delegate_sk = SigningKey::from_bytes(&ent.e32());
    let circle_read = || PerimeterEntry::Ethos {
        verb: Verb::Read,
        zone: Zone::Circle,
        dir: Vec::new(),
        tag: None,
    };
    let issue = PerimeterEntry::parse("issue#depth=1").map_err(bridge_err)?;
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
    .map_err(bridge_err)?;
    for mandate in [&parent, &sub] {
        bundle
            .store
            .put(
                &cert_path(&mandate.id),
                &serde_json::to_vec_pretty(mandate).map_err(bridge_err)?,
            )
            .map_err(bridge_err)?;
        bundle
            .log_owner_grant(&owner, &mandate.id, now, ent)
            .map_err(bridge_err)?;
    }
    bundle
        .deliver_zone_line(&owner, &agent_pub, Zone::Circle, "", None, ent)
        .map_err(bridge_err)?;
    Ok((parent.id.clone(), sub.id.clone()))
}

/// Owner/test-side view of any ethos gamma (opens the store read-only).
pub fn gamma_view(store: GatewayStore) -> Result<Vec<EntryView>> {
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    Ok(bundle
        .gamma_entries()
        .map_err(bridge_err)?
        .iter()
        .map(view)
        .collect())
}

/// Owner/ops-side view of a journal's memory shelf: the CLEAR index
/// skeleton (names, titles, tags — never a body), oldest first. What an
/// operator or test lists before opening a note with the owner keys.
pub fn journal_notes_view(store: GatewayStore) -> Result<Vec<NoteView>> {
    let bundle = Bundle::open(store).map_err(bridge_err)?;
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

/// Owner-side read of one memory note body (sovereignty §3bis.3: the
/// journal is enterprise-owned — the owner audits its agent's memory
/// with its own derived keys, no pen involved).
pub fn owner_read_journal_note(
    master: &[u8; 32],
    agent_label: &str,
    store: GatewayStore,
    name: &str,
) -> Result<String> {
    let owner = derived_owner(master, "journal", agent_label);
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    bundle
        .read_section(Zone::Circle, &format!("{MEMORY_FOLDER}/{name}"), &owner)
        .map_err(bridge_err)
}

/// The grantee public key (multibase) named by a stored certificate.
pub fn cert_grantee_pub(store: GatewayStore, mandate_id: &str) -> Result<String> {
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    let m: Mandate = read_json(&bundle, &cert_path(mandate_id))?;
    Ok(m.grantee.pubkey)
}

/// The constraints block carried by a stored certificate (owner/test-side
/// assertions — e.g. the token budget on the inference pen).
pub fn cert_constraints(store: GatewayStore, mandate_id: &str) -> Result<serde_json::Value> {
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    let m: Mandate = read_json(&bundle, &cert_path(mandate_id))?;
    Ok(m.constraints)
}

/// Canonical perimeter strings carried by a stored certificate.
pub fn cert_perimeter(store: GatewayStore, mandate_id: &str) -> Result<Vec<String>> {
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    let mandate: Mandate = read_json(&bundle, &cert_path(mandate_id))?;
    Ok(mandate.perimeter)
}

fn manifest_catalog_digest<'a>(
    server: &str,
    tools: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<String> {
    let mut tools = tools
        .into_iter()
        .map(|(name, pin_sha256)| serde_json::json!({ "name": name, "pin_sha256": pin_sha256 }))
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    hash_of(&serde_json::json!({
        "version": crate::hub::MANIFEST_VERSION,
        "server": server,
        "tools": tools,
    }))
}

// ------------------------------------------------- effective policy (M2)
//
// The pure heart of the mandates product surface: ONE function computes
// what the runtime would decide — the owner previews it, the UI renders
// it, and (a later lot, after equivalence is proven on the whole suite)
// the hot path plugs into it. In this lot the runtime is deliberately
// NOT rebranched: `tests/policy_equivalence.rs` replays the verdicts of
// the existing grants/bounds scenarios through these functions and
// requires literal equality with `Runner::authorize` + `check_bounds`.

/// Version tag of the effective-policy read-model JSON (stable for the UI).
pub const EFFECTIVE_POLICY_VERSION: &str = "aithos-effective-policy-v1";

/// The previewed tool universe: exposed name → (server, approved tool).
type EffectiveTools = BTreeMap<String, (String, ApprovedTool)>;

/// Everything the effective-policy functions need, loaded owner-side
/// from the files alone. Loading is I/O; deciding is pure.
struct PreviewInputs {
    context_did: String,
    chain: Vec<Mandate>,
    doc: DidDocument,
    revocations: Vec<Revocation>,
    tools: EffectiveTools,
}

fn preview_load(
    master: &[u8; 32],
    label: &str,
    servers: &[String],
    store: GatewayStore,
) -> Result<PreviewInputs> {
    let mut bundle = Bundle::open(store.clone()).map_err(bridge_err)?;
    let state: BridgeState = read_state_migrating(&mut bundle)?;
    let mandate: Mandate = read_json(&bundle, &cert_path(&state.agent_mandate))?;
    let doc: DidDocument = read_json(&bundle, "did.json")?;
    let entries = bundle.gamma_entries().map_err(bridge_err)?;
    let revocations = revocations(&entries);
    let mut tools: EffectiveTools = BTreeMap::new();
    for server in servers {
        let manifest = owner_read_hub_manifest(master, label, server, store.clone())?;
        for tool in manifest.tools {
            let exposed = tool.exposed_name.clone();
            if tools
                .insert(exposed.clone(), (manifest.server.clone(), tool))
                .is_some()
            {
                return Err(GatewayError::ConfigRejected(format!(
                    "exposed-name collision `{exposed}` across previewed servers"
                )));
            }
        }
    }
    Ok(PreviewInputs {
        context_did: bundle.did.clone(),
        chain: vec![mandate],
        doc,
        revocations,
        tools,
    })
}

/// Lifecycle status of a chain at `now`, from the verifier's own
/// primitives (string-ordered RFC 3339 Z instants, injected revocation
/// facts). Returns the status and an optional detail (reason).
fn preview_status(
    chain: &[Mandate],
    doc: &DidDocument,
    revs: &[Revocation],
    now: &str,
) -> (&'static str, Option<String>) {
    if let Err(e) = chain_revoked_at(chain, revs, now) {
        return ("revoked", Some(e.to_string()));
    }
    let Some(leaf) = chain.last() else {
        return ("invalid", Some("empty chain".to_owned()));
    };
    if now < leaf.not_before.as_str() {
        return ("not_yet_valid", None);
    }
    if now > leaf.not_after.as_str() {
        return ("expired", None);
    }
    match verify_chain_revocable(chain, doc, now, revs) {
        Ok(()) => ("active", None),
        Err(e) => ("invalid", Some(e.to_string())),
    }
}

/// The pure per-call verdict: EXACTLY the runtime's pre-relay decision
/// (resolve → mandate at T → owner bounds), computed from values alone.
/// Drift is deliberately absent — it is a runtime observation of the
/// upstream, never policy. Revocation facts are conjoined here (the
/// runtime pre-check defers them to the append wall; no reachable
/// runtime state diverges, and the equivalence tests prove it).
fn effective_call_verdict(
    inputs: &PreviewInputs,
    tool: &str,
    args: &serde_json::Value,
    now: &str,
) -> Result<()> {
    let Some((server, approved)) = inputs.tools.get(tool) else {
        return Err(GatewayError::ToolNotMapped(tool.to_owned()));
    };
    let op = hub_op_for_tool(server, &approved.name);
    let denied = |reason: String| GatewayError::MandateDenied {
        op: op.clone(),
        reason: format!("exposed tool `{tool}`: {reason}"),
    };
    verify_chain_revocable(&inputs.chain, &inputs.doc, now, &inputs.revocations)
        .map_err(|e| denied(e.to_string()))?;
    let leaf = inputs.chain.last().expect("verified chain is non-empty");
    let perimeter = leaf.parsed_perimeter().map_err(bridge_err)?;
    let covered = covers_act(
        &perimeter,
        &ActOp {
            connector: server.clone(),
            action: crate::policy::action_name(&approved.name),
        },
    );
    if !covered {
        return Err(denied("outside the granted perimeter".to_owned()));
    }
    // The owner-approved bounds, applied exactly as `Runner::check_bounds`
    // applies them: pinned-schema shape first, then each rule, fail-closed.
    for bound in &approved.bounds {
        let field = bound.field();
        let pinned_type = approved
            .input_schema
            .pointer(&format!("/properties/{field}/type"))
            .and_then(serde_json::Value::as_str);
        if pinned_type == Some("array") {
            if let Some(value) = args.get(field) {
                if !value.is_array() {
                    return Err(GatewayError::BoundViolated(format!(
                        "`{}.{field}` — must be an array of strings per the pinned schema",
                        approved.name
                    )));
                }
            }
        }
        bound.check(&approved.name, args)?;
    }
    Ok(())
}

/// The read-model JSON: mandate lifecycle, then every previewed tool
/// with its class, grant decision, mandate coverage at `now` and its
/// inherited bounds. `served` is what `tools/list` would show.
fn describe_effective_policy(inputs: &PreviewInputs, now: &str) -> Result<serde_json::Value> {
    let (status, status_detail) =
        preview_status(&inputs.chain, &inputs.doc, &inputs.revocations, now);
    let leaf = inputs.chain.last();
    let perimeter = leaf
        .map(|m| m.parsed_perimeter().map_err(bridge_err))
        .transpose()?
        .unwrap_or_default();
    let mandate = leaf.map(|m| {
        let mut view = serde_json::json!({
            "id": m.id,
            "grantee_label": m.grantee.label,
            "grantee_pub": m.grantee.pubkey,
            "not_before": m.not_before,
            "not_after": m.not_after,
            "status": status,
            "perimeter": m.perimeter,
            "constraints": m.constraints,
        });
        if let Some(detail) = &status_detail {
            view["status_detail"] = serde_json::Value::String(detail.clone());
        }
        view
    });
    let active = status == "active";
    let mut tools = Vec::with_capacity(inputs.tools.len());
    for (exposed, (server, approved)) in &inputs.tools {
        let covered = covers_act(
            &perimeter,
            &ActOp {
                connector: server.clone(),
                action: crate::policy::action_name(&approved.name),
            },
        );
        tools.push(serde_json::json!({
            "tool": exposed,
            "server": server,
            "upstream_tool": approved.name,
            "risk_class": approved.risk_class,
            "granted": approved.is_granted(),
            "covered": covered,
            "served": approved.is_granted() && covered && active,
            "bounds": approved.bounds,
        }));
    }
    Ok(serde_json::json!({
        "version": EFFECTIVE_POLICY_VERSION,
        "at": now,
        "context_did": inputs.context_did,
        "mandate": mandate,
        "tools": tools,
    }))
}

/// Owner-side preview of one equipped context's effective policy: the
/// stable, versioned JSON the UI renders (owner-preview-mandate). Reads
/// state, certificate, DID document, revocation facts and the sealed
/// manifests of the named servers — files alone, T injected.
pub fn owner_preview_mandate(
    master: &[u8; 32],
    label: &str,
    servers: &[String],
    store: GatewayStore,
    now: &str,
) -> Result<serde_json::Value> {
    let inputs = preview_load(master, label, servers, store)?;
    describe_effective_policy(&inputs, now)
}

/// Owner-side dry-run of ONE hypothetical call: the verdict the gateway
/// would give, out of the same decision logic — the preview IS the
/// decision. Refusals carry the runtime's exact code and message.
pub fn owner_preview_call(
    master: &[u8; 32],
    label: &str,
    servers: &[String],
    store: GatewayStore,
    tool: &str,
    args: &serde_json::Value,
    now: &str,
) -> Result<serde_json::Value> {
    let inputs = preview_load(master, label, servers, store)?;
    Ok(match effective_call_verdict(&inputs, tool, args, now) {
        Ok(()) => serde_json::json!({
            "version": EFFECTIVE_POLICY_VERSION,
            "at": now,
            "tool": tool,
            "verdict": "allowed",
        }),
        Err(e) => serde_json::json!({
            "version": EFFECTIVE_POLICY_VERSION,
            "at": now,
            "tool": tool,
            "verdict": "refused",
            "code": e.refusal_code(),
            "detail": e.to_string(),
        }),
    })
}

// ---------------------------------------------------------------- helpers

/// Mint one root mandate from pre-built perimeter entries — what the
/// memory pen uses (its Ethos entry carries resolved folder sids, not a
/// parseable string).
#[allow(clippy::too_many_arguments)]
fn mint_entries(
    owner: &OwnerKeys,
    bundle: &Bundle<GatewayStore>,
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
    .map_err(bridge_err)
}

/// One clear index row of the memory shelf (skeleton data — no body).
pub(crate) struct MemoryRow {
    name: String,
    title: String,
    tags: Vec<String>,
}

/// One agent-facing row of a zone index (lot G6): display path, clear
/// title and tags, and the folder sid-path (what `covers()` walks).
#[derive(Debug, Clone)]
pub(crate) struct EthosRow {
    sid: Sid,
    path: String,
    title: String,
    tags: Vec<String>,
    folders: Vec<Sid>,
}

#[cfg(test)]
mod delegated_session_tests {
    use super::*;

    fn historical_vector() -> serde_json::Value {
        serde_json::from_str(&crate::test_vectors::vector_str("cb2-session-proof.json"))
            .expect("historical CB2 vector parses")
    }

    fn delegated_chain_vector() -> serde_json::Value {
        serde_json::from_str(&crate::test_vectors::vector_str(
            "cb14-delegated-session-chain.json",
        ))
        .expect("CB14 delegated chain vector parses")
    }

    fn vector_revocations(candidate: &serde_json::Value) -> Vec<Revocation> {
        candidate["revocations"]
            .as_array()
            .expect("revocations array")
            .iter()
            .map(|item| Revocation {
                mandate_id: item["mandate_id"]
                    .as_str()
                    .expect("revoked mandate id")
                    .to_owned(),
                revoked_at: item["revoked_at"]
                    .as_str()
                    .expect("revocation time")
                    .to_owned(),
            })
            .collect()
    }

    #[test]
    fn gateway_bridge_consumes_the_historical_sc1_vector_exactly() {
        let vector = historical_vector();
        let positive = &vector["positive"];
        let verified = verify_delegated_session(DelegatedSessionEvidence {
            mandate: &positive["mandate"],
            certificate: &positive["certificate"],
            projection: &positive["operation_projection"],
            operation_ref: &positive["operation_ref"],
            native_leaf_proof: Some(&positive["native_leaf_proof_fixture"]),
            session_proof: Some(&positive["session_proof"]),
        })
        .expect("exact SC1 and both proofs pass through Core");
        assert_eq!(verified, positive["operation_ref"]);

        assert!(verify_delegated_session(DelegatedSessionEvidence {
            mandate: &positive["mandate"],
            certificate: &positive["certificate"],
            projection: &positive["operation_projection"],
            operation_ref: &positive["operation_ref"],
            native_leaf_proof: None,
            session_proof: Some(&positive["session_proof"]),
        })
        .is_err());
        assert!(verify_delegated_session(DelegatedSessionEvidence {
            mandate: &positive["mandate"],
            certificate: &positive["certificate"],
            projection: &positive["operation_projection"],
            operation_ref: &positive["operation_ref"],
            native_leaf_proof: Some(&positive["native_leaf_proof_fixture"]),
            session_proof: None,
        })
        .is_err());
    }

    #[test]
    fn gateway_bridge_verifies_the_non_root_chain_before_unchanged_sc1() {
        let vector = delegated_chain_vector();
        let positive = &vector["positive"];
        let chain: Vec<Mandate> =
            serde_json::from_value(positive["chain"].clone()).expect("mandate chain parses");
        let did: DidDocument =
            serde_json::from_value(positive["did"].clone()).expect("DID document parses");
        let revocations = vector_revocations(positive);

        let verified = verify_delegated_chain_session(DelegatedChainSessionEvidence {
            chain: &chain,
            did: &did,
            at: positive["at"].as_str().expect("verification time"),
            revocations: &revocations,
            mandate: &positive["mandate"],
            certificate: &positive["certificate"],
            projection: &positive["operation_projection"],
            operation_ref: &positive["operation_ref"],
            native_leaf_proof: &positive["native_leaf_proof"],
            session_proof: &positive["session_proof"],
        })
        .expect("delegated chain and unchanged SC1 pass through Core");
        assert_eq!(verified, positive["operation_ref"]);

        let revoked = vector["negative_cases"]
            .as_array()
            .expect("negative cases")
            .iter()
            .find(|case| case["id"] == "revoked-parent")
            .expect("revoked parent case");
        let candidate = &revoked["candidate"];
        let chain: Vec<Mandate> =
            serde_json::from_value(candidate["chain"].clone()).expect("mandate chain parses");
        let did: DidDocument =
            serde_json::from_value(candidate["did"].clone()).expect("DID document parses");
        let revocations = vector_revocations(candidate);
        assert!(
            verify_delegated_chain_session(DelegatedChainSessionEvidence {
                chain: &chain,
                did: &did,
                at: candidate["at"].as_str().expect("verification time"),
                revocations: &revocations,
                mandate: &candidate["mandate"],
                certificate: &candidate["certificate"],
                projection: &candidate["operation_projection"],
                operation_ref: &candidate["operation_ref"],
                native_leaf_proof: &candidate["native_leaf_proof"],
                session_proof: &candidate["session_proof"],
            })
            .is_err()
        );
    }

    #[test]
    fn enrollment_rejects_a_submandate_chain_even_when_it_is_otherwise_valid() {
        let vector = delegated_chain_vector();
        let chain: Vec<Mandate> = serde_json::from_value(vector["positive"]["chain"].clone())
            .expect("mandate chain parses");
        assert!(chain.len() > 1);
        assert!(!enrollment_chain_is_direct_owner(&chain));
        assert!(!enrollment_chain_is_direct_owner(&[chain
            .last()
            .expect("leaf")
            .clone()]));
        assert!(enrollment_chain_is_direct_owner(&[chain
            .first()
            .expect("root")
            .clone()]));
    }

    #[test]
    fn gateway_delegates_active_session_counting_to_core() {
        let keys: Vec<String> = (1u8..=4)
            .map(|byte| {
                let key = SigningKey::from_bytes(&[byte; 32]).verifying_key();
                aithos_core::wire::ed25519_pub_to_multibase(&key.to_bytes())
            })
            .collect();
        let first_three: Vec<&str> = keys.iter().take(3).map(String::as_str).collect();
        assert_eq!(enforce_max_sessions(3, &first_three).unwrap(), 3);
        let all_four: Vec<&str> = keys.iter().map(String::as_str).collect();
        assert!(enforce_max_sessions(3, &all_four).is_err());
        assert!(enforce_max_sessions(3, &[&keys[0], &keys[0]]).is_err());
    }

    #[test]
    fn session_parent_resource_binding_is_exact_and_fail_closed() {
        let resource = "https://demo.mcp.aithos.fr/mcp";
        assert!(constraints_bind_resource(
            &serde_json::json!({"purpose": resource}),
            resource
        ));
        assert!(!constraints_bind_resource(&serde_json::json!({}), resource));
        assert!(!constraints_bind_resource(
            &serde_json::json!({"purpose": "https://gateway-b.example/mcp"}),
            resource
        ));
        assert!(!constraints_bind_resource(
            &serde_json::json!({"purpose": null}),
            resource
        ));
    }
}
