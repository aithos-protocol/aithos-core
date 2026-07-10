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
//! Layering holds even here: everything core-shaped arrives through the
//! bridge's re-exports, never from aithos-core/bundle directly.

use std::sync::{Arc, Mutex as StdMutex};

use cucumber::{given, then, when, World};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use aithos_gateway::config::GatewayConfig;
use aithos_gateway::core_bridge::{
    Bridge, EntropySource, EntryView, MandateWindow, OnboardOutcome, SeqEntropy,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_mcp::{process, McpProxy, Upstream, POLICY_DENIED_CODE};
use aithos_gateway::store_adapter::GatewayStore;
use aithos_gateway::{GatewayError, Result};

/// Fixed test instants (RFC 3339 Z — the wire's instant format).
const T0: &str = "2026-07-10T12:00:00Z";
const NOT_BEFORE: &str = "2026-07-10T00:00:00Z";
const NOT_AFTER: &str = "2026-08-09T00:00:00Z";

/// Fake company MCP server: records every body that reaches it and
/// answers a canned tool result.
#[derive(Clone, Default)]
struct FakeMcp {
    seen: Arc<StdMutex<Vec<Value>>>,
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
struct GatewayWorld {
    upstream: FakeMcp,
    proxy: Option<Arc<McpProxy<FakeMcp>>>,
    outcome: Option<OnboardOutcome>,
    read_tool: String,
    write_tool: String,
    last_tool: String,
    last_response: Option<Value>,
    audit_export: Option<String>,
    auditor_seed: Option<[u8; 32]>,
}

impl GatewayWorld {
    fn empty() -> Self {
        Self {
            upstream: FakeMcp::default(),
            proxy: None,
            outcome: None,
            read_tool: String::new(),
            write_tool: String::new(),
            last_tool: String::new(),
            last_response: None,
            audit_export: None,
            auditor_seed: None,
        }
    }

    fn proxy(&self) -> &Arc<McpProxy<FakeMcp>> {
        self.proxy.as_ref().expect("gateway onboarded")
    }

    async fn entries(&self) -> Vec<EntryView> {
        self.proxy()
            .bridge
            .lock()
            .await
            .entries()
            .expect("log readable")
    }

    async fn call(&mut self, tool: &str, extra_params: Value) {
        let mut params = json!({
            "name": tool,
            "arguments": { "q": "who is alice" }
        });
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra_params.as_object()) {
            for (k, v) in e {
                p.insert(k.clone(), v.clone());
            }
        }
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": params
        });
        self.last_tool = tool.to_owned();
        self.last_response = Some(process(self.proxy(), body).await);
    }

    /// Acts of the agent that reached the log (`x.mcp`).
    async fn mcp_acts(&self) -> Vec<EntryView> {
        self.entries()
            .await
            .into_iter()
            .filter(|e| e.kind == "action" && e.target.as_deref() == Some("x.mcp"))
            .collect()
    }

    /// Governance refusals logged by the gateway (`x.gateway`).
    async fn refusals(&self) -> Vec<EntryView> {
        self.entries()
            .await
            .into_iter()
            .filter(|e| e.kind == "action" && e.target.as_deref() == Some("x.gateway"))
            .collect()
    }
}

impl std::fmt::Debug for GatewayWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayWorld")
            .field("proxy", &self.proxy.is_some())
            .field("last_tool", &self.last_tool)
            .field("last_response", &self.last_response)
            .finish_non_exhaustive()
    }
}

// ------------------------------------------------------------ background

#[given(expr = "a company MCP server exposing tools {string} and {string}")]
async fn company_mcp(w: &mut GatewayWorld, read_tool: String, write_tool: String) {
    w.read_tool = read_tool;
    w.write_tool = write_tool;
}

#[given("a gateway onboarded with a read-only mandate for those tools")]
async fn gateway_onboarded(w: &mut GatewayWorld) {
    let yaml = format!(
        "\
listen: 127.0.0.1:4870
upstream_mcp: http://127.0.0.1:4124/mcp
store:
  kind: fs
  root: /var/lib/aithos
tools:
  {}: read
  {}: write
",
        w.read_tool, w.write_tool
    );
    let cfg = GatewayConfig::from_yaml(&yaml).expect("config accepted");

    let mut ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(ent.e32(), ent.e32());
    let (bridge, outcome) = Bridge::onboard(
        &cfg,
        GatewayStore::in_memory(),
        keyholder,
        Box::new(ent),
        &MandateWindow {
            not_before: NOT_BEFORE.to_owned(),
            not_after: NOT_AFTER.to_owned(),
        },
        T0,
    )
    .expect("onboarding succeeds");

    w.auditor_seed = Some(
        hex::decode(&outcome.auditor_seed_hex)
            .expect("hex seed")
            .try_into()
            .expect("32 bytes"),
    );
    w.outcome = Some(outcome);
    w.proxy = Some(Arc::new(McpProxy {
        policy: Policy::new(cfg.tools.clone()),
        bridge: Mutex::new(bridge),
        upstream: w.upstream.clone(),
        clock: Arc::new(|| T0.to_owned()),
    }));
}

// ------------------------------------------------------------ tool calls

#[when(expr = "the agent calls tool {string} through the gateway")]
async fn agent_calls(w: &mut GatewayWorld, tool: String) {
    w.call(&tool, json!({})).await;
}

#[when(expr = "the agent calls tool {string} claiming kind {string}")]
async fn agent_calls_claiming_kind(w: &mut GatewayWorld, tool: String, kind: String) {
    // The claimed kind is junk the gateway must never honour.
    w.call(&tool, json!({ "kind": kind })).await;
}

#[given("the agent has made one allowed and one refused call")]
async fn one_allowed_one_refused(w: &mut GatewayWorld) {
    let (read_tool, write_tool) = (w.read_tool.clone(), w.write_tool.clone());
    w.call(&read_tool, json!({})).await;
    w.call(&write_tool, json!({})).await;
}

#[then("the call reaches the MCP server and the agent gets the answer")]
async fn call_reached(w: &mut GatewayWorld) {
    let seen = w.upstream.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1, "exactly one call relayed upstream");
    assert_eq!(
        seen[0].pointer("/params/name").and_then(Value::as_str),
        Some(w.last_tool.as_str())
    );
    let resp = w.last_response.as_ref().expect("a response");
    assert_eq!(
        resp.pointer("/result/content/0/text")
            .and_then(Value::as_str),
        Some("alice@example.com"),
        "the upstream answer reaches the agent untouched"
    );
}

#[then("the call never reaches the MCP server")]
async fn call_never_reached(w: &mut GatewayWorld) {
    assert!(
        w.upstream.seen.lock().unwrap().is_empty(),
        "nothing must be relayed upstream"
    );
}

#[then("the agent receives a policy refusal")]
async fn agent_got_refusal(w: &mut GatewayWorld) {
    let resp = w.last_response.as_ref().expect("a response");
    assert_eq!(
        resp.pointer("/error/code").and_then(Value::as_i64),
        Some(POLICY_DENIED_CODE)
    );
    assert!(
        resp.pointer("/result").is_none(),
        "a refusal carries no result"
    );
}

// ------------------------------------------------------------ gamma log

#[then("the gamma log gains one act entry whose kind names the read operation")]
async fn log_gained_act(w: &mut GatewayWorld) {
    let acts = w.mcp_acts().await;
    assert_eq!(acts.len(), 1, "exactly one act entry");
    let e = &acts[0];
    assert_eq!(e.kind, "action", "the kind is the canonical act kind");
    let p = e.payload.as_ref().expect("clear payload");
    assert_eq!(
        p.get("action").and_then(Value::as_str),
        Some(aithos_gateway::policy::action_name(&w.last_tool).as_str()),
        "the action is the mapped operation name"
    );
    assert_eq!(
        p.get("tool").and_then(Value::as_str),
        Some(w.last_tool.as_str()),
        "the raw tool name stays readable for auditors"
    );
}

#[then("the gamma log gains one refusal entry")]
async fn log_gained_refusal(w: &mut GatewayWorld) {
    refusal_logged(w).await;
}

#[then("the refusal is logged")]
async fn refusal_logged(w: &mut GatewayWorld) {
    let refusals = w.refusals().await;
    assert_eq!(refusals.len(), 1, "exactly one refusal entry");
    let p = refusals[0].payload.as_ref().expect("clear payload");
    assert_eq!(p.get("action").and_then(Value::as_str), Some("refuse"));
    assert_eq!(
        p.get("tool").and_then(Value::as_str),
        Some(w.last_tool.as_str()),
        "the refusal names the refused tool"
    );
    assert!(
        w.mcp_acts().await.is_empty(),
        "a refused call must not leave an agent act entry"
    );
}

#[then("the claimed kind is ignored")]
async fn claimed_kind_ignored(w: &mut GatewayWorld) {
    assert!(
        w.entries().await.iter().all(|e| e.kind != "heartbeat"),
        "the caller-claimed kind never reaches the log"
    );
}

#[then("the logged entry bears the kind imposed by the operation mapping")]
async fn kind_is_imposed(w: &mut GatewayWorld) {
    let acts = w.mcp_acts().await;
    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0].kind, "action");
    assert_eq!(
        acts[0]
            .payload
            .as_ref()
            .and_then(|p| p.get("action"))
            .and_then(Value::as_str),
        Some(aithos_gateway::policy::action_name(&w.last_tool).as_str()),
        "the kind and action come from the operation mapping, not the caller"
    );
}

#[then("its signature verifies against the gateway-held agent key")]
async fn signature_verifies(w: &mut GatewayWorld) {
    let bridge = w.proxy().bridge.lock().await;
    bridge
        .verify_log()
        .expect("the whole chain verifies offline");
    let agent_mandate = bridge.agent_mandate_id().to_owned();
    drop(bridge);
    let acts = w.mcp_acts().await;
    assert_eq!(
        acts[0].authorized_via.as_deref(),
        Some(&[agent_mandate][..]),
        "the act is authorised via the agent's mandate chain"
    );
}

// ------------------------------------------------------------ audit

#[given("an auditor granted read.gamma scoped to act entries")]
async fn auditor_granted(w: &mut GatewayWorld) {
    // Granted at onboarding; this step picks up the auditor credentials.
    assert!(
        w.auditor_seed.is_some(),
        "onboarding handed out the auditor seed"
    );
    assert!(w
        .outcome
        .as_ref()
        .is_some_and(|o| !o.auditor_mandate.is_empty()));
}

#[when("the auditor exports the audit log from the gateway")]
async fn auditor_exports(w: &mut GatewayWorld) {
    let seed = w.auditor_seed.expect("auditor seed");
    let export = {
        let bridge = w.proxy().bridge.lock().await;
        bridge
            .export_audit(&seed, Some("action"), T0)
            .expect("scoped export succeeds")
    };
    w.audit_export = Some(export);
}

#[then("the export contains the act entries and verifies offline")]
async fn export_verifies(w: &mut GatewayWorld) {
    let export: Value =
        serde_json::from_str(w.audit_export.as_ref().expect("an export")).expect("valid JSON");
    let entries = export["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 2, "one allowed act + one refusal");
    assert!(entries.iter().all(|e| e["kind"] == "action"));
    let targets: Vec<&str> = entries
        .iter()
        .filter_map(|e| e["target"].as_str())
        .collect();
    assert!(targets.contains(&"x.mcp") && targets.contains(&"x.gateway"));
    // Offline verification of the underlying chain (completeness proofs
    // land with H — Merkle roots).
    w.proxy()
        .bridge
        .lock()
        .await
        .verify_log()
        .expect("chain verifies");
}

#[then("entries outside the auditor's scope are not readable")]
async fn export_scoped(w: &mut GatewayWorld) {
    let seed = w.auditor_seed.expect("auditor seed");
    let bridge = w.proxy().bridge.lock().await;
    // The certificate half refuses any wider dimension (§07.8):
    // grant entries…
    assert!(matches!(
        bridge.export_audit(&seed, Some("grant"), T0),
        Err(GatewayError::AuditDenied(_))
    ));
    // …and the unscoped whole log.
    assert!(matches!(
        bridge.export_audit(&seed, None, T0),
        Err(GatewayError::AuditDenied(_))
    ));
    // And the granted slice leaked none of them.
    let export: Value =
        serde_json::from_str(w.audit_export.as_ref().expect("an export")).expect("valid JSON");
    assert!(export["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .all(|e| e["kind"] != "grant"));
}

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
