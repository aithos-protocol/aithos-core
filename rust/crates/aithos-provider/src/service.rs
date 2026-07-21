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

use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, Method, Request, Response, StatusCode};
use axum::middleware::Next;
use axum::routing::get;
use axum::Router;

use crate::acme::{self, AcmeState};
use crate::artifacts::{self, DepositRefusal};
use crate::control::{ControlStore, TenantState};
use crate::dns::DnsTxt;
use crate::envelope::{self, Principal, Refusal, RequestFacts};
use crate::heads::HeadsTable;
use crate::nonces::NonceStore;
use crate::objects::ObjectStore;
use crate::pathmap::{self, DataTarget, ObjectPath, TargetKind};
use crate::redact::{RequestLine, RouteClass};
use crate::time::{parse_rfc3339z_ms, render_rfc3339z};
use crate::STORE_WIRE_VERSION;

/// Anti-abuse default of annexe A.8: object ≤ 32 MiB.
pub const MAX_OBJECT_BYTES: usize = 32 * 1024 * 1024;

/// Per-`(tenant, did)` deposit serialization for the in-process backends:
/// the CAS of the heads table stays the contract's serialization point
/// (A.5); this lock only prevents one process from interleaving the
/// segment read-append-write around it. The DynamoDB/S3 backends (étape
/// 6) rely on the conditional write alone, behind the same seams.
type LockMap = std::collections::HashMap<(String, String), Arc<tokio::sync::Mutex<()>>>;

#[derive(Default)]
pub struct DepositLocks(Mutex<LockMap>);

impl DepositLocks {
    fn of(&self, tenant: &str, did: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.0
            .lock()
            .expect("deposit locks poisoned")
            .entry((tenant.to_owned(), did.to_owned()))
            .or_default()
            .clone()
    }
}

/// Everything the handlers need. All trust decisions flow through the
/// injected seams — the surface itself holds no policy and no secret.
pub struct AppState {
    /// The tenant read-model behind the P7 seam — the bootstrap plane in
    /// dev/tests, `CachedControl<DynamoDbControl>` in the deployed task.
    pub control: Arc<dyn ControlStore>,
    pub objects: Arc<dyn ObjectStore>,
    /// The A.5 heads table — the CAS seam (memory now, DynamoDB étape 6).
    pub heads: Arc<dyn HeadsTable>,
    /// In-process deposit serialization (see [`DepositLocks`]).
    pub deposit_locks: DepositLocks,
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
        .layer(axum::middleware::from_fn(public_read_cors))
        .with_state(state)
}

/// Public Ethos carriers are intentionally readable from any browser origin.
/// The middleware is narrowly derived from the same closed path grammar as
/// authorization; authenticated and non-public responses receive no CORS
/// relaxation.
async fn public_read_cors(request: Request<Body>, next: Next) -> Response<Body> {
    let expose = request.method() == Method::GET
        && request.headers().get("x-aithos-auth").is_none()
        && request
            .uri()
            .path_and_query()
            .and_then(|target| pathmap::parse_target(target.as_str()))
            .is_some_and(|target| {
                matches!(
                    target.kind,
                    TargetKind::Object(ref object) if pathmap::anonymous_covers(object)
                )
            });
    let mut response = next.run(request).await;
    if expose {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("etag, x-aithos-store"),
        );
    }
    response
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
    match state.control.tenant_state(&target.tenant, now_ms).await {
        Ok(TenantState::Unknown) => {
            return refused(Some(target), Refusal::UnknownTenant, req_bytes)
        }
        Ok(TenantState::Suspended) => return refused(Some(target), Refusal::Suspended, req_bytes),
        Ok(TenantState::Active) => {}
        // A mute control plane refuses — it NEVER invents an
        // unknown_tenant (P7 gate contrat, étape-6 seam pattern).
        Err(_) => return refused(Some(target), Refusal::Unavailable, req_bytes),
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
        state.heads.as_ref(),
        state.nonces.as_ref(),
        now_ms,
    )
    .await
    {
        Ok(principal) => principal,
        Err(refusal) => return refused(Some(target), refusal, req_bytes),
    };

    // The A.5 CAS header, raw. Its closed grammar (`none` |
    // `sha256:<64hex>`) is the deposit's concern, not the router's.
    let if_head = parts
        .headers
        .get("if-head")
        .and_then(|value| value.to_str().ok());

    // Dispatch.
    let (response, resp_bytes) = match (&target.kind, parts.method.as_str()) {
        (TargetKind::Object(object), "GET") => {
            // P3 — conditional revalidation on the A.6 revalidate classes:
            // the client replays the strong ETag it holds.
            let if_none_match = parts
                .headers
                .get(header::IF_NONE_MATCH)
                .and_then(|value| value.to_str().ok());
            serve_object(state, &target, object, now_ms, if_none_match).await
        }
        // Publish (A.4/A.5): CAS mandatory, deposit verified by
        // composition — under the in-process deposit lock.
        (TargetKind::Object(ObjectPath::Manifest), "PUT") => match &principal {
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
            Principal::Owner | Principal::Mandated(_) => {
                let chain = match &principal {
                    Principal::Mandated(chain) => Some(chain.as_slice()),
                    _ => None,
                };
                let lock = state.deposit_locks.of(&target.tenant, &target.did);
                let _guard = lock.lock().await;
                match artifacts::deposit_manifest(
                    state.objects.as_ref(),
                    state.heads.as_ref(),
                    &target.tenant,
                    &target.did,
                    chain,
                    if_head,
                    &body,
                )
                .await
                {
                    Ok(accepted) => accepted_json(serde_json::json!({
                        "head": accepted.head,
                        "height": accepted.height,
                    })),
                    Err(refusal) => refuse_deposit(refusal, now_ms),
                }
            }
        },
        // Cert deposit (A.4): id == filename, subject == did, chain
        // resolved from the stored certs, verified at now_serveur.
        (TargetKind::Object(ObjectPath::Cert(id)), "PUT") => match &principal {
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
            Principal::Owner | Principal::Mandated(_) => {
                match artifacts::deposit_cert(
                    state.objects.as_ref(),
                    &target.tenant,
                    &target.did,
                    id,
                    &render_rfc3339z(now_ms),
                    &body,
                )
                .await
                {
                    Ok(()) => no_content(),
                    Err(refusal) => refuse_deposit(refusal, now_ms),
                }
            }
        },
        // did.json deposit (A.4, étape 5): genesis under the deposited
        // root key (the #7 exception already resolved it), replacement
        // under the stored succession key.
        (TargetKind::Object(ObjectPath::DidJson), "PUT") => match &principal {
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
            Principal::Owner | Principal::Mandated(_) => {
                let lock = state.deposit_locks.of(&target.tenant, &target.did);
                let _guard = lock.lock().await;
                match artifacts::deposit_did(
                    state.objects.as_ref(),
                    &target.tenant,
                    &target.did,
                    &body,
                )
                .await
                {
                    Ok(()) => no_content(),
                    Err(refusal) => refuse_deposit(refusal, now_ms),
                }
            }
        },
        // Segment replica (A.4/A.5, mode A, étape 5): byte-exact prefix,
        // per-entry verification, segment-head CAS — under the lock.
        (TargetKind::Object(ObjectPath::GammaSegment(month)), "PUT") => match &principal {
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
            Principal::Owner | Principal::Mandated(_) => {
                let lock = state.deposit_locks.of(&target.tenant, &target.did);
                let _guard = lock.lock().await;
                match artifacts::deposit_replica(
                    state.objects.as_ref(),
                    state.heads.as_ref(),
                    &target.tenant,
                    &target.did,
                    month,
                    if_head,
                    &body,
                )
                .await
                {
                    Ok(accepted) => accepted_json(serde_json::json!({
                        "head": accepted.head,
                    })),
                    Err(refusal) => refuse_deposit(refusal, now_ms),
                }
            }
        },
        // K1-C sidecars (redline gate 5): light form + content addressing.
        (TargetKind::Object(ObjectPath::Changeset(hash)), "PUT")
        | (TargetKind::Object(ObjectPath::Evidence(hash)), "PUT") => match &principal {
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
            Principal::Owner | Principal::Mandated(_) => {
                let kind = if matches!(target.kind, TargetKind::Object(ObjectPath::Changeset(_))) {
                    artifacts::SidecarKind::Changeset
                } else {
                    artifacts::SidecarKind::Evidence
                };
                match artifacts::deposit_sidecar(
                    state.objects.as_ref(),
                    &target.tenant,
                    &target.did,
                    kind,
                    hash,
                    &body,
                )
                .await
                {
                    Ok(()) => no_content(),
                    Err(refusal) => refuse_deposit(refusal, now_ms),
                }
            }
        },
        // The edition slot has NO write line, owner included (redline
        // gate 5): in the grammar (not path_invalid), never client-written.
        (TargetKind::Object(ObjectPath::ManifestSlot(_)), "PUT") => {
            refuse(Refusal::NotCovered, now_ms)
        }
        (TargetKind::Object(object), "PUT") => match principal {
            // Owner and covered mandated writes share the same A.4
            // light-form deposit; #10 already gated the mandated rows
            // (pass L).
            Principal::Owner | Principal::Mandated(_) => {
                store_object(state, &target, object, body.to_vec(), now_ms).await
            }
            // Anonymous cannot reach here (a PUT without an envelope died
            // at #2); the refusal stays explicit.
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
        },
        // Gamma append (A.4/A.5): one entry, CAS mandatory, entry
        // verification delegated to core — under the deposit lock.
        (TargetKind::Gamma, "POST") => match &principal {
            Principal::Anonymous => refuse(Refusal::NotCovered, now_ms),
            Principal::Owner | Principal::Mandated(_) => {
                let lock = state.deposit_locks.of(&target.tenant, &target.did);
                let _guard = lock.lock().await;
                match artifacts::deposit_gamma(
                    state.objects.as_ref(),
                    state.heads.as_ref(),
                    &target.tenant,
                    &target.did,
                    if_head,
                    &body,
                )
                .await
                {
                    Ok(accepted) => accepted_json(serde_json::json!({
                        "head": accepted.head,
                    })),
                    Err(refusal) => refuse_deposit(refusal, now_ms),
                }
            }
        },
        // The read surface (A.3, étape 5).
        (TargetKind::Heads, "GET") => serve_heads(state, &target, now_ms).await,
        (TargetKind::List, "GET") => {
            serve_list(state, &target, &target_raw, &principal, now_ms).await
        }
        (TargetKind::Batch, "POST") => serve_batch(state, &target, &principal, &body, now_ms).await,
        (TargetKind::Sync, "POST") => serve_sync(state, &target, &principal, &body, now_ms).await,
        // A verb the A.3 table does not define on this target: the
        // path-map's default deny.
        _ => refuse(Refusal::NotCovered, now_ms),
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
    if_none_match: Option<&str>,
) -> (Response<Body>, usize) {
    match state
        .objects
        .get(&target.tenant, &target.did, &object.key())
        .await
    {
        // An unanswerable backend refuses — never an invented absence.
        Err(_) => refuse(Refusal::Unavailable, now_ms),
        Ok(Some(bytes)) => {
            let len = bytes.len();
            let etag = strong_etag(object, &bytes);
            // P3 (A.6): a replayed strong ETag on a revalidate-class
            // path answers 304 — same class, same ETag, no body. Only
            // an EXACT strong match (or `*`) revalidates; the immutable
            // and no-store classes never carry an ETag, so they never
            // enter this arm.
            if let (Some(etag), Some(inm)) = (&etag, if_none_match) {
                let matches = inm
                    .split(',')
                    .map(str::trim)
                    .any(|candidate| candidate == "*" || candidate == etag);
                if matches {
                    let response = Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header(header::CACHE_CONTROL, cache_class(object, now_ms))
                        .header(header::ETAG, etag.clone())
                        .body(Body::empty())
                        .expect("static response");
                    return (response, 0);
                }
            }
            let mut response = Response::builder()
                .status(StatusCode::OK)
                // The A.6 cache class is the PATH's, computed at the
                // serving instant — never the backend's decision.
                .header(header::CACHE_CONTROL, cache_class(object, now_ms))
                .header(header::CONTENT_TYPE, "application/octet-stream");
            if let Some(etag) = etag {
                response = response.header(header::ETAG, etag);
            }
            (
                response.body(Body::from(bytes)).expect("static response"),
                len,
            )
        }
        Ok(None) => refuse(Refusal::NotFound, now_ms),
    }
}

/// The A.6 cache classes, verbatim. `did.json`, `e/public/**` and
/// `x/<id>/**` are the A.6 COMPLETION (carried to the étape-6 gate,
/// never graved silently): anonymous-readable/CloudFront-fronted paths
/// take the public revalidate class, connector state the private one.
fn cache_class(object: &ObjectPath, now_ms: i64) -> &'static str {
    const IMMUTABLE: &str = "public, max-age=31536000, immutable";
    const NO_STORE: &str = "no-store";
    const PUBLIC_REVALIDATE: &str = "public, max-age=0, must-revalidate";
    const PRIVATE_REVALIDATE: &str = "private, max-age=0, must-revalidate";
    match object {
        // Addressed by id/height/content, never rewritten — the ⑧b
        // write-once makes the class opposable (A.6, redline gate 5).
        ObjectPath::Cert(_)
        | ObjectPath::ManifestSlot(_)
        | ObjectPath::Changeset(_)
        | ObjectPath::Evidence(_) => IMMUTABLE,
        // A month segment freezes once the serving instant leaves it
        // ("segments gamma des mois révolus", A.6).
        ObjectPath::GammaSegment(month) => {
            if month.as_str() < current_utc_month(now_ms).as_str() {
                IMMUTABLE
            } else {
                NO_STORE
            }
        }
        // The hot head and the mutable carriers advance with every
        // publication (A.6 + redline gate 5).
        ObjectPath::Manifest
        | ObjectPath::ZoneIndex(_)
        | ObjectPath::Hdr(_, _)
        | ObjectPath::IndicesPublic
        | ObjectPath::RootsPublic
        | ObjectPath::VaultCatalogPins => NO_STORE,
        // Stable name, re-editable content: strong-ETag revalidation.
        ObjectPath::PublicSectionAlias(_) | ObjectPath::DidJson | ObjectPath::Public(_) => {
            PUBLIC_REVALIDATE
        }
        ObjectPath::Blob(_, _)
        | ObjectPath::CircleBlobAlias(_)
        | ObjectPath::X(_, _)
        // Micro-redline A.1 (P3): the zone root header/blob are stable
        // names with re-editable content — the private revalidate class.
        // The connector carriers (redline extension, DEMO-LEA gate)
        // share it: stable name, re-editable sealed content.
        | ObjectPath::ZoneHeader(_)
        | ObjectPath::ZoneRoot(_)
        | ObjectPath::ConnectorHeader(_)
        | ObjectPath::ConnectorConfig(_) => PRIVATE_REVALIDATE,
    }
}

/// Strong ETag (quoted SHA-256 of the served bytes) on the revalidate
/// classes only — the immutable and no-store classes never revalidate.
fn strong_etag(object: &ObjectPath, bytes: &[u8]) -> Option<String> {
    use sha2::{Digest, Sha256};
    matches!(
        object,
        ObjectPath::PublicSectionAlias(_)
            | ObjectPath::DidJson
            | ObjectPath::Public(_)
            | ObjectPath::Blob(_, _)
            | ObjectPath::CircleBlobAlias(_)
            | ObjectPath::X(_, _)
            | ObjectPath::ZoneHeader(_)
            | ObjectPath::ZoneRoot(_)
            | ObjectPath::ConnectorHeader(_)
            | ObjectPath::ConnectorConfig(_)
    )
    .then(|| format!("\"{}\"", hex::encode(Sha256::digest(bytes))))
}

/// The UTC month (`YYYY-MM`) of the serving instant.
fn current_utc_month(now_ms: i64) -> String {
    crate::time::render_rfc3339z(now_ms)
        .get(..7)
        .unwrap_or_default()
        .to_owned()
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
        // Opaque ciphertext: no content check by design (§3.1). The
        // sealed connector config joins the class (redline extension).
        ObjectPath::Blob(_, _) | ObjectPath::ZoneRoot(_) | ObjectPath::ConnectorConfig(_) => true,
        // "JSON parsable là où c'est du JSON" (A.4 bullet 5).
        ObjectPath::ZoneIndex(_)
        | ObjectPath::Hdr(_, _)
        | ObjectPath::ZoneHeader(_)
        | ObjectPath::ConnectorHeader(_) => {
            serde_json::from_slice::<serde_json::Value>(&bytes).is_ok()
        }
        ObjectPath::Public(segs) | ObjectPath::X(_, segs) => json_where_json(segs),
        // K1-C aliases (redline gate 5): same light form as their `e/**`
        // equivalents — the `.md` section is opaque, the JSON carriers
        // must parse.
        ObjectPath::PublicSectionAlias(_) => true,
        ObjectPath::CircleBlobAlias(_)
        | ObjectPath::IndicesPublic
        | ObjectPath::RootsPublic
        | ObjectPath::VaultCatalogPins => {
            serde_json::from_slice::<serde_json::Value>(&bytes).is_ok()
        }
        // Dispatched before this function (their own deposits / the
        // no-write-line slot); kept refusing here so nothing can drift
        // into an unverified store.
        ObjectPath::ManifestSlot(_) | ObjectPath::Changeset(_) | ObjectPath::Evidence(_) => {
            return refuse(Refusal::NotCovered, now_ms)
        }
    };
    if !acceptable {
        return refuse(Refusal::ArtifactInvalid, now_ms);
    }
    if state
        .objects
        .put(&target.tenant, &target.did, &object.key(), bytes)
        .await
        .is_err()
    {
        return refuse(Refusal::Unavailable, now_ms);
    }
    let response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .expect("static response");
    (response, 0)
}

// ------------------------------------------------- the read surface (A.3)

/// A.8 bounds of the collection routes.
const MAX_BATCH_PATHS: usize = 256;
const MAX_LIST_PAGE: u64 = 1000;
const MAX_PACK_BYTES: usize = 32 * 1024 * 1024;

/// Can this principal READ this object? The same `covers()` rows as a
/// direct GET — the coarse filter of the collection routes (a shorter
/// answer, never a different authority; §3.1).
fn principal_reads(principal: &Principal, object: &ObjectPath) -> bool {
    match principal {
        Principal::Owner => true,
        Principal::Anonymous => pathmap::anonymous_covers(object),
        Principal::Mandated(chain) => {
            let leaf = chain.last().expect("a mandated chain is non-empty");
            // Draft.3 (K1-C) leaf parses no typed perimeter: only the
            // any-chain rows serve (the same fallback as the envelope's
            // #10) — an empty perimeter by default.
            let perimeter = leaf.parsed_perimeter().unwrap_or_default();
            pathmap::mandated_covers(&perimeter, &TargetKind::Object(object.clone()), "GET")
        }
    }
}

/// GET `/heads` — the two hot heads, exactly the values the accepts
/// served (A.5): `{"height", "manifest": "sha256:…"|null, "gamma":
/// "sha256:…"|null, "segment": "YYYY-MM"|null}`.
async fn serve_heads(
    state: &AppState,
    target: &DataTarget,
    now_ms: i64,
) -> (Response<Body>, usize) {
    let record = match state.heads.read(&target.tenant, &target.did).await {
        Err(_) => return refuse(Refusal::Unavailable, now_ms),
        Ok(record) => record.unwrap_or_default(),
    };
    let null_if_empty = |value: String| {
        if value.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(value)
        }
    };
    accepted_json(serde_json::json!({
        "height": record.height,
        "manifest": null_if_empty(if record.manifest_chain_hash.is_empty() {
            String::new()
        } else {
            format!("sha256:{}", record.manifest_chain_hash)
        }),
        "gamma": null_if_empty(record.gamma_head),
        "segment": null_if_empty(record.gamma_segment),
    }))
}

/// GET `?list=<prefix>[&after=<chemin>][&limit=<n>]` — the stored paths
/// under the prefix, FILTERED to the covered perimeter (coarse: the same
/// `covers()` rows as a direct GET — a shorter 200, never an error),
/// lexicographic, paginated. `limit` above the A.8 page bound refuses
/// `413`, never a silent clamp.
async fn serve_list(
    state: &AppState,
    target: &DataTarget,
    target_raw: &str,
    principal: &Principal,
    now_ms: i64,
) -> (Response<Body>, usize) {
    let Some(query) = target_raw
        .split_once('?')
        .and_then(|(_, q)| pathmap::parse_list_query(q))
    else {
        return refuse(Refusal::PathInvalid, now_ms);
    };
    let limit = query.limit.unwrap_or(MAX_LIST_PAGE);
    if limit == 0 || limit > MAX_LIST_PAGE {
        return refuse(Refusal::PayloadTooLarge, now_ms);
    }
    let Ok(all) = state.objects.list(&target.tenant, &target.did).await else {
        return refuse(Refusal::Unavailable, now_ms);
    };
    let visible: Vec<String> = all
        .into_iter()
        .filter(|chemin| chemin.starts_with(&query.prefix))
        .filter(|chemin| {
            query
                .after
                .as_deref()
                .is_none_or(|after| chemin.as_str() > after)
        })
        .filter(|chemin| {
            // Only grammar-parseable, covered paths are ever named — an
            // out-of-grammar stored key (impossible by construction) or
            // an uncovered one is silently absent, never an error.
            pathmap::parse_chemin_public(chemin)
                .is_some_and(|object| principal_reads(principal, &object))
        })
        .collect();
    let page: Vec<&String> = visible.iter().take(limit as usize).collect();
    let truncated = visible.len() > page.len();
    accepted_json(serde_json::json!({
        "paths": page,
        "truncated": truncated,
    }))
}

/// Fetch one pack's object bodies CONCURRENTLY (bounded, order
/// preserved) — the §3.6 sync gate: a 1 000-section cold pack must ride
/// ONE round trip in seconds, not a thousand sequential backend reads
/// (P4, 2026-07-21; 64-way — S3 sustains far more, the bound is
/// politeness not capacity). Coverage was already decided per part by the
/// caller; a backend error anywhere refuses the whole pack (fail-closed,
/// never a silent hole).
async fn fetch_pack_bodies(
    state: &AppState,
    target: &DataTarget,
    keys: Vec<Option<String>>,
) -> Result<Vec<Option<Option<Vec<u8>>>>, ()> {
    use futures::stream::{self, StreamExt as _};
    let fetches = stream::iter(keys.into_iter().map(|key| {
        let objects = Arc::clone(&state.objects);
        let (tenant, did) = (target.tenant.clone(), target.did.clone());
        async move {
            match key {
                None => Ok(None),
                Some(key) => objects
                    .get(&tenant, &did, &key)
                    .await
                    .map(Some)
                    .map_err(|_| ()),
            }
        }
    }))
    .buffered(64)
    .collect::<Vec<_>>()
    .await;
    fetches.into_iter().collect()
}

/// POST `/batch` — body `{"paths": […]}` (≤ 256), one multipart part per
/// path IN REQUEST ORDER: `Content-Location` + `X-Aithos-Status:
/// 200|403|404`, body only on 200 (A.3/A.8).
async fn serve_batch(
    state: &AppState,
    target: &DataTarget,
    principal: &Principal,
    body: &[u8],
    now_ms: i64,
) -> (Response<Body>, usize) {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct BatchBody {
        paths: Vec<String>,
    }
    // The request form is part of the closed wire form (named arbitrage,
    // gate contrat 5): a malformed body is `envelope_invalid` — the A.7
    // registry stays closed.
    let Ok(request) = serde_json::from_slice::<BatchBody>(body) else {
        return refuse(Refusal::EnvelopeInvalid, now_ms);
    };
    if request.paths.len() > MAX_BATCH_PATHS {
        return refuse(Refusal::PayloadTooLarge, now_ms);
    }
    // Coverage decided per path FIRST (the deny stays the path-map's),
    // then the covered bodies fetched concurrently (order preserved).
    let keys: Vec<Option<String>> = request
        .paths
        .iter()
        .map(|chemin| match pathmap::parse_chemin_public(chemin) {
            None => None,
            Some(object) if !principal_reads(principal, &object) => None,
            Some(object) => Some(object.key()),
        })
        .collect();
    let Ok(bodies) = fetch_pack_bodies(state, target, keys).await else {
        return refuse(Refusal::Unavailable, now_ms);
    };
    let mut parts = Vec::with_capacity(request.paths.len());
    let mut total = 0usize;
    for (chemin, fetched) in request.paths.iter().zip(bodies) {
        let (status, bytes) = match fetched {
            None => (403u16, None),
            Some(Some(bytes)) => (200, Some(bytes)),
            Some(None) => (404, None),
        };
        total += bytes.as_ref().map_or(0, Vec::len);
        if total > MAX_PACK_BYTES {
            return refuse(Refusal::PayloadTooLarge, now_ms);
        }
        parts.push(Part {
            location: format!("/t/{}/{}/{chemin}", target.tenant, target.did),
            status,
            bytes,
        });
    }
    multipart_response(&parts)
}

/// POST `/sync` — body `{"have_edition": N}`: the changed-paths pack
/// since the held edition (frozen rule, gate contrat 5: `manifest.json`
/// first, then the lexicographic diff of the pinned files maps held →
/// current), coverage-filtered per part like `/batch`. A held edition
/// whose `manifests/<N>.json` slot is gone answers `410 edition_gone`.
async fn serve_sync(
    state: &AppState,
    target: &DataTarget,
    principal: &Principal,
    body: &[u8],
    now_ms: i64,
) -> (Response<Body>, usize) {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SyncBody {
        have_edition: u64,
    }
    let Ok(request) = serde_json::from_slice::<SyncBody>(body) else {
        return refuse(Refusal::EnvelopeInvalid, now_ms);
    };
    let Ok(tip_read) = state
        .objects
        .get(&target.tenant, &target.did, "manifest.json")
        .await
    else {
        return refuse(Refusal::Unavailable, now_ms);
    };
    let Some(tip_bytes) = tip_read else {
        return refuse(Refusal::NotFound, now_ms);
    };
    let Ok(tip) = serde_json::from_slice::<aithos_bundle::manifest::Manifest>(&tip_bytes) else {
        // A stored tip the server cannot read is an ops fault, never a
        // silent answer.
        return refuse(Refusal::Unavailable, now_ms);
    };
    let height = tip.edition.height;
    if request.have_edition == 0 || request.have_edition > height {
        return refuse(Refusal::EditionGone, now_ms);
    }
    let changed: Vec<String> = if request.have_edition == height {
        Vec::new()
    } else {
        let held_key = format!("manifests/{}.json", request.have_edition);
        let Ok(held_read) = state
            .objects
            .get(&target.tenant, &target.did, &held_key)
            .await
        else {
            return refuse(Refusal::Unavailable, now_ms);
        };
        let Some(held_bytes) = held_read else {
            return refuse(Refusal::EditionGone, now_ms);
        };
        let Ok(held) = serde_json::from_slice::<aithos_bundle::manifest::Manifest>(&held_bytes)
        else {
            return refuse(Refusal::Unavailable, now_ms);
        };
        tip.files
            .iter()
            .filter(|(key, hash)| held.files.get(*key) != Some(*hash))
            .map(|(key, _)| key.clone())
            .collect() // BTreeMap iteration is already lexicographic
    };
    let mut parts = vec![Part {
        location: format!("/t/{}/{}/manifest.json", target.tenant, target.did),
        status: 200,
        bytes: Some(tip_bytes.to_vec()),
    }];
    let mut total = parts[0].bytes.as_ref().map_or(0, Vec::len);
    let keys: Vec<Option<String>> = changed
        .iter()
        .map(|chemin| match pathmap::parse_chemin_public(chemin) {
            None => None,
            Some(object) if !principal_reads(principal, &object) => None,
            Some(object) => Some(object.key()),
        })
        .collect();
    let Ok(bodies) = fetch_pack_bodies(state, target, keys).await else {
        return refuse(Refusal::Unavailable, now_ms);
    };
    for (chemin, fetched) in changed.iter().zip(bodies) {
        let (status, bytes) = match fetched {
            None => (403u16, None),
            Some(Some(bytes)) => (200, Some(bytes)),
            Some(None) => (404, None),
        };
        total += bytes.as_ref().map_or(0, Vec::len);
        if total > MAX_PACK_BYTES {
            return refuse(Refusal::PayloadTooLarge, now_ms);
        }
        parts.push(Part {
            location: format!("/t/{}/{}/{chemin}", target.tenant, target.did),
            status,
            bytes,
        });
    }
    multipart_response(&parts)
}

struct Part {
    location: String,
    status: u16,
    bytes: Option<Vec<u8>>,
}

/// One `multipart/mixed` response: per part `Content-Location` +
/// `X-Aithos-Status`, body only on 200 (A.3). The boundary is a fixed
/// server token — nothing in a part body is interpreted, so no boundary
/// collision can change what a part MEANS (lengths are delimited by the
/// closed header block; a hostile body is served as opaque bytes).
const PART_BOUNDARY: &str = "aithos-store-part";

fn multipart_response(parts: &[Part]) -> (Response<Body>, usize) {
    let mut wire = Vec::new();
    for part in parts {
        wire.extend_from_slice(format!("--{PART_BOUNDARY}\r\n").as_bytes());
        wire.extend_from_slice(format!("Content-Location: {}\r\n", part.location).as_bytes());
        wire.extend_from_slice(format!("X-Aithos-Status: {}\r\n\r\n", part.status).as_bytes());
        if let Some(bytes) = &part.bytes {
            wire.extend_from_slice(bytes);
        }
        wire.extend_from_slice(b"\r\n");
    }
    wire.extend_from_slice(format!("--{PART_BOUNDARY}--\r\n").as_bytes());
    let len = wire.len();
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/mixed; boundary={PART_BOUNDARY}"),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(wire))
        .expect("static response");
    (response, len)
}

/// The refusal's registry code, carried on the response so the log line
/// names exactly what was answered (codes are `&'static str` from the
/// closed registry — the log can never carry free text).
#[derive(Clone, Copy)]
struct ErrorCode(&'static str);

/// An accepted deposit's typed facts (the values `/heads` will serve):
/// `200` + a closed JSON body — never an artifact echo, never a path.
fn accepted_json(body: serde_json::Value) -> (Response<Body>, usize) {
    let body = body.to_string();
    let len = body.len();
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(body))
        .expect("static response");
    (response, len)
}

fn no_content() -> (Response<Body>, usize) {
    let response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .expect("static response");
    (response, 0)
}

/// A deposit refusal on the wire: the A.7 body plus its typed extras —
/// `head` (+ `height` on the manifest head) on `cas_mismatch`, the closed
/// short `reason` on `artifact_invalid`. Nothing else, ever.
fn refuse_deposit(refusal: DepositRefusal, now_ms: i64) -> (Response<Body>, usize) {
    let (registry, extras): (Refusal, Vec<(&str, serde_json::Value)>) = match refusal {
        DepositRefusal::Plain(code) => (code, vec![]),
        DepositRefusal::CasMismatch { head, height } => {
            let mut extras = vec![("head", serde_json::Value::String(head))];
            if let Some(height) = height {
                extras.push(("height", serde_json::json!(height)));
            }
            (Refusal::CasMismatch, extras)
        }
        DepositRefusal::Artifact(reason) => (
            Refusal::ArtifactInvalid,
            vec![("reason", serde_json::Value::String(reason.code().into()))],
        ),
    };
    let mut body = serde_json::json!({
        "error": registry.code(),
        "at": render_rfc3339z(now_ms),
    });
    for (key, value) in extras {
        body[key] = value;
    }
    let body = body.to_string();
    let len = body.len();
    let mut response = Response::builder()
        .status(StatusCode::from_u16(registry.status()).expect("registry status"))
        .header(header::CONTENT_TYPE, "application/json")
        // Arbitrage gate contrat P7 (2026-07-20): a refusal never caches
        // — a heuristically-cached 404/403 would outlive the < 60 s
        // control-plane propagation bound (RFC 9110 §9.3.2).
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(body))
        .expect("static response");
    response.extensions_mut().insert(ErrorCode(registry.code()));
    (response, len)
}

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
        // Arbitrage gate contrat P7: refusals carry no-store (see
        // refuse_deposit above).
        .header(header::CACHE_CONTROL, "no-store")
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
