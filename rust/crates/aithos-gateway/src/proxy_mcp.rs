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
use crate::policy::Policy;
use crate::{GatewayError, Result};

/// JSON-RPC error code for a gateway policy refusal (implementation-defined
/// range). The call never reached the tool.
pub const POLICY_DENIED_CODE: i64 = -32001;

/// JSON-RPC "method not found" — what the multi-context router answers
/// for methods it does not serve (v1: everything but `initialize`,
/// `tools/list` and `tools/call`).
pub const METHOD_NOT_FOUND_CODE: i64 = -32601;

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

/// Production upstream: JSON-RPC over POST (Streamable HTTP, stateless).
pub struct HttpUpstream {
    client: reqwest::Client,
    url: String,
}

impl HttpUpstream {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
        }
    }
}

impl Upstream for HttpUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        let resp = self
            .client
            .post(&self.url)
            .json(&body)
            .header("accept", "application/json")
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
            let tools: Vec<Value> = rt
                .runner
                .lock()
                .await
                .mapped_tools()
                .into_iter()
                .map(|name| json!({ "name": name, "inputSchema": { "type": "object" } }))
                .collect();
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
async fn tool_call_multi<U: Upstream>(rt: &McpRouter<U>, msg: Value) -> Value {
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

    // Default-deny across every context: unknown → journal refusal only.
    let Some(ctx) = runner.resolve(&tool).map(str::to_owned) else {
        let e = GatewayError::ToolNotMapped(tool.clone());
        runner.record_refusal(None, &tool, e.refusal_code(), &now);
        return error_response(id, &e);
    };

    // The resolved context's mandate at T — a named tool is not yet an
    // authorised tool (writes live here to be refused precisely).
    if let Err(deny) = runner.authorize(&ctx, &tool, &now) {
        runner.record_refusal(Some(&ctx), &tool, deny.refusal_code(), &now);
        return error_response(id, &deny);
    }

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
    let Some(upstream) = rt.upstreams.get(&ctx) else {
        let e = GatewayError::UpstreamFailed(format!("no upstream for context `{ctx}`"));
        return error_response(id, &e);
    };
    match upstream.forward(msg).await {
        Ok(resp) => resp,
        Err(e) => error_response(id, &e),
    }
}
