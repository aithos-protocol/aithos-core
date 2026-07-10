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

use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::core_bridge::Bridge;
use crate::policy::Policy;
use crate::{GatewayError, Result};

/// JSON-RPC error code for a gateway policy refusal (implementation-defined
/// range). The call never reached the tool.
pub const POLICY_DENIED_CODE: i64 = -32001;

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
