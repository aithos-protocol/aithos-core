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
    agent_pub_multibase, cert_constraints, cert_grantee_pub, gamma_view, gateway_pub_multibase,
    journal_notes_view, owner_grant_context, owner_init_context, owner_init_journal,
    owner_read_journal_note, Bridge, ContextRuntime, EntropySource, EntryView, EquipOutcome,
    MandateWindow, OnboardOutcome, RawStore, Runner, SeqEntropy, STATE_PATH,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_llm::{process_llm, LlmProxy, LlmUpstream, LLM_TOOL};
use aithos_gateway::proxy_mcp::{
    process, process_multi, McpProxy, McpRouter, Upstream, JOURNAL_SEARCH, JOURNAL_WRITE,
    POLICY_DENIED_CODE,
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
        }
    }
}

impl Upstream for FakeMcp {
    async fn forward(&self, body: Value) -> Result<Value> {
        self.seen.lock().unwrap().push(body.clone());
        let id = body.get("id").cloned().unwrap_or(Value::Null);
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
        self.last_response = Some(if let Some(router) = &self.router {
            process_multi(router, body).await
        } else {
            process(self.proxy(), body).await
        });
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
        for act in acts_on(&w.ctx_gamma(name), "x.mcp") {
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

    /// The tool entries of the last tools/list answer.
    fn listed_tools(&self) -> Vec<Value> {
        self.last_response
            .as_ref()
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
    let router = w.router.as_ref().expect("a provisioned router");
    w.last_response = Some(
        process_multi(
            router,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
        )
        .await,
    );
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
