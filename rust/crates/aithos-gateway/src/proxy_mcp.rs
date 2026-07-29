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

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock};

use axum::body::{Body, Bytes};
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::core_bridge::{Bridge, EntropySource, Runner};
use crate::credentials::{CredentialBroker, CredentialRef};
use crate::hub::discover_server;
use crate::policy::Policy;
use crate::upstream_oauth::{UpstreamOAuthClient, UpstreamOAuthRegistry};
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
pub const ETHOS_CREATE: &str = "ethos.create";
pub const ETHOS_EDIT: &str = "ethos.edit";
pub const ETHOS_DELETE: &str = "ethos.delete";
/// `journal.search` result cap: the default page, and the hard ceiling
/// a caller-supplied `limit` may not exceed (each opened hit is one
/// journalized read — the cap bounds the gamma cost of one recall).
const SEARCH_LIMIT_DEFAULT: u64 = 20;
const SEARCH_LIMIT_MAX: u64 = 100;

/// The MCP protocol revision the router's own `initialize` speaks.
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

/// Opt-in transport trace for diagnosing a remote MCP client without
/// exposing bearer tokens, request arguments, Ethos content or response
/// bodies. Disabled unless `AITHOS_MCP_TRACE` is present in the process
/// environment.
static MCP_TRACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Injected clock, RFC 3339 `Z` instants (the wire's instant format —
/// never epoch numbers). The binary passes system time; tests pass a
/// fixed T.
pub type Clock = Arc<dyn Fn() -> String + Send + Sync>;

/// Seam to the real MCP server: HTTP in production, in-process fake in
/// the acceptance tests (GATEWAY-BOOTSTRAP §8).
pub trait Upstream: Send + Sync + 'static {
    fn forward(&self, body: Value) -> impl std::future::Future<Output = Result<Value>> + Send;
}

trait ErasedUpstream: Send + Sync {
    fn forward_erased(
        &self,
        body: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + '_>>;
}

impl<T: Upstream> ErasedUpstream for T {
    fn forward_erased(
        &self,
        body: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + '_>> {
        Box::pin(self.forward(body))
    }
}

/// Cloneable type-erased upstream used only for connector bindings that are
/// activated after boot. Static configurations keep their generic map and
/// byte-identical path.
#[derive(Clone)]
pub struct DynamicUpstream(Arc<dyn ErasedUpstream>);

impl DynamicUpstream {
    pub fn new<T: Upstream>(upstream: T) -> Self {
        Self(Arc::new(upstream))
    }
}

impl Upstream for DynamicUpstream {
    fn forward(&self, body: Value) -> impl Future<Output = Result<Value>> + Send {
        self.0.forward_erased(body)
    }
}

pub type DynamicUpstreams = Arc<RwLock<BTreeMap<String, DynamicUpstream>>>;

pub fn empty_dynamic_upstreams() -> DynamicUpstreams {
    Arc::new(RwLock::new(BTreeMap::new()))
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
    /// Authorization-code + PKCE custody. The client resolves/refreshes
    /// its Vault token set per call and refuses before wire I/O on failure.
    OAuth(Arc<UpstreamOAuthClient>),
}

/// Production upstream: JSON-RPC over Streamable HTTP (MCP 2025-03-26).
/// One POST per call; the response body is read whether the server
/// answers `application/json` or `text/event-stream` (SSE) — the modern
/// default. A `Mcp-Session-Id` handed back by the server is captured and
/// replayed on later calls (many streamable servers require it), and a
/// server that insists on an `initialize` handshake first gets one,
/// lazily, on the retry path.
pub struct HttpUpstream {
    client: reqwest::Client,
    url: String,
    auth: UpstreamAuth,
    /// The server-assigned session id, once seen (interior mutability:
    /// the trait takes `&self` and the value is shared behind an `Arc`).
    session: StdMutex<Option<String>>,
}

impl HttpUpstream {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
            auth: UpstreamAuth::None,
            session: StdMutex::new(None),
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
            session: StdMutex::new(None),
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
            session: StdMutex::new(None),
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
        if server.oauth.is_some() {
            return Err(GatewayError::ConfigRejected(format!(
                "servers[{}].oauth requires the OAuth runtime registry",
                server.name
            )));
        }
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

    pub fn for_server_with_oauth(
        server: &crate::config::ServerConfig,
        brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
        oauth: &UpstreamOAuthRegistry,
    ) -> Result<Self> {
        if server.oauth.is_some() {
            let client = oauth.get(&server.name).ok_or_else(|| {
                GatewayError::ConfigRejected(format!(
                    "servers[{}].oauth has no runtime client",
                    server.name
                ))
            })?;
            return Ok(Self {
                client: reqwest::Client::new(),
                url: server.url.clone(),
                auth: UpstreamAuth::OAuth(client),
                session: StdMutex::new(None),
            });
        }
        Self::for_server(server, brokers)
    }

    pub fn with_oauth_client(url: String, client: Arc<UpstreamOAuthClient>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            auth: UpstreamAuth::OAuth(client),
            session: StdMutex::new(None),
        }
    }
}

impl HttpUpstream {
    /// Resolve auth (per call, last moment — a broker outage refuses
    /// BEFORE the request leaves, never a half-credentialed call) and
    /// attach it. The secret drops as soon as the header is built.
    async fn authorize(
        &self,
        mut request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder> {
        match &self.auth {
            UpstreamAuth::None => {}
            UpstreamAuth::InlineBearer(token) => request = request.bearer_auth(token),
            UpstreamAuth::Brokered { broker, reference } => {
                let secret = broker.resolve(reference).await?;
                request = request.bearer_auth(secret.expose());
            }
            UpstreamAuth::OAuth(client) => {
                let access = client.access_token().await?;
                request = request.bearer_auth(access.expose());
            }
        }
        Ok(request)
    }

    /// One request/response round trip. Announces both content types
    /// (json + SSE — the streamable default), replays a known session
    /// id, captures any new one, and decodes the body by its
    /// Content-Type: plain JSON, or the JSON-RPC message carried in an
    /// SSE frame.
    async fn round_trip(&self, body: &Value) -> Result<Value> {
        let want_id = body.get("id").cloned().unwrap_or(Value::Null);
        let mut request = self
            .client
            .post(&self.url)
            .json(body)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", MCP_PROTOCOL_VERSION);
        if let Some(sid) = self.session.lock().expect("session lock").clone() {
            request = request.header(MCP_SESSION_HEADER, sid);
        }
        request = self.authorize(request).await?;
        let resp = request
            .send()
            .await
            .map_err(|e| GatewayError::UpstreamFailed(e.to_string()))?;
        // Capture a server-assigned session id for the next call.
        if let Some(sid) = resp
            .headers()
            .get(MCP_SESSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
        {
            *self.session.lock().expect("session lock") = Some(sid);
        }
        let is_sse = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.to_ascii_lowercase().contains("text/event-stream"));
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| GatewayError::UpstreamFailed(e.to_string()))?;
        extract_jsonrpc(&bytes, is_sse, &want_id).ok_or_else(|| {
            GatewayError::UpstreamFailed("upstream response was neither valid JSON nor SSE".into())
        })
    }

    /// The `initialize` handshake, done ONCE for a server that demands a
    /// session before serving anything. Captures the session id (via
    /// `round_trip`) and posts the `notifications/initialized` follow-up
    /// the spec mandates. Best-effort: a server that ignores it loses
    /// nothing.
    async fn initialize_session(&self) -> Result<()> {
        let init = json!({
            "jsonrpc": "2.0",
            "id": "aithos-initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "aithos-gateway", "version": "1" }
            }
        });
        self.round_trip(&init).await?;
        // Fire-and-forget the initialized notification; a transport
        // failure here must not mask a working session.
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        let mut request = self
            .client
            .post(&self.url)
            .json(&note)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", MCP_PROTOCOL_VERSION);
        if let Some(sid) = self.session.lock().expect("session lock").clone() {
            request = request.header(MCP_SESSION_HEADER, sid);
        }
        if let Ok(request) = self.authorize(request).await {
            let _ = request.send().await;
        }
        Ok(())
    }
}

/// Does this JSON-RPC error mean « you must `initialize` first »? Kept
/// deliberately narrow (an explicit signal, never a guess) so it can
/// only fire for a server that truly gates on a session — the in-process
/// fakes and well-behaved servers never trip it.
fn demands_initialize(response: &Value) -> bool {
    let Some(error) = response.get("error") else {
        return false;
    };
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    message.contains("initialize")
        || message.contains("not initialized")
        || message.contains("session")
}

/// Extract the JSON-RPC response from a Streamable-HTTP body — plain
/// JSON, or the message carried in an SSE (`text/event-stream`) frame.
/// SSE framing (WHATWG): events separated by a blank line; `data:` lines
/// accumulate (joined by `\n`, one leading space stripped); `:` lines are
/// comments (keepalives). We collect every `data` payload, parse each as
/// JSON, and return the one matching our request id — or, failing that,
/// the first payload carrying `result` or `error`.
fn extract_jsonrpc(body: &[u8], is_sse: bool, want_id: &Value) -> Option<Value> {
    if !is_sse {
        return serde_json::from_slice::<Value>(body).ok();
    }
    let text = String::from_utf8_lossy(body);
    let mut payloads: Vec<String> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            if !cur.is_empty() {
                payloads.push(cur.join("\n"));
                cur.clear();
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        if field == "data" {
            cur.push(value.to_owned());
        }
    }
    if !cur.is_empty() {
        payloads.push(cur.join("\n"));
    }
    let parsed: Vec<Value> = payloads
        .iter()
        .filter_map(|p| serde_json::from_str::<Value>(p).ok())
        .collect();
    if let Some(hit) = parsed.iter().find(|m| m.get("id") == Some(want_id)) {
        return Some(hit.clone());
    }
    parsed
        .into_iter()
        .find(|m| m.get("result").is_some() || m.get("error").is_some())
}

impl Upstream for HttpUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        let response = self.round_trip(&body).await?;
        // A server that gates on a session says so explicitly: do the
        // handshake once, then retry the original call.
        if demands_initialize(&response) {
            self.initialize_session().await?;
            return self.round_trip(&body).await;
        }
        Ok(response)
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
    /// Exact-name native Ethos execution seam. Generic connectors never use
    /// it, even when their name contains the word `ethos`.
    pub ethos_backend: Arc<crate::ethos_backend::EthosBackend>,
    /// Hot connector targets. The control plane swaps complete entries;
    /// readers clone one handle and release the lock before network I/O.
    pub dynamic_upstreams: DynamicUpstreams,
    pub clock: Clock,
    pub session_entropy: std::sync::Mutex<Box<dyn EntropySource + Send>>,
    /// The embedded OAuth authorization server (lot G3), when the `as:`
    /// stanza is active. `None` = byte-identical legacy behaviour: `/mcp`
    /// stays open on loopback, the AS endpoints do not exist. `Some` =
    /// `/mcp` requires a valid bearer (401 + `WWW-Authenticate` pointing
    /// the resource metadata otherwise) and the AS rides this listener.
    pub oauth: Option<Arc<crate::oauth::AuthServer>>,
    /// Exact browser origins shared with the signed control plane. Empty
    /// preserves the historical non-browser/loopback transport behaviour.
    pub browser_origins: Arc<BTreeSet<String>>,
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
        .route("/mcp", post(handle_multi::<U>).options(cors_preflight_sink))
        .route_layer(middleware::from_fn_with_state(
            Arc::clone(&rt),
            browser_cors::<U>,
        ))
        .with_state(rt)
}

async fn cors_preflight_sink() -> StatusCode {
    StatusCode::NO_CONTENT
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
    let trace = std::env::var_os("AITHOS_MCP_TRACE").is_some();
    let trace_sequence = trace.then(|| MCP_TRACE_SEQUENCE.fetch_add(1, Ordering::Relaxed));
    let trace_started = std::time::Instant::now();
    if let Some(sequence) = trace_sequence {
        eprintln!(
            "[mcp-trace] request={sequence} stage=received bytes={} authorization={} session_header={}",
            body.len(),
            headers.contains_key(header::AUTHORIZATION),
            headers.contains_key(MCP_SESSION_HEADER)
        );
    }
    if !origin_is_allowed(&headers, &rt.browser_origins) {
        if let Some(sequence) = trace_sequence {
            eprintln!(
                "[mcp-trace] request={sequence} stage=rejected reason=origin elapsed_ms={}",
                trace_started.elapsed().as_millis()
            );
        }
        return StatusCode::FORBIDDEN.into_response();
    }
    // The bearer gate (lot G3): only when the `as:` stanza is active.
    // Absent, this whole block vanishes and `/mcp` stays byte-identical.
    // Order is deliberate — Origin (above) outranks a missing token, and
    // a valid token only grants ENTRY: the mandate chain still decides
    // every act behind it (a token is never an authority).
    let delegated_session = if let Some(oauth) = &rt.oauth {
        let now = (rt.clock)();
        let presented = bearer_token(&headers);
        let Some(token) = presented.as_deref() else {
            if let Some(sequence) = trace_sequence {
                eprintln!(
                    "[mcp-trace] request={sequence} stage=rejected reason=missing_bearer elapsed_ms={}",
                    trace_started.elapsed().as_millis()
                );
            }
            let mut resp = StatusCode::UNAUTHORIZED.into_response();
            if let Ok(v) = HeaderValue::from_str(&oauth.www_authenticate(false)) {
                resp.headers_mut().insert(header::WWW_AUTHENTICATE, v);
            }
            return resp;
        };
        match oauth.validate_bearer(token, &now) {
            Ok(Some(session)) => {
                let mut runner = rt.runner.lock().await;
                if runner
                    .validate_bearer_session(
                        &session.context,
                        &session.leaf_id,
                        &session.session_pub,
                        &session.leaf,
                        &now,
                    )
                    .is_err()
                {
                    let deny = GatewayError::MandateDenied {
                        op: "delegated_session".to_owned(),
                        reason: "the durable delegated authority is unavailable".to_owned(),
                    };
                    runner.record_refusal(
                        Some(&session.context),
                        "<session>",
                        deny.refusal_code(),
                        &now,
                    );
                    drop(runner);
                    if let Some(sequence) = trace_sequence {
                        eprintln!(
                            "[mcp-trace] request={sequence} stage=rejected reason=delegated_authority elapsed_ms={}",
                            trace_started.elapsed().as_millis()
                        );
                    }
                    let mut resp = StatusCode::UNAUTHORIZED.into_response();
                    if let Ok(v) = HeaderValue::from_str(&oauth.www_authenticate(true)) {
                        resp.headers_mut().insert(header::WWW_AUTHENTICATE, v);
                    }
                    return resp;
                }
                drop(runner);
                Some(session)
            }
            Ok(None) => None,
            Err(_) => {
                if let Some(sequence) = trace_sequence {
                    eprintln!(
                        "[mcp-trace] request={sequence} stage=rejected reason=invalid_bearer elapsed_ms={}",
                        trace_started.elapsed().as_millis()
                    );
                }
                let mut resp = StatusCode::UNAUTHORIZED.into_response();
                if let Ok(v) = HeaderValue::from_str(&oauth.www_authenticate(true)) {
                    resp.headers_mut().insert(header::WWW_AUTHENTICATE, v);
                }
                return resp;
            }
        }
    } else {
        None
    };
    if let Some(sequence) = trace_sequence {
        eprintln!(
            "[mcp-trace] request={sequence} stage=authorized delegated={} elapsed_ms={}",
            delegated_session.is_some(),
            trace_started.elapsed().as_millis()
        );
    }
    let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
        if let Some(sequence) = trace_sequence {
            eprintln!(
                "[mcp-trace] request={sequence} stage=rejected reason=parse elapsed_ms={}",
                trace_started.elapsed().as_millis()
            );
        }
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
        if let Some(sequence) = trace_sequence {
            eprintln!(
                "[mcp-trace] request={sequence} stage=rejected reason=batch elapsed_ms={}",
                trace_started.elapsed().as_millis()
            );
        }
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
            if let Some(sequence) = trace_sequence {
                eprintln!(
                    "[mcp-trace] request={sequence} stage=complete kind=notification status=202 elapsed_ms={}",
                    trace_started.elapsed().as_millis()
                );
            }
            return StatusCode::ACCEPTED.into_response();
        }
        if let Some(sequence) = trace_sequence {
            eprintln!(
                "[mcp-trace] request={sequence} stage=rejected reason=missing_id elapsed_ms={}",
                trace_started.elapsed().as_millis()
            );
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
    let method = msg.get("method").and_then(Value::as_str);
    let method_label = match method {
        Some("initialize") => "initialize",
        Some("ping") => "ping",
        Some("tools/list") => "tools/list",
        Some("tools/call") => "tools/call",
        _ => "other",
    };
    let tool_label = match msg.pointer("/params/name").and_then(Value::as_str) {
        Some(ETHOS_CONTEXT) => ETHOS_CONTEXT,
        Some(ETHOS_LIST) => ETHOS_LIST,
        Some(ETHOS_READ) => ETHOS_READ,
        Some(ETHOS_CREATE) => ETHOS_CREATE,
        Some(ETHOS_EDIT) => ETHOS_EDIT,
        Some(ETHOS_DELETE) => ETHOS_DELETE,
        Some(JOURNAL_WRITE) => JOURNAL_WRITE,
        Some(JOURNAL_SEARCH) => JOURNAL_SEARCH,
        Some(BRIEFING_READ) => BRIEFING_READ,
        Some(_) => "other",
        None => "none",
    };
    if let Some(sequence) = trace_sequence {
        eprintln!(
            "[mcp-trace] request={sequence} stage=dispatch method={method_label} tool={tool_label} elapsed_ms={}",
            trace_started.elapsed().as_millis()
        );
    }
    let is_initialize = method == Some("initialize");
    let presented = headers.get(MCP_SESSION_HEADER).cloned();
    let result = process_multi_as(&rt, msg, delegated_session.as_ref()).await;
    if let Some(sequence) = trace_sequence {
        let outcome = if result.get("error").is_some() {
            "rpc_error"
        } else {
            "ok"
        };
        let response_bytes = serde_json::to_vec(&result).map_or(0, |encoded| encoded.len());
        eprintln!(
            "[mcp-trace] request={sequence} stage=processed outcome={outcome} response_bytes={response_bytes} elapsed_ms={}",
            trace_started.elapsed().as_millis()
        );
    }
    let mut resp = Json(result).into_response();
    if is_initialize {
        if let Ok(v) = HeaderValue::from_str(&rt.mint_session_id()) {
            resp.headers_mut().insert(MCP_SESSION_HEADER, v);
        }
    } else if let Some(v) = presented {
        resp.headers_mut().insert(MCP_SESSION_HEADER, v);
    }
    if let Some(sequence) = trace_sequence {
        eprintln!(
            "[mcp-trace] request={sequence} stage=response_ready status={} elapsed_ms={}",
            resp.status().as_u16(),
            trace_started.elapsed().as_millis()
        );
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
fn origin_is_allowed(headers: &HeaderMap, configured: &BTreeSet<String>) -> bool {
    if !headers.contains_key(header::ORIGIN) {
        return true;
    }
    let Some(origin) = one_header(headers, header::ORIGIN) else {
        return false;
    };
    if !configured.is_empty() {
        return configured.contains(origin);
    }
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
        .route("/ceremony/prepare", post(oauth_ceremony_prepare::<U>))
        .route(
            "/ceremony/prepare-grant",
            post(oauth_ceremony_prepare_grant::<U>),
        )
        .route("/ceremony/complete", post(oauth_ceremony_complete::<U>))
        .route("/ceremony/cancel", post(oauth_ceremony_cancel::<U>))
        .route("/ceremony/enroll", axum::routing::get(ceremony_enroll))
        .route(
            "/ceremony/enroll.js",
            axum::routing::get(ceremony_enroll_app),
        )
        .route("/ceremony/app.js", axum::routing::get(ceremony_app))
        .route(
            "/ceremony/aithos_wasm.js",
            axum::routing::get(ceremony_wasm_js),
        )
        .route(
            "/ceremony/aithos_wasm_bg.wasm",
            axum::routing::get(ceremony_wasm_binary),
        )
        .route("/ceremony/style.css", axum::routing::get(ceremony_style))
        .route("/token", post(oauth_token::<U>))
        .route_layer(middleware::from_fn_with_state(
            Arc::clone(&rt),
            browser_cors::<U>,
        ))
        .with_state(rt)
}

#[derive(Clone, Copy)]
struct BrowserCorsRoute {
    methods: &'static [&'static str],
    headers: &'static [&'static str],
    expose: Option<&'static str>,
}

fn browser_cors_route(path: &str) -> Option<BrowserCorsRoute> {
    const JSON_HEADERS: &[&str] = &["accept", "content-type"];
    const MCP_HEADERS: &[&str] = &[
        "accept",
        "authorization",
        "content-type",
        "mcp-protocol-version",
        "mcp-session-id",
    ];
    match path {
        "/mcp" => Some(BrowserCorsRoute {
            methods: &["POST"],
            headers: MCP_HEADERS,
            expose: Some("MCP-Session-Id, WWW-Authenticate"),
        }),
        "/.well-known/oauth-protected-resource" | "/.well-known/oauth-authorization-server" => {
            Some(BrowserCorsRoute {
                methods: &["GET"],
                headers: &["accept"],
                expose: None,
            })
        }
        "/register"
        | "/ceremony/prepare"
        | "/ceremony/prepare-grant"
        | "/ceremony/complete"
        | "/ceremony/cancel"
        | "/token" => Some(BrowserCorsRoute {
            methods: &["POST"],
            headers: JSON_HEADERS,
            expose: None,
        }),
        "/authorize" => Some(BrowserCorsRoute {
            methods: &["GET", "POST"],
            headers: JSON_HEADERS,
            expose: None,
        }),
        _ => None,
    }
}

fn one_header(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

fn requested_headers(headers: &HeaderMap) -> Option<BTreeSet<String>> {
    let Some(raw) = one_header(headers, header::ACCESS_CONTROL_REQUEST_HEADERS) else {
        return Some(BTreeSet::new());
    };
    let mut parsed = BTreeSet::new();
    for item in raw.split(',') {
        let item = item.trim();
        if item.is_empty()
            || item.len() > 64
            || !item
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || !parsed.insert(item.to_ascii_lowercase())
        {
            return None;
        }
    }
    Some(parsed)
}

fn add_browser_cors(
    mut response: Response,
    origin: &str,
    expose: Option<&'static str>,
) -> Response {
    if let Ok(origin) = HeaderValue::from_str(origin) {
        response
            .headers_mut()
            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    response
        .headers_mut()
        .append(header::VARY, HeaderValue::from_static("Origin"));
    if let Some(expose) = expose {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(expose),
        );
    }
    response
}

async fn browser_cors<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(route) = browser_cors_route(request.uri().path()) else {
        return next.run(request).await;
    };
    if !request.headers().contains_key(header::ORIGIN) {
        return next.run(request).await;
    }
    let Some(origin) = one_header(request.headers(), header::ORIGIN).map(str::to_owned) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if rt.browser_origins.is_empty() {
        return if request.method() == Method::OPTIONS {
            StatusCode::METHOD_NOT_ALLOWED.into_response()
        } else {
            next.run(request).await
        };
    }
    if !rt.browser_origins.contains(&origin) {
        let mut response = StatusCode::FORBIDDEN.into_response();
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Origin"));
        return response;
    }
    if request.method() == Method::OPTIONS {
        let Some(method) = one_header(request.headers(), header::ACCESS_CONTROL_REQUEST_METHOD)
        else {
            return StatusCode::FORBIDDEN.into_response();
        };
        let Some(headers) = requested_headers(request.headers()) else {
            return StatusCode::FORBIDDEN.into_response();
        };
        if !route.methods.contains(&method)
            || headers
                .iter()
                .any(|requested| !route.headers.contains(&requested.as_str()))
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        let mut response = StatusCode::NO_CONTENT.into_response();
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_str(method).expect("validated HTTP method"),
        );
        if !headers.is_empty() {
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_str(&headers.into_iter().collect::<Vec<_>>().join(", "))
                    .expect("validated header names"),
            );
        }
        response.headers_mut().insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
        return add_browser_cors(response, &origin, route.expose);
    }
    let response = next.run(request).await;
    add_browser_cors(response, &origin, route.expose)
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
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<BTreeMap<String, String>>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = (rt.clock)();
    match oauth.authorize_at(&authorize_request(&params), &now) {
        crate::oauth::AuthorizeOutcome::Consent { html } => {
            axum::response::Html(html).into_response()
        }
        crate::oauth::AuthorizeOutcome::Ceremony { html, pending } => {
            if accepts_json(&headers) {
                ceremony_json(json!({ "v": 1, "ceremony": pending }))
            } else {
                ceremony_html(html)
            }
        }
        crate::oauth::AuthorizeOutcome::Redirect { location } => redirect_to(&location),
        crate::oauth::AuthorizeOutcome::HardError { detail } => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_request", "error_description": detail })),
        )
            .into_response(),
    }
}

fn accepts_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .filter_map(|item| item.split(';').next())
                .any(|item| item.trim().eq_ignore_ascii_case("application/json"))
        })
}

fn ceremony_response(content_type: &'static str, body: Body) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .header(
            "content-security-policy",
            "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        )
        .body(body)
        .expect("static ceremony response headers are valid")
}

fn ceremony_html(html: String) -> Response {
    ceremony_response("text/html; charset=utf-8", Body::from(html))
}

fn ceremony_json(document: Value) -> Response {
    let mut response = Json(document).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn ceremony_app() -> Response {
    ceremony_response(
        "text/javascript; charset=utf-8",
        Body::from(include_str!("../assets/ceremony/app.js")),
    )
}

async fn ceremony_enroll() -> Response {
    ceremony_response(
        "text/html; charset=utf-8",
        Body::from(include_str!("../assets/ceremony/enroll.html")),
    )
}

async fn ceremony_enroll_app() -> Response {
    ceremony_response(
        "text/javascript; charset=utf-8",
        Body::from(include_str!("../assets/ceremony/enroll.js")),
    )
}

async fn ceremony_wasm_js() -> Response {
    ceremony_response(
        "text/javascript; charset=utf-8",
        Body::from(include_str!("../assets/ceremony/aithos_wasm.js")),
    )
}

async fn ceremony_wasm_binary() -> Response {
    ceremony_response(
        "application/wasm",
        Body::from(&include_bytes!("../assets/ceremony/aithos_wasm_bg.wasm")[..]),
    )
}

async fn ceremony_style() -> Response {
    ceremony_response(
        "text/css; charset=utf-8",
        Body::from(include_str!("../assets/ceremony/style.css")),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CeremonyPrepareRequest {
    transaction_id: String,
    delegate_pub: String,
}

async fn oauth_ceremony_prepare<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    Json(request): Json<CeremonyPrepareRequest>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = (rt.clock)();
    let preparation =
        match oauth.prepare_ceremony(&request.transaction_id, &request.delegate_pub, &now) {
            Ok(preparation) => preparation,
            Err(error) => return oauth_error_response(&error),
        };
    let eligible_parents = rt.runner.lock().await.eligible_session_parents(
        &request.delegate_pub,
        &preparation.resource,
        &now,
    );
    Json(json!({
        "v": 1,
        "verified_at": now,
        "bindings": preparation,
        "eligible_parents": eligible_parents,
    }))
    .into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CeremonyPrepareGrantRequest {
    transaction_id: String,
    delegate_pub: String,
    context: String,
    parent_id: String,
    leaf: Value,
}

async fn oauth_ceremony_prepare_grant<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    Json(request): Json<CeremonyPrepareGrantRequest>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = (rt.clock)();
    let preparation =
        match oauth.prepare_ceremony(&request.transaction_id, &request.delegate_pub, &now) {
            Ok(preparation) => preparation,
            Err(error) => return oauth_error_response(&error),
        };
    let grant = rt.runner.lock().await.prepare_session_grant(
        &request.context,
        &request.parent_id,
        &preparation.delegate_pub,
        &preparation.gateway_pub,
        &preparation.gateway_kex_pub,
        &preparation.session_pub,
        &preparation.resource,
        &request.leaf,
        &now,
    );
    match grant {
        Ok(grant) => Json(json!({ "v": 1, "grant": grant })).into_response(),
        Err(error) => oauth_error_response(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CeremonyCompleteRequest {
    transaction_id: String,
    context: String,
    parent_id: String,
    leaf: Value,
    grant: Value,
    proof: crate::oauth::CeremonyProof,
}

async fn oauth_ceremony_complete<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    headers: HeaderMap,
    Json(request): Json<CeremonyCompleteRequest>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = (rt.clock)();
    let reserved = match oauth.reserve_ceremony_completion(
        &request.transaction_id,
        &request.context,
        &request.parent_id,
        &request.leaf,
        &request.grant,
        &request.proof,
        &now,
    ) {
        Ok(reserved) => reserved,
        Err(error) => return oauth_error_response(&error),
    };
    let authority = rt.runner.lock().await.activate_session_leaf(
        &reserved.context,
        &reserved.parent_id,
        &reserved.delegate_pub,
        &reserved.gateway_pub,
        &reserved.gateway_kex_pub,
        &reserved.session_pub,
        &reserved.resource,
        &request.leaf,
        &request.grant,
        &now,
    );
    let authority = match authority {
        Ok(authority) => authority,
        Err(error) => {
            let _ = oauth.release_ceremony_reservation(&reserved);
            let _ = oauth.cancel_ceremony(&reserved.transaction_id);
            return oauth_error_response(&error);
        }
    };
    match oauth.finalize_ceremony(reserved, &authority, &now) {
        Ok(location)
            if headers
                .get(header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| {
                    value
                        .split(',')
                        .any(|item| item.trim() == "application/json")
                }) =>
        {
            Json(json!({ "redirect_to": location })).into_response()
        }
        Ok(location) => redirect_to(&location),
        Err(error) => oauth_error_response(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CeremonyCancelRequest {
    transaction_id: String,
}

async fn oauth_ceremony_cancel<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    Json(request): Json<CeremonyCancelRequest>,
) -> Response {
    let Some(oauth) = &rt.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match oauth.cancel_ceremony(&request.transaction_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => oauth_error_response(&error),
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
    process_multi_as(rt, msg, None).await
}

async fn process_multi_as<U: Upstream>(
    rt: &McpRouter<U>,
    msg: Value,
    session: Option<&crate::oauth::BearerSession>,
) -> Value {
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let id = msg.get("id").cloned().unwrap_or(Value::Null);

    match method.as_str() {
        "tools/call" => tool_call_multi(rt, msg, session).await,
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
            let mut instructions = String::new();
            let now = (rt.clock)();
            let runner = rt.runner.lock().await;
            let (briefed, surface) = match session {
                // The delegated session's instructions mirror ITS OWN
                // surface (lot 1): briefing only when the pen exists AND
                // the session chain covers the tool; ethos zones only
                // when the session chain covers them. A verification
                // failure here is a mute surface, not an initialize
                // error — the bearer gate already ran.
                Some(session) => {
                    let briefed = runner.briefing_available_for(&session.context)
                        && runner
                            .session_covers_tool(
                                &session.context,
                                &session.leaf_id,
                                &session.session_pub,
                                &session.leaf,
                                BRIEFING_READ,
                                &now,
                            )
                            .unwrap_or(false);
                    let zones = runner
                        .ethos_surface_for_session(
                            &session.context,
                            &session.leaf_id,
                            &session.session_pub,
                            &session.leaf,
                            &now,
                        )
                        .unwrap_or_default();
                    let surface = if zones.is_empty() {
                        BTreeMap::new()
                    } else {
                        BTreeMap::from([(session.context.clone(), zones)])
                    };
                    (briefed, surface)
                }
                None => (runner.briefing_available(), runner.ethos_surface(&now)),
            };
            drop(runner);
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
            let tools = if let Some(session) = session {
                let mut tools = match runner.listed_tools_for_session(
                    &session.context,
                    &session.leaf_id,
                    &session.session_pub,
                    &session.leaf,
                    &now,
                ) {
                    Ok(tools) => tools,
                    Err(error) => return error_response(id, &error),
                };
                if runner.briefing_available_for(&session.context) {
                    match runner.session_covers_tool(
                        &session.context,
                        &session.leaf_id,
                        &session.session_pub,
                        &session.leaf,
                        BRIEFING_READ,
                        &now,
                    ) {
                        Ok(true) => tools.push(briefing_tool()),
                        Ok(false) => {}
                        Err(error) => return error_response(id, &error),
                    }
                }
                // The delegated ethos surface (lot 1): the native read
                // tools join the list only when the SESSION chain covers
                // at least one zone of ITS context — recomputed on every
                // list, so a revocation drops them hot, no restart.
                match runner.ethos_surface_for_session(
                    &session.context,
                    &session.leaf_id,
                    &session.session_pub,
                    &session.leaf,
                    &now,
                ) {
                    Ok(zones) if !zones.is_empty() => {
                        let surface = BTreeMap::from([(session.context.clone(), zones)]);
                        tools.extend(ethos_tools(&surface));
                    }
                    Ok(_) => {}
                    Err(error) => return error_response(id, &error),
                }
                // The delegated write tools (lot 4): each verb lights its
                // own tool, exactly as covered — create/edit on `append`,
                // delete on `delete`. Circle only this pass.
                match runner.ethos_write_surface_for_session(
                    &session.context,
                    &session.leaf_id,
                    &session.session_pub,
                    &session.leaf,
                    &now,
                ) {
                    Ok((append_covered, delete_covered)) => {
                        tools.extend(ethos_write_tools(append_covered, delete_covered));
                    }
                    Err(error) => return error_response(id, &error),
                }
                tools
            } else {
                let mut tools = runner.listed_tools();
                tools.extend(native_journal_tools());
                if runner.briefing_available() {
                    tools.push(briefing_tool());
                }
                let surface = runner.ethos_surface(&now);
                if !surface.is_empty() {
                    tools.extend(ethos_tools(&surface));
                }
                tools
            };
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
async fn tool_call_multi<U: Upstream>(
    rt: &McpRouter<U>,
    msg: Value,
    session: Option<&crate::oauth::BearerSession>,
) -> Value {
    if let Some(session) = session {
        return tool_call_delegated(rt, msg, session).await;
    }
    tool_call_legacy(rt, msg).await
}

async fn tool_call_delegated<U: Upstream>(
    rt: &McpRouter<U>,
    mut msg: Value,
    session: &crate::oauth::BearerSession,
) -> Value {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let now = (rt.clock)();
    let Some(tool) = msg
        .pointer("/params/name")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        let deny = GatewayError::RequestRejected("tools/call without params.name".into());
        let mut runner = rt.runner.lock().await;
        runner.record_refusal(None, "<unnamed>", deny.refusal_code(), &now);
        return error_response(id, &deny);
    };
    let mut args = msg
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    // A production bearer names a durable delegated session, but carries no
    // authority by itself. Re-resolve the tool against the live runner, build
    // an operation from the current chain and gamma head, have the embedded
    // session key co-sign it, then ask Core to verify the whole chain and both
    // proofs. Nothing below this guard contacts an upstream or its credentials.
    let mut runner = rt.runner.lock().await;
    let native_briefing = tool == BRIEFING_READ;
    let native_ethos = crate::ethos_backend::EthosBackend::handles(&tool);
    // Every native tool naming a context is pinned to the session's own
    // context BEFORE anything else: absent → injected, different →
    // refused. Visibility is not authorization — the dispatchers below
    // re-verify the chain and the coverage of every single call.
    if native_briefing || native_ethos {
        let Some(arguments) = args.as_object_mut() else {
            let deny = GatewayError::RequestRejected(format!("{tool} arguments must be an object"));
            runner.record_refusal(Some(&session.context), &tool, deny.refusal_code(), &now);
            return error_response(id, &deny);
        };
        match arguments.get("context").and_then(Value::as_str) {
            Some(context) if context != session.context => {
                let deny = GatewayError::MandateDenied {
                    op: "delegated_session".into(),
                    reason: format!(
                        "context `{context}` differs from delegated context `{}`",
                        session.context
                    ),
                };
                runner.record_refusal(Some(context), &tool, deny.refusal_code(), &now);
                return error_response(id, &deny);
            }
            Some(_) => {}
            None => {
                arguments.insert("context".into(), Value::String(session.context.clone()));
            }
        }
    }
    if native_ethos {
        let prepared = match rt.ethos_backend.prepare_delegated_mutation(
            &mut runner,
            session,
            &tool,
            &args,
            &now,
        ) {
            Ok(prepared) => prepared,
            Err(deny) => {
                runner.record_refusal(Some(&session.context), &tool, deny.refusal_code(), &now);
                return error_response(id, &deny);
            }
        };
        if let Some(prepared) = prepared {
            // The Runner guards keys, chains and mutable planning entropy. The
            // resulting requests are already signed and closed, so network I/O
            // must not serialize unrelated connectors behind this mutex.
            drop(runner);
            return match prepared.execute().await {
                Ok(text) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": text }],
                        "isError": false
                    }
                }),
                Err(deny) => {
                    let mut runner = rt.runner.lock().await;
                    runner.record_refusal(Some(&session.context), &tool, deny.refusal_code(), &now);
                    error_response(id, &deny)
                }
            };
        }
        return match rt
            .ethos_backend
            .dispatch_delegated(&mut runner, session, &tool, &args, &now)
        {
            Ok(text) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }
            }),
            Err(deny) => {
                runner.record_refusal(Some(&session.context), &tool, deny.refusal_code(), &now);
                error_response(id, &deny)
            }
        };
    }
    let ctx = if native_briefing || runner.tool_available_in_context(&session.context, &tool) {
        session.context.clone()
    } else if let Some(context) = runner.resolve(&tool).map(str::to_owned) {
        context
    } else {
        let deny = GatewayError::ToolNotMapped(tool.clone());
        runner.record_refusal(None, &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    };
    if ctx != session.context {
        let deny = GatewayError::MandateDenied {
            op: "delegated_session".into(),
            reason: format!(
                "tool `{tool}` belongs to context `{ctx}`, not delegated context `{}`",
                session.context
            ),
        };
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }
    if !native_briefing {
        if let Some(deny) = runner.manifest_drift_for(&tool) {
            runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
            return error_response(id, &deny);
        }
        if let Err(deny) = runner.check_bounds(&tool, &args) {
            runner.record_bound_refusal(Some(&ctx), &tool, &deny, &now);
            return error_response(id, &deny);
        }
    }
    let relay = if native_briefing {
        None
    } else {
        match runner.relay_target(&ctx, &tool) {
            Ok(relay) => Some(relay),
            Err(deny) => {
                runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
                return error_response(id, &deny);
            }
        }
    };
    let prepared = match runner.prepare_session_operation(
        &ctx,
        &session.leaf_id,
        &session.session_pub,
        &session.leaf,
        &session.certificate,
        &tool,
        &args,
        &now,
    ) {
        Ok(prepared) => prepared,
        Err(deny) => {
            runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
            return error_response(id, &deny);
        }
    };
    let Some(oauth) = rt.oauth.as_ref() else {
        let deny = GatewayError::RequestRejected(
            "delegated bearer reached a router without an authorization server".into(),
        );
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    };
    let session_proof = match oauth.sign_session_proof(session, &prepared.operation_ref, &now) {
        Ok(proof) => proof,
        Err(deny) => {
            runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
            return error_response(id, &deny);
        }
    };
    if let Err(deny) = crate::core_bridge::verify_delegated_chain_session(
        crate::core_bridge::DelegatedChainSessionEvidence {
            chain: &prepared.chain,
            did: &prepared.did,
            at: &now,
            revocations: &prepared.revocations,
            mandate: &prepared.mandate,
            certificate: &prepared.certificate,
            projection: &prepared.projection,
            operation_ref: &prepared.operation_ref,
            native_leaf_proof: &prepared.native_leaf_proof,
            session_proof: &session_proof,
        },
    ) {
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }
    if let Err(error) = runner.record_session_act_with_xref(
        &ctx,
        &tool,
        &args,
        &prepared.chain,
        &session.session_pub,
        &prepared.certificate_digest,
        &prepared.operation_ref,
        &now,
    ) {
        let deny = GatewayError::LogAppendRefused(error.to_string());
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }
    if native_briefing {
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
                runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
                error_response(id, &deny)
            }
        };
    }
    drop(runner);

    let relay = relay.expect("non-native delegated tools have a relay target");

    if let Some(name) = msg.pointer_mut("/params/name") {
        *name = Value::String(relay.raw_tool);
    }
    let forwarded = if relay.hot {
        let upstream = rt
            .dynamic_upstreams
            .read()
            .map_err(|_| {
                GatewayError::UpstreamFailed("dynamic upstream registry unavailable".into())
            })
            .and_then(|upstreams| {
                upstreams.get(&relay.server).cloned().ok_or_else(|| {
                    GatewayError::UpstreamFailed(format!(
                        "no active connector upstream for route `{}`",
                        relay.server
                    ))
                })
            });
        match upstream {
            Ok(upstream) => upstream.forward(msg).await,
            Err(error) => Err(error),
        }
    } else {
        match rt.upstreams.get(&relay.server) {
            Some(upstream) => upstream.forward(msg).await,
            None => Err(GatewayError::UpstreamFailed(format!(
                "no upstream for route `{}`",
                relay.server
            ))),
        }
    };
    match forwarded {
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
        Err(
            deny @ (GatewayError::CredentialUnavailable(_)
            | GatewayError::UpstreamOauthUnavailable(_)),
        ) => {
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

async fn tool_call_legacy<U: Upstream>(rt: &McpRouter<U>, mut msg: Value) -> Value {
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
    if crate::ethos_backend::EthosBackend::handles_legacy(&tool) {
        return match rt
            .ethos_backend
            .dispatch_legacy(&mut runner, &tool, &args, &now)
        {
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

    if let Some(name) = msg.pointer_mut("/params/name") {
        *name = Value::String(relay.raw_tool);
    }
    // The relay target records whether governance resolved a hot binding.
    // A hot route may never fall through to a same-name static template when
    // its dynamic entry is removed or the registry is unavailable.
    let forwarded = if relay.hot {
        let upstream = rt
            .dynamic_upstreams
            .read()
            .map_err(|_| {
                GatewayError::UpstreamFailed("dynamic upstream registry unavailable".into())
            })
            .and_then(|upstreams| {
                upstreams.get(&relay.server).cloned().ok_or_else(|| {
                    GatewayError::UpstreamFailed(format!(
                        "no active connector upstream for route `{}`",
                        relay.server
                    ))
                })
            });
        match upstream {
            Ok(upstream) => upstream.forward(msg).await,
            Err(error) => Err(error),
        }
    } else {
        match rt.upstreams.get(&relay.server) {
            Some(upstream) => upstream.forward(msg).await,
            None => Err(GatewayError::UpstreamFailed(format!(
                "no upstream for route `{}`",
                relay.server
            ))),
        }
    };
    match forwarded {
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
        Err(
            deny @ (GatewayError::CredentialUnavailable(_)
            | GatewayError::UpstreamOauthUnavailable(_)),
        ) => {
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
    verify_hub_upstreams_except(rt, &std::collections::BTreeSet::new()).await
}

/// OAuth servers may be intentionally disconnected while the callback is
/// being completed. Their data plane remains fail-closed; startup drift
/// verification is deferred until a token exists instead of preventing the
/// callback listener from coming up.
pub async fn verify_hub_upstreams_except<U: Upstream>(
    rt: &McpRouter<U>,
    deferred: &std::collections::BTreeSet<String>,
) -> Result<()> {
    let servers = rt.runner.lock().await.hub_servers();
    for server in servers {
        if deferred.contains(&server) {
            rt.runner.lock().await.mark_manifest_drift(
                &server,
                "OAuth connection is not yet verified; reconnect and restart the gateway".into(),
            );
            continue;
        }
        refresh_server_manifest(rt, &server).await?;
    }
    Ok(())
}

/// Refresh one server's control-plane observation. Exposed for the
/// explicit test seam and reused after an upstream tool error.
pub async fn refresh_server_manifest<U: Upstream>(rt: &McpRouter<U>, server: &str) -> Result<()> {
    let (expected, hot) = {
        let runner = rt.runner.lock().await;
        let expected = runner.server_pins(server).ok_or_else(|| {
            GatewayError::ConfigRejected(format!("unknown hub server `{server}`"))
        })?;
        (expected, runner.is_hot_server(server))
    };
    let observed = if hot {
        let upstream = rt
            .dynamic_upstreams
            .read()
            .map_err(|_| {
                GatewayError::UpstreamFailed("dynamic upstream registry unavailable".into())
            })?
            .get(server)
            .cloned()
            .ok_or_else(|| {
                GatewayError::UpstreamFailed(format!(
                    "no active connector upstream for hub server `{server}`"
                ))
            })?;
        discover_server(server, &upstream).await?
    } else {
        let upstream = rt.upstreams.get(server).ok_or_else(|| {
            GatewayError::UpstreamFailed(format!("no upstream for hub server `{server}`"))
        })?;
        discover_server(server, upstream).await?
    };
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
pub(crate) fn legacy_ethos_dispatch(
    runner: &mut Runner,
    tool: &str,
    args: &Value,
    now: &str,
) -> Result<String> {
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

/// The delegated mutation tools' MCP descriptors (lot 4): REAL argument
/// schemas, unknown fields pinned closed, one Core verb per tool. Only
/// the covered verbs appear — visibility already IS the mandate, and
/// every call still re-verifies before touching anything.
fn ethos_write_tools(append_covered: bool, delete_covered: bool) -> Vec<Value> {
    let mut tools = Vec::new();
    if append_covered {
        tools.push(json!({
            "name": ETHOS_CREATE,
            "description": "Create one new section in the governed Ethos (circle zone). \
                            Missing folders on the path are created with the section, \
                            within the mandate's perimeter (strictly below a covered \
                            root). Every creation — folders included — is a signed, \
                            journalized delegated mutation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context": { "type": "string" },
                    "zone": { "type": "string" },
                    "folder": { "type": "string" },
                    "name": { "type": "string" },
                    "title": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "body": { "type": "string" }
                },
                "required": ["zone", "name", "body"],
                "additionalProperties": false
            }
        }));
        tools.push(json!({
            "name": ETHOS_EDIT,
            "description": "Rewrite one existing section body (circle zone). Requires the \
                            `expected_digest` from the last ethos.read — a concurrent \
                            change refuses the call instead of being overwritten. Every \
                            rewrite is a signed, journalized delegated mutation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context": { "type": "string" },
                    "zone": { "type": "string" },
                    "path": { "type": "string" },
                    "body": { "type": "string" },
                    "expected_digest": { "type": "string" }
                },
                "required": ["zone", "path", "body", "expected_digest"],
                "additionalProperties": false
            }
        }));
    }
    if delete_covered {
        tools.push(json!({
            "name": ETHOS_DELETE,
            "description": "Delete one section row (circle zone) — erasure is \
                            cryptographic, the act is signed and journalized. Pass \
                            `expected_digest` to refuse if the section changed since \
                            it was read.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "context": { "type": "string" },
                    "zone": { "type": "string" },
                    "path": { "type": "string" },
                    "expected_digest": { "type": "string" }
                },
                "required": ["zone", "path"],
                "additionalProperties": false
            }
        }));
    }
    tools
}

/// Validate and run one native ethos call for a DELEGATED session
/// (lot 1). Same fail-closed argument parsing as [`ethos_dispatch`];
/// the runner then re-verifies the session chain fresh (revocations
/// included) and derives the authority from THAT chain only — never
/// from the agent's own chains, never from another context. No SC1
/// session co-proof here on purpose: unlike `briefing.read` (whose
/// reads are served under the AGENT chain and need the session-act
/// xref as their only session-linked trace), every sealed ethos read
/// is journalized under the session chain itself — the trace IS the
/// session's.
pub(crate) fn legacy_ethos_dispatch_delegated(
    runner: &mut Runner,
    session: &crate::oauth::BearerSession,
    tool: &str,
    args: &Value,
    now: &str,
) -> Result<String> {
    let obj = args
        .as_object()
        .ok_or_else(|| bad_args(tool, "arguments must be an object"))?;
    let allowed: &[&str] = match tool {
        ETHOS_READ => &["context", "zone", "path"],
        ETHOS_CREATE => &["context", "zone", "folder", "name", "title", "tags", "body"],
        ETHOS_EDIT => &["context", "zone", "path", "body", "expected_digest"],
        ETHOS_DELETE => &["context", "zone", "path", "expected_digest"],
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
    let required = |name: &str| {
        text_field(name)?.ok_or_else(|| bad_args(tool, &format!("`{name}` is required")))
    };
    // The context argument was pinned to the session's own context by
    // the caller; parse it anyway so a malformed value refuses cleanly.
    let _ = text_field("context")?;
    let payload = match tool {
        ETHOS_READ => {
            let zone = required("zone")?;
            let path = required("path")?;
            runner.ethos_read_for_session(
                &session.context,
                &session.leaf_id,
                &session.session_pub,
                &session.leaf,
                &zone,
                &path,
                now,
            )?
        }
        ETHOS_LIST => runner.ethos_list_for_session(
            &session.context,
            &session.leaf_id,
            &session.session_pub,
            &session.leaf,
            now,
        )?,
        ETHOS_CONTEXT => runner.ethos_context_pack_for_session(
            &session.context,
            &session.leaf_id,
            &session.session_pub,
            &session.leaf,
            now,
        )?,
        ETHOS_CREATE => {
            let zone = required("zone")?;
            let name = required("name")?;
            let body = required("body")?;
            let folder = text_field("folder")?.unwrap_or_default();
            let title = text_field("title")?.unwrap_or_else(|| name.clone());
            let tags = match obj.get("tags") {
                None => Vec::new(),
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| bad_args(tool, "`tags` must be an array of strings"))
                    })
                    .collect::<Result<Vec<_>>>()?,
                Some(_) => return Err(bad_args(tool, "`tags` must be an array of strings")),
            };
            runner.ethos_create_for_session(
                &session.context,
                &session.leaf_id,
                &session.session_pub,
                &session.leaf,
                &zone,
                &folder,
                &name,
                &title,
                &tags,
                &body,
                now,
            )?
        }
        ETHOS_EDIT => {
            let zone = required("zone")?;
            let path = required("path")?;
            let body = required("body")?;
            let expected = required("expected_digest")?;
            runner.ethos_edit_for_session(
                &session.context,
                &session.leaf_id,
                &session.session_pub,
                &session.leaf,
                &zone,
                &path,
                &body,
                &expected,
                now,
            )?
        }
        ETHOS_DELETE => {
            let zone = required("zone")?;
            let path = required("path")?;
            let expected = text_field("expected_digest")?;
            runner.ethos_delete_for_session(
                &session.context,
                &session.leaf_id,
                &session.session_pub,
                &session.leaf,
                &zone,
                &path,
                expected.as_deref(),
                now,
            )?
        }
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

#[cfg(test)]
mod upstream_transport_tests {
    //! P-compat (2026-07-21) — the gateway speaks the modern streamable
    //! HTTP dialect: SSE responses and session ids, not only the plain
    //! JSON of the early fakes.
    use super::*;

    #[tokio::test]
    async fn ceremony_assets_are_embedded_no_store_and_secret_storage_free() {
        let response = ceremony_app().await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("default-src 'none'"));
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let source = std::str::from_utf8(&body).unwrap();
        assert!(source.contains("new DelegateSigner(seed)"));
        assert!(source.contains("sign_delegated_grant"));
        assert!(source.contains("/ceremony/prepare-grant"));
        assert!(source.contains("destroySigner()"));
        assert!(!source.contains("localStorage"));
        assert!(!source.contains("sessionStorage"));

        let enrollment = ceremony_enroll_app().await;
        assert_eq!(enrollment.headers()[header::CACHE_CONTROL], "no-store");
        let enrollment_body = axum::body::to_bytes(enrollment.into_body(), usize::MAX)
            .await
            .unwrap();
        let enrollment_source = std::str::from_utf8(&enrollment_body).unwrap();
        assert!(enrollment_source.contains("PBKDF2"));
        assert!(enrollment_source.contains("AES-GCM"));
        assert!(enrollment_source.contains("seed.fill(0)"));
        assert!(!enrollment_source.contains("localStorage"));
        assert!(!enrollment_source.contains("sessionStorage"));

        let wasm = ceremony_wasm_binary().await;
        assert_eq!(wasm.headers()[header::CONTENT_TYPE], "application/wasm");
        assert!(!axum::body::to_bytes(wasm.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn delegated_cli_requests_the_existing_authorize_state_machine_as_json() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("text/html, application/json; q=1"),
        );
        assert!(accepts_json(&headers));
        let response = ceremony_json(json!({ "v": 1, "ceremony": { "transaction_id": "t" } }));
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        headers.insert(header::ACCEPT, HeaderValue::from_static("text/html, */*"));
        assert!(!accepts_json(&headers));
    }

    #[test]
    fn browser_origin_is_single_and_exact_when_configured() {
        let configured = BTreeSet::from(["https://app.aithos.fr".to_owned()]);
        let mut headers = HeaderMap::new();
        assert!(origin_is_allowed(&headers, &configured));

        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://app.aithos.fr"),
        );
        assert!(origin_is_allowed(&headers, &configured));

        headers.append(
            header::ORIGIN,
            HeaderValue::from_static("https://neighbor.aithos.fr"),
        );
        assert!(!origin_is_allowed(&headers, &configured));
    }

    #[test]
    fn plain_json_is_unchanged() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let got = extract_jsonrpc(body, false, &json!(1)).unwrap();
        assert_eq!(got["result"]["tools"], json!([]));
    }

    #[test]
    fn sse_frame_yields_the_jsonrpc_message() {
        // The exact shape a hosted MCP (GitHub, Notion) returns.
        let body = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[{\"name\":\"get_issue\"}]}}\n\n";
        let got = extract_jsonrpc(body, true, &json!(1)).unwrap();
        assert_eq!(got["result"]["tools"][0]["name"], "get_issue");
        assert_eq!(got["id"], json!(1));
    }

    #[test]
    fn sse_keepalive_multiline_and_crlf() {
        let body = b": keepalive\r\nevent: message\r\ndata: {\"jsonrpc\":\"2.0\",\r\ndata: \"id\":7,\"result\":{\"ok\":true}}\r\n\r\n";
        let got = extract_jsonrpc(body, true, &json!(7)).unwrap();
        assert_eq!(got["result"]["ok"], true);
    }

    #[test]
    fn sse_picks_the_response_by_id_past_a_notification() {
        let body = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"picked\":\"me\"}}\n\n";
        let got = extract_jsonrpc(body, true, &json!(1)).unwrap();
        assert_eq!(got["result"]["picked"], "me");
    }

    #[test]
    fn malformed_or_empty_sse_is_none() {
        assert!(extract_jsonrpc(b"event: message\ndata: not json\n\n", true, &json!(1)).is_none());
        assert!(extract_jsonrpc(b"", true, &json!(1)).is_none());
    }

    #[test]
    fn demands_initialize_only_on_explicit_signal() {
        assert!(demands_initialize(&json!({
            "jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"Server not initialized"}
        })));
        assert!(demands_initialize(&json!({
            "jsonrpc":"2.0","id":1,"error":{"code":-32001,"message":"missing session id"}
        })));
        // A normal result, or an UNRELATED error, never triggers a handshake.
        assert!(!demands_initialize(
            &json!({"jsonrpc":"2.0","id":1,"result":{}})
        ));
        assert!(!demands_initialize(&json!({
            "jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params"}
        })));
    }

    // --- integration: a REAL local server answering SSE, over HttpUpstream ---

    async fn spawn(kind: &'static str) -> (u16, tokio::task::JoinHandle<()>) {
        use axum::routing::post;
        use axum::{Json, Router};
        let app = Router::new().route(
            "/mcp",
            post(move |Json(req): Json<Value>| async move {
                let id = req.get("id").cloned().unwrap_or(Value::Null);
                let payload = json!({
                    "jsonrpc":"2.0","id":id,
                    "result":{"tools":[{"name":"probe","description":"d","inputSchema":{"type":"object"}}]}
                });
                match kind {
                    "sse" => {
                        let frame = format!("event: message\ndata: {payload}\n\n");
                        axum::response::Response::builder()
                            .header("content-type", "text/event-stream")
                            .header(MCP_SESSION_HEADER, "sess-xyz")
                            .body(axum::body::Body::from(frame))
                            .unwrap()
                    }
                    _ => axum::response::Response::builder()
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(payload.to_string()))
                        .unwrap(),
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (port, handle)
    }

    #[tokio::test]
    async fn http_upstream_reads_sse_and_captures_session() {
        let (port, _h) = spawn("sse").await;
        let up = HttpUpstream::new(format!("http://127.0.0.1:{port}/mcp"));
        let resp = up
            .forward(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await
            .unwrap();
        assert_eq!(resp["result"]["tools"][0]["name"], "probe");
        // the server's session id was captured for the next call
        assert_eq!(up.session.lock().unwrap().as_deref(), Some("sess-xyz"));
    }

    #[tokio::test]
    async fn http_upstream_still_reads_plain_json() {
        let (port, _h) = spawn("json").await;
        let up = HttpUpstream::new(format!("http://127.0.0.1:{port}/mcp"));
        let resp = up
            .forward(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await
            .unwrap();
        assert_eq!(resp["result"]["tools"][0]["name"], "probe");
        assert!(up.session.lock().unwrap().is_none());
    }
}
