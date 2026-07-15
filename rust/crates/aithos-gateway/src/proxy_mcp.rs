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

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::core_bridge::{Bridge, Runner};
use crate::credentials::{CredentialBroker, CredentialRef};
use crate::hub::discover_server;
use crate::policy::Policy;
use crate::{GatewayError, Result};

/// JSON-RPC error code for a gateway policy refusal (implementation-defined
/// range). The call never reached the tool.
pub const POLICY_DENIED_CODE: i64 = -32001;

/// JSON-RPC "method not found" — what the multi-context router answers
/// for methods it does not serve (v1: everything but `initialize`,
/// `tools/list` and `tools/call`).
pub const METHOD_NOT_FOUND_CODE: i64 = -32601;

/// The native journal tools (lot C2): served by the gateway itself on
/// `/mcp`, never relayed to any upstream. The `journal` prefix is
/// reserved at config time so no context tool can ever shadow them.
pub const JOURNAL_WRITE: &str = "journal.write";
pub const JOURNAL_SEARCH: &str = "journal.search";
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
pub struct McpRouter<U> {
    pub runner: Arc<Mutex<Runner>>,
    pub upstreams: BTreeMap<String, U>,
    pub clock: Clock,
}

/// Agent-facing router for the multi-context runtime: same single
/// Streamable HTTP endpoint as the mono proxy.
pub fn router_multi<U: Upstream>(rt: Arc<McpRouter<U>>) -> Router {
    Router::new()
        .route("/mcp", post(handle_multi::<U>))
        .with_state(rt)
}

async fn handle_multi<U: Upstream>(
    State(rt): State<Arc<McpRouter<U>>>,
    Json(msg): Json<Value>,
) -> Json<Value> {
    Json(process_multi(&rt, msg).await)
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
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "aithos-gateway",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }),
        "tools/list" => {
            // Names only, aggregated from the declared maps (read AND
            // write: refusals must name the tool precisely). Schemas are
            // not proxied in v1 — the open object is the honest minimum.
            // The NATIVE journal tools close the list with their REAL
            // schemas: the gateway serves them itself, so it pins what
            // it governs (the hub decision, applied at home first).
            let mut tools = rt.runner.lock().await.listed_tools();
            tools.extend(native_journal_tools());
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
