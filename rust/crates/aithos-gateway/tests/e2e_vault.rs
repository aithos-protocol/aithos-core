//! Network e2e of the enterprise credential vault slice (V3, CI-durable:
//! no Docker). The real binary provisions two governed servers whose
//! bearers live ONLY in a fake HashiCorp Vault KV v2 served over a real
//! socket: covered calls present the vault-resolved bearer to their own
//! upstream, refused calls wake neither the vault nor the upstream, a
//! vault outage closes the route with a journaled refusal, a KV rotation
//! is honoured on the next call without touching the YAML, the agent
//! cannot smuggle auth headers through tool arguments, and no secret
//! survives in any store, config, proposal or gateway stderr.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use serde_json::{json, Value};

use aithos_gateway::config::StoreConfig;
use aithos_gateway::core_bridge::{gamma_view, EntryView};
use aithos_gateway::store_adapter::GatewayStore;

const MASTER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const VAULT_ROOT: &str = "vault-root-sentinel-e2e";
const GITHUB_SECRET: &str = "github-vault-sentinel-e2e";
const GITHUB_ROTATED: &str = "github-rotated-sentinel-e2e";
const LINEAR_SECRET: &str = "linear-vault-sentinel-e2e";

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// ------------------------------------------------------- fake vault KV v2

#[derive(Clone)]
struct FakeVault {
    expected_token: String,
    secrets: Arc<Mutex<BTreeMap<String, BTreeMap<String, String>>>>,
    hits: Arc<Mutex<Vec<String>>>,
    tokens: Arc<Mutex<Vec<Option<String>>>>,
}

impl FakeVault {
    fn new(expected_token: &str) -> Self {
        Self {
            expected_token: expected_token.to_owned(),
            secrets: Arc::default(),
            hits: Arc::default(),
            tokens: Arc::default(),
        }
    }

    fn put(&self, path: &str, field: &str, value: &str) {
        self.secrets
            .lock()
            .unwrap()
            .entry(path.to_owned())
            .or_default()
            .insert(field.to_owned(), value.to_owned());
    }

    fn hit_count(&self) -> usize {
        self.hits.lock().unwrap().len()
    }
}

async fn spawn_vault(port: u16, vault: FakeVault) -> tokio::task::JoinHandle<()> {
    use axum::extract::{Path as AxumPath, State};
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::{Json, Router};

    let app = Router::new()
        .route(
            "/v1/secret/data/{*path}",
            get(
                |State(vault): State<FakeVault>,
                 AxumPath(path): AxumPath<String>,
                 headers: HeaderMap| async move {
                    let token = headers
                        .get("x-vault-token")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    vault.hits.lock().unwrap().push(path.clone());
                    vault.tokens.lock().unwrap().push(token.clone());
                    if token.as_deref() != Some(vault.expected_token.as_str()) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({ "errors": ["permission denied"] })),
                        );
                    }
                    match vault.secrets.lock().unwrap().get(&path) {
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
        .with_state(vault);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("fake vault binds");
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() })
}

// ------------------------------------------------------------- fake MCPs

#[derive(Clone)]
struct FakeMcp {
    tools: Arc<Mutex<Vec<Value>>>,
    seen: Arc<Mutex<Vec<Value>>>,
    auth: Arc<Mutex<Vec<Option<String>>>>,
    vault_header: Arc<Mutex<Vec<Option<String>>>>,
    answer: &'static str,
}

impl FakeMcp {
    fn new(tools: Vec<Value>, answer: &'static str) -> Self {
        Self {
            tools: Arc::new(Mutex::new(tools)),
            seen: Arc::default(),
            auth: Arc::default(),
            vault_header: Arc::default(),
            answer,
        }
    }

    fn call_count(&self) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["method"] == "tools/call")
            .count()
    }

    /// Forget the owner-side discovery traffic (which authenticates the
    /// OWNER's way, not through the runtime broker) before the run.
    fn clear_observations(&self) {
        self.seen.lock().unwrap().clear();
        self.auth.lock().unwrap().clear();
        self.vault_header.lock().unwrap().clear();
    }
}

async fn spawn_fake_mcp(fake: FakeMcp) -> u16 {
    use axum::extract::State;
    use axum::routing::post;
    use axum::{Json, Router};

    let port = free_port();
    let app = Router::new()
        .route(
            "/mcp",
            post(
                |State(fake): State<FakeMcp>, headers: HeaderMap, Json(body): Json<Value>| async move {
                    fake.seen.lock().unwrap().push(body.clone());
                    let header = |name: &str| {
                        headers
                            .get(name)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned)
                    };
                    fake.auth.lock().unwrap().push(header("authorization"));
                    fake.vault_header
                        .lock()
                        .unwrap()
                        .push(header("x-vault-token"));
                    let id = body.get("id").cloned().unwrap_or(Value::Null);
                    if body["method"] == "tools/list" {
                        return Json(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "tools": fake.tools.lock().unwrap().clone() }
                        }));
                    }
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
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

// ---------------------------------------------------------------- helpers

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn gateway_bin() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin("aithos-gateway"))
}

fn run_ok(args: &[&str]) -> String {
    let output = gateway_bin().args(args).output().expect("binary runs");
    assert!(
        output.status.success(),
        "`{}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn line_value(stdout: &str, prefix: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("`{prefix}` not printed"))
        .to_owned()
}

async fn wait_until_listening(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return;
        }
        assert!(Instant::now() < deadline, "gateway never started listening");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_until_closed(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
    {
        assert!(Instant::now() < deadline, "fake vault never went down");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn gamma(root: &std::path::Path) -> Vec<EntryView> {
    gamma_view(GatewayStore::from_config(&StoreConfig::Fs { root: root.into() }).unwrap())
        .expect("gamma readable")
}

fn acts_on(entries: &[EntryView], target: &str) -> Vec<EntryView> {
    entries
        .iter()
        .filter(|entry| entry.kind == "action" && entry.target.as_deref() == Some(target))
        .cloned()
        .collect()
}

fn payload_str<'a>(entry: &'a EntryView, key: &str) -> Option<&'a str> {
    entry
        .payload
        .as_ref()
        .and_then(|payload| payload.get(key))
        .and_then(Value::as_str)
}

fn all_files_exclude(root: &std::path::Path, needles: &[&str]) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            all_files_exclude(&entry.path(), needles);
        } else {
            let text = String::from_utf8_lossy(&std::fs::read(entry.path()).unwrap()).to_string();
            for needle in needles {
                assert!(
                    !text.contains(needle),
                    "`{needle}` leaked into {}",
                    entry.path().display(),
                );
            }
        }
    }
}

// -------------------------------------------------------------- the test

#[tokio::test(flavor = "multi_thread")]
async fn vault_brokered_credentials_over_real_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let support_store = tmp.path().join("support");
    let operations_store = tmp.path().join("operations");
    let journal_store = tmp.path().join("journal");
    let id_path = tmp.path().join("agent.id");
    let id = id_path.to_str().unwrap();

    // The vault holds both MCP tokens; nothing else ever will.
    let vault = FakeVault::new(VAULT_ROOT);
    vault.put("aithos/mcp/github", "token", GITHUB_SECRET);
    vault.put("aithos/mcp/linear", "token", LINEAR_SECRET);
    let vault_port = free_port();
    let vault_task = spawn_vault(vault_port, vault.clone()).await;

    let github = FakeMcp::new(
        vec![
            json!({
                "name": "issues.list",
                "description": "List issues",
                "inputSchema": { "type": "object", "additionalProperties": false }
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
        ],
        "github-ok",
    );
    let linear = FakeMcp::new(
        vec![json!({
            "name": "tickets.list",
            "description": "List tickets",
            "inputSchema": { "type": "object", "additionalProperties": false }
        })],
        "linear-ok",
    );
    let github_port = spawn_fake_mcp(github.clone()).await;
    let linear_port = spawn_fake_mcp(linear.clone()).await;
    let github_url = format!("http://127.0.0.1:{github_port}/mcp");
    let linear_url = format!("http://127.0.0.1:{linear_port}/mcp");

    // Birth + owner provisioning (discovery hits the MCPs, never the vault).
    let born = run_ok(&["--identity", id, "keygen"]);
    let agent_pub = line_value(&born, "agent_pub: ");
    let gateway_pub = line_value(&born, "gateway_pub: ");
    run_ok(&[
        "owner-init-journal",
        "--master-seed-hex",
        MASTER,
        "--agent-label",
        "agent-vault",
        "--agent-pub",
        &agent_pub,
        "--gateway-pub",
        &gateway_pub,
        "--store-root",
        journal_store.to_str().unwrap(),
    ]);
    let github_proposal = tmp.path().join("github.json");
    let linear_proposal = tmp.path().join("linear.json");
    run_ok(&[
        "owner-discover-server",
        "--server",
        "github",
        "--url",
        &github_url,
        "--output",
        github_proposal.to_str().unwrap(),
    ]);
    run_ok(&[
        "owner-discover-server",
        "--server",
        "linear",
        "--url",
        &linear_url,
        "--output",
        linear_proposal.to_str().unwrap(),
    ]);
    let mut auditor_seeds = BTreeMap::new();
    for (label, store, proposal, approvals) in [
        (
            "customer-support",
            &support_store,
            &github_proposal,
            ["issues.list=read", "issues.create=write"].as_slice(),
        ),
        (
            "operations",
            &operations_store,
            &linear_proposal,
            ["tickets.list=read"].as_slice(),
        ),
    ] {
        run_ok(&[
            "owner-init-context",
            "--master-seed-hex",
            MASTER,
            "--label",
            label,
            "--store-root",
            store.to_str().unwrap(),
        ]);
        let mut args = vec![
            "owner-enroll-server",
            "--master-seed-hex",
            MASTER,
            "--label",
            label,
            "--agent-pub",
            &agent_pub,
            "--gateway-pub",
            &gateway_pub,
            "--proposal",
            proposal.to_str().unwrap(),
        ];
        for approval in approvals {
            args.extend(["--approve", approval]);
        }
        args.extend(["--store-root", store.to_str().unwrap()]);
        let enrolled = run_ok(&args);
        auditor_seeds.insert(label, line_value(&enrolled, "auditor_seed_hex: "));
    }
    let vault_hits_before_run = vault.hit_count();
    assert_eq!(
        vault_hits_before_run, 0,
        "owner provisioning never touches the vault"
    );

    // The config carries references only — no token anywhere.
    let gw_port = free_port();
    let cfg_path = tmp.path().join("gateway.yaml");
    let cfg_text = format!(
        r#"listen: 127.0.0.1:{gw_port}
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:{vault_port}
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
servers:
  - name: github
    transport: http
    url: {github_url}
    credential:
      broker: enterprise
      path: aithos/mcp/github
      field: token
  - name: linear
    transport: http
    url: {linear_url}
    credential:
      broker: enterprise
      path: aithos/mcp/linear
      field: token
contexts:
  - name: customer-support
    store: {{ kind: fs, root: {} }}
    tools:
      github__issues_list: {{ server: github, tool: issues.list, access: read }}
      github__issues_create: {{ server: github, tool: issues.create, access: write }}
  - name: operations
    store: {{ kind: fs, root: {} }}
    tools:
      linear__tickets_list: {{ server: linear, tool: tickets.list, access: read }}
journal:
  store: {{ kind: fs, root: {} }}
"#,
        support_store.display(),
        operations_store.display(),
        journal_store.display(),
    );
    std::fs::write(&cfg_path, &cfg_text).unwrap();
    let cfg = cfg_path.to_str().unwrap();
    github.clear_observations();
    linear.clear_observations();

    // Run the gateway; the vault token reaches it ONLY as process env.
    let stderr_path = tmp.path().join("gateway.stderr");
    let child = gateway_bin()
        .args(["--config", cfg, "--identity", id, "run"])
        .env("AITHOS_VAULT_TOKEN", VAULT_ROOT)
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&stderr_path).unwrap())
        .spawn()
        .expect("vault run spawns");
    let child = ChildGuard(child);
    wait_until_listening(gw_port).await;

    // Session-open drift control reached both MCPs, authenticated by the
    // vault-resolved bearers, and consulted the vault for each server.
    let open_hits = vault.hit_count();
    assert!(open_hits >= 2, "session-open control resolves per server");
    assert!(!github.auth.lock().unwrap().is_empty());
    assert!(!linear.auth.lock().unwrap().is_empty());

    let client = reqwest::Client::new();
    let gateway_url = format!("http://127.0.0.1:{gw_port}/mcp");
    let post = |body: Value| {
        let client = client.clone();
        let gateway_url = gateway_url.clone();
        async move {
            client
                .post(gateway_url)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let call = |tool: &str, id: u64| {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": {} }
        })
    };

    // Agent tools/list: local pins only — zero vault, zero upstream delta.
    let github_calls_before = github.seen.lock().unwrap().len();
    let linear_calls_before = linear.seen.lock().unwrap().len();
    let listed = post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(&"github__issues_list"));
    assert!(names.contains(&"linear__tickets_list"));
    assert!(!names.contains(&"github__issues_create"));
    assert_eq!(vault.hit_count(), open_hits, "tools/list wakes no vault");
    assert_eq!(github.seen.lock().unwrap().len(), github_calls_before);
    assert_eq!(linear.seen.lock().unwrap().len(), linear_calls_before);

    // Covered calls: each upstream sees ITS vault bearer, raw tool names.
    assert_eq!(
        post(call("github__issues_list", 2)).await["result"]["content"][0]["text"],
        "github-ok"
    );
    assert_eq!(
        post(call("linear__tickets_list", 3)).await["result"]["content"][0]["text"],
        "linear-ok"
    );

    // The agent cannot choose the wire identity: header-shaped arguments
    // change nothing about the Authorization the upstream receives.
    let forged = post(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "github__issues_list", "arguments": {
            "Authorization": "Bearer agent-forged-token",
            "X-Vault-Token": "agent-forged-vault-token"
        } }
    }))
    .await;
    assert!(forged.get("error").is_none(), "the covered read passes");
    {
        let auths = github.auth.lock().unwrap();
        let calls: Vec<&Option<String>> = auths.iter().collect();
        assert!(
            calls
                .iter()
                .all(|auth| auth.as_deref() != Some("Bearer agent-forged-token")),
            "no forged bearer ever reaches the wire"
        );
        let call_auths: Vec<Option<String>> = github
            .seen
            .lock()
            .unwrap()
            .iter()
            .zip(auths.iter())
            .filter(|(body, _)| body["method"] == "tools/call")
            .map(|(_, auth)| auth.clone())
            .collect();
        assert_eq!(
            call_auths,
            vec![
                Some(format!("Bearer {GITHUB_SECRET}")),
                Some(format!("Bearer {GITHUB_SECRET}"))
            ],
            "both relays carry the vault bearer"
        );
        assert!(
            github
                .vault_header
                .lock()
                .unwrap()
                .iter()
                .all(Option::is_none),
            "the vault token never reaches an MCP server"
        );
    }
    assert!(linear
        .auth
        .lock()
        .unwrap()
        .iter()
        .all(|auth| auth.as_deref() == Some(format!("Bearer {LINEAR_SECRET}").as_str())));

    // A known but ungranted write: refused with zero vault and zero
    // upstream contact.
    let hits_before_refusal = vault.hit_count();
    let github_calls_before_refusal = github.call_count();
    let refused = post(call("github__issues_create", 5)).await;
    assert_eq!(refused["error"]["code"], -32001);
    assert_eq!(vault.hit_count(), hits_before_refusal);
    assert_eq!(github.call_count(), github_calls_before_refusal);

    // Vault outage: the route closes BEFORE any upstream contact and the
    // refusal is journaled under the stable credential code.
    vault_task.abort();
    wait_until_closed(vault_port).await;
    let github_calls_before_outage = github.call_count();
    let outage = post(call("github__issues_list", 6)).await;
    let message = outage["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("credential unavailable"),
        "the outage surfaces as a credential refusal: {outage}"
    );
    assert!(
        !message.contains(GITHUB_SECRET) && !message.contains(VAULT_ROOT),
        "the refusal is redacted"
    );
    assert_eq!(
        github.call_count(),
        github_calls_before_outage,
        "no call reaches the upstream without its credential"
    );

    // Restart the vault on the same port with a ROTATED github token:
    // the next call presents the new value — same YAML, same process.
    vault.put("aithos/mcp/github", "token", GITHUB_ROTATED);
    let _vault_task2 = spawn_vault(vault_port, vault.clone()).await;
    wait_until_listening(vault_port).await;
    assert_eq!(
        post(call("github__issues_list", 7)).await["result"]["content"][0]["text"],
        "github-ok"
    );
    assert_eq!(
        github.auth.lock().unwrap().last().cloned().flatten(),
        Some(format!("Bearer {GITHUB_ROTATED}")),
        "the rotation is live on the very next relay"
    );
    assert_eq!(
        std::fs::read_to_string(&cfg_path).unwrap(),
        cfg_text,
        "the config file never changed"
    );
    drop(child);

    // Every vault read carried the env-provided token, wire-side only.
    assert!(vault
        .tokens
        .lock()
        .unwrap()
        .iter()
        .all(|token| token.as_deref() == Some(VAULT_ROOT)));
    assert!(vault
        .hits
        .lock()
        .unwrap()
        .iter()
        .all(|path| path == "aithos/mcp/github" || path == "aithos/mcp/linear"));

    // Gammas: acts in the covering contexts, refusals precise, xrefs whole.
    let support_gamma = gamma(&support_store);
    let support_acts = acts_on(&support_gamma, "x.github");
    // Four intents: list, forged-args, the outage call (logged BEFORE
    // resolution — the refusal right after documents that it never
    // relayed) and the post-rotation call.
    assert_eq!(
        support_acts.len(),
        4,
        "list + forged-args + outage intent + post-rotation"
    );
    let support_refusals = acts_on(&support_gamma, "x.gateway");
    let refusal_reasons: Vec<Option<&str>> = support_refusals
        .iter()
        .map(|entry| payload_str(entry, "reason"))
        .collect();
    assert_eq!(
        refusal_reasons,
        [Some("mandate_denied"), Some("credential_unavailable")],
        "the write refusal then the outage refusal, in order"
    );
    assert_eq!(acts_on(&gamma(&operations_store), "x.linear").len(), 1);
    let journal_gamma = gamma(&journal_store);
    assert_eq!(acts_on(&journal_gamma, "x.xref").len(), 5);
    assert_eq!(acts_on(&journal_gamma, "x.gateway").len(), 2);

    // The audit slice still works per context.
    for (label, target) in [("customer-support", "x.github"), ("operations", "x.linear")] {
        let export = run_ok(&[
            "--config",
            cfg,
            "--identity",
            id,
            "audit-export",
            "--auditor-seed-hex",
            &auditor_seeds[label],
            "--context",
            label,
        ]);
        let export: Value = serde_json::from_str(&export).unwrap();
        assert!(export["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["target"] == target));
    }

    // The non-leak sweep: stores, config, proposals, identity, stderr —
    // none of the three secret values exists anywhere on disk.
    let needles = [GITHUB_SECRET, GITHUB_ROTATED, LINEAR_SECRET, VAULT_ROOT];
    all_files_exclude(tmp.path(), &needles);
    let stderr_text = std::fs::read_to_string(&stderr_path).unwrap();
    assert!(stderr_text.contains("gateway listening"));
}
