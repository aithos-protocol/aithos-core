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

use std::collections::BTreeMap;
use std::sync::Arc;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use aithos_bundle::bundle::Bundle;
use aithos_bundle::log::{ActionSpec, InferenceSpec, LogFilter};
use aithos_bundle::Store;
use aithos_core::did::DidDocument;
use aithos_core::header::{Header, Recipient};
use aithos_core::keys::{ed2x, grantee_kex_secret, succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{verify_chain, GammaQuery, Mandate, MandateSpec, PerimeterEntry, Verb};
use aithos_core::path::Zone;

use crate::config::{ContextTools, GatewayConfig, ToolAccess};
use crate::hub::{validate_approved, ApprovedManifest, ApprovedTool};
use crate::keyholder::Keyholder;
use crate::policy::{hub_op_for_tool, op_for_tool, Policy};
use crate::store_adapter::GatewayStore;
use crate::{GatewayError, Result};

/// Entropy seam, re-exported so surfaces (binary, tests) never import
/// the bundle directly: the bridge is the only door to the core.
pub use aithos_bundle::entropy::{EntropySource, OsEntropy, SeqEntropy};
/// Raw store trait, re-exported through the same single door — what
/// owner/test-side surgery uses to read or doctor a store it holds.
pub use aithos_bundle::Store as RawStore;

/// Where the bridge keeps its non-secret runtime state in the store.
pub const STATE_PATH: &str = "gateway/state.json";
/// The journal's memory shelf: the circle folder `owner-init-journal`
/// prepares and the memory pen writes into (lot C2).
pub const MEMORY_FOLDER: &str = "memory";
/// The one budget profile id the gateway cites on inference entries —
/// the same id `owner-init-journal --token-budget` writes into the
/// inference mandate (v1: one profile, one tap).
pub const LLM_BUDGET_REF: &str = "llm";
/// Vault record name for one approved hub manifest. The parent
/// `/x/<server>` header is pinned by the bundle's vault root.
const HUB_MANIFEST_FILE: &str = "manifest.enc";
/// Where mandate certificates live in the store.
fn cert_path(id: &str) -> String {
    format!("certs/{id}.json")
}

/// Wire encoding of the agent public key (`z…`, base58btc multicodec) —
/// the ONLY thing that leaves the runner at birth.
pub fn agent_pub_multibase(kh: &Keyholder) -> String {
    let vk = SigningKey::from_bytes(kh.agent_seed()).verifying_key();
    aithos_core::wire::ed25519_pub_to_multibase(&vk.to_bytes())
}

/// Wire encoding of the gateway's own public key.
pub fn gateway_pub_multibase(kh: &Keyholder) -> String {
    let vk = SigningKey::from_bytes(kh.gateway_seed()).verifying_key();
    aithos_core::wire::ed25519_pub_to_multibase(&vk.to_bytes())
}

/// Non-secret state persisted at equip time, reloaded by `open`.
#[derive(Debug, Serialize, Deserialize)]
struct BridgeState {
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
        let bundle = Bundle::open(store).map_err(bridge_err)?;
        let state: BridgeState = read_json(&bundle, STATE_PATH)?;
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
        Ok(Self {
            bundle,
            keyholder,
            agent_chain: vec![agent],
            gateway_chain: vec![gateway],
            auditor_mandate,
            inference_chain,
            memory_chain,
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
        let gateway_sk = SigningKey::from_bytes(self.keyholder.gateway_seed());
        let detail = serde_json::json!({ "tool": tool, "reason": reason });
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
                .map_err(read_denied)?;
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

    /// The mandate id the agent acts under (test assertions).
    pub fn agent_mandate_id(&self) -> &str {
        &self.agent_chain[0].id
    }

    /// The mandate id the gateway refuses under (test assertions).
    pub fn gateway_mandate_id(&self) -> &str {
        &self.gateway_chain[0].id
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

#[derive(Debug, Clone)]
struct HubRuntimeTool {
    context: String,
    server: String,
    raw_tool: String,
    description: Option<String>,
    input_schema: serde_json::Value,
    pin_sha256: String,
    access: ToolAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubRelayTarget {
    pub server: String,
    pub raw_tool: String,
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
        }
    }

    /// Open every context and the journal declared by a multi-context
    /// config (the binary's `run` path). The runner identity is shared:
    /// one keyholder, N bridges. Entropy stays injected — the factory is
    /// called once per bridge.
    pub fn open(
        cfg: &GatewayConfig,
        keyholder: Keyholder,
        mut entropy: impl FnMut() -> Box<dyn EntropySource + Send>,
    ) -> Result<Self> {
        let contexts_cfg = cfg.contexts.as_ref().ok_or_else(|| {
            GatewayError::ConfigRejected("the multi-context runner needs `contexts`".into())
        })?;
        let journal_cfg = cfg.journal.as_ref().ok_or_else(|| {
            GatewayError::ConfigRejected("the multi-context runner needs `journal`".into())
        })?;
        let keyholder = Arc::new(keyholder);
        let mut contexts = BTreeMap::new();
        let mut hub_tools = BTreeMap::new();
        let mut hub_server_pins: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for ctx in contexts_cfg {
            let store = GatewayStore::from_config(&ctx.store)?;
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
                        validate_runtime_tool(&ctx.name, exposed, reference.access, approved)?;
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
                            },
                        );
                    }
                    Policy::new(policy_map)
                }
            };
            contexts.insert(ctx.name.clone(), ContextRuntime { policy, bridge });
        }
        let journal = Bridge::open(
            GatewayStore::from_config(&journal_cfg.store)?,
            keyholder,
            entropy(),
        )?;
        Ok(Self {
            contexts,
            journal,
            hub_tools,
            hub_server_pins,
            hub_drift: BTreeMap::new(),
        })
    }

    /// The context whose tool map names this tool (read or write).
    /// Unambiguous by construction: config v2 rejects cross-context
    /// collisions. Unknown everywhere → `None` (default-deny).
    pub fn resolve(&self, tool: &str) -> Option<&str> {
        self.contexts
            .iter()
            .find(|(_, c)| c.policy.is_mapped(tool))
            .map(|(name, _)| name.as_str())
    }

    /// Every mapped tool name across all contexts (deterministic order) —
    /// what the router's aggregated `tools/list` advertises.
    pub fn mapped_tools(&self) -> Vec<String> {
        self.contexts
            .values()
            .flat_map(|c| c.policy.tools().map(str::to_owned))
            .collect()
    }

    /// Agent-visible descriptors. Hub mode exposes covered (`read`)
    /// pins only; legacy mode preserves the v2 names-only surface.
    pub fn listed_tools(&self) -> Vec<serde_json::Value> {
        if self.hub_tools.is_empty() {
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
            .filter(|(_, tool)| tool.access == ToolAccess::Read)
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
            });
        }
        Ok(HubRelayTarget {
            server: ctx.to_owned(),
            raw_tool: tool.to_owned(),
        })
    }

    pub fn server_pins(&self, server: &str) -> Option<BTreeMap<String, String>> {
        self.hub_server_pins.get(server).cloned()
    }

    pub fn hub_servers(&self) -> Vec<String> {
        self.hub_server_pins.keys().cloned().collect()
    }

    pub fn mark_manifest_drift(&mut self, server: &str, reason: String) {
        self.hub_drift.insert(server.to_owned(), reason);
    }

    pub fn clear_manifest_drift(&mut self, server: &str) {
        self.hub_drift.remove(server);
    }

    pub fn manifest_drift_for(&self, tool: &str) -> Option<GatewayError> {
        let hub = self.hub_tools.get(tool)?;
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

    /// Pre-check on the resolved context: does its mandate cover the
    /// tool at `now`? (`record_act_with_xref` re-verifies at append.)
    pub fn authorize(&self, ctx: &str, tool: &str, now: &str) -> Result<()> {
        if let Some(hub) = self.hub_tools.get(tool) {
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
        let hub = self.hub_tools.get(tool).cloned();
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

fn validate_runtime_tool(
    context: &str,
    exposed: &str,
    access: ToolAccess,
    approved: &ApprovedTool,
) -> Result<()> {
    if approved.exposed_name != exposed {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` names `{exposed}`, approved manifest names `{}`",
            approved.exposed_name
        )));
    }
    if approved.risk_class != access {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` class for `{exposed}` differs from the approved manifest"
        )));
    }
    Ok(())
}

fn merge_server_pins(
    pins: &mut BTreeMap<String, BTreeMap<String, String>>,
    manifest: &ApprovedManifest,
) -> Result<()> {
    let candidate: BTreeMap<String, String> = manifest
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), tool.pin_sha256.clone()))
        .collect();
    if let Some(existing) = pins.get(&manifest.server) {
        if existing != &candidate {
            return Err(GatewayError::ConfigRejected(format!(
                "contexts pin conflicting manifests for shared server `{}`",
                manifest.server
            )));
        }
    } else {
        pins.insert(manifest.server.clone(), candidate);
    }
    Ok(())
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
    validate_approved(manifest)?;
    let owner = derived_owner(master, "context", label);
    let mut bundle = Bundle::open(store).map_err(bridge_err)?;
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

    pin_hub_manifest(&mut bundle, &owner, gateway_pub_mb, manifest, now, ent)?;
    let read_ops: Vec<String> = manifest
        .tools
        .iter()
        .filter(|tool| tool.risk_class == crate::config::ToolAccess::Read)
        .map(|tool| hub_op_for_tool(&manifest.server, &tool.name))
        .collect();
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

fn hub_manifest_paths(server: &str) -> (String, String) {
    (
        format!("e/x/{server}/header.json"),
        format!("e/x/{server}/{HUB_MANIFEST_FILE}"),
    )
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
        let m = mint(
            owner,
            &bundle,
            ent,
            "auditor",
            &sk.verifying_key(),
            &["read.gamma#kind=action".to_owned()],
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

/// Pin of exactly the upstream-controlled fields approved by the owner.
pub fn manifest_tool_pin(
    name: &str,
    description: Option<&str>,
    input_schema: &serde_json::Value,
) -> Result<String> {
    hash_of(&serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    }))
}

// ---------------------------------------------------------------- helpers

/// Mint one root mandate: `ops` are perimeter entry strings. Every
/// caller passes its constraints explicitly (empty object = none).
#[allow(clippy::too_many_arguments)]
fn mint(
    owner: &OwnerKeys,
    bundle: &Bundle<GatewayStore>,
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
        .map(|op| PerimeterEntry::parse(op).map_err(bridge_err))
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

/// No constraints — the shape most mints use.
fn no_constraints() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn view(e: &aithos_core::gamma::Entry) -> EntryView {
    EntryView {
        id: e.id.clone(),
        at: e.at.clone(),
        kind: e.kind.clone(),
        target: e.target.clone(),
        authorized_via: e.authorized_via.clone(),
        payload: e.payload.clone(),
    }
}

fn hash_of(value: &serde_json::Value) -> Result<String> {
    let canon = aithos_core::jcs::canonical_bytes(value).map_err(bridge_err)?;
    Ok(format!("sha256:{}", aithos_core::gamma::sha256_hex(&canon)))
}

fn read_json<T: serde::de::DeserializeOwned>(
    bundle: &Bundle<GatewayStore>,
    path: &str,
) -> Result<T> {
    let bytes = bundle
        .store
        .get(path)
        .map_err(bridge_err)?
        .ok_or_else(|| GatewayError::BridgeFailed(format!("missing {path}")))?;
    serde_json::from_slice(&bytes).map_err(bridge_err)
}

fn bridge_err(e: impl std::fmt::Display) -> GatewayError {
    GatewayError::BridgeFailed(e.to_string())
}

/// One clear index row of the memory shelf (skeleton data — no body).
struct MemoryRow {
    name: String,
    title: String,
    tags: Vec<String>,
}

/// The memory shelf's clear index rows, oldest first, optionally
/// filtered by a case-insensitive `query` over name/title/tags and an
/// exact `tag`. This reads the SKELETON the readability frontier
/// already grants whoever holds the files — no body is touched here.
fn memory_rows(
    bundle: &Bundle<GatewayStore>,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<MemoryRow>> {
    let folders = bundle
        .resolve_folder(Zone::Circle, MEMORY_FOLDER)
        .map_err(bridge_err)?;
    let memory_sid = folders.last().map(ToString::to_string);
    let index: serde_json::Value = read_json(bundle, "e/circle/index.json")?;
    let needle = query.map(str::to_lowercase);
    let mut rows = Vec::new();
    for row in index["sections"].as_array().into_iter().flatten() {
        if row["folder_sid"].as_str().map(str::to_owned) != memory_sid {
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

/// Map a refused delegated read to the caller-facing verdict: a mandate
/// verdict (perimeter, window, revocation) is a denial; anything else
/// is a bridge failure (never silently empty).
fn read_denied(e: aithos_core::error::Error) -> GatewayError {
    match e {
        aithos_core::error::Error::InvalidMandate(reason) => GatewayError::MandateDenied {
            op: "journal.search".to_owned(),
            reason,
        },
        other => GatewayError::BridgeFailed(other.to_string()),
    }
}

/// Map a refused delegated write to the caller-facing verdict: a mandate
/// verdict (perimeter, window, revocation) is a denial; anything else is
/// an append refusal.
fn write_denied(e: aithos_core::error::Error) -> GatewayError {
    match e {
        aithos_core::error::Error::InvalidMandate(reason) => GatewayError::MandateDenied {
            op: "ethos.write".to_owned(),
            reason,
        },
        other => GatewayError::LogAppendRefused(other.to_string()),
    }
}
