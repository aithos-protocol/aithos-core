//! G7 control-plane bridge.
//!
//! This stays a child of `core_bridge`: transport code never imports Core or
//! Bundle directly. A.2 form/binding is consumed byte-for-byte here; Core keeps
//! every cryptographic, mandate, revocation and Gamma authority verdict.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aithos_bundle::Store;
use aithos_core::did::DidDocument;
use aithos_core::gamma::{Entry, Kind};
use aithos_core::gamma_replay::GammaReplayState;
use aithos_core::ids::Sid;
use aithos_core::mandate::{covers_gamma_query, verify_chain, GammaQuery, Mandate, PerimeterEntry};
use aithos_core::revocation::{chain_revoked_at, revocations};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::Runner;
use crate::store_adapter::GatewayStore;
use crate::{GatewayError, Result};

pub const MAX_CONTROL_ENVELOPE_BYTES: usize = 8 * 1024;
pub const CONTROL_SKEW_MS: i64 = 300_000;
const MAX_CHAIN_DEPTH: usize = 16;
const MAX_DID_BYTES: usize = 64 * 1024;
const MAX_CERT_BYTES: usize = 256 * 1024;
const MAX_CERTIFICATES: usize = 4_096;
const MAX_GAMMA_SEGMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_GAMMA_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_GAMMA_SEGMENTS: usize = 512;
const MAX_GAMMA_ENTRY_BYTES: usize = 256 * 1024;
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_PROOF_PAGE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlEnvelope {
    v: u8,
    host: String,
    method: String,
    path: String,
    body_b3: String,
    at: String,
    nonce: String,
    mandate: Vec<String>,
    key: String,
    signature: ControlEnvelopeSignature,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlEnvelopeSignature {
    alg: String,
    value: String,
}

/// Stable internal refusal classes. HTTP deliberately collapses all authority
/// faults to one redacted public code; only availability is distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAuthError {
    Invalid,
    ClockSkew,
    SignatureInvalid,
    ChainInvalid,
    ChainRevoked,
    NotCovered,
    Unavailable,
}

/// A form-checked, byte-bound A.2 envelope. Nonce reservation deliberately
/// happens between `prepare_control_envelope` and authority verification.
pub struct PreparedControlEnvelope {
    envelope: ControlEnvelope,
}

impl PreparedControlEnvelope {
    pub fn key(&self) -> &str {
        &self.envelope.key
    }

    pub fn nonce(&self) -> &str {
        &self.envelope.nonce
    }
}

/// The closed control operation for which authority is being established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlAccess {
    Status,
    Contexts,
    Certificates {
        context: String,
    },
    Gamma {
        context: String,
        kind: Option<String>,
    },
    Heads {
        context: String,
    },
    /// Owner-only list of bindings in the context selected by the owner's
    /// signing key. No registry read is needed to choose that context.
    Connectors,
    /// G7b consumes this already-closed authority seam. A wildcard act does
    /// not cover connector binding: the leaf must name `config` literally.
    ConnectorConfig {
        context: String,
        connector: String,
    },
    /// Connector routes classify before any registry read. Authority is
    /// therefore checked across contexts; the exact owner key or exact
    /// `act.x.<connector>.config` leaf selects one context fail-closed.
    ConnectorConfigAny {
        connector: String,
    },
    /// Plaintext approval review and external-effect decisions are never
    /// covered by the delegated connector-config perimeter.
    ConnectorOwnerAny {
        connector: String,
    },
}

impl ControlAccess {
    fn context(&self) -> Option<&str> {
        match self {
            Self::Status
            | Self::Contexts
            | Self::Connectors
            | Self::ConnectorConfigAny { .. }
            | Self::ConnectorOwnerAny { .. } => None,
            Self::Certificates { context }
            | Self::Gamma { context, .. }
            | Self::Heads { context }
            | Self::ConnectorConfig { context, .. } => Some(context),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ControlRole {
    Owner,
    Auditor,
    ConnectorConfig,
}

/// Verified authority passed to handlers. Core perimeter entries remain private
/// to this bridge and can only be consumed by the proof methods below.
#[derive(Clone)]
pub struct ControlPrincipal {
    context: String,
    role: ControlRole,
    gamma_grants: Vec<PerimeterEntry>,
    certificate_ids: BTreeSet<String>,
    did_b3: String,
    principal_b3: String,
}

impl ControlPrincipal {
    pub fn context(&self) -> &str {
        &self.context
    }

    pub fn is_owner(&self) -> bool {
        self.role == ControlRole::Owner
    }

    /// Stable verified principal used only to derive isolated connector
    /// custody. Public DTOs never expose this DID.
    pub fn principal_id(&self) -> &str {
        &self.principal_b3
    }

    fn permits_gamma_entry(&self, entry: &Entry) -> bool {
        role_permits_gamma_entry(self.role, &self.gamma_grants, entry)
    }
}

fn role_permits_gamma_entry(
    role: ControlRole,
    gamma_grants: &[PerimeterEntry],
    entry: &Entry,
) -> bool {
    if role == ControlRole::Owner {
        return true;
    }
    let action = entry
        .payload
        .as_ref()
        .and_then(|payload| payload.get("action"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let query = GammaQuery {
        kind: Some(entry.kind.clone()),
        action,
        since: Some(entry.at.clone()),
        until: Some(entry.at.clone()),
        ..GammaQuery::default()
    };
    covers_gamma_query(gamma_grants, &query)
}

#[derive(Clone)]
struct ControlContext {
    did: String,
    store: GatewayStore,
}

/// Fresh read-side view over every configured context store.
#[derive(Clone)]
pub struct ControlProofReader {
    contexts: Arc<BTreeMap<String, ControlContext>>,
}

/// One exact stored artifact. Transport base64url-encodes these bytes without
/// parsing or reserializing them.
#[derive(Clone)]
pub struct ControlRawArtifact {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub struct ControlPage {
    pub items: Vec<ControlRawArtifact>,
    pub next_offset: Option<usize>,
}

pub struct ControlContextProof {
    pub name: String,
    pub did: String,
    pub did_document: ControlRawArtifact,
}

pub struct ControlHeadsProof {
    pub context: String,
    pub did: String,
    pub manifest: Option<ControlRawArtifact>,
    pub gamma_tail: Option<ControlRawArtifact>,
}

struct AuthoritySnapshot {
    gamma: Vec<StoredGammaEntry>,
}

struct StoredGammaEntry {
    path: String,
    bytes: Vec<u8>,
    entry: Entry,
}

/// A.2 #2–#5: exact form/JCS, host/method/path/body binding and inclusive
/// ±300-second skew. The caller must reserve `(key, nonce)` immediately after
/// this returns and before invoking `verify_authority`.
pub fn prepare_control_envelope(
    header: &str,
    authority: &str,
    expected_authorities: &BTreeSet<String>,
    method: &str,
    target: &str,
    body: &[u8],
    now_ms: i64,
) -> std::result::Result<PreparedControlEnvelope, ControlAuthError> {
    if header.len() > MAX_CONTROL_ENVELOPE_BYTES {
        return Err(ControlAuthError::Invalid);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(header.as_bytes())
        .map_err(|_| ControlAuthError::Invalid)?;
    let envelope: ControlEnvelope =
        serde_json::from_slice(&bytes).map_err(|_| ControlAuthError::Invalid)?;
    let canonical =
        aithos_core::jcs::canonical_bytes(&envelope).map_err(|_| ControlAuthError::Invalid)?;
    if canonical != bytes
        || envelope.v != 1
        || envelope.signature.alg != "ed25519"
        || envelope.nonce.is_empty()
        || envelope.nonce.len() > 64
        || !valid_key_shape(&envelope.key)
        || envelope.host != authority
        || !expected_authorities.contains(authority)
        || envelope.method != method
        || envelope.path != target
    {
        return Err(ControlAuthError::Invalid);
    }
    let want_body_b3 = if body.is_empty() {
        String::new()
    } else {
        blake3::hash(body).to_hex().to_string()
    };
    if envelope.body_b3 != want_body_b3 {
        return Err(ControlAuthError::Invalid);
    }
    let at_ms = parse_rfc3339z_ms(&envelope.at).ok_or(ControlAuthError::Invalid)?;
    if now_ms.abs_diff(at_ms) > CONTROL_SKEW_MS as u64 {
        return Err(ControlAuthError::ClockSkew);
    }
    Ok(PreparedControlEnvelope { envelope })
}

impl ControlProofReader {
    /// Build from the already-open runtime. Store clones share the exact fs,
    /// memory or remote state; no second truth or cache is introduced.
    pub fn from_runner(runner: &Runner) -> Self {
        let contexts = runner
            .contexts
            .iter()
            .map(|(name, runtime)| {
                (
                    name.clone(),
                    ControlContext {
                        did: runtime.bridge.bundle.did.clone(),
                        store: runtime.bridge.bundle.store.clone(),
                    },
                )
            })
            .collect();
        Self {
            contexts: Arc::new(contexts),
        }
    }

    /// Test/operator seam for stores that were provisioned before a runner is
    /// opened. Each DID is verified at construction and again per request.
    pub fn from_stores(stores: BTreeMap<String, GatewayStore>) -> Result<Self> {
        let mut contexts = BTreeMap::new();
        for (name, store) in stores {
            let bytes = get_required_bounded(&store, "did.json", MAX_DID_BYTES)?;
            let doc: DidDocument = serde_json::from_slice(&bytes)
                .map_err(|_| redacted_bridge_error("invalid control DID document"))?;
            doc.verify()
                .map_err(|_| redacted_bridge_error("invalid control DID document"))?;
            contexts.insert(name, ControlContext { did: doc.id, store });
        }
        Ok(Self {
            contexts: Arc::new(contexts),
        })
    }

    /// A.2 #7–#10 after the caller has burned the nonce. DID, certificates,
    /// Gamma replay and revocations are reread on every invocation.
    pub fn verify_authority(
        &self,
        prepared: &PreparedControlEnvelope,
        access: &ControlAccess,
        now_ms: i64,
    ) -> std::result::Result<ControlPrincipal, ControlAuthError> {
        let candidates: Vec<(&String, &ControlContext)> = match access.context() {
            Some(name) => self.contexts.get_key_value(name).into_iter().collect(),
            None => self.contexts.iter().collect(),
        };
        if candidates.is_empty() {
            return Err(ControlAuthError::NotCovered);
        }
        let mut unavailable = false;
        let mut strongest = ControlAuthError::SignatureInvalid;
        for (name, context) in candidates {
            match verify_in_context(name, context, prepared, access, now_ms) {
                Ok(principal) => return Ok(principal),
                Err(ControlAuthError::Unavailable) => unavailable = true,
                Err(ControlAuthError::ChainRevoked) => strongest = ControlAuthError::ChainRevoked,
                Err(ControlAuthError::NotCovered)
                    if strongest != ControlAuthError::ChainRevoked =>
                {
                    strongest = ControlAuthError::NotCovered;
                }
                Err(ControlAuthError::ChainInvalid)
                    if !matches!(
                        strongest,
                        ControlAuthError::ChainRevoked | ControlAuthError::NotCovered
                    ) =>
                {
                    strongest = ControlAuthError::ChainInvalid;
                }
                Err(_) => {}
            }
        }
        if unavailable {
            Err(ControlAuthError::Unavailable)
        } else {
            Err(strongest)
        }
    }

    pub fn contexts(&self, principal: &ControlPrincipal) -> Result<Vec<ControlContextProof>> {
        let context = self.checked_context(principal)?;
        let did_document = get_required_bounded(&context.store, "did.json", MAX_DID_BYTES)?;
        Ok(vec![ControlContextProof {
            name: principal.context.clone(),
            did: context.did.clone(),
            did_document: ControlRawArtifact {
                path: "did.json".to_owned(),
                bytes: did_document,
            },
        }])
    }

    pub fn certificates(
        &self,
        principal: &ControlPrincipal,
        offset: usize,
        limit: usize,
    ) -> Result<ControlPage> {
        let context = self.checked_context(principal)?;
        let mut paths = bounded_paths(&context.store, "certs/", MAX_CERTIFICATES)?;
        paths.retain(|path| path.ends_with(".json"));
        if !principal.is_owner() {
            paths.retain(|path| {
                path.strip_prefix("certs/")
                    .and_then(|path| path.strip_suffix(".json"))
                    .is_some_and(|id| principal.certificate_ids.contains(id))
            });
        }
        paginate_artifacts(&context.store, paths, offset, limit, MAX_CERT_BYTES)
    }

    pub fn gamma(
        &self,
        principal: &ControlPrincipal,
        kind: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<ControlPage> {
        let context = self.checked_context(principal)?;
        let snapshot = load_authority_snapshot(context)?;
        let visible: Vec<ControlRawArtifact> = snapshot
            .gamma
            .into_iter()
            .filter(|stored| {
                kind.is_none_or(|kind| stored.entry.kind == kind)
                    && principal.permits_gamma_entry(&stored.entry)
            })
            .map(|stored| ControlRawArtifact {
                path: stored.path,
                bytes: stored.bytes,
            })
            .collect();
        paginate_loaded(visible, offset, limit)
    }

    pub fn heads(&self, principal: &ControlPrincipal) -> Result<ControlHeadsProof> {
        let context = self.checked_context(principal)?;
        let snapshot = load_authority_snapshot(context)?;
        let gamma_tail = snapshot
            .gamma
            .into_iter()
            .rev()
            .find(|stored| principal.permits_gamma_entry(&stored.entry))
            .map(|stored| ControlRawArtifact {
                path: stored.path,
                bytes: stored.bytes,
            });
        let manifest = if principal.is_owner() {
            get_optional_bounded(&context.store, "manifest.json", MAX_MANIFEST_BYTES)?.map(
                |bytes| ControlRawArtifact {
                    path: "manifest.json".to_owned(),
                    bytes,
                },
            )
        } else {
            None
        };
        Ok(ControlHeadsProof {
            context: principal.context.clone(),
            did: context.did.clone(),
            manifest,
            gamma_tail,
        })
    }

    fn checked_context(&self, principal: &ControlPrincipal) -> Result<&ControlContext> {
        let context = self
            .contexts
            .get(&principal.context)
            .ok_or_else(|| redacted_bridge_error("control context unavailable"))?;
        let did = get_required_bounded(&context.store, "did.json", MAX_DID_BYTES)?;
        if blake3::hash(&did).to_hex().as_str() != principal.did_b3 {
            return Err(redacted_bridge_error(
                "control context changed during request",
            ));
        }
        Ok(context)
    }
}

fn verify_in_context(
    name: &str,
    context: &ControlContext,
    prepared: &PreparedControlEnvelope,
    access: &ControlAccess,
    now_ms: i64,
) -> std::result::Result<ControlPrincipal, ControlAuthError> {
    let did_bytes = get_required_bounded(&context.store, "did.json", MAX_DID_BYTES)
        .map_err(|_| ControlAuthError::Unavailable)?;
    let doc: DidDocument =
        serde_json::from_slice(&did_bytes).map_err(|_| ControlAuthError::ChainInvalid)?;
    if doc.id != context.did || doc.verify().is_err() {
        return Err(ControlAuthError::ChainInvalid);
    }
    let did_b3 = blake3::hash(&did_bytes).to_hex().to_string();
    if matches!(prepared.envelope.key.as_str(), "#root" | "#content") {
        let encoded = if prepared.envelope.key == "#root" {
            &doc.keys.root
        } else {
            &doc.keys.content
        };
        let key = decode_verifying_key(encoded)?;
        verify_envelope_signature(&prepared.envelope, &key)?;
        return Ok(ControlPrincipal {
            context: name.to_owned(),
            role: ControlRole::Owner,
            gamma_grants: Vec::new(),
            certificate_ids: BTreeSet::new(),
            principal_b3: did_b3.clone(),
            did_b3,
        });
    }

    if prepared.envelope.mandate.is_empty() || prepared.envelope.mandate.len() > MAX_CHAIN_DEPTH {
        return Err(ControlAuthError::ChainInvalid);
    }
    let mut chain = Vec::with_capacity(prepared.envelope.mandate.len());
    for id in &prepared.envelope.mandate {
        if !valid_mandate_id(id) {
            return Err(ControlAuthError::ChainInvalid);
        }
        let path = format!("certs/{id}.json");
        let bytes = context
            .store
            .get_bounded(&path, MAX_CERT_BYTES)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::InvalidData {
                    ControlAuthError::ChainInvalid
                } else {
                    ControlAuthError::Unavailable
                }
            })?
            .ok_or(ControlAuthError::ChainInvalid)?;
        let mandate: Mandate =
            serde_json::from_slice(&bytes).map_err(|_| ControlAuthError::ChainInvalid)?;
        if mandate.id != *id {
            return Err(ControlAuthError::ChainInvalid);
        }
        chain.push(mandate);
    }
    let leaf = chain.last().ok_or(ControlAuthError::ChainInvalid)?;
    if leaf.grantee.pubkey != prepared.envelope.key {
        return Err(ControlAuthError::ChainInvalid);
    }
    let key = leaf
        .grantee_pub()
        .map_err(|_| ControlAuthError::ChainInvalid)?;
    verify_envelope_signature(&prepared.envelope, &key)?;
    verify_chain(&chain, &doc, &prepared.envelope.at)
        .map_err(|_| ControlAuthError::ChainInvalid)?;
    // Revocation/Gamma resolution is deliberately after signature and chain
    // validity, matching A.2's normative order and keeping invalid signatures
    // away from protected proof reads.
    let snapshot = load_authority_snapshot(context).map_err(|_| ControlAuthError::Unavailable)?;
    let verified_entries: Vec<Entry> = snapshot
        .gamma
        .iter()
        .map(|stored| stored.entry.clone())
        .collect();
    chain_revoked_at(
        &chain,
        &revocations(&verified_entries),
        &render_rfc3339z(now_ms),
    )
    .map_err(|_| ControlAuthError::ChainRevoked)?;
    let perimeter = leaf
        .parsed_perimeter()
        .map_err(|_| ControlAuthError::ChainInvalid)?;
    let gamma_grants: Vec<PerimeterEntry> = perimeter
        .iter()
        .filter(|entry| matches!(entry, PerimeterEntry::Gamma { .. }))
        .cloned()
        .collect();
    let role = match access {
        ControlAccess::Status | ControlAccess::Connectors => {
            return Err(ControlAuthError::NotCovered)
        }
        ControlAccess::Contexts
        | ControlAccess::Certificates { .. }
        | ControlAccess::Heads { .. }
            if !gamma_grants.is_empty() =>
        {
            ControlRole::Auditor
        }
        ControlAccess::Gamma { kind, .. } => {
            let query = GammaQuery {
                kind: kind.clone(),
                ..GammaQuery::default()
            };
            if !covers_gamma_query(&gamma_grants, &query) {
                return Err(ControlAuthError::NotCovered);
            }
            ControlRole::Auditor
        }
        ControlAccess::ConnectorOwnerAny { .. } => return Err(ControlAuthError::NotCovered),
        ControlAccess::ConnectorConfig { connector, .. }
        | ControlAccess::ConnectorConfigAny { connector }
            if perimeter.iter().any(|entry| {
                matches!(
                    entry,
                    PerimeterEntry::Act {
                        connector: granted,
                        action: Some(action),
                    } if granted == connector && action == "config"
                )
            }) =>
        {
            ControlRole::ConnectorConfig
        }
        _ => return Err(ControlAuthError::NotCovered),
    };
    let mut certificate_ids: BTreeSet<String> = prepared.envelope.mandate.iter().cloned().collect();
    for stored in &snapshot.gamma {
        if role_permits_gamma_entry(role, &gamma_grants, &stored.entry) {
            certificate_ids.extend(stored.entry.authorized_via.iter().flatten().cloned());
        }
    }
    let mut principal_material = String::from("aithos-control-principal-v1\0");
    principal_material.push_str(&prepared.envelope.key);
    let principal_b3 = blake3::hash(principal_material.as_bytes())
        .to_hex()
        .to_string();
    Ok(ControlPrincipal {
        context: name.to_owned(),
        role,
        gamma_grants,
        certificate_ids,
        did_b3,
        principal_b3,
    })
}

fn load_authority_snapshot(context: &ControlContext) -> Result<AuthoritySnapshot> {
    let did_bytes = get_required_bounded(&context.store, "did.json", MAX_DID_BYTES)?;
    let doc: DidDocument = serde_json::from_slice(&did_bytes)
        .map_err(|_| redacted_bridge_error("invalid control authority snapshot"))?;
    if doc.id != context.did || doc.verify().is_err() {
        return Err(redacted_bridge_error("invalid control authority snapshot"));
    }

    let cert_paths = bounded_paths(&context.store, "certs/", MAX_CERTIFICATES)?;
    let mut certificates = BTreeMap::new();
    for path in cert_paths
        .into_iter()
        .filter(|path| path.ends_with(".json"))
    {
        let bytes = get_required_bounded(&context.store, &path, MAX_CERT_BYTES)?;
        let mandate: Mandate = serde_json::from_slice(&bytes)
            .map_err(|_| redacted_bridge_error("invalid control authority snapshot"))?;
        if path != format!("certs/{}.json", mandate.id)
            || certificates.insert(mandate.id.clone(), mandate).is_some()
        {
            return Err(redacted_bridge_error("invalid control authority snapshot"));
        }
    }

    let gamma_paths = bounded_paths(&context.store, "gamma/", MAX_GAMMA_SEGMENTS)?;
    let mut gamma = Vec::new();
    let mut total = 0usize;
    for path in gamma_paths
        .into_iter()
        .filter(|path| path.ends_with(".jsonl"))
    {
        let segment = get_required_bounded(&context.store, &path, MAX_GAMMA_SEGMENT_BYTES)?;
        total = total
            .checked_add(segment.len())
            .ok_or_else(|| redacted_bridge_error("control Gamma exceeds bounds"))?;
        if total > MAX_GAMMA_TOTAL_BYTES {
            return Err(redacted_bridge_error("control Gamma exceeds bounds"));
        }
        for (line_number, line) in segment
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .enumerate()
        {
            if line.len() > MAX_GAMMA_ENTRY_BYTES {
                return Err(redacted_bridge_error("control Gamma entry exceeds bounds"));
            }
            let entry: Entry = serde_json::from_slice(line)
                .map_err(|_| redacted_bridge_error("invalid control Gamma"))?;
            gamma.push(StoredGammaEntry {
                path: format!("{path}#{}", line_number + 1),
                bytes: line.to_vec(),
                entry,
            });
        }
    }
    let mut replay = GammaReplayState::new(doc.clone(), certificates.clone());
    for stored in &gamma {
        replay
            .admit(&stored.entry)
            .map_err(|_| redacted_bridge_error("invalid control Gamma"))?;
    }
    replay
        .finish()
        .map_err(|_| redacted_bridge_error("invalid control Gamma"))?;
    Ok(AuthoritySnapshot { gamma })
}

fn paginate_artifacts(
    store: &GatewayStore,
    paths: Vec<String>,
    offset: usize,
    limit: usize,
    item_limit: usize,
) -> Result<ControlPage> {
    if offset > paths.len() {
        return Err(redacted_bridge_error("invalid control proof cursor"));
    }
    let total_paths = paths.len();
    let mut items = Vec::new();
    let mut bytes = 0usize;
    let mut next = offset;
    for path in paths.into_iter().skip(offset).take(limit) {
        let body = get_required_bounded(store, &path, item_limit)?;
        bytes = bytes
            .checked_add(body.len())
            .ok_or_else(|| redacted_bridge_error("control proof page exceeds bounds"))?;
        if bytes > MAX_PROOF_PAGE_BYTES {
            if items.is_empty() {
                return Err(redacted_bridge_error("control proof page exceeds bounds"));
            }
            break;
        }
        items.push(ControlRawArtifact { path, bytes: body });
        next += 1;
    }
    let next_offset = (next < total_paths).then_some(next);
    Ok(ControlPage { items, next_offset })
}

fn paginate_loaded(
    items: Vec<ControlRawArtifact>,
    offset: usize,
    limit: usize,
) -> Result<ControlPage> {
    if offset > items.len() {
        return Err(redacted_bridge_error("invalid control proof cursor"));
    }
    let total_items = items.len();
    let mut page = Vec::new();
    let mut bytes = 0usize;
    let mut next = offset;
    for item in items.into_iter().skip(offset).take(limit) {
        bytes = bytes
            .checked_add(item.bytes.len())
            .ok_or_else(|| redacted_bridge_error("control proof page exceeds bounds"))?;
        if bytes > MAX_PROOF_PAGE_BYTES {
            if page.is_empty() {
                return Err(redacted_bridge_error("control proof page exceeds bounds"));
            }
            break;
        }
        page.push(item);
        next += 1;
    }
    Ok(ControlPage {
        items: page,
        next_offset: (next < total_items).then_some(next),
    })
}

fn bounded_paths(store: &GatewayStore, prefix: &str, maximum: usize) -> Result<Vec<String>> {
    let mut paths = store
        .list(prefix)
        .map_err(|_| redacted_bridge_error("control proof store unavailable"))?;
    if paths.len() > maximum
        || paths
            .iter()
            .any(|path| path.len() > 512 || !path.starts_with(prefix))
    {
        return Err(redacted_bridge_error("control proof store exceeds bounds"));
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn get_required_bounded(store: &GatewayStore, path: &str, maximum: usize) -> Result<Vec<u8>> {
    get_optional_bounded(store, path, maximum)?
        .ok_or_else(|| redacted_bridge_error("control proof artifact unavailable"))
}

fn get_optional_bounded(
    store: &GatewayStore,
    path: &str,
    maximum: usize,
) -> Result<Option<Vec<u8>>> {
    let bytes = store
        .get_bounded(path, maximum)
        .map_err(|_| redacted_bridge_error("control proof store unavailable"))?;
    Ok(bytes)
}

fn verify_envelope_signature(
    envelope: &ControlEnvelope,
    key: &VerifyingKey,
) -> std::result::Result<(), ControlAuthError> {
    let mut unsigned = envelope.clone();
    unsigned.signature.value.clear();
    let bytes =
        aithos_core::jcs::canonical_bytes(&unsigned).map_err(|_| ControlAuthError::Invalid)?;
    let signature = hex::decode(&envelope.signature.value)
        .ok()
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .ok_or(ControlAuthError::SignatureInvalid)?;
    key.verify(&bytes, &Signature::from_bytes(&signature))
        .map_err(|_| ControlAuthError::SignatureInvalid)
}

fn decode_verifying_key(value: &str) -> std::result::Result<VerifyingKey, ControlAuthError> {
    let bytes = aithos_core::wire::multibase_to_ed25519_pub(value)
        .map_err(|_| ControlAuthError::ChainInvalid)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| ControlAuthError::ChainInvalid)
}

fn valid_key_shape(key: &str) -> bool {
    key == "#root" || key == "#content" || key.starts_with('z')
}

/// Route parsing consumes Core's existing Gamma kind grammar rather than
/// defining a transport-local approximation.
pub fn valid_control_gamma_kind(value: &str) -> bool {
    Kind::parse(value).is_ok()
}

fn valid_mandate_id(id: &str) -> bool {
    id.strip_prefix("mandate_")
        .is_some_and(|suffix| Sid::parse(suffix).is_ok())
}

fn redacted_bridge_error(message: &str) -> GatewayError {
    GatewayError::BridgeFailed(message.to_owned())
}

// Exact strict RFC3339 Zulu parser used by A.2. Fractions are accepted up to
// nanoseconds and truncated to milliseconds, matching the provider verifier.
fn parse_rfc3339z_ms(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || *bytes.last()? != b'Z'
    {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<i64> {
        let mut result = 0i64;
        for byte in &bytes[range] {
            if !byte.is_ascii_digit() {
                return None;
            }
            result = result * 10 + i64::from(*byte - b'0');
        }
        Some(result)
    };
    let year = digits(0..4)?;
    let month = digits(5..7)?;
    let day = digits(8..10)?;
    let hour = digits(11..13)?;
    let minute = digits(14..16)?;
    let second = digits(17..19)?;
    let mut millis = 0i64;
    if bytes.len() > 20 {
        if bytes[19] != b'.' || bytes.len() < 22 || bytes.len() > 30 {
            return None;
        }
        let fraction = &bytes[20..bytes.len() - 1];
        if fraction.is_empty() || !fraction.iter().all(u8::is_ascii_digit) {
            return None;
        }
        for (index, byte) in fraction.iter().take(3).enumerate() {
            millis += i64::from(*byte - b'0') * [100, 10, 1][index];
        }
    }
    if !(1..=12).contains(&month)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
        || day < 1
        || day > days_in_month(year, month)
    {
        return None;
    }
    Some(
        (((days_from_civil(year, month, day) * 24 + hour) * 60 + minute) * 60 + second) * 1_000
            + millis,
    )
}

fn render_rfc3339z(milliseconds: i64) -> String {
    let seconds = milliseconds.div_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let remaining = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remaining / 3_600,
        (remaining % 3_600) / 60,
        remaining % 60
    )
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = (month + 9) % 12;
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aithos_provider::envelope::{
        header_value, sign_envelope, Envelope as ProviderEnvelope,
        EnvelopeSignature as ProviderSignature,
    };
    use ed25519_dalek::SigningKey;

    #[test]
    fn strict_time_matches_the_a2_boundary_and_fraction_contract() {
        let at = parse_rfc3339z_ms("2026-07-16T12:00:00Z").unwrap();
        assert_eq!(
            parse_rfc3339z_ms("2026-07-16T12:05:00Z").unwrap() - at,
            CONTROL_SKEW_MS
        );
        assert_eq!(
            parse_rfc3339z_ms("1970-01-01T00:00:00.123456789Z"),
            Some(123)
        );
        assert_eq!(render_rfc3339z(at), "2026-07-16T12:00:00Z");
        assert!(parse_rfc3339z_ms("2026-02-29T00:00:00Z").is_none());
    }

    #[test]
    fn gateway_preparation_consumes_the_provider_a2_wire_byte_exactly() {
        let signing = SigningKey::from_bytes(&[0x42; 32]);
        let envelope = sign_envelope(
            ProviderEnvelope {
                v: 1,
                host: "acme.mcp.aithos.fr".to_owned(),
                method: "GET".to_owned(),
                path: "/control/v1/status".to_owned(),
                body_b3: String::new(),
                at: "2026-07-16T12:00:00Z".to_owned(),
                nonce: "control-nonce-01".to_owned(),
                mandate: Vec::new(),
                key: "#content".to_owned(),
                signature: ProviderSignature {
                    alg: "ed25519".to_owned(),
                    value: String::new(),
                },
            },
            &signing,
        )
        .unwrap();
        let header = header_value(&envelope).unwrap();
        let expected = BTreeSet::from(["acme.mcp.aithos.fr".to_owned()]);
        let now = parse_rfc3339z_ms("2026-07-16T12:05:00Z").unwrap();
        let prepared = prepare_control_envelope(
            &header,
            "acme.mcp.aithos.fr",
            &expected,
            "GET",
            "/control/v1/status",
            &[],
            now,
        )
        .unwrap();
        assert_eq!(prepared.key(), "#content");
        assert_eq!(prepared.nonce(), "control-nonce-01");
        assert!(matches!(
            prepare_control_envelope(
                &header,
                "acme.mcp.aithos.fr",
                &expected,
                "POST",
                "/control/v1/status",
                &[],
                now,
            ),
            Err(ControlAuthError::Invalid)
        ));
        assert!(matches!(
            prepare_control_envelope(
                &header,
                "neighbor.mcp.aithos.fr",
                &expected,
                "GET",
                "/control/v1/status",
                &[],
                now,
            ),
            Err(ControlAuthError::Invalid)
        ));
    }
}
