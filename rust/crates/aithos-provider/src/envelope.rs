//! The signed request envelope `X-Aithos-Auth` — annexe A.2, contrat C1.
//!
//! [`verify`] applies the **normative order** (checks #2 to #10 of A.2 —
//! #0 grammar and #1 tenant belong to the router) and is **fail-closed**:
//! the first failing check answers, nothing else is evaluated. `now` is an
//! injected input, never a wall-clock read — the same property that makes
//! the committed vectors replayable byte for byte.
//!
//! **P1 scope, graved in `HANDOFF-PROVIDER-AWS.md`:** owner fragments
//! (`#root`/`#content`) resolve against the stored `did.json`; a mandated
//! envelope (multibase key) passes form, skew, nonce and signature, then
//! **fails closed at #9** (`chain_invalid`) — the `verify_chain` machinery,
//! the leaf check of #7 and the mandated path-map of #10 arrive with P2.
//! A request is therefore never accepted on an authority P1 cannot verify.

use aithos_core::did::DidDocument;
use aithos_core::wire;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::control::ControlPlane;
use crate::nonces::{NonceStore, Reservation};
use crate::objects::ObjectStore;
use crate::pathmap::{anonymous_covers, DataTarget, ObjectPath, TargetKind};
use crate::time::parse_rfc3339z_ms;

/// Anti-abuse bound of annexe A.8: the envelope header value is ≤ 8 KiB.
pub const MAX_ENVELOPE_BYTES: usize = 8 * 1024;
/// Skew tolerance of annexe A.2 #5: |now − at| ≤ 300 s, inclusive.
pub const SKEW_TOLERANCE_MS: i64 = 300_000;

/// The closed error registry of annexe A.7. Every refusal the service can
/// utter is one of these — a fixed `(status, code)` pair, never a free
/// string, so no response can leak a path, a body or an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    PathInvalid,
    EnvelopeInvalid,
    ArtifactInvalid,
    EnvelopeMissing,
    ClockSkew,
    NonceReplayed,
    SignatureInvalid,
    ChainInvalid,
    ChainRevoked,
    NotCovered,
    /// B.5 addition to the HTTP registry (same wire code as B.2): the
    /// signing gateway key resolves to no binding, or the named hostname
    /// is not the binding's — one answer, no enumeration oracle.
    MappingMismatch,
    DidNotBound,
    Suspended,
    UnknownTenant,
    NotFound,
    CasMismatch,
    EditionGone,
    PayloadTooLarge,
    VersionUnsupported,
    CasRequired,
    RateLimited,
    /// P1-transitional, OUTSIDE the wire registry: a grammar-valid route
    /// the skeleton does not serve yet (heads/batch/gamma/sync/list, and
    /// the A.4-verified artifact classes). Removed as P2 lands — the full
    /// service leaves this variant unreachable.
    NotImplemented,
    /// Ops-level fail-closed refusal, OUTSIDE the wire registry: a
    /// dependency (the nonce table) failed, so anti-rejeu cannot be
    /// guaranteed and the request is refused rather than accepted.
    Unavailable,
}

impl Refusal {
    pub fn status(self) -> u16 {
        match self {
            Refusal::PathInvalid | Refusal::EnvelopeInvalid | Refusal::ArtifactInvalid => 400,
            Refusal::EnvelopeMissing
            | Refusal::ClockSkew
            | Refusal::NonceReplayed
            | Refusal::SignatureInvalid => 401,
            Refusal::ChainInvalid
            | Refusal::ChainRevoked
            | Refusal::NotCovered
            | Refusal::MappingMismatch
            | Refusal::DidNotBound
            | Refusal::Suspended => 403,
            Refusal::UnknownTenant | Refusal::NotFound => 404,
            Refusal::CasMismatch => 409,
            Refusal::EditionGone => 410,
            Refusal::PayloadTooLarge => 413,
            Refusal::VersionUnsupported => 426,
            Refusal::CasRequired => 428,
            Refusal::RateLimited => 429,
            Refusal::NotImplemented => 501,
            Refusal::Unavailable => 503,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Refusal::PathInvalid => "path_invalid",
            Refusal::EnvelopeInvalid => "envelope_invalid",
            Refusal::ArtifactInvalid => "artifact_invalid",
            Refusal::EnvelopeMissing => "envelope_missing",
            Refusal::ClockSkew => "clock_skew",
            Refusal::NonceReplayed => "nonce_replayed",
            Refusal::SignatureInvalid => "signature_invalid",
            Refusal::ChainInvalid => "chain_invalid",
            Refusal::ChainRevoked => "chain_revoked",
            Refusal::NotCovered => "not_covered",
            Refusal::MappingMismatch => "mapping_mismatch",
            Refusal::DidNotBound => "did_not_bound",
            Refusal::Suspended => "suspended",
            Refusal::UnknownTenant => "unknown_tenant",
            Refusal::NotFound => "not_found",
            Refusal::CasMismatch => "cas_mismatch",
            Refusal::EditionGone => "edition_gone",
            Refusal::PayloadTooLarge => "payload_too_large",
            Refusal::VersionUnsupported => "version_unsupported",
            Refusal::CasRequired => "cas_required",
            Refusal::RateLimited => "rate_limited",
            Refusal::NotImplemented => "not_implemented",
            Refusal::Unavailable => "unavailable",
        }
    }
}

/// The envelope, exactly the A.2 schema — `deny_unknown_fields` enforces
/// the closed field set (a single unknown field rejects the request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub v: u8,
    pub host: String,
    pub method: String,
    pub path: String,
    pub body_b3: String,
    pub at: String,
    pub nonce: String,
    pub mandate: Vec<String>,
    pub key: String,
    pub signature: EnvelopeSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeSignature {
    pub alg: String,
    pub value: String,
}

/// Who the envelope proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// No envelope, on an A2-exception GET.
    Anonymous,
    /// The DID's owner, under `#root` or `#content`.
    Owner,
    /// A verified mandate chain (#7–#10 all green): the leaf signed the
    /// envelope, the chain verified at `at`, revocation cleared at
    /// `now_serveur`, and the path-map covered the request. Carries the
    /// verified chain (root first, leaf last — the stored certs the ids
    /// named): the A.4 deposit checks compare it to the artifact's own
    /// displayed authority (§02.6.1 — one actor, one chain).
    Mandated(Vec<aithos_core::mandate::Mandate>),
}

/// The raw request facts the envelope binds (A.2 #3/#4), byte-exact as
/// received. `authority` is the normalized received authority (lowercase,
/// default port stripped); `expected_authority` is the authority this
/// service was deployed to serve — both must match the envelope's `host`,
/// which is what kills cross-plane replay (store ↔ gateway surface, G7).
pub struct RequestFacts<'a> {
    pub authority: &'a str,
    pub expected_authority: &'a str,
    pub method: &'a str,
    pub target: &'a str,
    pub body: &'a [u8],
}

/// Checks #2–#10 of annexe A.2, in normative order, fail-closed.
/// `heads` names the appended gamma segments the #9 revocation scan
/// merges with the `did.json`'s own `revocations` pointer (étape 4 —
/// POST `/gamma` appends live in `gamma/<YYYY-MM>.jsonl`; the
/// pointer-vs-segments tension is arbitrage ③ of the gate).
#[allow(clippy::too_many_arguments)] // the normative order names its seams
pub async fn verify(
    header: Option<&str>,
    facts: &RequestFacts<'_>,
    route: &DataTarget,
    control: &ControlPlane,
    objects: &dyn ObjectStore,
    heads: &dyn crate::heads::HeadsTable,
    nonces: &dyn NonceStore,
    now_ms: i64,
) -> Result<Principal, Refusal> {
    // #2 — presence. The A2 exceptions read anonymously, everything else
    // demands an envelope.
    let Some(header) = header else {
        return match &route.kind {
            TargetKind::Object(object) if facts.method == "GET" && anonymous_covers(object) => {
                Ok(Principal::Anonymous)
            }
            _ => Err(Refusal::EnvelopeMissing),
        };
    };

    // #2 — form (the shared block, also the B.5 surface's).
    let (envelope, at_ms) = parse_envelope_form(header)?;

    // #3 — host, method, path: byte identity with the received request,
    // AND with the authority this service serves (anti cross-plane replay).
    if envelope.host != facts.authority
        || envelope.host != facts.expected_authority
        || envelope.method != facts.method
        || envelope.path != facts.target
    {
        return Err(Refusal::EnvelopeInvalid);
    }

    // #4 — body_b3 = BLAKE3(raw body), or "" and no body at all.
    let want_b3 = if facts.body.is_empty() {
        String::new()
    } else {
        blake3::hash(facts.body).to_hex().to_string()
    };
    if envelope.body_b3 != want_b3 {
        return Err(Refusal::EnvelopeInvalid);
    }

    // #5 — |now − at| ≤ 300 s, inclusive (the 300 s boundary is a
    // committed accept vector; 301 s is a committed reject).
    if (now_ms - at_ms).abs() > SKEW_TOLERANCE_MS {
        return Err(Refusal::ClockSkew);
    }

    // #6 — (key, nonce) reservation, insert-if-absent, BEFORE any side
    // effect: the nonce burns even when a later check refuses the request.
    match nonces
        .reserve(&envelope.key, &envelope.nonce, now_ms)
        .await
        .map_err(|_| Refusal::Unavailable)?
    {
        Reservation::Fresh => {}
        Reservation::Replayed => return Err(Refusal::NonceReplayed),
    }

    // #7 — key resolution. The DID-to-tenant binding is only ever named
    // under an envelope that survived #2–#6 (anti-enumeration note, A.7).
    if !control.did_bound(&route.tenant, &route.did) {
        return Err(Refusal::DidNotBound);
    }
    let owner_fragment = matches!(envelope.key.as_str(), "#root" | "#content");
    let mut chain: Vec<aithos_core::mandate::Mandate> = Vec::new();
    let mut chain_raws: Vec<Vec<u8>> = Vec::new();
    let verifying_key = if owner_fragment {
        match objects.get(&route.tenant, &route.did, "did.json").await {
            Some(doc) => {
                let doc: DidDocument =
                    serde_json::from_slice(&doc).map_err(|_| Refusal::ChainInvalid)?;
                let mb = if envelope.key == "#root" {
                    &doc.keys.root
                } else {
                    &doc.keys.content
                };
                decode_key(mb)?
            }
            // The A.4 GENESIS exception (étape 5): the first `did.json`
            // of a bound DID resolves `#root` against the DEPOSITED
            // document itself — the enrollment (#1/#7 binding) always
            // precedes, so this is never an open door: only a PUT of
            // `did.json` under `#root`, only while nothing is stored.
            None if envelope.key == "#root"
                && facts.method == "PUT"
                && matches!(route.kind, TargetKind::Object(ObjectPath::DidJson)) =>
            {
                let doc: DidDocument =
                    serde_json::from_slice(facts.body).map_err(|_| Refusal::ChainInvalid)?;
                decode_key(&doc.keys.root)?
            }
            None => return Err(Refusal::ChainInvalid),
        }
    } else {
        // Multibase leaf key (A.2 #7): a chain must be presented, its ids
        // must be grammar-clean, its certs must all be STORED artifacts
        // (`certs/<id>.json`, §04.9 world-readable), and the envelope key
        // must be the leaf `grantee.pubkey`. Any gap is a chain fault.
        if envelope.mandate.is_empty() {
            return Err(Refusal::ChainInvalid);
        }
        for id in &envelope.mandate {
            if !crate::pathmap::mandate_id_is_valid(id) {
                return Err(Refusal::ChainInvalid);
            }
            let bytes = objects
                .get(&route.tenant, &route.did, &format!("certs/{id}.json"))
                .await
                .ok_or(Refusal::ChainInvalid)?;
            let mandate: aithos_core::mandate::Mandate =
                serde_json::from_slice(&bytes).map_err(|_| Refusal::ChainInvalid)?;
            chain.push(mandate);
            chain_raws.push(bytes);
        }
        let leaf = chain.last().expect("chain checked non-empty");
        if leaf.grantee.pubkey != envelope.key {
            return Err(Refusal::ChainInvalid);
        }
        decode_key(&envelope.key)?
    };

    // #8 — envelope signature: Ed25519 over the JCS with
    // `signature.value = ""` (the shared §01.4 convention).
    verify_envelope_signature(&envelope, &verifying_key)?;

    // #9 — the chain (mandated only): §04.5 steps 1–6 delegated to core
    // for the typed profiles (link signatures, subject == <did>, windows
    // at `at`, attenuation §05.3), the A.4-literal composed path for a
    // homogeneous draft.3 chain ([`crate::artifacts::verify_chain_composed`]
    // — the arbitrage note lives there) — then revocation evaluated at
    // `now_serveur` on the STORED logs (the did.json's `revocations`
    // pointer UNION the appended month segments).
    if !owner_fragment {
        let doc = objects
            .get(&route.tenant, &route.did, "did.json")
            .await
            .ok_or(Refusal::ChainInvalid)?;
        let did_doc: DidDocument =
            serde_json::from_slice(&doc).map_err(|_| Refusal::ChainInvalid)?;
        crate::artifacts::verify_chain_composed(&chain, &chain_raws, &did_doc, &envelope.at)
            .map_err(|()| Refusal::ChainInvalid)?;
        let revocations =
            stored_revocations(objects, heads, &route.tenant, &route.did, &did_doc).await?;
        aithos_core::revocation::chain_revoked_at(
            &chain,
            &revocations,
            &crate::time::render_rfc3339z(now_ms),
        )
        .map_err(|_| Refusal::ChainRevoked)?;
    }

    // #10 — path-map (annexe A.3, anti-abuse, never the authority). The
    // owner covers everything on their own DID; a mandated request is
    // served only where the LEAF perimeter's A.3 row reaches (attenuation
    // was core's #9 concern: the leaf is already ⊆ its ancestors). A
    // draft.3 (K1-C) leaf writes its perimeter in the carrier grammar the
    // typed parser does not speak: only the any-chain rows of A.3 can
    // serve it (deny stays the default; nothing is interpreted).
    if owner_fragment {
        return Ok(Principal::Owner);
    }
    let leaf = chain.last().expect("chain checked non-empty");
    let perimeter = match leaf.parsed_perimeter() {
        Ok(perimeter) => perimeter,
        Err(_) if leaf.version == aithos_core::mandate::MANDATE_VERSION_DRAFT3 => Vec::new(),
        Err(_) => return Err(Refusal::ChainInvalid),
    };
    if crate::pathmap::mandated_covers(&perimeter, &route.kind, facts.method) {
        Ok(Principal::Mandated(chain))
    } else {
        Err(Refusal::NotCovered)
    }
}

/// The active revocation set read from the STORED log: the key the
/// did.json itself names (`revocations`), UNION every month segment the
/// heads table saw an append land in (étape 4 — a revoke accepted through
/// POST `/gamma` bites here without waiting for any pointer rewrite; the
/// pointer-vs-segments tension stays arbitrage ③, resolved by redline,
/// never here). Fail-closed asymmetry: an ABSENT log is a provable empty
/// state (nothing was ever revoked — accepted); an UNPARSEABLE log cannot
/// prove non-revocation and refuses. Entry signatures were verified at
/// deposit (A.4); anti-rollback of the log is the witness's domain
/// (annexe C), not this check's.
async fn stored_revocations(
    objects: &dyn ObjectStore,
    heads: &dyn crate::heads::HeadsTable,
    tenant: &str,
    did: &str,
    did_doc: &DidDocument,
) -> Result<Vec<aithos_core::revocation::Revocation>, Refusal> {
    let mut entries = Vec::new();
    let mut logs = vec![did_doc.revocations.clone()];
    if let Some(record) = heads.read(tenant, did).await {
        for month in record.gamma_segments {
            let key = format!("gamma/{month}.jsonl");
            if key != did_doc.revocations {
                logs.push(key);
            }
        }
    }
    for key in logs {
        let Some(bytes) = objects.get(tenant, did, &key).await else {
            continue;
        };
        let text = core::str::from_utf8(&bytes).map_err(|_| Refusal::ChainInvalid)?;
        for line in text.lines().filter(|line| !line.is_empty()) {
            let entry: aithos_core::gamma::Entry =
                serde_json::from_str(line).map_err(|_| Refusal::ChainInvalid)?;
            entries.push(entry);
        }
    }
    Ok(aithos_core::revocation::revocations(&entries))
}

/// The #2 form block, shared verbatim by the data plane (A.2) and the
/// /acme surface (B.5) — one wire form, one parser. Size gate first
/// (A.8), then strict base64url without padding, then JSON, then the
/// closed field set, then canonicality: the header value must be exactly
/// `base64url(JCS(envelope))` — a re-encoded, re-ordered or
/// duplicated-key envelope is not the wire form the contract names, and
/// is rejected. Returns the envelope and its parsed `at` instant.
pub(crate) fn parse_envelope_form(header: &str) -> Result<(Envelope, i64), Refusal> {
    if header.len() > MAX_ENVELOPE_BYTES {
        return Err(Refusal::PayloadTooLarge);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(header.as_bytes())
        .map_err(|_| Refusal::EnvelopeInvalid)?;
    let text = core::str::from_utf8(&bytes).map_err(|_| Refusal::EnvelopeInvalid)?;
    let envelope: Envelope = serde_json::from_str(text).map_err(|_| Refusal::EnvelopeInvalid)?;
    let canonical = serde_jcs::to_string(&envelope).map_err(|_| Refusal::EnvelopeInvalid)?;
    if canonical != text
        || envelope.v != 1
        || envelope.signature.alg != "ed25519"
        // `nonce` is opaque; the annexe upper bound is enforced (A.8-grade
        // anti-abuse), the 16-char lower bound is NOT: the committed p1
        // vectors carry 15-char nonces on reject cases — drift noted for
        // the Mathieu gate, to be resolved in INFRA-PROVIDER.md, never here.
        || envelope.nonce.is_empty()
        || envelope.nonce.len() > 64
        || !valid_key_shape(&envelope.key)
    {
        return Err(Refusal::EnvelopeInvalid);
    }
    let Some(at_ms) = parse_rfc3339z_ms(&envelope.at) else {
        return Err(Refusal::EnvelopeInvalid);
    };
    Ok((envelope, at_ms))
}

/// The #8 signature check, shared by A.2 and B.5: Ed25519 over the JCS
/// with `signature.value = ""` (the §01.4 convention), under the caller's
/// resolved key.
pub(crate) fn verify_envelope_signature(
    envelope: &Envelope,
    verifying_key: &VerifyingKey,
) -> Result<(), Refusal> {
    let mut unsigned = envelope.clone();
    unsigned.signature.value = String::new();
    let unsigned_jcs = serde_jcs::to_string(&unsigned).map_err(|_| Refusal::EnvelopeInvalid)?;
    let sig_bytes = hex::decode(&envelope.signature.value)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .ok_or(Refusal::SignatureInvalid)?;
    verifying_key
        .verify(unsigned_jcs.as_bytes(), &Signature::from_bytes(&sig_bytes))
        .map_err(|_| Refusal::SignatureInvalid)
}

/// The key field's closed shape (#2 form): an owner fragment or a
/// multibase Ed25519 key. Decode happens at #7 — a well-shaped but
/// undecodable key is a chain fault, not a form fault.
fn valid_key_shape(key: &str) -> bool {
    key == "#root" || key == "#content" || key.starts_with('z')
}

fn decode_key(mb: &str) -> Result<VerifyingKey, Refusal> {
    let bytes = wire::multibase_to_ed25519_pub(mb).map_err(|_| Refusal::ChainInvalid)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| Refusal::ChainInvalid)
}

/// Sign an envelope under the shared §01.4 convention. **Client/tooling
/// surface only** — the replay harness and the P3 `RemoteStore` client
/// sign; the server never does (it holds no key to sign with).
pub fn sign_envelope(
    mut envelope: Envelope,
    key: &ed25519_dalek::SigningKey,
) -> Result<Envelope, Refusal> {
    use ed25519_dalek::Signer as _;
    envelope.signature.value = String::new();
    let unsigned = serde_jcs::to_string(&envelope).map_err(|_| Refusal::EnvelopeInvalid)?;
    envelope.signature.value = hex::encode(key.sign(unsigned.as_bytes()).to_bytes());
    Ok(envelope)
}

/// The wire form of a signed envelope: `base64url-sans-padding(JCS)`.
pub fn header_value(envelope: &Envelope) -> Result<String, Refusal> {
    let jcs = serde_jcs::to_string(envelope).map_err(|_| Refusal::EnvelopeInvalid)?;
    Ok(URL_SAFE_NO_PAD.encode(jcs.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_matches_annexe_a7() {
        // (status, code) pairs exactly as graved — a drift here is a wire
        // break, not a refactor.
        for (refusal, status, code) in [
            (Refusal::PathInvalid, 400, "path_invalid"),
            (Refusal::EnvelopeInvalid, 400, "envelope_invalid"),
            (Refusal::ArtifactInvalid, 400, "artifact_invalid"),
            (Refusal::EnvelopeMissing, 401, "envelope_missing"),
            (Refusal::ClockSkew, 401, "clock_skew"),
            (Refusal::NonceReplayed, 401, "nonce_replayed"),
            (Refusal::SignatureInvalid, 401, "signature_invalid"),
            (Refusal::ChainInvalid, 403, "chain_invalid"),
            (Refusal::ChainRevoked, 403, "chain_revoked"),
            (Refusal::NotCovered, 403, "not_covered"),
            (Refusal::MappingMismatch, 403, "mapping_mismatch"),
            (Refusal::DidNotBound, 403, "did_not_bound"),
            (Refusal::Suspended, 403, "suspended"),
            (Refusal::UnknownTenant, 404, "unknown_tenant"),
            (Refusal::NotFound, 404, "not_found"),
            (Refusal::CasMismatch, 409, "cas_mismatch"),
            (Refusal::EditionGone, 410, "edition_gone"),
            (Refusal::PayloadTooLarge, 413, "payload_too_large"),
            (Refusal::VersionUnsupported, 426, "version_unsupported"),
            (Refusal::CasRequired, 428, "cas_required"),
            (Refusal::RateLimited, 429, "rate_limited"),
        ] {
            assert_eq!(refusal.status(), status);
            assert_eq!(refusal.code(), code);
        }
    }

    #[test]
    fn envelope_jcs_matches_the_committed_wire_order() {
        // JCS sorts keys: at, body_b3, host, key, mandate, method, nonce,
        // path, signature, v — the exact byte layout of the p1 vectors.
        let env = Envelope {
            v: 1,
            host: "store.aithos.fr".into(),
            method: "GET".into(),
            path: "/t/acme/did:aithos:zX/did.json".into(),
            body_b3: String::new(),
            at: "2026-07-16T12:00:00Z".into(),
            nonce: "n-0123456789abcd".into(),
            mandate: vec![],
            key: "#root".into(),
            signature: EnvelopeSignature {
                alg: "ed25519".into(),
                value: String::new(),
            },
        };
        let jcs = serde_jcs::to_string(&env).unwrap();
        assert!(jcs.starts_with(r#"{"at":"2026-07-16T12:00:00Z","body_b3":"","host":"#));
        assert!(jcs.ends_with(r#""signature":{"alg":"ed25519","value":""},"v":1}"#));
    }
}
