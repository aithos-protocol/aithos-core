//! Network e2e of the Léa demo (lot D) — the dress rehearsal on real
//! sockets, no LLM, CI-durable (no Docker). The REAL binary provisions
//! the whole Innoestate world in owner gestures: three permissive MCP
//! servers behind ONE endpoint, their full-power bearers only in a fake
//! HashiCorp Vault KV v2, one "ventes" Ethos whose single agent mandate
//! covers the granted tools of all three servers, the circle directive
//! and the owner-only self note. Then the eight beats of
//! docs/DEMO-LEA-SCENARIO.md §4 run over the wire — the same JSON-RPC
//! the real agent will send on demo day — including the hot owner edit
//! through the running gateway, the auditor replay, and the sentinel
//! sweep proving no secret survives anywhere on disk.

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

const MASTER: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const VAULT_ROOT: &str = "vault-root-sentinel-lea";
const NOTION_SECRET: &str = "notion-vault-sentinel-lea";
const GMAIL_SECRET: &str = "gmail-vault-sentinel-lea";
const CALENDAR_SECRET: &str = "calendar-vault-sentinel-lea";
const DIRECTIVE: &str = "Tout mail de prise de rendez-vous mentionne le DPE du bien et propose \
                         d'abord une visite virtuelle.";
const APPENDED: &str = "Joindre le lien du dossier de visite.";
const SELF_NOTE: &str = "Marge de negociation interne max 8% — owner only.";

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

    fn hits_on(&self, path: &str) -> usize {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|hit| hit.as_str() == path)
            .count()
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
// Deliberately permissive: they accept EVERY call that reaches them and
// record what the wire actually carried — whatever restriction the story
// shows is the gateway's alone.

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

    fn call_count(&self) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["method"] == "tools/call")
            .count()
    }

    fn call_bodies(&self) -> Vec<Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["method"] == "tools/call")
            .cloned()
            .collect()
    }

    fn call_auths(&self) -> Vec<Option<String>> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .zip(self.auth.lock().unwrap().iter())
            .filter(|(body, _)| body["method"] == "tools/call")
            .map(|(_, auth)| auth.clone())
            .collect()
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
                |State(fake): State<FakeMcp>,
                 headers: HeaderMap,
                 Json(body): Json<Value>| async move {
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
        .expect("fake mcp binds");
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
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
async fn demo_lea_dress_rehearsal_over_real_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let ventes_store = tmp.path().join("ventes");
    let journal_store = tmp.path().join("journal");
    let id_path = tmp.path().join("agent.id");
    let id = id_path.to_str().unwrap();
    let ventes = ventes_store.to_str().unwrap().to_owned();

    // The vault holds one FULL-POWER bearer per server; nothing else will.
    let vault = FakeVault::new(VAULT_ROOT);
    vault.put("aithos/mcp/notion", "token", NOTION_SECRET);
    vault.put("aithos/mcp/gmail", "token", GMAIL_SECRET);
    vault.put("aithos/mcp/calendar", "token", CALENDAR_SECRET);
    let vault_port = free_port();
    let _vault_task = spawn_vault(vault_port, vault.clone()).await;

    // Three separate, permissive upstreams — each its own endpoint.
    let notion = FakeMcp::new(
        vec![
            json!({
                "name": "query_database",
                "description": "Query one Notion database",
                "inputSchema": schema(json!({ "database": { "type": "string" } }), &[])
            }),
            json!({
                "name": "create_page",
                "description": "Create one Notion page",
                "inputSchema": schema(json!({ "title": { "type": "string" } }), &["title"])
            }),
        ],
        "prospects: a, b, c, d, e",
    );
    let gmail = FakeMcp::new(
        vec![
            json!({
                "name": "search_emails",
                "description": "Search the mailbox",
                "inputSchema": schema(json!({ "query": { "type": "string" } }), &[])
            }),
            json!({
                "name": "send_email",
                "description": "Send an email",
                "inputSchema": schema(
                    json!({
                        "to": { "type": "array", "items": { "type": "string" } },
                        "cc": { "type": "array", "items": { "type": "string" } },
                        "bcc": { "type": "array", "items": { "type": "string" } },
                        "subject": { "type": "string" },
                        "body": { "type": "string" }
                    }),
                    &["to"]
                )
            }),
            json!({
                "name": "delete_email",
                "description": "Delete one email",
                "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"])
            }),
        ],
        "email sent",
    );
    let calendar = FakeMcp::new(
        vec![
            json!({
                "name": "list_events",
                "description": "List calendar events",
                "inputSchema": schema(
                    json!({ "from": { "type": "string" }, "to": { "type": "string" } }),
                    &[]
                )
            }),
            json!({
                "name": "create_event",
                "description": "Create one calendar event",
                "inputSchema": schema(
                    json!({ "start": { "type": "string" }, "title": { "type": "string" } }),
                    &["start"]
                )
            }),
        ],
        "event booked",
    );
    let notion_port = spawn_fake_mcp(notion.clone()).await;
    let gmail_port = spawn_fake_mcp(gmail.clone()).await;
    let calendar_port = spawn_fake_mcp(calendar.clone()).await;

    // Owner provisioning, all through the real binary: birth, journal,
    // discovery of the three servers, ONE batch enrollment carrying the
    // whole distribution table (classes, decisions, bounds), the
    // briefing pen, the circle directive and the owner-only self note.
    let born = run_ok(&["--identity", id, "keygen"]);
    let agent_pub = line_value(&born, "agent_pub: ");
    let gateway_pub = line_value(&born, "gateway_pub: ");
    run_ok(&[
        "owner-init-journal",
        "--master-seed-hex",
        MASTER,
        "--agent-label",
        "lea",
        "--agent-pub",
        &agent_pub,
        "--gateway-pub",
        &gateway_pub,
        "--store-root",
        journal_store.to_str().unwrap(),
    ]);
    let mut proposals = Vec::new();
    for (server, port) in [
        ("notion", notion_port),
        ("gmail", gmail_port),
        ("calendar", calendar_port),
    ] {
        let path = tmp.path().join(format!("{server}.json"));
        run_ok(&[
            "owner-discover-server",
            "--server",
            server,
            "--url",
            &format!("http://127.0.0.1:{port}/mcp"),
            "--output",
            path.to_str().unwrap(),
        ]);
        proposals.push(path);
    }
    run_ok(&[
        "owner-init-context",
        "--master-seed-hex",
        MASTER,
        "--label",
        "ventes",
        "--store-root",
        &ventes,
    ]);
    let enrolled = run_ok(&[
        "owner-enroll-server",
        "--master-seed-hex",
        MASTER,
        "--label",
        "ventes",
        "--agent-pub",
        &agent_pub,
        "--gateway-pub",
        &gateway_pub,
        "--proposal",
        proposals[0].to_str().unwrap(),
        "--proposal",
        proposals[1].to_str().unwrap(),
        "--proposal",
        proposals[2].to_str().unwrap(),
        "--approve",
        "query_database=read:granted",
        "--approve",
        "create_page=write:denied",
        "--approve",
        "search_emails=read:granted",
        "--approve",
        "send_email=write:granted",
        "--approve",
        "delete_email=write:denied",
        "--approve",
        "list_events=read:granted",
        "--approve",
        "create_event=write:granted",
        "--bound",
        "send_email:to=one_of:a,b,c",
        "--bound",
        "send_email:bcc=forbid",
        "--bound",
        "send_email:to=max:3",
        "--bound",
        "send_email:subject=require",
        "--bound",
        "create_event:start=slots:tue,thu@14:00-18:00",
        "--store-root",
        &ventes,
    ]);
    let auditor_seed = line_value(&enrolled, "auditor_seed_hex: ");
    for server in ["notion", "gmail", "calendar"] {
        assert!(
            enrolled.contains(&format!("server: {server}")),
            "the batch names `{server}`"
        );
    }
    run_ok(&[
        "owner-grant-briefing",
        "--master-seed-hex",
        MASTER,
        "--label",
        "ventes",
        "--agent-pub",
        &agent_pub,
        "--store-root",
        &ventes,
    ]);
    run_ok(&[
        "owner-set-briefing",
        "--master-seed-hex",
        MASTER,
        "--label",
        "ventes",
        "--zone",
        "circle",
        "--title",
        "Consigne commerciale",
        "--text",
        DIRECTIVE,
        "--store-root",
        &ventes,
    ]);
    run_ok(&[
        "owner-set-briefing",
        "--master-seed-hex",
        MASTER,
        "--label",
        "ventes",
        "--zone",
        "self",
        "--title",
        "Note owner",
        "--text",
        SELF_NOTE,
        "--store-root",
        &ventes,
    ]);
    assert_eq!(
        vault.hit_count(),
        0,
        "owner provisioning never touches the vault"
    );

    // The runtime config: references only, one endpoint, seven refs.
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
  - name: notion
    transport: http
    url: http://127.0.0.1:{notion_port}/mcp
    credential: {{ broker: enterprise, path: aithos/mcp/notion, field: token }}
  - name: gmail
    transport: http
    url: http://127.0.0.1:{gmail_port}/mcp
    credential: {{ broker: enterprise, path: aithos/mcp/gmail, field: token }}
  - name: calendar
    transport: http
    url: http://127.0.0.1:{calendar_port}/mcp
    credential: {{ broker: enterprise, path: aithos/mcp/calendar, field: token }}
contexts:
  - name: ventes
    store: {{ kind: fs, root: {} }}
    tools:
      notion__query_database: {{ server: notion, tool: query_database, access: read, granted: true }}
      notion__create_page: {{ server: notion, tool: create_page, access: write, granted: false }}
      gmail__search_emails: {{ server: gmail, tool: search_emails, access: read, granted: true }}
      gmail__send_email: {{ server: gmail, tool: send_email, access: write, granted: true }}
      gmail__delete_email: {{ server: gmail, tool: delete_email, access: write, granted: false }}
      calendar__list_events: {{ server: calendar, tool: list_events, access: read, granted: true }}
      calendar__create_event: {{ server: calendar, tool: create_event, access: write, granted: true }}
journal:
  store: {{ kind: fs, root: {} }}
"#,
        ventes_store.display(),
        journal_store.display(),
    );
    std::fs::write(&cfg_path, &cfg_text).unwrap();
    let cfg = cfg_path.to_str().unwrap();

    let stderr_path = tmp.path().join("gateway.stderr");
    let child = gateway_bin()
        .args(["--config", cfg, "--identity", id, "run"])
        .env("AITHOS_VAULT_TOKEN", VAULT_ROOT)
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&stderr_path).unwrap())
        .spawn()
        .expect("demo run spawns");
    let child = ChildGuard(child);
    wait_until_listening(gw_port).await;
    let open_hits = vault.hit_count();

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
    let call = |tool: &str, args: Value, id: u64| {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": args }
        })
    };
    let briefing_text = |answer: &Value| {
        answer["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };

    // Beat 1 — the surface is exactly the granted, briefed world.
    let init = post(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" })).await;
    let instructions = init["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        instructions.contains("briefing.read") && instructions.contains("before"),
        "initialize recommends the briefing first: {init}"
    );
    let listed = post(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let mut names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "briefing.read",
            "calendar__create_event",
            "calendar__list_events",
            "gmail__search_emails",
            "gmail__send_email",
            "journal.search",
            "journal.write",
            "notion__query_database",
        ],
        "exposure = the mandate's coverage, nothing else"
    );
    assert_eq!(vault.hit_count(), open_hits, "the surface wakes no vault");
    assert_eq!(
        notion.call_count() + gmail.call_count() + calendar.call_count(),
        0
    );

    // Beat 2 — the data comes from notion under the read grant.
    let prospects = post(call("notion__query_database", json!({}), 3)).await;
    let text = briefing_text(&prospects);
    for prospect in ["a", "b", "c", "d", "e"] {
        assert!(text.contains(prospect), "prospect `{prospect}` served");
    }
    assert_eq!(
        notion.call_auths(),
        vec![Some(format!("Bearer {NOTION_SECRET}"))],
        "notion saw only its own vault bearer"
    );

    // Beat 3 — sending to everyone is refused and teaches the three.
    let gmail_vault_hits = vault.hits_on("aithos/mcp/gmail");
    let refused = post(call(
        "gmail__send_email",
        json!({
            "to": ["a", "b", "c", "d", "e"],
            "subject": "Prise de rendez-vous",
            "body": "Bonjour"
        }),
        4,
    ))
    .await;
    let message = refused["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("bound violated"),
        "a bound refusal: {refused}"
    );
    for needle in ["send_email.to", "d", "e", "approved set", "a, b, c"] {
        assert!(message.contains(needle), "the refusal teaches `{needle}`");
    }
    assert_eq!(
        vault.hits_on("aithos/mcp/gmail"),
        gmail_vault_hits,
        "the refused send wakes no gmail secret"
    );
    assert_eq!(
        gmail.call_count(),
        0,
        "the refused send never reaches gmail"
    );

    // Beat 4 — the corrected send passes, the wire carries the bearer.
    let sent = post(call(
        "gmail__send_email",
        json!({
            "to": ["a", "b", "c"],
            "subject": "Prise de rendez-vous — visite du bien",
            "body": "Bonjour, proposons un créneau."
        }),
        5,
    ))
    .await;
    assert!(
        sent.get("error").is_none(),
        "the corrected send passes: {sent}"
    );
    let gmail_calls = gmail.call_bodies();
    assert_eq!(gmail_calls.len(), 1, "gmail saw exactly one call");
    assert_eq!(
        gmail_calls[0]
            .pointer("/params/name")
            .and_then(Value::as_str),
        Some("send_email"),
        "the raw upstream name is restored on the wire"
    );
    assert_eq!(
        gmail.call_auths(),
        vec![Some(format!("Bearer {GMAIL_SECRET}"))],
        "gmail saw its own vault bearer"
    );

    // Beat 5 — the visit lands inside the approved slots only.
    let outside = post(call(
        "calendar__create_event",
        json!({ "start": "2026-07-15T10:00:00+02:00", "title": "Visite du bien" }),
        6,
    ))
    .await;
    let message = outside["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("bound violated")
            && message.contains("tuesday")
            && message.contains("thursday")
            && message.contains("14:00"),
        "the slots are named: {outside}"
    );
    assert_eq!(calendar.call_count(), 0, "the refused visit never lands");
    let inside = post(call(
        "calendar__create_event",
        json!({ "start": "2026-07-16T15:00:00+02:00", "title": "Visite du bien" }),
        7,
    ))
    .await;
    assert!(inside.get("error").is_none(), "the slotted visit passes");
    let calendar_calls = calendar.call_bodies();
    assert_eq!(calendar_calls.len(), 1, "calendar saw exactly one call");
    assert_eq!(
        calendar_calls[0]
            .pointer("/params/name")
            .and_then(Value::as_str),
        Some("create_event")
    );
    assert_eq!(
        calendar.call_auths(),
        vec![Some(format!("Bearer {CALENDAR_SECRET}"))],
        "calendar saw its own vault bearer"
    );

    // Beat 6 — the briefing steers before action, on the record.
    let briefed = post(call("briefing.read", json!({}), 8)).await;
    let text = briefing_text(&briefed);
    assert!(text.contains(DIRECTIVE), "the exact directive is served");
    assert!(
        !briefed.to_string().contains(SELF_NOTE),
        "the self zone never reaches the agent"
    );
    let reads_after_beat6 = gamma(&ventes_store)
        .iter()
        .filter(|entry| entry.kind == "ethos.read")
        .count();
    assert_eq!(reads_after_beat6, 1, "one journalized briefing read");

    // Beat 7 — the owner edits the circle directive THROUGH THE RUNNING
    // GATEWAY's store; the very next read serves the new text. No
    // restart, no redeploy — the character is governed, not baked in.
    run_ok(&[
        "owner-set-briefing",
        "--master-seed-hex",
        MASTER,
        "--label",
        "ventes",
        "--zone",
        "circle",
        "--title",
        "Consigne commerciale",
        "--text",
        &format!("{DIRECTIVE} {APPENDED}"),
        "--store-root",
        &ventes,
    ]);
    let rebriefed = post(call("briefing.read", json!({}), 9)).await;
    let text = briefing_text(&rebriefed);
    assert!(text.contains(APPENDED), "the hot edit is served: {text}");
    let reads_after_beat7 = gamma(&ventes_store)
        .iter()
        .filter(|entry| entry.kind == "ethos.read")
        .count();
    assert_eq!(reads_after_beat7, 2, "both reads are on the record");

    // Beat 8 — the auditor replays the whole story from the gamma.
    let acts_export = run_ok(&[
        "--config",
        cfg,
        "--identity",
        id,
        "audit-export",
        "--auditor-seed-hex",
        &auditor_seed,
        "--context",
        "ventes",
        "--kind",
        "action",
    ]);
    let reads_export = run_ok(&[
        "--config",
        cfg,
        "--identity",
        id,
        "audit-export",
        "--auditor-seed-hex",
        &auditor_seed,
        "--context",
        "ventes",
        "--kind",
        "ethos.read",
    ]);
    for target in ["x.notion", "x.gmail", "x.calendar"] {
        assert!(
            acts_export.contains(target),
            "the `{target}` act is replayed"
        );
    }
    assert!(acts_export.contains("bound_violated"));
    assert!(
        acts_export.contains("send_email.to") && acts_export.contains("create_event.start"),
        "the refusals replay their pedagogical detail"
    );
    let reads: Value = serde_json::from_str(&reads_export).unwrap();
    assert_eq!(
        reads["entries"].as_array().map(Vec::len),
        Some(2),
        "the two briefing reads are in the auditor's slice"
    );
    // A wider slice than the granted kinds stays refused outright.
    let widened = gateway_bin()
        .args([
            "--config",
            cfg,
            "--identity",
            id,
            "audit-export",
            "--auditor-seed-hex",
            &auditor_seed,
            "--context",
            "ventes",
            "--kind",
            "grant",
        ])
        .output()
        .expect("binary runs");
    assert!(!widened.status.success(), "out-of-scope audit must fail");

    // The gammas, counted: one act per connector, two pedagogical
    // refusals, two reads in ventes; three xrefs and two refusal mirrors
    // in the journal (§3bis.8).
    let ventes_gamma = gamma(&ventes_store);
    assert_eq!(acts_on(&ventes_gamma, "x.notion").len(), 1);
    assert_eq!(acts_on(&ventes_gamma, "x.gmail").len(), 1);
    assert_eq!(acts_on(&ventes_gamma, "x.calendar").len(), 1);
    let refusals = acts_on(&ventes_gamma, "x.gateway");
    assert_eq!(refusals.len(), 2, "the two bound refusals, nothing else");
    for refusal in &refusals {
        assert_eq!(payload_str(refusal, "reason"), Some("bound_violated"));
        assert!(
            payload_str(refusal, "detail").is_some(),
            "the pedagogical detail is on the record"
        );
    }
    let journal_gamma = gamma(&journal_store);
    assert_eq!(acts_on(&journal_gamma, "x.xref").len(), 3);
    assert_eq!(acts_on(&journal_gamma, "x.gateway").len(), 2);

    drop(child);

    // Every vault read carried the env token; only the three MCP paths.
    assert!(vault
        .tokens
        .lock()
        .unwrap()
        .iter()
        .all(|token| token.as_deref() == Some(VAULT_ROOT)));
    assert!(vault.hits.lock().unwrap().iter().all(|path| {
        [
            "aithos/mcp/notion",
            "aithos/mcp/gmail",
            "aithos/mcp/calendar",
        ]
        .contains(&path.as_str())
    }));

    // The sentinel sweep: no secret — and no clear self note — survives
    // in any store, config, proposal, identity or stderr file.
    let needles = [
        NOTION_SECRET,
        GMAIL_SECRET,
        CALENDAR_SECRET,
        VAULT_ROOT,
        SELF_NOTE,
    ];
    all_files_exclude(tmp.path(), &needles);
    assert!(
        !cfg_text.contains(NOTION_SECRET)
            && !cfg_text.contains(GMAIL_SECRET)
            && !cfg_text.contains(CALENDAR_SECRET)
            && !cfg_text.contains(VAULT_ROOT),
        "the config text carries references only"
    );
    let stderr_text = std::fs::read_to_string(&stderr_path).unwrap();
    assert!(stderr_text.contains("gateway listening"));
}
