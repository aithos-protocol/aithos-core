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
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use cucumber::{given, then, when, World};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use aithos_gateway::config::{GatewayConfig, ToolAccess, ToolMap};
use aithos_gateway::core_bridge::{
    agent_pub_multibase, cert_constraints, cert_grantee_pub, cert_perimeter, gamma_view,
    gateway_pub_multibase, journal_notes_view, owner_add_section, owner_enroll_server,
    owner_grant_briefing, owner_grant_context, owner_grant_ethos_read, owner_init_context,
    owner_init_journal, owner_issue_ethos_read_subchain, owner_preview_call, owner_preview_mandate,
    owner_read_hub_manifest, owner_read_journal_note, owner_reenroll_server,
    owner_revoke_mandate_id, owner_set_briefing, Bridge, ContextRuntime, EntropySource, EntryView,
    EquipOutcome, MandateWindow, OnboardOutcome, OsEntropy, RawStore, ReenrollOutcome, Runner,
    SeqEntropy, EFFECTIVE_POLICY_VERSION, STATE_PATH,
};
use aithos_gateway::credentials::{CredentialBroker, CredentialRef, SecretValue};
use aithos_gateway::hub::{
    approve_manifest, discover_server, ApprovedManifest, ArgumentBound, ToolApproval,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::oauth::{b64url_decode, s256_challenge, AdapterKey, AuthServer};
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_llm::{process_llm, LlmProxy, LlmUpstream, LLM_TOOL};
use aithos_gateway::proxy_mcp::{
    process, process_multi, refresh_server_manifest, router_multi, router_oauth, HttpUpstream,
    McpProxy, McpRouter, Upstream, BRIEFING_READ, ETHOS_CONTEXT, ETHOS_LIST, ETHOS_READ,
    JOURNAL_SEARCH, JOURNAL_WRITE, METHOD_NOT_FOUND_CODE, POLICY_DENIED_CODE,
};
use aithos_gateway::store_adapter::GatewayStore;
use aithos_gateway::upstream_oauth::{self, UpstreamOAuthRegistry};
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
        "ventes" => ("crm.read".into(), "crm.update".into()),
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
    /// Bounds scenarios (lot P): the exact arguments of the last bounded
    /// call, for the arguments-unmodified assertion.
    last_args: Option<Value>,
    /// Briefing scenarios (lot K): directives declared before the world
    /// opens — (context, zone, title, text) — the granted pen per
    /// context, the last initialize answer, every agent-facing response
    /// of the scenario (the self-zone leak assertion sweeps them all),
    /// and the owner entropy kept alive for hot edits.
    briefing_pending: Vec<(String, String, String, String)>,
    briefing_mandates: BTreeMap<String, String>,
    briefing_rewritten: Option<String>,
    last_init: Option<Value>,
    agent_responses: Vec<Value>,
    briefing_owner_ent: Option<SeqEntropy>,
    /// Demo Léa (lot D): the Background's provisioning table, the notion
    /// prospect list, the circle directive and self note, the per-server
    /// wire bearers and the ventes auditor seed for beat 8.
    demo_specs: Vec<DemoToolSpec>,
    demo_prospects: Vec<String>,
    demo_directive: Option<String>,
    demo_note: Option<String>,
    demo_bearers: BTreeMap<String, String>,
    demo_auditor_seed: Option<String>,
    /// Restricted mandates (M2): the last owner preview (read-model or
    /// dry-run verdict) and the tool it previewed.
    preview: Option<Value>,
    preview_tool: Option<String>,
    /// Streamable transport scenarios (G2): the served base URL of the
    /// REAL axum shell over a real socket, the wire exchanges in call
    /// order, the last minted session id, and the per-store gamma
    /// counts captured right after provisioning (the no-new-entry
    /// assertions compare against them).
    wire_base: Option<String>,
    wire_responses: Vec<WireResponse>,
    wire_session: Option<String>,
    gamma_baseline: BTreeMap<String, usize>,
    /// Ethos reading scenarios (lot G6): sections declared before the
    /// world opens — (context, zone, path, text) — the granted pen per
    /// context, the delegate sub-chain ids, the last refused gesture
    /// and the certificate count captured before it.
    ethos_pending: Vec<(String, String, String, String)>,
    ethos_mandates: BTreeMap<String, String>,
    ethos_subchain: Option<(String, String)>,
    ethos_gesture_error: Option<String>,
    ethos_cert_baseline: Option<usize>,
    /// OAuth AS scenarios (lot G3): the served issuer base, the adapter
    /// seed (to forge right-key/wrong-audience tokens), a mutable clock
    /// cell (advanced by the expiry scenarios), the live flow state
    /// (client, redirect, PKCE, code, token pair), and every captured
    /// HTTP exchange in order.
    oauth_base: Option<String>,
    oauth_adapter_seed: Option<[u8; 32]>,
    oauth_clock: Option<Arc<StdMutex<String>>>,
    oauth_client_id: Option<String>,
    oauth_redirect: Option<String>,
    oauth_verifier: Option<String>,
    oauth_challenge: Option<String>,
    oauth_state: Option<String>,
    oauth_code: Option<String>,
    oauth_access: Option<String>,
    oauth_refresh: Option<String>,
    oauth_http: Vec<HttpCapture>,
    /// OAuth client scenarios: strict config text, in-memory Vault, fake
    /// token/resource wire and the last owner/callback/runtime outcomes.
    upstream_oauth_config: Option<String>,
    upstream_oauth: Option<UpstreamOAuthHarness>,
    upstream_oauth_consent: Option<String>,
    upstream_oauth_callback: Option<HttpCapture>,
    upstream_oauth_result: Option<std::result::Result<Value, String>>,
    ctx_agent_mandates: BTreeMap<String, String>,
}

/// One raw Streamable HTTP exchange (G2): what the wire actually said.
#[derive(Debug, Clone)]
struct WireResponse {
    status: u16,
    session: Option<String>,
    body: Vec<u8>,
}

#[derive(Default)]
struct MemoryOAuthVault {
    values: StdMutex<BTreeMap<(String, String), String>>,
}

impl MemoryOAuthVault {
    fn put_clear(&self, path: &str, field: &str, value: &str) {
        self.values
            .lock()
            .unwrap()
            .insert((path.to_owned(), field.to_owned()), value.to_owned());
    }

    fn clear(&self, path: &str, field: &str) -> Option<String> {
        self.values
            .lock()
            .unwrap()
            .get(&(path.to_owned(), field.to_owned()))
            .cloned()
    }
}

impl CredentialBroker for MemoryOAuthVault {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(async move {
            self.values
                .lock()
                .unwrap()
                .get(&(reference.path.clone(), reference.field.clone()))
                .cloned()
                .map(SecretValue::new)
                .ok_or_else(|| {
                    GatewayError::CredentialUnavailable("test Vault field absent".into())
                })
        })
    }

    fn store<'a>(
        &'a self,
        reference: &'a CredentialRef,
        value: SecretValue,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.put_clear(&reference.path, &reference.field, value.expose());
            Ok(())
        })
    }
}

struct UpstreamOAuthHarness {
    vault: Arc<MemoryOAuthVault>,
    registry: Arc<UpstreamOAuthRegistry>,
    upstream: HttpUpstream,
    token_grants: Arc<StdMutex<Vec<BTreeMap<String, String>>>>,
    resource_bearers: Arc<StdMutex<Vec<Option<String>>>>,
    refuse_refresh: Arc<AtomicBool>,
    callback_url: String,
}

const UPSTREAM_CLIENT_SECRET: &str = "oauth-client-secret-sentinel";
const UPSTREAM_ACCESS_1: &str = "oauth-access-sentinel-one";
const UPSTREAM_ACCESS_2: &str = "oauth-access-sentinel-two";
const UPSTREAM_REFRESH_1: &str = "oauth-refresh-sentinel-one";
const UPSTREAM_REFRESH_2: &str = "oauth-refresh-sentinel-two";
const UPSTREAM_TOKEN_PATH: &str = "aithos/oauth/protected";
const UPSTREAM_TOKEN_FIELD: &str = "state";

impl WireResponse {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("a JSON body")
    }
}

/// One row of the demo Background table.
#[derive(Debug, Clone)]
struct DemoToolSpec {
    server: String,
    tool: String,
    class: ToolAccess,
    granted: bool,
    bounds: Vec<ArgumentBound>,
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
            last_args: None,
            briefing_pending: Vec::new(),
            briefing_mandates: BTreeMap::new(),
            briefing_rewritten: None,
            last_init: None,
            agent_responses: Vec::new(),
            briefing_owner_ent: None,
            demo_specs: Vec::new(),
            demo_prospects: Vec::new(),
            demo_directive: None,
            demo_note: None,
            demo_bearers: BTreeMap::new(),
            demo_auditor_seed: None,
            preview: None,
            preview_tool: None,
            wire_base: None,
            wire_responses: Vec::new(),
            wire_session: None,
            gamma_baseline: BTreeMap::new(),
            ethos_pending: Vec::new(),
            ethos_mandates: BTreeMap::new(),
            ethos_subchain: None,
            ethos_gesture_error: None,
            ethos_cert_baseline: None,
            oauth_base: None,
            oauth_adapter_seed: None,
            oauth_clock: None,
            oauth_client_id: None,
            oauth_redirect: None,
            oauth_verifier: None,
            oauth_challenge: None,
            oauth_state: None,
            oauth_code: None,
            oauth_access: None,
            oauth_refresh: None,
            oauth_http: Vec::new(),
            upstream_oauth_config: None,
            upstream_oauth: None,
            upstream_oauth_consent: None,
            upstream_oauth_callback: None,
            upstream_oauth_result: None,
            ctx_agent_mandates: BTreeMap::new(),
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
        self.agent_responses.push(response.clone());
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
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
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
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
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
    provision_pending_world(w).await;
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
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
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
    // The revocation lives in whichever context re-enrolled: the hub
    // world's customer-support, or the bounds world's ventes.
    let context = if w.ctx_stores.contains_key("customer-support") {
        "customer-support"
    } else {
        BOUNDS_CONTEXT
    };
    assert!(w
        .ctx_gamma(context)
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
        // The agent mandate id per context (lot G3 revocation scenario).
        w.ctx_agent_mandates
            .insert(label.clone(), outcome.agent_mandate.clone());
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
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
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
            session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
            oauth: None,
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
async fn no_agent_response_contains(w: &mut GatewayWorld, needle: String) {
    // Whatever world answered — vault harness or plain router — every
    // agent-facing response of the scenario is swept.
    let mut texts: Vec<String> = w.agent_responses.iter().map(ToString::to_string).collect();
    if let Some(vault) = &w.vault {
        texts.extend(vault.responses.iter().map(ToString::to_string));
    }
    assert!(!texts.is_empty(), "responses captured");
    assert!(
        !texts.join("\n").contains(&needle),
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
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
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

// ------------------------------------------------ argument bounds (lot P)
//
// The bounds world rides the FULL governed stack on purpose: real
// HttpUpstream, real VaultKv2Broker against the fake KV v2 vault, and a
// header-recording wire MCP — so "a bound violation wakes neither the
// vault nor the upstream" is a counted fact on live sockets.

const BOUNDS_CONTEXT: &str = "ventes";
const BOUNDS_APPROVED: [&str; 3] = [
    "prospect-a@clients.example",
    "prospect-b@clients.example",
    "prospect-c@clients.example",
];

// ------------------------------------------------ briefing world (lot K)
//
// The smallest world the briefing scenarios need: one legacy context
// ("ventes") granted to the runner's key, the briefing pen on top, the
// directives declared by the Givens written owner-side BEFORE the
// bridges open, a journal, and the plain multi-context router. No hub,
// no vault, no sockets — the briefing is the gateway's own surface.

/// The default directive of the count-only Given ("holds a directive").
const BRIEFING_DEFAULT_DIRECTIVE: &str = "Toujours confirmer le créneau par écrit.";

async fn provision_briefing_world(w: &mut GatewayWorld) {
    if w.router.is_some() || w.vault.is_some() || w.proxy.is_some() {
        return;
    }
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let window = GatewayWorld::window();
    let mut owner_ent = SeqEntropy::default();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Arc::new(Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32()));

    let mut labels: std::collections::BTreeSet<String> =
        w.briefing_pending.iter().map(|d| d.0.clone()).collect();
    labels.extend(w.ethos_pending.iter().map(|d| d.0.clone()));
    labels.insert("ventes".to_owned());

    let mut contexts = BTreeMap::new();
    for label in labels {
        let (read, write) = context_tools(&label);
        let store = GatewayStore::in_memory();
        owner_init_context(&master, &label, store.clone(), T0, &mut owner_ent)
            .expect("briefing context created");
        owner_grant_context(
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
        .expect("briefing context granted");
        let pen = owner_grant_briefing(
            &master,
            &label,
            &agent_pub,
            store.clone(),
            &window,
            T0,
            &mut owner_ent,
        )
        .expect("briefing pen granted");
        w.briefing_mandates.insert(label.clone(), pen);
        for (ctx, zone, title, text) in w.briefing_pending.clone() {
            if ctx == label {
                owner_set_briefing(
                    &master,
                    &label,
                    &zone,
                    &title,
                    &text,
                    store.clone(),
                    T0,
                    &mut owner_ent,
                )
                .expect("directive written owner-side");
            }
        }
        for (ctx, zone, path, text) in w.ethos_pending.clone() {
            if ctx == label {
                owner_add_section(
                    &master,
                    &label,
                    &zone,
                    &path,
                    &text,
                    store.clone(),
                    T0,
                    &mut owner_ent,
                )
                .expect("section written owner-side");
            }
        }
        let bridge = Bridge::open(
            store.clone(),
            Arc::clone(&keyholder),
            Box::new(SeqEntropy::default()),
        )
        .expect("briefing context bridge opens");
        let mut tools = ToolMap::new();
        tools.insert(read, ToolAccess::Read);
        tools.insert(write, ToolAccess::Write);
        w.ctx_stores.insert(label.clone(), store);
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
    owner_init_journal(
        &master,
        "lea",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &window,
        T0,
        &mut owner_ent,
    )
    .expect("briefing journal created");
    let journal = Bridge::open(
        journal_store.clone(),
        keyholder,
        Box::new(SeqEntropy::default()),
    )
    .expect("briefing journal bridge opens");
    w.journal_store = Some(journal_store);
    w.briefing_owner_ent = Some(owner_ent);
    w.router = Some(Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(Runner::from_parts(contexts, journal))),
        upstreams: w.multi_upstreams.clone(),
        clock: Arc::new(|| T0.to_owned()),
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
    }));
}

impl GatewayWorld {
    /// One agent-facing request through whichever world is live, the
    /// answer recorded like every other (the self-zone leak assertion
    /// sweeps `agent_responses`).
    async fn agent_request(&mut self, body: Value) -> Value {
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
        self.agent_responses.push(response.clone());
        response
    }

    /// The directive texts declared for one zone of one context.
    fn briefing_texts(&self, zone: &str) -> Vec<String> {
        self.briefing_pending
            .iter()
            .filter(|(_, z, _, _)| z == zone)
            .map(|(_, _, _, text)| text.clone())
            .collect()
    }
}

/// Provision whichever lazy world the scenario's Givens declared: the
/// demo Léa harness when a Background table is pending, the briefing
/// world otherwise. No-op once any world is live.
async fn provision_pending_world(w: &mut GatewayWorld) {
    if w.demo_specs.is_empty() {
        provision_briefing_world(w).await;
    } else {
        provision_demo_world(w).await;
    }
}

async fn briefing_call(w: &mut GatewayWorld, args: Value) {
    provision_pending_world(w).await;
    w.last_tool = BRIEFING_READ.to_owned();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 61,
        "method": "tools/call",
        "params": { "name": BRIEFING_READ, "arguments": args }
    });
    let response = w.agent_request(body).await;
    w.last_response = Some(response);
}

#[given(expr = "the {string} context circle zone holds the directive {string}")]
async fn briefing_circle_directive(w: &mut GatewayWorld, context: String, text: String) {
    w.briefing_pending
        .push((context, "circle".into(), "Consigne".into(), text));
}

#[given(expr = "the {string} context public zone holds the directive {string}")]
async fn briefing_public_directive(w: &mut GatewayWorld, context: String, text: String) {
    w.briefing_pending
        .push((context, "public".into(), "Consigne".into(), text));
}

#[given(expr = "the {string} context self zone holds the note {string}")]
async fn briefing_self_note(w: &mut GatewayWorld, context: String, text: String) {
    w.briefing_pending
        .push((context, "self".into(), "Note owner".into(), text));
}

#[given(expr = "the {string} context circle zone holds a directive")]
async fn briefing_some_directive(w: &mut GatewayWorld, context: String) {
    w.briefing_pending.push((
        context,
        "circle".into(),
        "Consigne".into(),
        BRIEFING_DEFAULT_DIRECTIVE.into(),
    ));
}

#[given("no granted zone of any context holds a directive")]
async fn briefing_nothing_declared(w: &mut GatewayWorld) {
    assert!(
        w.briefing_pending.is_empty(),
        "the mute-surface scenario starts empty"
    );
}

#[given("the agent has no write right on any briefing zone")]
async fn briefing_no_write_right(_w: &mut GatewayWorld) {
    // Structural in v1: the gateway exposes no briefing write tool and
    // the briefing pen carries the Read verb only — nothing to doctor.
}

#[given("the agent has read the briefing once")]
async fn briefing_read_once(w: &mut GatewayWorld) {
    briefing_call(w, json!({})).await;
    let text = w.last_result_text();
    assert!(
        !text.is_empty(),
        "the first read serves the directive before the edit"
    );
}

#[when("the agent initializes and lists the tools")]
async fn briefing_init_and_list(w: &mut GatewayWorld) {
    provision_pending_world(w).await;
    let init = w
        .agent_request(json!({ "jsonrpc": "2.0", "id": 60, "method": "initialize" }))
        .await;
    w.last_init = Some(init);
    let list = w
        .agent_request(json!({ "jsonrpc": "2.0", "id": 62, "method": "tools/list" }))
        .await;
    w.last_list = Some(list);
}

#[when(expr = "the agent calls {string}")]
async fn briefing_agent_calls(w: &mut GatewayWorld, tool: String) {
    match tool.as_str() {
        BRIEFING_READ => briefing_call(w, json!({})).await,
        // The G2 liveness probe rides the same bare step: a direct
        // JSON-RPC method, never a tools/call.
        "ping" => {
            let resp = w
                .agent_request(json!({ "jsonrpc": "2.0", "id": 80, "method": "ping" }))
                .await;
            w.last_response = Some(resp);
        }
        ETHOS_LIST | ETHOS_CONTEXT => {
            let tool = tool.clone();
            ethos_call(w, &tool, json!({})).await;
        }
        other => panic!(
            "the bare call step serves briefing.read, ping or the ethos tools, not `{other}`"
        ),
    }
}

#[when(expr = "the agent calls {string} twice")]
async fn briefing_agent_calls_twice(w: &mut GatewayWorld, tool: String) {
    assert_eq!(tool, BRIEFING_READ);
    briefing_call(w, json!({})).await;
    briefing_call(w, json!({})).await;
}

#[when(expr = "the agent calls {string} again")]
async fn briefing_agent_calls_again(w: &mut GatewayWorld, tool: String) {
    assert_eq!(tool, BRIEFING_READ);
    briefing_call(w, json!({})).await;
}

#[when(expr = "the agent calls {string} with an unknown argument field")]
async fn briefing_agent_calls_unknown_field(w: &mut GatewayWorld, tool: String) {
    match tool.as_str() {
        BRIEFING_READ => briefing_call(w, json!({ "audience": "everyone" })).await,
        ETHOS_READ => ethos_call(w, ETHOS_READ, json!({ "audience": "everyone" })).await,
        other => panic!("the unknown-field step serves briefing.read or ethos.read, not `{other}`"),
    }
}

#[when(expr = "the owner rewrites the directive to {string}")]
async fn briefing_owner_rewrites(w: &mut GatewayWorld, text: String) {
    let master = w.master();
    let store = w.ctx_stores.get("ventes").expect("ventes store").clone();
    let ent = w.briefing_owner_ent.as_mut().expect("owner entropy");
    owner_set_briefing(
        &master, "ventes", "circle", "Consigne", &text, store, T0, ent,
    )
    .expect("owner rewrite lands");
    w.briefing_rewritten = Some(text);
}

#[then(
    expr = "the initialize result carries instructions recommending {string} before outbound actions"
)]
async fn briefing_init_recommends(w: &mut GatewayWorld, tool: String) {
    let init = w.last_init.as_ref().expect("an initialize answer");
    let instructions = init
        .pointer("/result/instructions")
        .and_then(Value::as_str)
        .expect("initialize carries instructions");
    assert!(
        instructions.contains(&tool),
        "the instructions name the tool: {instructions}"
    );
    assert!(
        instructions.contains("before") && instructions.contains("outbound"),
        "the instructions say to look before outbound actions: {instructions}"
    );
}

#[then("the initialize result carries no instructions")]
async fn briefing_init_mute(w: &mut GatewayWorld) {
    let init = w.last_init.as_ref().expect("an initialize answer");
    assert!(
        init.pointer("/result/instructions").is_none(),
        "a mute surface recommends nothing"
    );
}

#[then(
    expr = "the list includes {string} with a description that says to consult it before acting"
)]
async fn briefing_listed_with_description(w: &mut GatewayWorld, tool: String) {
    let listed = w.listed_tools();
    let descriptor = listed
        .iter()
        .find(|t| t["name"] == tool.as_str())
        .expect("the briefing tool is listed");
    let description = descriptor["description"].as_str().expect("a description");
    assert!(
        description.to_lowercase().contains("consult") && description.contains("before acting"),
        "the description says to consult it before acting: {description}"
    );
}

#[then(expr = "the answer carries both directives verbatim under the {string} label")]
async fn briefing_answer_both_directives(w: &mut GatewayWorld, context: String) {
    let text = w.last_result_text();
    assert!(text.contains(&context), "the context label rides along");
    for directive in [w.briefing_texts("public"), w.briefing_texts("circle")].concat() {
        assert!(
            text.contains(&directive),
            "the exact owner text is served: {directive}"
        );
    }
}

#[then("the answer names the zone of each directive")]
async fn briefing_answer_names_zones(w: &mut GatewayWorld) {
    let text = w.last_result_text();
    for zone in ["public", "circle"] {
        if !w.briefing_texts(zone).is_empty() {
            assert!(text.contains(zone), "the `{zone}` zone is named");
        }
    }
}

#[then("the answer carries the circle directive")]
async fn briefing_answer_circle(w: &mut GatewayWorld) {
    let text = w.last_result_text();
    for directive in w.briefing_texts("circle") {
        assert!(text.contains(&directive), "the circle directive is served");
    }
}

#[then(expr = "the {string} context gamma gains exactly two read entries for the briefing")]
async fn briefing_two_reads_journalized(w: &mut GatewayWorld, context: String) {
    let reads: Vec<EntryView> = w
        .ctx_gamma(&context)
        .into_iter()
        .filter(|e| e.kind == "ethos.read")
        .collect();
    assert_eq!(reads.len(), 2, "one journalized read per served section");
}

#[then("each entry is covered by the agent's read mandate")]
async fn briefing_reads_covered(w: &mut GatewayWorld) {
    let pen = w.briefing_mandates.get("ventes").expect("briefing pen");
    let reads: Vec<EntryView> = w
        .ctx_gamma("ventes")
        .into_iter()
        .filter(|e| e.kind == "ethos.read")
        .collect();
    assert!(!reads.is_empty(), "there are journalized reads to check");
    for entry in reads {
        assert_eq!(
            entry.authorized_via.as_deref(),
            Some(std::slice::from_ref(pen)),
            "the read rides the briefing pen"
        );
    }
}

#[then(expr = "the answer carries the rewritten directive verbatim")]
async fn briefing_answer_rewritten(w: &mut GatewayWorld) {
    let rewritten = w.briefing_rewritten.clone().expect("a rewrite happened");
    let text = w.last_result_text();
    assert!(
        text.contains(&rewritten),
        "the rewritten directive is served: {text}"
    );
}

#[then("the previous wording appears nowhere in the answer")]
async fn briefing_old_wording_gone(w: &mut GatewayWorld) {
    let text = w.last_result_text();
    let old = w
        .briefing_texts("circle")
        .first()
        .cloned()
        .expect("the original directive");
    assert!(!text.contains(&old), "the pre-edit wording is gone: {text}");
}

#[when(expr = "a hub config declares a server or a tool named under the {string} prefix")]
async fn briefing_reserved_config(w: &mut GatewayWorld, prefix: String) {
    // One hub shape naming the SERVER after the prefix, one legacy map
    // naming a TOOL under it — both must die at the config door.
    let server_shape = format!(
        "listen: 127.0.0.1:4877\nservers:\n  - name: {prefix}\n    transport: http\n    url: http://127.0.0.1:9/mcp\ncontexts:\n  - name: ventes\n    store: {{ kind: fs, root: /tmp/ventes }}\n    tools:\n      {prefix}__x: {{ server: {prefix}, tool: x, access: read }}\njournal:\n  store: {{ kind: fs, root: /tmp/journal }}\n"
    );
    let tool_shape = format!(
        "listen: 127.0.0.1:4877\ncontexts:\n  - name: ventes\n    upstream_mcp: http://127.0.0.1:9/mcp\n    store: {{ kind: fs, root: /tmp/ventes }}\n    tools:\n      {prefix}.read: read\njournal:\n  store: {{ kind: fs, root: /tmp/journal }}\n"
    );
    let mut verdicts = Vec::new();
    for text in [server_shape, tool_shape] {
        match GatewayConfig::from_yaml(&text) {
            Ok(_) => panic!("a `{prefix}`-named surface must be rejected"),
            Err(e) => {
                let verdict = e.to_string();
                assert!(
                    verdict.contains("reserved"),
                    "each shape dies for the reservation, not another rule: {verdict}"
                );
                verdicts.push(verdict);
            }
        }
    }
    w.config_error = Some(verdicts.join("\n"));
}

#[then("the call is refused naming the unknown field")]
async fn briefing_unknown_field_refused(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response");
    assert_eq!(response["error"]["code"], POLICY_DENIED_CODE);
    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("unknown field") && message.contains("audience"),
        "the refusal names the field: {message}"
    );
}

#[then("the refusal is journalized")]
async fn briefing_refusal_journalized(w: &mut GatewayWorld) {
    let refusals: Vec<EntryView> = w
        .journal_gamma()
        .into_iter()
        .filter(|e| {
            e.kind == "action"
                && e.target.as_deref() == Some("x.gateway")
                && payload_str(e, "tool") == Some(w.last_tool.as_str())
        })
        .collect();
    assert_eq!(refusals.len(), 1, "the journal records the refusal");
}

fn bounds_server_for(raw_tool: &str) -> &'static str {
    match raw_tool {
        "send_email" => "gmail",
        "repo_admin" => "github",
        "create_event" => "calendar",
        other => panic!("unknown bounds tool `{other}`"),
    }
}

fn bounds_fixture(raw_tool: &str) -> Value {
    match raw_tool {
        "send_email" => json!({
            "name": "send_email",
            "description": "Send an email",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": { "type": "array", "items": { "type": "string" } },
                    "cc": { "type": "array", "items": { "type": "string" } },
                    "bcc": { "type": "array", "items": { "type": "string" } },
                    "subject": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["to"],
                "additionalProperties": false
            }
        }),
        "repo_admin" => json!({
            "name": "repo_admin",
            "description": "Administer one repository item",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string" },
                    "target": { "type": "string" }
                },
                "required": ["action"],
                "additionalProperties": false
            }
        }),
        "create_event" => json!({
            "name": "create_event",
            "description": "Create one calendar event",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["start"],
                "additionalProperties": false
            }
        }),
        other => panic!("unknown bounds fixture `{other}`"),
    }
}

/// One governed server, one granted write tool with bounds, the vault
/// holding its bearer — the smallest world every bounds scenario needs.
async fn provision_bounds_world(w: &mut GatewayWorld, raw_tool: &str, bounds: Vec<ArgumentBound>) {
    use aithos_gateway::credentials::build_brokers;
    use aithos_gateway::proxy_mcp::HttpUpstream;

    if w.vault.is_some() {
        return;
    }
    static BOUNDS_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = BOUNDS_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let env_name = format!("AITHOS_CUCUMBER_BOUNDS_TOKEN_{seq}");
    let vault_token = format!("vault-access-bounds-{seq}");
    std::env::set_var(&env_name, &vault_token);
    let server = bounds_server_for(raw_tool);

    let dir = tempfile::tempdir().expect("bounds tempdir");
    let context_root = dir.path().join(BOUNDS_CONTEXT);
    let journal_root = dir.path().join("journal");
    let store_cfg = |root: &std::path::Path| aithos_gateway::config::StoreConfig::Fs {
        root: root.to_owned(),
    };
    let context_store =
        GatewayStore::from_config(&store_cfg(&context_root)).expect("bounds context store");
    let journal_store =
        GatewayStore::from_config(&store_cfg(&journal_root)).expect("bounds journal store");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let mut owner_ent = SeqEntropy::default();
    owner_init_context(
        &master,
        BOUNDS_CONTEXT,
        context_store.clone(),
        T0,
        &mut owner_ent,
    )
    .expect("bounds context created");
    let discovery = FakeMcp::advertising(vec![bounds_fixture(raw_tool)]);
    let proposed = discover_server(server, &discovery)
        .await
        .expect("bounds discovery");
    let approved = approve_manifest(
        &proposed,
        &BTreeMap::from([(
            raw_tool.to_owned(),
            ToolApproval::granted(ToolAccess::Write).with_bounds(bounds),
        )]),
    )
    .expect("bounds approval");
    owner_enroll_server(
        &master,
        BOUNDS_CONTEXT,
        &agent_pub,
        &gateway_pub,
        &approved,
        context_store.clone(),
        &GatewayWorld::window(),
        T0,
        &mut owner_ent,
    )
    .expect("bounds enrollment");
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
    .expect("bounds journal");

    let fake_vault = FakeVault {
        expected_token: vault_token.clone(),
        ..FakeVault::default()
    };
    fake_vault
        .secrets
        .lock()
        .unwrap()
        .entry(format!("aithos/mcp/{server}"))
        .or_default()
        .insert("token".to_owned(), format!("wire-bearer-bounds-{seq}"));
    let vault_port = spawn_fake_vault(fake_vault.clone()).await;
    let wire = WireMcp::new("bound-ok");
    let wire_port = spawn_wire_mcp(wire.clone()).await;

    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let exposed = aithos_gateway::policy::hub_exposed_name(server, raw_tool);
    let config_text = format!(
        "listen: 127.0.0.1:4870\ncredential_brokers:\n  enterprise:\n    kind: vault-kv2\n    address: http://127.0.0.1:{vault_port}\n    mount: secret\n    auth:\n      kind: token-env\n      env: {env_name}\nservers:\n  - name: {server}\n    transport: http\n    url: http://127.0.0.1:{wire_port}/mcp\n    credential:\n      broker: enterprise\n      path: aithos/mcp/{server}\n      field: token\ncontexts:\n  - name: {BOUNDS_CONTEXT}\n    store: {{ kind: fs, root: {} }}\n    tools:\n      {exposed}: {{ server: {server}, tool: {raw_tool}, access: write, granted: true }}\njournal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(&context_root),
        quote(&journal_root)
    );
    let config_path = dir.path().join("gateway.yaml");
    std::fs::write(&config_path, &config_text).expect("bounds config written");
    let cfg = GatewayConfig::from_yaml(&config_text).expect("bounds config parses");
    let brokers = build_brokers(&cfg).expect("bounds brokers");
    let upstreams: BTreeMap<String, HttpUpstream> = cfg
        .servers
        .as_ref()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry.name.clone(),
                HttpUpstream::for_server(entry, &brokers).expect("bounds upstream"),
            )
        })
        .collect();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner = Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
        .expect("bounds governed runner");

    w.ctx_stores
        .insert(BOUNDS_CONTEXT.to_owned(), context_store.clone());
    w.journal_store = Some(journal_store);
    w.grants_store = Some(context_store);
    w.approved_manifest = Some(approved);
    w.vault = Some(VaultHarness {
        router: Arc::new(McpRouter {
            runner: Arc::new(Mutex::new(runner)),
            upstreams,
            clock: Arc::new(|| T0.to_owned()),
            session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
            oauth: None,
        }),
        vault: fake_vault,
        wires: BTreeMap::from([(server.to_owned(), wire)]),
        config_path,
        config_text,
        vault_token,
        store_roots: vec![context_root, journal_root],
        responses: Vec::new(),
    });
    w.scratch = Some(dir);
}

/// Reopen the bounds runtime after a re-enrollment: same config text,
/// freshly loaded sealed manifest (the narrowed bounds), clean wires.
fn reopen_bounds_runtime(w: &mut GatewayWorld) {
    use aithos_gateway::credentials::build_brokers;
    use aithos_gateway::proxy_mcp::HttpUpstream;

    let harness = w.vault.as_mut().expect("bounds harness");
    let cfg = GatewayConfig::from_yaml(&harness.config_text).expect("bounds config reparses");
    let brokers = build_brokers(&cfg).expect("bounds brokers rebuild");
    let upstreams: BTreeMap<String, HttpUpstream> = cfg
        .servers
        .as_ref()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry.name.clone(),
                HttpUpstream::for_server(entry, &brokers).expect("bounds upstream rebuild"),
            )
        })
        .collect();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner = Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default()))
        .expect("bounds runner reopens on the new pin");
    for wire in harness.wires.values() {
        wire.requests.lock().unwrap().clear();
        wire.auths.lock().unwrap().clear();
    }
    harness.router = Arc::new(McpRouter {
        runner: Arc::new(Mutex::new(runner)),
        upstreams,
        clock: Arc::new(|| T0.to_owned()),
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: None,
    });
}

async fn bounds_call(w: &mut GatewayWorld, tool: &str, args: Value) {
    w.last_args = Some(args.clone());
    w.call(tool, json!({ "arguments": args })).await;
}

impl GatewayWorld {
    /// The exposed name of the bounds world's single tool.
    fn bounds_exposed(&self, raw_tool: &str) -> String {
        aithos_gateway::policy::hub_exposed_name(bounds_server_for(raw_tool), raw_tool)
    }
}

#[given(
    expr = "tool {string} is granted write with a one_of bound on {string} allowing {string}, {string} and {string}"
)]
async fn bounds_one_of_three(
    w: &mut GatewayWorld,
    tool: String,
    field: String,
    first: String,
    second: String,
    third: String,
) {
    provision_bounds_world(
        w,
        &tool,
        vec![ArgumentBound::OneOf {
            field,
            values: vec![first, second, third],
        }],
    )
    .await;
}

#[given(expr = "tool {string} is granted write with a one_of bound on {string} allowing {string}")]
#[given(
    expr = "tool {string} is granted write with a one_of bound on field {string} allowing {string}"
)]
async fn bounds_one_of_single(w: &mut GatewayWorld, tool: String, field: String, value: String) {
    provision_bounds_world(
        w,
        &tool,
        vec![ArgumentBound::OneOf {
            field,
            values: vec![value],
        }],
    )
    .await;
}

#[given(expr = "tool {string} is granted write with a one_of bound on {string}")]
async fn bounds_one_of_default(w: &mut GatewayWorld, tool: String, field: String) {
    provision_bounds_world(
        w,
        &tool,
        vec![ArgumentBound::OneOf {
            field,
            values: BOUNDS_APPROVED
                .iter()
                .map(|value| value.to_string())
                .collect(),
        }],
    )
    .await;
}

#[given(
    expr = "tool {string} is granted write with a one_of bound on {string} allowing three prospects"
)]
async fn bounds_one_of_three_prospects(w: &mut GatewayWorld, tool: String, field: String) {
    bounds_one_of_default(w, tool, field).await;
}

#[given(
    expr = "tool {string} is granted write with time slots {string} and {string} from {string} to {string} on field {string}"
)]
async fn bounds_time_slots(
    w: &mut GatewayWorld,
    tool: String,
    first_day: String,
    second_day: String,
    from: String,
    to: String,
    field: String,
) {
    provision_bounds_world(
        w,
        &tool,
        vec![ArgumentBound::TimeSlots {
            field,
            days: vec![first_day, second_day],
            from,
            to,
        }],
    )
    .await;
}

#[given(expr = "tool {string} is granted write with bound {string}")]
async fn bounds_mini_spec(w: &mut GatewayWorld, tool: String, spec: String) {
    let bound = match spec.as_str() {
        "forbid bcc" => ArgumentBound::Forbid {
            field: "bcc".into(),
        },
        "require subject" => ArgumentBound::Require {
            field: "subject".into(),
        },
        "to max_items 3" => ArgumentBound::MaxItems {
            field: "to".into(),
            max: 3,
        },
        other => panic!("unknown bound spec `{other}`"),
    };
    provision_bounds_world(w, &tool, vec![bound]).await;
}

#[when(expr = "the agent calls {string} with recipients {string} and {string}")]
async fn bounds_call_two_recipients(
    w: &mut GatewayWorld,
    tool: String,
    first: String,
    second: String,
) {
    bounds_call(
        w,
        &tool,
        json!({ "to": [first, second], "subject": "Visite", "body": "Bonjour" }),
    )
    .await;
}

#[when(expr = "the agent calls {string} with recipients including {string} and {string}")]
async fn bounds_call_including_intruders(
    w: &mut GatewayWorld,
    tool: String,
    first_intruder: String,
    second_intruder: String,
) {
    let mut recipients: Vec<String> = BOUNDS_APPROVED
        .iter()
        .map(|value| value.to_string())
        .collect();
    recipients.push(first_intruder);
    recipients.push(second_intruder);
    bounds_call(
        w,
        &tool,
        json!({ "to": recipients, "subject": "Visite", "body": "Bonjour" }),
    )
    .await;
}

#[when(expr = "the agent calls {string} with action {string}")]
async fn bounds_call_action(w: &mut GatewayWorld, tool: String, action: String) {
    bounds_call(w, &tool, json!({ "action": action, "target": "pr-42" })).await;
}

#[when(expr = "the agent calls {string} starting {string}")]
async fn bounds_call_starting(w: &mut GatewayWorld, tool: String, start: String) {
    bounds_call(
        w,
        &tool,
        json!({ "start": start, "title": "Visite du bien" }),
    )
    .await;
}

#[when(expr = "the agent calls {string} with arguments {string}")]
async fn bounds_call_shaped(w: &mut GatewayWorld, tool: String, shape: String) {
    let args = match shape.as_str() {
        "a bcc field" => json!({
            "to": ["someone@clients.example"],
            "subject": "Visite",
            "bcc": ["hidden@clients.example"]
        }),
        "no subject field" => json!({ "to": ["someone@clients.example"] }),
        "four whitelisted recipients" => json!({
            "to": ["r1@clients.example", "r2@clients.example",
                    "r3@clients.example", "r4@clients.example"],
            "subject": "Visite"
        }),
        other => panic!("unknown argument shape `{other}`"),
    };
    bounds_call(w, &tool, args).await;
}

#[when(expr = "the agent calls {string} with {string} as a single string instead of an array")]
async fn bounds_call_mistyped(w: &mut GatewayWorld, tool: String, field: String) {
    bounds_call(
        w,
        &tool,
        json!({ field: "prospect-a@clients.example", "subject": "Visite" }),
    )
    .await;
}

#[when(expr = "the agent calls {string} without any {string} field")]
async fn bounds_call_without_field(w: &mut GatewayWorld, tool: String, field: String) {
    assert_eq!(field, "cc");
    bounds_call(
        w,
        &tool,
        json!({ "to": ["prospect-a@clients.example"], "subject": "Visite" }),
    )
    .await;
}

#[then("the call is refused as a bound violation")]
async fn bounds_refused(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("bound violated")),
        "the refusal carries the bound code: {response}"
    );
}

#[then(expr = "the call is refused as a bound violation naming the expected shape of {string}")]
async fn bounds_refused_shape(w: &mut GatewayWorld, field: String) {
    let response = w.last_response.as_ref().expect("a response");
    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("bound violated")
            && message.contains(&format!(".{field}`"))
            && message.contains("must be an array of strings"),
        "the refusal names the expected shape: {message}"
    );
}

#[then(expr = "the refusal names field {string}, the offending values and the approved set")]
async fn bounds_refusal_pedagogical(w: &mut GatewayWorld, field: String) {
    let message = w.last_response.as_ref().expect("a response")["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        message.contains(&format!(".{field}`")),
        "names the field: {message}"
    );
    assert!(
        message.contains("prospect-d@clients.example")
            && message.contains("prospect-e@clients.example"),
        "names every offender: {message}"
    );
    for approved in BOUNDS_APPROVED {
        assert!(
            message.contains(approved),
            "teaches the approved set: {message}"
        );
    }
}

#[then(expr = "the refusal names field {string}, value {string} and the allowed actions")]
async fn bounds_refusal_action(w: &mut GatewayWorld, field: String, value: String) {
    let message = w.last_response.as_ref().expect("a response")["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        message.contains(&format!(".{field}`"))
            && message.contains(&value)
            && message.contains("comment"),
        "names field, value and allowed actions: {message}"
    );
}

#[then(expr = "the refusal names field {string}, the offending instant and the approved slots")]
async fn bounds_refusal_slots(w: &mut GatewayWorld, field: String) {
    let message = w.last_response.as_ref().expect("a response")["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let instant = w
        .last_args
        .as_ref()
        .and_then(|args| args["start"].as_str())
        .expect("the offending instant")
        .to_owned();
    assert!(
        message.contains(&format!(".{field}`"))
            && message.contains(&instant)
            && message.contains("tuesday")
            && message.contains("thursday")
            && message.contains("14:00")
            && message.contains("18:00"),
        "names instant and slots: {message}"
    );
}

#[then("the call reaches the upstream with its arguments unmodified")]
async fn bounds_call_untouched(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response");
    let hits = w.vault_harness().vault.hits.lock().unwrap().clone();
    let tokens = w.vault_harness().vault.tokens.lock().unwrap().clone();
    assert!(
        response.get("error").is_none(),
        "the bounded call passes: {response} (vault hits: {hits:?}, tokens: {tokens:?})"
    );
    let harness = w.vault_harness();
    let sent = w.last_args.as_ref().expect("the sent arguments");
    let wire = harness.wires.values().next().expect("one wire");
    let last = wire
        .requests
        .lock()
        .unwrap()
        .iter()
        .rfind(|body| body["method"] == "tools/call")
        .cloned()
        .expect("one relayed call");
    assert_eq!(
        last.pointer("/params/arguments"),
        Some(sent),
        "arguments relay byte-identical — the gateway never rewrites"
    );
}

#[then("the act is logged in the granting context gamma")]
async fn bounds_act_logged(w: &mut GatewayWorld) {
    let server = w
        .vault_harness()
        .wires
        .keys()
        .next()
        .expect("one wired server")
        .clone();
    let target = format!("x.{server}");
    assert_eq!(
        acts_on(&w.ctx_gamma(BOUNDS_CONTEXT), &target).len(),
        1,
        "exactly one act in the granting context"
    );
}

#[then(expr = "the context gamma and the journal each gain one {string} refusal")]
async fn bounds_refusal_logged(w: &mut GatewayWorld, reason: String) {
    let context_refusals: Vec<EntryView> = acts_on(&w.ctx_gamma(BOUNDS_CONTEXT), "x.gateway")
        .into_iter()
        .filter(|entry| payload_str(entry, "reason") == Some(reason.as_str()))
        .collect();
    assert_eq!(context_refusals.len(), 1, "one context `{reason}` refusal");
    let journal_refusals: Vec<EntryView> = acts_on(&w.journal_gamma(), "x.gateway")
        .into_iter()
        .filter(|entry| payload_str(entry, "reason") == Some(reason.as_str()))
        .collect();
    assert_eq!(journal_refusals.len(), 1, "one journal `{reason}` refusal");
}

#[then("the runtime config text declares no bound at all")]
async fn bounds_config_is_boundless(w: &mut GatewayWorld) {
    let config = &w.vault_harness().config_text;
    assert!(
        !config.contains("one_of") && !config.contains("bounds") && !config.contains("prospect-"),
        "the YAML carries topology and references only"
    );
}

#[then("the sealed manifest of the granting context records the bound")]
async fn bounds_sealed_in_manifest(w: &mut GatewayWorld) {
    let master = w.master();
    let store = w.grants_store.clone().expect("bounds store");
    let manifest = owner_read_hub_manifest(&master, BOUNDS_CONTEXT, "gmail", store)
        .expect("sealed manifest opens owner-side");
    let tool = &manifest.tools[0];
    assert!(
        matches!(tool.bounds.first(), Some(ArgumentBound::OneOf { field, .. }) if field == "to"),
        "the bound is sealed with the manifest"
    );
}

#[then("the upstream pin hash is unchanged by the bound")]
async fn bounds_pin_unchanged(w: &mut GatewayWorld) {
    let manifest = w.approved_manifest.as_ref().expect("approved manifest");
    let tool = &manifest.tools[0];
    let recomputed = aithos_gateway::core_bridge::manifest_tool_pin(
        &tool.name,
        tool.description.as_deref(),
        &tool.input_schema,
    )
    .expect("pin recomputes");
    assert_eq!(
        tool.pin_sha256, recomputed,
        "the pin covers the upstream's word only — bounds live outside it"
    );
}

#[given(expr = "the agent has sent to {string} once")]
async fn bounds_sent_once(w: &mut GatewayWorld, recipient: String) {
    let exposed = w.bounds_exposed("send_email");
    bounds_call(
        w,
        &exposed,
        json!({ "to": [recipient], "subject": "Visite" }),
    )
    .await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(response.get("error").is_none(), "the first send passes");
}

#[when(
    expr = "the owner re-enrolls {string} narrowing the bound to {string} for the same agent key"
)]
async fn bounds_reenroll_narrowed(w: &mut GatewayWorld, server: String, survivor: String) {
    assert_eq!(server, "gmail");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let store = w.grants_store.clone().expect("bounds store");
    let state: Value = serde_json::from_slice(
        &store
            .clone()
            .get(STATE_PATH)
            .expect("state readable")
            .expect("state present"),
    )
    .expect("state JSON");
    w.old_agent_mandate = Some(state["agent_mandate"].as_str().unwrap().to_owned());
    let discovery = FakeMcp::advertising(vec![bounds_fixture("send_email")]);
    let proposed = discover_server("gmail", &discovery)
        .await
        .expect("re-discovery");
    let approved = approve_manifest(
        &proposed,
        &BTreeMap::from([(
            "send_email".to_owned(),
            ToolApproval::granted(ToolAccess::Write).with_bounds(vec![ArgumentBound::OneOf {
                field: "to".into(),
                values: vec![survivor],
            }]),
        )]),
    )
    .expect("narrowed approval");
    let mut ent = SeqEntropy::default();
    let outcome = owner_reenroll_server(
        &master,
        BOUNDS_CONTEXT,
        &agent_pub,
        &gateway_pub,
        &approved,
        store,
        &GatewayWorld::window(),
        T0,
        &mut ent,
    )
    .expect("narrowing re-enrollment");
    w.reenroll = Some(outcome);
    w.approved_manifest = Some(approved);
    reopen_bounds_runtime(w);
}

#[then(expr = "a call to {string} is now refused as a bound violation")]
async fn bounds_call_now_refused(w: &mut GatewayWorld, recipient: String) {
    let exposed = w.bounds_exposed("send_email");
    bounds_call(
        w,
        &exposed,
        json!({ "to": [recipient], "subject": "Visite" }),
    )
    .await;
    bounds_refused(w).await;
}

#[then(expr = "a call to {string} still passes")]
async fn bounds_call_still_passes(w: &mut GatewayWorld, recipient: String) {
    let exposed = w.bounds_exposed("send_email");
    bounds_call(
        w,
        &exposed,
        json!({ "to": [recipient], "subject": "Visite" }),
    )
    .await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response.get("error").is_none(),
        "the surviving recipient passes: {response}"
    );
}

#[when(expr = "the owner approves {string} as a denied write carrying a one_of bound")]
async fn bounds_approve_ungranted(w: &mut GatewayWorld, tool: String) {
    let discovery = FakeMcp::advertising(vec![json!({
        "name": tool,
        "description": "Delete an email",
        "inputSchema": { "type": "object", "additionalProperties": false }
    })]);
    let proposed = discover_server("gmail", &discovery)
        .await
        .expect("discovery");
    let verdict = approve_manifest(
        &proposed,
        &BTreeMap::from([(
            tool,
            ToolApproval::denied(ToolAccess::Write).with_bounds(vec![ArgumentBound::OneOf {
                field: "id".into(),
                values: vec!["x".into()],
            }]),
        )]),
    );
    w.config_error = Some(
        verdict
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default(),
    );
}

#[then("the approval is rejected naming the ungranted bound")]
async fn bounds_ungranted_rejected(w: &mut GatewayWorld) {
    let err = w.config_error.as_deref().expect("an approval verdict");
    assert!(
        err.contains("did not grant"),
        "the rejection names the ungranted bound: {err}"
    );
}

// ------------------------------------------------- demo Léa world (lot D)
//
// The dress rehearsal: the fusion of the worlds that came before it —
// the bounds world's real wires (fake Vault + wire MCPs on sockets,
// real brokers, real router) generalised to THREE servers under ONE
// "ventes" context, plus the briefing world's owner directives. Every
// upstream is deliberately permissive and fully credentialed: whatever
// restriction the beats show is the gateway's alone.

/// The owner-only self note (never served — the sentinel the leak
/// assertions sweep for).
const DEMO_SELF_NOTE: &str = "Marge de negociation interne max 8% — owner only.";

fn demo_fixture(server: &str, tool: &str) -> Value {
    match (server, tool) {
        ("notion", "query_database") => json!({
            "name": "query_database",
            "description": "Query one Notion database",
            "inputSchema": {
                "type": "object",
                "properties": { "database": { "type": "string" } },
                "additionalProperties": false
            }
        }),
        ("notion", "create_page") => json!({
            "name": "create_page",
            "description": "Create one Notion page",
            "inputSchema": {
                "type": "object",
                "properties": { "title": { "type": "string" } },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
        ("gmail", "search_emails") => json!({
            "name": "search_emails",
            "description": "Search the mailbox",
            "inputSchema": {
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "additionalProperties": false
            }
        }),
        ("gmail", "send_email") => json!({
            "name": "send_email",
            "description": "Send an email",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": { "type": "array", "items": { "type": "string" } },
                    "cc": { "type": "array", "items": { "type": "string" } },
                    "bcc": { "type": "array", "items": { "type": "string" } },
                    "subject": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["to"],
                "additionalProperties": false
            }
        }),
        ("gmail", "delete_email") => json!({
            "name": "delete_email",
            "description": "Delete one email",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        ("calendar", "list_events") => json!({
            "name": "list_events",
            "description": "List calendar events",
            "inputSchema": {
                "type": "object",
                "properties": { "from": { "type": "string" }, "to": { "type": "string" } },
                "additionalProperties": false
            }
        }),
        ("calendar", "create_event") => json!({
            "name": "create_event",
            "description": "Create one calendar event",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["start"],
                "additionalProperties": false
            }
        }),
        other => panic!("unknown demo fixture {other:?}"),
    }
}

/// Parse the Background table's bounds mini-language:
/// `to one_of {a,b,c}; bcc forbid; to max 3; subject require` and
/// `start slots tue,thu 14:00-18:00`.
fn parse_demo_bounds(text: &str) -> Vec<ArgumentBound> {
    let day = |short: &str| -> String {
        match short {
            "mon" => "monday",
            "tue" => "tuesday",
            "wed" => "wednesday",
            "thu" => "thursday",
            "fri" => "friday",
            "sat" => "saturday",
            "sun" => "sunday",
            other => other,
        }
        .to_owned()
    };
    text.split(';')
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(|rule| {
            let mut words = rule.split_whitespace();
            let field = words.next().expect("bound field").to_owned();
            match words.next().expect("bound kind") {
                "one_of" => {
                    let set = rule
                        .split_once('{')
                        .and_then(|(_, rest)| rest.split_once('}'))
                        .map(|(inner, _)| inner)
                        .expect("one_of set");
                    ArgumentBound::OneOf {
                        field,
                        values: set.split(',').map(|v| v.trim().to_owned()).collect(),
                    }
                }
                "forbid" => ArgumentBound::Forbid { field },
                "require" => ArgumentBound::Require { field },
                "max" => ArgumentBound::MaxItems {
                    field,
                    max: words.next().expect("max size").parse().expect("a number"),
                },
                "slots" => {
                    let days = words
                        .next()
                        .expect("slot days")
                        .split(',')
                        .map(day)
                        .collect();
                    let window = words.next().expect("slot window");
                    let (from, to) = window.split_once('-').expect("HH:MM-HH:MM");
                    ArgumentBound::TimeSlots {
                        field,
                        days,
                        from: from.to_owned(),
                        to: to.to_owned(),
                    }
                }
                other => panic!("unknown bound rule `{other}` in `{rule}`"),
            }
        })
        .collect()
}

#[given("the Innoestate demo world is provisioned:")]
async fn demo_table(w: &mut GatewayWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("a provisioning table");
    for row in table.rows.iter().skip(1) {
        let bounds_text = row.get(4).map(String::as_str).unwrap_or_default();
        w.demo_specs.push(DemoToolSpec {
            server: row[0].clone(),
            tool: row[1].clone(),
            class: match row[2].as_str() {
                "read" => ToolAccess::Read,
                "write" => ToolAccess::Write,
                other => panic!("unknown class `{other}`"),
            },
            granted: match row[3].as_str() {
                "granted" => true,
                "denied" => false,
                other => panic!("unknown decision `{other}`"),
            },
            bounds: parse_demo_bounds(bounds_text),
        });
    }
    assert!(!w.demo_specs.is_empty(), "the table provisions tools");
}

#[given("the vault stores one distinct token per server")]
async fn demo_vault_tokens(w: &mut GatewayWorld) {
    // The provisioning mints one wire bearer per server by construction;
    // this Given pins the intent (asserted live in beats 2, 4 and 5).
    assert!(!w.demo_specs.is_empty(), "the table comes first");
}

#[given(
    expr = "the notion database holds prospects {string}, {string}, {string}, {string} and {string}"
)]
async fn demo_prospects(
    w: &mut GatewayWorld,
    a: String,
    b: String,
    c: String,
    d: String,
    e: String,
) {
    w.demo_prospects = vec![a, b, c, d, e];
}

#[given(expr = "the {string} circle zone directs {string}")]
async fn demo_circle_directive(w: &mut GatewayWorld, context: String, text: String) {
    assert_eq!(context, "ventes");
    w.demo_directive = Some(text);
}

#[given(expr = "the {string} self zone holds an owner-only note")]
async fn demo_self_note(w: &mut GatewayWorld, context: String) {
    assert_eq!(context, "ventes");
    w.demo_note = Some(DEMO_SELF_NOTE.to_owned());
}

async fn provision_demo_world(w: &mut GatewayWorld) {
    use aithos_gateway::credentials::build_brokers;
    use aithos_gateway::proxy_mcp::HttpUpstream;

    if w.vault.is_some() {
        return;
    }
    static DEMO_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = DEMO_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let env_name = format!("AITHOS_CUCUMBER_DEMO_TOKEN_{seq}");
    let vault_token = format!("vault-access-demo-{seq}");
    std::env::set_var(&env_name, &vault_token);

    let dir = tempfile::tempdir().expect("demo tempdir");
    let context_root = dir.path().join("ventes");
    let journal_root = dir.path().join("journal");
    let store_cfg = |root: &std::path::Path| aithos_gateway::config::StoreConfig::Fs {
        root: root.to_owned(),
    };
    let context_store =
        GatewayStore::from_config(&store_cfg(&context_root)).expect("demo context store");
    let journal_store =
        GatewayStore::from_config(&store_cfg(&journal_root)).expect("demo journal store");
    let master = w.master();
    let (agent_pub, gateway_pub) = w.pubs();
    let window = GatewayWorld::window();
    let mut owner_ent = SeqEntropy::default();

    owner_init_context(&master, "ventes", context_store.clone(), T0, &mut owner_ent)
        .expect("ventes context created");

    // One approved manifest per server, from the Background table.
    let servers: Vec<String> = {
        let mut seen = Vec::new();
        for spec in &w.demo_specs {
            if !seen.contains(&spec.server) {
                seen.push(spec.server.clone());
            }
        }
        seen
    };
    let mut approved_manifests = Vec::new();
    for server in &servers {
        let fixtures: Vec<Value> = w
            .demo_specs
            .iter()
            .filter(|spec| &spec.server == server)
            .map(|spec| demo_fixture(server, &spec.tool))
            .collect();
        let discovery = FakeMcp::advertising(fixtures);
        let proposed = discover_server(server, &discovery)
            .await
            .expect("demo discovery");
        let approvals: BTreeMap<String, ToolApproval> = w
            .demo_specs
            .iter()
            .filter(|spec| &spec.server == server)
            .map(|spec| {
                let approval = if spec.granted {
                    ToolApproval::granted(spec.class)
                } else {
                    ToolApproval::denied(spec.class)
                };
                (spec.tool.clone(), approval.with_bounds(spec.bounds.clone()))
            })
            .collect();
        approved_manifests.push(approve_manifest(&proposed, &approvals).expect("demo approval"));
    }
    let outcome = aithos_gateway::core_bridge::owner_enroll_servers(
        &master,
        "ventes",
        &agent_pub,
        &gateway_pub,
        &approved_manifests,
        context_store.clone(),
        &window,
        T0,
        &mut owner_ent,
    )
    .expect("demo batch enrollment");
    w.demo_auditor_seed = outcome.auditor_seed_hex.clone();

    // The briefing: pen + circle directive + owner-only self note.
    let pen = owner_grant_briefing(
        &master,
        "ventes",
        &agent_pub,
        context_store.clone(),
        &window,
        T0,
        &mut owner_ent,
    )
    .expect("demo briefing pen");
    w.briefing_mandates.insert("ventes".to_owned(), pen);
    if let Some(text) = w.demo_directive.clone() {
        owner_set_briefing(
            &master,
            "ventes",
            "circle",
            "Consigne commerciale",
            &text,
            context_store.clone(),
            T0,
            &mut owner_ent,
        )
        .expect("demo circle directive");
    }
    if let Some(note) = w.demo_note.clone() {
        owner_set_briefing(
            &master,
            "ventes",
            "self",
            "Note owner",
            &note,
            context_store.clone(),
            T0,
            &mut owner_ent,
        )
        .expect("demo self note");
    }
    owner_init_journal(
        &master,
        "lea",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store.clone(),
        &window,
        T0,
        &mut owner_ent,
    )
    .expect("demo journal");

    // The wires: one fake Vault holding one DISTINCT bearer per server,
    // one recording MCP per server — all real sockets.
    let fake_vault = FakeVault {
        expected_token: vault_token.clone(),
        ..FakeVault::default()
    };
    let mut wires = BTreeMap::new();
    for server in &servers {
        let bearer = format!("wire-bearer-{server}-{seq}");
        fake_vault
            .secrets
            .lock()
            .unwrap()
            .entry(format!("aithos/mcp/{server}"))
            .or_default()
            .insert("token".to_owned(), bearer.clone());
        w.demo_bearers.insert(server.clone(), bearer);
        let answer = match server.as_str() {
            "notion" => format!("prospects: {}", w.demo_prospects.join(", ")),
            "gmail" => "email sent".to_owned(),
            "calendar" => "event booked".to_owned(),
            other => format!("{other}-ok"),
        };
        wires.insert(server.clone(), WireMcp::new(&answer));
    }
    let vault_port = spawn_fake_vault(fake_vault.clone()).await;
    let mut server_blocks = String::new();
    let mut tool_lines = String::new();
    for server in &servers {
        let wire_port = spawn_wire_mcp(wires[server].clone()).await;
        server_blocks.push_str(&format!(
            "  - name: {server}\n    transport: http\n    url: http://127.0.0.1:{wire_port}/mcp\n    credential:\n      broker: enterprise\n      path: aithos/mcp/{server}\n      field: token\n",
        ));
        for spec in w.demo_specs.iter().filter(|s| &s.server == server) {
            let exposed = aithos_gateway::policy::hub_exposed_name(server, &spec.tool);
            let class = match spec.class {
                ToolAccess::Read => "read",
                ToolAccess::Write => "write",
            };
            tool_lines.push_str(&format!(
                "      {exposed}: {{ server: {server}, tool: {}, access: {class}, granted: {} }}\n",
                spec.tool, spec.granted
            ));
        }
    }
    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let config_text = format!(
        "listen: 127.0.0.1:4890\ncredential_brokers:\n  enterprise:\n    kind: vault-kv2\n    address: http://127.0.0.1:{vault_port}\n    mount: secret\n    auth:\n      kind: token-env\n      env: {env_name}\nservers:\n{server_blocks}contexts:\n  - name: ventes\n    store: {{ kind: fs, root: {} }}\n    tools:\n{tool_lines}journal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(&context_root),
        quote(&journal_root)
    );
    let config_path = dir.path().join("gateway.yaml");
    std::fs::write(&config_path, &config_text).expect("demo config written");
    let cfg = GatewayConfig::from_yaml(&config_text).expect("demo config parses");
    let brokers = build_brokers(&cfg).expect("demo brokers");
    let upstreams: BTreeMap<String, HttpUpstream> = cfg
        .servers
        .as_ref()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry.name.clone(),
                HttpUpstream::for_server(entry, &brokers).expect("demo upstream"),
            )
        })
        .collect();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let runner =
        Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default())).expect("demo runner");

    w.ctx_stores.insert("ventes".to_owned(), context_store);
    w.journal_store = Some(journal_store);
    w.briefing_owner_ent = Some(owner_ent);
    w.vault = Some(VaultHarness {
        router: Arc::new(McpRouter {
            runner: Arc::new(Mutex::new(runner)),
            upstreams,
            clock: Arc::new(|| T0.to_owned()),
            session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
            oauth: None,
        }),
        vault: fake_vault,
        wires,
        config_path,
        config_text,
        vault_token,
        store_roots: vec![context_root, journal_root],
        responses: Vec::new(),
    });
    w.scratch = Some(dir);
}

// ------------------------------------------------------ demo beat steps

async fn demo_send(w: &mut GatewayWorld, recipients: Vec<String>) {
    provision_pending_world(w).await;
    w.call(
        "gmail__send_email",
        json!({ "arguments": {
            "to": recipients,
            "subject": "Prise de rendez-vous — visite du bien",
            "body": "Bonjour, proposons un créneau."
        } }),
    )
    .await;
}

async fn demo_book(w: &mut GatewayWorld, start: &str) {
    provision_pending_world(w).await;
    w.call(
        "calendar__create_event",
        json!({ "arguments": { "start": start, "title": "Visite du bien" } }),
    )
    .await;
}

#[when("the agent sends a meeting email to all five prospects")]
async fn demo_send_all_five(w: &mut GatewayWorld) {
    let all = w.demo_prospects.clone();
    demo_send(w, all).await;
}

#[when(expr = "the agent sends a meeting email to prospects {string}, {string} and {string}")]
async fn demo_send_three(w: &mut GatewayWorld, a: String, b: String, c: String) {
    demo_send(w, vec![a, b, c]).await;
}

#[when(expr = "the agent books a visit starting {string}")]
async fn demo_book_visit(w: &mut GatewayWorld, start: String) {
    demo_book(w, &start).await;
}

#[then(expr = "the initialize result recommends {string} before outbound actions")]
async fn demo_init_recommends(w: &mut GatewayWorld, tool: String) {
    let init = w.last_init.as_ref().expect("an initialize answer");
    let instructions = init
        .pointer("/result/instructions")
        .and_then(Value::as_str)
        .expect("initialize carries instructions");
    assert!(
        instructions.contains(&tool) && instructions.contains("before"),
        "the instructions recommend the briefing first: {instructions}"
    );
}

#[then(expr = "the list is exactly the granted tools, {string} and the journal tools")]
async fn demo_list_exact(w: &mut GatewayWorld, briefing: String) {
    let mut listed: Vec<String> = w
        .listed_tools()
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect();
    listed.sort();
    let mut expected: Vec<String> = w
        .demo_specs
        .iter()
        .filter(|spec| spec.granted)
        .map(|spec| aithos_gateway::policy::hub_exposed_name(&spec.server, &spec.tool))
        .collect();
    expected.push(briefing);
    expected.push(JOURNAL_WRITE.to_owned());
    expected.push(JOURNAL_SEARCH.to_owned());
    expected.sort();
    assert_eq!(listed, expected, "exposure = the mandate's coverage");
}

#[then(expr = "the list includes {string} and {string}")]
async fn demo_list_includes_two(w: &mut GatewayWorld, first: String, second: String) {
    let listed = w.listed_tools();
    for name in [first, second] {
        assert!(
            listed.iter().any(|tool| tool["name"] == name.as_str()),
            "`{name}` is listed"
        );
    }
}

#[then("the answer carries the five prospects")]
async fn demo_answer_five_prospects(w: &mut GatewayWorld) {
    let text = w.last_result_text();
    for prospect in &w.demo_prospects {
        assert!(text.contains(prospect), "prospect `{prospect}` is served");
    }
}

#[then(expr = "the {string} upstream saw only its own vault bearer")]
async fn demo_upstream_own_bearer(w: &mut GatewayWorld, server: String) {
    let expected = format!("Bearer {}", w.demo_bearers[&server]);
    let auths = w.vault_harness().wires[&server]
        .auths
        .lock()
        .unwrap()
        .clone();
    assert!(!auths.is_empty(), "the upstream was reached");
    assert!(
        auths.iter().all(|auth| auth.as_deref() == Some(&expected)),
        "`{server}` saw exactly its own vault bearer: {auths:?}"
    );
}

#[then(expr = "the act is logged in the {string} gamma with one journal cross-reference")]
async fn demo_act_logged_with_xref(w: &mut GatewayWorld, context: String) {
    let tool = w.last_tool.clone();
    let acts: Vec<EntryView> = w
        .ctx_gamma(&context)
        .iter()
        .filter(|entry| {
            entry.kind == "action"
                && payload_str(entry, "tool") == Some(tool.as_str())
                && entry
                    .target
                    .as_deref()
                    .is_some_and(|target| target != "x.gateway")
        })
        .cloned()
        .collect();
    assert_eq!(acts.len(), 1, "exactly one `{tool}` act in `{context}`");
    let xrefs: Vec<EntryView> = w
        .journal_gamma()
        .into_iter()
        .filter(|entry| {
            entry.kind == "action"
                && entry.target.as_deref() == Some("x.xref")
                && payload_str(entry, "entry_id") == Some(acts[0].id.as_str())
        })
        .collect();
    assert_eq!(xrefs.len(), 1, "one journal xref mirrors the act");
}

#[then(
    expr = "the refusal names field {string}, prospects {string} and {string} and the approved set"
)]
async fn demo_refusal_names_intruders(w: &mut GatewayWorld, field: String, d: String, e: String) {
    let response = w.last_response.as_ref().expect("a response");
    let message = response["error"]["message"].as_str().unwrap_or_default();
    for needle in [
        format!(".{field}"),
        d.clone(),
        e.clone(),
        "approved set".to_owned(),
    ] {
        assert!(
            message.contains(&needle),
            "the refusal teaches `{needle}`: {message}"
        );
    }
}

#[then("the gmail vault path received zero requests")]
async fn demo_gmail_vault_untouched(w: &mut GatewayWorld) {
    let hits = w.vault_harness().vault.hits.lock().unwrap().clone();
    assert!(
        hits.iter().all(|path| !path.contains("gmail")),
        "the gmail secret never woke: {hits:?}"
    );
}

#[then(expr = "the {string} upstream received zero requests")]
async fn demo_upstream_untouched(w: &mut GatewayWorld, server: String) {
    assert!(
        w.vault_harness().wires[&server]
            .requests
            .lock()
            .unwrap()
            .is_empty(),
        "no request may reach `{server}`"
    );
}

#[then(expr = "the {string} gamma and the journal each gain one {string} refusal")]
async fn demo_one_refusal_each(w: &mut GatewayWorld, context: String, reason: String) {
    let count = |entries: Vec<EntryView>| {
        entries
            .iter()
            .filter(|entry| {
                entry.kind == "action"
                    && entry.target.as_deref() == Some("x.gateway")
                    && payload_str(entry, "reason") == Some(reason.as_str())
            })
            .count()
    };
    assert_eq!(count(w.ctx_gamma(&context)), 1, "one in the context");
    assert_eq!(count(w.journal_gamma()), 1, "one in the journal");
}

#[then("the call succeeds")]
async fn demo_call_succeeds(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response.get("error").is_none() && response.get("result").is_some(),
        "the call went through: {response}"
    );
}

#[then(
    expr = "the {string} upstream saw exactly one call under raw name {string} bearing its vault token"
)]
async fn demo_upstream_one_raw_call(w: &mut GatewayWorld, server: String, raw: String) {
    let wire = w.vault_harness().wires[&server].clone();
    let requests = wire.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "exactly one call reached `{server}`");
    assert_eq!(
        requests[0].pointer("/params/name").and_then(Value::as_str),
        Some(raw.as_str()),
        "the raw upstream name is restored"
    );
    let expected = format!("Bearer {}", w.demo_bearers[&server]);
    let auths = wire.auths.lock().unwrap().clone();
    assert_eq!(
        auths,
        vec![Some(expected)],
        "the wire carried the vault bearer"
    );
}

#[then("the call is refused as a bound violation naming the approved slots")]
async fn demo_refused_naming_slots(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response");
    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("bound violated"),
        "a bound refusal: {message}"
    );
    for needle in ["tuesday", "thursday", "14:00", "18:00"] {
        assert!(
            message.contains(needle),
            "the slots are named (`{needle}`): {message}"
        );
    }
}

#[then("the answer carries the DPE directive verbatim")]
async fn demo_answer_dpe(w: &mut GatewayWorld) {
    let directive = w.demo_directive.clone().expect("a circle directive");
    let text = w.last_result_text();
    assert!(
        text.contains(&directive),
        "the exact owner text is served: {text}"
    );
}

#[then("no agent-facing response contains the owner-only note")]
async fn demo_no_note_leak(w: &mut GatewayWorld) {
    let note = w.demo_note.clone().expect("an owner-only note");
    let mut texts: Vec<String> = w.agent_responses.iter().map(ToString::to_string).collect();
    if let Some(vault) = &w.vault {
        texts.extend(vault.responses.iter().map(ToString::to_string));
    }
    assert!(!texts.is_empty(), "responses captured");
    assert!(
        !texts.join("\n").contains(&note),
        "the self zone never reaches the agent"
    );
}

#[then(expr = "the {string} gamma gains one briefing read entry")]
async fn demo_one_briefing_read(w: &mut GatewayWorld, context: String) {
    let reads = w
        .ctx_gamma(&context)
        .iter()
        .filter(|entry| entry.kind == "ethos.read")
        .count();
    assert_eq!(reads, 1, "the briefing read is on the record");
}

#[when(expr = "the owner appends {string} to the circle directive")]
async fn demo_owner_appends(w: &mut GatewayWorld, appended: String) {
    let master = w.master();
    let old = w.demo_directive.clone().expect("a circle directive");
    let text = format!("{old} {appended}");
    let store = w.ctx_stores.get("ventes").expect("ventes store").clone();
    let ent = w.briefing_owner_ent.as_mut().expect("owner entropy");
    owner_set_briefing(
        &master,
        "ventes",
        "circle",
        "Consigne commerciale",
        &text,
        store,
        T0,
        ent,
    )
    .expect("owner append lands");
    w.briefing_rewritten = Some(appended);
}

#[then("the answer carries the appended directive verbatim")]
async fn demo_answer_appended(w: &mut GatewayWorld) {
    let appended = w.briefing_rewritten.clone().expect("an appended sentence");
    let text = w.last_result_text();
    assert!(text.contains(&appended), "the hot edit is served: {text}");
}

#[then(expr = "both reads are journalized in the {string} gamma")]
async fn demo_both_reads_journalized(w: &mut GatewayWorld, context: String) {
    let reads = w
        .ctx_gamma(&context)
        .iter()
        .filter(|entry| entry.kind == "ethos.read")
        .count();
    assert_eq!(reads, 2, "one journalized read per briefing call");
}

#[given("the agent has walked beats 2 through 7")]
async fn demo_walk_beats(w: &mut GatewayWorld) {
    provision_pending_world(w).await;
    // Beat 2 — the data comes from notion.
    w.call("notion__query_database", json!({ "arguments": {} }))
        .await;
    // Beat 3 — the wall that teaches.
    let all = w.demo_prospects.clone();
    demo_send(w, all).await;
    // Beat 4 — the corrected send.
    let three = w.demo_prospects[..3].to_vec();
    demo_send(w, three).await;
    // Beat 5 — outside then inside the slots.
    demo_book(w, "2026-07-15T10:00:00+02:00").await;
    demo_book(w, "2026-07-16T15:00:00+02:00").await;
    // Beat 6 — the briefing.
    briefing_call(w, json!({})).await;
    // Beat 7 — the hot edit, then the next read.
    demo_owner_appends(w, "Joindre le lien du dossier de visite.".to_owned()).await;
    briefing_call(w, json!({})).await;
}

#[when(expr = "the auditor exports the {string} context with the auditor mandate")]
async fn demo_auditor_exports(w: &mut GatewayWorld, context: String) {
    let seed_hex = w.demo_auditor_seed.clone().expect("an auditor seed");
    let seed: [u8; 32] = hex_to_seed(&seed_hex);
    let store = w.ctx_stores.get(&context).expect("context store").clone();
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Arc::new(Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32()));
    let bridge = Bridge::open(store, keyholder, Box::new(SeqEntropy::default()))
        .expect("auditor-side bridge opens");
    // One scoped query per granted kind — anything wider stays refused
    // by the certificate half; the merged document is the demo's replay.
    let acts: Value = serde_json::from_str(
        &bridge
            .export_audit(&seed, Some("action"), T0)
            .expect("the act slice exports"),
    )
    .expect("valid act export");
    let reads: Value = serde_json::from_str(
        &bridge
            .export_audit(&seed, Some("ethos.read"), T0)
            .expect("the read slice exports"),
    )
    .expect("valid read export");
    w.audit_export = Some(json!({ "acts": acts, "reads": reads }).to_string());
}

fn hex_to_seed(hex_str: &str) -> [u8; 32] {
    let bytes = (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).expect("hex"))
        .collect::<Vec<_>>();
    bytes.try_into().expect("32 bytes")
}

#[then("the export carries the notion act, the gmail act and the calendar act")]
async fn demo_export_carries_acts(w: &mut GatewayWorld) {
    let export: Value =
        serde_json::from_str(w.audit_export.as_deref().expect("an export")).expect("valid JSON");
    let entries = export["acts"]["entries"].as_array().expect("act entries");
    for target in ["x.notion", "x.gmail", "x.calendar"] {
        assert!(
            entries.iter().any(|entry| entry["target"] == target),
            "the `{target}` act is exported"
        );
    }
}

#[then(expr = "the export carries the {string} refusals naming {string} and {string}")]
async fn demo_export_carries_refusals(
    w: &mut GatewayWorld,
    reason: String,
    first: String,
    second: String,
) {
    let export: Value =
        serde_json::from_str(w.audit_export.as_deref().expect("an export")).expect("valid JSON");
    let entries = export["acts"]["entries"].as_array().expect("act entries");
    let refusal_details: Vec<&str> = entries
        .iter()
        .filter(|entry| entry["payload"]["reason"] == reason.as_str())
        .filter_map(|entry| entry["payload"]["detail"].as_str())
        .collect();
    assert_eq!(refusal_details.len(), 2, "the two refusals are exported");
    for field in [first, second] {
        assert!(
            refusal_details
                .iter()
                .any(|detail| detail.contains(&format!(".{field}"))),
            "one refusal detail names `{field}`: {refusal_details:?}"
        );
    }
}

#[then("the export carries the briefing read entries")]
async fn demo_export_carries_reads(w: &mut GatewayWorld) {
    let export: Value =
        serde_json::from_str(w.audit_export.as_deref().expect("an export")).expect("valid JSON");
    let reads = export["reads"]["entries"].as_array().expect("read entries");
    assert_eq!(
        reads.len(),
        2,
        "both journalized briefing reads are in the auditor's slice"
    );
    assert!(reads.iter().all(|entry| entry["kind"] == "ethos.read"));
}

#[then("no file of any Ethos store contains any vault token or upstream secret")]
async fn demo_stores_leak_free(w: &mut GatewayWorld) {
    let mut needles: Vec<String> = w.demo_bearers.values().cloned().collect();
    needles.push(w.vault_harness().vault_token.clone());
    let roots = w.vault_harness().store_roots.clone();
    for root in roots {
        for needle in &needles {
            files_exclude(&root, needle);
        }
    }
}

#[then("the gateway config text contains references only")]
async fn demo_config_references_only(w: &mut GatewayWorld) {
    let harness = w.vault_harness();
    assert!(
        harness.config_text.contains("aithos/mcp/gmail"),
        "the reference is declared"
    );
    for bearer in w.demo_bearers.values() {
        assert!(
            !harness.config_text.contains(bearer),
            "no wire bearer in the config"
        );
    }
    assert!(
        !harness.config_text.contains(&harness.vault_token),
        "no vault access token in the config"
    );
}

// ---------------------------------------------- restricted mandates (M2)

#[given(
    expr = "server {string} is enrolled with {string} as a granted read and {string} as a granted write with a one_of bound on {string}"
)]
async fn mandates_enrolled_read_write_bound(
    w: &mut GatewayWorld,
    server: String,
    read_tool: String,
    write_tool: String,
    field: String,
) {
    assert_eq!(server, GRANTS_SERVER);
    let approvals = BTreeMap::from([
        (read_tool, ToolApproval::granted(ToolAccess::Read)),
        (
            write_tool,
            ToolApproval::granted(ToolAccess::Write).with_bounds(vec![ArgumentBound::OneOf {
                field,
                values: BOUNDS_APPROVED
                    .iter()
                    .map(|value| value.to_string())
                    .collect(),
            }]),
        ),
    ]);
    provision_grants_world(w, approvals, false).await;
}

#[when("the owner previews the agent mandate")]
async fn mandates_owner_previews(w: &mut GatewayWorld) {
    let master = w.master();
    let store = w.grants_store.clone().expect("an enrolled grants context");
    let preview = owner_preview_mandate(
        &master,
        GRANTS_CONTEXT,
        &[GRANTS_SERVER.to_owned()],
        store,
        T0,
    )
    .expect("the preview computes from the files alone");
    w.preview = Some(preview);
}

#[then("the preview JSON names exactly the granted tools, each with its inherited bounds")]
async fn mandates_preview_lists_granted(w: &mut GatewayWorld) {
    let preview = w.preview.as_ref().expect("a preview");
    assert_eq!(preview["version"], EFFECTIVE_POLICY_VERSION);
    let tools = preview["tools"].as_array().expect("a tools array");
    let granted: Vec<&str> = tools
        .iter()
        .filter(|tool| tool["granted"] == json!(true))
        .map(|tool| tool["tool"].as_str().expect("a tool name"))
        .collect();
    assert_eq!(
        granted,
        vec!["gmail__search_emails", "gmail__send_email"],
        "exactly the granted tools: {preview}"
    );
    for tool in tools {
        assert_eq!(tool["covered"], json!(true), "covered by the mandate");
        assert_eq!(tool["served"], json!(true), "served by tools/list");
        let bounds = tool["bounds"].as_array().expect("a bounds array");
        if tool["tool"] == json!("gmail__send_email") {
            assert_eq!(bounds.len(), 1, "the inherited bound rides along");
            assert_eq!(bounds[0]["kind"], "one_of");
            assert_eq!(bounds[0]["field"], "to");
            let values: Vec<&str> = bounds[0]["values"]
                .as_array()
                .expect("bound values")
                .iter()
                .map(|value| value.as_str().expect("a string value"))
                .collect();
            assert_eq!(values, BOUNDS_APPROVED.to_vec());
        } else {
            assert!(bounds.is_empty(), "no bound was approved on {tool}");
        }
    }
}

#[then(expr = "the preview names the validity window and the status {string}")]
async fn mandates_preview_window_status(w: &mut GatewayWorld, status: String) {
    let preview = w.preview.as_ref().expect("a preview");
    let mandate = &preview["mandate"];
    assert_eq!(mandate["status"], json!(status), "status: {preview}");
    assert_eq!(mandate["not_before"], json!(NOT_BEFORE));
    assert_eq!(mandate["not_after"], json!(NOT_AFTER));
}

#[when(expr = "the owner previews a call of {string} to {string}")]
async fn mandates_preview_call(w: &mut GatewayWorld, tool: String, recipient: String) {
    let master = w.master();
    let (server, _) = tool.split_once("__").expect("an exposed hub tool name");
    let context_root = w
        .vault
        .as_ref()
        .expect("a provisioned bounds harness")
        .store_roots[0]
        .clone();
    let store =
        GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs { root: context_root })
            .expect("the bounds context store reopens owner-side");
    let args = json!({ "to": [recipient] });
    let verdict = owner_preview_call(
        &master,
        BOUNDS_CONTEXT,
        &[server.to_owned()],
        store,
        &tool,
        &args,
        T0,
    )
    .expect("the dry-run verdict computes");
    w.preview = Some(verdict);
    w.preview_tool = Some(tool);
    w.last_args = Some(args);
}

#[then(expr = "the preview verdict is a refusal naming field {string} and the approved set")]
async fn mandates_preview_refusal(w: &mut GatewayWorld, field: String) {
    let verdict = w.preview.as_ref().expect("a dry-run verdict");
    assert_eq!(verdict["verdict"], "refused", "refused: {verdict}");
    assert_eq!(verdict["code"], "bound_violated");
    let detail = verdict["detail"].as_str().expect("a pedagogical detail");
    assert!(
        detail.contains(&format!(".{field}`")),
        "the field is named: {detail}"
    );
    assert!(
        detail.contains("mallory@evil.example"),
        "the offending value is named: {detail}"
    );
    assert!(
        detail.contains("prospect-a@clients.example"),
        "the approved set is named: {detail}"
    );
}

#[then("the running gateway refuses the same call with the same verdict")]
async fn mandates_runtime_matches_preview(w: &mut GatewayWorld) {
    let tool = w.preview_tool.clone().expect("a previewed tool");
    let args = w.last_args.clone().expect("the previewed arguments");
    let detail = w.preview.as_ref().expect("a dry-run verdict")["detail"]
        .as_str()
        .expect("a detail")
        .to_owned();
    bounds_call(w, &tool, args).await;
    let response = w.last_response.as_ref().expect("a runtime response");
    let message = response["error"]["message"]
        .as_str()
        .expect("a runtime refusal message");
    assert_eq!(
        message,
        format!("aithos gateway: {detail}"),
        "one function, two callers: {response}"
    );
}

// ------------------------------------------------------------------ main

// --------------------------------------------- ethos reading (lot G6)

/// Snapshot every gamma length (contexts + journal): the no-new-entry
/// and exactly-one-entry assertions diff against it — append-only makes
/// the slice beyond the baseline THE new entries.
fn snapshot_gammas(w: &mut GatewayWorld) {
    let mut baseline = BTreeMap::new();
    for (name, store) in &w.ctx_stores {
        baseline.insert(
            name.clone(),
            gamma_view(store.clone()).expect("gamma").len(),
        );
    }
    if let Some(journal) = &w.journal_store {
        baseline.insert(
            "journal".to_owned(),
            gamma_view(journal.clone()).expect("gamma").len(),
        );
    }
    w.gamma_baseline = baseline;
}

/// One native ethos call through the black-box door, gammas snapshotted
/// right before it.
async fn ethos_call(w: &mut GatewayWorld, tool: &str, args: Value) {
    provision_pending_world(w).await;
    snapshot_gammas(w);
    w.last_tool = tool.to_owned();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 63,
        "method": "tools/call",
        "params": { "name": tool, "arguments": args }
    });
    let response = w.agent_request(body).await;
    w.last_response = Some(response);
}

/// Mint (or fail to mint) the v1 ethos-read pen on one context, the
/// certificate count captured first for the nothing-written assertion.
async fn grant_ethos_read(w: &mut GatewayWorld, zones: &str, context: &str) {
    provision_pending_world(w).await;
    let master = w.master();
    let (agent_pub, _) = w.pubs();
    let store = w.ctx_stores.get(context).expect("a context store").clone();
    let window = GatewayWorld::window();
    let zone_list: Vec<String> = zones
        .split(',')
        .map(str::trim)
        .filter(|z| !z.is_empty())
        .map(str::to_owned)
        .collect();
    w.ethos_cert_baseline = Some(store.list("certs/").map(|v| v.len()).unwrap_or(0));
    let ent = w.briefing_owner_ent.as_mut().expect("owner entropy");
    match owner_grant_ethos_read(
        &master, context, &agent_pub, &zone_list, store, &window, T0, ent,
    ) {
        Ok(id) => {
            w.ethos_mandates.insert(context.to_owned(), id);
            w.ethos_gesture_error = None;
        }
        Err(e) => w.ethos_gesture_error = Some(e.to_string()),
    }
}

/// The three ethos tool descriptions of the last list (empty when the
/// surface is mute — the callers assert accordingly).
fn ethos_descriptions(w: &GatewayWorld) -> Vec<String> {
    w.listed_tools()
        .iter()
        .filter(|t| {
            t["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("ethos."))
        })
        .map(|t| t["description"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// The parsed JSON payload of the last native answer.
fn last_payload(w: &GatewayWorld) -> Value {
    serde_json::from_str(&w.last_result_text()).expect("a JSON answer")
}

/// Every listed row across contexts of an ethos.list / pack index answer.
fn tree_rows(payload: &Value) -> Vec<Value> {
    payload["contexts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|ctx| {
            ctx["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect()
}

#[given(expr = "the {string} context {word} zone holds the section {string} with text {string}")]
async fn ethos_zone_section(
    w: &mut GatewayWorld,
    context: String,
    zone: String,
    path: String,
    text: String,
) {
    w.ethos_pending.push((context, zone, path, text));
}

#[given("no mandate covers any sealed zone")]
async fn ethos_no_sealed_coverage(_w: &mut GatewayWorld) {
    // Nothing granted — exactly the point of the scenario.
}

#[given("every zone of every context is empty")]
async fn ethos_everything_empty(w: &mut GatewayWorld) {
    assert!(
        w.ethos_pending.is_empty() && w.briefing_pending.is_empty(),
        "the mute-surface scenario starts with empty zones"
    );
}

#[given("no mandate covers the circle zone")]
async fn ethos_no_circle_coverage(_w: &mut GatewayWorld) {
    // No ethos-read pen is minted — the briefing pen alone covers
    // nothing outside its own shelves.
}

#[given(expr = "an equipped {string} context")]
async fn ethos_equipped_context(_w: &mut GatewayWorld, _context: String) {
    // The lazy briefing world always equips "ventes"; the gesture under
    // test runs against it.
}

#[given(expr = "the owner granted ethos read on zones {string} for the {string} context")]
async fn ethos_granted_given(w: &mut GatewayWorld, zones: String, context: String) {
    grant_ethos_read(w, &zones, &context).await;
    assert!(
        w.ethos_gesture_error.is_none(),
        "the grant lands: {:?}",
        w.ethos_gesture_error
    );
}

#[when(expr = "the owner grants ethos read on zones {string} for the {string} context")]
async fn ethos_granted_when(w: &mut GatewayWorld, zones: String, context: String) {
    grant_ethos_read(w, &zones, &context).await;
}

#[given("the agent lists the tools once")]
async fn ethos_lists_once(w: &mut GatewayWorld) {
    provision_pending_world(w).await;
    ethos_list_tools(w).await;
}

#[when("the agent lists the tools again")]
async fn ethos_lists_again(w: &mut GatewayWorld) {
    ethos_list_tools(w).await;
}

async fn ethos_list_tools(w: &mut GatewayWorld) {
    let list = w
        .agent_request(json!({ "jsonrpc": "2.0", "id": 64, "method": "tools/list" }))
        .await;
    w.last_list = Some(list);
}

#[when(expr = "the owner revokes the ethos-read mandate of the {string} context")]
async fn ethos_revokes(w: &mut GatewayWorld, context: String) {
    let master = w.master();
    let id = w
        .ethos_mandates
        .get(&context)
        .expect("a granted ethos-read pen")
        .clone();
    let store = w.ctx_stores.get(&context).expect("a context store").clone();
    let ent = w.briefing_owner_ent.as_mut().expect("owner entropy");
    owner_revoke_mandate_id(&master, &context, &id, "gate check", store, T0, ent)
        .expect("the revocation lands");
}

#[given("a delegate holding an issue mandate mints a read.circle sub-mandate to the agent key")]
async fn ethos_delegate_subchain(w: &mut GatewayWorld) {
    provision_pending_world(w).await;
    let master = w.master();
    let (agent_pub, _) = w.pubs();
    let store = w.ctx_stores.get("ventes").expect("ventes store").clone();
    let window = GatewayWorld::window();
    let ent = w.briefing_owner_ent.as_mut().expect("owner entropy");
    let (parent, leaf) =
        owner_issue_ethos_read_subchain(&master, "ventes", &agent_pub, store, &window, T0, ent)
            .expect("the sub-chain is minted");
    w.ethos_subchain = Some((parent, leaf));
}

#[when(expr = "the agent calls {string} on zone {string} path {string} of context {string}")]
async fn ethos_read_call(
    w: &mut GatewayWorld,
    tool: String,
    zone: String,
    path: String,
    context: String,
) {
    assert_eq!(tool, ETHOS_READ, "the zoned call step serves ethos.read");
    ethos_call(
        w,
        ETHOS_READ,
        json!({ "context": context, "zone": zone, "path": path }),
    )
    .await;
}

#[then(expr = "the list includes {string} and {string} and {string}")]
async fn list_includes_three(w: &mut GatewayWorld, a: String, b: String, c: String) {
    let listed = w.listed_tools();
    for tool in [a, b, c] {
        assert!(
            listed.iter().any(|t| t["name"] == tool.as_str()),
            "the list includes `{tool}`"
        );
    }
}

#[then(expr = "the ethos tool descriptions name {word} access on {string}")]
async fn ethos_descriptions_name_zone(w: &mut GatewayWorld, zone: String, context: String) {
    let descriptions = ethos_descriptions(w);
    assert_eq!(descriptions.len(), 3, "the three ethos tools are listed");
    for description in &descriptions {
        assert!(
            description.contains(&context) && description.contains(&zone),
            "the description names `{context}` and `{zone}`: {description}"
        );
    }
}

#[then("the ethos tool descriptions name no other zone")]
async fn ethos_descriptions_no_other_zone(w: &mut GatewayWorld) {
    for description in ethos_descriptions(w) {
        assert!(
            !description.contains("circle") && !description.contains("self"),
            "no unserved zone is named: {description}"
        );
    }
}

#[then("the ethos tool descriptions no longer name circle")]
async fn ethos_descriptions_dropped_circle(w: &mut GatewayWorld) {
    for description in ethos_descriptions(w) {
        assert!(
            !description.contains("circle"),
            "circle dropped from the surface: {description}"
        );
    }
}

#[then("no restart happened")]
async fn ethos_no_restart(w: &mut GatewayWorld) {
    assert!(
        w.router.is_some(),
        "the same router instance served every call of the scenario"
    );
}

#[then("a subsequent circle read is refused naming the revoked chain")]
async fn ethos_read_after_revocation(w: &mut GatewayWorld) {
    ethos_call(
        w,
        ETHOS_READ,
        json!({ "context": "ventes", "zone": "circle", "path": "memoire/prospects" }),
    )
    .await;
    let response = w.last_response.as_ref().expect("a response");
    let message = response["error"]["message"]
        .as_str()
        .expect("a refusal message");
    assert!(
        message.contains("revoked"),
        "the refusal names the revocation: {message}"
    );
}

#[then("a circle read under that chain names the full chain in its entry")]
async fn ethos_subchain_read_names_chain(w: &mut GatewayWorld) {
    ethos_call(
        w,
        ETHOS_READ,
        json!({ "context": "ventes", "zone": "circle", "path": "memoire/prospects" }),
    )
    .await;
    let response = w.last_response.as_ref().expect("a response");
    assert!(
        response.get("error").is_none(),
        "the read under the sub-chain serves: {response}"
    );
    let (parent, leaf) = w.ethos_subchain.clone().expect("the minted sub-chain");
    let gamma = w.ctx_gamma("ventes");
    let entry = gamma
        .iter()
        .rev()
        .find(|e| e.kind == "ethos.read")
        .expect("a journalized read");
    assert_eq!(
        entry.authorized_via.clone().expect("authorized_via"),
        vec![parent, leaf],
        "the full chain, root first"
    );
}

#[then(expr = "the tree names the public section {string}")]
async fn ethos_tree_public(w: &mut GatewayWorld, path: String) {
    let rows = tree_rows(&last_payload(w));
    assert!(
        rows.iter()
            .any(|row| row["zone"] == "public" && row["path"] == path.as_str()),
        "the public row is listed: {rows:?}"
    );
}

#[then(expr = "the tree names the circle section {string} with its title and no body")]
async fn ethos_tree_circle(w: &mut GatewayWorld, path: String) {
    let rows = tree_rows(&last_payload(w));
    let row = rows
        .iter()
        .find(|row| row["zone"] == "circle" && row["path"] == path.as_str())
        .expect("the circle row is listed");
    assert!(
        row["title"].as_str().is_some_and(|t| !t.is_empty()),
        "the clear title rides along: {row}"
    );
    assert!(
        row.get("text").is_none() && row.get("body").is_none(),
        "no body leaves with the skeleton: {row}"
    );
}

#[then("no self row, sid or title appears in any agent-facing response")]
async fn ethos_no_self_anywhere(w: &mut GatewayWorld) {
    let swept: String = w
        .agent_responses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !swept.contains("\"zone\":\"self\""),
        "no self row is ever listed"
    );
    for text in w.briefing_texts("self") {
        assert!(!swept.contains(&text), "no self text leaks");
    }
}

#[then("no gamma entry was written by the listing")]
async fn ethos_listing_costless(w: &mut GatewayWorld) {
    assert_gammas_unmoved(w);
}

#[then("no gamma entry was written by the read")]
async fn ethos_public_read_costless(w: &mut GatewayWorld) {
    assert_gammas_unmoved(w);
}

fn assert_gammas_unmoved(w: &GatewayWorld) {
    for (name, baseline) in &w.gamma_baseline {
        let count = if name == "journal" {
            gamma_view(w.journal_store.clone().expect("journal store"))
                .expect("gamma")
                .len()
        } else {
            gamma_view(w.ctx_stores[name].clone()).expect("gamma").len()
        };
        assert_eq!(count, *baseline, "gamma of `{name}` unmoved");
    }
}

#[then(expr = "the answer carries {string} verbatim")]
async fn ethos_answer_verbatim(w: &mut GatewayWorld, text: String) {
    let answer = w.last_result_text();
    assert!(answer.contains(&text), "the exact text is served: {answer}");
}

#[then(expr = "the {string} context gamma gains exactly one ethos.read entry")]
async fn ethos_one_read_entry(w: &mut GatewayWorld, context: String) {
    let baseline = w.gamma_baseline[&context];
    let gamma = w.ctx_gamma(&context);
    let fresh: Vec<&EntryView> = gamma[baseline..].iter().collect();
    assert_eq!(fresh.len(), 1, "exactly one new entry: {fresh:?}");
    assert_eq!(fresh[0].kind, "ethos.read", "the read is the record");
}

#[then("that entry names the granting chain in authorized_via")]
async fn ethos_entry_names_chain(w: &mut GatewayWorld) {
    let pen = w.ethos_mandates.get("ventes").expect("the granted pen");
    let gamma = w.ctx_gamma("ventes");
    let entry = gamma
        .iter()
        .rev()
        .find(|e| e.kind == "ethos.read")
        .expect("a journalized read");
    assert!(
        entry
            .authorized_via
            .clone()
            .expect("authorized_via")
            .contains(pen),
        "the granting chain is named"
    );
}

#[then(expr = "the call is refused naming the missing {string} perimeter")]
async fn ethos_refused_naming_perimeter(w: &mut GatewayWorld, perimeter: String) {
    let response = w.last_response.as_ref().expect("a response");
    assert_eq!(response["error"]["code"], POLICY_DENIED_CODE);
    let message = response["error"]["message"]
        .as_str()
        .expect("a refusal message");
    assert!(
        message.contains(&perimeter),
        "the refusal names `{perimeter}`: {message}"
    );
}

#[then("no section text leaks in the refusal")]
async fn ethos_refusal_leaks_nothing(w: &mut GatewayWorld) {
    let response = w.last_response.as_ref().expect("a response").to_string();
    for (_, _, _, text) in &w.ethos_pending {
        assert!(!response.contains(text), "no body text rides a refusal");
    }
}

#[then(expr = "the {string} context gamma records the refusal too")]
async fn ethos_context_records_refusal(w: &mut GatewayWorld, context: String) {
    let baseline = w.gamma_baseline[&context];
    let gamma = w.ctx_gamma(&context);
    let recorded = gamma[baseline..].iter().any(|e| {
        e.kind == "action"
            && e.target.as_deref() == Some("x.gateway")
            && payload_str(e, "tool") == Some(w.last_tool.as_str())
    });
    assert!(recorded, "the context auditor sees the attempt");
}

#[then("the pack carries the directive verbatim")]
async fn ethos_pack_carries_directive(w: &mut GatewayWorld) {
    let answer = w.last_result_text();
    for directive in w.briefing_texts("circle") {
        assert!(
            answer.contains(&directive),
            "the directive rides the pack verbatim"
        );
    }
}

#[then(expr = "the pack names the circle section {string} without its body")]
async fn ethos_pack_names_section(w: &mut GatewayWorld, path: String) {
    let answer = w.last_result_text();
    assert!(answer.contains(&path), "the covered index names the path");
    for (_, zone, pending_path, text) in &w.ethos_pending {
        if zone == "circle" && pending_path == &path {
            assert!(!answer.contains(text), "the body stays sealed in the pack");
        }
    }
}

#[then("the only new gamma entries are the briefing directive reads")]
async fn ethos_pack_costs_reads_only(w: &mut GatewayWorld) {
    let mut fresh_total = 0;
    for (name, baseline) in &w.gamma_baseline {
        let gamma = if name == "journal" {
            gamma_view(w.journal_store.clone().expect("journal store")).expect("gamma")
        } else {
            gamma_view(w.ctx_stores[name].clone()).expect("gamma")
        };
        for entry in &gamma[*baseline..] {
            assert_eq!(
                entry.kind, "ethos.read",
                "only reads are on the new record: {entry:?}"
            );
            fresh_total += 1;
        }
    }
    assert!(fresh_total >= 1, "the circle directive read is journalized");
}

#[then("the tree carries no circle row")]
async fn ethos_tree_no_circle(w: &mut GatewayWorld) {
    let rows = tree_rows(&last_payload(w));
    assert!(
        rows.iter().all(|row| row["zone"] != "circle"),
        "no circle row is listed: {rows:?}"
    );
}

#[then(expr = "the pack carries {string} verbatim")]
async fn ethos_pack_carries_verbatim(w: &mut GatewayWorld, text: String) {
    let answer = w.last_result_text();
    assert!(
        answer.contains(&text),
        "the exact text rides the pack: {answer}"
    );
}

#[then("the pack carries no circle row")]
async fn ethos_pack_no_circle(w: &mut GatewayWorld) {
    let payload = last_payload(w);
    for ctx in payload["contexts"].as_array().into_iter().flatten() {
        let index = ctx["pack"]["index"].as_array().expect("an index array");
        assert!(index.is_empty(), "no covered circle row: {index:?}");
    }
}

#[then("the circle section title appears in no agent-facing response")]
async fn ethos_circle_title_nowhere(w: &mut GatewayWorld) {
    let swept: String = w
        .agent_responses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    for (_, zone, path, text) in &w.ethos_pending {
        if zone == "circle" {
            assert!(!swept.contains(path), "the circle path never leaves");
            assert!(!swept.contains(text), "the circle body never leaves");
        }
    }
}

#[then("the gesture is refused naming the pending delegated self resolution")]
async fn ethos_gesture_refused_self(w: &mut GatewayWorld) {
    let error = w
        .ethos_gesture_error
        .as_ref()
        .expect("the gesture is refused");
    assert!(
        error.contains("self-resolution") || error.contains("read.self"),
        "the refusal names the pending core lot: {error}"
    );
}

#[then("no certificate is written")]
async fn ethos_no_certificate_written(w: &mut GatewayWorld) {
    let baseline = w.ethos_cert_baseline.expect("a certificate baseline");
    let store = w.ctx_stores.get("ventes").expect("ventes store").clone();
    let count = store.list("certs/").map(|v| v.len()).unwrap_or(0);
    assert_eq!(count, baseline, "the cert shelf did not move");
}

// ------------------------------------- streamable transport (lot G2)

/// The G2 world: the standard two-context runner, its gamma counts
/// captured as a baseline, and the REAL router served over a loopback
/// socket — the axum shell (Origin, notifications, sessions, methods)
/// IS the thing under test, so these scenarios speak actual HTTP.
#[given("a provisioned multi-context gateway")]
async fn streamable_world(w: &mut GatewayWorld) {
    provision_runner(w, "company-brand".into(), "ui-designer".into(), false).await;
    let mut baseline = BTreeMap::new();
    for (name, store) in &w.ctx_stores {
        baseline.insert(
            name.clone(),
            gamma_view(store.clone()).expect("gamma").len(),
        );
    }
    baseline.insert(
        "journal".to_owned(),
        gamma_view(w.journal_store.clone().expect("journal store"))
            .expect("gamma")
            .len(),
    );
    w.gamma_baseline = baseline;
    let app = router_multi(Arc::clone(w.router.as_ref().expect("a provisioned router")));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("loopback listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    w.wire_base = Some(format!("http://{addr}/mcp"));
    // The host base (no /mcp) — the AS well-known paths hang off it, and
    // without `as:` they must 404 (this world serves oauth: None).
    w.oauth_base = Some(format!("http://{addr}"));
}

/// POST one raw body to the served endpoint, extra headers included,
/// and record exactly what the wire answered.
async fn wire_post(w: &mut GatewayWorld, body: String, extra: &[(&str, &str)]) {
    let url = w.wire_base.clone().expect("a served gateway");
    let mut req = reqwest::Client::new()
        .post(&url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(body);
    for (name, value) in extra {
        req = req.header(*name, *value);
    }
    let resp = req.send().await.expect("the wire answers");
    let status = resp.status().as_u16();
    let session = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = resp.bytes().await.expect("a body").to_vec();
    w.wire_responses.push(WireResponse {
        status,
        session,
        body,
    });
}

#[when(expr = "the agent posts the notification {string}")]
async fn wire_posts_notification(w: &mut GatewayWorld, method: String) {
    let body = json!({ "jsonrpc": "2.0", "method": method }).to_string();
    wire_post(w, body, &[]).await;
}

#[when(expr = "the agent posts a {string} for {string} without an id")]
async fn wire_posts_idless_call(w: &mut GatewayWorld, method: String, tool: String) {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": { "name": tool, "arguments": {} }
    })
    .to_string();
    wire_post(w, body, &[]).await;
}

#[when("the agent initializes over HTTP")]
async fn wire_initializes(w: &mut GatewayWorld) {
    let body = json!({ "jsonrpc": "2.0", "id": 70, "method": "initialize" }).to_string();
    wire_post(w, body, &[]).await;
    w.wire_session = w.wire_responses.last().and_then(|r| r.session.clone());
}

#[when(
    expr = "the agent initializes over HTTP and calls {string} presenting the returned session id"
)]
async fn wire_init_then_call(w: &mut GatewayWorld, method: String) {
    wire_initializes(w).await;
    let sid = w.wire_session.clone().expect("a minted session id");
    let body = json!({ "jsonrpc": "2.0", "id": 71, "method": method }).to_string();
    wire_post(w, body, &[("mcp-session-id", sid.as_str())]).await;
}

#[when(expr = "the agent calls {string} over HTTP presenting the session id {string}")]
async fn wire_call_with_session(w: &mut GatewayWorld, method: String, sid: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 72, "method": method }).to_string();
    wire_post(w, body, &[("mcp-session-id", sid.as_str())]).await;
}

#[when(expr = "the agent calls {string} over HTTP presenting no session id")]
async fn wire_call_without_session(w: &mut GatewayWorld, method: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 73, "method": method }).to_string();
    wire_post(w, body, &[]).await;
}

#[when("the agent issues a GET to the MCP endpoint")]
async fn wire_get(w: &mut GatewayWorld) {
    let url = w.wire_base.clone().expect("a served gateway");
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("the wire answers");
    let status = resp.status().as_u16();
    let body = resp.bytes().await.expect("a body").to_vec();
    w.wire_responses.push(WireResponse {
        status,
        session: None,
        body,
    });
}

#[when("the agent issues a DELETE to the MCP endpoint")]
async fn wire_delete(w: &mut GatewayWorld) {
    let url = w.wire_base.clone().expect("a served gateway");
    let resp = reqwest::Client::new()
        .delete(&url)
        .send()
        .await
        .expect("the wire answers");
    let status = resp.status().as_u16();
    let body = resp.bytes().await.expect("a body").to_vec();
    w.wire_responses.push(WireResponse {
        status,
        session: None,
        body,
    });
}

#[when("the agent posts a JSON array batching two requests")]
async fn wire_posts_batch(w: &mut GatewayWorld) {
    let body = json!([
        { "jsonrpc": "2.0", "id": 74, "method": "tools/list" },
        { "jsonrpc": "2.0", "id": 75, "method": "ping" }
    ])
    .to_string();
    wire_post(w, body, &[]).await;
}

#[when(expr = "the agent posts {string} with the Origin header {string}")]
async fn wire_posts_with_origin(w: &mut GatewayWorld, method: String, origin: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 76, "method": method }).to_string();
    wire_post(w, body, &[("origin", origin.as_str())]).await;
}

#[when(expr = "the agent posts {string} without an Origin header")]
async fn wire_posts_without_origin(w: &mut GatewayWorld, method: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 77, "method": method }).to_string();
    wire_post(w, body, &[]).await;
}

#[when("the agent initializes and requests MCP resources through the hub")]
async fn wire_init_and_resources(w: &mut GatewayWorld) {
    let init = w
        .agent_request(json!({ "jsonrpc": "2.0", "id": 78, "method": "initialize" }))
        .await;
    w.last_init = Some(init);
    let resp = w
        .agent_request(json!({ "jsonrpc": "2.0", "id": 79, "method": "resources/list" }))
        .await;
    w.last_response = Some(resp);
}

#[then(expr = "the HTTP status is {int}")]
async fn wire_status_is(w: &mut GatewayWorld, status: u16) {
    let last = w.wire_responses.last().expect("a wire exchange");
    assert_eq!(last.status, status, "the wire status");
}

#[then("the HTTP body is empty")]
async fn wire_body_empty(w: &mut GatewayWorld) {
    let last = w.wire_responses.last().expect("a wire exchange");
    assert!(
        last.body.is_empty(),
        "an empty transport body, got: {}",
        String::from_utf8_lossy(&last.body)
    );
}

#[then("no request reaches any upstream")]
async fn wire_upstreams_untouched(w: &mut GatewayWorld) {
    for (name, fake) in &w.multi_upstreams {
        assert!(
            fake.seen.lock().unwrap().is_empty(),
            "upstream `{name}` was never contacted"
        );
    }
}

#[then("no act is recorded in any gamma")]
async fn wire_gammas_unmoved(w: &mut GatewayWorld) {
    for (name, store) in &w.ctx_stores {
        let count = gamma_view(store.clone()).expect("gamma").len();
        assert_eq!(count, w.gamma_baseline[name], "gamma of `{name}` unmoved");
    }
    let journal = gamma_view(w.journal_store.clone().expect("journal store"))
        .expect("gamma")
        .len();
    assert_eq!(journal, w.gamma_baseline["journal"], "journal unmoved");
}

#[then("the response is a JSON-RPC error with a null id naming the missing id")]
async fn wire_idless_refused(w: &mut GatewayWorld) {
    let msg = w.wire_responses.last().expect("a wire exchange").json();
    assert!(msg["id"].is_null(), "a null id: {msg}");
    let text = msg["error"]["message"].as_str().expect("an error message");
    assert!(text.contains("id"), "the refusal names the id: {text}");
}

#[then("the answer is exactly the empty JSON-RPC result")]
async fn ping_empty_result(w: &mut GatewayWorld) {
    let resp = w.last_response.as_ref().expect("an answer");
    assert!(resp.get("error").is_none(), "no error: {resp}");
    assert_eq!(resp["result"], json!({}), "the empty result: {resp}");
}

#[then("the response carries an Mcp-Session-Id header of visible ASCII")]
async fn wire_session_ascii(w: &mut GatewayWorld) {
    let sid = w.wire_session.as_deref().expect("a minted session id");
    assert!(
        !sid.is_empty() && sid.bytes().all(|b| (0x21..=0x7e).contains(&b)),
        "visible ASCII: {sid}"
    );
}

#[then("two initializations yield two different session ids")]
async fn wire_sessions_differ(w: &mut GatewayWorld) {
    let first = w.wire_session.clone().expect("a first id");
    wire_initializes(w).await;
    let second = w.wire_session.clone().expect("a second id");
    assert_ne!(first, second, "opaque ids never repeat");
}

#[then("the call is served")]
async fn wire_call_served(w: &mut GatewayWorld) {
    let last = w.wire_responses.last().expect("a wire exchange");
    assert_eq!(last.status, 200, "a served call");
    let msg = last.json();
    assert!(
        msg.get("error").is_none() && msg.get("result").is_some(),
        "served: {msg}"
    );
}

#[then("the response echoes the same Mcp-Session-Id header")]
async fn wire_session_echoed(w: &mut GatewayWorld) {
    let minted = w.wire_session.clone().expect("the minted id");
    let last = w.wire_responses.last().expect("a wire exchange");
    assert_eq!(
        last.session.as_deref(),
        Some(minted.as_str()),
        "the id rides back"
    );
}

#[then("both calls are served")]
async fn wire_both_served(w: &mut GatewayWorld) {
    let n = w.wire_responses.len();
    assert!(n >= 2, "two wire exchanges");
    for r in &w.wire_responses[n - 2..] {
        assert_eq!(r.status, 200, "a served call");
        let msg = r.json();
        assert!(
            msg.get("error").is_none() && msg.get("result").is_some(),
            "served: {msg}"
        );
    }
}

#[then("neither response is an error")]
async fn wire_neither_error(w: &mut GatewayWorld) {
    let n = w.wire_responses.len();
    for r in &w.wire_responses[n - 2..] {
        assert!(r.json().get("error").is_none(), "no error rode the wire");
    }
}

#[then(expr = "the response is a JSON-RPC error with a null id and code {int}")]
async fn wire_error_code(w: &mut GatewayWorld, code: i64) {
    let msg = w.wire_responses.last().expect("a wire exchange").json();
    assert!(msg["id"].is_null(), "a null id: {msg}");
    assert_eq!(msg["error"]["code"].as_i64(), Some(code), "the code: {msg}");
}

#[then("the error message names batching as unsupported")]
async fn wire_batch_named(w: &mut GatewayWorld) {
    let msg = w.wire_responses.last().expect("a wire exchange").json();
    let text = msg["error"]["message"].as_str().expect("an error message");
    assert!(text.contains("batching"), "names batching: {text}");
}

#[then("the initialize capabilities announce tools and nothing else")]
async fn init_capabilities_tools_only(w: &mut GatewayWorld) {
    let init = w.last_init.as_ref().expect("an initialize answer");
    assert_eq!(
        init.pointer("/result/capabilities"),
        Some(&json!({ "tools": {} })),
        "tools, nothing else"
    );
}

#[then("the resources request is refused with method-not-found")]
async fn resources_method_not_found(w: &mut GatewayWorld) {
    let resp = w.last_response.as_ref().expect("an answer");
    assert_eq!(
        resp.pointer("/error/code").and_then(Value::as_i64),
        Some(METHOD_NOT_FOUND_CODE),
        "method-not-found: {resp}"
    );
}

// ============================================ OAuth AS (lot G3)

/// One captured HTTP exchange from the OAuth flow — status, headers
/// (lowercased names), body. The wire IS the thing under test.
#[derive(Clone, Default)]
struct HttpCapture {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl HttpCapture {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

fn no_redirect_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client builds")
}

/// Turn a reqwest response into a capture and record it (both in the
/// OAuth log and the shared wire log, so `the HTTP status is` works).
async fn record_http(w: &mut GatewayWorld, resp: reqwest::Response) -> HttpCapture {
    let status = resp.status().as_u16();
    let mut headers = BTreeMap::new();
    for (name, value) in resp.headers() {
        if let Ok(v) = value.to_str() {
            headers.insert(name.as_str().to_lowercase(), v.to_owned());
        }
    }
    let body = resp.bytes().await.expect("a body").to_vec();
    let cap = HttpCapture {
        status,
        headers,
        body,
    };
    w.oauth_http.push(cap.clone());
    w.wire_responses.push(WireResponse {
        status,
        session: None,
        body: cap.body.clone(),
    });
    cap
}

fn oauth_base(w: &GatewayWorld) -> String {
    w.oauth_base.clone().expect("a served gateway base")
}

fn oauth_resource(w: &GatewayWorld) -> String {
    format!("{}/mcp", oauth_base(w))
}

fn capture_gamma_baseline(w: &mut GatewayWorld) {
    let mut baseline = BTreeMap::new();
    for (name, store) in &w.ctx_stores {
        baseline.insert(
            name.clone(),
            gamma_view(store.clone()).expect("gamma").len(),
        );
    }
    baseline.insert(
        "journal".to_owned(),
        gamma_view(w.journal_store.clone().expect("journal store"))
            .expect("gamma")
            .len(),
    );
    w.gamma_baseline = baseline;
}

/// Serve the standard two-context runner WITH an active authorization
/// server over a real loopback socket. The issuer is the served base, so
/// `resource = {issuer}/mcp` matches what clients send. The clock is a
/// mutable cell (the expiry scenarios advance it).
async fn serve_with_as(w: &mut GatewayWorld, extra_allow: Vec<String>, clock0: &str) {
    provision_runner(w, "company-brand".into(), "ui-designer".into(), false).await;
    let runner = Arc::clone(&w.router.as_ref().expect("a provisioned router").runner);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("loopback listener");
    let addr = listener.local_addr().expect("local addr");
    let issuer = format!("http://{addr}");

    // The adapter key from a known seed (the harness keeps the seed to
    // forge "right key, wrong audience" tokens later).
    let mut ent = SeqEntropy::default();
    let seed = ent.e32();
    let adapter = AdapterKey::from_seed(seed);
    w.oauth_adapter_seed = Some(seed);

    let auth = Arc::new(AuthServer::new(
        adapter,
        &issuer,
        3_600,
        7 * 86_400,
        extra_allow,
        Box::new(SeqEntropy::default()),
    ));
    let clock_cell = Arc::new(StdMutex::new(clock0.to_owned()));
    let cc = Arc::clone(&clock_cell);
    let routing = Arc::new(McpRouter {
        runner,
        upstreams: w.multi_upstreams.clone(),
        clock: Arc::new(move || cc.lock().unwrap().clone()),
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: Some(Arc::clone(&auth)),
    });
    let app = router_multi(Arc::clone(&routing)).merge(router_oauth(Arc::clone(&routing)));
    let listener = listener;
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    w.oauth_base = Some(issuer);
    w.oauth_clock = Some(clock_cell);
    w.router = Some(routing);
    capture_gamma_baseline(w);
}

#[given("a gateway served with an active authorization server")]
async fn given_as(w: &mut GatewayWorld) {
    serve_with_as(w, Vec::new(), T0).await;
}

#[given(expr = "a gateway served with an authorization server also allowing {string}")]
async fn given_as_extra(w: &mut GatewayWorld, uri: String) {
    serve_with_as(w, vec![uri], T0).await;
}

#[given(
    expr = "a gateway served with an active authorization server whose clock sits 30 minutes before the chain expiry"
)]
async fn given_as_near_expiry(w: &mut GatewayWorld) {
    // NOT_AFTER is 2026-08-09T00:00:00Z; 30 minutes earlier:
    serve_with_as(w, Vec::new(), "2026-08-08T23:30:00Z").await;
}

async fn do_register(w: &mut GatewayWorld, body: Value) -> HttpCapture {
    let url = format!("{}/register", oauth_base(w));
    let resp = no_redirect_client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .expect("register answers");
    record_http(w, resp).await
}

#[given(expr = "a registered public client with the redirect uri {string}")]
async fn given_registered_client(w: &mut GatewayWorld, uri: String) {
    let cap = do_register(w, json!({ "redirect_uris": [uri] })).await;
    assert_eq!(cap.status, 201, "registration: {}", cap.text());
    w.oauth_client_id = cap.json()["client_id"].as_str().map(str::to_owned);
    w.oauth_redirect = Some(uri);
}

#[when(expr = "a client registers with the redirect uri {string}")]
async fn when_register(w: &mut GatewayWorld, uri: String) {
    let cap = do_register(w, json!({ "redirect_uris": [uri] })).await;
    if cap.status == 201 {
        w.oauth_client_id = cap.json()["client_id"].as_str().map(str::to_owned);
    }
}

#[when(expr = "a client registers asking for the token endpoint auth method {string}")]
async fn when_register_method(w: &mut GatewayWorld, method: String) {
    do_register(
        w,
        json!({ "redirect_uris": [CLAUDE_CALLBACK_HARNESS], "token_endpoint_auth_method": method }),
    )
    .await;
}

const CLAUDE_CALLBACK_HARNESS: &str = "https://claude.ai/api/mcp/auth_callback";

fn pkce_pair() -> (String, String) {
    let verifier = "harness-pkce-verifier-aaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
    let challenge = s256_challenge(&verifier);
    (verifier, challenge)
}

fn authorize_query(
    client_id: &str,
    redirect: &str,
    challenge: Option<&str>,
    method: Option<&str>,
    resource: Option<&str>,
    state: &str,
) -> Vec<(String, String)> {
    let mut q = vec![
        ("client_id".to_owned(), client_id.to_owned()),
        ("redirect_uri".to_owned(), redirect.to_owned()),
        ("response_type".to_owned(), "code".to_owned()),
        ("state".to_owned(), state.to_owned()),
    ];
    if let Some(c) = challenge {
        q.push(("code_challenge".to_owned(), c.to_owned()));
    }
    if let Some(m) = method {
        q.push(("code_challenge_method".to_owned(), m.to_owned()));
    }
    if let Some(r) = resource {
        q.push(("resource".to_owned(), r.to_owned()));
    }
    q
}

async fn open_authorize(w: &mut GatewayWorld, query: Vec<(String, String)>) -> HttpCapture {
    let url = format!("{}/authorize", oauth_base(w));
    let resp = no_redirect_client()
        .get(&url)
        .query(&query)
        .send()
        .await
        .expect("authorize answers");
    record_http(w, resp).await
}

#[when("the client opens the authorize page with an S256 challenge and the resource")]
async fn when_authorize_ok(w: &mut GatewayWorld) {
    let (verifier, challenge) = pkce_pair();
    w.oauth_verifier = Some(verifier);
    w.oauth_challenge = Some(challenge.clone());
    w.oauth_state = Some("harness-state".to_owned());
    let client = w.oauth_client_id.clone().expect("a client");
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let resource = oauth_resource(w);
    let q = authorize_query(
        &client,
        &redirect,
        Some(&challenge),
        Some("S256"),
        Some(&resource),
        "harness-state",
    );
    open_authorize(w, q).await;
}

#[when(expr = "the client opens the authorize page with a {string} challenge")]
async fn when_authorize_method(w: &mut GatewayWorld, method: String) {
    let client = w.oauth_client_id.clone().expect("a client");
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let resource = oauth_resource(w);
    let q = authorize_query(
        &client,
        &redirect,
        Some("some-challenge"),
        Some(&method),
        Some(&resource),
        "harness-state",
    );
    open_authorize(w, q).await;
}

#[when("the client opens the authorize page without a code challenge")]
async fn when_authorize_no_challenge(w: &mut GatewayWorld) {
    let client = w.oauth_client_id.clone().expect("a client");
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let resource = oauth_resource(w);
    let q = authorize_query(
        &client,
        &redirect,
        None,
        None,
        Some(&resource),
        "harness-state",
    );
    open_authorize(w, q).await;
}

#[when("the client opens the authorize page without a resource")]
async fn when_authorize_no_resource(w: &mut GatewayWorld) {
    let (_v, challenge) = pkce_pair();
    let client = w.oauth_client_id.clone().expect("a client");
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let q = authorize_query(
        &client,
        &redirect,
        Some(&challenge),
        Some("S256"),
        None,
        "harness-state",
    );
    open_authorize(w, q).await;
}

#[when(expr = "the authorize page is opened for the unregistered client {string}")]
async fn when_authorize_unknown(w: &mut GatewayWorld, client: String) {
    let resource = oauth_resource(w);
    let (_v, challenge) = pkce_pair();
    let q = authorize_query(
        &client,
        "http://127.0.0.1:9410/cb",
        Some(&challenge),
        Some("S256"),
        Some(&resource),
        "harness-state",
    );
    open_authorize(w, q).await;
}

#[when(expr = "the client opens the authorize page with the redirect uri {string}")]
async fn when_authorize_bad_redirect(w: &mut GatewayWorld, redirect: String) {
    let client = w.oauth_client_id.clone().expect("a client");
    let resource = oauth_resource(w);
    let (_v, challenge) = pkce_pair();
    let q = authorize_query(
        &client,
        &redirect,
        Some(&challenge),
        Some("S256"),
        Some(&resource),
        "harness-state",
    );
    open_authorize(w, q).await;
}

#[when("the user approves the consent")]
async fn when_approve(w: &mut GatewayWorld) {
    let client = w.oauth_client_id.clone().expect("a client");
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let challenge = w.oauth_challenge.clone().expect("a challenge");
    let resource = oauth_resource(w);
    let state = w
        .oauth_state
        .clone()
        .unwrap_or_else(|| "harness-state".to_owned());
    let form = [
        ("client_id", client.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("resource", resource.as_str()),
        ("state", state.as_str()),
    ];
    let url = format!("{}/authorize", oauth_base(w));
    let resp = no_redirect_client()
        .post(&url)
        .form(&form)
        .send()
        .await
        .expect("approve answers");
    let cap = record_http(w, resp).await;
    if let Some(loc) = cap.header("location") {
        w.oauth_code = code_from_location(loc);
    }
}

fn code_from_location(location: &str) -> Option<String> {
    location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("code="))
        .map(|c| c.split('&').next().unwrap_or(c).to_owned())
}

/// Register + PKCE + consent + approve → a fresh code stored in the world.
async fn mint_code(w: &mut GatewayWorld) {
    if w.oauth_client_id.is_none() {
        given_registered_client(w, "http://127.0.0.1:9410/cb".to_owned()).await;
    }
    let (verifier, challenge) = pkce_pair();
    w.oauth_verifier = Some(verifier);
    w.oauth_challenge = Some(challenge);
    w.oauth_state = Some("harness-state".to_owned());
    when_approve(w).await;
}

#[given("an approved authorization code")]
async fn given_code(w: &mut GatewayWorld) {
    mint_code(w).await;
    assert!(w.oauth_code.is_some(), "a code was issued");
}

async fn do_token(w: &mut GatewayWorld, form: Vec<(String, String)>) -> HttpCapture {
    let url = format!("{}/token", oauth_base(w));
    let resp = no_redirect_client()
        .post(&url)
        .form(&form)
        .send()
        .await
        .expect("token answers");
    record_http(w, resp).await
}

async fn exchange(w: &mut GatewayWorld, verifier: &str, resource: &str) -> HttpCapture {
    let code = w.oauth_code.clone().expect("a code");
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let form = vec![
        ("grant_type".to_owned(), "authorization_code".to_owned()),
        ("code".to_owned(), code),
        ("code_verifier".to_owned(), verifier.to_owned()),
        ("resource".to_owned(), resource.to_owned()),
        ("redirect_uri".to_owned(), redirect),
    ];
    let cap = do_token(w, form).await;
    if cap.status == 200 {
        let body = cap.json();
        w.oauth_access = body["access_token"].as_str().map(str::to_owned);
        w.oauth_refresh = body["refresh_token"].as_str().map(str::to_owned);
    }
    cap
}

#[when("the client exchanges the code with its verifier and the resource")]
async fn when_exchange_ok(w: &mut GatewayWorld) {
    let verifier = w.oauth_verifier.clone().expect("a verifier");
    let resource = oauth_resource(w);
    exchange(w, &verifier, &resource).await;
}

#[when("the client exchanges the code with a wrong verifier")]
async fn when_exchange_wrong(w: &mut GatewayWorld) {
    let resource = oauth_resource(w);
    exchange(w, "the-wrong-verifier-000000000000000000000000", &resource).await;
}

#[when(expr = "the client exchanges the code naming the resource {string}")]
async fn when_exchange_bad_resource(w: &mut GatewayWorld, resource: String) {
    let verifier = w.oauth_verifier.clone().expect("a verifier");
    exchange(w, &verifier, &resource).await;
}

/// Full flow → a live token pair in the world.
async fn mint_pair(w: &mut GatewayWorld) {
    given_code(w).await;
    when_exchange_ok(w).await;
    assert!(
        w.oauth_access.is_some() && w.oauth_refresh.is_some(),
        "a pair was minted"
    );
}

#[given("a minted token pair")]
async fn given_pair(w: &mut GatewayWorld) {
    mint_pair(w).await;
}

#[given("a full authorization flow has run")]
async fn given_full_flow(w: &mut GatewayWorld) {
    mint_pair(w).await;
}

async fn do_refresh(w: &mut GatewayWorld, token: &str) -> HttpCapture {
    let form = vec![
        ("grant_type".to_owned(), "refresh_token".to_owned()),
        ("refresh_token".to_owned(), token.to_owned()),
    ];
    do_token(w, form).await
}

#[when("the client refreshes with the refresh token")]
async fn when_refresh(w: &mut GatewayWorld) {
    let token = w.oauth_refresh.clone().expect("a refresh token");
    let cap = do_refresh(w, &token).await;
    if cap.status == 200 {
        let body = cap.json();
        // Keep the OLD token around for the reuse assertion; store the new.
        w.oauth_access = body["access_token"].as_str().map(str::to_owned);
        w.oauth_state = Some(token); // stash the consumed token
        w.oauth_refresh = body["refresh_token"].as_str().map(str::to_owned);
    }
}

#[when("the client refreshes again with the consumed refresh token")]
async fn when_refresh_reuse(w: &mut GatewayWorld) {
    let consumed = w.oauth_state.clone().expect("the consumed token");
    do_refresh(w, &consumed).await;
}

#[when("the clock advances past the agent chain's not_after")]
async fn when_clock_past_chain(w: &mut GatewayWorld) {
    *w.oauth_clock.as_ref().expect("a clock").lock().unwrap() = "2026-08-10T00:00:00Z".to_owned();
}

#[when("the clock advances past the access token lifetime")]
async fn when_clock_past_access(w: &mut GatewayWorld) {
    // Access ttl is 3600s from T0 (2026-07-10T12:00:00Z).
    *w.oauth_clock.as_ref().expect("a clock").lock().unwrap() = "2026-07-10T14:00:00Z".to_owned();
}

async fn post_mcp(
    w: &mut GatewayWorld,
    body: Value,
    bearer: Option<&str>,
    origin: Option<&str>,
) -> HttpCapture {
    let url = format!("{}/mcp", oauth_base(w));
    let mut req = no_redirect_client()
        .post(&url)
        .header("content-type", "application/json")
        .body(body.to_string());
    if let Some(b) = bearer {
        req = req.header("authorization", format!("Bearer {b}"));
    }
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    let resp = req.send().await.expect("mcp answers");
    record_http(w, resp).await
}

#[when(expr = "the agent posts {string} without a bearer token")]
async fn when_post_no_bearer(w: &mut GatewayWorld, method: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 90, "method": method });
    post_mcp(w, body, None, None).await;
}

#[when(expr = "the agent posts {string} with the Origin header {string} and no bearer token")]
async fn when_post_origin_no_bearer(w: &mut GatewayWorld, method: String, origin: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 91, "method": method });
    post_mcp(w, body, None, Some(&origin)).await;
}

#[when(expr = "the agent issues a GET to {string}")]
async fn when_get_path(w: &mut GatewayWorld, path: String) {
    let url = format!("{}{}", oauth_base(w), path);
    let resp = no_redirect_client()
        .get(&url)
        .send()
        .await
        .expect("GET answers");
    record_http(w, resp).await;
}

#[when(expr = "the agent posts a tools call for {string} with the access token")]
async fn when_tools_call_bearer(w: &mut GatewayWorld, tool: String) {
    let access = w.oauth_access.clone().expect("an access token");
    w.last_tool = tool.clone();
    let body = json!({
        "jsonrpc": "2.0", "id": 92, "method": "tools/call",
        "params": { "name": tool, "arguments": {} }
    });
    post_mcp(w, body, Some(&access), None).await;
}

#[when(expr = "the agent posts {string} with the bearer token {string}")]
async fn when_post_bad_bearer(w: &mut GatewayWorld, method: String, token: String) {
    let body = json!({ "jsonrpc": "2.0", "id": 93, "method": method });
    post_mcp(w, body, Some(&token), None).await;
}

#[when(expr = "the agent posts {string} with the access token")]
async fn when_post_access(w: &mut GatewayWorld, method: String) {
    let access = w.oauth_access.clone().expect("an access token");
    let body = json!({ "jsonrpc": "2.0", "id": 95, "method": method });
    post_mcp(w, body, Some(&access), None).await;
}

#[when(
    expr = "the agent posts {string} with a token signed by the adapter key for another audience"
)]
async fn when_post_wrong_aud(w: &mut GatewayWorld, method: String) {
    let seed = w.oauth_adapter_seed.expect("the adapter seed");
    let adapter = AdapterKey::from_seed(seed);
    let token = adapter.sign_access_token(&json!({
        "iss": oauth_base(w),
        "aud": "https://elsewhere.example/mcp",
        "exp": 9_999_999_999i64,
    }));
    let body = json!({ "jsonrpc": "2.0", "id": 94, "method": method });
    post_mcp(w, body, Some(&token), None).await;
}

#[given("the agent's covering mandate is revoked")]
async fn given_revoked(w: &mut GatewayWorld) {
    let master = w.master();
    let mandate = w
        .ctx_agent_mandates
        .get("company-brand")
        .expect("the company-brand agent mandate")
        .clone();
    let store = w.ctx_stores.get("company-brand").expect("store").clone();
    owner_revoke_mandate_id(
        &master,
        "company-brand",
        &mandate,
        "revoked in the G3 scenario",
        store,
        T0,
        &mut OsEntropy,
    )
    .expect("revocation lands");
}

// -------------------------------------------------- G3 config parse

const AS_MULTI_BASE: &str = "\
listen: 127.0.0.1:4870
contexts:
  - name: company-brand
    upstream_mcp: http://127.0.0.1:5001/mcp
    store:
      kind: fs
      root: /var/lib/aithos/brand
    tools:
      brand.read: read
journal:
  store:
    kind: fs
    root: /var/lib/aithos/journal
";

const AS_MONO_BASE: &str = "\
listen: 127.0.0.1:4870
upstream_mcp: http://127.0.0.1:4124/mcp
store:
  kind: fs
  root: /var/lib/aithos
tools:
  user.read: read
";

fn record_config_error(w: &mut GatewayWorld, text: &str) {
    w.config_error = match GatewayConfig::from_yaml(text) {
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    };
}

#[when("a gateway config declares an as: stanza on the mono shape")]
async fn cfg_as_mono(w: &mut GatewayWorld) {
    let text = format!("{AS_MONO_BASE}as:\n  issuer: http://127.0.0.1:4870\n");
    record_config_error(w, &text);
}

#[when(expr = "a gateway config declares the as: issuer {string}")]
async fn cfg_as_issuer(w: &mut GatewayWorld, issuer: String) {
    let text = format!("{AS_MULTI_BASE}as:\n  issuer: {issuer}\n");
    record_config_error(w, &text);
}

#[when("a gateway config declares an as: stanza with an unknown field")]
async fn cfg_as_unknown(w: &mut GatewayWorld) {
    let text = format!("{AS_MULTI_BASE}as:\n  issuer: http://127.0.0.1:4870\n  surprise: true\n");
    record_config_error(w, &text);
}

#[then("the config is rejected naming the multi-context requirement")]
async fn cfg_rejected_multi(w: &mut GatewayWorld) {
    let err = w.config_error.as_ref().expect("a rejection");
    assert!(err.contains("multi-context"), "names multi-context: {err}");
}

#[then("the config is rejected naming the TLS requirement")]
async fn cfg_rejected_tls(w: &mut GatewayWorld) {
    let err = w.config_error.as_ref().expect("a rejection");
    assert!(err.contains("TLS"), "names TLS: {err}");
}

#[then("the config is rejected naming the unknown field")]
async fn cfg_rejected_unknown(w: &mut GatewayWorld) {
    let err = w.config_error.as_ref().expect("a rejection");
    assert!(
        err.contains("unknown field") || err.contains("surprise"),
        "names the unknown field: {err}"
    );
}

// -------------------------------------------------- G3 then-steps

#[then("the WWW-Authenticate header points the protected resource metadata")]
async fn then_www_points_metadata(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    let header = cap.header("www-authenticate").expect("a challenge header");
    assert!(
        header.contains("resource_metadata") && header.contains("oauth-protected-resource"),
        "points the metadata: {header}"
    );
}

#[then("the WWW-Authenticate header names an invalid token")]
async fn then_www_invalid(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    let header = cap.header("www-authenticate").expect("a challenge header");
    assert!(
        header.contains("invalid_token"),
        "names invalid_token: {header}"
    );
}

#[then("the metadata names the /mcp endpoint as the resource")]
async fn then_meta_resource(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.json()["resource"], oauth_resource(w));
}

#[then("the metadata lists the issuer as the only authorization server")]
async fn then_meta_issuer(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.json()["authorization_servers"], json!([oauth_base(w)]));
}

#[then("the metadata names the issuer and the authorize, token and registration endpoints")]
async fn then_meta_endpoints(w: &mut GatewayWorld) {
    let base = oauth_base(w);
    let cap = w.oauth_http.last().expect("an exchange");
    let m = cap.json();
    assert_eq!(m["issuer"], base);
    assert_eq!(m["authorization_endpoint"], format!("{base}/authorize"));
    assert_eq!(m["token_endpoint"], format!("{base}/token"));
    assert_eq!(m["registration_endpoint"], format!("{base}/register"));
}

#[then(expr = "the only code challenge method is {string}")]
async fn then_only_challenge_method(w: &mut GatewayWorld, method: String) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(
        cap.json()["code_challenge_methods_supported"],
        json!([method])
    );
}

#[then(expr = "the only token endpoint auth method is {string}")]
async fn then_only_auth_method(w: &mut GatewayWorld, method: String) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(
        cap.json()["token_endpoint_auth_methods_supported"],
        json!([method])
    );
}

#[then("the grant types are exactly authorization code and refresh token")]
async fn then_grant_types(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(
        cap.json()["grant_types_supported"],
        json!(["authorization_code", "refresh_token"])
    );
}

#[then("the registration answers 201 with a client_id and no client_secret")]
async fn then_registered_public(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 201, "created: {}", cap.text());
    let body = cap.json();
    assert!(body["client_id"].is_string(), "a client_id");
    assert!(
        body.get("client_secret").is_none(),
        "no secret for a public client"
    );
}

#[then("both registrations answer 201")]
async fn then_both_registered(w: &mut GatewayWorld) {
    let n = w.oauth_http.len();
    assert!(n >= 2, "two registrations");
    for cap in &w.oauth_http[n - 2..] {
        assert_eq!(cap.status, 201, "created: {}", cap.text());
    }
}

#[then(expr = "the registration is refused with the error {string}")]
async fn then_registration_refused(w: &mut GatewayWorld, error: String) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 400, "a 400: {}", cap.text());
    assert_eq!(cap.json()["error"], error);
}

#[then("the refusal names the built-in allowlist")]
async fn then_names_allowlist(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    let desc = cap.json()["error_description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(desc.contains("allowlist"), "names the allowlist: {desc}");
}

#[then("no client is registered")]
async fn then_no_client(w: &mut GatewayWorld) {
    assert!(w.oauth_client_id.is_none(), "no client_id was captured");
}

#[then("the refusal names public PKCE clients")]
async fn then_names_public(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    let desc = cap.json()["error_description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(desc.contains("public"), "names public clients: {desc}");
}

#[then("the page is marked DEV and names the client_id and the resource")]
async fn then_consent_page(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 200, "a page");
    let html = cap.text();
    let client = w.oauth_client_id.clone().expect("a client");
    assert!(html.contains("DEV consent"), "marked DEV");
    assert!(html.contains(&client), "names the client");
    assert!(html.contains(&oauth_resource(w)), "names the resource");
}

#[then("no authorization code is issued yet")]
async fn then_no_code_yet(w: &mut GatewayWorld) {
    assert!(w.oauth_code.is_none(), "no code before approval");
}

#[then("no authorization code is issued")]
async fn then_no_code(w: &mut GatewayWorld) {
    assert!(w.oauth_code.is_none(), "no code issued");
}

#[then("the redirect goes to the registered redirect uri with a code and the presented state")]
async fn then_redirect_code(w: &mut GatewayWorld) {
    let redirect = w.oauth_redirect.clone().expect("a redirect");
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 302, "a redirect");
    let loc = cap.header("location").expect("a location");
    assert!(loc.starts_with(&redirect), "to the registered uri: {loc}");
    assert!(loc.contains("code="), "carries a code");
    assert!(
        loc.contains("state=harness-state"),
        "echoes the state: {loc}"
    );
}

#[then(expr = "the redirect carries the error {string} naming S256")]
async fn then_redirect_err_s256(w: &mut GatewayWorld, error: String) {
    then_redirect_error_naming(w, &error, "s256").await;
}

#[then(expr = "the redirect carries the error {string} naming PKCE")]
async fn then_redirect_err_pkce(w: &mut GatewayWorld, error: String) {
    then_redirect_error_naming(w, &error, "pkce").await;
}

#[then(expr = "the redirect carries the error {string} naming the resource requirement")]
async fn then_redirect_err_resource(w: &mut GatewayWorld, error: String) {
    then_redirect_error_naming(w, &error, "resource").await;
}

async fn then_redirect_error_naming(w: &mut GatewayWorld, error: &str, needle: &str) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 302, "a redirect");
    let loc = cap.header("location").expect("a location").to_lowercase();
    assert!(
        loc.contains(&format!("error={error}")),
        "carries {error}: {loc}"
    );
    assert!(loc.contains(needle), "names {needle}: {loc}");
}

#[then("the answer names dynamic registration as the supported path")]
async fn then_names_dcr(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 400, "a 400");
    let desc = cap.json()["error_description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(desc.contains("regist"), "names registration: {desc}");
}

#[then("the answer carries an access token, a refresh token and the default lifetimes")]
async fn then_token_answer(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 200, "a token: {}", cap.text());
    let body = cap.json();
    assert!(body["access_token"].is_string(), "an access token");
    assert!(body["refresh_token"].is_string(), "a refresh token");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], 3600, "the default access lifetime");
}

fn jwt_payload(token: &str) -> Value {
    let part = token.split('.').nth(1).expect("a payload segment");
    let bytes = b64url_decode(part).expect("payload decodes");
    serde_json::from_slice(&bytes).expect("payload is JSON")
}

#[then("the access token audience is the /mcp resource")]
async fn then_token_aud(w: &mut GatewayWorld) {
    let access = w.oauth_access.clone().expect("an access token");
    assert_eq!(jwt_payload(&access)["aud"], oauth_resource(w));
}

#[then("the issuance is journalized as a governance act naming the client")]
async fn then_issuance_journalized(w: &mut GatewayWorld) {
    let journal = gamma_view(w.journal_store.clone().expect("journal")).expect("gamma");
    let client = w.oauth_client_id.clone().expect("a client");
    let hit = journal.iter().any(|e| {
        e.kind == "action"
            && e.target.as_deref() == Some("x.gateway")
            && payload_str(e, "event") == Some("oauth.issue")
            && payload_str(e, "client_id") == Some(client.as_str())
    });
    assert!(hit, "an oauth.issue governance act names the client");
}

#[then("no token byte appears in any gamma payload")]
async fn then_no_token_in_gamma(w: &mut GatewayWorld) {
    assert_no_secret_in_gamma(w);
}

#[then(expr = "the exchange is refused with the error {string}")]
async fn then_exchange_refused(w: &mut GatewayWorld, error: String) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_ne!(cap.status, 200, "not a success: {}", cap.text());
    assert_eq!(cap.json()["error"], error, "the error code: {}", cap.text());
}

#[then("a fresh access token and a fresh refresh token come back")]
async fn then_fresh_pair(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_eq!(cap.status, 200, "a rotation: {}", cap.text());
    let body = cap.json();
    assert!(body["access_token"].is_string(), "a fresh access token");
    let fresh = body["refresh_token"]
        .as_str()
        .expect("a fresh refresh token");
    let consumed = w.oauth_state.clone().expect("the consumed token");
    assert_ne!(fresh, consumed, "the refresh token rotated");
}

#[then("the successor refresh token is dead too")]
async fn then_successor_dead(w: &mut GatewayWorld) {
    let successor = w.oauth_refresh.clone().expect("the successor token");
    let cap = do_refresh(w, &successor).await;
    assert_ne!(cap.status, 200, "the family is cut: {}", cap.text());
    assert_eq!(cap.json()["error"], "invalid_grant");
}

#[then("the exchange is refused naming the expired authority")]
async fn then_refused_expired_authority(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    assert_ne!(cap.status, 200, "refused: {}", cap.text());
    let desc = cap.json()["error_description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        desc.contains("authority") || desc.contains("expired"),
        "names the expired authority: {desc}"
    );
}

#[then("the access token expires with the chain, not after it")]
async fn then_token_capped(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    let body = cap.json();
    // 30 minutes to the chain end, not the full 60-minute access ttl.
    assert_eq!(body["expires_in"], 1800, "capped at the chain expiry");
}

#[then(expr = "the act is recorded in the {string} gamma")]
async fn then_act_in_gamma(w: &mut GatewayWorld, ctx: String) {
    let store = w.ctx_stores.get(&ctx).expect("a context store").clone();
    let acts = acts_on(&gamma_view(store).expect("gamma"), "x.mcp");
    assert!(!acts.is_empty(), "an act landed in the `{ctx}` gamma");
}

#[then("the call is refused naming the revoked authority")]
async fn then_call_refused_revoked(w: &mut GatewayWorld) {
    let cap = w.oauth_http.last().expect("an exchange");
    let msg = cap.json();
    let text = msg["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        text.contains("revok") || text.contains("append"),
        "the refusal names the revoked authority: {text}"
    );
}

#[then("no gamma payload anywhere carries a token or code byte")]
async fn then_no_secret_anywhere(w: &mut GatewayWorld) {
    assert_no_secret_in_gamma(w);
}

fn assert_no_secret_in_gamma(w: &GatewayWorld) {
    let mut needles = Vec::new();
    for v in [&w.oauth_access, &w.oauth_refresh, &w.oauth_code]
        .into_iter()
        .flatten()
    {
        needles.push(v.clone());
    }
    let mut stores: Vec<GatewayStore> = w.ctx_stores.values().cloned().collect();
    if let Some(j) = &w.journal_store {
        stores.push(j.clone());
    }
    for store in stores {
        for entry in gamma_view(store).expect("gamma") {
            let blob = serde_json::to_string(&entry).unwrap_or_default();
            for needle in &needles {
                assert!(
                    needle.len() < 8 || !blob.contains(needle.as_str()),
                    "a token/code leaked into a gamma payload"
                );
            }
        }
    }
}

#[then("no error body of the flow echoed a token or code")]
async fn then_no_secret_in_errors(w: &mut GatewayWorld) {
    let mut needles = Vec::new();
    for v in [&w.oauth_access, &w.oauth_refresh, &w.oauth_code]
        .into_iter()
        .flatten()
    {
        needles.push(v.clone());
    }
    for cap in &w.oauth_http {
        if cap.status >= 400 {
            let body = cap.text();
            for needle in &needles {
                assert!(
                    needle.len() < 8 || !body.contains(needle.as_str()),
                    "an error body echoed a token or code"
                );
            }
        }
    }
}

// ============================================ upstream OAuth client

#[derive(Clone)]
struct FakeUpstreamOAuthState {
    token_grants: Arc<StdMutex<Vec<BTreeMap<String, String>>>>,
    resource_bearers: Arc<StdMutex<Vec<Option<String>>>>,
    refuse_refresh: Arc<AtomicBool>,
    initial_expires_in: u64,
}

fn upstream_oauth_yaml(base: &str, callback: &str, bearer: bool) -> String {
    let bearer = if bearer {
        "    bearer_token: forbidden-inline-secret\n"
    } else {
        ""
    };
    format!(
        "listen: 127.0.0.1:4870
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth: {{ kind: token-env, env: AITHOS_VAULT_TOKEN }}
servers:
  - name: protected
    transport: http
    url: {base}/mcp
{bearer}    oauth:
      auth_url: {base}/authorize
      token_url: {base}/token
      client_id: owner-public-client
      client_secret:
        broker: enterprise
        path: aithos/oauth/client
        field: client_secret
      scopes: [resource.read]
      redirect_uri: {callback}
      token_vault:
        broker: enterprise
        path: {UPSTREAM_TOKEN_PATH}
        field: {UPSTREAM_TOKEN_FIELD}
contexts:
  - name: protected-context
    store: {{ kind: fs, root: /tmp/aithos-upstream-oauth-context }}
    tools:
      protected__read:
        server: protected
        tool: read
        access: read
journal:
  store: {{ kind: fs, root: /tmp/aithos-upstream-oauth-journal }}
"
    )
}

async fn provision_upstream_oauth(initial_expires_in: u64) -> UpstreamOAuthHarness {
    use axum::extract::{Form, State};
    use axum::http::HeaderMap;
    use axum::routing::post;
    use axum::{Json, Router};

    let state = FakeUpstreamOAuthState {
        token_grants: Arc::default(),
        resource_bearers: Arc::default(),
        refuse_refresh: Arc::new(AtomicBool::new(false)),
        initial_expires_in,
    };
    let app = Router::new()
        .route(
            "/token",
            post(
                |State(state): State<FakeUpstreamOAuthState>,
                 Form(form): Form<BTreeMap<String, String>>| async move {
                    state.token_grants.lock().unwrap().push(form.clone());
                    let grant = form.get("grant_type").map(String::as_str);
                    if grant == Some("refresh_token") && state.refuse_refresh.load(Ordering::SeqCst)
                    {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(json!({
                                "error": "invalid_grant",
                                "adversarial_body": [
                                    UPSTREAM_CLIENT_SECRET,
                                    UPSTREAM_REFRESH_1,
                                    UPSTREAM_ACCESS_1
                                ]
                            })),
                        );
                    }
                    let body = if grant == Some("refresh_token") {
                        json!({
                            "access_token": UPSTREAM_ACCESS_2,
                            "refresh_token": UPSTREAM_REFRESH_2,
                            "expires_in": 3600,
                            "token_type": "Bearer",
                            "scope": "resource.read"
                        })
                    } else {
                        json!({
                            "access_token": UPSTREAM_ACCESS_1,
                            "refresh_token": UPSTREAM_REFRESH_1,
                            "expires_in": state.initial_expires_in,
                            "token_type": "Bearer",
                            "scope": "resource.read"
                        })
                    };
                    (axum::http::StatusCode::OK, Json(body))
                },
            ),
        )
        .route(
            "/mcp",
            post(
                |State(state): State<FakeUpstreamOAuthState>,
                 headers: HeaderMap,
                 Json(body): Json<Value>| async move {
                    state.resource_bearers.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned),
                    );
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id").cloned().unwrap_or(Value::Null),
                        "result": { "content": [{"type":"text", "text":"protected-ok"}] }
                    }))
                },
            ),
        )
        .with_state(state.clone());
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fake OAuth listener");
    let upstream_port = upstream_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(upstream_listener, app).await.ok();
    });
    let base = format!("http://127.0.0.1:{upstream_port}");

    let callback_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("callback listener");
    let callback_port = callback_listener.local_addr().unwrap().port();
    let callback_url = format!("http://127.0.0.1:{callback_port}/oauth/callback");
    let text = upstream_oauth_yaml(&base, &callback_url, false);
    let cfg = GatewayConfig::from_yaml(&text).expect("OAuth config");
    let vault = Arc::new(MemoryOAuthVault::default());
    vault.put_clear(
        "aithos/oauth/client",
        "client_secret",
        UPSTREAM_CLIENT_SECRET,
    );
    let mut brokers: BTreeMap<String, Arc<dyn CredentialBroker>> = BTreeMap::new();
    brokers.insert("enterprise".into(), vault.clone());
    let registry =
        Arc::new(UpstreamOAuthRegistry::from_config(&cfg, &brokers).expect("OAuth registry"));
    let server = &cfg.servers.as_ref().unwrap()[0];
    let upstream =
        HttpUpstream::for_server_with_oauth(server, &brokers, &registry).expect("OAuth upstream");
    let callback_app = upstream_oauth::router(Arc::clone(&registry));
    tokio::spawn(async move {
        axum::serve(callback_listener, callback_app).await.ok();
    });

    UpstreamOAuthHarness {
        vault,
        registry,
        upstream,
        token_grants: state.token_grants,
        resource_bearers: state.resource_bearers,
        refuse_refresh: state.refuse_refresh,
        callback_url,
    }
}

async fn start_upstream_consent(w: &mut GatewayWorld) {
    let harness = w.upstream_oauth.as_ref().expect("OAuth harness");
    let start = harness
        .registry
        .start("protected")
        .await
        .expect("consent URL");
    w.upstream_oauth_consent = Some(start.authorization_url);
}

fn upstream_consent_state(w: &GatewayWorld) -> String {
    let url =
        reqwest::Url::parse(w.upstream_oauth_consent.as_deref().expect("consent URL")).unwrap();
    url.query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .expect("state")
}

async fn complete_upstream_consent(w: &mut GatewayWorld) {
    start_upstream_consent(w).await;
    let state = upstream_consent_state(w);
    let callback_url = w
        .upstream_oauth
        .as_ref()
        .expect("OAuth harness")
        .callback_url
        .clone();
    let response = reqwest::Client::new()
        .get(callback_url)
        .query(&[("code", "approved-code"), ("state", state.as_str())])
        .send()
        .await
        .expect("callback response");
    let status = response.status().as_u16();
    let body = response.bytes().await.unwrap().to_vec();
    w.upstream_oauth_callback = Some(HttpCapture {
        status,
        headers: BTreeMap::new(),
        body,
    });
}

fn upstream_token_record(w: &GatewayWorld) -> String {
    w.upstream_oauth
        .as_ref()
        .expect("OAuth harness")
        .vault
        .clear(UPSTREAM_TOKEN_PATH, UPSTREAM_TOKEN_FIELD)
        .expect("OAuth token record")
}

#[when("a hub server declares OAuth authorization code with PKCE and Vault custody")]
async fn upstream_oauth_config_valid(w: &mut GatewayWorld) {
    let text = upstream_oauth_yaml(
        "https://protected.example",
        "https://gateway.example/oauth/callback",
        false,
    );
    w.config_error = GatewayConfig::from_yaml(&text)
        .err()
        .map(|error| error.to_string());
    w.upstream_oauth_config = Some(text);
}

#[then("the OAuth configuration is accepted without any secret value")]
async fn upstream_oauth_config_secretless(w: &mut GatewayWorld) {
    assert!(w.config_error.is_none(), "{:?}", w.config_error);
    let text = w.upstream_oauth_config.as_deref().unwrap();
    for secret in [
        UPSTREAM_CLIENT_SECRET,
        UPSTREAM_ACCESS_1,
        UPSTREAM_REFRESH_1,
    ] {
        assert!(!text.contains(secret));
    }
    assert!(text.contains("client_secret:") && text.contains("token_vault:"));
}

#[when("a hub server declares OAuth and a static bearer together")]
async fn upstream_oauth_config_competing(w: &mut GatewayWorld) {
    let text = upstream_oauth_yaml(
        "https://protected.example",
        "https://gateway.example/oauth/callback",
        true,
    );
    w.config_error = GatewayConfig::from_yaml(&text)
        .err()
        .map(|error| error.to_string());
}

#[then("the configuration is rejected naming the competing credential modes")]
async fn upstream_oauth_config_competing_rejected(w: &mut GatewayWorld) {
    let error = w.config_error.as_deref().expect("config rejected");
    assert!(error.contains("competing credential modes"), "{error}");
}

#[given("a protected upstream with a fake OAuth authorization server")]
async fn upstream_oauth_fake(w: &mut GatewayWorld) {
    w.upstream_oauth = Some(provision_upstream_oauth(3600).await);
}

#[when("the owner builds the consent URL")]
async fn upstream_oauth_owner_consent(w: &mut GatewayWorld) {
    start_upstream_consent(w).await;
}

#[then("the URL carries S256 PKCE, state, the configured scopes and redirect URI")]
async fn upstream_oauth_consent_exact(w: &mut GatewayWorld) {
    let harness = w.upstream_oauth.as_ref().unwrap();
    let url = reqwest::Url::parse(w.upstream_oauth_consent.as_deref().unwrap()).unwrap();
    let query: BTreeMap<_, _> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert!(query
        .get("code_challenge")
        .is_some_and(|value| !value.is_empty()));
    assert!(query.get("state").is_some_and(|value| !value.is_empty()));
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("resource.read")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some(harness.callback_url.as_str())
    );
}

#[then("the pending verifier lives only in the Vault record")]
async fn upstream_oauth_pending_in_vault(w: &mut GatewayWorld) {
    let record = upstream_token_record(w);
    let json: Value = serde_json::from_str(&record).unwrap();
    let verifier = json["code_verifier"].as_str().expect("verifier");
    assert!(verifier.len() >= 43);
    assert!(!w
        .upstream_oauth_consent
        .as_deref()
        .unwrap()
        .contains(verifier));
    assert!(!w
        .upstream_oauth_config
        .as_deref()
        .unwrap_or_default()
        .contains(verifier));
}

#[given("the owner has started consent")]
async fn upstream_oauth_started(w: &mut GatewayWorld) {
    start_upstream_consent(w).await;
}

#[when("the OAuth callback receives the approved code and matching state")]
async fn upstream_oauth_callback(w: &mut GatewayWorld) {
    let state = upstream_consent_state(w);
    let callback_url = w.upstream_oauth.as_ref().unwrap().callback_url.clone();
    let response = reqwest::Client::new()
        .get(callback_url)
        .query(&[("code", "approved-code"), ("state", state.as_str())])
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.bytes().await.unwrap().to_vec();
    w.upstream_oauth_callback = Some(HttpCapture {
        status,
        headers: BTreeMap::new(),
        body,
    });
}

#[then("the Vault record contains the access token, refresh token and expiry")]
async fn upstream_oauth_vault_connected(w: &mut GatewayWorld) {
    let record = upstream_token_record(w);
    let json: Value = serde_json::from_str(&record).unwrap();
    assert_eq!(json["status"], "connected");
    assert_eq!(json["access_token"], UPSTREAM_ACCESS_1);
    assert_eq!(json["refresh_token"], UPSTREAM_REFRESH_1);
    assert!(json["expires_at"].as_i64().is_some_and(|value| value > 0));
}

#[then("the callback response contains no token byte")]
async fn upstream_oauth_callback_redacted(w: &mut GatewayWorld) {
    let callback = w.upstream_oauth_callback.as_ref().unwrap();
    assert_eq!(callback.status, 200);
    let body = callback.text();
    for secret in [
        UPSTREAM_ACCESS_1,
        UPSTREAM_REFRESH_1,
        UPSTREAM_CLIENT_SECRET,
    ] {
        assert!(!body.contains(secret));
    }
}

#[given("a protected upstream with a completed OAuth consent")]
async fn upstream_oauth_completed(w: &mut GatewayWorld) {
    w.upstream_oauth = Some(provision_upstream_oauth(3600).await);
    complete_upstream_consent(w).await;
}

#[given("a protected upstream with an expired OAuth access token")]
async fn upstream_oauth_expired(w: &mut GatewayWorld) {
    w.upstream_oauth = Some(provision_upstream_oauth(1).await);
    complete_upstream_consent(w).await;
}

#[given("the fake OAuth server refuses refresh")]
async fn upstream_oauth_refuse_refresh(w: &mut GatewayWorld) {
    w.upstream_oauth
        .as_ref()
        .unwrap()
        .refuse_refresh
        .store(true, Ordering::SeqCst);
}

#[when("the gateway calls the protected resource")]
async fn upstream_oauth_call_resource(w: &mut GatewayWorld) {
    let result = w
        .upstream_oauth
        .as_ref()
        .unwrap()
        .upstream
        .forward(json!({"jsonrpc":"2.0", "id":1, "method":"tools/list"}))
        .await
        .map_err(|error| error.to_string());
    w.upstream_oauth_result = Some(result);
}

#[then("the resource sees exactly the Vault access token")]
async fn upstream_oauth_resource_access(w: &mut GatewayWorld) {
    let seen = w
        .upstream_oauth
        .as_ref()
        .unwrap()
        .resource_bearers
        .lock()
        .unwrap();
    assert_eq!(
        seen.as_slice(),
        &[Some(format!("Bearer {UPSTREAM_ACCESS_1}"))]
    );
}

#[then("no token byte appears in the gateway result or error text")]
async fn upstream_oauth_result_redacted(w: &mut GatewayWorld) {
    let text = format!("{:?}", w.upstream_oauth_result.as_ref().unwrap());
    for secret in [
        UPSTREAM_ACCESS_1,
        UPSTREAM_REFRESH_1,
        UPSTREAM_CLIENT_SECRET,
    ] {
        assert!(!text.contains(secret));
    }
}

#[then("the token endpoint receives one refresh grant")]
async fn upstream_oauth_one_refresh(w: &mut GatewayWorld) {
    let grants = w
        .upstream_oauth
        .as_ref()
        .unwrap()
        .token_grants
        .lock()
        .unwrap();
    assert_eq!(
        grants
            .iter()
            .filter(|form| form.get("grant_type").map(String::as_str) == Some("refresh_token"))
            .count(),
        1
    );
}

#[then("the resource sees the rotated access token")]
async fn upstream_oauth_resource_rotated(w: &mut GatewayWorld) {
    let seen = w
        .upstream_oauth
        .as_ref()
        .unwrap()
        .resource_bearers
        .lock()
        .unwrap();
    assert_eq!(
        seen.as_slice(),
        &[Some(format!("Bearer {UPSTREAM_ACCESS_2}"))]
    );
}

#[then("the rotated token set replaces the expired Vault record")]
async fn upstream_oauth_vault_rotated(w: &mut GatewayWorld) {
    let record = upstream_token_record(w);
    assert!(record.contains(UPSTREAM_ACCESS_2));
    assert!(record.contains(UPSTREAM_REFRESH_2));
    assert!(!record.contains(UPSTREAM_ACCESS_1));
}

#[then("the call is refused as OAuth unavailable")]
async fn upstream_oauth_refused(w: &mut GatewayWorld) {
    let error = w
        .upstream_oauth_result
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .expect("OAuth refusal");
    assert!(error.contains("upstream OAuth unavailable"), "{error}");
}

#[then("the protected resource receives zero requests")]
async fn upstream_oauth_no_resource(w: &mut GatewayWorld) {
    assert!(w
        .upstream_oauth
        .as_ref()
        .unwrap()
        .resource_bearers
        .lock()
        .unwrap()
        .is_empty());
}

#[then("the refusal contains no access token, refresh token or client secret")]
async fn upstream_oauth_refusal_redacted(w: &mut GatewayWorld) {
    let error = w
        .upstream_oauth_result
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .unwrap();
    for secret in [
        UPSTREAM_ACCESS_1,
        UPSTREAM_ACCESS_2,
        UPSTREAM_REFRESH_1,
        UPSTREAM_REFRESH_2,
        UPSTREAM_CLIENT_SECRET,
    ] {
        assert!(!error.contains(secret));
    }
}

#[tokio::main]
async fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/features");
    // Serial on purpose: worlds spawn real socket servers and do
    // blocking owner-side crypto inside steps; under concurrent
    // scenarios the tokio workers starve and wire responses miss the
    // brokered 5s budget (observed as flaky vault timeouts). One
    // scenario at a time keeps every timing assertion deterministic.
    GatewayWorld::cucumber()
        .max_concurrent_scenarios(Some(1))
        .filter_run(features, |_, _, scenario| {
            !scenario.tags.iter().any(|t| t == "wip")
        })
        .await;
}
