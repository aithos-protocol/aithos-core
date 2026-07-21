//! G7 enterprise control and proof HTTP surface.
//!
//! CORS is an exact browser barrier. A.2 plus current Core authority is the
//! actual gate on every non-preflight request. All proof bytes stay signed or
//! ciphertext; this module only paginates and base64url-transports them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::{Extension, Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
    CACHE_CONTROL, HOST, ORIGIN, VARY,
};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::Serialize;
use serde_json::json;
use tokio::sync::Semaphore;

use crate::config::DashboardConfig;
use crate::core_bridge::{
    prepare_control_envelope, valid_control_gamma_kind, ControlAccess, ControlAuthError,
    ControlContextProof, ControlHeadsProof, ControlPage, ControlPrincipal, ControlProofReader,
    ControlRawArtifact,
};
use crate::credentials::{CredentialBroker, CredentialBrokerReadiness};
use crate::relay::{RelayHealth, RelayReadiness};
use crate::{GatewayError, Result};

const X_AITHOS_AUTH: &str = "x-aithos-auth";
const MAX_CONTROL_BODY_BYTES: usize = 64 * 1024;
const MAX_CONTROL_TARGET_BYTES: usize = 1_024;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const PREFLIGHT_MAX_AGE_SECONDS: &str = "300";
const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 100;
const NONCE_RETENTION_MS: i64 = 601_000;
const MAX_NONCES: usize = 65_536;
const MAX_CONTROL_WORKERS: usize = 32;

type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Clone)]
pub struct ControlState {
    reader: ControlProofReader,
    origins: Arc<BTreeSet<String>>,
    authorities: Arc<BTreeSet<String>>,
    nonces: ControlNonces,
    workers: Arc<Semaphore>,
    relay: RelayHealth,
    brokers: Arc<BTreeMap<String, Arc<dyn CredentialBroker>>>,
    clock: Clock,
}

impl ControlState {
    pub fn new(
        reader: ControlProofReader,
        dashboard: &DashboardConfig,
        authorities: impl IntoIterator<Item = String>,
        relay: RelayHealth,
        brokers: BTreeMap<String, Arc<dyn CredentialBroker>>,
    ) -> Result<Self> {
        let authorities: BTreeSet<String> = authorities
            .into_iter()
            .map(|authority| {
                canonical_authority(&authority).ok_or_else(|| {
                    GatewayError::ConfigRejected(
                        "dashboard control authority is not a canonical HTTP authority".into(),
                    )
                })
            })
            .collect::<Result<_>>()?;
        if authorities.is_empty() {
            return Err(GatewayError::ConfigRejected(
                "dashboard control has no accepted HTTP authority".into(),
            ));
        }
        Ok(Self {
            reader,
            origins: Arc::new(dashboard.allowed_origins.iter().cloned().collect()),
            authorities: Arc::new(authorities),
            nonces: ControlNonces::default(),
            workers: Arc::new(Semaphore::new(MAX_CONTROL_WORKERS)),
            relay,
            brokers: Arc::new(brokers),
            clock: Arc::new(system_now_ms),
        })
    }

    /// Deterministic clock seam used by contract/E2E harnesses.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    async fn vault_readiness(&self) -> &'static str {
        if self.brokers.is_empty() {
            return "unconfigured";
        }
        if self.brokers.len() > 32 {
            return "unavailable";
        }
        let probes = self
            .brokers
            .values()
            .map(|broker| broker.readiness())
            .collect::<Vec<_>>();
        match tokio::time::timeout(CONTROL_TIMEOUT, futures::future::join_all(probes)).await {
            Ok(results)
                if results
                    .iter()
                    .all(|result| *result == CredentialBrokerReadiness::Ready) =>
            {
                "ready"
            }
            _ => "unavailable",
        }
    }
}

#[derive(Clone, Default)]
struct ControlNonces(Arc<StdMutex<BTreeMap<(String, String), i64>>>);

impl ControlNonces {
    fn reserve(
        &self,
        key: &str,
        nonce: &str,
        now_ms: i64,
    ) -> std::result::Result<(), NonceRefusal> {
        let mut entries = self.0.lock().map_err(|_| NonceRefusal::Unavailable)?;
        entries.retain(|_, expires_at| *expires_at > now_ms);
        let pair = (key.to_owned(), nonce.to_owned());
        if entries.contains_key(&pair) {
            return Err(NonceRefusal::Replayed);
        }
        if entries.len() >= MAX_NONCES {
            return Err(NonceRefusal::Unavailable);
        }
        entries.insert(pair, now_ms.saturating_add(NONCE_RETENTION_MS));
        Ok(())
    }
}

enum NonceRefusal {
    Replayed,
    Unavailable,
}

#[derive(Clone)]
struct ControlRoute {
    access: ControlAccess,
    offset: usize,
    limit: usize,
}

pub fn router(state: Arc<ControlState>) -> Router {
    Router::new()
        .route("/control/v1/status", get(status).options(preflight_sink))
        .route(
            "/control/v1/contexts",
            get(contexts).options(preflight_sink),
        )
        .route(
            "/control/v1/contexts/{name}/certs",
            get(certificates).options(preflight_sink),
        )
        .route(
            "/control/v1/contexts/{name}/gamma",
            get(gamma).options(preflight_sink),
        )
        .route(
            "/control/v1/contexts/{name}/heads",
            get(heads).options(preflight_sink),
        )
        .route_layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            control_guard,
        ))
        .with_state(state)
}

async fn preflight_sink() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn control_guard(
    State(state): State<Arc<ControlState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(origin) = exact_origin(&state, request.headers()) else {
        return public_error(StatusCode::FORBIDDEN, "origin_denied");
    };
    if request.method() == Method::OPTIONS {
        return preflight(&request, &origin);
    }
    let target = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_else(|| request.uri().path())
        .to_owned();
    let route = match classify_route(request.method().as_str(), &target) {
        Some(route) => route,
        None => {
            return cors(
                public_error(StatusCode::BAD_REQUEST, "authority_denied"),
                &origin,
            )
        }
    };
    let authority = single_header(request.headers(), HOST.as_str()).and_then(canonical_authority);
    let Some(authority) = authority else {
        return cors(
            public_error(StatusCode::UNAUTHORIZED, "authority_denied"),
            &origin,
        );
    };
    let auth = single_header(request.headers(), X_AITHOS_AUTH).map(str::to_owned);
    let Some(auth) = auth else {
        return cors(
            public_error(StatusCode::UNAUTHORIZED, "authority_denied"),
            &origin,
        );
    };
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, MAX_CONTROL_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            return cors(
                public_error(StatusCode::PAYLOAD_TOO_LARGE, "authority_denied"),
                &origin,
            )
        }
    };
    if !body.is_empty() {
        return cors(
            public_error(StatusCode::UNAUTHORIZED, "authority_denied"),
            &origin,
        );
    }
    let now_ms = (state.clock)();
    let prepared = match prepare_control_envelope(
        &auth,
        &authority,
        &state.authorities,
        parts.method.as_str(),
        &target,
        &body,
        now_ms,
    ) {
        Ok(prepared) => prepared,
        Err(_) => {
            return cors(
                public_error(StatusCode::UNAUTHORIZED, "authority_denied"),
                &origin,
            )
        }
    };
    if let Err(refusal) = state
        .nonces
        .reserve(prepared.key(), prepared.nonce(), now_ms)
    {
        let (status, code) = match refusal {
            NonceRefusal::Replayed => (StatusCode::UNAUTHORIZED, "authority_denied"),
            NonceRefusal::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "gateway_offline"),
        };
        return cors(public_error(status, code), &origin);
    }
    let reader = state.reader.clone();
    let access = route.access.clone();
    let permit =
        match tokio::time::timeout(CONTROL_TIMEOUT, Arc::clone(&state.workers).acquire_owned())
            .await
        {
            Ok(Ok(permit)) => permit,
            _ => {
                return cors(
                    public_error(StatusCode::SERVICE_UNAVAILABLE, "gateway_offline"),
                    &origin,
                )
            }
        };
    let authority = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        reader.verify_authority(&prepared, &access, now_ms)
    });
    let principal = match tokio::time::timeout(CONTROL_TIMEOUT, authority).await {
        Ok(Ok(Ok(principal))) => principal,
        Ok(Ok(Err(ControlAuthError::Unavailable))) | Ok(Err(_)) | Err(_) => {
            return cors(
                public_error(StatusCode::SERVICE_UNAVAILABLE, "gateway_offline"),
                &origin,
            )
        }
        Ok(Ok(Err(_))) => {
            return cors(
                public_error(StatusCode::UNAUTHORIZED, "authority_denied"),
                &origin,
            )
        }
    };
    request = Request::from_parts(parts, Body::from(body));
    request.extensions_mut().insert(principal);
    request.extensions_mut().insert(route);
    let response = match tokio::time::timeout(CONTROL_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => public_error(StatusCode::SERVICE_UNAVAILABLE, "gateway_offline"),
    };
    cors(response, &origin)
}

fn preflight(request: &Request, origin: &str) -> Response {
    let requested_method = single_header(request.headers(), ACCESS_CONTROL_REQUEST_METHOD.as_str());
    let target = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_else(|| request.uri().path());
    let Some(method) = requested_method.filter(|method| *method == "GET") else {
        return cors(public_error(StatusCode::FORBIDDEN, "origin_denied"), origin);
    };
    if classify_route(method, target).is_none() {
        return cors(public_error(StatusCode::FORBIDDEN, "origin_denied"), origin);
    }
    let requested_headers =
        single_header(request.headers(), ACCESS_CONTROL_REQUEST_HEADERS.as_str())
            .and_then(parse_requested_headers);
    let Some(requested_headers) = requested_headers else {
        return cors(public_error(StatusCode::FORBIDDEN, "origin_denied"), origin);
    };
    if !requested_headers.contains(X_AITHOS_AUTH)
        || requested_headers
            .iter()
            .any(|header| !matches!(header.as_str(), X_AITHOS_AUTH | "content-type"))
    {
        return cors(public_error(StatusCode::FORBIDDEN, "origin_denied"), origin);
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET"),
    );
    let allowed_headers = if requested_headers.contains("content-type") {
        "Content-Type, X-Aithos-Auth"
    } else {
        "X-Aithos-Auth"
    };
    response.headers_mut().insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(allowed_headers),
    );
    response.headers_mut().insert(
        ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static(PREFLIGHT_MAX_AGE_SECONDS),
    );
    cors(response, origin)
}

fn parse_requested_headers(value: &str) -> Option<BTreeSet<String>> {
    let mut headers = BTreeSet::new();
    for value in value.split(',') {
        let value = value.trim();
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || !headers.insert(value.to_ascii_lowercase())
        {
            return None;
        }
    }
    (!headers.is_empty()).then_some(headers)
}

fn exact_origin(state: &ControlState, headers: &axum::http::HeaderMap) -> Option<String> {
    let origin = single_header(headers, ORIGIN.as_str())?;
    state.origins.contains(origin).then(|| origin.to_owned())
}

fn single_header<'a>(headers: &'a axum::http::HeaderMap, name: &'static str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

fn cors(mut response: Response, origin: &str) -> Response {
    if let Ok(origin) = HeaderValue::from_str(origin) {
        response
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Origin"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn public_error(status: StatusCode, code: &'static str) -> Response {
    let mut response = (status, Json(json!({ "error": code }))).into_response();
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Origin"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn canonical_authority(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > 512 || raw.contains('@') {
        return None;
    }
    let authority: axum::http::uri::Authority = raw.parse().ok()?;
    let host = authority.host().to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    let display_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    match authority.port_u16() {
        Some(443) | None => Some(display_host),
        Some(port) => Some(format!("{display_host}:{port}")),
    }
}

fn classify_route(method: &str, target: &str) -> Option<ControlRoute> {
    if method != "GET" || target.len() > MAX_CONTROL_TARGET_BYTES {
        return None;
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if path == "/control/v1/status" && query.is_none() {
        return Some(ControlRoute {
            access: ControlAccess::Status,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        });
    }
    if path == "/control/v1/contexts" && query.is_none() {
        return Some(ControlRoute {
            access: ControlAccess::Contexts,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        });
    }
    let rest = path.strip_prefix("/control/v1/contexts/")?;
    let (context, surface) = rest.split_once('/')?;
    if !canonical_label(context) || surface.contains('/') {
        return None;
    }
    match surface {
        "certs" => {
            let page = parse_page_query(query, false)?;
            Some(ControlRoute {
                access: ControlAccess::Certificates {
                    context: context.to_owned(),
                },
                offset: page.offset,
                limit: page.limit,
            })
        }
        "gamma" => {
            let page = parse_page_query(query, true)?;
            Some(ControlRoute {
                access: ControlAccess::Gamma {
                    context: context.to_owned(),
                    kind: page.kind,
                },
                offset: page.offset,
                limit: page.limit,
            })
        }
        "heads" if query.is_none() => Some(ControlRoute {
            access: ControlAccess::Heads {
                context: context.to_owned(),
            },
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        }),
        _ => None,
    }
}

struct PageQuery {
    kind: Option<String>,
    offset: usize,
    limit: usize,
}

fn parse_page_query(query: Option<&str>, allow_kind: bool) -> Option<PageQuery> {
    let mut kind = None;
    let mut offset = 0usize;
    let mut limit = DEFAULT_PAGE_LIMIT;
    let mut seen = BTreeSet::new();
    if let Some(query) = query {
        if query.is_empty() || query.contains(['%', '+']) {
            return None;
        }
        for parameter in query.split('&') {
            let (name, value) = parameter.split_once('=')?;
            if value.is_empty() || !seen.insert(name) {
                return None;
            }
            match name {
                "kind" if allow_kind && canonical_kind(value) => kind = Some(value.to_owned()),
                "cursor" => {
                    offset = value.strip_prefix("v1.")?.parse().ok()?;
                }
                "limit" => {
                    limit = value.parse().ok()?;
                    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    }
    Some(PageQuery {
        kind,
        offset,
        limit,
    })
}

fn canonical_label(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn canonical_kind(value: &str) -> bool {
    valid_control_gamma_kind(value)
}

async fn status(State(state): State<Arc<ControlState>>) -> Json<StatusResponse> {
    Json(StatusResponse {
        version: 1,
        process: "ready",
        vault: state.vault_readiness().await,
        relay: match state.relay.get() {
            RelayReadiness::Disabled => "disabled",
            RelayReadiness::Connecting => "connecting",
            RelayReadiness::Ready => "ready",
            RelayReadiness::Unavailable => "unavailable",
        },
    })
}

async fn contexts(
    State(state): State<Arc<ControlState>>,
    Extension(principal): Extension<ControlPrincipal>,
) -> Response {
    let reader = state.reader.clone();
    proof_response(&state, move || {
        reader.contexts(&principal).map(contexts_dto)
    })
    .await
}

async fn certificates(
    State(state): State<Arc<ControlState>>,
    Extension(principal): Extension<ControlPrincipal>,
    Extension(route): Extension<ControlRoute>,
) -> Response {
    let reader = state.reader.clone();
    proof_response(&state, move || {
        reader
            .certificates(&principal, route.offset, route.limit)
            .map(page_dto)
    })
    .await
}

async fn gamma(
    State(state): State<Arc<ControlState>>,
    Extension(principal): Extension<ControlPrincipal>,
    Extension(route): Extension<ControlRoute>,
) -> Response {
    let reader = state.reader.clone();
    let kind = match &route.access {
        ControlAccess::Gamma { kind, .. } => kind.clone(),
        _ => None,
    };
    proof_response(&state, move || {
        reader
            .gamma(&principal, kind.as_deref(), route.offset, route.limit)
            .map(page_dto)
    })
    .await
}

async fn heads(
    State(state): State<Arc<ControlState>>,
    Extension(principal): Extension<ControlPrincipal>,
) -> Response {
    let reader = state.reader.clone();
    proof_response(&state, move || reader.heads(&principal).map(heads_dto)).await
}

async fn proof_response<T, F>(state: &ControlState, proof: F) -> Response
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let permit =
        match tokio::time::timeout(CONTROL_TIMEOUT, Arc::clone(&state.workers).acquire_owned())
            .await
        {
            Ok(Ok(permit)) => permit,
            _ => return public_error(StatusCode::SERVICE_UNAVAILABLE, "gateway_offline"),
        };
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        proof()
    });
    match tokio::time::timeout(CONTROL_TIMEOUT, task).await {
        Ok(Ok(Ok(value))) => Json(value).into_response(),
        _ => public_error(StatusCode::SERVICE_UNAVAILABLE, "gateway_offline"),
    }
}

#[derive(Serialize)]
struct StatusResponse {
    version: u8,
    process: &'static str,
    vault: &'static str,
    relay: &'static str,
}

#[derive(Serialize)]
struct ArtifactDto {
    path: String,
    bytes_b64: String,
}

#[derive(Serialize)]
struct PageDto {
    version: u8,
    items: Vec<ArtifactDto>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct ContextDto {
    name: String,
    did: String,
    did_document: ArtifactDto,
}

#[derive(Serialize)]
struct ContextsDto {
    version: u8,
    items: Vec<ContextDto>,
}

#[derive(Serialize)]
struct HeadsDto {
    version: u8,
    context: String,
    did: String,
    manifest: Option<ArtifactDto>,
    gamma_tail: Option<ArtifactDto>,
}

fn artifact_dto(artifact: ControlRawArtifact) -> ArtifactDto {
    ArtifactDto {
        path: artifact.path,
        bytes_b64: URL_SAFE_NO_PAD.encode(artifact.bytes),
    }
}

fn page_dto(page: ControlPage) -> PageDto {
    PageDto {
        version: 1,
        items: page.items.into_iter().map(artifact_dto).collect(),
        next_cursor: page.next_offset.map(|offset| format!("v1.{offset}")),
    }
}

fn contexts_dto(contexts: Vec<ControlContextProof>) -> ContextsDto {
    ContextsDto {
        version: 1,
        items: contexts
            .into_iter()
            .map(|context| ContextDto {
                name: context.name,
                did: context.did,
                did_document: artifact_dto(context.did_document),
            })
            .collect(),
    }
}

fn heads_dto(heads: ControlHeadsProof) -> HeadsDto {
    HeadsDto {
        version: 1,
        context: heads.context,
        did: heads.did,
        manifest: heads.manifest.map(artifact_dto),
        gamma_tail: heads.gamma_tail.map(artifact_dto),
    }
}

fn system_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_grammar_is_closed_and_pagination_is_bounded() {
        assert!(classify_route("GET", "/control/v1/status").is_some());
        assert!(classify_route(
            "GET",
            "/control/v1/contexts/company-brand/gamma?kind=action&cursor=v1.2&limit=10"
        )
        .is_some());
        for rejected in [
            ("POST", "/control/v1/status"),
            ("GET", "/control/v1/status?x=1"),
            ("GET", "/control/v1/contexts/../gamma"),
            ("GET", "/control/v1/contexts/company/gamma?limit=101"),
            (
                "GET",
                "/control/v1/contexts/company/gamma?kind=action&kind=revoke",
            ),
            (
                "GET",
                "/control/v1/contexts/company/gamma?kind=transport-local-kind",
            ),
            ("GET", "/control/v1/contexts/company/certs?kind=action"),
        ] {
            assert!(classify_route(rejected.0, rejected.1).is_none());
        }
    }

    #[test]
    fn preflight_header_grammar_is_unique_and_closed() {
        assert_eq!(
            parse_requested_headers("Content-Type, X-Aithos-Auth"),
            Some(BTreeSet::from([
                "content-type".to_owned(),
                "x-aithos-auth".to_owned(),
            ]))
        );
        for rejected in [
            "",
            "X-Aithos-Auth,",
            "X-Aithos-Auth, x-aithos-auth",
            "X-Aithos-Auth, bad header",
        ] {
            assert!(parse_requested_headers(rejected).is_none());
        }
    }

    #[test]
    fn nonce_is_one_shot_and_capacity_failure_is_closed() {
        let nonces = ControlNonces::default();
        assert!(nonces.reserve("#content", "n-1", 1_000).is_ok());
        assert!(matches!(
            nonces.reserve("#content", "n-1", 1_001),
            Err(NonceRefusal::Replayed)
        ));
        assert!(nonces
            .reserve("#content", "n-1", 1_000 + NONCE_RETENTION_MS)
            .is_ok());
    }

    #[test]
    fn authorities_are_normalized_without_cross_plane_guessing() {
        assert_eq!(
            canonical_authority("ACME.MCP.AITHOS.FR:443").as_deref(),
            Some("acme.mcp.aithos.fr")
        );
        assert_eq!(
            canonical_authority("127.0.0.1:4870").as_deref(),
            Some("127.0.0.1:4870")
        );
        assert!(canonical_authority("user@host").is_none());
    }
}
