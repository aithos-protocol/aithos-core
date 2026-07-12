//! Network-level end-to-end test of the LLM front (Phase C): the real
//! binary all the way, a fake OpenAI-compatible provider on a real
//! socket, and the metered-inference journey as an enterprise would run
//! it —
//!
//! keygen → owner provisioning with `--token-budget` (the budgeted
//! inference pen minted towards the agent key) → `run` on the
//! multi-context config with `llm:` → the agent asks for a completion
//! over the wire: the credential is applied WIRE-SIDE only (the bearer
//! the provider sees, never anything agent-side), the model is imposed
//! over the agent's choice, and the journal meters ONE `inference`
//! entry from the provider's REAL usage — never the prompt → a second
//! call finds the budget spent and is refused BEFORE the provider is
//! touched. The cucumber suite contracts the same behaviours at library
//! level; this covers the transport and process surfaces.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use aithos_gateway::config::StoreConfig;
use aithos_gateway::core_bridge::{gamma_view, EntryView};
use aithos_gateway::store_adapter::GatewayStore;

/// A dev master seed (32 bytes hex) for the enterprise side.
const MASTER: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
/// The provider credential in the config — it must surface ONLY as the
/// bearer on the provider wire, never agent-side, never in any file of
/// the journal story.
const API_KEY: &str = "sk-e2e-provider-secret";
/// The imposed model — whatever the agent asks for is overwritten.
const IMPOSED: &str = "gpt-enterprise-pinned";
/// A distinctive prompt: it must reach the provider and NEVER any gamma.
const PROMPT: &str = "the-quarterly-numbers-are-confidential-7413";

/// Kills the `run` child even when an assert panics.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
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

/// Run one owner-side (or keygen) command of the real binary, expect
/// success, hand back stdout.
fn run_ok(args: &[&str]) -> String {
    let out = gateway_bin().args(args).output().expect("binary runs");
    assert!(
        out.status.success(),
        "`{}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn line_value(stdout: &str, prefix: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("`{prefix}` not printed"))
        .to_owned()
}

/// One captured provider call: the Authorization header exactly as seen
/// on the wire, and the model the request body carried.
#[derive(Clone)]
struct SeenCall {
    bearer: String,
    model: String,
}

/// Minimal fake OpenAI-compatible provider on a real socket: records the
/// bearer and the model of every call, answers a completion with a fixed
/// REAL-shaped usage (400 in + 300 out — exactly the 700 budget).
async fn spawn_fake_provider(seen: Arc<Mutex<Vec<SeenCall>>>) -> u16 {
    use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
    let port = free_port();
    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(
                move |State(seen): State<Arc<Mutex<Vec<SeenCall>>>>,
                      headers: HeaderMap,
                      Json(body): Json<Value>| async move {
                    seen.lock().unwrap().push(SeenCall {
                        bearer: headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or_default()
                            .to_owned(),
                        model: body
                            .get("model")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    });
                    Json(json!({
                        "id": "chatcmpl-e2e",
                        "object": "chat.completion",
                        "model": body.get("model").cloned().unwrap_or(Value::Null),
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": "fake-completion-42"
                            },
                            "finish_reason": "stop"
                        }],
                        "usage": {
                            "prompt_tokens": 400,
                            "completion_tokens": 300,
                            "total_tokens": 700
                        }
                    }))
                },
            ),
        )
        .with_state(seen);
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

/// Owner-side view of one fs-backed gamma.
fn gamma(root: &std::path::Path) -> Vec<EntryView> {
    gamma_view(GatewayStore::from_config(&StoreConfig::Fs { root: root.into() }).expect("fs store"))
        .expect("gamma readable")
}

/// A string field of an entry's clear payload.
fn payload_str<'a>(e: &'a EntryView, key: &str) -> Option<&'a str> {
    e.payload
        .as_ref()
        .and_then(|p| p.get(key))
        .and_then(Value::as_str)
}

/// Every byte under a store root, lossily stringified — the no-leak net.
fn read_all_files(dir: &std::path::Path) -> String {
    let mut all = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            for e in std::fs::read_dir(&p).unwrap() {
                stack.push(e.unwrap().path());
            }
        } else if let Ok(bytes) = std::fs::read(&p) {
            all.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    all
}

#[tokio::test(flavor = "multi_thread")]
async fn metered_inference_journey_over_real_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let brand_store = tmp.path().join("brand");
    let journal_store = tmp.path().join("journal");
    let id_path = tmp.path().join("agent.id");
    let id = id_path.to_str().unwrap();

    // 1. Birth, then owner provisioning WITH the token budget: the
    //    budgeted inference pen is minted towards the agent key (a
    //    separate mandate — the xref pen stays budget-free on purpose).
    let born = run_ok(&["--identity", id, "keygen"]);
    let agent_pub = line_value(&born, "agent_pub: ");
    let gateway_pub = line_value(&born, "gateway_pub: ");
    let equipped = run_ok(&[
        "owner-init-journal",
        "--master-seed-hex",
        MASTER,
        "--agent-label",
        "agent-7",
        "--agent-pub",
        &agent_pub,
        "--gateway-pub",
        &gateway_pub,
        "--store-root",
        journal_store.to_str().unwrap(),
        "--token-budget",
        "700",
    ]);
    line_value(&equipped, "inference_mandate: ");

    // The multi shape needs at least one context; its MCP upstream is
    // never touched by this journey (nothing listens on port 9).
    run_ok(&[
        "owner-init-context",
        "--master-seed-hex",
        MASTER,
        "--label",
        "company-brand",
        "--store-root",
        brand_store.to_str().unwrap(),
    ]);
    run_ok(&[
        "owner-grant-context",
        "--master-seed-hex",
        MASTER,
        "--label",
        "company-brand",
        "--agent-pub",
        &agent_pub,
        "--gateway-pub",
        &gateway_pub,
        "--read",
        "brand.read",
        "--store-root",
        brand_store.to_str().unwrap(),
    ]);

    // 2. The fake provider on a real socket, and the v2 config with the
    //    `llm:` front: credential + imposed model, gateway custody only.
    let seen = Arc::new(Mutex::new(Vec::new()));
    let provider_port = spawn_fake_provider(seen.clone()).await;
    let gw_port = free_port();
    let cfg_path = tmp.path().join("gateway.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            "listen: 127.0.0.1:{gw_port}\n\
             contexts:\n\
             \x20 - name: company-brand\n\
             \x20   upstream_mcp: http://127.0.0.1:9/mcp\n\
             \x20   store: {{ kind: fs, root: {} }}\n\
             \x20   tools:\n\
             \x20     brand.read: read\n\
             journal:\n\
             \x20 store: {{ kind: fs, root: {} }}\n\
             llm:\n\
             \x20 upstream: http://127.0.0.1:{provider_port}/v1/chat/completions\n\
             \x20 api_key: {API_KEY}\n\
             \x20 model: {IMPOSED}\n",
            brand_store.display(),
            journal_store.display(),
        ),
    )
    .unwrap();

    // 3. Run the real gateway as a child process.
    let child = gateway_bin()
        .args([
            "--config",
            cfg_path.to_str().unwrap(),
            "--identity",
            id,
            "run",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("run spawns");
    let _child = ChildGuard(child);
    wait_until_listening(gw_port).await;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{gw_port}/v1/chat/completions");
    let ask = json!({
        "model": "gpt-agent-picked",
        "messages": [{ "role": "user", "content": PROMPT }]
    });

    // 4. First completion: relayed under the imposed model, credential
    //    applied on the provider wire, answer handed back to the agent.
    let first = client.post(&url).json(&ask).send().await.unwrap();
    assert_eq!(first.status(), reqwest::StatusCode::OK);
    let body: Value = first.json().await.unwrap();
    assert_eq!(
        body.pointer("/choices/0/message/content")
            .and_then(Value::as_str),
        Some("fake-completion-42"),
        "the provider's answer reaches the agent"
    );
    {
        let s = seen.lock().unwrap();
        assert_eq!(s.len(), 1, "exactly one provider round-trip");
        assert_eq!(
            s[0].bearer,
            format!("Bearer {API_KEY}"),
            "the credential rides the provider wire — and only there"
        );
        assert_eq!(
            s[0].model, IMPOSED,
            "the agent's model choice is overwritten by the config"
        );
    }

    // 5. Second completion: the 700-token budget is exactly spent — the
    //    tap closes BEFORE the provider is touched.
    let second = client.post(&url).json(&ask).send().await.unwrap();
    assert_eq!(second.status(), reqwest::StatusCode::FORBIDDEN);
    let err: Value = second.json().await.unwrap();
    assert_eq!(
        err.pointer("/error/type").and_then(Value::as_str),
        Some("aithos_gateway_refusal")
    );
    assert!(
        err.pointer("/error/message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.contains("token budget exhausted")),
        "the refusal names the exhausted budget: {err}"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "the spent tap never reaches the provider"
    );

    // 6. Stop the gateway; the journal tells the story from the files
    //    alone: ONE metered inference (the provider's REAL usage, meta
    //    only), ONE refusal (journal only — no context is involved).
    drop(_child);
    let journal_gamma = gamma(&journal_store);
    let inferences: Vec<_> = journal_gamma
        .iter()
        .filter(|e| e.kind == "inference")
        .collect();
    assert_eq!(inferences.len(), 1, "one inference entry per completion");
    assert_eq!(inferences[0].target.as_deref(), Some("x.llm"));
    let meta = inferences[0].payload.as_ref().expect("clear meta payload");
    assert_eq!(meta["provider"], "openai-compat", "default provider tag");
    assert_eq!(meta["model"], IMPOSED);
    assert_eq!(meta["tokens_in"], 400, "the provider's REAL prompt tokens");
    assert_eq!(
        meta["tokens_out"], 300,
        "the provider's REAL completion tokens"
    );
    assert_eq!(
        meta["budget_ref"], "llm",
        "every metered entry cites the tap"
    );

    let refusals: Vec<_> = journal_gamma
        .iter()
        .filter(|e| e.kind == "action" && e.target.as_deref() == Some("x.gateway"))
        .collect();
    assert_eq!(refusals.len(), 1, "the budget refusal is on record");
    assert_eq!(payload_str(refusals[0], "tool"), Some("llm.chat"));
    assert_eq!(payload_str(refusals[0], "reason"), Some("mandate_denied"));

    // 7. Neither the prompt nor the credential ever touched the files —
    //    in the journal or in the context store.
    for store in [&journal_store, &brand_store] {
        let bytes = read_all_files(store);
        assert!(
            !bytes.contains(PROMPT),
            "the prompt must never land in {}",
            store.display()
        );
        assert!(
            !bytes.contains(API_KEY),
            "the credential must never land in {}",
            store.display()
        );
    }
}
