//! The delegated ACME DNS-01 surface — annexe B.5, contrat C2 (lot P6,
//! jalon M2). Graved by `vectors/p6-acme-txt.json` and
//! `tests/features/store/store-acme.feature`.
//!
//! `PUT`/`DELETE /acme/txt` pose/retire `TXT _acme-challenge.<hostname>`
//! (TTL 60 s) so the POD can obtain and renew its own public certificate
//! — the private key never exists server-side (A3). The envelope is the
//! A.2 wire form with the **graved B.5 exception**: `key = gateway_pub`
//! (multibase) and `mandate: []` — the authority is the control-plane
//! mapping of the signing gateway key (the B.2 model), never a mandate
//! chain. There is no DID and no path-map on this surface.
//!
//! **Normative order (fail-closed, first error answers):**
//! presence → envelope form (`parse_envelope_form`, plus the B.5 form:
//! multibase key, empty mandate) → host/method/path byte-identity →
//! body_b3 → skew ±300 s → nonce reservation → signature under
//! `gateway_pub` → verb (PUT/DELETE, else `not_covered`) → body form
//! (closed `{hostname, value}`, strict grammars) → mapping (resolve →
//! suspended → tenant state → hostname match) → rate (PUT only, ≤ 10 per
//! rolling hour per hostname, counted AFTER full authorization) → DNS
//! effect. Errors: registre A.7 + `mapping_mismatch`.
//!
//! Hygiene: every record this service poses is remembered and purged
//! after 10 minutes regardless of the client's DELETE (B.5: « de toute
//! façon purgé après 10 min ») — [`AcmeState::purge_stale`], driven by
//! the binary's timer and by the BDD clock.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;

use crate::control::{ControlPlane, TenantState};
use crate::dns::DnsTxt;
use crate::envelope::{parse_envelope_form, verify_envelope_signature, Refusal, RequestFacts};
use crate::nonces::{NonceStore, Reservation};

/// The one route of the B.5 surface (exact, no query).
pub const ACME_PATH: &str = "/acme/txt";
/// B.5 bound: the challenge value is ≤ 255 characters.
pub const MAX_VALUE_CHARS: usize = 255;
/// B.5 anti-abus: at most 10 PUT per rolling hour per hostname.
pub const MAX_PUTS_PER_HOUR: usize = 10;
/// The rolling window of the PUT budget.
pub const RATE_WINDOW_MS: i64 = 3_600_000;
/// B.5: a posed record is purged server-side after 10 minutes.
pub const PURGE_AFTER_MS: i64 = 600_000;

/// The closed request body — one unknown field rejects (B.5 form).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcmeBody {
    pub hostname: String,
    pub value: String,
}

/// What an accepted request proved — the log line's verified tenant
/// (A.8: the register is closed; the hostname and the value never log).
pub struct AcmeAccepted {
    pub tenant: String,
}

/// Mutable surface state: the PUT budget and the posed-record ledger the
/// purge sweeps. Time is always injected (`now_ms`) — the vector replays
/// with no wall clock.
#[derive(Default)]
pub struct AcmeState {
    /// hostname → admission instants (ms) within the rolling hour.
    admitted: Mutex<HashMap<String, Vec<i64>>>,
    /// Records this process posed: `(record name, value, posed_at_ms)`.
    posted: Mutex<Vec<PostedTxt>>,
}

struct PostedTxt {
    name: String,
    value: String,
    at_ms: i64,
}

impl AcmeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spend one PUT admission for `hostname` if the rolling-hour budget
    /// allows. Called only after FULL authorization — a stranger cannot
    /// burn a hostname's budget (same placement as the B.2 anti-flap).
    fn admit_put(&self, hostname: &str, now_ms: i64) -> bool {
        let mut admitted = self.admitted.lock().expect("rate table poisoned");
        let times = admitted.entry(hostname.to_owned()).or_default();
        times.retain(|t| now_ms - *t < RATE_WINDOW_MS);
        if times.len() >= MAX_PUTS_PER_HOUR {
            return false;
        }
        times.push(now_ms);
        true
    }

    fn remember_posted(&self, name: &str, value: &str, now_ms: i64) {
        let mut posted = self.posted.lock().expect("posted ledger poisoned");
        posted.retain(|p| p.name != name);
        posted.push(PostedTxt {
            name: name.to_owned(),
            value: value.to_owned(),
            at_ms: now_ms,
        });
    }

    fn forget_posted(&self, name: &str, value: &str) {
        self.posted
            .lock()
            .expect("posted ledger poisoned")
            .retain(|p| !(p.name == name && p.value == value));
    }

    /// Retire every record posed more than 10 minutes ago (B.5). Returns
    /// how many were swept. Failures stay in the ledger and retry at the
    /// next sweep — the purge never gives up on a record it posed.
    pub async fn purge_stale(&self, dns: &dyn DnsTxt, now_ms: i64) -> usize {
        let stale: Vec<(String, String)> = {
            let posted = self.posted.lock().expect("posted ledger poisoned");
            posted
                .iter()
                .filter(|p| now_ms - p.at_ms >= PURGE_AFTER_MS)
                .map(|p| (p.name.clone(), p.value.clone()))
                .collect()
        };
        let mut swept = 0;
        for (name, value) in stale {
            if dns.delete(&name, &value).await.is_ok() {
                self.forget_posted(&name, &value);
                swept += 1;
            }
        }
        swept
    }
}

/// Strict lowercase LDH hostname: ≥ 2 labels, 1..=63 chars each, no
/// leading/trailing hyphen, ≤ 253 total, no trailing dot. The client
/// posts its ENROLLED name verbatim — no case games on this surface
/// (graved by the p6 vector; B.4's case-insensitivity is SNI matching,
/// not this API).
fn valid_hostname(h: &str) -> bool {
    if h.is_empty() || h.len() > 253 || !h.contains('.') {
        return false;
    }
    h.split('.').all(|label| {
        let b = label.as_bytes();
        !b.is_empty()
            && b.len() <= 63
            && b[0] != b'-'
            && b[b.len() - 1] != b'-'
            && b.iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
    })
}

/// The challenge value: 1..=255 chars of the base64url alphabet — the
/// ACME digest shape. Nothing else ever reaches DNS through this surface.
fn valid_value(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= MAX_VALUE_CHARS
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

/// Decide one `/acme/txt` request end to end (verification + effect), in
/// the normative B.5 order, fail-closed. `now_ms` is injected — the p6
/// vector replays byte for byte with the test clock.
#[allow(clippy::too_many_arguments)]
pub async fn decide_acme(
    header: Option<&str>,
    facts: &RequestFacts<'_>,
    control: &ControlPlane,
    nonces: &dyn NonceStore,
    state: &AcmeState,
    dns: &dyn DnsTxt,
    now_ms: i64,
) -> Result<AcmeAccepted, Refusal> {
    // Presence — every /acme route demands the envelope (never a banner).
    let Some(header) = header else {
        return Err(Refusal::EnvelopeMissing);
    };

    // Envelope form (the shared A.2 #2 block), then the B.5 form: the key
    // IS a gateway public key and the mandate list IS empty — a chain or
    // an owner fragment here is a FORM fault, never evaluated (there is
    // no DID and no chain machinery on this surface).
    let (envelope, at_ms) = parse_envelope_form(header)?;
    if !envelope.key.starts_with('z') || !envelope.mandate.is_empty() {
        return Err(Refusal::EnvelopeInvalid);
    }

    // Host, method, path: byte identity with the received request AND
    // with the deployed authority (anti cross-plane replay, A.2 #3).
    if envelope.host != facts.authority
        || envelope.host != facts.expected_authority
        || envelope.method != facts.method
        || envelope.path != facts.target
    {
        return Err(Refusal::EnvelopeInvalid);
    }

    // body_b3 = BLAKE3(raw body) — the bytes bind, not the JSON (A.2 #4).
    let want_b3 = if facts.body.is_empty() {
        String::new()
    } else {
        blake3::hash(facts.body).to_hex().to_string()
    };
    if envelope.body_b3 != want_b3 {
        return Err(Refusal::EnvelopeInvalid);
    }

    // Skew ±300 s, inclusive (A.2 #5).
    if (now_ms - at_ms).abs() > crate::envelope::SKEW_TOLERANCE_MS {
        return Err(Refusal::ClockSkew);
    }

    // Nonce reservation, insert-if-absent, BEFORE any side effect — the
    // nonce burns even when the mapping later refuses (A.2 #6).
    match nonces
        .reserve(&envelope.key, &envelope.nonce, now_ms)
        .await
        .map_err(|_| Refusal::Unavailable)?
    {
        Reservation::Fresh => {}
        Reservation::Replayed => return Err(Refusal::NonceReplayed),
    }

    // Signature under gateway_pub — the envelope's own key, decoded the
    // B.2 way (an undecodable key can sign nothing: signature_invalid).
    let key_bytes = aithos_core::wire::multibase_to_ed25519_pub(&envelope.key)
        .map_err(|_| Refusal::SignatureInvalid)?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| Refusal::SignatureInvalid)?;
    verify_envelope_signature(&envelope, &verifying_key)?;

    // Verb: the route defines PUT and DELETE only — default deny, decided
    // AFTER the envelope authenticated (the A.3 dispatch model).
    if facts.method != "PUT" && facts.method != "DELETE" {
        return Err(Refusal::NotCovered);
    }

    // Body form: closed field set, strict grammars. The body is data the
    // effect names — a malformed one is a request-form fault (B.5).
    let body: AcmeBody =
        serde_json::from_slice(facts.body).map_err(|_| Refusal::EnvelopeInvalid)?;
    if !valid_hostname(&body.hostname) || !valid_value(&body.value) {
        return Err(Refusal::EnvelopeInvalid);
    }

    // Mapping — the graved B.5 authority: resolve by gateway_pub, then
    // suspension (binding, then tenant), then the exact hostname. A key
    // enrolled nowhere, a binding onto an unknown tenant and a foreign
    // hostname all answer the SAME mapping_mismatch (no oracle).
    let Some(binding) = control.resolve_tunnel(&envelope.key) else {
        return Err(Refusal::MappingMismatch);
    };
    if binding.suspended {
        return Err(Refusal::Suspended);
    }
    match control.tenant_state(&binding.tenant) {
        TenantState::Unknown => return Err(Refusal::MappingMismatch),
        TenantState::Suspended => return Err(Refusal::Suspended),
        TenantState::Active => {}
    }
    if binding.hostname != body.hostname {
        return Err(Refusal::MappingMismatch);
    }

    // Anti-abus (PUT only), counted AFTER full authorization.
    if facts.method == "PUT" && !state.admit_put(&body.hostname, now_ms) {
        return Err(Refusal::RateLimited);
    }

    // The effect. `_acme-challenge.<hostname>`, TTL 60 (the seam's
    // constant). A backend failure refuses 503 — fail-closed, never a
    // silent acceptance.
    let record = format!("_acme-challenge.{}", body.hostname);
    match facts.method {
        "PUT" => {
            dns.upsert(&record, &body.value)
                .await
                .map_err(|_| Refusal::Unavailable)?;
            state.remember_posted(&record, &body.value, now_ms);
        }
        _ => {
            dns.delete(&record, &body.value)
                .await
                .map_err(|_| Refusal::Unavailable)?;
            state.forget_posted(&record, &body.value);
        }
    }
    Ok(AcmeAccepted {
        tenant: binding.tenant.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::MemDnsTxt;

    #[test]
    fn hostname_grammar_is_strict_lowercase_ldh() {
        for ok in ["demo.mcp.aithos.fr", "a-b.mcp.aithos.fr", "x1.y2"] {
            assert!(valid_hostname(ok), "should accept {ok}");
        }
        for bad in [
            "",
            "single-label",
            "DeMo.McP.AiThOs.Fr",
            "demo.mcp.aithos.fr.",
            ".demo.mcp.aithos.fr",
            "-demo.mcp.aithos.fr",
            "demo-.mcp.aithos.fr",
            "demo..mcp.aithos.fr",
            "demo.mcp.aithos.fr/evil",
            "demo.mcp.aithos.fr evil",
            &("a".repeat(64) + ".fr"),
            &(format!(
                "{}.{}.{}.{}.fr",
                "a".repeat(63),
                "b".repeat(63),
                "c".repeat(63),
                "d".repeat(63)
            )),
        ] {
            assert!(!valid_hostname(bad), "should reject {bad:?}");
        }
    }

    #[test]
    fn value_grammar_is_the_base64url_alphabet_bounded() {
        assert!(valid_value("tok_A-1"));
        assert!(valid_value(&"A".repeat(255)));
        for bad in ["", "no spaces allowed", "quote\"break", "dot.break"] {
            assert!(!valid_value(bad), "should reject {bad:?}");
        }
        assert!(!valid_value(&"A".repeat(256)));
    }

    #[test]
    fn the_put_budget_is_ten_per_rolling_hour_per_hostname() {
        let state = AcmeState::new();
        for i in 0..MAX_PUTS_PER_HOUR {
            assert!(state.admit_put("demo.mcp.aithos.fr", 1_000 + i as i64));
        }
        // The 11th inside the hour is refused; refusals are not admissions.
        assert!(!state.admit_put("demo.mcp.aithos.fr", 2_000));
        assert!(!state.admit_put("demo.mcp.aithos.fr", 3_000));
        // Another hostname has its own budget.
        assert!(state.admit_put("rate.mcp.aithos.fr", 2_000));
        // The rolling hour frees it.
        assert!(state.admit_put("demo.mcp.aithos.fr", 1_000 + RATE_WINDOW_MS));
    }

    #[test]
    fn the_purge_sweeps_only_records_past_ten_minutes() {
        let state = AcmeState::new();
        let dns = MemDnsTxt::new();
        futures::executor::block_on(async {
            dns.upsert("_acme-challenge.a.mcp", "va").await.unwrap();
            dns.upsert("_acme-challenge.b.mcp", "vb").await.unwrap();
            state.remember_posted("_acme-challenge.a.mcp", "va", 0);
            state.remember_posted("_acme-challenge.b.mcp", "vb", 5_000);
            // At t = PURGE_AFTER_MS only the first is stale.
            let swept = state.purge_stale(&dns, PURGE_AFTER_MS).await;
            assert_eq!(swept, 1);
            assert!(dns.record_of("_acme-challenge.a.mcp").is_none());
            assert!(dns.record_of("_acme-challenge.b.mcp").is_some());
            // A later sweep takes the second.
            let swept = state.purge_stale(&dns, PURGE_AFTER_MS + 5_000).await;
            assert_eq!(swept, 1);
            assert!(dns.record_of("_acme-challenge.b.mcp").is_none());
        });
    }

    #[test]
    fn a_fresh_put_replaces_the_remembered_record_and_its_clock() {
        let state = AcmeState::new();
        state.remember_posted("_acme-challenge.a.mcp", "old", 0);
        state.remember_posted("_acme-challenge.a.mcp", "new", 400_000);
        let dns = MemDnsTxt::new();
        futures::executor::block_on(async {
            dns.upsert("_acme-challenge.a.mcp", "new").await.unwrap();
            // At t=600 000 the OLD stamp would be stale, but the record was
            // re-posed at 400 000: nothing sweeps.
            assert_eq!(state.purge_stale(&dns, 600_000).await, 0);
            assert!(dns.record_of("_acme-challenge.a.mcp").is_some());
        });
    }
}
