//! The HTTP surface of `aithos-store-api` — consumed by `bin/store_api`
//! and driven in-process by the BDD suite.
//!
//! One fallback handler owns the whole data plane: the routing IS the
//! grammar of annexe A.1, so no axum route pattern can drift from the
//! contract. Decision order per request, fail-closed at the first refusal:
//!
//! 1. wire version negotiation (A.1 — `426 version_unsupported`);
//! 2. body size gate (A.8 — 32 MiB default);
//! 3. check #0 — target grammar ([`crate::pathmap`], `path_invalid`);
//! 4. check #1 — tenant state (`unknown_tenant` / `suspended`);
//! 5. checks #2–#10 — the envelope ([`crate::envelope::verify`]);
//! 6. dispatch — P1 serves GET objects, PUTs light-form artifacts
//!    (A.4 bullet 5), answers `501 not_implemented` on the grammar-valid
//!    routes the skeleton does not carry yet, and `404 not_found` only
//!    inside a covered perimeter.
//!
//! Every response carries `X-Aithos-Store: 1.0.0-draft.1`; error bodies
//! are exactly `{"error": <code>, "at": <now serveur>}` (A.7) — never a
//! path, a body excerpt or an envelope. One log line per data request,
//! through [`crate::redact`] only.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, Request, Response, StatusCode};
use axum::routing::get;
use axum::Router;

use crate::acme::{self, AcmeState};
use crate::control::{ControlPlane, TenantState};
use crate::dns::DnsTxt;
use crate::envelope::{self, Principal, Refusal, RequestFacts};
use crate::nonces::NonceStore;
use crate::objects::ObjectStore;
use crate::pathmap::{self, DataTarget, ObjectPath, TargetKind};
use crate::redact::{RequestLine, RouteClass};
use crate::time::{parse_rfc3339z_ms, render_rfc3339z};
use crate::STORE_WIRE_VERSION;

/// Anti-abuse default of annexe A.8: object ≤ 32 MiB.
pub const MAX_OBJECT_BYTES: usize = 32 * 1024 * 1024;

/// Everything the handlers need. All trust decisions flow through the
/// injected seams — the surface itself holds no policy and no secret.
pub struct AppState {
    pub control: ControlPlane,
    pub objects: Arc<dyn ObjectStore>,
    pub nonces: Arc<dyn NonceStore>,
    /// The DNS TXT seam of the B.5 surface (Route 53 in the task, memory
    /// in tests, disabled by default — every effect then refuses 503).
    pub dns: Arc<dyn DnsTxt>,
    /// Mutable B.5 state: the PUT budget and the posed-record ledger the
    /// 10-minute purge sweeps.
    pub acme: AcmeState,
    /// The authority this deployment serves (`store.aithos.fr`,
    /// `store.dev.aithos.fr`, …) — pinned against the envelope's `host`,
    /// which is what kills cross-plane replay (A.2, note on `host`).
    pub authority: String,
    /// Test-clock override (`X-Aithos-Test-Now`), enabled ONLY by the
    /// explicit `AITHOS_STORE_TEST_NOW=1` startup opt-in. Never set in any
    /// deployment manifest; exists for the byte-exact vector replay, whose
    /// instants are frozen inputs (`server_now`), never wall-clock.
    pub test_now_enabled: bool,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .fallback(handle)
        .layer(axum::middleware::map_response(stamp_wire_version))
        .with_state(state)
}

async fn stamp_wire_version(mut response: axum::response::Response) -> axum::response::Response {
    response.headers_mut().insert(
        "x-aithos-store",
        HeaderValue::from_static(STORE_WIRE_VERSION),
    );
    response
}

/// What one request resolved to — the response plus the redacted log
/// facts (every field is from the closed A.8 register; the acme surface
/// has no DID and never logs a hostname).
struct Outcome {
    tenant: Option<String>,
    did: Option<String>,
    class: Option<RouteClass>,
    response: Response<Body>,
    req_bytes: usize,
    resp_bytes: usize,
}

impl Outcome {
    fn of_target(
        target: Option<DataTarget>,
        verb: &str,
        response: Response<Body>,
        req_bytes: usize,
        resp_bytes: usize,
    ) -> Self {
        Outcome {
            class: target.as_ref().and_then(|t| RouteClass::of(t, verb)),
            tenant: target.as_ref().map(|t| t.tenant.clone()),
            did: target.as_ref().map(|t| t.did.clone()),
            response,
            req_bytes,
            resp_bytes,
        }
    }
}

/// One data-plane request, one decision, one redacted log line.
async fn handle(State(state): State<Arc<AppState>>, request: Request<Body>) -> Response<Body> {
    let started = Instant::now();
    let (parts, body) = request.into_parts();

    // The instant of record: the system clock, or the explicit test
    // override. A malformed test instant refuses loudly (harness bug).
    let wall_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let now_ms = match test_now(&state, &parts.headers) {
        Ok(Some(ms)) => ms,
        Ok(None) => wall_ms,
        Err(refusal) => {
            let (response, resp_bytes) = refuse(refusal, wall_ms);
            return finish(
                started,
                wall_ms,
                parts.method.as_str(),
                Outcome::of_target(None, parts.method.as_str(), response, 0, resp_bytes),
            );
        }
    };

    let outcome = decide(&state, &parts, body, now_ms).await;
    finish(started, now_ms, parts.method.as_str(), outcome)
}

async fn decide(
    state: &AppState,
    parts: &axum::http::request::Parts,
    body: Body,
    now_ms: i64,
) -> Outcome {
    let verb = parts.method.as_str();
    let refused = |target: Option<DataTarget>, refusal: Refusal, req_bytes: usize| {
        let (response, resp_bytes) = refuse(refusal, now_ms);
        Outcome::of_target(target, verb, response, req_bytes, resp_bytes)
    };

    // Wire version negotiation (A.1): an unknown MAJOR refuses the dialect.
    if let Some(value) = parts.headers.get("x-aithos-store") {
        let major = value
            .to_str()
            .ok()
            .and_then(|v| v.split('.').next())
            .and_then(|m| m.parse::<u64>().ok());
        if major != Some(1) {
            return refused(None, Refusal::VersionUnsupported, 0);
        }
    }

    // Body, bounded (A.8) before anything hashes it.
    let Ok(body) = axum::body::to_bytes(body, MAX_OBJECT_BYTES).await else {
        return refused(None, Refusal::PayloadTooLarge, 0);
    };
    let req_bytes = body.len();

    // #0 — the grammar gates everything (byte-exact target, query included).
    let target_raw = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str().to_owned())
        .unwrap_or_else(|| parts.uri.path().to_owned());

    // The B.5 surface: exactly `/acme/txt`, no query — outside the /t/…
    // data grammar, its own closed order (module `acme`). Anything else
    // under a different spelling falls through to #0 and refuses
    // path_invalid.
    if target_raw == acme::ACME_PATH {
        return decide_acme_route(state, parts, &body, &target_raw, now_ms).await;
    }

    let Some(target) = pathmap::parse_target(&target_raw) else {
        return refused(None, Refusal::PathInvalid, req_bytes);
    };

    // #1 — the tenant routes (the DID binding is only ever named at #7,
    // under a valid envelope — anti-enumeration note, A.7).
    match state.control.tenant_state(&target.tenant) {
        TenantState::Unknown => return refused(Some(target), Refusal::UnknownTenant, req_bytes),
        TenantState::Suspended => return refused(Some(target), Refusal::Suspended, req_bytes),
        TenantState::Active => {}
    }

    // #2–#10 — the envelope, in normative order.
    let header = match parts.headers.get("x-aithos-auth") {
        Some(value) => match value.to_str() {
            Ok(v) => Some(v),
            Err(_) => return refused(Some(target), Refusal::EnvelopeInvalid, req_bytes),
        },
        None => None,
    };
    let authority = received_authority(parts);
    let facts = RequestFacts {
        authority: &authority,
        expected_authority: &state.authority,
        method: parts.method.as_str(),
        target: &target_raw,
        body: &body,
    };
    let principal = match envelope::verify(
        header,
        &facts,
        &target,
        &state.control,
        state.objects.as_ref(),
        state.nonces.as_ref(),
        now_ms,
    )
    .await
    {
        Ok(principal) => principal,
        Err(refusal) => return refused(Some(target), refusal, req_bytes),
    };

    // Dispatch.
    let (response, resp_bytes) = match (&target.kind, parts.method.as_str()) {
        (TargetKind::Object(object), "GET") => serve_object(state, &target, object, now_ms).await,
        (TargetKind::Object(object), "PUT") => {
            // Defensive fail-closed: only the owner writes in P1. The
            // anonymous principal cannot reach here (a PUT without an
            // envelope died at #2), but the refusal stays explicit.
            if principal != Principal::Owner {
                refuse(Refusal::NotCovered, now_ms)
            } else {
                store_object(state, &target, object, body.to_vec(), now_ms).await
            }
        }
        // A verb the A.3 table does not define on this target: the
        // path-map's default deny.
        (TargetKind::Object(_), _) => refuse(Refusal::NotCovered, now_ms),
        // Grammar-valid routes the P1 skeleton does not carry: heads,
        // batch, gamma append, sync, list — wired for real in P2.
        _ => refuse(Refusal::NotImplemented, now_ms),
    };
    Outcome::of_target(Some(target), verb, response, req_bytes, resp_bytes)
}

/// The `/acme/txt` route (annexe B.5): verification + effect live in
/// [`crate::acme::decide_acme`]; this wrapper only extracts the wire
/// facts and builds the redacted outcome. The log line carries class
/// `acme` and the VERIFIED tenant — never the hostname, never the value
/// (the A.8 register is closed; there is no DID on this surface).
async fn decide_acme_route(
    state: &AppState,
    parts: &axum::http::request::Parts,
    body: &[u8],
    target_raw: &str,
    now_ms: i64,
) -> Outcome {
    let acme_outcome =
        |tenant: Option<String>, response: Response<Body>, resp_bytes: usize| Outcome {
            tenant,
            did: None,
            class: Some(RouteClass::Acme),
            response,
            req_bytes: body.len(),
            resp_bytes,
        };

    let header = match parts.headers.get("x-aithos-auth") {
        Some(value) => match value.to_str() {
            Ok(v) => Some(v),
            Err(_) => {
                let (response, resp_bytes) = refuse(Refusal::EnvelopeInvalid, now_ms);
                return acme_outcome(None, response, resp_bytes);
            }
        },
        None => None,
    };
    let authority = received_authority(parts);
    let facts = RequestFacts {
        authority: &authority,
        expected_authority: &state.authority,
        method: parts.method.as_str(),
        target: target_raw,
        body,
    };
    match acme::decide_acme(
        header,
        &facts,
        &state.control,
        state.nonces.as_ref(),
        &state.acme,
        state.dns.as_ref(),
        now_ms,
    )
    .await
    {
        Ok(accepted) => {
            let response = Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .expect("static response");
            acme_outcome(Some(accepted.tenant), response, 0)
        }
        Err(refusal) => {
            let (response, resp_bytes) = refuse(refusal, now_ms);
            acme_outcome(None, response, resp_bytes)
        }
    }
}

async fn serve_object(
    state: &AppState,
    target: &DataTarget,
    object: &ObjectPath,
    now_ms: i64,
) -> (Response<Body>, usize) {
    match state
        .objects
        .get(&target.tenant, &target.did, &object.key())
        .await
    {
        Some(bytes) => {
            let len = bytes.len();
            let response = Response::builder()
                .status(StatusCode::OK)
                // Caching per annexe A.6 arrives with the real backend
                // (P2); the skeleton stays conservative.
                .header(header::CACHE_CONTROL, "no-store")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(bytes))
                .expect("static response");
            (response, len)
        }
        None => refuse(Refusal::NotFound, now_ms),
    }
}

/// P1 writes: the A.4 "light form check" classes only. The classes A.4
/// verifies in depth (manifest, did.json, certs, gamma) answer
/// `501 not_implemented` until P2 — the server NEVER stores what it
/// cannot verify, and never repairs anything.
async fn store_object(
    state: &AppState,
    target: &DataTarget,
    object: &ObjectPath,
    bytes: Vec<u8>,
    now_ms: i64,
) -> (Response<Body>, usize) {
    let json_where_json = |segs: &[String]| -> bool {
        segs.last().is_none_or(|name| {
            !name.ends_with(".json") || serde_json::from_slice::<serde_json::Value>(&bytes).is_ok()
        })
    };
    let acceptable = match object {
        ObjectPath::Manifest
        | ObjectPath::DidJson
        | ObjectPath::Cert(_)
        | ObjectPath::GammaSegment(_) => return refuse(Refusal::NotImplemented, now_ms),
        // Opaque ciphertext: no content check by design (§3.1).
        ObjectPath::Blob(_, _) => true,
        // "JSON parsable là où c'est du JSON" (A.4 bullet 5).
        ObjectPath::ZoneIndex(_) | ObjectPath::Hdr(_, _) => {
            serde_json::from_slice::<serde_json::Value>(&bytes).is_ok()
        }
        ObjectPath::Public(segs) | ObjectPath::X(_, segs) => json_where_json(segs),
    };
    if !acceptable {
        return refuse(Refusal::ArtifactInvalid, now_ms);
    }
    state
        .objects
        .put(&target.tenant, &target.did, &object.key(), bytes)
        .await;
    let response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .expect("static response");
    (response, 0)
}

/// The refusal's registry code, carried on the response so the log line
/// names exactly what was answered (codes are `&'static str` from the
/// closed registry — the log can never carry free text).
#[derive(Clone, Copy)]
struct ErrorCode(&'static str);

/// `{"error": <code>, "at": <now>}` — the A.7 error body, nothing else.
fn refuse(refusal: Refusal, now_ms: i64) -> (Response<Body>, usize) {
    let body = serde_json::json!({
        "error": refusal.code(),
        "at": render_rfc3339z(now_ms),
    })
    .to_string();
    let len = body.len();
    let mut response = Response::builder()
        .status(StatusCode::from_u16(refusal.status()).expect("registry status"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("static response");
    response.extensions_mut().insert(ErrorCode(refusal.code()));
    (response, len)
}

/// The received authority, normalized as the annexe writes envelopes:
/// lowercase, without a default port.
fn received_authority(parts: &axum::http::request::Parts) -> String {
    let raw = parts
        .headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| parts.uri.authority().map(|a| a.as_str().to_owned()))
        .unwrap_or_default();
    let lower = raw.trim().to_ascii_lowercase();
    lower
        .strip_suffix(":443")
        .or_else(|| lower.strip_suffix(":80"))
        .unwrap_or(&lower)
        .to_owned()
}

fn test_now(state: &AppState, headers: &axum::http::HeaderMap) -> Result<Option<i64>, Refusal> {
    let Some(value) = headers.get("x-aithos-test-now") else {
        return Ok(None);
    };
    if !state.test_now_enabled {
        // Dead surface unless the binary opted in at startup: the header
        // is ignored, the wall clock rules.
        return Ok(None);
    }
    value
        .to_str()
        .ok()
        .and_then(parse_rfc3339z_ms)
        .map(Some)
        .ok_or(Refusal::Unavailable)
}

fn finish(started: Instant, now_ms: i64, verb: &str, outcome: Outcome) -> Response<Body> {
    RequestLine {
        at_ms: now_ms,
        tenant: outcome.tenant.as_deref(),
        did: outcome.did.as_deref(),
        class: outcome.class,
        verb,
        status: outcome.response.status().as_u16(),
        error: outcome
            .response
            .extensions()
            .get::<ErrorCode>()
            .map(|code| code.0),
        req_bytes: outcome.req_bytes,
        resp_bytes: outcome.resp_bytes,
        duration_ms: started.elapsed().as_millis(),
    }
    .emit();
    outcome.response
}
