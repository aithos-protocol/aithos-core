//! MCP proxy — the agent-facing side of the gateway.
//!
//! Transport (decided 2026-07-10): Streamable HTTP first — JSON-RPC 2.0
//! over POST, the shape a network sidecar interposes on naturally. stdio
//! upstreams can be wrapped later without touching this flow.
//!
//! Flow for `tools/call` (GATEWAY-BOOTSTRAP §5):
//! 1. map the tool through the enterprise whitelist (absent → deny),
//! 2. verify the mandate covers the op at T (`core_bridge::authorize`),
//! 3. **log before relaying** — an unlogged act must never reach the
//!    upstream; if the gamma append fails, the call is refused,
//! 4. relay to the real MCP server and hand the answer back.
//!
//! Refusals are logged too, as governance acts of the *gateway's own*
//! identity (the agent did not act — that is the point), then surfaced
//! as a JSON-RPC error. Everything else (`initialize`, `tools/list`,
//! notifications) passes through untouched in the MVP.
//!
//! **Multi-context router (v2, lot 3).** [`McpRouter`] serves N
//! provisioned contexts at once: each `tools/call` resolves to the ONE
//! context whose tool map names it, is authorised against that context's
//! mandate, logged there (plus the journal xref) and relayed to that
//! context's own upstream. v1 simplification, documented on purpose:
//! with N upstreams there is no single passthrough target, so the router
//! answers `initialize` itself (static minimal result), aggregates
//! `tools/list` from the declared maps (names only), and refuses every
//! other method with JSON-RPC `-32601` — honest, and exactly what the
//! routed scenarios need. Streaming/SSE and per-upstream capability
//! merging land with Phase D.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::core_bridge::{Bridge, EntropySource, Runner};
use crate::credentials::{CredentialBroker, CredentialRef};
use crate::hub::discover_server;
use crate::policy::Policy;
use crate::{GatewayError, Result};

/// JSON-RPC error code for a gateway policy refusal (implementation-defined
/// range). The call never reached the tool.
pub const POLICY_DENIED_CODE: i64 = -32001;

/// JSON-RPC "method not found" — what the multi-context router answers
/// for methods it does not serve (v1: everything but `initialize`,
/// `tools/list`, `tools/call` and `ping`).
pub const METHOD_NOT_FOUND_CODE: i64 = -32601;

/// JSON-RPC "invalid request" — id-less non-notifications (an act whose
/// result has no return channel must never happen) and batched bodies
/// (removed from the protocol by the 2025-06-18 revision; refused here,
/// decided 2026-07-16).
pub const INVALID_REQUEST_CODE: i64 = -32600;

/// JSON-RPC "parse error" — a POST body that is not JSON at all.
pub const PARSE_ERROR_CODE: i64 = -32700;

/// The Streamable HTTP session header (MCP 2025-03-26). Decided
/// 2026-07-16: the gateway is STATELESS — it emits an opaque id on
/// `initialize` (injected entropy, visible ASCII) and echoes whatever
/// id the client presents on later calls, but never requires or stores
/// one. Authority never rides this header: it stays with the mandate
/// chain (per-session chains arrive with G5, through OAuth).
pub const MCP_SESSION_HEADER: &str = "mcp-session-id";

/// The native journal tools (lot C2): served by the gateway itself on
/// `/mcp`, never relayed to any upstream. The `journal` prefix is
/// reserved at config time so no context tool can ever shadow them.
pub const JOURNAL_WRITE: &str = "journal.write";
pub const JOURNAL_SEARCH: &str = "journal.search";
/// The native briefing tool (lot K): the owner's directives, served by
/// the hub itself from the public+circle zones of the granted contexts
/// (`self` never). Conditional surface: it is listed — and `initialize`
/// recommends it — only while a granted zone has something to say. The
/// `briefing` prefix is reserved at config time like `journal`.
pub const BRIEFING_READ: &str = "briefing.read";
/// The native Ethos data tools (lot G6): served by the hub itself,
/// never relayed, their surface DERIVED from the mandates per call
/// (decided 2026-07-16) — public content informs any connected
/// session, sealed zones appear only under a covering chain whose
/// lines open them. The `ethos` prefix is reserved at config time like
/// `journal` and `briefing`.
pub const ETHOS_READ: &str = "ethos.read";
pub const ETHOS_LIST: &str = "ethos.list";
pub const ETHOS_CONTEXT: &str = "ethos.context";
/// `journal.search` result cap: the default page, and the hard ceiling
/// a caller-supplied `limit` may not exceed (each opened hit is one
/// journalized read — the cap bounds the gamma cost of one recall).
const SEARCH_LIMIT_DEFAULT: u64 = 20;
const SEARCH_LIMIT_MAX: u64 = 100;

/// The MCP protocol revision the router's own `initialize` speaks.
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

/// Injected clock, RFC 3339 `Z` instants (the wire's instant format —
/// never epoch numbers). The binary passes system time; tests pass a
/// fixed T.
pub type Clock = Arc<dyn Fn() -> String + Send + Sync>;

/// Seam to the real MCP server: HTTP in production, in-process fake in
/// the acceptance tests (GATEWAY-BOOTSTRAP §8).
pub trait Upstream: Send + Sync + 'static {
    fn forward(&self, body: Value) -> impl std::future::Future<Output = Result<Value>> + Send;
}

/// How one HTTP upstream authenticates on the wire. The agent never
/// chooses this — it is config custody (inline legacy) or vault custody
/// (brokered), decided at startup.
enum UpstreamAuth {
    /// No wire credential (dev fakes, open upstreams).
    None,
    /// LEGACY/UNSAFE (H3 seam): the clear token from the config file,
    /// durably in process memory. Kept for old configs only.
    InlineBearer(String),
    /// The governed seam: a non-secret reference resolved through the
    /// enterprise broker per call — at the last possible moment, after
    /// the caller logged the act. No secret is retained between calls,
    /// so a vault-side rotation is honoured on the very next relay.
    Brokered {
        broker: Arc<dyn CredentialBroker>,
        reference: CredentialRef,
    },
}

/// Production upstream: JSON-RPC over POST (Streamable HTTP, stateless).
pub struct HttpUpstream {
    client: reqwest::Client,
    url: String,
    auth: UpstreamAuth,
}

impl HttpUpstream {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
            auth: UpstreamAuth::None,
        }
    }

    pub fn with_bearer(url: impl Into<String>, bearer_token: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
            auth: match bearer_token {
                Some(token) => UpstreamAuth::InlineBearer(token),
                None => UpstreamAuth::None,
            },
        }
    }

    pub fn with_credential(
        url: impl Into<String>,
        broker: Arc<dyn CredentialBroker>,
        reference: CredentialRef,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
            auth: UpstreamAuth::Brokered { broker, reference },
        }
    }

    /// The one constructor the binary and the harnesses share: wire one
    /// declared server to its credential source (brokered reference,
    /// legacy inline bearer, or none), fail-closed on a dangling broker
    /// name — the config validator already refused ambiguity.
    pub fn for_server(
        server: &crate::config::ServerConfig,
        brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
    ) -> Result<Self> {
        if let Some(reference) = &server.credential {
            let broker = brokers.get(&reference.broker).cloned().ok_or_else(|| {
                GatewayError::ConfigRejected(format!(
                    "servers[{}].credential references unknown credential broker `{}`",
                    server.name, reference.broker
                ))
            })?;
            return Ok(Self::with_credential(
                server.url.clone(),
                broker,
                reference.clone(),
            ));
        }
        Ok(Self::with_bearer(
            server.url.clone(),
            server.bearer_token.clone(),
        ))
    }
}

impl Upstream for HttpUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        let mut request = self
            .client
            .post(&self.url)
            .json(&body)
            .header("accept", "application/json");
        match &self.auth {
            UpstreamAuth::None => {}
            UpstreamAuth::InlineBearer(token) => request = request.bearer_auth(token),
            UpstreamAuth::Brokered { broker, reference } => {
                // Resolved at the last possible moment: any broker
                // failure surfaces BEFORE the request is sent, so a
                // vault outage can never produce an unauthenticated or
                // half-credentialed upstream call. The secret drops
                // (and zeroizes) as soon as the header is built.
                let secret = broker.resolve(reference).await?;
                request = request.bearer_auth(secret.expose());
            }
        }
        let resp = request
            .send()
            .await
            .map_err(|e| GatewayError::UpstreamFailed(e.to_string()))?;
        resp.json::<Value>()
            .await
            .map_err(|e| GatewayError::UpstreamFailed(e.to_string()))
    }
}

/// The proxy state shared by all requests.
pub struct McpProxy<U> {
    pub policy: Policy,
    pub bridge: Mutex<Bridge>,
    pub upstream: U,
    pub clock: Clock,
}

/// Agent-facing router: one Streamable HTTP endpoint.
pub fn router<U: Upstream>(proxy: Arc<McpProxy<U>>) -> Router {
    Router::new()
        .route("/mcp", post(handle::<U>))
        .with_state(proxy)
}

async fn handle<U: Upstream>(
    State(gw): State<Arc<McpProxy<U>>>,
    Json(msg): Json<Value>,
) -> Json<Value> {
    Json(process(&gw, msg).await)
}

/// Transport-free core of the proxy — acceptance tests drive this
/// directly, the axum handler is a thin shell around it.
pub async fn process<U: Upstream>(gw: &McpProxy<U>, msg: Value) -> Value {
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    if method == "tools/call" {
        tool_call(gw, msg).await
    } else {
        // Passthrough: initialize, tools/list, notifications…
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        match gw.upstream.forward(msg).await {
            Ok(resp) => resp,
            Err(e) => error_response(id, &e),
        }
    }
}

async fn tool_call<U: Upstream>(gw: &McpProxy<U>, msg: Value) -> Value {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let now = (gw.clock)();

    // A call we cannot even name is refused and recorded as such.
    let Some(tool) = msg
        .pointer("/params/name")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        let e = GatewayError::RequestRejected("tools/call without params.name".into());
        let mut bridge = gw.bridge.lock().await;
        let _ = bridge.record_refusal("<unnamed>", e.refusal_code(), &now);
        return error_response(id, &e);
    };

    // The arguments only ever enter the log as a hash.
    let args = msg
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    // Fail-closed policy: whitelist map first, then the mandate at T —
    // both layers must say yes, and the mandate is the authority.
    let mut bridge = gw.bridge.lock().await;
    let denial = match gw.policy.access_for(&tool) {
        Ok(_) => bridge.authorize(&tool, &now).err(),
        Err(e) => Some(e),
    };

    if let Some(deny) = denial {
        // Refusals are governance acts of the gateway itself: logged,
        // then surfaced. Even if the refusal log fails we still refuse.
        let _ = bridge.record_refusal(&tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }

    // Log before relaying: if the act cannot be recorded, it does not happen.
    if let Err(e) = bridge.record_act(&tool, &args, &now) {
        let deny = GatewayError::LogAppendRefused(e.to_string());
        let _ = bridge.record_refusal(&tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }
    drop(bridge);

    match gw.upstream.forward(msg).await {
        Ok(resp) => resp,
        Err(e) => error_response(id, &e),
    }
}

/// A refusal or failure as a well-formed JSON-RPC error message.
fn error_response(id: Value, err: &GatewayError) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": POLICY_DENIED_CODE,
            "message": format!("aithos gateway: {err}"),
        }
    })
}

// ------------------------------------------------- multi-context router

/// The multi-context router state: the runner (N context bridges + the
/// journal) and one upstream per context, keyed by the same names. The
/// runner is shared (`Arc`) so the LLM front (Phase C) meters into the
/// SAME journal — one story, never a second bridge over one store.
/// `session_entropy` mints the opaque `Mcp-Session-Id` values — the
/// gateway only ever has the injected [`EntropySource`], no wild rand.
pub struct McpRouter<U> {
    pub runner: Arc<Mutex<Runner>>,
    pub upstreams: BTreeMap<String, U>,
    pub clock: Clock,
    pub session_entropy: std::sync::Mutex<Box<dyn EntropySource + Send>>,
    /// The embedded OAuth authorization server (lot G3), when the `as:`
    /// stanza is active. `None` = byte-identical legacy behaviour: `/mcp`
    /// stays open on loopback, the AS endpoints do not exist. `Some` =
    /// `/mcp` requires a valid bearer (401 + `WWW-Authenticate` pointing
    /// the resource metadata otherwise) and the AS rides this listener.
    pub oauth: Option<Arc<crate::oauth::AuthServer>>,
}

impl<U> McpRouter<U> {
    /// One opaque session id: hex of 16 injected-entropy bytes —
    /// visible ASCII per spec, never a secret, never an authority.
    fn mint_session_id(&self) -> String {
        let mut ent = self.session_entropy.lock().expect("session entropy lock");
        hex::encode(ent.e16())
    }
}

/// Agent-facing router for the multi-context runtime: same single
/// Streamable HTTP endpoint as the mono proxy.
pub fn router_multi<U: Upstream>(rt: Arc<McpRouter<U>>) -> Router {
    Router::new()
        .route("/mcp", post(handle_multi::<U>))
        .with_state(rt)
}

/// The Streamable HTTP shell around [`process_multi`] (lot G2): the
/// transport rules a real MCP host exercises, applied in order and
/// fail-closed — Origin first (spec security MUST, anti DNS-rebinding),
/// then body shape (no batches), then the notification rule (a JSON-RPC
/// message without an id is NEVER answered: HTTP 202, empty body — and
/// an id-less non-notification is refused 400, never silently acted
/// on), and last the session header: minted on `initialize`, echoed
/// when presented, required never (stateless, decided 2026-07-16).
async fn handle_multi<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !origin_is_local(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // The bearer gate (lot G3): only when the `as:` stanza is active.
    // Absent, this whole block vanishes and `/mcp` stays byte-identical.
    // Order is deliberate — Origin (above) outranks a missing token, and
    // a valid token only grants ENTRY: the mandate chain still decides
    // every act behind it (a token is never an authority).
    if let Some(oauth) = &rt.oauth {
        let now = (rt.clock)();
        let presented = bearer_token(&headers);
        let ok = presented
            .as_deref()
            .is_some_and(|token| oauth.validate_bearer(token, &now).is_ok());
        if !ok {
            let mut resp = StatusCode::UNAUTHORIZED.into_response();
            if let Ok(v) = HeaderValue::from_str(&oauth.www_authenticate(presented.is_some())) {
                resp.headers_mut().insert(header::WWW_AUTHENTICATE, v);
            }
            return resp;
        }
    }
    let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(rpc_error_null_id(
                PARSE_ERROR_CODE,
                "aithos gateway: request body is not valid JSON",
            )),
        )
            .into_response();
    };
    if msg.is_array() {
        return Json(rpc_error_null_id(
            INVALID_REQUEST_CODE,
            "aithos gateway: batching is not supported — one JSON-RPC message per POST",
        ))
        .into_response();
    }
    if msg.get("id").is_none_or(Value::is_null) {
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if method.starts_with("notifications/") {
            return StatusCode::ACCEPTED.into_response();
        }
        return (
            StatusCode::BAD_REQUEST,
            Json(rpc_error_null_id(
                INVALID_REQUEST_CODE,
                "aithos gateway: id-less request refused — only notifications/* may omit the id",
            )),
        )
            .into_response();
    }
    let is_initialize = msg.get("method").and_then(Value::as_str) == Some("initialize");
    let presented = headers.get(MCP_SESSION_HEADER).cloned();
    let mut resp = Json(process_multi(&rt, msg).await).into_response();
    if is_initialize {
        if let Ok(v) = HeaderValue::from_str(&rt.mint_session_id()) {
            resp.headers_mut().insert(MCP_SESSION_HEADER, v);
        }
    } else if let Some(v) = presented {
        resp.headers_mut().insert(MCP_SESSION_HEADER, v);
    }
    resp
}

/// One well-formed JSON-RPC error with a null id — the transport-level
/// refusals (parse, batch, id-less) that never reach the router.
fn rpc_error_null_id(code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": { "code": code, "message": message }
    })
}

/// Origin validation (MCP 2025-03-26 security MUST, decided 2026-07-16):
/// absent = a non-browser client, pass; present and loopback-hosted =
/// pass; anything else = refused before any JSON-RPC processing. The
/// error never carries the offending value anywhere near a log.
fn origin_is_local(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .unwrap_or("");
    let host = rest.split('/').next().unwrap_or("");
    let host = if let Some(v6) = host.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// The bearer token presented on `/mcp` (`Authorization: Bearer <t>`),
/// if any. The value is never logged.
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
}

// ------------------------------------------------- OAuth AS (lot G3)

/// The authorization-server routes (lot G3): discovery metadata,
/// dynamic registration, the authorize page + consent, and the token
/// endpoint. They ride the SAME listener as `/mcp` (the G2 shell
/// precedent) and share the router state so the token endpoint can read
/// the runner's live authority ceiling. Merged only when `as:` is
/// active — absent, none of these paths exist (404), and the gateway is
/// byte-identical.
pub fn router_oauth<U: Upstream>(rt: Arc<McpRouter<U>>) -> Router {
    Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            axum::routing::get(oauth_protected_resource::<U>),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            axum::routing::get(oauth_as_metadata::<U>),
        )
        .route("/register", post(oauth_register::<U>))
        .route(
            "/authorize",
            axum::routing::get(oauth_authorize_get::<U>).post(oauth_authorize_post::<U>),
        )
        .route("/token", post(oauth_token::<U>))
        .with_state(rt)
}

/// One OAuth error as an HTTP response: `{error, error_description}` with
/// the status the code implies (RFC 6749 §5.2). The detail is a fixed,
/// leak-free string built by the AS — never a token or a secret.
fn oauth_error_response(err: &GatewayError) -> Response {
    let (error, detail) = match err {
        GatewayError::OauthDenied { error, detail } => (error.clone(), detail.clone()),
        other => ("server_error".to_owned(), other.to_string()),
    };
    let status = match error.as_str() {
        "invalid_client_metadata" | "invalid_redirect_uri" => StatusCode::BAD_REQUEST,
        "invalid_token" => StatusCode::UNAUTHORIZED,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(json!({ "error": error, "error_description": detail })),
    )
        .into_response()
}

async fn oauth_protected_resource<U: Upstream>(State(rt): State<Arc<McpRouter<U>>>) -> Response {
    match &rt.oauth {
        Some(oauth) => Json(oauth.protected_resource_metadata()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn oauth_as_metadata<U: Upstream>(State(rt): State<Arc<McpRouter<U>>>) -> Response {
    match &rt.oauth {
        Some(oauth) => Json(oauth.authorization_server_metadata()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn oauth_register<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    Json(body): Json<Value>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match oauth.register(&body) {
        Ok(doc) => (StatusCode::CREATED, Json(doc)).into_response(),
        Err(e) => oauth_error_response(&e),
    }
}

/// Assemble an [`crate::oauth::AuthorizeRequest`] from a flat parameter
/// map (query string or form body).
fn authorize_request(params: &BTreeMap<String, String>) -> crate::oauth::AuthorizeRequest {
    let get = |k: &str| params.get(k).cloned();
    crate::oauth::AuthorizeRequest {
        client_id: get("client_id").unwrap_or_default(),
        redirect_uri: get("redirect_uri").unwrap_or_default(),
        response_type: get("response_type").unwrap_or_default(),
        code_challenge: get("code_challenge"),
        code_challenge_method: get("code_challenge_method"),
        resource: get("resource"),
        scope: get("scope"),
        state: get("state"),
    }
}

async fn oauth_authorize_get<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    axum::extract::Query(params): axum::extract::Query<BTreeMap<String, String>>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match oauth.authorize(&authorize_request(&params)) {
        crate::oauth::AuthorizeOutcome::Consent { html } => {
            axum::response::Html(html).into_response()
        }
        crate::oauth::AuthorizeOutcome::Redirect { location } => redirect_to(&location),
        crate::oauth::AuthorizeOutcome::HardError { detail } => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_request", "error_description": detail })),
        )
            .into_response(),
    }
}

async fn oauth_authorize_post<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    axum::extract::Form(params): axum::extract::Form<BTreeMap<String, String>>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = (rt.clock)();
    match oauth.approve(&authorize_request(&params), &now) {
        Ok(location) => redirect_to(&location),
        Err(e) => oauth_error_response(&e),
    }
}

async fn oauth_token<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    axum::extract::Form(params): axum::extract::Form<BTreeMap<String, String>>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = (rt.clock)();
    let get = |k: &str| params.get(k).cloned().unwrap_or_default();
    // The bound authority's ceiling (injectable pre-G4): the runner's
    // live agent-chain `not_after`, read fresh. G4/G5 swap in the
    // session sub-mandate's not_after through this same call.
    let ceiling = rt.runner.lock().await.agent_authority_ceiling(&now);
    let grant = match get("grant_type").as_str() {
        "authorization_code" => oauth.exchange_code(
            &get("code"),
            &get("code_verifier"),
            &get("resource"),
            &get("redirect_uri"),
            ceiling.as_deref(),
            &now,
        ),
        "refresh_token" => oauth.refresh(&get("refresh_token"), ceiling.as_deref(), &now),
        other => {
            return oauth_error_response(&GatewayError::OauthDenied {
                error: "unsupported_grant_type".into(),
                detail: format!("grant_type `{other}` is not served"),
            })
        }
    };
    match grant {
        Ok((tokens, client_id)) => {
            // Issuance is an act, never silent (I5): one governance entry
            // in the journal, naming the client — no token byte in it.
            rt.runner.lock().await.record_oauth_issue(&client_id, &now);
            Json(json!({
                "access_token": tokens.access_token,
                "token_type": "Bearer",
                "expires_in": tokens.access_expires_secs,
                "refresh_token": tokens.refresh_token,
            }))
            .into_response()
        }
        Err(e) => oauth_error_response(&e),
    }
}

/// A 302 redirect to an absolute location (the AS's authorize replies).
fn redirect_to(location: &str) -> Response {
    match HeaderValue::from_str(location) {
        Ok(v) => {
            let mut resp = StatusCode::FOUND.into_response();
            resp.headers_mut().insert(header::LOCATION, v);
            resp
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Transport-free core of the multi-context router — acceptance tests
/// drive this directly. Serves `tools/call` (routed), answers
/// `initialize` and `tools/list` itself, refuses the rest (v1 — see the
/// module doc).
pub async fn process_multi<U: Upstream>(rt: &McpRouter<U>, msg: Value) -> Value {
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let id = msg.get("id").cloned().unwrap_or(Value::Null);

    match method.as_str() {
        "tools/call" => tool_call_multi(rt, msg).await,
        // The MCP liveness probe: an empty result, promptly, touching
        // neither the runner nor any upstream (spec utilities/ping).
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "initialize" => {
            // The static minimal result, plus the briefing instructions
            // WHEN a granted zone has directives (lot K, decision n°5)
            // and the ethos-data sentence WHEN the derived surface is
            // non-mute (lot G6). Recomputed per call on purpose: an
            // owner edit, a fresh grant or a revocation flips the
            // surface with no restart.
            let mut result = json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "aithos-gateway",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            });
            let now = (rt.clock)();
            let runner = rt.runner.lock().await;
            let briefed = runner.briefing_available();
            let surface = runner.ethos_surface(&now);
            drop(runner);
            let mut instructions = String::new();
            if briefed {
                instructions.push_str(
                    "The owner left directives for this agent: call `briefing.read` \
                     FIRST, before any outbound action, and follow what it says.",
                );
            }
            if !surface.is_empty() {
                if !instructions.is_empty() {
                    instructions.push(' ');
                }
                instructions.push_str(&format!(
                    "Governed Ethos data is readable here — call `ethos.context` for \
                     the map (zones by context: {}).",
                    ethos_coverage(&surface)
                ));
            }
            if !instructions.is_empty() {
                result["instructions"] = Value::String(instructions);
            }
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            })
        }
        "tools/list" => {
            // Names only, aggregated from the declared maps (read AND
            // write: refusals must name the tool precisely). Schemas are
            // not proxied in v1 — the open object is the honest minimum.
            // The NATIVE journal tools close the list with their REAL
            // schemas: the gateway serves them itself, so it pins what
            // it governs (the hub decision, applied at home first). The
            // briefing tool joins them only while there is something to
            // brief (conditional surface, lot K).
            let now = (rt.clock)();
            let runner = rt.runner.lock().await;
            let mut tools = runner.listed_tools();
            tools.extend(native_journal_tools());
            if runner.briefing_available() {
                tools.push(briefing_tool());
            }
            let surface = runner.ethos_surface(&now);
            if !surface.is_empty() {
                tools.extend(ethos_tools(&surface));
            }
            drop(runner);
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tools }
            })
        }
        other => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": METHOD_NOT_FOUND_CODE,
                "message": format!(
                    "aithos gateway: method `{other}` is not served by the multi-context router"
                ),
            }
        }),
    }
}

/// The routed `tools/call`: resolve → authorize on the context → log the
/// act there + the xref in the journal (log-before-relay) → relay to the
/// context's own upstream. Refusals follow §3bis.8: journal always, the
/// context too when the tool names one.
async fn tool_call_multi<U: Upstream>(rt: &McpRouter<U>, mut msg: Value) -> Value {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let now = (rt.clock)();

    // A call we cannot even name is refused — into the journal only
    // (no context is identifiable).
    let Some(tool) = msg
        .pointer("/params/name")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        let e = GatewayError::RequestRejected("tools/call without params.name".into());
        let mut runner = rt.runner.lock().await;
        runner.record_refusal(None, "<unnamed>", e.refusal_code(), &now);
        return error_response(id, &e);
    };

    // The arguments only ever enter the log as a hash.
    let args = msg
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let mut runner = rt.runner.lock().await;

    // The native journal tools are served HERE, never relayed (lot C2):
    // the journal bridge writes/reads the agent's own memory under its
    // pen, and the delegated trace (section.add / ethos.read) IS the
    // log-before-effect. Refusals follow §3bis.8 — no context is ever
    // identifiable for a native tool, so the journal alone records them.
    if tool == JOURNAL_WRITE || tool == JOURNAL_SEARCH {
        return match journal_dispatch(&mut runner, &tool, &args, &now) {
            Ok(text) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }
            }),
            Err(deny) => {
                runner.record_refusal(None, &tool, deny.refusal_code(), &now);
                error_response(id, &deny)
            }
        };
    }

    // The native briefing tool (lot K), served HERE, never relayed: the
    // owner's directives from the granted public+circle zones, every
    // served section a journalized read in ITS context's gamma. The
    // reads live in the context (that is the record the demo sells);
    // the refusals follow §3bis.8 journal-only, like every native tool.
    if tool == BRIEFING_READ {
        return match briefing_dispatch(&mut runner, &args, &now) {
            Ok(text) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }
            }),
            Err(deny) => {
                runner.record_refusal(None, &tool, deny.refusal_code(), &now);
                error_response(id, &deny)
            }
        };
    }

    // The native ethos tools (lot G6), served HERE, never relayed: the
    // mandate-derived reading surface. Refusals follow §3bis.8 — the
    // journal always, and the CONTEXT too when the call names one (an
    // ethos call is the one native surface whose target context is
    // identifiable).
    if tool == ETHOS_READ || tool == ETHOS_LIST || tool == ETHOS_CONTEXT {
        return match ethos_dispatch(&mut runner, &tool, &args, &now) {
            Ok(text) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }
            }),
            Err(deny) => {
                let named = args
                    .get("context")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                runner.record_refusal(named.as_deref(), &tool, deny.refusal_code(), &now);
                error_response(id, &deny)
            }
        };
    }

    // Default-deny across every context: unknown → journal refusal only.
    let Some(ctx) = runner.resolve(&tool).map(str::to_owned) else {
        let e = GatewayError::ToolNotMapped(tool.clone());
        runner.record_refusal(None, &tool, e.refusal_code(), &now);
        return error_response(id, &e);
    };

    if let Some(deny) = runner.manifest_drift_for(&tool) {
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }

    // The resolved context's mandate at T — a named tool is not yet an
    // authorised tool (writes live here to be refused precisely).
    if let Err(deny) = runner.authorize(&ctx, &tool, &now) {
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }

    // The owner-approved argument bounds (lot P): the mandate said WHAT
    // the agent may do, the bounds say ON WHAT. A violation refuses the
    // whole call before anything is logged as an act — no rewriting,
    // no vault wake-up, no upstream contact; the pedagogical refusal
    // (field, offending values, approved rule) is the teaching surface,
    // and it goes on the record verbatim (lot D): the auditor replays
    // the same lesson the agent was taught.
    if let Err(deny) = runner.check_bounds(&tool, &args) {
        runner.record_bound_refusal(Some(&ctx), &tool, &deny, &now);
        return error_response(id, &deny);
    }

    let relay = match runner.relay_target(&ctx, &tool) {
        Ok(relay) => relay,
        Err(deny) => {
            runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
            return error_response(id, &deny);
        }
    };

    // Log before relaying, twice (context act + journal xref): if either
    // append fails, the call does not happen.
    if let Err(e) = runner.record_act_with_xref(&ctx, &tool, &args, &now) {
        let deny = GatewayError::LogAppendRefused(e.to_string());
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }
    drop(runner);

    // Relay to THAT context's upstream. A missing upstream is a config
    // mismatch, surfaced as an upstream failure (fail closed).
    let Some(upstream) = rt.upstreams.get(&relay.server) else {
        let e = GatewayError::UpstreamFailed(format!("no upstream for route `{}`", relay.server));
        return error_response(id, &e);
    };
    if let Some(name) = msg.pointer_mut("/params/name") {
        *name = Value::String(relay.raw_tool);
    }
    match upstream.forward(msg).await {
        Ok(resp) if resp.get("error").is_none() => resp,
        Ok(resp) => {
            if let Err(drift) = refresh_server_manifest(rt, &relay.server).await {
                if matches!(drift, GatewayError::ManifestDrift { .. }) {
                    let mut runner = rt.runner.lock().await;
                    runner.record_refusal(Some(&ctx), &tool, drift.refusal_code(), &now);
                    return error_response(id, &drift);
                }
            }
            resp
        }
        // The brokered credential could not be resolved: the upstream
        // was never contacted (resolution precedes the request), so
        // this is a governed refusal, not an upstream failure — logged
        // as such, and no drift probe (it would only wake the vault
        // again for a route that is already closed).
        Err(deny @ GatewayError::CredentialUnavailable(_)) => {
            let mut runner = rt.runner.lock().await;
            runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
            error_response(id, &deny)
        }
        Err(error) => {
            if let Err(drift) = refresh_server_manifest(rt, &relay.server).await {
                if matches!(drift, GatewayError::ManifestDrift { .. }) {
                    let mut runner = rt.runner.lock().await;
                    runner.record_refusal(Some(&ctx), &tool, drift.refusal_code(), &now);
                    return error_response(id, &drift);
                }
            }
            error_response(id, &error)
        }
    }
}

/// Session-open drift control for every governed server. `tools/list`
/// served to the agent remains entirely local; this control plane call
/// only compares the upstream with owner-approved pins.
pub async fn verify_hub_upstreams<U: Upstream>(rt: &McpRouter<U>) -> Result<()> {
    let servers = rt.runner.lock().await.hub_servers();
    for server in servers {
        refresh_server_manifest(rt, &server).await?;
    }
    Ok(())
}

/// Refresh one server's control-plane observation. Exposed for the
/// explicit test seam and reused after an upstream tool error.
pub async fn refresh_server_manifest<U: Upstream>(rt: &McpRouter<U>, server: &str) -> Result<()> {
    let expected =
        rt.runner.lock().await.server_pins(server).ok_or_else(|| {
            GatewayError::ConfigRejected(format!("unknown hub server `{server}`"))
        })?;
    let upstream = rt.upstreams.get(server).ok_or_else(|| {
        GatewayError::UpstreamFailed(format!("no upstream for hub server `{server}`"))
    })?;
    let observed = discover_server(server, upstream).await?;
    let actual: BTreeMap<String, String> = observed
        .tools
        .into_iter()
        .map(|tool| (tool.name, tool.pin_sha256))
        .collect();
    let mut runner = rt.runner.lock().await;
    if actual == expected {
        runner.clear_manifest_drift(server);
        return Ok(());
    }
    let reason = "upstream tools/list differs from the owner-approved pin".to_owned();
    runner.mark_manifest_drift(server, reason.clone());
    Err(GatewayError::ManifestDrift {
        server: server.to_owned(),
        reason,
    })
}

// -------------------------------------------------- native journal tools

/// The native tools' MCP descriptors: REAL argument schemas, unknown
/// fields pinned closed — the surface mirrors the fail-closed parser.
fn native_journal_tools() -> Vec<Value> {
    vec![
        json!({
            "name": JOURNAL_WRITE,
            "description": "Consolidate one memory note into the agent's own journal: \
                            a sealed section under the memory pen, mandate-traced.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "text": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["text"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": JOURNAL_SEARCH,
            "description": "Recall memory notes from the journal's clear index \
                            (name/title/tags, newest first); every opened body is a \
                            journalized read.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "tag": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": SEARCH_LIMIT_MAX }
                },
                "additionalProperties": false
            }
        }),
    ]
}

/// The native briefing tool's MCP descriptor (lot K): REAL argument
/// schema, unknown fields pinned closed, and a description that tells
/// the agent to consult it BEFORE acting — the tool description and the
/// initialize instructions are the same recommendation, twice.
fn briefing_tool() -> Value {
    json!({
        "name": BRIEFING_READ,
        "description": "The owner's standing directives for this agent. \
                        Consult it FIRST, before acting — especially before \
                        any outbound action — and follow what it says. \
                        Directives are labeled by context and zone; every \
                        read is on the record.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "context": { "type": "string" }
            },
            "additionalProperties": false
        }
    })
}

/// The one-line coverage summary the informed-access surface shows
/// (decided 2026-07-16): deterministic, context by context, zones in
/// serving order — e.g. `ventes: public, circle`.
fn ethos_coverage(surface: &BTreeMap<String, Vec<String>>) -> String {
    surface
        .iter()
        .map(|(context, zones)| format!("{context}: {}", zones.join(", ")))
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// The native ethos tools' MCP descriptors (lot G6): REAL argument
/// schemas, unknown fields pinned closed, and descriptions that NAME
/// the zones served right now, context by context — recomputed on
/// every list, so a grant or a revocation rewrites the surface hot.
fn ethos_tools(surface: &BTreeMap<String, Vec<String>>) -> Vec<Value> {
    let coverage = ethos_coverage(surface);
    vec![
        json!({
            "name": ETHOS_READ,
            "description": format!(
                "Read one section body from the governed Ethos. Readable now — {coverage}. \
                 Every sealed read is on the record."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context": { "type": "string" },
                    "zone": { "type": "string" },
                    "path": { "type": "string" }
                },
                "required": ["zone", "path"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": ETHOS_LIST,
            "description": format!(
                "List the readable skeleton of the governed Ethos — titles, tags and \
                 paths, never a body. Readable now — {coverage}."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context": { "type": "string" }
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": ETHOS_CONTEXT,
            "description": format!(
                "The starting pack: the owner's directives, the readable open sections \
                 and the covered sealed index. Call it FIRST to get the map. \
                 Readable now — {coverage}."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context": { "type": "string" }
                },
                "additionalProperties": false
            }
        }),
    ]
}

/// Validate and run one native ethos call against the runner (lot G6).
/// Argument parsing is fail-closed — unknown fields and wrong types
/// refuse naming the field; the bridge then derives the authority from
/// the scanned chains and journalizes every sealed open.
fn ethos_dispatch(runner: &mut Runner, tool: &str, args: &Value, now: &str) -> Result<String> {
    let obj = args
        .as_object()
        .ok_or_else(|| bad_args(tool, "arguments must be an object"))?;
    let allowed: &[&str] = match tool {
        ETHOS_READ => &["context", "zone", "path"],
        _ => &["context"],
    };
    for key in obj.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(bad_args(tool, &format!("unknown field `{key}`")));
        }
    }
    let text_field = |name: &str| match obj.get(name) {
        None => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(Some(s.clone())),
        Some(Value::String(_)) => Err(bad_args(tool, &format!("`{name}` must not be empty"))),
        Some(_) => Err(bad_args(tool, &format!("`{name}` must be a string"))),
    };
    let context = text_field("context")?;
    let payload = match tool {
        ETHOS_READ => {
            let zone = text_field("zone")?.ok_or_else(|| bad_args(tool, "`zone` is required"))?;
            let path = text_field("path")?.ok_or_else(|| bad_args(tool, "`path` is required"))?;
            runner.ethos_read(context.as_deref(), &zone, &path, now)?
        }
        ETHOS_LIST => runner.ethos_list(context.as_deref(), now)?,
        ETHOS_CONTEXT => runner.ethos_context_pack(context.as_deref(), now)?,
        other => return Err(bad_args(other, "not a native ethos tool")),
    };
    serde_json::to_string(&payload).map_err(|e| GatewayError::BridgeFailed(e.to_string()))
}

/// Validate and run one `briefing.read` against the runner's contexts.
/// Argument parsing is fail-closed (unknown fields, wrong types refuse
/// naming the field); the bridges then enforce the pen and journalize
/// every served section.
fn briefing_dispatch(runner: &mut Runner, args: &Value, now: &str) -> Result<String> {
    let obj = args
        .as_object()
        .ok_or_else(|| bad_args(BRIEFING_READ, "arguments must be an object"))?;
    for key in obj.keys() {
        if key != "context" {
            return Err(bad_args(BRIEFING_READ, &format!("unknown field `{key}`")));
        }
    }
    let context = match obj.get("context") {
        None => None,
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.as_str()),
        Some(Value::String(_)) => {
            return Err(bad_args(BRIEFING_READ, "`context` must not be empty"))
        }
        Some(_) => return Err(bad_args(BRIEFING_READ, "`context` must be a string")),
    };
    let briefing = runner.briefing_read(context, now)?;
    serde_json::to_string(&briefing).map_err(|e| GatewayError::BridgeFailed(e.to_string()))
}

/// One rejected argument shape — the reason names the field precisely.
fn bad_args(tool: &str, reason: &str) -> GatewayError {
    GatewayError::RequestRejected(format!("{tool}: {reason}"))
}

/// Validate and run one native journal call against the runner's
/// journal bridge. Argument parsing is fail-closed (unknown fields,
/// wrong types, empty text all refuse); the bridge then enforces the
/// pen (chain, window, revocations, verb) and journalizes the effect.
fn journal_dispatch(runner: &mut Runner, tool: &str, args: &Value, now: &str) -> Result<String> {
    let obj = args
        .as_object()
        .ok_or_else(|| bad_args(tool, "arguments must be an object"))?;
    match tool {
        JOURNAL_WRITE => {
            for key in obj.keys() {
                if !["title", "text", "tags"].contains(&key.as_str()) {
                    return Err(bad_args(tool, &format!("unknown field `{key}`")));
                }
            }
            let text = match obj.get("text") {
                Some(Value::String(s)) if !s.trim().is_empty() => s.as_str(),
                Some(Value::String(_)) | None => {
                    return Err(bad_args(tool, "`text` is required and must not be empty"))
                }
                Some(_) => return Err(bad_args(tool, "`text` must be a string")),
            };
            let title = match obj.get("title") {
                None => "",
                Some(Value::String(s)) => s.as_str(),
                Some(_) => return Err(bad_args(tool, "`title` must be a string")),
            };
            let tags: Vec<String> = match obj.get("tags") {
                None => Vec::new(),
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|t| {
                        t.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| bad_args(tool, "`tags` must be an array of strings"))
                    })
                    .collect::<Result<_>>()?,
                Some(_) => return Err(bad_args(tool, "`tags` must be an array of strings")),
            };
            let note = runner.journal_write(title, &tags, text, now)?;
            serde_json::to_string(&json!({
                "recorded": {
                    "name": note.name,
                    "title": note.title,
                    "path": format!("memory/{}", note.name),
                }
            }))
            .map_err(|e| GatewayError::BridgeFailed(e.to_string()))
        }
        JOURNAL_SEARCH => {
            for key in obj.keys() {
                if !["query", "tag", "limit"].contains(&key.as_str()) {
                    return Err(bad_args(tool, &format!("unknown field `{key}`")));
                }
            }
            let text_field = |name: &str| match obj.get(name) {
                None => Ok(None),
                Some(Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(bad_args(tool, &format!("`{name}` must be a string"))),
            };
            let query = text_field("query")?;
            let tag = text_field("tag")?;
            let limit = match obj.get("limit") {
                None => SEARCH_LIMIT_DEFAULT,
                Some(v) => match v.as_u64() {
                    Some(n) if (1..=SEARCH_LIMIT_MAX).contains(&n) => n,
                    _ => {
                        return Err(bad_args(
                            tool,
                            &format!("`limit` must be an integer in 1..={SEARCH_LIMIT_MAX}"),
                        ))
                    }
                },
            };
            let hits =
                runner.journal_search(query.as_deref(), tag.as_deref(), limit as usize, now)?;
            serde_json::to_string(&json!({ "total": hits.len(), "hits": hits }))
                .map_err(|e| GatewayError::BridgeFailed(e.to_string()))
        }
        other => Err(bad_args(other, "not a native journal tool")),
    }
}
