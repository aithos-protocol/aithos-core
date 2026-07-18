//! The gamma log: chain, envelope, meter, liveness, anchor (spec §07).
//!
//! Pure throughout: entries come in as slices, time `T` and nonces are
//! injected, verdicts are `Result`s. Storage (segments, `gamma_head` pinning)
//! lives in aithos-bundle; nothing here does I/O.

use crate::derive::derive_key;
use crate::did::{DidDocument, SignatureBlock};
use crate::error::{Error, Result};
use crate::jcs;
use crate::mandate::{covers_act, ActOp, Mandate};
use crate::seal;
use crate::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const GAMMA_ENTRY_VERSION: u64 = 1;
/// Derivation context of the body recognition tag (§07.3).
pub const CTX_GAMMA_HINT: &str = "aithos-core/v1/gamma-hint";
/// AAD purpose of a sealed entry body (§07.3).
pub const PURPOSE_GAMMA_BODY: &[u8] = b"aithos-core/v1/gamma-body";

// ------------------------------------------------------------------ kinds

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    SectionAdd,
    SectionModify,
    SectionDelete,
    SectionRedact,
    Action,
    Inference,
    EthosRead,
    Heartbeat,
    Grant,
    Revoke,
    Rotate,
    Merge,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "section.add" => Kind::SectionAdd,
            "section.modify" => Kind::SectionModify,
            "section.delete" => Kind::SectionDelete,
            "section.redact" => Kind::SectionRedact,
            "action" => Kind::Action,
            "inference" => Kind::Inference,
            "ethos.read" => Kind::EthosRead,
            "heartbeat" => Kind::Heartbeat,
            "grant" => Kind::Grant,
            "revoke" => Kind::Revoke,
            "rotate" => Kind::Rotate,
            "merge" => Kind::Merge,
            other => return Err(Error::InvalidGammaEntry(format!("unknown kind {other}"))),
        })
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::SectionAdd => "section.add",
            Kind::SectionModify => "section.modify",
            Kind::SectionDelete => "section.delete",
            Kind::SectionRedact => "section.redact",
            Kind::Action => "action",
            Kind::Inference => "inference",
            Kind::EthosRead => "ethos.read",
            Kind::Heartbeat => "heartbeat",
            Kind::Grant => "grant",
            Kind::Revoke => "revoke",
            Kind::Rotate => "rotate",
            Kind::Merge => "merge",
        }
    }

    /// Mutation kinds carry sealed bodies on keyed zones (§07.3).
    #[must_use]
    pub fn is_mutation(self) -> bool {
        matches!(
            self,
            Kind::SectionAdd | Kind::SectionModify | Kind::SectionDelete | Kind::SectionRedact
        )
    }

    /// Registry class (§07.9.2) — the query-level grouping; wire kinds
    /// never change.
    #[must_use]
    pub fn class(self) -> &'static str {
        match self {
            k if k.is_mutation() => "ethos.write",
            Kind::EthosRead => "ethos.read",
            Kind::Action | Kind::Inference => "act",
            Kind::Heartbeat => "liveness",
            _ => "structural",
        }
    }
}

// ------------------------------------------------------------------ entry

/// Sealed body of a content mutation (§07.3): `hint` is the recognition tag,
/// `n`/`c` the AEAD nonce and ciphertext, all lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyEnc {
    pub hint: String,
    pub n: String,
    pub c: String,
}

/// One gamma entry (§07.1): clear counting header + clear payload OR sealed
/// body. Field order is irrelevant on the wire (JCS sorts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub v: u64,
    pub id: String,
    /// `sha256:<hex>` of the previous entry's JCS; empty for the first entry.
    pub prev: String,
    /// Two-predecessor join (§07.6, pass I): the sub-chain tips a `merge`
    /// entry re-joins, ordered like the manifest's `merges`; `prev` repeats
    /// the first. Additive — only `kind:"merge"` may carry it (check_form),
    /// pre-I entries are untouched on the wire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prevs: Option<Vec<String>>,
    pub at: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_via: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_enc: Option<BodyEnc>,
    pub signature: SignatureBlock,
}

/// Plaintext of a sealed body — serialized as JCS of `{payload, target}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub payload: serde_json::Value,
    pub target: String,
}

impl Entry {
    fn unsigned_jcs(&self) -> Result<Vec<u8>> {
        let mut unsigned = self.clone();
        unsigned.signature.value = String::new();
        jcs::canonical_bytes(&unsigned)
    }

    /// The hash a successor pins as `prev` (§07.1): over the FULL entry,
    /// signature included — a signed entry is pinned as signed.
    pub fn chain_hash(&self) -> Result<String> {
        Ok(format!(
            "sha256:{}",
            crate::gamma::sha256_hex(&jcs::canonical_bytes(self)?)
        ))
    }

    pub fn kind(&self) -> Result<Kind> {
        Kind::parse(&self.kind)
    }

    /// Structural well-formedness (§07.1): version, id shape, payload XOR
    /// body_enc discipline per kind.
    pub fn check_form(&self) -> Result<()> {
        let err = |m: String| Error::InvalidGammaEntry(format!("{}: {m}", self.id));
        if self.v != GAMMA_ENTRY_VERSION {
            return Err(err(format!("unsupported version {}", self.v)));
        }
        if !self.id.starts_with("gamma_") {
            return Err(err("id must start with gamma_".into()));
        }
        ts_epoch(&self.at)?;
        let kind = self.kind()?;
        // `prevs` discipline (§07.6, pass I): merge entries — and ONLY merge
        // entries — carry the two sub-chain tips, `prev` repeating the first;
        // their clear payload mirrors the manifest (`merges`, ascending).
        match (kind == Kind::Merge, &self.prevs) {
            (false, None) => {}
            (false, Some(_)) => {
                return Err(err("only merge entries may carry prevs".into()));
            }
            (true, Some(p)) if p.len() == 2 && p[0] != p[1] && self.prev == p[0] => {
                if self.target.is_some() {
                    return Err(err("merge entries carry no target".into()));
                }
                let merges_ok = self
                    .payload
                    .as_ref()
                    .and_then(|pl| pl.get("merges"))
                    .and_then(|m| m.as_array())
                    .is_some_and(|m| {
                        m.len() == 2
                            && m.iter().all(serde_json::Value::is_string)
                            && m[0].as_str() < m[1].as_str()
                    });
                if !merges_ok {
                    return Err(err(
                        "merge payload must carry merges = [low, high] ascending".into(),
                    ));
                }
            }
            (true, _) => {
                return Err(err(
                    "merge entries carry prevs = [low tip, high tip], prev = the low tip".into(),
                ));
            }
        }
        let sealed_only = kind.is_mutation() || kind == Kind::EthosRead;
        match (sealed_only, &self.payload, &self.body_enc) {
            // Mutation (or logged read) on a keyed zone: sealed body only.
            (true, None, Some(_)) if self.target.is_none() => Ok(()),
            // Mutation on public (no zone key, §07.3): clear, like structural.
            (true, Some(_), None) if self.target.is_some() && kind != Kind::EthosRead => Ok(()),
            // A journalized public read/list is clear because public has no
            // content key. Vault-config reads also stay clear only at the
            // connector/opaque-record commitment layer; config plaintext is
            // never present. Keyed Ethos reads remain sealed.
            (true, Some(_), None)
                if kind == Kind::EthosRead
                    && self.target.as_deref().is_some_and(|target| {
                        target.starts_with("/e/public") || target.starts_with("/x/")
                    }) =>
            {
                Ok(())
            }
            // Actions may add a sealed args body next to the clear payload
            // (§07.9.3); every other clear kind stays clear-only.
            (false, Some(_), Some(_)) if kind == Kind::Action => Ok(()),
            (false, Some(_), None) => Ok(()),
            _ => Err(err("payload/body_enc do not match the kind".into())),
        }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

// --------------------------------------------------------------- building

fn sign_entry(entry: &mut Entry, key: &SigningKey) -> Result<()> {
    entry.signature.value = String::new();
    let bytes = entry.unsigned_jcs()?;
    entry.signature.value = hex::encode(key.sign(&bytes).to_bytes());
    Ok(())
}

fn verify_entry_sig(entry: &Entry, key: &VerifyingKey) -> Result<()> {
    let sig: [u8; 64] = hex::decode(&entry.signature.value)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| Error::InvalidGammaEntry(format!("{}: bad signature", entry.id)))?;
    key.verify(&entry.unsigned_jcs()?, &Signature::from_bytes(&sig))
        .map_err(|_| Error::InvalidGammaEntry(format!("{}: signature does not verify", entry.id)))
}

pub struct EntrySpec {
    pub id: String,
    pub prev: String,
    /// Merge entries only (§07.6): the two sub-chain tips being re-joined.
    pub prevs: Option<Vec<String>>,
    pub at: String,
    pub kind: Kind,
    pub target: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub body_enc: Option<BodyEnc>,
}

impl EntrySpec {
    fn into_entry(self, sig_key: String) -> Entry {
        Entry {
            v: GAMMA_ENTRY_VERSION,
            id: self.id,
            prev: self.prev,
            prevs: self.prevs,
            at: self.at,
            kind: self.kind.as_str().to_owned(),
            target: self.target,
            authorized_by: None,
            authorized_via: None,
            payload: self.payload,
            body_enc: self.body_enc,
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: sig_key,
                value: String::new(),
            },
        }
    }
}

/// Owner entry (§07.2): signed by `content_sign`, no mandate attached.
pub fn owner_entry(spec: EntrySpec, content_sign: &SigningKey) -> Result<Entry> {
    let mut e = spec.into_entry("#content".to_owned());
    sign_entry(&mut e, content_sign)?;
    e.check_form()?;
    Ok(e)
}

/// Delegated entry (§07.2): signed by the leaf grantee key, carrying the
/// full mandate chain ids.
pub fn delegated_entry(spec: EntrySpec, via: Vec<String>, sk: &SigningKey) -> Result<Entry> {
    let leaf = via
        .last()
        .cloned()
        .ok_or_else(|| Error::InvalidGammaEntry("empty authorized_via".to_owned()))?;
    let mut e = spec.into_entry(wire::ed25519_pub_to_multibase(
        &sk.verifying_key().to_bytes(),
    ));
    e.authorized_by = Some(leaf);
    e.authorized_via = Some(via);
    sign_entry(&mut e, sk)?;
    e.check_form()?;
    Ok(e)
}

// ------------------------------------------------------------------- body

/// Recognition tag of a node's sealed bodies (§07.3): derivable only by
/// holders of the node key.
#[must_use]
pub fn body_hint(node_key: &[u8; 32]) -> String {
    hex::encode(derive_key(CTX_GAMMA_HINT, node_key))
}

/// AAD of a sealed body (§07.3), house scheme (§03.8):
/// purpose ‖ did ‖ canonical sid-path ‖ key_version.
#[must_use]
pub fn body_aad(subject_did: &str, target: &str, key_version: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(PURPOSE_GAMMA_BODY);
    out.push(0);
    out.extend_from_slice(subject_did.as_bytes());
    out.push(0);
    out.extend_from_slice(target.as_bytes());
    out.push(0);
    out.extend_from_slice(key_version.to_string().as_bytes());
    out
}

/// Seal `{payload, target}` under the target node's key (§07.3).
pub fn seal_body(
    node_key: &[u8; 32],
    subject_did: &str,
    target: &str,
    key_version: u64,
    payload: &serde_json::Value,
    nonce: &[u8; 24],
) -> Result<BodyEnc> {
    let body = Body {
        payload: payload.clone(),
        target: target.to_owned(),
    };
    let plain = jcs::canonical_bytes(&body)?;
    let c = seal::blob_seal(
        node_key,
        &plain,
        nonce,
        &body_aad(subject_did, target, key_version),
    );
    Ok(BodyEnc {
        hint: body_hint(node_key),
        n: hex::encode(nonce),
        c: hex::encode(c),
    })
}

/// Open a sealed body with a candidate node (matched by hint). Fail-closed:
/// wrong key, wrong AAD, hint mismatch, or a decrypted target that does not
/// match the candidate all reject.
pub fn open_body(
    node_key: &[u8; 32],
    subject_did: &str,
    candidate_target: &str,
    key_version: u64,
    enc: &BodyEnc,
) -> Result<Body> {
    let err = |m: &str| Error::SealRejected(format!("gamma body: {m}"));
    if enc.hint != body_hint(node_key) {
        return Err(err("hint does not match the candidate node"));
    }
    let nonce: [u8; 24] = hex::decode(&enc.n)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| err("bad nonce"))?;
    let cipher = hex::decode(&enc.c).map_err(|_| err("bad ciphertext encoding"))?;
    let plain = seal::blob_open(
        node_key,
        &cipher,
        &nonce,
        &body_aad(subject_did, candidate_target, key_version),
    )?;
    let body: Body =
        serde_json::from_slice(&plain).map_err(|_| err("body is not canonical JSON"))?;
    if body.target != candidate_target {
        return Err(err("decrypted target does not match the candidate"));
    }
    Ok(body)
}

// ------------------------------------------------------------------ chain

/// Link integrity + per-entry form over an ordered slice (§07.1, §07.6).
///
/// A linear log walks exactly as before: first `prev` empty, each `prev`
/// pins its predecessor, `at` never decreases ALONG THE CHAIN. Pass I adds
/// the fork-and-rejoin shape of a merged segment: a non-merge entry may
/// extend an already-extended entry (opening a sub-chain), and a `merge`
/// entry consumes exactly the two open tips named in its `prevs`. `at`
/// monotonicity is relaxed at the join — THERE and only there (§07.6): the
/// signed merge entry documents it; every other link keeps child ≥ parent.
/// The walk must end on a single tip — a fork never re-joined is refused.
pub fn verify_links(entries: &[Entry]) -> Result<()> {
    let err = |m: String| Error::InvalidGammaChain(m);
    // chain hash → its entry's `at` (epoch), for parent-relative time checks.
    let mut seen: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    // open sub-chain heads: hashes no later entry has extended or merged yet.
    let mut tips: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut last_hash = String::new();
    for e in entries {
        e.check_form()?;
        let t = ts_epoch(&e.at)?;
        let h = e.chain_hash()?;
        if seen.contains_key(&h) {
            return Err(err(format!("{}: duplicate entry", e.id)));
        }
        match &e.prevs {
            Some(p) => {
                // Merge entry (form-checked above): both tips must be OPEN —
                // a merge joins the current sub-chain heads, never history.
                for tip in p {
                    if !tips.remove(tip) {
                        return Err(err(format!(
                            "{}: merge prevs do not join the open tips",
                            e.id
                        )));
                    }
                }
                // `at` deliberately unconstrained at the join (§07.6).
            }
            None => {
                if e.prev.is_empty() {
                    // The genesis point. A SECOND empty-prev entry opens a
                    // fork at the very start of the log — legal only if a
                    // merge re-joins it (the single-tip check below).
                } else {
                    let Some(parent_at) = seen.get(&e.prev) else {
                        return Err(err(format!("{}: prev does not pin its predecessor", e.id)));
                    };
                    if t < *parent_at {
                        return Err(err(format!("{}: at goes backward", e.id)));
                    }
                    // Extending an open tip is the linear walk; extending a
                    // consumed hash OPENS a fork the walk must later re-join.
                    tips.remove(&e.prev);
                }
            }
        }
        tips.insert(h.clone());
        seen.insert(h.clone(), t);
        last_hash = h;
    }
    if seen.is_empty() {
        return Ok(());
    }
    if tips.len() != 1 {
        return Err(err(format!(
            "unresolved fork inside the log: {} open tips",
            tips.len()
        )));
    }
    if !tips.contains(&last_hash) {
        return Err(err("the last entry is not the chain tip".into()));
    }
    Ok(())
}

/// The tip the manifest pins as `gamma_head` (§02.7); empty log = empty head.
pub fn head(entries: &[Entry]) -> Result<String> {
    match entries.last() {
        Some(e) => e.chain_hash(),
        None => Ok(String::new()),
    }
}

/// Owner entry check (§07.2): no mandate fields, `#content` signature.
pub fn verify_owner_entry(entry: &Entry, did_doc: &DidDocument) -> Result<()> {
    let err = |m: &str| Error::InvalidGammaEntry(format!("{}: {m}", entry.id));
    if entry.authorized_by.is_some() || entry.authorized_via.is_some() {
        return Err(err("owner entries carry no mandate"));
    }
    if entry.signature.key != "#content" {
        return Err(err("owner entries are signed by #content"));
    }
    let key_bytes = wire::multibase_to_ed25519_pub(&did_doc.keys.content)?;
    let key =
        VerifyingKey::from_bytes(&key_bytes).map_err(|_| err("malformed owner content key"))?;
    verify_entry_sig(entry, &key)
}

/// Delegated entry check (§07.2): the chain matches `authorized_via`
/// exactly, verifies at the entry's own `at` (§04.5), the leaf grantee key
/// signs, and for `action` kinds the leaf perimeter covers the act.
pub fn verify_delegated_entry(
    entry: &Entry,
    chain: &[Mandate],
    did_doc: &DidDocument,
) -> Result<()> {
    let err = |m: &str| Error::InvalidGammaEntry(format!("{}: {m}", entry.id));
    let via = entry
        .authorized_via
        .as_ref()
        .ok_or_else(|| err("delegated entry without authorized_via"))?;
    let ids: Vec<&str> = chain.iter().map(|m| m.id.as_str()).collect();
    if via.iter().map(String::as_str).collect::<Vec<_>>() != ids {
        return Err(err("authorized_via does not match the presented chain"));
    }
    if entry.authorized_by.as_deref() != ids.last().copied() {
        return Err(err("authorized_by is not the leaf mandate"));
    }
    crate::mandate::verify_chain(chain, did_doc, &entry.at)?;
    let leaf = chain.last().expect("non-empty via checked above");
    if entry.signature.key != leaf.grantee.pubkey {
        return Err(err("entry is not signed by the leaf grantee key"));
    }
    verify_entry_sig(entry, &leaf.grantee_pub()?)?;
    if entry.kind()? == Kind::Action {
        let op = act_op_of(entry)?;
        if !covers_act(&leaf.parsed_perimeter()?, &op) {
            return Err(err("action not covered by the leaf perimeter"));
        }
    }
    Ok(())
}

fn act_op_of(entry: &Entry) -> Result<ActOp> {
    let err = |m: &str| Error::InvalidGammaEntry(format!("{}: {m}", entry.id));
    let connector = entry
        .target
        .as_deref()
        .and_then(|t| t.strip_prefix("x."))
        .ok_or_else(|| err("action entries target x.<connector>"))?;
    let action = entry
        .payload
        .as_ref()
        .and_then(|p| p.get("action"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| err("action entries carry payload.action"))?;
    Ok(ActOp {
        connector: connector.to_owned(),
        action: action.to_owned(),
    })
}

// ------------------------------------------------------------------ meter

/// Count action entries whose `authorized_via` contains `mandate_id` (§07.4
/// subtree rule), optionally filtered on clear `payload.action`, optionally
/// inside the rolling window `(at_t - window_secs, at_t]`.
#[must_use]
pub fn count_actions(
    entries: &[Entry],
    mandate_id: &str,
    action: Option<&str>,
    window: Option<(i64, i64)>,
) -> usize {
    entries
        .iter()
        .filter(|e| e.kind.as_str() == "action")
        .filter(|e| {
            e.authorized_via
                .as_ref()
                .is_some_and(|v| v.iter().any(|id| id == mandate_id))
        })
        .filter(|e| {
            action.is_none_or(|a| {
                e.payload
                    .as_ref()
                    .and_then(|p| p.get("action"))
                    .and_then(|x| x.as_str())
                    == Some(a)
            })
        })
        .filter(|e| match window {
            None => true,
            Some((secs, at_t)) => {
                ts_epoch(&e.at).is_ok_and(|t| t > at_t.saturating_sub(secs) && t <= at_t)
            }
        })
        .count()
}

/// Count `grant` entries minted under `mandate_id` (§07.4 `max_children`).
#[must_use]
pub fn count_children(entries: &[Entry], mandate_id: &str) -> usize {
    entries
        .iter()
        .filter(|e| e.kind.as_str() == "grant")
        .filter(|e| e.authorized_by.as_deref() == Some(mandate_id))
        .count()
}

/// Is the minting of `child_mandate_id` on the log? (§07.4: an unlogged
/// grant is a silent action — I5.)
#[must_use]
pub fn grant_logged(entries: &[Entry], child_mandate_id: &str) -> bool {
    entries
        .iter()
        .any(|e| e.kind.as_str() == "grant" && e.target.as_deref() == Some(child_mandate_id))
}

fn constraint_u64(m: &Mandate, key: &str) -> Option<u64> {
    m.constraints.get(key).and_then(serde_json::Value::as_u64)
}

fn constraint_window(m: &Mandate, key: &str) -> Result<Option<(String, i64, u64)>> {
    let Some(v) = m.constraints.get(key) else {
        return Ok(None);
    };
    let err = |m: &str| Error::InvalidMandate(format!("{key}: {m}"));
    let window = v
        .get("window")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| err("missing window"))?;
    let n = v
        .get("n")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| err("missing n"))?;
    let action = v
        .get("action")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_default();
    Ok(Some((action, parse_duration(window)?, n)))
}

/// May `candidate` (an `action` entry) be appended after `existing`? Applies
/// every counting constraint of every mandate in the chain — the subtree
/// rule — plus the unlogged-grant and heartbeat rules (§07.4, §07.5).
pub fn check_action_append(
    existing: &[Entry],
    candidate: &Entry,
    chain: &[Mandate],
    did_doc: &DidDocument,
) -> Result<()> {
    let at_t = ts_epoch(&candidate.at)?;
    // Action-plane counters cap ACTIONS; an `inference` candidate is bounded
    // by budgets/windows only (§04.11, §07.9.1).
    let is_action = candidate.kind()? == Kind::Action;
    let action = candidate
        .payload
        .as_ref()
        .and_then(|p| p.get("action"))
        .and_then(|a| a.as_str());
    // Every non-root link must have its grant on the log (I5).
    for m in chain.iter().skip(1) {
        if !grant_logged(existing, &m.id) {
            return Err(Error::GammaGrantNotLogged(m.id.clone()));
        }
    }
    for m in chain {
        // Absolute windows (§04.10) conjoin with everything else.
        crate::constraints::check_windows(
            crate::constraints::parse_windows(&m.constraints)?.as_deref(),
            &candidate.at,
        )?;
        // Budget profiles (§04.11) — actions and inferences alike.
        if let Some(profiles) = crate::constraints::parse_budgets(&m.constraints)? {
            crate::constraints::check_budgets(existing, candidate, &m.id, &profiles)?;
        }
        // Obligations (§04.12) — every covering gate must be discharged by
        // a valid receipt in checks[]; fail-closed, tier V.
        if is_action {
            crate::constraints::check_obligations(
                candidate,
                &m.constraints,
                &did_doc.keys.content,
            )?;
        }
        if let Some(n) = constraint_u64(m, "max_actions") {
            if is_action && count_actions(existing, &m.id, None, None) as u64 + 1 > n {
                return Err(Error::GammaBudgetExhausted(format!(
                    "{}: max_actions {n} spent",
                    m.id
                )));
            }
        }
        if let Some((_, secs, n)) = constraint_window(m, "max_actions_per")? {
            if is_action && count_actions(existing, &m.id, None, Some((secs, at_t))) as u64 + 1 > n
            {
                return Err(Error::GammaBudgetExhausted(format!(
                    "{}: max_actions_per {n} spent in window",
                    m.id
                )));
            }
        }
        if let Some((rl_action, secs, n)) = constraint_window(m, "rate_limit")? {
            if action == Some(rl_action.as_str())
                && count_actions(existing, &m.id, Some(&rl_action), Some((secs, at_t))) as u64 + 1
                    > n
            {
                return Err(Error::GammaBudgetExhausted(format!(
                    "{}: rate_limit {n} '{rl_action}' spent in window",
                    m.id
                )));
            }
        }
        check_heartbeat_constraint(existing, m, at_t, did_doc)?;
    }
    Ok(())
}

/// May a new sub-mandate be minted under `minting_mandate_id`? (§07.4:
/// `max_children` counts logged grants; the grant entry itself is the act.)
pub fn check_grant_append(existing: &[Entry], minting_mandate: &Mandate) -> Result<()> {
    if let Some(n) = constraint_u64(minting_mandate, "max_children") {
        if count_children(existing, &minting_mandate.id) as u64 + 1 > n {
            return Err(Error::GammaBudgetExhausted(format!(
                "{}: max_children {n} spent",
                minting_mandate.id
            )));
        }
    }
    Ok(())
}

// -------------------------------------------------------------- heartbeat

/// Latest owner-signed beacon at or before `at_t`, verified (§07.5): a
/// grantee can never beacon for itself — forged beacons simply do not count.
fn latest_beacon(entries: &[Entry], at_t: i64, did_doc: &DidDocument) -> Option<i64> {
    entries
        .iter()
        .filter(|e| e.kind.as_str() == "heartbeat")
        .filter(|e| verify_owner_entry(e, did_doc).is_ok())
        .filter_map(|e| ts_epoch(&e.at).ok())
        .filter(|t| *t <= at_t)
        .max()
}

fn check_heartbeat_constraint(
    entries: &[Entry],
    m: &Mandate,
    at_t: i64,
    did_doc: &DidDocument,
) -> Result<()> {
    let Some(hb) = m.constraints.get("heartbeat") else {
        return Ok(());
    };
    let err = |msg: &str| Error::InvalidMandate(format!("{}: heartbeat: {msg}", m.id));
    let every = hb
        .get("every")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| err("missing every"))?;
    let grace = hb
        .get("grace")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| err("missing grace"))?;
    let bound = parse_duration(every)? + parse_duration(grace)?;
    match latest_beacon(entries, at_t, did_doc) {
        Some(b) if at_t <= b + bound => Ok(()),
        _ => Err(Error::GammaHeartbeatStale(m.id.clone())),
    }
}

/// Standalone heartbeat validity of one mandate at `T` (§07.5).
pub fn heartbeat_ok(entries: &[Entry], m: &Mandate, at: &str, did_doc: &DidDocument) -> Result<()> {
    check_heartbeat_constraint(entries, m, ts_epoch(at)?, did_doc)
}

// ----------------------------------------------------------------- anchor

/// Freshness anchor (§07.7): an off-log artifact must embed a `gamma_head`
/// no older than `freshness` at presentation time `at`. Unknown anchors
/// fail closed.
pub fn check_anchor(entries: &[Entry], anchor: &str, freshness: &str, at: &str) -> Result<()> {
    let at_t = ts_epoch(at)?;
    let bound = parse_duration(freshness)?;
    let anchored = entries
        .iter()
        .find(|e| e.chain_hash().is_ok_and(|h| h == anchor))
        .ok_or_else(|| Error::GammaStaleAnchor("anchor not on the log".to_owned()))?;
    let t = ts_epoch(&anchored.at)?;
    if at_t > t + bound {
        return Err(Error::GammaStaleAnchor(format!(
            "anchor {} older than {freshness}",
            anchored.id
        )));
    }
    Ok(())
}

// -------------------------------------------- committed roots (§07.10, H2)
//
// Per-segment roots in chain order plus the counts trie — the §02.10 wire
// reused byte-for-byte. Pure: segments come in as their exact file lines,
// entries as parsed slices; the manifest committing lives in aithos-bundle.

/// Per-budget counters of one trie leaf (§07.10): `actions` = `action`
/// entries citing the ref; `tokens` = the §04.11 tally, attested receipt
/// tokens beating declarations. Zero counters are omitted from the JCS.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetCounters {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub actions: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tokens: u64,
}

/// One counts-trie leaf (§07.10): the §07.4 meters of one mandate. Zero
/// counters and empty maps are omitted; a mandate with nothing counted has
/// no leaf at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GammaCounters {
    /// ALL kinds whose `authorized_via` contains the id — the audit total.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub entries: u64,
    /// `action` entries whose `authorized_via` contains the id (subtree rule).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub actions: u64,
    /// `grant` entries whose `authorized_by` is the id.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub children: u64,
    /// Per cited `budget_ref`, under the same subtree rule.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub budgets: std::collections::BTreeMap<String, BudgetCounters>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// A segment's root: `mroot` over `H_leaf(<exact line bytes>)` in chain
/// order — the log's order is the truth, nothing is sorted (§07.10).
#[must_use]
pub fn segment_root(lines: &[&[u8]]) -> [u8; 32] {
    let hashes: Vec<[u8; 32]> = lines.iter().map(|l| crate::merkle::h_leaf(l)).collect();
    crate::merkle::mroot(&hashes)
}

/// The raw §07.4/§04.11 tallies of the reachable chain, shaped into trie
/// leaves. Owner-signed entries carry no `authorized_via` and feed nothing.
#[must_use]
pub fn counts_tally(entries: &[Entry]) -> std::collections::BTreeMap<String, GammaCounters> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut per: BTreeMap<String, GammaCounters> = BTreeMap::new();
    for e in entries {
        let via: BTreeSet<&String> = e.authorized_via.iter().flatten().collect();
        let budget_ref = e
            .payload
            .as_ref()
            .and_then(|p| p.get("budget_ref"))
            .and_then(|b| b.as_str());
        for mid in via {
            let c = per.entry(mid.clone()).or_default();
            c.entries += 1;
            if e.kind.as_str() == "action" {
                c.actions += 1;
            }
            if matches!(e.kind.as_str(), "action" | "inference") {
                if let Some(r) = budget_ref {
                    let slot = c.budgets.entry(r.to_owned()).or_default();
                    if e.kind.as_str() == "action" {
                        slot.actions += 1;
                    }
                    slot.tokens += crate::constraints::entry_tokens(e);
                }
            }
        }
        if e.kind.as_str() == "grant" {
            if let Some(by) = &e.authorized_by {
                per.entry(by.clone()).or_default().children += 1;
            }
        }
    }
    per.retain(|_, c| {
        c.budgets.retain(|_, s| *s != BudgetCounters::default());
        *c != GammaCounters::default()
    });
    per
}

/// A trie leaf's claimed payload: `mandate_id ‖ 0x00 ‖ JCS(counters)`.
pub fn counts_leaf_payload(id: &str, c: &GammaCounters) -> Result<Vec<u8>> {
    let mut p = id.as_bytes().to_vec();
    p.push(0);
    p.extend_from_slice(&jcs::canonical_bytes(c)?);
    Ok(p)
}

fn counts_leaves(
    tallies: &std::collections::BTreeMap<String, GammaCounters>,
) -> Result<Vec<[u8; 32]>> {
    tallies
        .iter()
        .map(|(id, c)| Ok(crate::merkle::h_leaf(&counts_leaf_payload(id, c)?)))
        .collect()
}

/// `gamma_counts_root` (§07.10): `mroot` over the leaves sorted by mandate
/// id — 32×0x00 when nothing was ever counted.
pub fn counts_root(
    tallies: &std::collections::BTreeMap<String, GammaCounters>,
) -> Result<[u8; 32]> {
    Ok(crate::merkle::mroot(&counts_leaves(tallies)?))
}

/// Inclusion proof of the entry at `idx` (chain order) against its segment
/// root — the v1 wire, claimed payload = the exact line bytes.
pub fn prove_entry(lines: &[&[u8]], idx: usize) -> Result<crate::merkle::Proof> {
    if idx >= lines.len() {
        return Err(Error::MerkleProofInvalid(format!(
            "entry index {idx} beyond segment length {}",
            lines.len()
        )));
    }
    let hashes: Vec<[u8; 32]> = lines.iter().map(|l| crate::merkle::h_leaf(l)).collect();
    Ok(crate::merkle::Proof {
        payload: hex::encode(lines[idx]),
        steps: crate::merkle::mroot_path(&hashes, idx),
        root: hex::encode(crate::merkle::mroot(&hashes)),
    })
}

/// Count proof of one mandate against `gamma_counts_root` — the v1 wire,
/// claimed payload = `mandate_id ‖ 0x00 ‖ JCS(counters)`.
pub fn prove_count(
    tallies: &std::collections::BTreeMap<String, GammaCounters>,
    mandate_id: &str,
) -> Result<crate::merkle::Proof> {
    let idx = tallies
        .keys()
        .position(|k| k == mandate_id)
        .ok_or_else(|| Error::MerkleProofInvalid(format!("{mandate_id}: not in the trie")))?;
    let leaves = counts_leaves(tallies)?;
    Ok(crate::merkle::Proof {
        payload: hex::encode(counts_leaf_payload(mandate_id, &tallies[mandate_id])?),
        steps: crate::merkle::mroot_path(&leaves, idx),
        root: hex::encode(crate::merkle::mroot(&leaves)),
    })
}

/// Verify a count proof and hand back its parsed leaf: the mandate id and
/// the proven counters (fail-closed on any malformation).
pub fn verify_count_proof(
    proof: &crate::merkle::Proof,
    pinned_root: &[u8; 32],
) -> Result<(String, GammaCounters)> {
    crate::merkle::verify_proof(proof, pinned_root)?;
    parse_count_payload(&proof.payload)
}

fn parse_count_payload(payload_hex: &str) -> Result<(String, GammaCounters)> {
    let err = |m: &str| Error::MerkleProofInvalid(format!("count leaf: {m}"));
    let bytes = hex::decode(payload_hex).map_err(|_| err("bad payload encoding"))?;
    let nul = bytes
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| err("no id separator"))?;
    let id = String::from_utf8(bytes[..nul].to_vec()).map_err(|_| err("id not utf-8"))?;
    let counters: GammaCounters =
        serde_json::from_slice(&bytes[nul + 1..]).map_err(|_| err("malformed counters"))?;
    // The claimed bytes must BE the canonical bytes — a re-encoding that
    // verifies but parses differently would be a second preimage.
    if counts_leaf_payload(&id, &counters)? != bytes {
        return Err(err("payload is not canonical JCS"));
    }
    Ok((id, counters))
}

/// An absence proof (§07.10): the leaves adjacent in the tree that bracket
/// the absent id. `None` sides claim the id falls before the first or past
/// the last leaf; both `None` claims the empty trie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsenceProof {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<crate::merkle::Proof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<crate::merkle::Proof>,
}

/// Build the absence proof for `absent_id` — fails if the id is counted.
pub fn prove_absence(
    tallies: &std::collections::BTreeMap<String, GammaCounters>,
    absent_id: &str,
) -> Result<AbsenceProof> {
    if tallies.contains_key(absent_id) {
        return Err(Error::GammaAbsenceInvalid(format!(
            "{absent_id}: counted — nothing to prove absent"
        )));
    }
    let before = tallies.keys().filter(|k| k.as_str() < absent_id).count();
    let left = before
        .checked_sub(1)
        .map(|i| prove_count(tallies, tallies.keys().nth(i).expect("index in range")))
        .transpose()?;
    let right = (before < tallies.len())
        .then(|| prove_count(tallies, tallies.keys().nth(before).expect("index in range")))
        .transpose()?;
    Ok(AbsenceProof { left, right })
}

/// A proof's node steps, decoded: `(side, sibling hash)`, innermost first.
type NodeSteps = Vec<(crate::merkle::Side, [u8; 32])>;
/// A verified, parsed count leaf: `(mandate id, node steps, leaf hash)`.
type ParsedLeaf = (String, NodeSteps, [u8; 32]);

/// All `node` steps of a v1 proof (an absence proof folds no parents).
fn node_steps(p: &crate::merkle::Proof) -> Result<NodeSteps> {
    p.steps
        .iter()
        .map(|s| match s {
            crate::merkle::ProofStep::Node { side, hash } => {
                let h = hex::decode(hash)
                    .ok()
                    .and_then(|v| <[u8; 32]>::try_from(v).ok())
                    .ok_or_else(|| Error::GammaAbsenceInvalid("bad sibling encoding".into()))?;
                Ok((*side, h))
            }
            crate::merkle::ProofStep::Wrap { .. } => Err(Error::GammaAbsenceInvalid(
                "wrap step in a flat-tree proof".into(),
            )),
        })
        .collect()
}

fn replay_nodes(start: [u8; 32], steps: &[(crate::merkle::Side, [u8; 32])]) -> [u8; 32] {
    steps.iter().fold(start, |cur, (side, sib)| match side {
        crate::merkle::Side::Left => crate::merkle::h_node(sib, &cur),
        crate::merkle::Side::Right => crate::merkle::h_node(&cur, sib),
    })
}

/// Verify an absence proof against the pinned counts root (§07.10).
///
/// Interior case: both leaves verify, their ids bracket `absent_id`, and
/// they are ADJACENT in the tree — above their divergence the step lists
/// are identical; at it, each proof's sibling replays from the other's
/// lower steps; below it, the left leaf is the rightmost of the left
/// subtree (`side:"left"` only) and the right leaf the leftmost of the
/// right (`side:"right"` only). Rim cases: a missing left demands the
/// right leaf be the tree's first (all `side:"right"`), and symmetrically;
/// both missing demands the empty root.
pub fn verify_absence(absent_id: &str, proof: &AbsenceProof, pinned_root: &[u8; 32]) -> Result<()> {
    let err = |m: String| Error::GammaAbsenceInvalid(m);
    let parse = |p: &crate::merkle::Proof| -> Result<ParsedLeaf> {
        crate::merkle::verify_proof(p, pinned_root)?;
        let (id, _) = parse_count_payload(&p.payload)?;
        let leaf =
            crate::merkle::h_leaf(&hex::decode(&p.payload).map_err(|_| err("bad payload".into()))?);
        Ok((id, node_steps(p)?, leaf))
    };
    match (&proof.left, &proof.right) {
        (None, None) => {
            if pinned_root == &crate::merkle::EMPTY_ROOT {
                Ok(())
            } else {
                Err(err("empty-trie claim over a non-empty root".into()))
            }
        }
        (None, Some(r)) => {
            let (rid, rsteps, _) = parse(r)?;
            if absent_id >= rid.as_str() {
                return Err(err(format!("{absent_id} is not before the first leaf")));
            }
            if rsteps.iter().any(|(s, _)| *s != crate::merkle::Side::Right) {
                return Err(err("claimed first leaf is not the leftmost".into()));
            }
            Ok(())
        }
        (Some(l), None) => {
            let (lid, lsteps, _) = parse(l)?;
            if absent_id <= lid.as_str() {
                return Err(err(format!("{absent_id} is not past the last leaf")));
            }
            if lsteps.iter().any(|(s, _)| *s != crate::merkle::Side::Left) {
                return Err(err("claimed last leaf is not the rightmost".into()));
            }
            Ok(())
        }
        (Some(l), Some(r)) => {
            let (lid, lsteps, lleaf) = parse(l)?;
            let (rid, rsteps, rleaf) = parse(r)?;
            if !(lid.as_str() < absent_id && absent_id < rid.as_str()) {
                return Err(err(format!("{lid} .. {rid} do not bracket {absent_id}")));
            }
            // Longest common suffix = the shared path above the divergence.
            let common = lsteps
                .iter()
                .rev()
                .zip(rsteps.iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let (ld, rd) = (
                lsteps.len().checked_sub(common + 1),
                rsteps.len().checked_sub(common + 1),
            );
            let (Some(ld), Some(rd)) = (ld, rd) else {
                return Err(err("no divergence — not two distinct leaves".into()));
            };
            let (lside, lsib) = &lsteps[ld];
            let (rside, rsib) = &rsteps[rd];
            if *lside != crate::merkle::Side::Right || *rside != crate::merkle::Side::Left {
                return Err(err("divergence sides do not face each other".into()));
            }
            if *lsib != replay_nodes(rleaf, &rsteps[..rd]) {
                return Err(err("left sibling is not the right leaf's subtree".into()));
            }
            if *rsib != replay_nodes(lleaf, &lsteps[..ld]) {
                return Err(err("right sibling is not the left leaf's subtree".into()));
            }
            if lsteps[..ld]
                .iter()
                .any(|(s, _)| *s != crate::merkle::Side::Left)
            {
                return Err(err("left leaf is not the rightmost of its subtree".into()));
            }
            if rsteps[..rd]
                .iter()
                .any(|(s, _)| *s != crate::merkle::Side::Right)
            {
                return Err(err("right leaf is not the leftmost of its subtree".into()));
            }
            Ok(())
        }
    }
}

/// Verify a mirror's "every action under this mandate" answer (§07.10):
/// the count leaf fixes k, then k inclusion proofs of pairwise-distinct
/// `action` entries, each carrying the mandate in its clear
/// `authorized_via`, each against its segment's pinned root. Returns the
/// parsed entries. Fail-closed: one withheld or forged line kills it.
pub fn verify_complete_actions(
    mandate_id: &str,
    count_proof: &crate::merkle::Proof,
    entry_proofs: &[(String, crate::merkle::Proof)],
    segment_roots: &std::collections::BTreeMap<String, [u8; 32]>,
    counts_root: &[u8; 32],
) -> Result<Vec<Entry>> {
    let werr = |m: String| Error::GammaWithholdDetected(m);
    let (id, counters) = verify_count_proof(count_proof, counts_root)?;
    if id != mandate_id {
        return Err(werr(format!("count leaf is for {id}, not {mandate_id}")));
    }
    if entry_proofs.len() as u64 != counters.actions {
        return Err(werr(format!(
            "{} entries served against a proven count of {}",
            entry_proofs.len(),
            counters.actions
        )));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (segment, proof) in entry_proofs {
        let pinned = segment_roots
            .get(segment)
            .ok_or_else(|| werr(format!("{segment}: no committed root")))?;
        crate::merkle::verify_proof(proof, pinned)?;
        if !seen.insert((segment.clone(), proof.payload.clone())) {
            return Err(werr("duplicate entry in the answer".into()));
        }
        let bytes = hex::decode(&proof.payload)
            .map_err(|_| Error::MerkleProofInvalid("bad entry payload".into()))?;
        let entry: Entry = serde_json::from_slice(&bytes)
            .map_err(|e| Error::InvalidGammaEntry(format!("proven entry: {e}")))?;
        if entry.kind.as_str() != "action" {
            return Err(werr(format!("{}: not an action entry", entry.id)));
        }
        if !entry
            .authorized_via
            .as_ref()
            .is_some_and(|v| v.iter().any(|m| m == mandate_id))
        {
            return Err(werr(format!(
                "{}: does not run under {mandate_id}",
                entry.id
            )));
        }
        out.push(entry);
    }
    Ok(out)
}

// ------------------------------------------------------------------- time

/// Parse `"<n>d" | "<n>h" | "<n>m" | "<n>s"` into seconds.
pub fn parse_duration(s: &str) -> Result<i64> {
    let err = || Error::InvalidGammaEntry(format!("bad duration: {s}"));
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: i64 = num.parse().map_err(|_| err())?;
    if n < 0 {
        return Err(err());
    }
    let mult = match unit {
        "d" => 86_400,
        "h" => 3_600,
        "m" => 60,
        "s" => 1,
        _ => return Err(err()),
    };
    Ok(n * mult)
}

/// Strict `YYYY-MM-DDTHH:MM:SSZ` → Unix seconds. Pure, no clock, no locale;
/// days-from-civil per Howard Hinnant's algorithm. Cross-checked by F3.
pub fn ts_epoch(at: &str) -> Result<i64> {
    let err = || Error::InvalidGammaEntry(format!("bad timestamp: {at}"));
    let b = at.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return Err(err());
    }
    let num = |r: core::ops::Range<usize>| -> Result<i64> {
        at.get(r)
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(err)
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return Err(err());
    }
    // days_from_civil (public-domain algorithm, exact for all Gregorian dates)
    let y_adj = y - i64::from(mo <= 2);
    let era = y_adj.div_euclid(400);
    let yoe = y_adj - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Ok(days * 86_400 + h * 3_600 + mi * 60 + s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_matches_known_instants() {
        assert_eq!(ts_epoch("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(ts_epoch("2026-07-01T00:00:00Z").unwrap(), 1_782_864_000);
        assert_eq!(ts_epoch("2024-02-29T12:00:00Z").unwrap(), 1_709_208_000);
        assert!(ts_epoch("2026-13-01T00:00:00Z").is_err());
        assert!(ts_epoch("2026-07-01 00:00:00Z").is_err());
    }

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("30d").unwrap(), 2_592_000);
        assert_eq!(parse_duration("72h").unwrap(), 259_200);
        assert!(parse_duration("30x").is_err());
    }
}
