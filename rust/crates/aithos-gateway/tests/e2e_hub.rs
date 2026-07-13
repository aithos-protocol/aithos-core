//! Network-level end-to-end test of the governed MCP hub (Phase H4).
//! The real binary opens owner-approved pins, controls two real HTTP MCP
//! servers (one shared by two Ethos), serves the aggregated pinned surface,
//! routes raw tool names under the right mandate, and refuses a drifted
//! server before a restarted session begins listening.

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

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[derive(Clone)]
struct FakeMcp {
    tools: Arc<Mutex<Vec<Value>>>,
    seen: Arc<Mutex<Vec<Value>>>,
    auth: Arc<Mutex<Vec<Option<String>>>>,
    answer: &'static str,
}

impl FakeMcp {
    fn new(tools: Vec<Value>, answer: &'static str) -> Self {
        Self {
            tools: Arc::new(Mutex::new(tools)),
            seen: Arc::default(),
            auth: Arc::default(),
            answer,
        }
    }

    fn clear_observations(&self) {
        self.seen.lock().unwrap().clear();
        self.auth.lock().unwrap().clear();
    }
}

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

async fn spawn_fake_mcp(fake: FakeMcp) -> u16 {
    use axum::{extract::State, routing::post, Json, Router};

    let port = free_port();
    let app = Router::new()
        .route(
            "/mcp",
            post(
                |State(fake): State<FakeMcp>, headers: HeaderMap, Json(body): Json<Value>| async move {
                    fake.seen.lock().unwrap().push(body.clone());
                    fake.auth.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned),
                    );
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

fn all_files_exclude(root: &std::path::Path, needle: &str) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            all_files_exclude(&entry.path(), needle);
        } else {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(
                !String::from_utf8_lossy(&bytes).contains(needle),
                "credential leaked into {}",
                entry.path().display(),
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn governed_hub_journey_over_real_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let support_store = tmp.path().join("support");
    let engineering_store = tmp.path().join("engineering");
    let operations_store = tmp.path().join("operations");
    let journal_store = tmp.path().join("journal");
    let id_path = tmp.path().join("agent.id");
    let id = id_path.to_str().unwrap();

    let github = FakeMcp::new(
        vec![
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

    let born = run_ok(&["--identity", id, "keygen"]);
    let agent_pub = line_value(&born, "agent_pub: ");
    let gateway_pub = line_value(&born, "gateway_pub: ");
    run_ok(&[
        "owner-init-journal",
        "--master-seed-hex",
        MASTER,
        "--agent-label",
        "agent-hub",
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

    let contexts = [
        (
            "customer-support",
            &support_store,
            &github_proposal,
            [
                "issues.list=read",
                "pulls.list=write",
                "issues.create=write",
            ]
            .as_slice(),
        ),
        (
            "engineering",
            &engineering_store,
            &github_proposal,
            [
                "issues.list=write",
                "pulls.list=read",
                "issues.create=write",
            ]
            .as_slice(),
        ),
        (
            "operations",
            &operations_store,
            &linear_proposal,
            ["tickets.list=read"].as_slice(),
        ),
    ];
    let mut auditor_seeds = BTreeMap::new();
    for (label, store, proposal, approvals) in contexts {
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

    let gw_port = free_port();
    let cfg_path = tmp.path().join("gateway.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"listen: 127.0.0.1:{gw_port}
servers:
  - name: github
    transport: http
    url: {github_url}
    bearer_token: github-runtime-secret
  - name: linear
    transport: http
    url: {linear_url}
    bearer_token: linear-runtime-secret
contexts:
  - name: customer-support
    store: {{ kind: fs, root: {} }}
    tools:
      github__issues_list: {{ server: github, tool: issues.list, access: read }}
      github__issues_create: {{ server: github, tool: issues.create, access: write }}
  - name: engineering
    store: {{ kind: fs, root: {} }}
    tools:
      github__pulls_list: {{ server: github, tool: pulls.list, access: read }}
  - name: operations
    store: {{ kind: fs, root: {} }}
    tools:
      linear__tickets_list: {{ server: linear, tool: tickets.list, access: read }}
journal:
  store: {{ kind: fs, root: {} }}
"#,
            support_store.display(),
            engineering_store.display(),
            operations_store.display(),
            journal_store.display(),
        ),
    )
    .unwrap();
    let cfg = cfg_path.to_str().unwrap();
    github.clear_observations();
    linear.clear_observations();

    let child = gateway_bin()
        .args(["--config", cfg, "--identity", id, "run"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("hub run spawns");
    let child = ChildGuard(child);
    wait_until_listening(gw_port).await;

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

    let listed = post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "github__issues_list",
            "github__pulls_list",
            "linear__tickets_list",
            "journal.write",
            "journal.search"
        ]
    );
    assert!(!names.contains(&"github__issues_create"));

    assert_eq!(
        post(call("github__issues_list", 2)).await["result"]["content"][0]["text"],
        "github-ok"
    );
    assert_eq!(
        post(call("github__pulls_list", 3)).await["result"]["content"][0]["text"],
        "github-ok"
    );
    assert_eq!(
        post(call("linear__tickets_list", 4)).await["result"]["content"][0]["text"],
        "linear-ok"
    );
    assert_eq!(
        post(call("github__issues_create", 5)).await["error"]["code"],
        -32001
    );

    let github_raw: Vec<String> = github
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body["method"] == "tools/call")
        .filter_map(|body| body.pointer("/params/name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    assert_eq!(github_raw, ["issues.list", "pulls.list"]);
    assert!(github
        .auth
        .lock()
        .unwrap()
        .iter()
        .all(|auth| auth.as_deref() == Some("Bearer github-runtime-secret")));
    assert!(linear
        .auth
        .lock()
        .unwrap()
        .iter()
        .all(|auth| auth.as_deref() == Some("Bearer linear-runtime-secret")));
    drop(child);

    let support_acts = acts_on(&gamma(&support_store), "x.github");
    let engineering_acts = acts_on(&gamma(&engineering_store), "x.github");
    let operations_acts = acts_on(&gamma(&operations_store), "x.linear");
    assert_eq!(support_acts.len(), 1);
    assert_eq!(
        payload_str(&support_acts[0], "tool"),
        Some("github__issues_list")
    );
    assert_eq!(engineering_acts.len(), 1);
    assert_eq!(
        payload_str(&engineering_acts[0], "tool"),
        Some("github__pulls_list")
    );
    assert_eq!(operations_acts.len(), 1);
    assert_eq!(
        payload_str(&operations_acts[0], "tool"),
        Some("linear__tickets_list")
    );
    assert_eq!(acts_on(&gamma(&journal_store), "x.xref").len(), 3);
    assert_eq!(acts_on(&gamma(&support_store), "x.gateway").len(), 1);

    for (label, target) in [
        ("customer-support", "x.github"),
        ("engineering", "x.github"),
        ("operations", "x.linear"),
    ] {
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
    all_files_exclude(&support_store, "github-runtime-secret");
    all_files_exclude(&engineering_store, "github-runtime-secret");
    all_files_exclude(&operations_store, "linear-runtime-secret");
    all_files_exclude(&journal_store, "runtime-secret");

    github.tools.lock().unwrap()[0]["description"] =
        Value::String("POISONED after enrollment".to_owned());
    github.clear_observations();
    let drifted = gateway_bin()
        .args(["--config", cfg, "--identity", id, "run"])
        .output()
        .expect("drifted run exits");
    assert!(!drifted.status.success());
    assert!(String::from_utf8_lossy(&drifted.stderr).contains("manifest drift"));
    assert!(github
        .seen
        .lock()
        .unwrap()
        .iter()
        .all(|body| body["method"] == "tools/list"));
}
