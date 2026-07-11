//! Network-level end-to-end test: the REAL binary (`run`), a REAL fake
//! MCP upstream over HTTP, REAL TCP sockets — the full MVP journey as a
//! customer would run it locally.
//!
//! Journey: onboard (real binary) → run (real binary, child process) →
//! JSON-RPC tools/call over the wire (read relayed, write refused,
//! unknown refused) → audit-export (real binary) shows exactly one act
//! and two refusals. The cucumber suite covers the same logic at library
//! level; this covers the transport and the process surfaces.

use std::io::Read;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

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

/// Minimal fake MCP server on a real socket: records tools/call bodies,
/// answers a canned result.
async fn spawn_fake_mcp(seen: Arc<Mutex<Vec<Value>>>) -> u16 {
    use axum::{extract::State, routing::post, Json, Router};
    let port = free_port();
    let app = Router::new()
        .route(
            "/mcp",
            post(
                |State(seen): State<Arc<Mutex<Vec<Value>>>>, Json(body): Json<Value>| async move {
                    seen.lock().unwrap().push(body.clone());
                    let id = body.get("id").cloned().unwrap_or(Value::Null);
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": "alice@example.com" }],
                            "isError": false
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

#[tokio::test(flavor = "multi_thread")]
async fn full_journey_over_real_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let upstream_port = spawn_fake_mcp(seen.clone()).await;
    let gw_port = free_port();

    let cfg_path = tmp.path().join("gateway.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            "listen: 127.0.0.1:{gw_port}\nupstream_mcp: http://127.0.0.1:{upstream_port}/mcp\nstore:\n  kind: fs\n  root: {}\ntools:\n  user.read: read\n  user.update: write\n",
            tmp.path().join("ethos").display()
        ),
    )
    .unwrap();
    let cfg = cfg_path.to_str().unwrap();
    let id_path = tmp.path().join("agent.id");
    let id = id_path.to_str().unwrap();

    // 1. Onboard with the real binary; capture the auditor seed.
    let out = gateway_bin()
        .args(["--config", cfg, "--identity", id, "onboard"])
        .output()
        .expect("onboard runs");
    assert!(
        out.status.success(),
        "onboard failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let auditor_seed = stdout
        .lines()
        .find_map(|l| l.strip_prefix("auditor_seed_hex: "))
        .expect("auditor seed printed")
        .to_owned();

    // 2. Run the real gateway as a child process.
    let child = gateway_bin()
        .args(["--config", cfg, "--identity", id, "run"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run spawns");
    let mut child = ChildGuard(child);
    wait_until_listening(gw_port).await;

    // 3. The agent's calls, over the wire.
    let client = reqwest::Client::new();
    let call = |tool: &str, id: u64| {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": { "q": "who is alice" } }
        })
    };
    let url = format!("http://127.0.0.1:{gw_port}/mcp");

    // Read: relayed, answer comes back through.
    let ok: Value = client
        .post(&url)
        .json(&call("user.read", 1))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        ok.pointer("/result/content/0/text").and_then(Value::as_str),
        Some("alice@example.com")
    );

    // Write: refused, never relayed.
    let denied: Value = client
        .post(&url)
        .json(&call("user.update", 2))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        denied.pointer("/error/code").and_then(Value::as_i64),
        Some(-32001)
    );

    // Unknown tool: default deny.
    let unknown: Value = client
        .post(&url)
        .json(&call("user.delete", 3))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        unknown.pointer("/error/code").and_then(Value::as_i64),
        Some(-32001)
    );

    // Exactly one call crossed to the company MCP.
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "only the covered read may cross");
        assert_eq!(
            seen[0].pointer("/params/name").and_then(Value::as_str),
            Some("user.read")
        );
    }

    // 4. Stop the gateway, then audit with the real binary.
    let _ = child.0.kill();
    let _ = child.0.wait();

    let export = gateway_bin()
        .args([
            "--config",
            cfg,
            "--identity",
            id,
            "audit-export",
            "--auditor-seed-hex",
            &auditor_seed,
        ])
        .output()
        .expect("audit-export runs");
    assert!(
        export.status.success(),
        "audit-export failed: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let export: Value = serde_json::from_slice(&export.stdout).expect("valid JSON export");
    let entries = export["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 3, "one act + two refusals");
    let acts: Vec<&str> = entries
        .iter()
        .filter_map(|e| e["target"].as_str())
        .collect();
    assert_eq!(acts.iter().filter(|t| **t == "x.mcp").count(), 1);
    assert_eq!(acts.iter().filter(|t| **t == "x.gateway").count(), 2);

    // The child's stderr should show the listening banner, never a seed.
    let mut stderr = String::new();
    if let Some(mut e) = child.0.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    assert!(
        !stderr.contains("seed"),
        "no seed material on the run console"
    );
}
