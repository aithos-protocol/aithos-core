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
        match (kind.is_mutation(), &self.payload, &self.body_enc) {
            // Mutation on a keyed zone: sealed body, nothing clear.
            (true, None, Some(_)) if self.target.is_none() => Ok(()),
            // Mutation on public (no zone key, §07.3): clear, like structural.
            (true, Some(_), None) if self.target.is_some() => Ok(()),
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

/// Link integrity + per-entry form over an ordered slice (§07.1): first
/// `prev` empty, each `prev` pins its predecessor, `at` never decreases.
pub fn verify_links(entries: &[Entry]) -> Result<()> {
    let err = |m: String| Error::InvalidGammaChain(m);
    let mut prev_hash = String::new();
    let mut prev_at: Option<i64> = None;
    for e in entries {
        e.check_form()?;
        if e.prev != prev_hash {
            return Err(err(format!("{}: prev does not pin its predecessor", e.id)));
        }
        let t = ts_epoch(&e.at)?;
        if prev_at.is_some_and(|p| t < p) {
            return Err(err(format!("{}: at goes backward", e.id)));
        }
        prev_at = Some(t);
        prev_hash = e.chain_hash()?;
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
        if let Some(n) = constraint_u64(m, "max_actions") {
            if count_actions(existing, &m.id, None, None) as u64 + 1 > n {
                return Err(Error::GammaBudgetExhausted(format!(
                    "{}: max_actions {n} spent",
                    m.id
                )));
            }
        }
        if let Some((_, secs, n)) = constraint_window(m, "max_actions_per")? {
            if count_actions(existing, &m.id, None, Some((secs, at_t))) as u64 + 1 > n {
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
pub fn check_grant_append(
    existing: &[Entry],
    minting_mandate: &Mandate,
) -> Result<()> {
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
pub fn heartbeat_ok(
    entries: &[Entry],
    m: &Mandate,
    at: &str,
    did_doc: &DidDocument,
) -> Result<()> {
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
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':'
        || b[16] != b':' || b[19] != b'Z'
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
