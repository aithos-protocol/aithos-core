//! BDD acceptance harness for the gateway (cucumber-rs).
//!
//! Same ritual as the protocol suite (docs/EXECUTION-PLAN.md): the
//! feature is co-written BEFORE the code, scenarios tagged @wip are
//! skipped so the suite stays green until each one is implemented and
//! untagged. Features live inside this crate — the repo-root `features/`
//! directory is the protocol's territory.
//!
//! Test topology (GATEWAY-BOOTSTRAP §8): a fake in-process MCP upstream
//! that records what reaches it, the real policy + bridge + gamma
//! underneath, and the transport-free `proxy_mcp::process` entry point.

use std::sync::{Arc, Mutex as StdMutex};

use cucumber::{given, then, when, World};
use serde_json::{json, Value};

use aithos_gateway::proxy_mcp::{McpProxy, Upstream};
use aithos_gateway::Result;

/// Fixed test instant (RFC 3339 Z — the wire's instant format).
pub const T0: &str = "2026-07-10T12:00:00Z";

/// Fake company MCP server: records every body that reaches it and
/// answers a canned tool result.
#[derive(Clone, Default)]
pub struct FakeMcp {
    pub seen: Arc<StdMutex<Vec<Value>>>,
}

impl Upstream for FakeMcp {
    async fn forward(&self, body: Value) -> Result<Value> {
        self.seen.lock().unwrap().push(body.clone());
        let id = body.get("id").cloned().unwrap_or(Value::Null);
        Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": "alice@example.com" }],
                "isError": false
            }
        }))
    }
}

/// One plugged-in gateway under test.
#[derive(World)]
#[world(init = Self::empty)]
pub struct GatewayWorld {
    /// The fake upstream the gateway relays to.
    pub upstream: FakeMcp,
    /// The proxy under test (None until the Background plugs it in).
    pub proxy: Option<Arc<McpProxy<FakeMcp>>>,
    /// Last JSON-RPC response the agent saw.
    pub last_response: Option<Value>,
    /// Last audit export produced for the auditor.
    pub audit_export: Option<String>,
}

impl GatewayWorld {
    fn empty() -> Self {
        Self {
            upstream: FakeMcp::default(),
            proxy: None,
            last_response: None,
            audit_export: None,
        }
    }
}

impl std::fmt::Debug for GatewayWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayWorld")
            .field("proxy", &self.proxy.is_some())
            .field("last_response", &self.last_response)
            .finish_non_exhaustive()
    }
}

// ------------------------------------------------------------ background

#[given(expr = "a company MCP server exposing tools {string} and {string}")]
async fn company_mcp(_w: &mut GatewayWorld, _read_tool: String, _write_tool: String) {}

#[given("a gateway onboarded with a read-only mandate for those tools")]
async fn gateway_onboarded(_w: &mut GatewayWorld) {}

// ------------------------------------------------------------ tool calls

#[when(expr = "the agent calls tool {string} through the gateway")]
async fn agent_calls(_w: &mut GatewayWorld, _tool: String) {}

#[when(expr = "the agent calls tool {string} claiming kind {string}")]
async fn agent_calls_claiming_kind(_w: &mut GatewayWorld, _tool: String, _kind: String) {}

#[then("the call reaches the MCP server and the agent gets the answer")]
async fn call_reached(_w: &mut GatewayWorld) {}

#[then("the call never reaches the MCP server")]
async fn call_never_reached(_w: &mut GatewayWorld) {}

#[then("the agent receives a policy refusal")]
async fn agent_got_refusal(_w: &mut GatewayWorld) {}

// ------------------------------------------------------------ gamma log

#[then("the gamma log gains one act entry whose kind names the read operation")]
async fn log_gained_act(_w: &mut GatewayWorld) {}

#[then("the gamma log gains one refusal entry")]
async fn log_gained_refusal(_w: &mut GatewayWorld) {}

#[then("the refusal is logged")]
async fn refusal_logged(_w: &mut GatewayWorld) {}

#[then("the claimed kind is ignored")]
async fn claimed_kind_ignored(_w: &mut GatewayWorld) {}

#[then("the logged entry bears the kind imposed by the operation mapping")]
async fn kind_is_imposed(_w: &mut GatewayWorld) {}

#[then("its signature verifies against the gateway-held agent key")]
async fn signature_verifies(_w: &mut GatewayWorld) {}

// ------------------------------------------------------------ audit

#[given("an auditor granted read.gamma scoped to act entries")]
async fn auditor_granted(_w: &mut GatewayWorld) {}

#[when("the auditor exports the audit log from the gateway")]
async fn auditor_exports(_w: &mut GatewayWorld) {}

#[then("the export contains the act entries and verifies offline")]
async fn export_verifies(_w: &mut GatewayWorld) {}

#[then("entries outside the auditor's scope are not readable")]
async fn export_scoped(_w: &mut GatewayWorld) {}

// ------------------------------------------------------------------ main

#[tokio::main]
async fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/features");
    GatewayWorld::cucumber()
        .filter_run(features, |_, _, scenario| {
            !scenario.tags.iter().any(|t| t == "wip")
        })
        .await;
}
