//! Advanced agentic constraints (spec §04.10, §04.11): absolute active
//! windows, budget profiles, attestation receipts. Pure interval and
//! counting arithmetic — `T` is injected, verdicts are `Result`s.

use crate::error::{Error, Result};
use crate::gamma::{parse_duration, ts_epoch, Entry};
use crate::jcs;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Occurrence-enumeration bound for attenuation checks (§04.10): a
/// containment the verifier cannot enumerate is a rejection, never a pass.
const ATTENUATION_ENUM_CAP: i64 = 100_000;

// ---------------------------------------------------------------- windows

/// One absolute arithmetic window (§04.10): occurrence k is the half-open
/// interval `[anchor + k·period, anchor + k·period + duration)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub anchor: i64,
    pub duration: i64,
    pub period: Option<i64>,
    pub until: Option<i64>,
    pub count: Option<u64>,
}

impl Window {
    pub fn from_json(v: &serde_json::Value) -> Result<Self> {
        let err = |m: &str| Error::InvalidMandate(format!("active_windows: {m}"));
        let anchor = v
            .get("anchor")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| err("missing anchor"))?;
        let duration = v
            .get("duration")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| err("missing duration"))?;
        let period = v
            .get("period")
            .map(|p| {
                p.as_str()
                    .ok_or_else(|| err("period must be a duration string"))
                    .and_then(parse_duration)
            })
            .transpose()?;
        if period.is_some_and(|p| p <= 0) {
            return Err(err("period must be positive"));
        }
        let until = v
            .get("until")
            .map(|u| {
                u.as_str()
                    .ok_or_else(|| err("until must be an RFC 3339 instant"))
                    .and_then(ts_epoch)
            })
            .transpose()?;
        let count = v.get("count").and_then(serde_json::Value::as_u64);
        Ok(Window {
            anchor: ts_epoch(anchor)?,
            duration: parse_duration(duration)?,
            period,
            until,
            count,
        })
    }

    /// The occurrence index whose slot could contain `t`, if any.
    fn occurrence_of(&self, t: i64) -> Option<i64> {
        match self.period {
            None => Some(0),
            Some(p) => {
                if t < self.anchor {
                    None
                } else {
                    Some((t - self.anchor).div_euclid(p))
                }
            }
        }
    }

    fn occurrence_start(&self, k: i64) -> i64 {
        self.anchor + k * self.period.unwrap_or(0)
    }

    fn occurrence_exists(&self, k: i64) -> bool {
        if k < 0 || (self.period.is_none() && k > 0) {
            return false;
        }
        if self.count.is_some_and(|c| k as u64 >= c) {
            return false;
        }
        let start = self.occurrence_start(k);
        self.until.is_none_or(|u| start <= u)
    }

    /// Is `t` inside some occurrence? Start inclusive, end exclusive.
    #[must_use]
    pub fn contains(&self, t: i64) -> bool {
        let Some(k) = self.occurrence_of(t) else {
            return false;
        };
        self.occurrence_exists(k)
            && t >= self.occurrence_start(k)
            && t < self.occurrence_start(k) + self.duration
    }
}

/// Parse a mandate's `active_windows` array; `None` when absent.
pub fn parse_windows(constraints: &serde_json::Value) -> Result<Option<Vec<Window>>> {
    let Some(v) = constraints.get("active_windows") else {
        return Ok(None);
    };
    let arr = v
        .as_array()
        .ok_or_else(|| Error::InvalidMandate("active_windows must be an array".to_owned()))?;
    Ok(Some(
        arr.iter().map(Window::from_json).collect::<Result<_>>()?,
    ))
}

/// Union semantics (§04.10): `t` must fall inside some window. An absent
/// constraint covers any instant; a present-but-empty list covers none.
pub fn check_windows(windows: Option<&[Window]>, at: &str) -> Result<()> {
    let Some(ws) = windows else { return Ok(()) };
    let t = ts_epoch(at)?;
    if ws.iter().any(|w| w.contains(t)) {
        Ok(())
    } else {
        Err(Error::GammaBudgetExhausted(format!(
            "outside every active window at {at}"
        )))
    }
}

/// Attenuation (§04.10, §05.3): every child occurrence clipped to the
/// child's validity MUST be contained in some parent occurrence. Bounded
/// enumeration; excess fails closed.
pub fn windows_attenuate(
    parent: Option<&[Window]>,
    child: Option<&[Window]>,
    child_not_before: &str,
    child_not_after: &str,
) -> Result<()> {
    let Some(parent) = parent else {
        return Ok(()); // absent dimension covers anything
    };
    let child_windows: Vec<Window> = match child {
        Some(c) => c.to_vec(),
        // Parent windowed, child unconstrained: the child would be wider.
        None => {
            return Err(Error::InvalidMandate(
                "child drops the parent's active_windows".to_owned(),
            ))
        }
    };
    let (nb, na) = (ts_epoch(child_not_before)?, ts_epoch(child_not_after)?);
    let mut enumerated = 0i64;
    for cw in &child_windows {
        let mut k = 0i64;
        loop {
            if !cw.occurrence_exists(k) {
                break;
            }
            let start = cw.occurrence_start(k);
            if start > na {
                break;
            }
            let end = start + cw.duration;
            // Clip to the child validity window.
            let (s, e) = (start.max(nb), end.min(na));
            if s < e {
                let covered = parent.iter().any(|pw| {
                    pw.occurrence_of(s).is_some_and(|pk| {
                        pw.occurrence_exists(pk)
                            && s >= pw.occurrence_start(pk)
                            && e <= pw.occurrence_start(pk) + pw.duration
                    })
                });
                if !covered {
                    return Err(Error::InvalidMandate(format!(
                        "child window occurrence at {start} exceeds the parent's windows"
                    )));
                }
            }
            enumerated += 1;
            if enumerated > ATTENUATION_ENUM_CAP {
                return Err(Error::InvalidMandate(
                    "active_windows containment not verifiable within bounds".to_owned(),
                ));
            }
            if cw.period.is_none() {
                break;
            }
            k += 1;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- budgets

/// One budget profile (§04.11) — a conjunction; profiles compose with OR.
#[derive(Debug, Clone)]
pub struct BudgetProfile {
    pub id: String,
    pub models: Option<Vec<String>>,
    pub token_budget: Option<u64>,
    pub windows: Option<Vec<Window>>,
    pub max_actions: Option<u64>,
    pub require_attestation: bool,
    pub attestation_key: Option<String>,
}

pub fn parse_budgets(constraints: &serde_json::Value) -> Result<Option<Vec<BudgetProfile>>> {
    let Some(v) = constraints.get("budgets") else {
        return Ok(None);
    };
    let err = |m: &str| Error::InvalidMandate(format!("budgets: {m}"));
    let arr = v.as_array().ok_or_else(|| err("must be an array"))?;
    let mut out = Vec::new();
    for p in arr {
        let id = p
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| err("profile without id"))?
            .to_owned();
        let models = p.get("models").map(|m| {
            m.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        let windows = match p.get("active_windows") {
            None => None,
            Some(w) => Some(
                w.as_array()
                    .ok_or_else(|| err("active_windows must be an array"))?
                    .iter()
                    .map(Window::from_json)
                    .collect::<Result<Vec<_>>>()?,
            ),
        };
        out.push(BudgetProfile {
            id,
            models,
            token_budget: p.get("token_budget").and_then(serde_json::Value::as_u64),
            windows,
            max_actions: p.get("max_actions").and_then(serde_json::Value::as_u64),
            require_attestation: p
                .get("require_attestation")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            attestation_key: p
                .get("attestation_key")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        });
    }
    Ok(Some(out))
}

fn payload_str<'a>(e: &'a Entry, key: &str) -> Option<&'a str> {
    e.payload.as_ref()?.get(key)?.as_str()
}

fn payload_u64(e: &Entry, key: &str) -> Option<u64> {
    e.payload.as_ref()?.get(key)?.as_u64()
}

/// Tokens one entry contributes to its cited profile (§04.11): the attested
/// usage when a receipt is present, the declaration otherwise.
#[must_use]
pub fn entry_tokens(e: &Entry) -> u64 {
    if let Some(r) = e.payload.as_ref().and_then(|p| p.get("receipt")) {
        if let Some(t) = r.get("tokens").and_then(serde_json::Value::as_u64) {
            return t;
        }
    }
    payload_u64(e, "tokens").unwrap_or(0)
        + payload_u64(e, "tokens_in").unwrap_or(0)
        + payload_u64(e, "tokens_out").unwrap_or(0)
}

/// Subtree tally of tokens cited on `profile_id` under `mandate_id`.
#[must_use]
pub fn tally_tokens(entries: &[Entry], mandate_id: &str, profile_id: &str) -> u64 {
    entries
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "action" | "inference"))
        .filter(|e| {
            e.authorized_via
                .as_ref()
                .is_some_and(|v| v.iter().any(|id| id == mandate_id))
        })
        .filter(|e| payload_str(e, "budget_ref") == Some(profile_id))
        .map(entry_tokens)
        .sum()
}

/// Subtree count of ACTIONS cited on `profile_id` (§04.11: `max_actions`
/// caps actions; inferences are bounded by the token budget).
#[must_use]
pub fn count_profile_entries(entries: &[Entry], mandate_id: &str, profile_id: &str) -> usize {
    entries
        .iter()
        .filter(|e| e.kind.as_str() == "action")
        .filter(|e| {
            e.authorized_via
                .as_ref()
                .is_some_and(|v| v.iter().any(|id| id == mandate_id))
        })
        .filter(|e| payload_str(e, "budget_ref") == Some(profile_id))
        .count()
}

// ------------------------------------------------------------ attestation

/// The receipt's signed payload (§04.11.1) — JCS of exactly these fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReceiptPayload {
    args_hash: String,
    model: String,
    tokens: u64,
}

/// Verify a receipt against the profile's attestation key and the entry's
/// own clear fields (anti-replay binding, §04.11.1).
pub fn verify_receipt(candidate: &Entry, profile: &BudgetProfile) -> Result<()> {
    let err = |m: &str| Error::InvalidGammaEntry(format!("{}: receipt: {m}", candidate.id));
    let receipt = candidate
        .payload
        .as_ref()
        .and_then(|p| p.get("receipt"))
        .ok_or_else(|| err("required but absent"))?;
    let key_mb = profile
        .attestation_key
        .as_deref()
        .ok_or_else(|| err("profile has no attestation_key"))?;
    let key_bytes = crate::wire::multibase_to_ed25519_pub(key_mb)?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| err("malformed key"))?;

    let payload = ReceiptPayload {
        args_hash: receipt
            .get("args_hash")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| err("missing args_hash"))?
            .to_owned(),
        model: receipt
            .get("model")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| err("missing model"))?
            .to_owned(),
        tokens: receipt
            .get("tokens")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| err("missing tokens"))?,
    };
    // Anti-replay: the receipt binds THIS entry's args and model.
    if Some(payload.args_hash.as_str()) != payload_str(candidate, "args_hash") {
        return Err(err("args_hash does not match the entry"));
    }
    if payload_str(candidate, "model").is_some_and(|m| m != payload.model) {
        return Err(err("model does not match the entry"));
    }
    let sig_hex = receipt
        .get("sig")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| err("missing sig"))?;
    let sig: [u8; 64] = hex::decode(sig_hex)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| err("bad sig encoding"))?;
    key.verify(
        &jcs::canonical_bytes(&payload)?,
        &Signature::from_bytes(&sig),
    )
    .map_err(|_| err("signature does not verify under the attestation key"))
}

// --------------------------------------------------------------- verdicts

/// Budget verdict for a candidate `action`/`inference` entry against ONE
/// budgets-bearing mandate (§04.11): the cited profile must be satisfied in
/// full — the OR lives in *which* profile the entry cites.
pub fn check_budgets(
    existing: &[Entry],
    candidate: &Entry,
    mandate_id: &str,
    profiles: &[BudgetProfile],
) -> Result<()> {
    let exhausted = |m: String| Error::GammaBudgetExhausted(format!("{mandate_id}: {m}"));
    let Some(cited) = payload_str(candidate, "budget_ref") else {
        return Err(exhausted("budgets present but no budget_ref cited".into()));
    };
    let profile = profiles
        .iter()
        .find(|p| p.id == cited)
        .ok_or_else(|| exhausted(format!("unknown budget_ref '{cited}'")))?;

    if let Some(models) = &profile.models {
        let m = payload_str(candidate, "model")
            .ok_or_else(|| exhausted(format!("profile '{cited}' requires a model")))?;
        if !models.iter().any(|x| x == m) {
            return Err(exhausted(format!("model '{m}' not allowed by '{cited}'")));
        }
    }
    check_windows(profile.windows.as_deref(), &candidate.at)
        .map_err(|_| exhausted(format!("outside profile '{cited}' windows")))?;
    if let Some(n) = profile.max_actions {
        if count_profile_entries(existing, mandate_id, cited) as u64 + 1 > n {
            return Err(exhausted(format!("profile '{cited}' action cap {n} spent")));
        }
    }
    if let Some(budget) = profile.token_budget {
        let spent = tally_tokens(existing, mandate_id, cited);
        if spent + entry_tokens(candidate) > budget {
            return Err(exhausted(format!(
                "profile '{cited}' token budget {budget} spent ({spent} used)"
            )));
        }
    }
    if profile.require_attestation {
        verify_receipt(candidate, profile)?;
    } else if candidate
        .payload
        .as_ref()
        .is_some_and(|p| p.get("receipt").is_some())
    {
        // A volunteered receipt must still be valid — never a decoration.
        verify_receipt(candidate, profile)?;
    }
    Ok(())
}

// ----------------------------------------------------------- action_params

/// Evaluate `action_params` predicates (§04.4) against a real argument
/// object — the container's tier-X duty, reused verbatim by the owner's
/// a-posteriori audit (§07.9.3). Minimal predicate set, additive later:
/// `recipients_allow: [addr]`, `no_attachments: true`.
pub fn check_action_params(
    constraints: &serde_json::Value,
    action: &str,
    args: &serde_json::Value,
) -> Result<()> {
    let err = |m: String| Error::InvalidMandate(format!("action_params: {m}"));
    let Some(params) = constraints.get("action_params").and_then(|p| p.get(action)) else {
        return Ok(()); // no predicates for this action
    };
    if let Some(allow) = params.get("recipients_allow").and_then(|a| a.as_array()) {
        let allowed: Vec<&str> = allow.iter().filter_map(|x| x.as_str()).collect();
        let mut recipients: Vec<&str> = Vec::new();
        if let Some(r) = args.get("recipient").and_then(|r| r.as_str()) {
            recipients.push(r);
        }
        if let Some(rs) = args.get("recipients").and_then(|r| r.as_array()) {
            recipients.extend(rs.iter().filter_map(|x| x.as_str()));
        }
        if recipients.is_empty() {
            return Err(err(format!("{action}: no recipient to check")));
        }
        for r in recipients {
            if !allowed.contains(&r) {
                return Err(err(format!("{action}: recipient '{r}' not allowed")));
            }
        }
    }
    if params.get("no_attachments").and_then(|b| b.as_bool()) == Some(true)
        && args
            .get("attachments")
            .and_then(|a| a.as_array())
            .is_some_and(|a| !a.is_empty())
    {
        return Err(err(format!("{action}: attachments are not allowed")));
    }
    Ok(())
}

// ------------------------------------------------------------ obligations

/// Reserved obligation id for the owner counter-signature (§4.6 desugared,
/// decided 2026-07-10): one wire shape, no special case.
pub const CO_SIGN_ID: &str = "co_sign";
/// Δ_cosign — normative default freshness of an owner co-signature (§4.6).
pub const CO_SIGN_MAX_AGE: &str = "5m";

/// What an obligation gates (§04.12 `applies_to`). Declared obligations
/// carry a perimeter `act.` pattern; desugared `counter_sign`/`binding`
/// entries gate a bare action name on any connector.
#[derive(Debug, Clone)]
pub enum AppliesTo {
    /// `act.x.<connector>.<action|*>` — perimeter grammar (§4.2).
    Pattern(crate::mandate::PerimeterEntry),
    /// Bare action name (counter_sign/binding shorthand).
    Action(String),
}

impl AppliesTo {
    fn covers(&self, op: &crate::mandate::ActOp) -> bool {
        match self {
            AppliesTo::Pattern(p) => crate::mandate::covers_act(core::slice::from_ref(p), op),
            AppliesTo::Action(name) => *name == op.action,
        }
    }
}

/// One discharge requirement on a permit (§04.12): an in-scope action may
/// consume only with a valid receipt from a pinned attestor whose verdict
/// satisfies the predicate. The `check` id is opaque — the logic lives in
/// the attestor, the protocol holds a signature.
#[derive(Debug, Clone)]
pub struct Obligation {
    pub id: String,
    pub attestor: Vec<String>,
    pub applies_to: AppliesTo,
    pub verdict: String,
    pub max_age: Option<i64>,
}

impl Obligation {
    fn from_json(v: &serde_json::Value) -> Result<Self> {
        let err = |m: &str| Error::InvalidMandate(format!("obligation: {m}"));
        let s = |k: &str| -> Result<String> {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| err(&format!("missing {k}")))
        };
        let applies_raw = s("applies_to")?;
        if !applies_raw.starts_with("act.") {
            return Err(err("applies_to must be an act. perimeter pattern"));
        }
        let attestor: Vec<String> = v
            .get("attestor")
            .and_then(serde_json::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .ok_or_else(|| err("missing attestor"))?;
        if attestor.is_empty() {
            return Err(err("empty attestor set"));
        }
        s("check")?; // required by the shape, opaque to the verifier
        Ok(Self {
            id: s("id")?,
            attestor,
            applies_to: AppliesTo::Pattern(crate::mandate::PerimeterEntry::parse(&applies_raw)?),
            verdict: s("verdict")?,
            max_age: v
                .get("max_age")
                .map(|d| parse_duration(d.as_str().ok_or_else(|| err("bad max_age"))?))
                .transpose()?,
        })
    }
}

/// Parse a mandate's obligations (§04.12) — the declared `obligations`
/// array plus the `counter_sign`/`binding` shorthands desugared to the
/// reserved `co_sign` instance (attestor = the owner content key, verdict
/// "approve", Δ_cosign freshness). Malformed input fails closed.
pub fn parse_obligations(
    constraints: &serde_json::Value,
    owner_content_key: &str,
) -> Result<Vec<Obligation>> {
    let mut out = Vec::new();
    if let Some(list) = constraints.get("obligations") {
        let list = list
            .as_array()
            .ok_or_else(|| Error::InvalidMandate("obligations: expected an array".into()))?;
        for v in list {
            out.push(Obligation::from_json(v)?);
        }
    }
    for key in ["counter_sign", "binding"] {
        if let Some(actions) = constraints.get(key) {
            let actions = actions.as_array().ok_or_else(|| {
                Error::InvalidMandate(format!("{key}: expected an array of actions"))
            })?;
            for a in actions {
                let name = a.as_str().ok_or_else(|| {
                    Error::InvalidMandate(format!("{key}: expected action names"))
                })?;
                out.push(Obligation {
                    id: CO_SIGN_ID.to_owned(),
                    attestor: vec![owner_content_key.to_owned()],
                    applies_to: AppliesTo::Action(name.to_owned()),
                    verdict: "approve".to_owned(),
                    max_age: Some(parse_duration(CO_SIGN_MAX_AGE)?),
                });
            }
        }
    }
    Ok(out)
}

/// The §04.12 signed payload, reconstructed from the ENTRY's coordinates —
/// binding is enforcement: a receipt signed for another mandate, action, or
/// args yields different bytes and the signature dies.
#[derive(Serialize)]
struct ObligationPayload<'a> {
    obligation: &'a str,
    mandate_id: &'a str,
    action: &'a str,
    args_hash: &'a str,
    verdict: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    presented_digest: Option<&'a str>,
    at: &'a str,
}

/// Verify ONE `checks[]` receipt against ONE obligation for the entry's
/// coordinates (leaf `mandate_id` = the entry's `authorized_by`, decided
/// 2026-07-10). Same Ed25519-over-JCS skeleton as `verify_receipt`
/// (§4.11.1) — this one gates, that one meters.
pub fn verify_obligation_receipt(
    check: &serde_json::Value,
    obligation: &Obligation,
    mandate_id: &str,
    action: &str,
    entry_args_hash: &str,
    entry_at: &str,
) -> Result<()> {
    let err = |m: String| Error::GammaObligationUnsatisfied(format!("{}: {m}", obligation.id));
    let f = |k: &str| -> Result<&str> {
        check
            .get(k)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| err(format!("receipt missing {k}")))
    };
    if f("obligation")? != obligation.id {
        return Err(err("receipt cites another obligation".into()));
    }
    // Anti-replay: the receipt binds THIS entry's args.
    if f("args_hash")? != entry_args_hash {
        return Err(err("args_hash does not match the entry".into()));
    }
    let verdict = f("verdict")?;
    if verdict != obligation.verdict {
        return Err(err(format!("verdict '{verdict}' does not satisfy")));
    }
    let at = f("at")?;
    if let Some(max_age) = obligation.max_age {
        let delta = (ts_epoch(entry_at)? - ts_epoch(at)?).abs();
        if delta > max_age {
            return Err(err(format!("receipt is stale ({delta}s > {max_age}s)")));
        }
    }
    let payload = ObligationPayload {
        obligation: &obligation.id,
        mandate_id,
        action,
        args_hash: entry_args_hash,
        verdict,
        presented_digest: check.get("presented_digest").and_then(|d| d.as_str()),
        at,
    };
    let sig: [u8; 64] = hex::decode(f("sig")?)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| err("bad sig encoding".into()))?;
    let bytes = jcs::canonical_bytes(&payload)?;
    // A valid receipt from ANY pinned attestor satisfies (§04.12).
    for key_mb in &obligation.attestor {
        let key_bytes = crate::wire::multibase_to_ed25519_pub(key_mb)?;
        let key =
            VerifyingKey::from_bytes(&key_bytes).map_err(|_| err("malformed attestor".into()))?;
        if key.verify(&bytes, &Signature::from_bytes(&sig)).is_ok() {
            return Ok(());
        }
    }
    Err(err("signature verifies under no pinned attestor".into()))
}

/// Obligation verdict for a candidate `action` entry against ONE mandate
/// (§04.12, tier V): every obligation whose `applies_to` covers the entry's
/// action must be discharged by a valid `checks[]` receipt. Fail-closed: a
/// blocked or missing receipt rejects the append — the refusal log is the
/// gateway's duty, off-protocol.
pub fn check_obligations(
    candidate: &Entry,
    constraints: &serde_json::Value,
    owner_content_key: &str,
) -> Result<()> {
    let obligations = parse_obligations(constraints, owner_content_key)?;
    if obligations.is_empty() {
        return Ok(());
    }
    let miss = Error::GammaObligationUnsatisfied;
    let action = payload_str(candidate, "action")
        .ok_or_else(|| miss("action entry without an action".into()))?;
    let connector = candidate
        .target
        .as_deref()
        .and_then(|t| t.strip_prefix("x."))
        .ok_or_else(|| miss("action entry without an x.<connector> target".into()))?;
    let op = crate::mandate::ActOp {
        connector: connector.to_owned(),
        action: action.to_owned(),
    };
    let leaf = candidate
        .authorized_by
        .as_deref()
        .ok_or_else(|| miss("action entry without authorized_by".into()))?;
    let args_hash = payload_str(candidate, "args_hash")
        .ok_or_else(|| miss("action entry without args_hash".into()))?;
    let empty = Vec::new();
    let checks = candidate
        .payload
        .as_ref()
        .and_then(|p| p.get("checks"))
        .and_then(serde_json::Value::as_array)
        .unwrap_or(&empty);
    for ob in obligations.iter().filter(|o| o.applies_to.covers(&op)) {
        let mut last = Error::GammaObligationUnsatisfied(format!(
            "{}: entry carries no receipt for this obligation",
            ob.id
        ));
        let mut ok = false;
        for check in checks
            .iter()
            .filter(|c| c.get("obligation").and_then(|o| o.as_str()) == Some(ob.id.as_str()))
        {
            match verify_obligation_receipt(check, ob, leaf, &op.action, args_hash, &candidate.at) {
                Ok(()) => {
                    ok = true;
                    break;
                }
                Err(e) => last = e,
            }
        }
        if !ok {
            return Err(last);
        }
    }
    Ok(())
}

/// Attenuation of obligations along a delegation link (§05.3, decided
/// 2026-07-10): every parent obligation must appear JCS-identical in the
/// child — a sub-mandate may ADD, never drop or alter; tightening is
/// expressed by adding. `counter_sign`/`binding` shorthands attenuate as
/// action-name supersets.
pub fn obligations_attenuate(parent: &serde_json::Value, child: &serde_json::Value) -> Result<()> {
    let err = Error::InvalidMandate;
    if let Some(list) = parent.get("obligations").and_then(|l| l.as_array()) {
        let child_list = child
            .get("obligations")
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        let child_jcs: Vec<Vec<u8>> = child_list
            .iter()
            .map(jcs::canonical_bytes)
            .collect::<Result<_>>()?;
        for ob in list {
            let bytes = jcs::canonical_bytes(ob)?;
            if !child_jcs.contains(&bytes) {
                let id = ob.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                return Err(err(format!(
                    "obligation '{id}' dropped or altered by the sub-mandate"
                )));
            }
        }
    }
    for key in ["counter_sign", "binding"] {
        if let Some(actions) = parent.get(key).and_then(|a| a.as_array()) {
            let child_set: Vec<&str> = child
                .get(key)
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            for a in actions.iter().filter_map(|x| x.as_str()) {
                if !child_set.contains(&a) {
                    return Err(err(format!("{key} '{a}' dropped by the sub-mandate")));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(json: serde_json::Value) -> Window {
        Window::from_json(&json).unwrap()
    }

    #[test]
    fn half_open_periodic_window() {
        let win = w(serde_json::json!({
            "anchor": "2026-07-02T14:00:00Z", "duration": "4h", "period": "7d"
        }));
        let t = |s: &str| ts_epoch(s).unwrap();
        assert!(win.contains(t("2026-07-02T14:00:00Z"))); // start inclusive
        assert!(win.contains(t("2026-07-02T17:59:59Z")));
        assert!(!win.contains(t("2026-07-02T18:00:00Z"))); // end exclusive
        assert!(!win.contains(t("2026-07-04T15:00:00Z"))); // between
        assert!(win.contains(t("2026-07-16T15:00:00Z"))); // occurrence 2
        assert!(!win.contains(t("2026-07-02T13:59:59Z"))); // before anchor
    }

    #[test]
    fn until_and_count_bound_occurrences() {
        let until = w(serde_json::json!({
            "anchor": "2026-07-02T14:00:00Z", "duration": "4h", "period": "7d",
            "until": "2026-07-21T14:00:00Z"
        }));
        let t = |s: &str| ts_epoch(s).unwrap();
        assert!(until.contains(t("2026-07-16T15:00:00Z")));
        assert!(!until.contains(t("2026-07-23T15:00:00Z")));

        let count = w(serde_json::json!({
            "anchor": "2026-07-02T14:00:00Z", "duration": "4h", "period": "7d",
            "count": 2
        }));
        assert!(count.contains(t("2026-07-09T15:00:00Z")));
        assert!(!count.contains(t("2026-07-16T15:00:00Z")));
    }

    #[test]
    fn attenuation_is_containment() {
        let parent = vec![w(serde_json::json!({
            "anchor": "2026-07-01T00:00:00Z", "duration": "20d"
        }))];
        let inside = vec![w(serde_json::json!({
            "anchor": "2026-07-03T14:00:00Z", "duration": "4h"
        }))];
        let outside = vec![w(serde_json::json!({
            "anchor": "2026-07-15T00:00:00Z", "duration": "25d"
        }))];
        windows_attenuate(
            Some(&parent),
            Some(&inside),
            "2026-07-01T00:00:00Z",
            "2026-07-20T00:00:00Z",
        )
        .unwrap();
        assert!(windows_attenuate(
            Some(&parent),
            Some(&outside),
            "2026-07-01T00:00:00Z",
            "2026-08-09T00:00:00Z",
        )
        .is_err());
    }
}
