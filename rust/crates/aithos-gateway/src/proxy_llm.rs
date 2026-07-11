//! LLM proxy — the provider-facing front of the gateway (Phase C).
//!
//! Decided 2026-07-10, contracted in `gateway-inference.feature`: v1
//! speaks the OpenAI-compatible chat-completions API (the widest
//! coverage: OpenAI, vLLM, Ollama, Mistral, Groq…). The gateway holds
//! the provider credential (the agent never sees it), **imposes the
//! model** whatever the agent asked for, reads the provider's **real
//! `usage`**, and meters one `inference` entry per call into the agent's
//! journal — metadata only, NEVER the prompt (that stays in the agent's
//! cache).
//!
//! The tap is fail-closed both ways around the provider:
//! - **before**: no inference pen, an invalid chain or an exhausted
//!   token budget refuses the call without any provider round-trip;
//! - **after**: an answer without usage, or a usage the budget cannot
//!   absorb, is **withheld** — an inference that cannot be metered into
//!   the journal never reaches the agent. Every refusal is a governance
//!   entry in the journal (§3bis.8: no context is ever involved here).

use std::sync::Arc;

use axum::http::StatusCode;
use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::core_bridge::Runner;
use crate::proxy_mcp::Clock;
use crate::{GatewayError, Result};

/// The tool name refusal entries carry for the LLM front — there is no
/// MCP tool here, but the journal story still names what was refused.
pub const LLM_TOOL: &str = "llm.chat";

/// Seam to the real provider: HTTP in production, in-process fake in the
/// acceptance tests. The implementation holds the credential — the proxy
/// itself never touches key material.
pub trait LlmUpstream: Send + Sync + 'static {
    fn complete(&self, body: Value) -> impl std::future::Future<Output = Result<Value>> + Send;
}

/// Production upstream: JSON over POST with the bearer credential. The
/// key lives here and is applied on the wire only.
pub struct HttpLlmUpstream {
    client: reqwest::Client,
    url: String,
    api_key: String,
}

impl HttpLlmUpstream {
    pub fn new(url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
            api_key: api_key.into(),
        }
    }
}

impl LlmUpstream for HttpLlmUpstream {
    async fn complete(&self, body: Value) -> Result<Value> {
        let resp = self
            .client
            .post(&self.url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::UpstreamFailed(e.to_string()))?;
        resp.json::<Value>()
            .await
            .map_err(|e| GatewayError::UpstreamFailed(e.to_string()))
    }
}

/// The LLM front shared by all requests. The runner is the SAME one the
/// MCP router serves (one journal, one story): the `Arc<Mutex<_>>` is
/// shared, never a second bridge over the same store.
pub struct LlmProxy<L> {
    pub runner: Arc<Mutex<Runner>>,
    pub upstream: L,
    /// The imposed model — the agent's choice is overwritten.
    pub model: String,
    /// Provider tag recorded in inference entries.
    pub provider: String,
    pub clock: Clock,
}

/// Agent-facing router: the OpenAI-compatible completions endpoint.
pub fn router_llm<L: LlmUpstream>(proxy: Arc<LlmProxy<L>>) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(handle_llm::<L>))
        .with_state(proxy)
}

async fn handle_llm<L: LlmUpstream>(
    State(gw): State<Arc<LlmProxy<L>>>,
    Json(msg): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let (status, body) = process_llm(&gw, msg).await;
    (status, Json(body))
}

/// Transport-free core of the LLM front — acceptance tests drive this
/// directly, the axum handler is a thin shell around it.
pub async fn process_llm<L: LlmUpstream>(gw: &LlmProxy<L>, mut body: Value) -> (StatusCode, Value) {
    let now = (gw.clock)();
    let mut runner = gw.runner.lock().await;

    // The tap, before the provider is touched: pen present, chain valid,
    // tokens remaining. (`record_inference` re-runs the full budget
    // check at append with the REAL usage — this is the polite half.)
    if let Err(deny) = runner.inference_headroom(&now) {
        runner.record_refusal(None, LLM_TOOL, deny.refusal_code(), &now);
        return (StatusCode::FORBIDDEN, llm_error(&deny));
    }

    // The model is imposed — whatever the agent asked for.
    if let Some(o) = body.as_object_mut() {
        o.insert("model".to_owned(), json!(gw.model));
    }

    let resp = match gw.upstream.complete(body).await {
        Ok(r) => r,
        Err(e) => {
            runner.record_refusal(None, LLM_TOOL, e.refusal_code(), &now);
            return (StatusCode::BAD_GATEWAY, llm_error(&e));
        }
    };

    // Meter from the provider's OWN usage. No usage → nothing to meter
    // → the completion is withheld (fail-closed, never guessed at).
    let usage = (
        resp.pointer("/usage/prompt_tokens").and_then(Value::as_u64),
        resp.pointer("/usage/completion_tokens")
            .and_then(Value::as_u64),
    );
    let (Some(tokens_in), Some(tokens_out)) = usage else {
        let e = GatewayError::UpstreamFailed(
            "the provider answer carries no usage — completion withheld".into(),
        );
        runner.record_refusal(None, LLM_TOOL, "usage_missing", &now);
        return (StatusCode::BAD_GATEWAY, llm_error(&e));
    };

    // Log-or-withhold: an inference the journal refuses to meter (e.g.
    // this call overran the remaining budget) never reaches the agent.
    if let Err(e) = runner.record_inference(&gw.provider, &gw.model, tokens_in, tokens_out, &now) {
        runner.record_refusal(None, LLM_TOOL, e.refusal_code(), &now);
        return (StatusCode::TOO_MANY_REQUESTS, llm_error(&e));
    }

    (StatusCode::OK, resp)
}

/// A refusal or failure as an OpenAI-style error body — message text
/// only, no key material, no prompt echo.
fn llm_error(err: &GatewayError) -> Value {
    json!({
        "error": {
            "message": format!("aithos gateway: {err}"),
            "type": "aithos_gateway_refusal",
        }
    })
}
