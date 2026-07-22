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
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use aithos_bundle::bundle::Bundle;
use aithos_bundle::log::{ActionSpec, InferenceSpec, LogFilter};
use aithos_bundle::remote::{KeySigner, RemoteStore};
use aithos_bundle::Store;
use aithos_core::constraints::verify_max_sessions;
use aithos_core::did::DidDocument;
use aithos_core::header::{Header, Recipient};
use aithos_core::ids::Sid;
use aithos_core::keys::{ed2x, grantee_kex_secret, succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{
    covers_act, verify_chain, verify_chain_revocable, verify_op, ActOp, GammaQuery, Mandate,
    MandateSpec, Op, PerimeterEntry, Verb,
};
use aithos_core::operation::{verify_session, SessionEvidence};
use aithos_core::path::Zone;
use aithos_core::revocation::{chain_revoked_at, revocations, Revocation};

use crate::config::{ContextTools, GatewayConfig, ToolAccess};
use crate::hub::{validate_approved, ApprovedManifest, ApprovedTool, ProposedManifest};
use crate::keyholder::Keyholder;
use crate::policy::{hub_op_for_tool, op_for_tool, Policy};
use crate::store_adapter::{replicate_owner_history, GatewayStore, OwnerReplicationReport};
use crate::{GatewayError, Result};

/// Entropy seam, re-exported so surfaces (binary, tests) never import
/// the bundle directly: the bridge is the only door to the core.
pub use aithos_bundle::entropy::{EntropySource, OsEntropy, SeqEntropy};
/// Raw store trait, re-exported through the same single door — what
/// owner/test-side surgery uses to read or doctor a store it holds.
pub use aithos_bundle::Store as RawStore;

mod control;
pub use control::{
    prepare_control_envelope, valid_control_gamma_kind, ControlAccess, ControlAuthError,
    ControlContextProof, ControlHeadsProof, ControlPage, ControlPrincipal, ControlProofReader,
    ControlRawArtifact, PreparedControlEnvelope,
};

/// Where the bridge keeps its non-secret runtime state in the store.
pub const STATE_PATH: &str = "gateway/state.json";
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

/// Verify SC1 and the two independent proofs over the same operation_ref.
/// No session-specific verifier is implemented in the gateway.
pub fn verify_delegated_session(
    evidence: DelegatedSessionEvidence<'_>,
) -> Result<serde_json::Value> {
    verify_session(SessionEvidence {
        mandate: evidence.mandate,
        certificate: evidence.certificate,
        projection: evidence.projection,
        operation_ref: evidence.operation_ref,
        native_leaf_proof: evidence.native_leaf_proof,
        native_leaf_domain: MCP_SESSION_NATIVE_PROOF_DOMAIN,
        session_proof: evidence.session_proof,
    })
    .map(|verified| verified.operation_ref().clone())
    .map_err(|error| GatewayError::MandateDenied {
        op: "delegated_session".into(),
        reason: error.to_string(),
    })
}

/// Apply Core's active-session tally to an injected, already-verified set.
pub fn enforce_max_sessions(max_sessions: u64, active_session_keys: &[&str]) -> Result<usize> {
    verify_max_sessions(max_sessions, active_session_keys)
        .map(|verified| verified.active())
        .map_err(|error| GatewayError::MandateDenied {
            op: "delegated_session".into(),
            reason: error.to_string(),
        })
}
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

/// Build the one bounded C2/B.2 registration line under the existing
/// gateway identity. The private seed remains inside the bridge; callers
/// cannot ask the keyholder to sign arbitrary bytes.
pub fn gateway_tunnel_registration_line(
    kh: &Keyholder,
    tenant: &str,
    hostname: &str,
    at: &str,
    nonce: &str,
) -> Result<Vec<u8>> {
    let signing = SigningKey::from_bytes(kh.gateway_seed());
    crate::relay::registration_line_with_key(
        tenant,
        hostname,
        &gateway_pub_multibase(kh),
        at,
        nonce,
        &signing,
    )
}

/// Build the bounded B.5 `X-Aithos-Auth` value for the delegated
/// DNS-01 endpoint. This is deliberately not a general-purpose signing
/// primitive: the path, version, mandate set and signature algorithm are
/// fixed here, and only PUT/DELETE challenge effects are admitted.
pub fn gateway_acme_authorization_header(
    kh: &Keyholder,
    host: &str,
    method: &str,
    body: &[u8],
    at: &str,
    nonce: &str,
) -> Result<String> {
    use ed25519_dalek::Signer as _;

    if !matches!(method, "PUT" | "DELETE")
        || host.is_empty()
        || host
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte <= b' ')
        || at.is_empty()
        || nonce.len() < 16
        || nonce.len() > 64
        || nonce.contains('\n')
    {
        return Err(GatewayError::RelayUnavailable(
            "acme_authorization_input_invalid".into(),
        ));
    }
    let signing = SigningKey::from_bytes(kh.gateway_seed());
    let mut envelope = serde_json::json!({
        "v": 1,
        "host": host,
        "method": method,
        "path": "/acme/txt",
        "body_b3": blake3::hash(body).to_hex().to_string(),
        "at": at,
        "nonce": nonce,
        "mandate": [],
        "key": gateway_pub_multibase(kh),
        "signature": { "alg": "ed25519", "value": "" },
    });
    let unsigned = aithos_core::jcs::canonicalize(&envelope)
        .map_err(|_| GatewayError::RelayUnavailable("acme_authorization_encode_failed".into()))?;
    envelope["signature"]["value"] =
        serde_json::Value::String(hex::encode(signing.sign(unsigned.as_bytes()).to_bytes()));
    let signed = aithos_core::jcs::canonicalize(&envelope)
        .map_err(|_| GatewayError::RelayUnavailable("acme_authorization_encode_failed".into()))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signed.as_bytes()))
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
        let Ok(paths) = self.bundle.store.list("certs/") else {
            return Vec::new();
        };
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
        let mut chains: Vec<Vec<Mandate>> = Vec::new();
        for leaf in by_id.values() {
            if leaf.grantee.pubkey != agent_pub {
                continue;
            }
            let has_ethos_entry = leaf
                .parsed_perimeter()
                .map(|entries| {
                    entries
                        .iter()
                        .any(|entry| matches!(entry, PerimeterEntry::Ethos { .. }))
                })
                .unwrap_or(false);
            if !has_ethos_entry {
                continue;
            }
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
                        break;
                    }
                }
            }
            if resolvable {
                chains.push(chain);
            }
        }
        chains
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
            hot_server_pins: BTreeMap::new(),
        }
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
        self.validate_hot_connector(context, connector, manifest)?;
        self.hot_tools.retain(|_, tool| tool.server != connector);
        for approved in &manifest.tools {
            self.hot_tools.insert(
                approved.exposed_name.clone(),
                HubRuntimeTool {
                    context: context.to_owned(),
                    server: connector.to_owned(),
                    raw_tool: approved.name.clone(),
                    description: approved.description.clone(),
                    input_schema: approved.input_schema.clone(),
                    pin_sha256: approved.pin_sha256.clone(),
                    access: approved.risk_class,
                    granted: approved.is_granted(),
                    bounds: approved.bounds.clone(),
                },
            );
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

    pub fn remove_hot_connector(&mut self, connector: &str) {
        self.hot_tools.retain(|_, tool| tool.server != connector);
        self.hot_server_pins.remove(connector);
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
            if hub.context != ctx {
                return Err(GatewayError::ConfigRejected(format!(
                    "hub route `{tool}` resolved to `{ctx}`, pin belongs to `{}`",
                    hub.context
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

fn validate_runtime_tool(
    context: &str,
    exposed: &str,
    reference: &crate::config::HubToolRef,
    approved: &ApprovedTool,
) -> Result<()> {
    if approved.exposed_name != exposed {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` names `{exposed}`, approved manifest names `{}`",
            approved.exposed_name
        )));
    }
    if approved.risk_class != reference.access {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` class for `{exposed}` differs from the approved manifest"
        )));
    }
    if reference.is_granted() != approved.is_granted() {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` grant decision for `{exposed}` differs from the approved manifest"
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
    let state: BridgeState = read_json(&bundle, STATE_PATH)?;
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
        let mut fresh: BridgeState = read_json(&bundle, STATE_PATH)?;
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
    let mut state: BridgeState = read_json(&bundle, STATE_PATH)?;
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

/// Owner-side read of one zone's directive (test/ops assertions — the
/// sovereignty mirror of `owner_read_journal_note`).
pub fn owner_read_briefing(
    master: &[u8; 32],
    label: &str,
    zone: &str,
    store: GatewayStore,
) -> Result<String> {
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
    let bundle = Bundle::open(store).map_err(bridge_err)?;
    bundle
        .read_section(
            zone,
            &format!("{BRIEFING_FOLDER}/{BRIEFING_SECTION}"),
            &owner,
        )
        .map_err(bridge_err)
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
    let state: BridgeState = read_json(&bundle, STATE_PATH)?;
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

/// Stable digest of the exact upstream-controlled catalogue sealed in an
/// owner-approved H3 manifest. Risk/grant/bounds remain owner policy and do
/// not alter the live-discovery comparison digest.
pub fn approved_manifest_catalog_digest(manifest: &ApprovedManifest) -> Result<String> {
    validate_approved(manifest)?;
    manifest_catalog_digest(
        &manifest.server,
        manifest
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.pin_sha256.as_str())),
    )
}

pub fn proposed_manifest_catalog_digest(manifest: &ProposedManifest) -> Result<String> {
    manifest_catalog_digest(
        &manifest.server,
        manifest
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.pin_sha256.as_str())),
    )
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
    let bundle = Bundle::open(store.clone()).map_err(bridge_err)?;
    let state: BridgeState = read_json(&bundle, STATE_PATH)?;
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

/// The memory shelf's clear index rows, oldest first — see [`zone_rows`].
fn memory_rows(
    bundle: &Bundle<GatewayStore>,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<MemoryRow>> {
    zone_rows(bundle, Zone::Circle, MEMORY_FOLDER, query, tag)
}

/// One zone folder's clear index rows, oldest first, optionally filtered
/// by a case-insensitive `query` over name/title/tags and an exact
/// `tag`. This reads the SKELETON the readability frontier already
/// grants whoever holds the files — no body is touched here. A folder
/// that does not exist yields no rows (nothing was ever shelved there).
fn zone_rows(
    bundle: &Bundle<GatewayStore>,
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

/// One agent-facing row of a zone index (lot G6): display path, clear
/// title and tags, and the folder sid-path (what `covers()` walks).
#[derive(Debug, Clone)]
struct EthosRow {
    path: String,
    title: String,
    tags: Vec<String>,
    folders: Vec<Sid>,
}

/// The whole CLEAR index of one zone, display paths resolved — the
/// readability frontier (§02.1: the gateway holds the files); AUTHORITY
/// is checked per row by the callers. A zone with no index yields no
/// rows. `self` never goes through here in v1: its structure is sealed
/// and the delegated resolution is its own core lot. The `briefing/`
/// shelves are EXCLUDED: the owner's directives keep their own
/// dedicated surface (`briefing.read`, lot K) — the data tools serve
/// the rest of the Ethos, and the demo hot path stays byte-identical.
fn zone_all_rows(bundle: &Bundle<GatewayStore>, zone: Zone) -> Vec<EthosRow> {
    let Ok(index) =
        read_json::<serde_json::Value>(bundle, &format!("e/{}/index.json", zone.as_str()))
    else {
        return Vec::new();
    };
    let mut parents: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for folder in index["folders"].as_array().into_iter().flatten() {
        let sid = folder["sid"].as_str().unwrap_or_default().to_owned();
        let name = folder["name"].as_str().unwrap_or_default().to_owned();
        let parent = folder["parent_sid"].as_str().map(str::to_owned);
        parents.insert(sid, (name, parent));
    }
    let resolve = |folder_sid: Option<&str>| -> Option<(Vec<String>, Vec<Sid>)> {
        let mut names = Vec::new();
        let mut sids = Vec::new();
        let mut cursor = folder_sid.map(str::to_owned);
        while let Some(sid) = cursor {
            let (name, parent) = parents.get(&sid)?.clone();
            sids.insert(0, Sid::parse(&sid).ok()?);
            names.insert(0, name);
            cursor = parent;
        }
        Some((names, sids))
    };
    let mut rows = Vec::new();
    for row in index["sections"].as_array().into_iter().flatten() {
        let name = row["name"].as_str().unwrap_or_default().to_owned();
        let Some((mut names, sids)) = resolve(row["folder_sid"].as_str()) else {
            continue;
        };
        names.push(name);
        if names.first().map(String::as_str) == Some(BRIEFING_FOLDER) {
            continue;
        }
        let tags: Vec<String> = row["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        rows.push(EthosRow {
            path: names.join("/"),
            title: row["title"].as_str().unwrap_or_default().to_owned(),
            tags,
            folders: sids,
        });
    }
    rows
}

/// Map a refused delegated read to the caller-facing verdict: a mandate
/// verdict (perimeter, window, revocation) is a denial; anything else
/// is a bridge failure (never silently empty).
fn read_denied_op(op: &'static str) -> impl Fn(aithos_core::error::Error) -> GatewayError {
    move |e| match e {
        aithos_core::error::Error::InvalidMandate(reason) => GatewayError::MandateDenied {
            op: op.to_owned(),
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

#[cfg(test)]
mod delegated_session_tests {
    use super::*;

    fn vector() -> serde_json::Value {
        serde_json::from_str(include_str!("../../../../vectors/cb2-session-proof.json"))
            .expect("historical CB2 vector parses")
    }

    #[test]
    fn gateway_bridge_consumes_the_historical_sc1_vector_exactly() {
        let vector = vector();
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
}
