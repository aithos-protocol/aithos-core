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

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use aithos_bundle::bundle::Bundle;
use aithos_bundle::log::{ActionSpec, LogFilter};
use aithos_bundle::Store;
use aithos_core::did::DidDocument;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{verify_chain, GammaQuery, Mandate, MandateSpec, PerimeterEntry};

use crate::config::GatewayConfig;
use crate::keyholder::Keyholder;
use crate::policy::op_for_tool;
use crate::store_adapter::GatewayStore;
use crate::{GatewayError, Result};

/// Entropy seam, re-exported so surfaces (binary, tests) never import
/// the bundle directly: the bridge is the only door to the core.
pub use aithos_bundle::entropy::{EntropySource, OsEntropy, SeqEntropy};

/// Where the bridge keeps its non-secret runtime state in the store.
const STATE_PATH: &str = "gateway/state.json";
/// Where the gateway-held seeds persist between `onboard` and `run`.
/// v1 is local-disk only (the store adapter refuses cloud); move this
/// behind a KMS/sealed keystore before any cloud store lands.
const KEYS_PATH: &str = "gateway/keys.json";
/// Where mandate certificates live in the store.
fn cert_path(id: &str) -> String {
    format!("certs/{id}.json")
}

/// Persisted key material (see `KEYS_PATH` caveat).
#[derive(Serialize, Deserialize)]
struct StoredKeys {
    agent_seed_hex: String,
    gateway_seed_hex: String,
}

/// Non-secret state persisted at onboarding, reloaded by `open`.
#[derive(Debug, Serialize, Deserialize)]
struct BridgeState {
    agent_mandate: String,
    gateway_mandate: String,
    auditor_mandate: String,
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

/// Live bridge: the ethos, the mandate chains and the keyholder,
/// assembled and ready to authorise, log and export.
pub struct Bridge {
    bundle: Bundle<GatewayStore>,
    keyholder: Keyholder,
    agent_chain: Vec<Mandate>,
    gateway_chain: Vec<Mandate>,
    auditor_mandate: Mandate,
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
            auditor_mandate: auditor_mandate.id.clone(),
        };
        bundle
            .store
            .put(
                STATE_PATH,
                &serde_json::to_vec_pretty(&state).map_err(bridge_err)?,
            )
            .map_err(bridge_err)?;
        let keys = StoredKeys {
            agent_seed_hex: hex::encode(keyholder.agent_seed()),
            gateway_seed_hex: hex::encode(keyholder.gateway_seed()),
        };
        bundle
            .store
            .put(KEYS_PATH, &serde_json::to_vec(&keys).map_err(bridge_err)?)
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
            auditor_mandate,
            entropy,
        };
        Ok((bridge, outcome))
    }

    /// Reload a bridge onboarded earlier (the `run` and `audit-export`
    /// paths): state, certs and the gateway-held seeds come from the
    /// store (see `KEYS_PATH` caveat — v1, local disk only).
    pub fn open(store: GatewayStore, entropy: Box<dyn EntropySource + Send>) -> Result<Self> {
        let bundle = Bundle::open(store).map_err(bridge_err)?;
        let keys: StoredKeys = read_json(&bundle, KEYS_PATH)?;
        let keyholder = Keyholder::from_entropy(
            decode_seed(&keys.agent_seed_hex)?,
            decode_seed(&keys.gateway_seed_hex)?,
        );
        let state: BridgeState = read_json(&bundle, STATE_PATH)?;
        let agent = read_json(&bundle, &cert_path(&state.agent_mandate))?;
        let gateway = read_json(&bundle, &cert_path(&state.gateway_mandate))?;
        let auditor_mandate = read_json(&bundle, &cert_path(&state.auditor_mandate))?;
        Ok(Self {
            bundle,
            keyholder,
            agent_chain: vec![agent],
            gateway_chain: vec![gateway],
            auditor_mandate,
            entropy,
        })
    }

    // ------------------------------------------------------------ policy

    /// Is this tool covered by the agent's mandate at `now`? The polite
    /// pre-check — `record_act` re-verifies everything at append time.
    pub fn authorize(&self, tool: &str, now: &str) -> Result<()> {
        let doc = self.did_doc()?;
        verify_chain(&self.agent_chain, &doc, now).map_err(|e| GatewayError::MandateDenied {
            op: op_for_tool(tool),
            reason: e.to_string(),
        })?;
        let covered = self
            .bundle
            .action_covered(
                &self.agent_chain,
                crate::policy::MCP_CONNECTOR,
                &crate::policy::action_name(tool),
            )
            .map_err(bridge_err)?;
        if covered {
            Ok(())
        } else {
            Err(GatewayError::MandateDenied {
                op: op_for_tool(tool),
                reason: "outside the granted perimeter".to_owned(),
            })
        }
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
        let chain = vec![self.auditor_mandate.clone()];
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
            "mandate": self.auditor_mandate.id,
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

// ---------------------------------------------------------------- helpers

/// Mint one root mandate: `ops` are perimeter entry strings.
#[allow(clippy::too_many_arguments)]
fn mint(
    owner: &OwnerKeys,
    bundle: &Bundle<GatewayStore>,
    ent: &mut dyn EntropySource,
    label: &str,
    grantee_pub: &ed25519_dalek::VerifyingKey,
    ops: &[String],
    window: &MandateWindow,
    now: &str,
) -> Result<Mandate> {
    let perimeter = ops
        .iter()
        .map(|op| PerimeterEntry::parse(op).map_err(bridge_err))
        .collect::<Result<Vec<_>>>()?;
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
            constraints: serde_json::Value::Object(serde_json::Map::new()),
            not_before: window.not_before.clone(),
            not_after: window.not_after.clone(),
            issued_at: now.to_owned(),
            nonce: hex::encode(ent.e16()),
        },
    )
    .map_err(bridge_err)
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

fn decode_seed(hex_str: &str) -> Result<[u8; 32]> {
    hex::decode(hex_str)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| GatewayError::BridgeFailed("stored seed is not 32 hex bytes".into()))
}
