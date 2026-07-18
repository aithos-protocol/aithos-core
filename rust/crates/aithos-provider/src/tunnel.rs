//! Tunnel registration — annexe B.2, contrat C2 (lot P6).
//!
//! After the pod's outbound TLS (ALPN `aithos-tunnel/1`) it sends **one
//! line** — `JCS(registration) + "\n"`, ≤ 4 KiB — signed by the gateway
//! key. [`verify_registration`] applies the **normative order** of annexe
//! B.2 and is **fail-closed**: the first failing check answers and the
//! connection closes. `now` is an injected input, never a wall-clock read
//! — the committed `p3` vector replays byte for byte.
//!
//! The relay authenticates by this signed line, **never by mTLS**: the
//! gateway key already exists (zero new secret, A3/§5). And the relay
//! never terminates the public TLS nor reads an application byte — this
//! module only decides whether to open the mux; the passthrough plumbing
//! (SNI routing, yamux) carries no application-layer logging by
//! construction, which the P6 deploy gate proves.

use aithos_core::wire;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::control::ControlPlane;
use crate::nonces::{NonceStore, Reservation};
use crate::time::parse_rfc3339z_ms;

/// The wire version of the tunnel protocol (annexe B.2).
pub const TUNNEL_WIRE_VERSION: &str = "1.0.0-draft.1";
/// Anti-abuse bound of annexe B.2: the registration line is ≤ 4 KiB.
pub const MAX_REGISTRATION_BYTES: usize = 4 * 1024;
/// Skew tolerance of annexe B.2: |now − at| ≤ 300 s, inclusive.
pub const SKEW_TOLERANCE_MS: i64 = 300_000;

/// The restricted refusal registry of annexe B.2 (a strict subset of A.7
/// plus `mapping_mismatch`). A refused registration answers one JSON line
/// `{"ok": false, "error": <code>}` then the connection closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelRefusal {
    EnvelopeInvalid,
    ClockSkew,
    NonceReplayed,
    SignatureInvalid,
    MappingMismatch,
    Suspended,
    RateLimited,
    /// Ops fail-closed (outside the wire registry): the nonce table is
    /// unreachable, so anti-rejeu cannot be guaranteed — refuse, never
    /// accept.
    Unavailable,
}

impl TunnelRefusal {
    pub fn code(self) -> &'static str {
        match self {
            TunnelRefusal::EnvelopeInvalid => "envelope_invalid",
            TunnelRefusal::ClockSkew => "clock_skew",
            TunnelRefusal::NonceReplayed => "nonce_replayed",
            TunnelRefusal::SignatureInvalid => "signature_invalid",
            TunnelRefusal::MappingMismatch => "mapping_mismatch",
            TunnelRefusal::Suspended => "suspended",
            TunnelRefusal::RateLimited => "rate_limited",
            TunnelRefusal::Unavailable => "unavailable",
        }
    }
}

/// The registration line, exactly the B.2 schema — `deny_unknown_fields`
/// enforces the closed field set (a single unknown field rejects).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    #[serde(rename = "aithos-tunnel")]
    pub version: String,
    pub tenant: String,
    pub hostname: String,
    pub gateway_pub: String,
    pub at: String,
    pub nonce: String,
    pub signature: RegistrationSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationSignature {
    pub alg: String,
    pub value: String,
}

/// The accepted registration's routing facts — what the relay pins into
/// its tunnel registry (a hostname = one active tunnel, B.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub tenant: String,
    pub hostname: String,
    pub gateway_pub: String,
}

/// Verify a registration line against annexe B.2, in normative order,
/// fail-closed. `line` is the raw bytes read from the tunnel (LF included
/// or not — a single trailing LF is tolerated and required-canonical).
pub async fn verify_registration(
    line: &[u8],
    control: &ControlPlane,
    nonces: &dyn NonceStore,
    now_ms: i64,
) -> Result<Accepted, TunnelRefusal> {
    // Size gate (B.2) before anything parses.
    if line.len() > MAX_REGISTRATION_BYTES {
        return Err(TunnelRefusal::EnvelopeInvalid);
    }
    // Exactly one optional trailing LF; the signed bytes are the JCS.
    let body = match line.strip_suffix(b"\n") {
        Some(b) => b,
        None => line,
    };
    if body.contains(&b'\n') {
        return Err(TunnelRefusal::EnvelopeInvalid);
    }
    let text = core::str::from_utf8(body).map_err(|_| TunnelRefusal::EnvelopeInvalid)?;

    // Step 0 — form: JSON, closed field set, canonical JCS, known version.
    let reg: Registration =
        serde_json::from_str(text).map_err(|_| TunnelRefusal::EnvelopeInvalid)?;
    let canonical = serde_jcs::to_string(&reg).map_err(|_| TunnelRefusal::EnvelopeInvalid)?;
    if canonical != text
        || reg.version != TUNNEL_WIRE_VERSION
        || reg.signature.alg != "ed25519"
        || reg.nonce.is_empty()
        || reg.nonce.len() > 64
    {
        return Err(TunnelRefusal::EnvelopeInvalid);
    }
    let Some(at_ms) = parse_rfc3339z_ms(&reg.at) else {
        return Err(TunnelRefusal::EnvelopeInvalid);
    };

    // Step 1 — skew ±300 s, inclusive.
    if (now_ms - at_ms).abs() > SKEW_TOLERANCE_MS {
        return Err(TunnelRefusal::ClockSkew);
    }

    // Step 2 — nonce burns on first sight (keyed by gateway_pub), BEFORE
    // the signature, exactly as B.2 orders it and as the store does (#6).
    match nonces
        .reserve(&reg.gateway_pub, &reg.nonce, now_ms)
        .await
        .map_err(|_| TunnelRefusal::Unavailable)?
    {
        Reservation::Fresh => {}
        Reservation::Replayed => return Err(TunnelRefusal::NonceReplayed),
    }

    // Step 3 — signature under gateway_pub (JCS with value="", §01.4).
    let key_bytes = wire::multibase_to_ed25519_pub(&reg.gateway_pub)
        .map_err(|_| TunnelRefusal::SignatureInvalid)?;
    let verifying_key =
        VerifyingKey::from_bytes(&key_bytes).map_err(|_| TunnelRefusal::SignatureInvalid)?;
    let mut unsigned = reg.clone();
    unsigned.signature.value = String::new();
    let unsigned_jcs =
        serde_jcs::to_string(&unsigned).map_err(|_| TunnelRefusal::EnvelopeInvalid)?;
    let sig_bytes = hex::decode(&reg.signature.value)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .ok_or(TunnelRefusal::SignatureInvalid)?;
    verifying_key
        .verify(unsigned_jcs.as_bytes(), &Signature::from_bytes(&sig_bytes))
        .map_err(|_| TunnelRefusal::SignatureInvalid)?;

    // Step 4 — control-plane mapping: resolve by gateway_pub, then
    // suspended, then exact (tenant, hostname) match. A key enrolled for
    // no tunnel and a key enrolled for a different hostname answer the
    // SAME `mapping_mismatch` — no enumeration oracle.
    let Some(binding) = control.resolve_tunnel(&reg.gateway_pub) else {
        return Err(TunnelRefusal::MappingMismatch);
    };
    if binding.suspended {
        return Err(TunnelRefusal::Suspended);
    }
    if binding.tenant != reg.tenant || binding.hostname != reg.hostname {
        return Err(TunnelRefusal::MappingMismatch);
    }

    Ok(Accepted {
        tenant: reg.tenant,
        hostname: reg.hostname,
        gateway_pub: reg.gateway_pub,
    })
}

/// The relay's one-line answer (B.2): `{"aithos-tunnel": "…", "ok": true}`
/// on success, `{"ok": false, "error": <code>}` on refusal. Emitted as
/// exactly these JCS bytes; the caller appends the framing LF.
pub fn answer(result: &Result<Accepted, TunnelRefusal>) -> String {
    match result {
        Ok(_) => serde_json::json!({"aithos-tunnel": TUNNEL_WIRE_VERSION, "ok": true}).to_string(),
        Err(refusal) => serde_json::json!({"ok": false, "error": refusal.code()}).to_string(),
    }
}

/// Sign a registration under the §01.4 convention. **Client/tooling
/// surface only** (the gateway signs; the relay holds no key to sign
/// with). Used by the p3 replay and the future gateway client (G1).
pub fn sign_registration(mut reg: Registration, key: &ed25519_dalek::SigningKey) -> Registration {
    use ed25519_dalek::Signer as _;
    reg.signature.value = String::new();
    let unsigned = serde_jcs::to_string(&reg).expect("registration is serializable");
    reg.signature.value = hex::encode(key.sign(unsigned.as_bytes()).to_bytes());
    reg
}

/// The wire line of a signed registration: `JCS + "\n"` (B.2).
pub fn registration_line(reg: &Registration) -> String {
    format!(
        "{}\n",
        serde_jcs::to_string(reg).expect("registration is serializable")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_restricted_registry_is_a_subset_of_a7_plus_mapping() {
        // B.2 codes exactly (mapping_mismatch is the only addition vs A.7).
        for (refusal, code) in [
            (TunnelRefusal::EnvelopeInvalid, "envelope_invalid"),
            (TunnelRefusal::ClockSkew, "clock_skew"),
            (TunnelRefusal::NonceReplayed, "nonce_replayed"),
            (TunnelRefusal::SignatureInvalid, "signature_invalid"),
            (TunnelRefusal::MappingMismatch, "mapping_mismatch"),
            (TunnelRefusal::Suspended, "suspended"),
            (TunnelRefusal::RateLimited, "rate_limited"),
        ] {
            assert_eq!(refusal.code(), code);
        }
    }

    #[test]
    fn registration_jcs_matches_the_committed_wire_order() {
        // JCS sorts keys: aithos-tunnel, at, gateway_pub, hostname, nonce,
        // signature, tenant — the exact layout of the p3 vector line.
        let reg = Registration {
            version: TUNNEL_WIRE_VERSION.into(),
            tenant: "acme".into(),
            hostname: "demo.mcp.aithos.fr".into(),
            gateway_pub: "z6MksPykuQeYh4zgthFRFBExrgo1dwFWWenY2TEJ9SvT9jn1".into(),
            at: "2026-07-16T12:00:00Z".into(),
            nonce: "p0-t-ok-01".into(),
            signature: RegistrationSignature {
                alg: "ed25519".into(),
                value: String::new(),
            },
        };
        let jcs = serde_jcs::to_string(&reg).unwrap();
        assert!(jcs.starts_with(
            r#"{"aithos-tunnel":"1.0.0-draft.1","at":"2026-07-16T12:00:00Z","gateway_pub":"#
        ));
        assert!(jcs.ends_with(r#""tenant":"acme"}"#));
    }
}
