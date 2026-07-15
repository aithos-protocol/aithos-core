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

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex as StdMutex};

use cucumber::{given, then, when, World};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use aithos_gateway::config::{GatewayConfig, ToolAccess, ToolMap};
use aithos_gateway::core_bridge::{
    agent_pub_multibase, cert_constraints, cert_grantee_pub, cert_perimeter, gamma_view,
    gateway_pub_multibase, journal_notes_view, owner_enroll_server, owner_grant_context,
    owner_init_context, owner_init_journal, owner_read_hub_manifest, owner_read_journal_note,
    owner_reenroll_server, Bridge, ContextRuntime, EntropySource, EntryView, EquipOutcome,
    MandateWindow, OnboardOutcome, RawStore, ReenrollOutcome, Runner, SeqEntropy, STATE_PATH,
};
use aithos_gateway::hub::{approve_manifest, discover_server, ApprovedManifest, ToolApproval};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_llm::{process_llm, LlmProxy, LlmUpstream, LLM_TOOL};
use aithos_gateway::proxy_mcp::{
    process, process_multi, refresh_server_manifest, McpProxy, McpRouter, Upstream, JOURNAL_SEARCH,
    JOURNAL_WRITE, METHOD_NOT_FOUND_CODE, POLICY_DENIED_CODE,
};
use aithos_gateway::store_adapter::GatewayStore;
use aithos_gateway::{GatewayError, Result};

/// Fixed test instants (RFC 3339 Z — the wire's instant format).
const T0: &str = "2026-07-10T12:00:00Z";
const NOT_BEFORE: &str = "2026-07-10T00:00:00Z";
const NOT_AFTER: &str = "2026-08-09T00:00:00Z";

/// Fake company MCP server: records every body that reaches it and
/// answers a canned tool result (distinct texts tell N upstreams apart).
#[derive(Clone)]
struct FakeMcp {
    seen: Arc<StdMutex<Vec<Value>>>,
    text: String,
    advertised_tools: Arc<StdMutex<Vec<Value>>>,
}

impl Default for FakeMcp {
    fn default() -> Self {
        Self::with_text("alice@example.com")
    }
}

impl FakeMcp {
    fn with_text(text: &str) -> Self {
        Self {
            seen: Arc::default(),
            text: text.to_owned(),
            advertised_tools: Arc::default(),
        }
    }

    fn advertising(tools: Vec<Value>) -> Self {
        Self {
            advertised_tools: Arc::new(StdMutex::new(tools)),
            ..Self::default()
        }
    }
}

impl Upstream for FakeMcp {
    async fn forward(&self, body: Value) -> Result<Value> {
        self.seen.lock().unwrap().push(body.clone());
        let id = body.get("id").cloned().unwrap_or(Value::Null);
        if body.get("method").and_then(Value::as_str) == Some("tools/list") {
            return Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": self.advertised_tools.lock().unwrap().clone() }
            }));
        }
        Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": self.text }],
                "isError": false
            }
        }))
    }
}

/// Fake OpenAI-compatible provider: records every body that reaches it,
/// answers a canned completion, and — like the real HTTP upstream — is
/// the one place holding the credential (which must never surface).
#[derive(Clone)]
struct FakeLlm {
    seen: Arc<StdMutex<Vec<Value>>>,
    /// The usage the provider reports; `None` = an answer without usage.
    usage: Arc<StdMutex<Option<(u64, u64)>>>,
    api_key: String,
    answer: String,
}

impl FakeLlm {
    fn new() -> Self {
        Self {
            seen: Arc::default(),
            usage: Arc::new(StdMutex::new(Some((12, 30)))),
            api_key: "sk-live-secret-credential".to_owned(),
            answer: "the capital of Prussia was Königsberg".to_owned(),
        }
    }
}

impl LlmUpstream for FakeLlm {
    async fn complete(&self, body: Value) -> Result<Value> {
        // The credential is applied here, wire-side, as HttpLlmUpstream
        // would — asserting it never leaks agent-side stays meaningful.
        let _bearer = format!("Bearer {}", self.api_key);
        self.seen.lock().unwrap().push(body);
        let mut resp = json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": self.answer },
                "finish_reason": "stop"
            }],
        });
        if let Some((t_in, t_out)) = *self.usage.lock().unwrap() {
            resp["usage"] = json!({
                "prompt_tokens": t_in,
                "completion_tokens": t_out,
                "total_tokens": t_in + t_out,
            });
        }
        Ok(resp)
    }
}

/// The harness tool map of one named context (feature contract: labels
/// carry their own tool families).
fn context_tools(label: &str) -> (String, String) {
    match label {
        "company-brand" => ("brand.read".into(), "brand.update".into()),
        "ui-designer" => ("figma.read".into(), "figma.update".into()),
        other => panic!("unknown context label in the harness: {other}"),
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
    /// Birth scenario (Phase B): what the runner published, and where
    /// its identity file lives (the tempdir keeps it alive).
    published: Option<String>,
    identity_file: Option<std::path::PathBuf>,
    scratch: Option<tempfile::TempDir>,
    /// Provisioning scenarios (Phase B): owner side.
    master: Option<[u8; 32]>,
    agent_pub: Option<String>,
    gateway_pub: Option<String>,
    journal_store: Option<GatewayStore>,
    journal_outcome: Option<EquipOutcome>,
    /// Single equipped context for the grant scenario: (label, store,
    /// read tool, write tool, outcome).
    ctx: Option<(String, GatewayStore, String, String, Option<EquipOutcome>)>,
    /// Multi-context runtime (lot 3): the router, one fake upstream per
    /// context, and the owner-side view of every context store.
    router: Option<Arc<McpRouter<FakeMcp>>>,
    multi_upstreams: BTreeMap<String, FakeMcp>,
    ctx_stores: BTreeMap<String, GatewayStore>,
    ctx_dids: BTreeMap<String, String>,
    /// LLM front (Phase C): the proxy under test, its fake provider and
    /// the transport status of the last completion call.
    llm: Option<Arc<LlmProxy<FakeLlm>>>,
    llm_provider: Option<FakeLlm>,
    llm_status: Option<axum::http::StatusCode>,
    /// Journal tools (lot C2): the parse verdict of a config under test.
    config_error: Option<String>,
    /// Hub H1 config scenarios: the first declared server/raw tool pair.
    hub_server: Option<String>,
    hub_tool: Option<String>,
    approved_manifest: Option<ApprovedManifest>,
    reenroll: Option<ReenrollOutcome>,
    old_agent_mandate: Option<String>,
    /// Vault credential scenarios: secrets declared before provisioning
    /// (path → field → value), an optional path answering garbage, and
    /// the live harness once enrolled.
    vault_pending: BTreeMap<String, BTreeMap<String, String>>,
    vault_malformed: Option<String>,
    vault: Option<VaultHarness>,
    /// The last tools/list answer when a step captured it separately
    /// from `last_response` (combined list+call steps).
    last_list: Option<Value>,
    /// Expected pinned surface per exposed name: (description, schema) —
    /// set by whichever provisioning ran, asserted by the shared step.
    expected_pins: BTreeMap<String, (Option<String>, Value)>,
    /// Grants scenarios (lot W): the enrolled store when no runtime is
    /// opened yet, and its context label.
    grants_store: Option<GatewayStore>,
    grants_label: Option<String>,
    grants_roots: Option<(std::path::PathBuf, std::path::PathBuf)>,
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
            published: None,
            identity_file: None,
            scratch: None,
            master: None,
            agent_pub: None,
            gateway_pub: None,
            journal_store: None,
            journal_outcome: None,
            ctx: None,
            router: None,
            multi_upstreams: BTreeMap::new(),
            ctx_stores: BTreeMap::new(),
            ctx_dids: BTreeMap::new(),
            llm: None,
            llm_provider: None,
            llm_status: None,
            config_error: None,
            hub_server: None,
            hub_tool: None,
            approved_manifest: None,
            reenroll: None,
            old_agent_mandate: None,
            vault_pending: BTreeMap::new(),
            vault_malformed: None,
            vault: None,
            last_list: None,
            expected_pins: BTreeMap::new(),
            grants_store: None,
            grants_label: None,
            grants_roots: None,
        }
    }

    fn master(&mut self) -> [u8; 32] {
        *self.master.get_or_insert([7u8; 32])
    }

    /// The runner's published public keys (born once per scenario).
    fn pubs(&mut self) -> (String, String) {
        if self.agent_pub.is_none() {
            let mut ent = SeqEntropy::default();
            let kh = Keyholder::from_entropy(ent.e32(), ent.e32());
            self.agent_pub = Some(agent_pub_multibase(&kh));
            self.gateway_pub = Some(gateway_pub_multibase(&kh));
        }
        (
            self.agent_pub.clone().unwrap(),
            self.gateway_pub.clone().unwrap(),
        )
    }

    fn window() -> MandateWindow {
        MandateWindow {
            not_before: NOT_BEFORE.to_owned(),
            not_after: NOT_AFTER.to_owned(),
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
        let response = if let Some(vault) = &self.vault {
            process_multi(&vault.router, body).await
        } else if let Some(router) = &self.router {
            process_multi(router, body).await
        } else {
            process(self.proxy(), body).await
        };
        if let Some(vault) = &mut self.vault {
            vault.responses.push(response.clone());
        }
        self.last_response = Some(response);
    }

    /// Owner-side gamma view of a named context store.
    fn ctx_gamma(&self, ctx: &str) -> Vec<EntryView> {
        let store = self.ctx_stores.get(ctx).expect("a provisioned context");
        gamma_view(store.clone()).expect("context gamma readable")
    }

    /// Owner-side gamma view of the journal store.
    fn journal_gamma(&self) -> Vec<EntryView> {
        gamma_view(self.journal_store.clone().expect("a journal")).expect("journal gamma readable")
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

async fn provision_single_hub(w: &mut GatewayWorld) {
    if w.router.is_some() {
        return;
    }
    let dir = tempfile::tempdir().expect("hub tempdir");
    let context_root = dir.path().join("support");
    let journal_root = dir.path().join("journal");
    let context_cfg = aithos_gateway::config::StoreConfig::Fs {
        root: context_root.clone(),
    };
    let journal_cfg = aithos_gateway::config::StoreConfig::Fs {
        root: journal_root.clone(),
    };
    let context_store = GatewayStore::from_config(&context_cfg).expect("context store");
    let journal_store = GatewayStore::from_config(&journal_cfg).expect("journal store");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let mut owner_ent = SeqEntropy::default();
    owner_init_context(
        &master,
        "customer-support",
        context_store.clone(),
        T0,
        &mut owner_ent,
    )
    .expect("hub context created");

    let advertised = vec![
        json!({
            "name": "issues.list",
            "description": "List approved issues",
            "inputSchema": {
                "type": "object",
                "properties": { "state": { "type": "string" } },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "issues.create",
            "description": "Create an issue",
            "inputSchema": {
                "type": "object",
                "properties": { "title": { "type": "string" } },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
    ];
    let upstream = FakeMcp::advertising(advertised);
    let proposed = discover_server("github", &upstream)
        .await
        .expect("hub discovery");
    let approved = approve_manifest(
        &proposed,
        &BTreeMap::from([
            (
                "issues.list".to_owned(),
                ToolApproval::class(ToolAccess::Read),
            ),
            (
                "issues.create".to_owned(),
                ToolApproval::class(ToolAccess::Write),
            ),
        ]),
    )
    .expect("hub approval");
    owner_enroll_server(
        &master,
        "customer-support",
        &agent_pub,
        &gateway_pub,
        &approved,
        context_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("hub enrollment");
    let journal_outcome = owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("hub journal");

    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let cfg = GatewayConfig::from_yaml(&format!(
        r#"listen: 127.0.0.1:4870
servers:
  - name: github
    transport: http
    url: https://github.invalid/mcp
contexts:
  - name: customer-support
    store: {{ kind: fs, root: {} }}
    tools:
      github__issues_list: {{ server: github, tool: issues.list, access: read }}
      github__issues_create: {{ server: github, tool: issues.create, access: write }}
journal:
  store: {{ kind: fs, root: {} }}
"#,
        quote(&context_root),
        quote(&journal_root)
    ))
    .expect("hub config");
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner = Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
        .expect("governed runner opens its pins");
    upstream.seen.lock().unwrap().clear();
    w.expected_pins.insert(
        "github__issues_list".to_owned(),
        (
            Some("List approved issues".to_owned()),
            json!({
                "type": "object",
                "properties": { "state": { "type": "string" } },
                "additionalProperties": false
            }),
        ),
    );
    w.ctx_stores
        .insert("customer-support".to_owned(), context_store);
    w.journal_store = Some(journal_store);
    w.journal_outcome = Some(journal_outcome);
    w.approved_manifest = Some(approved);
    w.upstream = upstream.clone();
    w.multi_upstreams
        .insert("github".to_owned(), upstream.clone());
    w.router = Some(Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(runner)),
        upstreams: BTreeMap::from([("github".to_owned(), upstream)]),
        clock: Arc::new(|| T0.to_owned()),
    }));
    w.scratch = Some(dir);
}

async fn provision_shared_hub(w: &mut GatewayWorld) {
    if w.router.is_some() {
        return;
    }
    let dir = tempfile::tempdir().expect("shared hub tempdir");
    let support_root = dir.path().join("support");
    let engineering_root = dir.path().join("engineering");
    let journal_root = dir.path().join("journal");
    let store_cfg = |root: &std::path::Path| aithos_gateway::config::StoreConfig::Fs {
        root: root.to_owned(),
    };
    let support_store =
        GatewayStore::from_config(&store_cfg(&support_root)).expect("support store");
    let engineering_store =
        GatewayStore::from_config(&store_cfg(&engineering_root)).expect("engineering store");
    let journal_store =
        GatewayStore::from_config(&store_cfg(&journal_root)).expect("journal store");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let mut owner_ent = SeqEntropy::default();
    let advertised = vec![
        json!({
            "name": "issues.list",
            "description": "List issues",
            "inputSchema": { "type": "object", "additionalProperties": false }
        }),
        json!({
            "name": "pulls.list",
            "description": "List pull requests",
            "inputSchema": { "type": "object", "additionalProperties": false }
        }),
    ];
    let upstream = FakeMcp::advertising(advertised);
    let proposed = discover_server("github", &upstream)
        .await
        .expect("shared discovery");
    for (label, store, approvals) in [
        (
            "customer-support",
            support_store.clone(),
            BTreeMap::from([
                (
                    "issues.list".to_owned(),
                    ToolApproval::class(ToolAccess::Read),
                ),
                (
                    "pulls.list".to_owned(),
                    ToolApproval::class(ToolAccess::Write),
                ),
            ]),
        ),
        (
            "engineering",
            engineering_store.clone(),
            BTreeMap::from([
                (
                    "issues.list".to_owned(),
                    ToolApproval::class(ToolAccess::Write),
                ),
                (
                    "pulls.list".to_owned(),
                    ToolApproval::class(ToolAccess::Read),
                ),
            ]),
        ),
    ] {
        owner_init_context(&master, label, store.clone(), T0, &mut owner_ent)
            .expect("shared context created");
        let approved = approve_manifest(&proposed, &approvals).expect("shared approval");
        let outcome = owner_enroll_server(
            &master,
            label,
            &agent_pub,
            &gateway_pub,
            &approved,
            store.clone(),
            &GatewayWorld::window(),
            T0,
            &mut owner_ent,
        )
        .expect("shared enrollment");
        w.ctx_dids.insert(label.to_owned(), outcome.ethos_did);
        w.ctx_stores.insert(label.to_owned(), store);
    }
    owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("shared journal");
    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let cfg = GatewayConfig::from_yaml(&format!(
        r#"listen: 127.0.0.1:4870
servers:
  - name: github
    transport: http
    url: https://github.invalid/mcp
contexts:
  - name: customer-support
    store: {{ kind: fs, root: {} }}
    tools:
      github__issues_list: {{ server: github, tool: issues.list, access: read }}
  - name: engineering
    store: {{ kind: fs, root: {} }}
    tools:
      github__pulls_list: {{ server: github, tool: pulls.list, access: read }}
journal:
  store: {{ kind: fs, root: {} }}
"#,
        quote(&support_root),
        quote(&engineering_root),
        quote(&journal_root)
    ))
    .expect("shared hub config");
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner = Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
        .expect("shared governed runner");
    upstream.seen.lock().unwrap().clear();
    w.upstream = upstream.clone();
    w.multi_upstreams
        .insert("github".to_owned(), upstream.clone());
    w.journal_store = Some(journal_store);
    w.router = Some(Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(runner)),
        upstreams: BTreeMap::from([("github".to_owned(), upstream)]),
        clock: Arc::new(|| T0.to_owned()),
    }));
    w.scratch = Some(dir);
}

#[given(expr = "server {string} is shared by contexts {string} and {string}")]
async fn hub_server_shared(w: &mut GatewayWorld, server: String, first: String, second: String) {
    assert_eq!(server, "github");
    assert_eq!(
        (first.as_str(), second.as_str()),
        ("customer-support", "engineering")
    );
    provision_shared_hub(w).await;
}

#[given(expr = "{string} covers exposed tool {string}")]
async fn hub_context_covers(w: &mut GatewayWorld, context: String, tool: String) {
    let router = w.router.as_ref().expect("shared router");
    assert_eq!(
        router.runner.lock().await.resolve(&tool),
        Some(context.as_str())
    );
}

#[when("the agent calls both covered tools through the hub")]
async fn agent_calls_shared_tools(w: &mut GatewayWorld) {
    w.call("github__issues_list", json!({})).await;
    w.call("github__pulls_list", json!({})).await;
}

#[then(expr = "both calls reach the same {string} upstream under their raw tool names")]
async fn shared_calls_use_raw_names(w: &mut GatewayWorld, server: String) {
    assert_eq!(server, "github");
    let names: Vec<String> = w
        .upstream
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body["method"] == "tools/call")
        .filter_map(|body| body.pointer("/params/name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    assert_eq!(names, ["issues.list", "pulls.list"]);
}

#[then(expr = "{string} is logged in the {string} gamma only")]
async fn hub_act_logged_in_context_only(w: &mut GatewayWorld, tool: String, context: String) {
    let own = acts_on(&w.ctx_gamma(&context), "x.github");
    assert_eq!(
        own.iter()
            .filter(|entry| payload_str(entry, "tool") == Some(tool.as_str()))
            .count(),
        1
    );
    for other in w.ctx_stores.keys().filter(|name| **name != context) {
        assert!(acts_on(&w.ctx_gamma(other), "x.github")
            .iter()
            .all(|entry| payload_str(entry, "tool") != Some(tool.as_str())));
    }
}

#[given(expr = "server {string} is enrolled with covered tool {string}")]
async fn hub_server_enrolled(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, "github");
    assert_eq!(tool, "issues.list");
    provision_single_hub(w).await;
}

#[given(expr = "server {string} has known but ungranted tool {string}")]
async fn hub_known_ungranted(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, "github");
    assert_eq!(tool, "issues.create");
    // The vault harness enrolls the write half too — nothing to add.
    if w.vault.is_some() {
        return;
    }
    provision_single_hub(w).await;
}

#[when("the upstream is unavailable and the agent lists the tools")]
async fn hub_lists_offline(w: &mut GatewayWorld) {
    let router = w.router.as_ref().expect("hub router");
    w.last_response = Some(
        process_multi(
            router,
            json!({ "jsonrpc": "2.0", "id": 41, "method": "tools/list" }),
        )
        .await,
    );
}

#[then(expr = "the list includes {string} with its pinned description and input schema")]
async fn hub_list_has_pin(w: &mut GatewayWorld, exposed: String) {
    let listed = w.listed_tools();
    let tool = listed
        .iter()
        .find(|tool| tool["name"] == exposed)
        .expect("covered pin listed");
    let (description, schema) = w
        .expected_pins
        .get(&exposed)
        .expect("the provisioning recorded the expected pin");
    match description {
        Some(text) => assert_eq!(tool["description"], text.as_str()),
        None => assert!(tool.get("description").is_none()),
    }
    assert_eq!(
        &tool["inputSchema"], schema,
        "the exact pinned schema is served"
    );
}

#[then(expr = "the list does not include {string}")]
async fn hub_list_hides_ungranted(w: &mut GatewayWorld, exposed: String) {
    assert!(w.listed_tools().iter().all(|tool| tool["name"] != exposed));
}

#[then("no request reaches the upstream")]
async fn hub_upstream_untouched(w: &mut GatewayWorld) {
    assert!(w.upstream.seen.lock().unwrap().is_empty());
}

#[when("the agent requests MCP resources through the hub")]
async fn hub_requests_resources(w: &mut GatewayWorld) {
    let router = w.router.as_ref().expect("hub router");
    w.last_response = Some(
        process_multi(
            router,
            json!({ "jsonrpc": "2.0", "id": 42, "method": "resources/list" }),
        )
        .await,
    );
}

#[then("the gateway answers method not found")]
async fn hub_method_not_found(w: &mut GatewayWorld) {
    assert_eq!(
        w.last_response.as_ref().expect("response")["error"]["code"],
        METHOD_NOT_FOUND_CODE
    );
}

#[given(expr = "the agent calls {string} through the hub")]
#[when(expr = "the agent calls {string} through the hub")]
async fn agent_calls_hub(w: &mut GatewayWorld, tool: String) {
    w.call(&tool, json!({})).await;
}

#[then("the call never reaches the upstream")]
async fn hub_call_not_relayed(w: &mut GatewayWorld) {
    assert!(
        w.upstream.seen.lock().unwrap().is_empty(),
        "the governed tool call must not be relayed"
    );
}

#[then(expr = "the refusal names {string}")]
async fn hub_refusal_names_tool(w: &mut GatewayWorld, tool: String) {
    let response = w.last_response.as_ref().expect("refusal response");
    assert!(response["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains(&tool)));
}

#[then("the gamma of the context that knows the tool gains one governance refusal")]
async fn hub_context_governance_refusal(w: &mut GatewayWorld) {
    let refusals = acts_on(&w.ctx_gamma("customer-support"), "x.gateway");
    assert_eq!(refusals.len(), 1);
    assert_eq!(
        payload_str(&refusals[0], "tool"),
        Some(w.last_tool.as_str())
    );
}

#[given(expr = "the upstream now advertises a different description for {string}")]
async fn upstream_description_drift(w: &mut GatewayWorld, raw_tool: String) {
    provision_single_hub(w).await;
    let mut advertised = w.upstream.advertised_tools.lock().unwrap();
    let tool = advertised
        .iter_mut()
        .find(|tool| tool["name"] == raw_tool)
        .expect("advertised tool");
    tool["description"] = Value::String("POISONED runtime description".to_owned());
}

#[given("the gateway's runtime drift control observes that change")]
async fn hub_observes_drift(w: &mut GatewayWorld) {
    let router = w.router.as_ref().expect("hub router");
    let error = refresh_server_manifest(router, "github")
        .await
        .expect_err("changed manifest is refused");
    assert!(matches!(error, GatewayError::ManifestDrift { .. }));
    w.upstream.seen.lock().unwrap().clear();
}

#[then("the call is refused as manifest drift before the tool is relayed")]
async fn hub_call_refused_for_drift(w: &mut GatewayWorld) {
    assert!(w.upstream.seen.lock().unwrap().is_empty());
    assert!(
        w.last_response.as_ref().expect("response")["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("manifest drift"))
    );
}

#[then("the granting context gamma gains one governance refusal")]
async fn hub_granting_context_refusal(w: &mut GatewayWorld) {
    hub_context_governance_refusal(w).await;
}

#[given(expr = "discovery finds an owner-accepted schema change for {string}")]
async fn hub_discovers_accepted_change(w: &mut GatewayWorld, raw_tool: String) {
    provision_single_hub(w).await;
    let advertised = {
        let mut advertised = w.upstream.advertised_tools.lock().unwrap();
        let tool = advertised
            .iter_mut()
            .find(|tool| tool["name"] == raw_tool)
            .expect("advertised tool");
        tool["inputSchema"]["properties"]["page"] = json!({ "type": "integer", "minimum": 1 });
        advertised.clone()
    };
    let discovery = FakeMcp::advertising(advertised);
    let proposed = discover_server("github", &discovery)
        .await
        .expect("changed discovery");
    w.approved_manifest = Some(
        approve_manifest(
            &proposed,
            &BTreeMap::from([
                (
                    "issues.list".to_owned(),
                    ToolApproval::class(ToolAccess::Read),
                ),
                (
                    "issues.create".to_owned(),
                    ToolApproval::class(ToolAccess::Write),
                ),
            ]),
        )
        .expect("owner accepts changed schema"),
    );
}

#[when("the owner re-enrolls the tool for the same agent key")]
async fn owner_reenrolls_hub(w: &mut GatewayWorld) {
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let store = w
        .ctx_stores
        .get("customer-support")
        .expect("context store")
        .clone();
    let state: Value = serde_json::from_slice(
        &store
            .clone()
            .get(STATE_PATH)
            .expect("state readable")
            .expect("state present"),
    )
    .expect("state JSON");
    w.old_agent_mandate = Some(state["agent_mandate"].as_str().unwrap().to_owned());
    let mut ent = SeqEntropy::default();
    let result = owner_reenroll_server(
        &master,
        "customer-support",
        &agent_pub,
        &gateway_pub,
        w.approved_manifest.as_ref().expect("changed approval"),
        store,
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("governed replacement");

    let dir = w.scratch.as_ref().expect("hub scratch").path();
    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let cfg = GatewayConfig::from_yaml(&format!(
        r#"listen: 127.0.0.1:4870
servers:
  - name: github
    transport: http
    url: https://github.invalid/mcp
contexts:
  - name: customer-support
    store: {{ kind: fs, root: {} }}
    tools:
      github__issues_list: {{ server: github, tool: issues.list, access: read }}
      github__issues_create: {{ server: github, tool: issues.create, access: write }}
journal:
  store: {{ kind: fs, root: {} }}
"#,
        quote(&dir.join("support")),
        quote(&dir.join("journal"))
    ))
    .unwrap();
    let mut kh_ent = SeqEntropy::default();
    let runner = Runner::open(
        &cfg,
        Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32()),
        || Box::new(SeqEntropy::default()),
    )
    .expect("runner reopens the replacement pin");
    w.router = Some(Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(runner)),
        upstreams: BTreeMap::from([("github".to_owned(), w.upstream.clone())]),
        clock: Arc::new(|| T0.to_owned()),
    }));
    w.reenroll = Some(result);
}

#[then("a new mandate covers the newly pinned manifest")]
async fn new_mandate_covers_reenroll(w: &mut GatewayWorld) {
    let result = w.reenroll.as_ref().expect("replacement result");
    assert_ne!(
        result.equipment.agent_mandate,
        *w.old_agent_mandate.as_ref().unwrap()
    );
    assert_eq!(
        cert_perimeter(
            w.ctx_stores["customer-support"].clone(),
            &result.equipment.agent_mandate
        )
        .unwrap(),
        vec!["act.x.github.issues_list"]
    );
    let pinned = owner_read_hub_manifest(
        &w.master(),
        "customer-support",
        "github",
        w.ctx_stores["customer-support"].clone(),
    )
    .unwrap();
    assert_eq!(
        pinned
            .tools
            .iter()
            .find(|tool| tool.name == "issues.list")
            .unwrap()
            .input_schema["properties"]["page"]["type"],
        "integer"
    );
}

#[then("the old mandate is politically revoked")]
async fn old_mandate_revoked(w: &mut GatewayWorld) {
    let old = w.old_agent_mandate.as_ref().unwrap();
    assert!(w
        .ctx_gamma("customer-support")
        .iter()
        .any(|entry| entry.kind == "revoke" && entry.target.as_deref() == Some(old)));
}

#[then("the granting context gamma records the new grant and the revocation")]
async fn reenroll_gamma_story(w: &mut GatewayWorld) {
    let result = w.reenroll.as_ref().unwrap();
    let entries = w.ctx_gamma("customer-support");
    assert!(entries.iter().any(|entry| {
        entry.kind == "grant"
            && entry.target.as_deref() == Some(result.equipment.agent_mandate.as_str())
    }));
    assert!(entries.iter().any(|entry| {
        entry.kind == "revoke" && entry.target.as_deref() == w.old_agent_mandate.as_deref()
    }));
}

#[then("tools/list serves only the newly pinned schema")]
async fn tools_list_serves_reenrolled_schema(w: &mut GatewayWorld) {
    let router = w.router.as_ref().unwrap();
    w.last_response = Some(
        process_multi(
            router,
            json!({ "jsonrpc": "2.0", "id": 88, "method": "tools/list" }),
        )
        .await,
    );
    let listed = w.listed_tools();
    let issue = listed
        .iter()
        .find(|tool| tool["name"] == "github__issues_list")
        .unwrap();
    assert_eq!(
        issue["inputSchema"]["properties"]["page"]["type"],
        "integer"
    );
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

// ------------------------------------------ provisioning (Phase B, @wip)
// Stubs for gateway-provisioning.feature — implemented lot by lot.

#[when("a runner generates its agent identity")]
async fn runner_keygen(w: &mut GatewayWorld) {
    let dir = tempfile::tempdir().unwrap();
    let mut ent = SeqEntropy::default();
    let kh = Keyholder::from_entropy(ent.e32(), ent.e32());
    let path = dir.path().join("agent.id");
    kh.save(&path).expect("identity saved");
    // What the runner hands out at birth — public material only.
    w.published = Some(format!(
        "agent_pub: {}\ngateway_pub: {}",
        agent_pub_multibase(&kh),
        gateway_pub_multibase(&kh)
    ));
    w.identity_file = Some(path);
    w.scratch = Some(dir);
}

#[then("it publishes the agent public key")]
async fn pubkey_published(w: &mut GatewayWorld) {
    let published = w.published.as_ref().expect("something published");
    assert!(
        published.contains("agent_pub: z"),
        "multibase public key published (z…): {published}"
    );
}

#[then("the provision artifacts contain no seed material")]
async fn provision_has_no_seed(w: &mut GatewayWorld) {
    let published = w.published.as_ref().expect("something published");
    let identity: Value = serde_json::from_slice(
        &std::fs::read(w.identity_file.as_ref().expect("identity file")).unwrap(),
    )
    .unwrap();
    for k in ["agent_seed_hex", "gateway_seed_hex"] {
        let seed = identity[k].as_str().expect("seed stored for the runner");
        assert!(
            !published.contains(seed),
            "{k} must never appear in provision artifacts"
        );
    }
}

#[given("an enterprise master seed")]
async fn enterprise_master(w: &mut GatewayWorld) {
    w.master = Some([7u8; 32]);
}

#[when("the owner creates a journal for the agent's public key")]
async fn owner_creates_journal(w: &mut GatewayWorld) {
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let store = GatewayStore::in_memory();
    let mut ent = SeqEntropy::default();
    let outcome = owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("journal created");
    w.journal_store = Some(store);
    w.journal_outcome = Some(outcome);
}

#[then("the journal is an isolated Ethos owned by the enterprise")]
async fn journal_is_isolated(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    assert!(
        outcome.ethos_did.starts_with("did:"),
        "a full Ethos identity"
    );
    if let Some((_, _, _, _, Some(ctx))) = &w.ctx {
        assert_ne!(outcome.ethos_did, ctx.ethos_did, "isolated from contexts");
    }
    assert!(
        outcome.auditor_mandate.is_none(),
        "no audit grant by default"
    );
}

#[then("the agent holds a mandate to write its journal")]
async fn agent_holds_journal_mandate(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    let store = w.journal_store.clone().expect("journal store");
    let grantee = cert_grantee_pub(store, &outcome.agent_mandate).expect("cert readable");
    assert_eq!(Some(grantee), w.agent_pub, "mandate names the agent key");
}

#[then("the journal gamma records that a mandate was received")]
async fn journal_records_mandate(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    let entries = gamma_view(w.journal_store.clone().expect("store")).expect("gamma readable");
    assert!(
        entries
            .iter()
            .any(|e| e.kind == "grant" && e.target.as_deref() == Some(&outcome.agent_mandate)),
        "the agent grant is on the journal record"
    );
}

#[given(expr = "a context Ethos {string} with tools {string} and {string}")]
async fn context_ethos(w: &mut GatewayWorld, name: String, read: String, write: String) {
    let master = w.master();
    let store = GatewayStore::in_memory();
    let mut ent = SeqEntropy::default();
    owner_init_context(&master, &name, store.clone(), T0, &mut ent).expect("context created");
    w.ctx = Some((name, store, read, write, None));
}

#[when("the owner grants the agent read access to that context")]
async fn owner_grants_context(w: &mut GatewayWorld) {
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let (label, store, read, _, _) = w.ctx.as_ref().expect("a context").clone();
    let mut ent = SeqEntropy::default();
    let outcome = owner_grant_context(
        &master,
        &label,
        &agent_pub,
        &gateway_pub,
        std::slice::from_ref(&read),
        store,
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("context granted");
    w.ctx.as_mut().expect("a context").4 = Some(outcome);
}

#[then("the context gamma records the grant")]
async fn context_records_grant(w: &mut GatewayWorld) {
    let (_, store, _, _, outcome) = w.ctx.as_ref().expect("a context");
    let outcome = outcome.as_ref().expect("granted");
    let entries = gamma_view(store.clone()).expect("gamma readable");
    assert!(
        entries
            .iter()
            .any(|e| e.kind == "grant" && e.target.as_deref() == Some(&outcome.agent_mandate)),
        "the agent grant is on the context record"
    );
}

#[then("the granted certificate names the agent public key")]
async fn cert_names_agent_pub(w: &mut GatewayWorld) {
    let (_, store, _, _, outcome) = w.ctx.as_ref().expect("a context");
    let outcome = outcome.as_ref().expect("granted");
    let grantee = cert_grantee_pub(store.clone(), &outcome.agent_mandate).expect("cert readable");
    assert_eq!(Some(grantee), w.agent_pub);
}

/// Acts of one connector (`x.mcp` acts, `x.gateway` refusals, `x.xref`
/// mirrors) in an owner-side gamma view.
fn acts_on(entries: &[EntryView], target: &str) -> Vec<EntryView> {
    entries
        .iter()
        .filter(|e| e.kind == "action" && e.target.as_deref() == Some(target))
        .cloned()
        .collect()
}

/// A string field of an entry's clear payload.
fn payload_str<'a>(e: &'a EntryView, key: &str) -> Option<&'a str> {
    e.payload
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(Value::as_str)
}

#[given(expr = "a runner provisioned with contexts {string} and {string}")]
async fn runner_provisioned(w: &mut GatewayWorld, a: String, b: String) {
    provision_runner(w, a, b, false).await;
}

#[given("a runner whose journal predates the memory pen")]
async fn legacy_journal_runner(w: &mut GatewayWorld) {
    // Provision the modern way, then strip the memory pen from the
    // persisted runtime state BEFORE the journal bridge opens —
    // byte-for-byte what a pre-C2 journal hands a fresh runner.
    provision_runner(w, "company-brand".into(), "ui-designer".into(), true).await;
}

/// The provisioning walk shared by the modern and the legacy runners.
async fn provision_runner(w: &mut GatewayWorld, a: String, b: String, strip_memory_pen: bool) {
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let window = GatewayWorld::window();
    let mut owner_ent = SeqEntropy::default();

    // The runner's ONE identity — the same seeds behind the published
    // pubkeys (SeqEntropy is deterministic: a fresh sequence replays
    // them), shared by every bridge.
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Arc::new(Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32()));

    let mut contexts = BTreeMap::new();
    for label in [a, b] {
        let (read, write) = context_tools(&label);
        let store = GatewayStore::in_memory();
        owner_init_context(&master, &label, store.clone(), T0, &mut owner_ent)
            .expect("context created");
        let outcome = owner_grant_context(
            &master,
            &label,
            &agent_pub,
            &gateway_pub,
            std::slice::from_ref(&read),
            store.clone(),
            &window,
            T0,
            &mut owner_ent,
        )
        .expect("context granted");
        let bridge = Bridge::open(
            store.clone(),
            Arc::clone(&keyholder),
            Box::new(SeqEntropy::default()),
        )
        .expect("context bridge opens");
        let mut tools = ToolMap::new();
        tools.insert(read, ToolAccess::Read);
        tools.insert(write, ToolAccess::Write);
        w.ctx_stores.insert(label.clone(), store);
        w.ctx_dids.insert(label.clone(), outcome.ethos_did.clone());
        w.multi_upstreams.insert(
            label.clone(),
            FakeMcp::with_text(&format!("{label}-answer")),
        );
        contexts.insert(
            label,
            ContextRuntime {
                policy: Policy::new(tools),
                bridge,
            },
        );
    }

    let journal_store = GatewayStore::in_memory();
    let journal_outcome = owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &window,
        T0,
        &mut owner_ent,
    )
    .expect("journal created");
    if strip_memory_pen {
        let mut store = journal_store.clone();
        let bytes = store
            .get(STATE_PATH)
            .expect("state readable")
            .expect("state present");
        let mut state: Value = serde_json::from_slice(&bytes).expect("state parses");
        state
            .as_object_mut()
            .expect("state object")
            .remove("memory_mandate");
        store
            .put(STATE_PATH, &serde_json::to_vec_pretty(&state).unwrap())
            .expect("state written");
    }
    let journal = Bridge::open(
        journal_store.clone(),
        keyholder,
        Box::new(SeqEntropy::default()),
    )
    .expect("journal bridge opens");
    w.journal_store = Some(journal_store);
    w.journal_outcome = Some(journal_outcome);

    w.router = Some(Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(Runner::from_parts(contexts, journal))),
        upstreams: w.multi_upstreams.clone(),
        clock: Arc::new(|| T0.to_owned()),
    }));
}

#[then(expr = "the act on {string} is logged in the {string} gamma only")]
async fn act_logged_in_context_only(w: &mut GatewayWorld, tool: String, ctx: String) {
    let own = acts_on(&w.ctx_gamma(&ctx), "x.mcp");
    let hits: Vec<&EntryView> = own
        .iter()
        .filter(|e| payload_str(e, "tool") == Some(tool.as_str()))
        .collect();
    assert_eq!(hits.len(), 1, "exactly one act for `{tool}` in `{ctx}`");

    for other in w.ctx_stores.keys().filter(|n| n.as_str() != ctx.as_str()) {
        assert!(
            acts_on(&w.ctx_gamma(other), "x.mcp")
                .iter()
                .all(|e| payload_str(e, "tool") != Some(tool.as_str())),
            "`{tool}` must not leak into the `{other}` gamma"
        );
    }
    // The journal never copies acts — its mirror is the xref index.
    assert!(
        acts_on(&w.journal_gamma(), "x.mcp").is_empty(),
        "the journal holds xrefs, not act copies"
    );
    // And the call crossed to THAT context's upstream, no other.
    let named = |body: &Value| {
        body.pointer("/params/name")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    assert!(
        w.multi_upstreams[&ctx]
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|b| named(b).as_deref() == Some(tool.as_str())),
        "the call must reach the `{ctx}` upstream"
    );
    for (name, fake) in &w.multi_upstreams {
        if name.as_str() == ctx.as_str() {
            continue;
        }
        assert!(
            fake.seen
                .lock()
                .unwrap()
                .iter()
                .all(|b| named(b).as_deref() != Some(tool.as_str())),
            "`{tool}` must not reach the `{name}` upstream"
        );
    }
}

#[then("the journal holds one cross-reference per act, joinable both ways")]
async fn journal_xrefs_join(w: &mut GatewayWorld) {
    let xrefs = acts_on(&w.journal_gamma(), "x.xref");
    let mut total_acts = 0;
    for name in w.ctx_stores.keys() {
        let did = w.ctx_dids.get(name).expect("context did").as_str();
        for act in w.ctx_gamma(name).into_iter().filter(|entry| {
            entry.kind == "action"
                && entry.target.as_deref() != Some("x.gateway")
                && entry.target.as_deref() != Some("x.xref")
        }) {
            total_acts += 1;
            // Journal → context: the xref names the authoritative entry…
            let joined: Vec<&EntryView> = xrefs
                .iter()
                .filter(|x| {
                    payload_str(x, "ethos_did") == Some(did)
                        && payload_str(x, "entry_id") == Some(act.id.as_str())
                })
                .collect();
            // …context → journal: that act has exactly one mirror.
            assert_eq!(
                joined.len(),
                1,
                "exactly one xref joins act `{}` of `{name}`",
                act.id
            );
            assert_eq!(
                payload_str(joined[0], "tool"),
                payload_str(&act, "tool"),
                "the xref names the same tool as the act"
            );
        }
    }
    assert_eq!(xrefs.len(), total_acts, "one cross-reference per act");
}

#[then("the call never reaches any upstream")]
async fn no_upstream_reached(w: &mut GatewayWorld) {
    for (name, fake) in &w.multi_upstreams {
        assert!(
            fake.seen.lock().unwrap().is_empty(),
            "nothing may reach the `{name}` upstream"
        );
    }
}

#[then(expr = "the {string} gamma gains one refusal entry")]
async fn context_gains_refusal(w: &mut GatewayWorld, ctx: String) {
    let refusals = acts_on(&w.ctx_gamma(&ctx), "x.gateway");
    assert_eq!(refusals.len(), 1, "exactly one refusal in `{ctx}`");
    assert_eq!(
        payload_str(&refusals[0], "tool"),
        Some(w.last_tool.as_str()),
        "the refusal names the refused tool"
    );
    // The routing is surgical: the other contexts saw nothing.
    for other in w.ctx_stores.keys().filter(|n| n.as_str() != ctx.as_str()) {
        assert!(
            acts_on(&w.ctx_gamma(other), "x.gateway").is_empty(),
            "no refusal may leak into the `{other}` gamma"
        );
    }
}

#[then("the journal gains one refusal entry")]
async fn journal_gains_refusal(w: &mut GatewayWorld) {
    let refusals = acts_on(&w.journal_gamma(), "x.gateway");
    assert_eq!(refusals.len(), 1, "exactly one journal refusal");
    assert_eq!(
        payload_str(&refusals[0], "tool"),
        Some(w.last_tool.as_str()),
        "the journal refusal names the refused tool"
    );
}

#[then("no context gamma gains any entry")]
async fn no_context_entry(w: &mut GatewayWorld) {
    for name in w.ctx_stores.keys() {
        assert!(
            w.ctx_gamma(name).iter().all(|e| e.kind != "action"),
            "the `{name}` gamma must hold nothing beyond its provisioning record"
        );
    }
}

// --------------------------------------------- inference (Phase C, @wip)
// Steps for gateway-inference.feature — the LLM front of the gateway.

#[when(expr = "the owner creates a journal with a token budget of {int}")]
async fn owner_creates_budgeted_journal(w: &mut GatewayWorld, budget: u64) {
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let store = GatewayStore::in_memory();
    let mut ent = SeqEntropy::default();
    let outcome = owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        Some(budget),
        store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("budgeted journal created");
    w.journal_store = Some(store);
    w.journal_outcome = Some(outcome);
}

#[then(expr = "the agent holds an inference mandate carrying that token budget")]
async fn agent_holds_inference_mandate(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    let pen = outcome
        .inference_mandate
        .as_ref()
        .expect("an inference mandate was minted");
    let store = w.journal_store.clone().expect("journal store");
    let grantee = cert_grantee_pub(store.clone(), pen).expect("cert readable");
    assert_eq!(Some(grantee), w.agent_pub, "the pen names the agent key");
    let constraints = cert_constraints(store, pen).expect("cert readable");
    assert_eq!(
        constraints.pointer("/budgets/0/id").and_then(Value::as_str),
        Some("llm"),
        "the budget profile the gateway cites"
    );
    assert_eq!(
        constraints
            .pointer("/budgets/0/token_budget")
            .and_then(Value::as_u64),
        Some(1000),
        "the granted token budget rides the certificate"
    );
}

#[then("the journal gamma records that the inference mandate was received")]
async fn journal_records_inference_mandate(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    let pen = outcome
        .inference_mandate
        .as_ref()
        .expect("an inference mandate was minted");
    let entries = gamma_view(w.journal_store.clone().expect("store")).expect("gamma readable");
    assert!(
        entries
            .iter()
            .any(|e| e.kind == "grant" && e.target.as_deref() == Some(pen)),
        "the inference grant is on the journal record"
    );
}

#[given(expr = "a runner with an inference pen budgeted at {int} tokens")]
async fn runner_with_inference_pen(w: &mut GatewayWorld, budget: u64) {
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let mut owner_ent = SeqEntropy::default();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Arc::new(Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32()));

    let journal_store = GatewayStore::in_memory();
    let journal_outcome = owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        Some(budget),
        journal_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("budgeted journal created");
    let journal = Bridge::open(
        journal_store.clone(),
        keyholder,
        Box::new(SeqEntropy::default()),
    )
    .expect("journal bridge opens");
    w.journal_store = Some(journal_store);
    w.journal_outcome = Some(journal_outcome);

    // The LLM front rides the SAME runner shape as the MCP router — no
    // contexts are needed to talk to the provider.
    let provider = FakeLlm::new();
    w.llm_provider = Some(provider.clone());
    w.llm = Some(Arc::new(LlmProxy {
        runner: Arc::new(Mutex::new(Runner::from_parts(BTreeMap::new(), journal))),
        upstream: provider,
        model: "gpt-4o-imposed".to_owned(),
        provider: "openai-compat".to_owned(),
        clock: Arc::new(|| T0.to_owned()),
    }));
}

#[given("the provider omits usage from its answers")]
async fn provider_omits_usage(w: &mut GatewayWorld) {
    let provider = w.llm_provider.as_ref().expect("an LLM front");
    *provider.usage.lock().unwrap() = None;
}

#[given("the budget is already spent")]
async fn budget_already_spent(w: &mut GatewayWorld) {
    // The whole tap was legitimately consumed earlier (one metered call
    // that spent exactly the budget); this call finds nothing left.
    let llm = w.llm.clone().expect("an LLM front");
    let mut runner = llm.runner.lock().await;
    runner
        .record_inference("openai-compat", "gpt-4o-imposed", 400, 600, T0)
        .expect("the spending inference itself fits the budget");
}

#[given("the provider reports a usage larger than the remaining budget")]
async fn provider_reports_overrun(w: &mut GatewayWorld) {
    let provider = w.llm_provider.as_ref().expect("an LLM front");
    *provider.usage.lock().unwrap() = Some((800, 300));
}

#[when(expr = "the agent asks for a chat completion with model {string}")]
async fn agent_asks_completion(w: &mut GatewayWorld, model: String) {
    let llm = w.llm.clone().expect("an LLM front");
    let body = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": "the secret prompt words" }
        ]
    });
    w.last_tool = LLM_TOOL.to_owned();
    let (status, resp) = process_llm(&llm, body).await;
    w.llm_status = Some(status);
    w.last_response = Some(resp);
}

#[then("the provider is called with the configured model only")]
async fn provider_sees_imposed_model(w: &mut GatewayWorld) {
    let provider = w.llm_provider.as_ref().expect("an LLM front");
    let seen = provider.seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "exactly one provider call");
    assert_eq!(
        seen[0].get("model").and_then(Value::as_str),
        Some("gpt-4o-imposed"),
        "the agent's model choice is overwritten"
    );
}

#[then("the provider's answer comes back to the agent")]
async fn provider_answer_returns(w: &mut GatewayWorld) {
    assert_eq!(w.llm_status, Some(axum::http::StatusCode::OK));
    let resp = w.last_response.as_ref().expect("a response");
    let provider = w.llm_provider.as_ref().expect("an LLM front");
    assert_eq!(
        resp.pointer("/choices/0/message/content")
            .and_then(Value::as_str),
        Some(provider.answer.as_str())
    );
}

#[then("no agent-visible surface contains the provider credentials")]
async fn no_credential_agent_side(w: &mut GatewayWorld) {
    let key = &w.llm_provider.as_ref().expect("an LLM front").api_key;
    let surface = serde_json::to_string(w.last_response.as_ref().expect("a response")).unwrap();
    assert!(
        !surface.contains(key),
        "the provider credential leaked into the agent-facing response"
    );
}

#[then("no journal entry contains the provider credentials")]
async fn no_credential_in_journal(w: &mut GatewayWorld) {
    let key = &w.llm_provider.as_ref().expect("an LLM front").api_key;
    let journal = serde_json::to_string(&w.journal_gamma()).unwrap();
    assert!(
        !journal.contains(key),
        "the provider credential leaked into the journal"
    );
}

#[then("the journal gains one inference entry with the provider's reported usage")]
async fn journal_gains_inference(w: &mut GatewayWorld) {
    let inferences: Vec<EntryView> = w
        .journal_gamma()
        .into_iter()
        .filter(|e| e.kind == "inference")
        .collect();
    assert_eq!(inferences.len(), 1, "exactly one inference entry");
    let e = &inferences[0];
    assert_eq!(e.target.as_deref(), Some("x.llm"));
    let (t_in, t_out) = w
        .llm_provider
        .as_ref()
        .expect("an LLM front")
        .usage
        .lock()
        .unwrap()
        .expect("the provider reported usage");
    let payload = e.payload.as_ref().expect("clear payload");
    assert_eq!(payload["provider"], json!("openai-compat"));
    assert_eq!(payload["model"], json!("gpt-4o-imposed"));
    assert_eq!(payload["tokens_in"], json!(t_in), "the REAL usage, in");
    assert_eq!(payload["tokens_out"], json!(t_out), "the REAL usage, out");
    assert_eq!(payload["budget_ref"], json!("llm"), "the cited tap");
}

#[then("no journal entry contains the prompt or the completion text")]
async fn no_prompt_in_journal(w: &mut GatewayWorld) {
    let provider = w.llm_provider.as_ref().expect("an LLM front");
    let journal = serde_json::to_string(&w.journal_gamma()).unwrap();
    assert!(
        !journal.contains("the secret prompt words"),
        "the prompt leaked into the journal"
    );
    assert!(
        !journal.contains(&provider.answer),
        "the completion text leaked into the journal"
    );
}

#[then("the completion is withheld from the agent")]
async fn completion_withheld(w: &mut GatewayWorld) {
    let resp = w.last_response.as_ref().expect("a response");
    assert!(
        resp.get("error").is_some(),
        "the agent gets a refusal, not a completion: {resp}"
    );
    assert!(
        resp.get("choices").is_none(),
        "no completion content may leak through a refusal"
    );
}

#[then("the provider is never called")]
async fn provider_never_called(w: &mut GatewayWorld) {
    let provider = w.llm_provider.as_ref().expect("an LLM front");
    assert!(
        provider.seen.lock().unwrap().is_empty(),
        "nothing may reach the provider"
    );
}

#[then("the journal gains no inference entry")]
async fn journal_gains_no_inference(w: &mut GatewayWorld) {
    // The pre-spent inference of the exhausted-budget setup is not part
    // of the call under test; scenarios using this step spend nothing.
    assert!(
        w.journal_gamma()
            .iter()
            .filter(|e| e.kind == "inference")
            .count()
            == 0,
        "no inference may be metered for a withheld completion"
    );
}

// ------------------------------------------------ journal tools (lot C2)
// Steps for gateway-journal.feature — the agent's memory consolidation:
// sealed sections under the memory pen, recalls journalized read by read.

impl GatewayWorld {
    /// Drive one native journal tool through the multi-context router —
    /// the same black-box door every agent call takes.
    async fn journal_call(&mut self, tool: &str, args: Value) {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": { "name": tool, "arguments": args }
        });
        self.last_tool = tool.to_owned();
        let router = self.router.as_ref().expect("a provisioned router");
        self.last_response = Some(process_multi(router, body).await);
    }

    /// How many journal gamma entries carry this kind.
    fn journal_kind_count(&self, kind: &str) -> usize {
        self.journal_gamma()
            .iter()
            .filter(|e| e.kind == kind)
            .count()
    }

    /// The JSON text content of the last tools/call answer.
    fn last_result_text(&self) -> String {
        self.last_response
            .as_ref()
            .and_then(|r| r.pointer("/result/content/0/text"))
            .and_then(Value::as_str)
            .expect("a result with text content")
            .to_owned()
    }

    /// The tool entries of the last tools/list answer — preferring the
    /// separately captured one when a combined step listed before calling.
    fn listed_tools(&self) -> Vec<Value> {
        self.last_list
            .as_ref()
            .or(self.last_response.as_ref())
            .and_then(|r| r.pointer("/result/tools"))
            .and_then(Value::as_array)
            .expect("tools listed")
            .clone()
    }
}

#[then("the agent holds a memory pen separate from the xref pen")]
async fn agent_holds_memory_pen(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    let memory = outcome.memory_mandate.as_ref().expect("a memory pen");
    assert_ne!(
        memory, &outcome.agent_mandate,
        "one pen per usage — never the xref mandate widened"
    );
    let store = w.journal_store.clone().expect("journal store");
    let grantee = cert_grantee_pub(store, memory).expect("cert readable");
    assert_eq!(Some(grantee), w.agent_pub, "the pen names the agent key");
}

#[then("the journal gamma records that the memory pen was received")]
async fn journal_records_memory_pen(w: &mut GatewayWorld) {
    let outcome = w.journal_outcome.as_ref().expect("journal outcome");
    let memory = outcome.memory_mandate.as_ref().expect("a memory pen");
    let entries = gamma_view(w.journal_store.clone().expect("store")).expect("gamma readable");
    assert!(
        entries
            .iter()
            .any(|e| e.kind == "grant" && e.target.as_deref() == Some(memory.as_str())),
        "the memory grant is on the journal record"
    );
}

#[when(expr = "the agent writes a note titled {string} with text {string} and tag {string}")]
async fn agent_writes_note(w: &mut GatewayWorld, title: String, text: String, tag: String) {
    w.journal_call(
        JOURNAL_WRITE,
        json!({ "title": title, "text": text, "tags": [tag] }),
    )
    .await;
}

#[when("the agent writes a note carrying an unknown argument field")]
async fn agent_writes_unknown_field(w: &mut GatewayWorld) {
    w.journal_call(JOURNAL_WRITE, json!({ "text": "x", "surprise": true }))
        .await;
}

#[when("the agent writes a note with an empty text")]
async fn agent_writes_empty_text(w: &mut GatewayWorld) {
    w.journal_call(JOURNAL_WRITE, json!({ "title": "t", "text": "  " }))
        .await;
}

#[then(expr = "the owner reads back one memory note titled {string} with text {string}")]
async fn owner_reads_back_note(w: &mut GatewayWorld, title: String, text: String) {
    let master = w.master();
    let store = w.journal_store.clone().expect("journal store");
    let notes = journal_notes_view(store.clone()).expect("clear index readable");
    let hits: Vec<_> = notes.iter().filter(|n| n.title == title).collect();
    assert_eq!(hits.len(), 1, "one note titled `{title}` on the shelf");
    let body = owner_read_journal_note(&master, "leo", store, &hits[0].name)
        .expect("the owner opens its agent's memory");
    assert_eq!(body, text, "the sealed body reads back owner-side");
}

#[then(expr = "the journal gamma logs one delegated {string} with a sealed body")]
async fn journal_logs_one_sealed_mutation(w: &mut GatewayWorld, kind: String) {
    let entries: Vec<EntryView> = w
        .journal_gamma()
        .into_iter()
        .filter(|e| e.kind == kind)
        .collect();
    assert_eq!(entries.len(), 1, "exactly one `{kind}`");
    let e = &entries[0];
    assert!(
        e.target.is_none() && e.payload.is_none(),
        "target and payload sealed — the keyless learn nothing"
    );
    let memory = w
        .journal_outcome
        .as_ref()
        .expect("outcome")
        .memory_mandate
        .clone()
        .expect("a memory pen");
    assert_eq!(
        e.authorized_via,
        Some(vec![memory]),
        "the mutation cites the memory pen"
    );
}

#[then("the answer names the recorded note")]
async fn answer_names_note(w: &mut GatewayWorld) {
    let v: Value = serde_json::from_str(&w.last_result_text()).expect("JSON answer");
    let name = v
        .pointer("/recorded/name")
        .and_then(Value::as_str)
        .expect("a note name in the answer");
    assert!(name.starts_with("n-"), "a fresh technical name: {name}");
    assert!(
        v.pointer("/recorded/path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .starts_with("memory/"),
        "the answer names the shelf path"
    );
}

#[then(expr = "the journal gamma logs no {string}")]
async fn journal_logs_no_kind(w: &mut GatewayWorld, kind: String) {
    assert_eq!(w.journal_kind_count(&kind), 0, "no `{kind}` entry");
}

#[given(expr = "the journal holds a note titled {string} with text {string}")]
async fn journal_holds_note_text(w: &mut GatewayWorld, title: String, text: String) {
    w.journal_call(JOURNAL_WRITE, json!({ "title": title, "text": text }))
        .await;
    let resp = w.last_response.as_ref().unwrap();
    assert!(resp.get("error").is_none(), "setup write must pass: {resp}");
}

#[given(expr = "the journal holds a note titled {string} tagged {string}")]
async fn journal_holds_note_tag(w: &mut GatewayWorld, title: String, tag: String) {
    w.journal_call(
        JOURNAL_WRITE,
        json!({ "title": title, "text": format!("body of {title}"), "tags": [tag] }),
    )
    .await;
    let resp = w.last_response.as_ref().unwrap();
    assert!(resp.get("error").is_none(), "setup write must pass: {resp}");
}

#[when(expr = "the agent searches the journal for {string}")]
async fn agent_searches(w: &mut GatewayWorld, query: String) {
    w.journal_call(JOURNAL_SEARCH, json!({ "query": query }))
        .await;
}

#[when(expr = "the agent searches the journal for tag {string}")]
async fn agent_searches_tag(w: &mut GatewayWorld, tag: String) {
    w.journal_call(JOURNAL_SEARCH, json!({ "tag": tag })).await;
}

#[then(expr = "the answer carries the note titled {string} only")]
async fn answer_carries_only(w: &mut GatewayWorld, title: String) {
    let v: Value = serde_json::from_str(&w.last_result_text()).expect("JSON answer");
    let hits = v["hits"].as_array().expect("hits");
    assert_eq!(hits.len(), 1, "exactly one hit: {v}");
    assert_eq!(hits[0]["title"].as_str(), Some(title.as_str()));
    assert_eq!(v["total"].as_u64(), Some(1));
}

#[then(expr = "its text {string} comes back with it")]
async fn answer_hit_has_text(w: &mut GatewayWorld, text: String) {
    let v: Value = serde_json::from_str(&w.last_result_text()).expect("JSON answer");
    assert_eq!(
        v["hits"][0]["text"].as_str(),
        Some(text.as_str()),
        "the opened body rides with the hit"
    );
}

#[then("the answer carries no note")]
async fn answer_carries_none(w: &mut GatewayWorld) {
    let v: Value = serde_json::from_str(&w.last_result_text()).expect("JSON answer");
    assert_eq!(v["total"].as_u64(), Some(0));
    assert!(v["hits"].as_array().expect("hits").is_empty());
}

#[then(expr = "the journal gamma logs exactly one {string}")]
async fn journal_logs_exactly_one(w: &mut GatewayWorld, kind: String) {
    assert_eq!(w.journal_kind_count(&kind), 1, "exactly one `{kind}`");
}

#[when("the agent lists the tools")]
async fn agent_lists_tools(w: &mut GatewayWorld) {
    let body = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" });
    let response = if let Some(vault) = &w.vault {
        process_multi(&vault.router, body).await
    } else {
        let router = w.router.as_ref().expect("a provisioned router");
        process_multi(router, body).await
    };
    if let Some(vault) = &mut w.vault {
        vault.responses.push(response.clone());
    }
    w.last_response = Some(response);
}

#[then(expr = "the list includes {string} and {string} with their argument schemas")]
async fn list_includes_native(w: &mut GatewayWorld, write_name: String, search_name: String) {
    let tools = w.listed_tools();
    let find = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("`{name}` must be listed"))
            .clone()
    };
    let write = find(&write_name);
    assert_eq!(
        write
            .pointer("/inputSchema/required/0")
            .and_then(Value::as_str),
        Some("text"),
        "journal.write pins its required field"
    );
    assert_eq!(
        write.pointer("/inputSchema/additionalProperties"),
        Some(&json!(false)),
        "unknown fields are pinned closed at the surface too"
    );
    let search = find(&search_name);
    assert!(
        search.pointer("/inputSchema/properties/query").is_some()
            && search.pointer("/inputSchema/properties/tag").is_some(),
        "journal.search names its arguments"
    );
    assert_eq!(
        search.pointer("/inputSchema/additionalProperties"),
        Some(&json!(false))
    );
}

#[then("the context tools keep their open schemas")]
async fn context_tools_open_schema(w: &mut GatewayWorld) {
    let tools = w.listed_tools();
    let brand = tools
        .iter()
        .find(|t| t["name"].as_str() == Some("brand.read"))
        .expect("brand.read listed");
    assert_eq!(
        brand["inputSchema"],
        json!({ "type": "object" }),
        "relayed tools keep the honest open object (schemas pinned per-tool land with the hub)"
    );
}

#[when(expr = "a config maps the context tool {string}")]
async fn config_maps_reserved(w: &mut GatewayWorld, tool: String) {
    let yaml = format!(
        "\
listen: 127.0.0.1:4870
contexts:
  - name: company-brand
    upstream_mcp: http://127.0.0.1:5001/mcp
    store: {{ kind: fs, root: /var/lib/aithos/brand }}
    tools:
      {tool}: read
journal:
  store: {{ kind: fs, root: /var/lib/aithos/journal }}
"
    );
    w.config_error = Some(
        GatewayConfig::from_yaml(&yaml)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default(),
    );
}

#[then("the config is rejected naming the reserved prefix")]
async fn config_rejected_reserved(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("a parse verdict");
    assert!(!err.is_empty(), "the config must be rejected, not accepted");
    assert!(
        err.contains("reserved"),
        "the rejection names the reservation: {err}"
    );
}

// ------------------------------------------- governed hub enrollment (H2)

#[given(expr = "MCP server {string} advertises tools {string} and {string}")]
async fn mcp_server_advertises_hub_tools(
    w: &mut GatewayWorld,
    server: String,
    first: String,
    second: String,
) {
    w.hub_server = Some(server);
    w.upstream = FakeMcp::advertising(vec![
        json!({
            "name": first,
            "description": "List the repository issues",
            "inputSchema": {
                "type": "object",
                "properties": { "state": { "type": "string" } },
                "additionalProperties": false
            }
        }),
        json!({
            "name": second,
            "description": "Create one repository issue",
            "inputSchema": {
                "type": "object",
                "properties": { "title": { "type": "string" } },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
    ]);
}

#[when(expr = "the owner discovers {string} and approves each tool's risk class")]
async fn owner_discovers_and_approves(w: &mut GatewayWorld, server: String) {
    assert_eq!(w.hub_server.as_deref(), Some(server.as_str()));
    let proposed = discover_server(&server, &w.upstream)
        .await
        .expect("discovery succeeds");
    let approvals = proposed
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), ToolApproval::class(ToolAccess::Read)))
        .collect();
    let approved = approve_manifest(&proposed, &approvals).expect("owner approval succeeds");

    let label = "hub-granting-context".to_owned();
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let store = GatewayStore::in_memory();
    let mut ent = SeqEntropy::default();
    owner_init_context(&master, &label, store.clone(), T0, &mut ent).expect("context created");
    let outcome = owner_enroll_server(
        &master,
        &label,
        &agent_pub,
        &gateway_pub,
        &approved,
        store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("server enrolled");
    w.ctx = Some((label, store, String::new(), String::new(), Some(outcome)));
    w.approved_manifest = Some(approved);
}

#[then("the approved manifest pins each tool's name, description and input schema")]
async fn approved_manifest_is_pinned(w: &mut GatewayWorld) {
    let (label, store, _, _, _) = w.ctx.as_ref().expect("enrolled context");
    let approved = w.approved_manifest.as_ref().expect("approved manifest");
    let pinned = owner_read_hub_manifest(
        &w.master.expect("enterprise master"),
        label,
        &approved.server,
        store.clone(),
    )
    .expect("owner opens pinned manifest");
    assert_eq!(&pinned, approved, "the exact approved fields are pinned");
    assert!(pinned.tools.iter().all(|tool| {
        !tool.name.is_empty()
            && tool.description.is_some()
            && tool.input_schema.is_object()
            && tool.pin_sha256.starts_with("sha256:")
    }));

    // Store custody alone reveals no upstream-controlled prompt text.
    let sealed = store
        .clone()
        .get(&format!("e/x/{}/manifest.enc", approved.server))
        .expect("vault readable")
        .expect("sealed manifest present");
    let visible = String::from_utf8_lossy(&sealed);
    assert!(!visible.contains("repository issues"));
}

#[then("the agent receives a mandate covering the approved exposed actions")]
async fn agent_mandate_covers_approved_actions(w: &mut GatewayWorld) {
    let (_, store, _, _, outcome) = w.ctx.as_ref().expect("enrolled context");
    let outcome = outcome.as_ref().expect("equip outcome");
    let perimeter =
        cert_perimeter(store.clone(), &outcome.agent_mandate).expect("agent certificate readable");
    assert_eq!(
        perimeter,
        vec![
            "act.x.github.issues_create".to_owned(),
            "act.x.github.issues_list".to_owned(),
        ],
        "the approved read-class actions become the exact mandate perimeter"
    );
}

#[then("the granting context gamma records the grant")]
async fn granting_context_records_hub_grant(w: &mut GatewayWorld) {
    let (_, store, _, _, outcome) = w.ctx.as_ref().expect("enrolled context");
    let outcome = outcome.as_ref().expect("equip outcome");
    let entries = gamma_view(store.clone()).expect("gamma readable");
    assert!(entries.iter().any(|entry| {
        entry.kind == "grant" && entry.target.as_deref() == Some(&outcome.agent_mandate)
    }));
}

// ------------------------------------------------ governed hub config (H1)

fn remember_config_verdict(w: &mut GatewayWorld, yaml: &str) {
    w.config_error = Some(
        GatewayConfig::from_yaml(yaml)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default(),
    );
}

#[when(expr = "a hub config declares server {string}")]
async fn hub_declares_server(w: &mut GatewayWorld, server: String) {
    let yaml = format!(
        "\
listen: 127.0.0.1:4870
servers:
  - name: {server}
    transport: http
    url: https://example.test/mcp
contexts:
  - name: engineering
    store: {{ kind: fs, root: /var/lib/aithos/engineering }}
    tools: {{}}
journal:
  store: {{ kind: fs, root: /var/lib/aithos/journal }}
"
    );
    remember_config_verdict(w, &yaml);
}

#[then("the config is rejected naming the reserved server name")]
async fn config_rejected_reserved_server(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("a parse verdict");
    assert!(
        err.contains("reserved server name"),
        "the rejection names the reserved server: {err}"
    );
}

#[given(expr = "server {string} advertises tool {string}")]
async fn server_advertises_tool(w: &mut GatewayWorld, server: String, tool: String) {
    w.hub_server = Some(server);
    w.hub_tool = Some(tool);
}

#[when(expr = "contexts {string} and {string} both grant that upstream tool")]
async fn contexts_grant_same_tool(w: &mut GatewayWorld, first: String, second: String) {
    let server = w.hub_server.as_deref().expect("advertised server");
    let tool = w.hub_tool.as_deref().expect("advertised tool");
    let yaml = format!(
        "\
listen: 127.0.0.1:4870
servers:
  - name: {server}
    transport: http
    url: https://example.test/mcp
contexts:
  - name: {first}
    store: {{ kind: fs, root: /var/lib/aithos/first }}
    tools:
      first-route: {{ server: {server}, tool: {tool}, access: read }}
  - name: {second}
    store: {{ kind: fs, root: /var/lib/aithos/second }}
    tools:
      second-route: {{ server: {server}, tool: {tool}, access: read }}
journal:
  store: {{ kind: fs, root: /var/lib/aithos/journal }}
"
    );
    remember_config_verdict(w, &yaml);
}

#[then("the config is rejected as an ambiguous context route")]
async fn config_rejected_ambiguous_route(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("a parse verdict");
    assert!(
        err.contains("ambiguous context route"),
        "the rejection names the route ambiguity: {err}"
    );
}

#[given(expr = "server {string} grants raw tool {string}")]
async fn server_grants_raw_tool(w: &mut GatewayWorld, server: String, tool: String) {
    w.hub_server = Some(server);
    w.hub_tool = Some(tool);
}

#[when(expr = "that server also grants raw tool {string}")]
async fn same_server_grants_second_raw_tool(w: &mut GatewayWorld, second_tool: String) {
    let server = w.hub_server.as_deref().expect("first server");
    let first_tool = w.hub_tool.as_deref().expect("first raw tool");
    let yaml = format!(
        "\
listen: 127.0.0.1:4870
servers:
  - name: {server}
    transport: http
    url: https://example.test/mcp
contexts:
  - name: engineering
    store: {{ kind: fs, root: /var/lib/aithos/engineering }}
    tools:
      first-route: {{ server: {server}, tool: {first_tool}, access: read }}
      second-route: {{ server: {server}, tool: {second_tool}, access: read }}
journal:
  store: {{ kind: fs, root: /var/lib/aithos/journal }}
"
    );
    remember_config_verdict(w, &yaml);
}

#[when(expr = "a hub config also declares server {string} granting raw tool {string}")]
async fn hub_adds_colliding_server(
    w: &mut GatewayWorld,
    second_server: String,
    second_tool: String,
) {
    let first_server = w.hub_server.as_deref().expect("first server");
    let first_tool = w.hub_tool.as_deref().expect("first raw tool");
    let yaml = format!(
        "\
listen: 127.0.0.1:4870
servers:
  - name: {first_server}
    transport: http
    url: https://first.example/mcp
  - name: {second_server}
    transport: http
    url: https://second.example/mcp
contexts:
  - name: engineering
    store: {{ kind: fs, root: /var/lib/aithos/engineering }}
    tools:
      first-route: {{ server: {first_server}, tool: {first_tool}, access: read }}
      second-route: {{ server: {second_server}, tool: {second_tool}, access: read }}
journal:
  store: {{ kind: fs, root: /var/lib/aithos/journal }}
"
    );
    remember_config_verdict(w, &yaml);
}

#[then("the config is rejected naming the exposed-name collision")]
async fn config_rejected_exposed_collision(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("a parse verdict");
    assert!(
        err.contains("exposed-name collision"),
        "the rejection names the flattened exposed-name collision: {err}"
    );
}

// ------------------------------------------- vault credential broker (V1)

#[when(
    "a hub config gives one server both a vault credential reference and an inline bearer_token"
)]
async fn config_declares_double_credential_source(w: &mut GatewayWorld) {
    let yaml = "\
listen: 127.0.0.1:4870
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
servers:
  - name: github
    transport: http
    url: https://mcp.github.example/mcp
    bearer_token: inline-legacy-secret
    credential:
      broker: enterprise
      path: aithos/mcp/github
      field: token
contexts:
  - name: customer-support
    store: { kind: fs, root: /var/lib/aithos/support }
    tools:
      github__issues_list: { server: github, tool: issues.list, access: read }
journal:
  store: { kind: fs, root: /var/lib/aithos/journal }
";
    remember_config_verdict(w, yaml);
}

#[then("the config is rejected naming the double credential source")]
async fn config_rejected_double_credential_source(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("a parse verdict");
    assert!(
        err.contains("both `credential` and `bearer_token`")
            && err.contains("one credential source"),
        "the rejection names the double credential source: {err}"
    );
}

// -------------------------------------------- vault runtime harness (V2)
//
// These scenarios exercise the REAL pieces end to end inside the test
// process: the real `VaultKv2Broker` (reqwest) against a fake KV v2
// HTTP server, the real `HttpUpstream` against fake MCP servers that
// record the Authorization header, and the real router — so "the
// upstream saw the bearer" is observed on an actual wire, and "zero
// vault hits" is a counted fact, not an inference.

/// Fake HashiCorp Vault KV v2: strict token check, per-path secrets,
/// optional malformed answer, and observability (paths hit, tokens
/// seen, context-act count at each hit — the log-before-resolve proof).
#[derive(Clone, Default)]
struct FakeVault {
    expected_token: String,
    secrets: Arc<StdMutex<BTreeMap<String, BTreeMap<String, String>>>>,
    malformed: Arc<StdMutex<Option<String>>>,
    hits: Arc<StdMutex<Vec<String>>>,
    tokens: Arc<StdMutex<Vec<Option<String>>>>,
    acts_at_hit: Arc<StdMutex<Vec<usize>>>,
    #[allow(clippy::type_complexity)]
    acts_probe: Arc<StdMutex<Option<Box<dyn Fn() -> usize + Send + Sync>>>>,
}

async fn spawn_fake_vault(fake: FakeVault) -> u16 {
    use axum::extract::{Path as AxumPath, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use axum::{Json, Router};

    let app = Router::new()
        .route(
            "/v1/secret/data/{*path}",
            get(
                |State(fake): State<FakeVault>,
                 AxumPath(path): AxumPath<String>,
                 headers: HeaderMap| async move {
                    let token = headers
                        .get("x-vault-token")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    fake.tokens.lock().unwrap().push(token.clone());
                    fake.hits.lock().unwrap().push(path.clone());
                    if let Some(probe) = fake.acts_probe.lock().unwrap().as_ref() {
                        let count = probe();
                        fake.acts_at_hit.lock().unwrap().push(count);
                    }
                    if token.as_deref() != Some(fake.expected_token.as_str()) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({ "errors": ["permission denied"] })),
                        );
                    }
                    if fake.malformed.lock().unwrap().as_deref() == Some(path.as_str()) {
                        return (StatusCode::OK, Json(json!({ "data": "not-a-kv2-secret" })));
                    }
                    match fake.secrets.lock().unwrap().get(&path) {
                        Some(fields) => (
                            StatusCode::OK,
                            Json(json!({
                                "data": { "data": fields, "metadata": { "version": 1 } }
                            })),
                        ),
                        None => (StatusCode::NOT_FOUND, Json(json!({ "errors": [] }))),
                    }
                },
            ),
        )
        .with_state(fake);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("fake vault binds");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

/// Fake MCP server on a real socket: records every body AND the
/// Authorization header the gateway put on the wire.
#[derive(Clone)]
struct WireMcp {
    requests: Arc<StdMutex<Vec<Value>>>,
    auths: Arc<StdMutex<Vec<Option<String>>>>,
    answer: String,
}

impl WireMcp {
    fn new(answer: &str) -> Self {
        Self {
            requests: Arc::default(),
            auths: Arc::default(),
            answer: answer.to_owned(),
        }
    }
}

async fn spawn_wire_mcp(fake: WireMcp) -> u16 {
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::routing::post;
    use axum::{Json, Router};

    let app = Router::new()
        .route(
            "/mcp",
            post(
                |State(fake): State<WireMcp>, headers: HeaderMap, Json(body): Json<Value>| async move {
                    fake.requests.lock().unwrap().push(body.clone());
                    fake.auths.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned),
                    );
                    let id = body.get("id").cloned().unwrap_or(Value::Null);
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": fake.answer }],
                            "isError": false
                        }
                    }))
                },
            ),
        )
        .with_state(fake);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("wire mcp binds");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

/// One enrolled server of the vault harness.
struct VaultServerSpec {
    server: &'static str,
    context: &'static str,
    read_raw: &'static str,
    write_raw: Option<&'static str>,
    path: String,
    field: String,
    answer: &'static str,
}

impl VaultServerSpec {
    fn github(path: &str, field: &str) -> Self {
        Self {
            server: "github",
            context: "customer-support",
            read_raw: "issues.list",
            write_raw: Some("issues.create"),
            path: path.to_owned(),
            field: field.to_owned(),
            answer: "github-ok",
        }
    }

    fn linear() -> Self {
        Self {
            server: "linear",
            context: "operations",
            read_raw: "tickets.list",
            write_raw: None,
            path: "aithos/mcp/linear".to_owned(),
            field: "token".to_owned(),
            answer: "linear-ok",
        }
    }
}

/// The live vault world: the router under test, both fakes, and every
/// agent-facing response captured for the non-leak scans.
struct VaultHarness {
    router: Arc<McpRouter<aithos_gateway::proxy_mcp::HttpUpstream>>,
    vault: FakeVault,
    wires: BTreeMap<String, WireMcp>,
    config_path: std::path::PathBuf,
    config_text: String,
    vault_token: String,
    store_roots: Vec<std::path::PathBuf>,
    responses: Vec<Value>,
}

async fn provision_vault_hub(w: &mut GatewayWorld, specs: Vec<VaultServerSpec>, vault_down: bool) {
    use aithos_gateway::credentials::build_brokers;
    use aithos_gateway::proxy_mcp::HttpUpstream;

    if w.vault.is_some() {
        return;
    }
    static VAULT_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = VAULT_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let env_name = format!("AITHOS_CUCUMBER_VAULT_TOKEN_{seq}");
    let vault_token = format!("vault-access-cucumber-{seq}");
    std::env::set_var(&env_name, &vault_token);

    let dir = tempfile::tempdir().expect("vault tempdir");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let mut owner_ent = SeqEntropy::default();

    let fake_vault = FakeVault {
        expected_token: vault_token.clone(),
        secrets: Arc::new(StdMutex::new(w.vault_pending.clone())),
        malformed: Arc::new(StdMutex::new(w.vault_malformed.clone())),
        ..FakeVault::default()
    };
    let vault_port = if vault_down {
        // A port nothing serves: the broker meets connection-refused.
        let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = dead.local_addr().unwrap().port();
        drop(dead);
        port
    } else {
        spawn_fake_vault(fake_vault.clone()).await
    };

    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let journal_root = dir.path().join("journal");
    let journal_store = GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
        root: journal_root.clone(),
    })
    .expect("journal store");

    let mut yaml_servers = String::new();
    let mut yaml_contexts = String::new();
    let mut wires = BTreeMap::new();
    let mut store_roots = vec![journal_root.clone()];
    for spec in &specs {
        let root = dir.path().join(spec.context);
        let store = GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
            root: root.clone(),
        })
        .expect("context store");
        owner_init_context(&master, spec.context, store.clone(), T0, &mut owner_ent)
            .expect("vault context created");
        let mut advertised = vec![json!({
            "name": spec.read_raw,
            "description": "Read half of the vault harness",
            "inputSchema": { "type": "object", "additionalProperties": false }
        })];
        let mut approvals = BTreeMap::from([(
            spec.read_raw.to_owned(),
            ToolApproval::class(ToolAccess::Read),
        )]);
        if let Some(write_raw) = spec.write_raw {
            advertised.push(json!({
                "name": write_raw,
                "description": "Write half of the vault harness",
                "inputSchema": { "type": "object", "additionalProperties": false }
            }));
            approvals.insert(write_raw.to_owned(), ToolApproval::class(ToolAccess::Write));
        }
        let discovery = FakeMcp::advertising(advertised);
        let proposed = discover_server(spec.server, &discovery)
            .await
            .expect("vault discovery");
        let approved = approve_manifest(&proposed, &approvals).expect("vault approval");
        owner_enroll_server(
            &master,
            spec.context,
            &agent_pub,
            &gateway_pub,
            &approved,
            store.clone(),
            &GatewayWorld::window(),
            T0,
            &mut owner_ent,
        )
        .expect("vault enrollment");
        w.ctx_stores.insert(spec.context.to_owned(), store);
        store_roots.push(root.clone());

        let wire = WireMcp::new(spec.answer);
        let port = spawn_wire_mcp(wire.clone()).await;
        wires.insert(spec.server.to_owned(), wire);

        yaml_servers.push_str(&format!(
            "  - name: {}\n    transport: http\n    url: http://127.0.0.1:{}/mcp\n    credential:\n      broker: enterprise\n      path: {}\n      field: {}\n",
            spec.server, port, spec.path, spec.field
        ));
        yaml_contexts.push_str(&format!(
            "  - name: {}\n    store: {{ kind: fs, root: {} }}\n    tools:\n",
            spec.context,
            quote(&root)
        ));
        yaml_contexts.push_str(&format!(
            "      {}: {{ server: {}, tool: {}, access: read }}\n",
            aithos_gateway::policy::hub_exposed_name(spec.server, spec.read_raw),
            spec.server,
            spec.read_raw
        ));
        if let Some(write_raw) = spec.write_raw {
            yaml_contexts.push_str(&format!(
                "      {}: {{ server: {}, tool: {}, access: write }}\n",
                aithos_gateway::policy::hub_exposed_name(spec.server, write_raw),
                spec.server,
                write_raw
            ));
        }
    }
    owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("vault journal");

    let config_text = format!(
        "listen: 127.0.0.1:4870\ncredential_brokers:\n  enterprise:\n    kind: vault-kv2\n    address: http://127.0.0.1:{vault_port}\n    mount: secret\n    auth:\n      kind: token-env\n      env: {env_name}\nservers:\n{yaml_servers}contexts:\n{yaml_contexts}journal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(&journal_root)
    );
    let config_path = dir.path().join("gateway.yaml");
    std::fs::write(&config_path, &config_text).expect("config written");
    let cfg = GatewayConfig::from_yaml(&config_text).expect("vault hub config parses");
    let brokers = build_brokers(&cfg).expect("brokers build");
    let upstreams: BTreeMap<String, HttpUpstream> = cfg
        .servers
        .as_ref()
        .unwrap()
        .iter()
        .map(|server| {
            (
                server.name.clone(),
                HttpUpstream::for_server(server, &brokers).expect("upstream wires"),
            )
        })
        .collect();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner = Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
        .expect("vault governed runner");

    // The ordering probe: at every vault hit, how many acts the first
    // context's gamma already holds — log-before-resolve, observed.
    {
        let store = w
            .ctx_stores
            .get(specs[0].context)
            .expect("probe store")
            .clone();
        let target = format!("x.{}", specs[0].server);
        *fake_vault.acts_probe.lock().unwrap() = Some(Box::new(move || {
            gamma_view(store.clone())
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|entry| {
                            entry.kind == "action"
                                && entry.target.as_deref() == Some(target.as_str())
                        })
                        .count()
                })
                .unwrap_or(0)
        }));
    }

    w.journal_store = Some(journal_store);
    w.vault = Some(VaultHarness {
        router: Arc::new(McpRouter {
            runner: Arc::new(Mutex::new(runner)),
            upstreams,
            clock: Arc::new(|| T0.to_owned()),
        }),
        vault: fake_vault,
        wires,
        config_path,
        config_text,
        vault_token,
        store_roots,
        responses: Vec::new(),
    });
    w.scratch = Some(dir);
}

impl GatewayWorld {
    fn vault_harness(&self) -> &VaultHarness {
        self.vault.as_ref().expect("a provisioned vault harness")
    }

    /// Every string surface the agent saw in this scenario.
    fn vault_agent_text(&self) -> String {
        self.vault_harness()
            .responses
            .iter()
            .map(|response| response.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every gamma and journal entry, debug-rendered (headers, targets,
    /// payloads) — the "logged text" surface of the non-leak scans.
    fn vault_logged_text(&self) -> String {
        let mut out = String::new();
        for store in self.ctx_stores.values() {
            for entry in gamma_view(store.clone()).expect("context gamma readable") {
                out.push_str(&format!("{entry:?}\n"));
            }
        }
        for entry in self.journal_gamma() {
            out.push_str(&format!("{entry:?}\n"));
        }
        out
    }

    fn wire_auths(&self, server: &str) -> Vec<Option<String>> {
        self.vault_harness()
            .wires
            .get(server)
            .expect("a wired upstream")
            .auths
            .lock()
            .unwrap()
            .clone()
    }
}

/// Recursive scan: no file under these roots may contain the needle.
fn files_exclude(root: &std::path::Path, needle: &str) {
    for entry in std::fs::read_dir(root).expect("store root readable") {
        let entry = entry.expect("dir entry");
        if entry.file_type().expect("file type").is_dir() {
            files_exclude(&entry.path(), needle);
        } else {
            let bytes = std::fs::read(entry.path()).expect("file readable");
            assert!(
                !String::from_utf8_lossy(&bytes).contains(needle),
                "credential leaked into {}",
                entry.path().display()
            );
        }
    }
}

#[given(expr = "a vault stores {string} under path {string} field {string}")]
async fn vault_stores(w: &mut GatewayWorld, value: String, path: String, field: String) {
    w.vault_pending
        .entry(path)
        .or_default()
        .insert(field, value);
}

#[given(expr = "the vault also stores {string} under path {string} field {string}")]
async fn vault_also_stores(w: &mut GatewayWorld, value: String, path: String, field: String) {
    w.vault_pending
        .entry(path)
        .or_default()
        .insert(field, value);
}

#[given(expr = "a vault answers path {string} with a payload that is not a KV v2 secret")]
async fn vault_answers_malformed(w: &mut GatewayWorld, path: String) {
    w.vault_malformed = Some(path);
}

#[given(
    expr = "server {string} is enrolled with covered tool {string} referencing that vault secret"
)]
async fn vault_server_enrolled(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, "github");
    assert_eq!(tool, "issues.list");
    assert!(
        w.vault_pending.contains_key("aithos/mcp/github"),
        "the scenario declared the vault secret first"
    );
    provision_vault_hub(
        w,
        vec![VaultServerSpec::github("aithos/mcp/github", "token")],
        false,
    )
    .await;
}

#[given(
    expr = "server {string} is enrolled with covered tool {string} referencing vault path {string} field {string}"
)]
async fn vault_server_enrolled_custom_ref(
    w: &mut GatewayWorld,
    server: String,
    tool: String,
    path: String,
    field: String,
) {
    assert_eq!(server, "github");
    assert_eq!(tool, "issues.list");
    provision_vault_hub(w, vec![VaultServerSpec::github(&path, &field)], false).await;
}

#[given(
    expr = "server {string} is enrolled with covered tool {string} referencing a vault that is down"
)]
async fn vault_server_enrolled_vault_down(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, "github");
    assert_eq!(tool, "issues.list");
    provision_vault_hub(
        w,
        vec![VaultServerSpec::github("aithos/mcp/github", "token")],
        true,
    )
    .await;
}

#[given(
    expr = "server {string} is enrolled with covered tool {string} referencing that vault path"
)]
async fn vault_server_enrolled_malformed_path(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, "github");
    assert_eq!(tool, "issues.list");
    let path = w
        .vault_malformed
        .clone()
        .expect("the malformed path was declared first");
    provision_vault_hub(w, vec![VaultServerSpec::github(&path, "token")], false).await;
}

#[given(
    expr = "servers {string} and {string} are enrolled with covered tools referencing their own secrets"
)]
async fn vault_two_servers_enrolled(w: &mut GatewayWorld, first: String, second: String) {
    assert_eq!((first.as_str(), second.as_str()), ("github", "linear"));
    provision_vault_hub(
        w,
        vec![
            VaultServerSpec::github("aithos/mcp/github", "token"),
            VaultServerSpec::linear(),
        ],
        false,
    )
    .await;
}

#[when("the agent initializes, lists the tools, calls the covered tool and calls an unknown tool")]
async fn vault_agent_full_surface(w: &mut GatewayWorld) {
    let bodies = [
        json!({ "jsonrpc": "2.0", "id": 701, "method": "initialize" }),
        json!({ "jsonrpc": "2.0", "id": 702, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 703, "method": "tools/call",
                "params": { "name": "github__issues_list", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 704, "method": "tools/call",
                "params": { "name": "nosuch__tool", "arguments": {} } }),
    ];
    for body in bodies {
        let router = Arc::clone(&w.vault_harness().router);
        let response = process_multi(&router, body).await;
        w.vault.as_mut().unwrap().responses.push(response.clone());
        w.last_response = Some(response);
    }
}

#[when(expr = "the agent calls {string} and then a completely unknown tool")]
async fn vault_agent_calls_refused_pair(w: &mut GatewayWorld, tool: String) {
    w.call(&tool, json!({})).await;
    w.call("nosuch__tool", json!({})).await;
}

#[when("the agent calls one covered tool of each server through the hub")]
async fn vault_agent_calls_both_servers(w: &mut GatewayWorld) {
    w.call("github__issues_list", json!({})).await;
    w.call("linear__tickets_list", json!({})).await;
}

#[when(expr = "the vault value rotates to {string}")]
async fn vault_value_rotates(w: &mut GatewayWorld, value: String) {
    let harness = w.vault_harness();
    harness
        .vault
        .secrets
        .lock()
        .unwrap()
        .get_mut("aithos/mcp/github")
        .expect("the rotated path exists")
        .insert("token".to_owned(), value);
}

#[then(expr = "the call succeeds and the upstream saw exactly one bearer {string}")]
async fn vault_call_succeeded_with_bearer(w: &mut GatewayWorld, value: String) {
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response.get("error").is_none(),
        "the covered call passes: {response}"
    );
    assert_eq!(
        w.wire_auths("github"),
        [Some(format!("Bearer {value}"))],
        "exactly one relay, bearing the vault value"
    );
}

#[then("the vault was consulted after the act was logged")]
async fn vault_consulted_after_log(w: &mut GatewayWorld) {
    let harness = w.vault_harness();
    assert_eq!(
        harness.vault.hits.lock().unwrap().as_slice(),
        ["aithos/mcp/github"],
        "one resolution for one relay"
    );
    assert_eq!(
        harness.vault.acts_at_hit.lock().unwrap().as_slice(),
        [1],
        "at resolution time the act was already in the context gamma"
    );
    assert_eq!(
        harness.vault.tokens.lock().unwrap().as_slice(),
        [Some(harness.vault_token.clone())],
        "the vault read carried the X-Vault-Token from the environment"
    );
}

#[then(expr = "no agent-facing response contains {string}")]
async fn vault_no_response_contains(w: &mut GatewayWorld, needle: String) {
    assert!(
        !w.vault_harness().responses.is_empty(),
        "responses captured"
    );
    assert!(
        !w.vault_agent_text().contains(&needle),
        "an agent-facing response leaked `{needle}`"
    );
}

#[then("no agent-facing response contains the vault access token")]
async fn vault_no_response_contains_vault_token(w: &mut GatewayWorld) {
    let token = w.vault_harness().vault_token.clone();
    assert!(
        !w.vault_agent_text().contains(&token),
        "an agent-facing response leaked the vault access token"
    );
}

#[then(expr = "the gateway config text contains the reference but never {string}")]
async fn vault_config_is_reference_only(w: &mut GatewayWorld, needle: String) {
    let config = &w.vault_harness().config_text;
    assert!(
        config.contains("aithos/mcp/github") && config.contains("broker: enterprise"),
        "the config names the non-secret reference"
    );
    assert!(
        !config.contains(&needle),
        "the config must never hold the secret value"
    );
}

#[then(expr = "no file of any Ethos store contains {string}")]
async fn vault_no_store_file_contains(w: &mut GatewayWorld, needle: String) {
    for root in &w.vault_harness().store_roots {
        files_exclude(root, &needle);
    }
}

#[then(expr = "no gamma or journal entry contains {string}")]
async fn vault_no_entry_contains(w: &mut GatewayWorld, needle: String) {
    assert!(
        !w.vault_logged_text().contains(&needle),
        "a gamma or journal entry leaked `{needle}`"
    );
}

#[then(expr = "no agent-facing or logged text contains {string}")]
async fn vault_no_text_contains(w: &mut GatewayWorld, needle: String) {
    assert!(
        !w.vault_agent_text().contains(&needle) && !w.vault_logged_text().contains(&needle),
        "`{needle}` escaped into an agent-facing or logged surface"
    );
}

#[then("both calls are refused")]
async fn vault_both_calls_refused(w: &mut GatewayWorld) {
    let responses = &w.vault_harness().responses;
    assert!(responses.len() >= 2, "two calls were made");
    for response in &responses[responses.len() - 2..] {
        assert!(
            response.get("error").is_some(),
            "the call is refused: {response}"
        );
    }
}

#[then("the vault received zero requests")]
async fn vault_zero_hits(w: &mut GatewayWorld) {
    assert!(
        w.vault_harness().vault.hits.lock().unwrap().is_empty(),
        "no request may wake the vault"
    );
}

#[then("the upstream received zero requests")]
async fn vault_upstreams_zero_hits(w: &mut GatewayWorld) {
    for (server, wire) in &w.vault_harness().wires {
        assert!(
            wire.requests.lock().unwrap().is_empty(),
            "no request may reach upstream `{server}`"
        );
    }
}

#[then("the call is refused as credential unavailable")]
async fn vault_call_refused_credential(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response");
    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("credential unavailable"),
        "the refusal carries the stable credential code: {response}"
    );
}

#[then("the journal gains one refusal entry naming the credential failure")]
async fn vault_journal_refusal(w: &mut GatewayWorld) {
    let refusals = acts_on(&w.journal_gamma(), "x.gateway");
    assert_eq!(refusals.len(), 1, "exactly one journal refusal");
    assert_eq!(
        payload_str(&refusals[0], "tool"),
        Some("github__issues_list"),
        "the refusal names the tool whose credential failed"
    );
    assert_eq!(
        payload_str(&refusals[0], "reason"),
        Some("credential_unavailable"),
        "the refusal carries the stable reason code"
    );
}

#[then("the refusal text names neither the vault answer nor any secret value")]
async fn vault_refusal_redacted(w: &mut GatewayWorld) {
    let harness = w.vault_harness();
    let message = w.last_response.as_ref().expect("a response")["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        !message.contains("github-mcp-sentinel") && !message.contains(&harness.vault_token),
        "the refusal must not carry secret material: {message}"
    );
    assert!(
        !message.contains("data\":") && !message.contains("errors\":"),
        "the refusal must not embed the raw vault answer: {message}"
    );
}

#[then(expr = "the {string} upstream saw only bearer {string}")]
async fn vault_upstream_saw_only(w: &mut GatewayWorld, server: String, value: String) {
    let auths = w.wire_auths(&server);
    assert!(!auths.is_empty(), "the `{server}` upstream was reached");
    assert!(
        auths
            .iter()
            .all(|auth| auth.as_deref() == Some(format!("Bearer {value}").as_str())),
        "`{server}` saw exactly its own bearer: {auths:?}"
    );
}

#[then(expr = "the upstream saw bearer {string} then bearer {string}")]
async fn vault_upstream_saw_rotation(w: &mut GatewayWorld, first: String, second: String) {
    assert_eq!(
        w.wire_auths("github"),
        [
            Some(format!("Bearer {first}")),
            Some(format!("Bearer {second}"))
        ],
        "the rotation is honoured on the very next relay"
    );
}

#[then("the gateway config was never modified")]
async fn vault_config_untouched(w: &mut GatewayWorld) {
    let harness = w.vault_harness();
    let on_disk = std::fs::read_to_string(&harness.config_path).expect("config readable");
    assert_eq!(
        on_disk, harness.config_text,
        "rotation must not touch the config"
    );
}

#[then(expr = "the list includes {string}")]
async fn vault_list_includes(w: &mut GatewayWorld, name: String) {
    assert!(
        w.listed_tools()
            .iter()
            .any(|tool| tool["name"].as_str() == Some(name.as_str())),
        "`{name}` must be listed"
    );
}

// --------------------------------------------------- write grants (lot W)

const GRANTS_SERVER: &str = "gmail";
const GRANTS_CONTEXT: &str = "ventes";

fn grants_fixture(name: &str) -> Value {
    match name {
        "search_emails" => json!({
            "name": "search_emails",
            "description": "Search the mailbox",
            "inputSchema": {
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "additionalProperties": false
            }
        }),
        "send_email" => json!({
            "name": "send_email",
            "description": "Send an email",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": { "type": "array", "items": { "type": "string" } },
                    "subject": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["to"],
                "additionalProperties": false
            }
        }),
        other => panic!("unknown grants fixture `{other}`"),
    }
}

fn grants_raw_name(exposed: &str) -> String {
    exposed
        .strip_prefix("gmail__")
        .expect("a gmail exposed name")
        .to_owned()
}

fn grants_config_text(
    context_root: &std::path::Path,
    journal_root: &std::path::Path,
    approved: &ApprovedManifest,
    overrides: &BTreeMap<String, bool>,
) -> String {
    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let mut tools = String::new();
    for tool in &approved.tools {
        let access = match tool.risk_class {
            ToolAccess::Read => "read",
            ToolAccess::Write => "write",
        };
        let granted = overrides
            .get(&tool.name)
            .copied()
            .unwrap_or_else(|| tool.is_granted());
        tools.push_str(&format!(
            "      {}: {{ server: {GRANTS_SERVER}, tool: {}, access: {access}, granted: {granted} }}\n",
            tool.exposed_name, tool.name
        ));
    }
    format!(
        "listen: 127.0.0.1:4870\nservers:\n  - name: {GRANTS_SERVER}\n    transport: http\n    url: https://gmail.invalid/mcp\ncontexts:\n  - name: {GRANTS_CONTEXT}\n    store: {{ kind: fs, root: {} }}\n    tools:\n{tools}journal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(context_root),
        quote(journal_root)
    )
}

/// Enroll the gmail fixture under the ventes context with the given
/// approvals; optionally open the governed runtime over a fake wire.
async fn provision_grants_world(
    w: &mut GatewayWorld,
    approvals: BTreeMap<String, ToolApproval>,
    open_runtime: bool,
) {
    if w.router.is_some() || w.grants_store.is_some() {
        return;
    }
    let dir = tempfile::tempdir().expect("grants tempdir");
    let context_root = dir.path().join(GRANTS_CONTEXT);
    let journal_root = dir.path().join("journal");
    let store_cfg = |root: &std::path::Path| aithos_gateway::config::StoreConfig::Fs {
        root: root.to_owned(),
    };
    let context_store =
        GatewayStore::from_config(&store_cfg(&context_root)).expect("grants context store");
    let journal_store =
        GatewayStore::from_config(&store_cfg(&journal_root)).expect("grants journal store");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let mut owner_ent = SeqEntropy::default();
    owner_init_context(
        &master,
        GRANTS_CONTEXT,
        context_store.clone(),
        T0,
        &mut owner_ent,
    )
    .expect("grants context created");
    let advertised: Vec<Value> = approvals.keys().map(|name| grants_fixture(name)).collect();
    let upstream = FakeMcp::advertising(advertised);
    let proposed = discover_server(GRANTS_SERVER, &upstream)
        .await
        .expect("grants discovery");
    let approved = approve_manifest(&proposed, &approvals).expect("grants approval");
    owner_enroll_server(
        &master,
        GRANTS_CONTEXT,
        &agent_pub,
        &gateway_pub,
        &approved,
        context_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("grants enrollment");
    owner_init_journal(
        &master,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("grants journal");
    for tool in &approved.tools {
        w.expected_pins.insert(
            tool.exposed_name.clone(),
            (tool.description.clone(), tool.input_schema.clone()),
        );
    }
    w.ctx_stores
        .insert(GRANTS_CONTEXT.to_owned(), context_store.clone());
    w.journal_store = Some(journal_store);
    w.grants_store = Some(context_store);
    w.grants_label = Some(GRANTS_CONTEXT.to_owned());
    w.grants_roots = Some((context_root.clone(), journal_root.clone()));
    w.approved_manifest = Some(approved.clone());
    w.upstream = upstream.clone();

    if open_runtime {
        open_grants_runtime(w, &approved, &BTreeMap::new());
    }
    w.scratch = Some(dir);
}

/// (Re)open the governed runtime for the grants world against the
/// current sealed manifest, with optional per-tool config overrides of
/// the granted flag (the mismatch scenario).
fn open_grants_runtime(
    w: &mut GatewayWorld,
    approved: &ApprovedManifest,
    overrides: &BTreeMap<String, bool>,
) {
    let (context_root, journal_root) = w.grants_roots.clone().expect("grants roots");
    let text = grants_config_text(&context_root, &journal_root, approved, overrides);
    let cfg = GatewayConfig::from_yaml(&text).expect("grants config parses");
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner = Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
        .expect("grants governed runner");
    w.upstream.seen.lock().unwrap().clear();
    w.multi_upstreams
        .insert(GRANTS_SERVER.to_owned(), w.upstream.clone());
    w.router = Some(Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(runner)),
        upstreams: BTreeMap::from([(GRANTS_SERVER.to_owned(), w.upstream.clone())]),
        clock: Arc::new(|| T0.to_owned()),
    }));
}

fn parse_class_spec(spec: &str) -> (String, ToolAccess) {
    let (tool, class) = spec.split_once('=').expect("TOOL=class");
    let class = match class {
        "read" => ToolAccess::Read,
        "write" => ToolAccess::Write,
        other => panic!("unknown class `{other}`"),
    };
    (tool.to_owned(), class)
}

#[given(expr = "server {string} advertises tools {string} and {string}")]
async fn grants_server_advertises(
    w: &mut GatewayWorld,
    server: String,
    first: String,
    second: String,
) {
    assert_eq!(server, GRANTS_SERVER);
    assert_eq!(
        (first.as_str(), second.as_str()),
        ("search_emails", "send_email")
    );
    let _ = w;
}

#[given(
    expr = "the owner enrolls {string} approving {string} as a granted read and {string} as a granted write"
)]
async fn grants_enroll_granted_pair(
    w: &mut GatewayWorld,
    server: String,
    read_tool: String,
    write_tool: String,
) {
    assert_eq!(server, GRANTS_SERVER);
    provision_grants_world(
        w,
        BTreeMap::from([
            (read_tool, ToolApproval::granted(ToolAccess::Read)),
            (write_tool, ToolApproval::granted(ToolAccess::Write)),
        ]),
        true,
    )
    .await;
}

#[when(expr = "the owner enrolls {string} approving only classes {string} and {string}")]
async fn grants_enroll_classes_only(
    w: &mut GatewayWorld,
    server: String,
    first: String,
    second: String,
) {
    assert_eq!(server, GRANTS_SERVER);
    let (first_tool, first_class) = parse_class_spec(&first);
    let (second_tool, second_class) = parse_class_spec(&second);
    provision_grants_world(
        w,
        BTreeMap::from([
            (first_tool, ToolApproval::class(first_class)),
            (second_tool, ToolApproval::class(second_class)),
        ]),
        true,
    )
    .await;
}

#[given(
    expr = "the owner enrolls {string} approving {string} as a denied read and {string} as a granted write"
)]
async fn grants_enroll_denied_read(
    w: &mut GatewayWorld,
    server: String,
    read_tool: String,
    write_tool: String,
) {
    assert_eq!(server, GRANTS_SERVER);
    provision_grants_world(
        w,
        BTreeMap::from([
            (read_tool, ToolApproval::denied(ToolAccess::Read)),
            (write_tool, ToolApproval::granted(ToolAccess::Write)),
        ]),
        true,
    )
    .await;
}

#[given(expr = "the owner enrolls {string} approving {string} as a granted write")]
async fn grants_enroll_single_granted(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, GRANTS_SERVER);
    provision_grants_world(
        w,
        BTreeMap::from([(tool, ToolApproval::granted(ToolAccess::Write))]),
        false,
    )
    .await;
}

#[given(expr = "the owner enrolls {string} approving {string} as a denied write")]
async fn grants_enroll_single_denied(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, GRANTS_SERVER);
    provision_grants_world(
        w,
        BTreeMap::from([(tool, ToolApproval::denied(ToolAccess::Write))]),
        false,
    )
    .await;
}

#[given(expr = "server {string} is enrolled with {string} as a granted write")]
async fn grants_enrolled_running(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, GRANTS_SERVER);
    provision_grants_world(
        w,
        BTreeMap::from([(tool, ToolApproval::granted(ToolAccess::Write))]),
        true,
    )
    .await;
}

#[when(expr = "the agent lists the tools and calls {string} through the hub")]
async fn grants_list_then_call(w: &mut GatewayWorld, tool: String) {
    let router = Arc::clone(w.router.as_ref().expect("grants router"));
    w.last_list = Some(
        process_multi(
            &router,
            json!({ "jsonrpc": "2.0", "id": 61, "method": "tools/list" }),
        )
        .await,
    );
    w.call(&tool, json!({})).await;
}

#[then(expr = "the call reaches the upstream under raw name {string}")]
async fn grants_call_reached_raw(w: &mut GatewayWorld, raw: String) {
    let calls: Vec<String> = w
        .upstream
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body["method"] == "tools/call")
        .filter_map(|body| body.pointer("/params/name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    assert_eq!(calls.last().map(String::as_str), Some(raw.as_str()));
}

#[then("the act is logged in the granting context gamma with one journal cross-reference")]
async fn grants_act_logged(w: &mut GatewayWorld) {
    let target = format!("x.{GRANTS_SERVER}");
    assert_eq!(acts_on(&w.ctx_gamma(GRANTS_CONTEXT), &target).len(), 1);
    assert_eq!(acts_on(&w.journal_gamma(), "x.xref").len(), 1);
}

#[then(expr = "{string} is listed and served")]
async fn grants_listed_and_served(w: &mut GatewayWorld, tool: String) {
    let listed = grants_fresh_list(w).await;
    assert!(
        listed
            .iter()
            .any(|entry| entry["name"].as_str() == Some(tool.as_str())),
        "`{tool}` must be listed"
    );
    w.call(&tool, json!({})).await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response.get("error").is_none(),
        "`{tool}` serves: {response}"
    );
}

/// One fresh tools/list through the grants router.
async fn grants_fresh_list(w: &GatewayWorld) -> Vec<Value> {
    let router = Arc::clone(w.router.as_ref().expect("grants router"));
    let listed = process_multi(
        &router,
        json!({ "jsonrpc": "2.0", "id": 63, "method": "tools/list" }),
    )
    .await;
    listed["result"]["tools"]
        .as_array()
        .expect("tools listed")
        .clone()
}

#[then(expr = "{string} is hidden and precisely refused with zero upstream contact")]
async fn grants_hidden_refused(w: &mut GatewayWorld, tool: String) {
    let listed = grants_fresh_list(w).await;
    assert!(
        listed
            .iter()
            .all(|entry| entry["name"].as_str() != Some(tool.as_str())),
        "`{tool}` must stay hidden"
    );
    w.call(&tool, json!({})).await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains(&tool)),
        "the refusal names `{tool}`: {response}"
    );
    let raw = grants_raw_name(&tool);
    assert!(
        w.upstream
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["method"] == "tools/call")
            .all(|body| body.pointer("/params/name").and_then(Value::as_str) != Some(raw.as_str())),
        "`{raw}` never reaches the upstream"
    );
}

#[then(expr = "the granting context gamma grant entry names {string} as a granted {string}")]
async fn grants_grant_on_record(w: &mut GatewayWorld, tool: String, class: String) {
    let store = w.grants_store.clone().expect("grants store");
    let state: Value = serde_json::from_slice(
        &store
            .clone()
            .get(STATE_PATH)
            .expect("state readable")
            .expect("state present"),
    )
    .expect("state JSON");
    let agent_mandate = state["agent_mandate"]
        .as_str()
        .expect("agent mandate")
        .to_owned();
    let op = format!("act.x.{GRANTS_SERVER}.{}", tool.replace('.', "_"));
    let perimeter = cert_perimeter(store.clone(), &agent_mandate).expect("perimeter readable");
    assert!(
        perimeter.contains(&op),
        "the granted mandate covers `{op}`: {perimeter:?}"
    );
    let grants: Vec<EntryView> = gamma_view(store)
        .expect("gamma readable")
        .into_iter()
        .filter(|entry| {
            entry.kind == "grant" && entry.target.as_deref() == Some(agent_mandate.as_str())
        })
        .collect();
    assert_eq!(
        grants.len(),
        1,
        "the grant of that mandate is on the record"
    );
    let manifest = w.approved_manifest.as_ref().expect("approved manifest");
    let approved = manifest
        .tools
        .iter()
        .find(|t| t.name == tool)
        .expect("tool");
    let recorded_class = match approved.risk_class {
        ToolAccess::Read => "read",
        ToolAccess::Write => "write",
    };
    assert_eq!(recorded_class, class);
    assert!(approved.is_granted());
}

#[then("the sealed manifest records the decision next to the risk class")]
async fn grants_sealed_decision(w: &mut GatewayWorld) {
    let master = w.master();
    let store = w.grants_store.clone().expect("grants store");
    let manifest = owner_read_hub_manifest(&master, GRANTS_CONTEXT, GRANTS_SERVER, store)
        .expect("sealed manifest opens owner-side");
    let tool = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "send_email")
        .expect("send_email sealed");
    assert_eq!(tool.granted, Some(true), "the decision is explicit at rest");
    assert_eq!(tool.risk_class, ToolAccess::Write);
}

#[when(expr = "a runtime config declares {string} as granted")]
async fn grants_config_overclaims(w: &mut GatewayWorld, tool: String) {
    let raw = grants_raw_name(&tool);
    let approved = w.approved_manifest.clone().expect("approved manifest");
    let (context_root, journal_root) = w.grants_roots.clone().expect("grants roots");
    let text = grants_config_text(
        &context_root,
        &journal_root,
        &approved,
        &BTreeMap::from([(raw, true)]),
    );
    let cfg = GatewayConfig::from_yaml(&text).expect("the shape itself parses");
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    w.config_error = Some(
        Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default(),
    );
}

#[then("the gateway refuses to open, naming the grant mismatch")]
async fn grants_open_refused(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("an open verdict");
    assert!(
        err.contains("grant decision"),
        "the refusal names the grant mismatch: {err}"
    );
}

#[given(expr = "the agent has called {string} through the hub once")]
async fn grants_called_once(w: &mut GatewayWorld, tool: String) {
    w.call(&tool, json!({})).await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(response.get("error").is_none(), "the granted call passes");
}

#[when(expr = "the owner re-enrolls {string} with {string} denied for the same agent key")]
async fn grants_reenroll_denied(w: &mut GatewayWorld, server: String, tool: String) {
    assert_eq!(server, GRANTS_SERVER);
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let store = w.grants_store.clone().expect("grants store");
    let state: Value = serde_json::from_slice(
        &store
            .clone()
            .get(STATE_PATH)
            .expect("state readable")
            .expect("state present"),
    )
    .expect("state JSON");
    w.old_agent_mandate = Some(state["agent_mandate"].as_str().unwrap().to_owned());
    let discovery = FakeMcp::advertising(vec![grants_fixture(&tool)]);
    let proposed = discover_server(GRANTS_SERVER, &discovery)
        .await
        .expect("re-discovery");
    let approved = approve_manifest(
        &proposed,
        &BTreeMap::from([(tool, ToolApproval::denied(ToolAccess::Write))]),
    )
    .expect("denied approval");
    let mut ent = SeqEntropy::default();
    let outcome = owner_reenroll_server(
        &master,
        GRANTS_CONTEXT,
        &agent_pub,
        &gateway_pub,
        &approved,
        store,
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("re-enrollment");
    w.reenroll = Some(outcome);
    w.approved_manifest = Some(approved.clone());
    open_grants_runtime(w, &approved, &BTreeMap::new());
}

#[then("a new mandate excludes the write and the old mandate is politically revoked")]
async fn grants_reenroll_verdict(w: &mut GatewayWorld) {
    let result = w.reenroll.as_ref().expect("re-enrollment outcome");
    let old = w.old_agent_mandate.as_deref().expect("old mandate id");
    assert!(result.revoked_mandates.iter().any(|mandate| mandate == old));
    let store = w.grants_store.clone().expect("grants store");
    let perimeter =
        cert_perimeter(store.clone(), &result.equipment.agent_mandate).expect("new perimeter");
    assert!(
        !perimeter.contains(&format!("act.x.{GRANTS_SERVER}.send_email")),
        "the new mandate excludes the write: {perimeter:?}"
    );
    assert!(gamma_view(store)
        .expect("gamma readable")
        .iter()
        .any(|entry| entry.kind == "revoke" && entry.target.as_deref() == Some(old)));
}

#[then(expr = "the next call to {string} is refused and never reaches the upstream")]
async fn grants_next_call_refused(w: &mut GatewayWorld, tool: String) {
    w.call(&tool, json!({})).await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains(&tool)),
        "the refusal names `{tool}`: {response}"
    );
    assert!(
        w.upstream
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|body| body["method"] != "tools/call"),
        "nothing reaches the upstream after revocation"
    );
}

#[then(expr = "tools\\/list no longer includes {string}")]
async fn grants_list_excludes(w: &mut GatewayWorld, tool: String) {
    let router = Arc::clone(w.router.as_ref().expect("grants router"));
    let listed = process_multi(
        &router,
        json!({ "jsonrpc": "2.0", "id": 62, "method": "tools/list" }),
    )
    .await;
    assert!(listed["result"]["tools"]
        .as_array()
        .expect("tools listed")
        .iter()
        .all(|entry| entry["name"].as_str() != Some(tool.as_str())));
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
