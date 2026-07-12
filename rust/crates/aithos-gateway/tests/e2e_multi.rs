//! Network-level end-to-end test of the PROVISIONED, multi-context
//! runtime (Phase B): the real binary all the way, two real fake MCP
//! upstreams on real sockets, and the full v2 journey as an enterprise
//! would run it —
//!
//! keygen (identity born in the runner, only pubkeys leave) → owner
//! provisioning with the real binary (journal + two context Ethos +
//! grants towards the published pubkeys) → `run` on the multi-context
//! config → JSON-RPC over the wire: each read routed to ITS context's
//! upstream, a write refused precisely, an unknown tool default-denied →
//! the gammas tell the decided story (act in the covering context, xref
//! mirror in the journal, refusals routed per §3bis.8) → audit-export
//! scoped per context. The cucumber suite contracts the same behaviours
//! at library level; this covers the transport and process surfaces.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use aithos_gateway::config::StoreConfig;
use aithos_gateway::core_bridge::{gamma_view, EntryView};
use aithos_gateway::store_adapter::GatewayStore;

/// A dev master seed (32 bytes hex) for the enterprise side.
const MASTER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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

/// Minimal fake MCP server on a real socket: records tools/call bodies,
/// answers a text unique to this upstream (so routing is observable on
/// the wire, not only in the logs).
async fn spawn_fake_mcp(answer: &'static str, seen: Arc<Mutex<Vec<Value>>>) -> u16 {
    use axum::{extract::State, routing::post, Json, Router};
    let port = free_port();
    let app = Router::new()
        .route(
            "/mcp",
            post(
                move |State(seen): State<Arc<Mutex<Vec<Value>>>>,
                      Json(body): Json<Value>| async move {
                    seen.lock().unwrap().push(body.clone());
                    let id = body.get("id").cloned().unwrap_or(Value::Null);
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": answer }],
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

/// Owner-side view of one fs-backed gamma.
fn gamma(root: &std::path::Path) -> Vec<EntryView> {
    gamma_view(GatewayStore::from_config(&StoreConfig::Fs { root: root.into() }).expect("fs store"))
        .expect("gamma readable")
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

#[tokio::test(flavor = "multi_thread")]
async fn provisioned_two_context_journey_over_real_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let brand_store = tmp.path().join("brand");
    let figma_store = tmp.path().join("figma");
    let journal_store = tmp.path().join("journal");
    let id_path = tmp.path().join("agent.id");
    let id = id_path.to_str().unwrap();

    // 1. Birth: the identity is born in the runner; only pubkeys leave.
    let born = run_ok(&["--identity", id, "keygen"]);
    let agent_pub = line_value(&born, "agent_pub: ");
    let gateway_pub = line_value(&born, "gateway_pub: ");

    // 2. Owner provisioning, real binary: the journal, then each context
    //    Ethos granted towards the published pubkeys. The owner keeps
    //    each context's auditor seed.
    run_ok(&[
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
    ]);
    let mut auditor_seeds = std::collections::BTreeMap::new();
    for (label, store, read) in [
        ("company-brand", &brand_store, "brand.read"),
        ("ui-designer", &figma_store, "figma.read"),
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
        let granted = run_ok(&[
            "owner-grant-context",
            "--master-seed-hex",
            MASTER,
            "--label",
            label,
            "--agent-pub",
            &agent_pub,
            "--gateway-pub",
            &gateway_pub,
            "--read",
            read,
            "--store-root",
            store.to_str().unwrap(),
        ]);
        auditor_seeds.insert(label, line_value(&granted, "auditor_seed_hex: "));
    }

    // 3. Two distinct fake MCP upstreams on real sockets.
    let seen_brand = Arc::new(Mutex::new(Vec::new()));
    let seen_figma = Arc::new(Mutex::new(Vec::new()));
    let brand_port = spawn_fake_mcp("brand-guidelines-v3", seen_brand.clone()).await;
    let figma_port = spawn_fake_mcp("figma-frame-42", seen_figma.clone()).await;
    let gw_port = free_port();

    // 4. The v2 config: two contexts (each with its own store, upstream
    //    and tool map) and the journal. `brand.update` is declared write:
    //    known, never granted — so its refusal names it precisely.
    let cfg_path = tmp.path().join("gateway.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            "listen: 127.0.0.1:{gw_port}\n\
             contexts:\n\
             \x20 - name: company-brand\n\
             \x20   upstream_mcp: http://127.0.0.1:{brand_port}/mcp\n\
             \x20   store: {{ kind: fs, root: {} }}\n\
             \x20   tools:\n\
             \x20     brand.read: read\n\
             \x20     brand.update: write\n\
             \x20 - name: ui-designer\n\
             \x20   upstream_mcp: http://127.0.0.1:{figma_port}/mcp\n\
             \x20   store: {{ kind: fs, root: {} }}\n\
             \x20   tools:\n\
             \x20     figma.read: read\n\
             journal:\n\
             \x20 store: {{ kind: fs, root: {} }}\n",
            brand_store.display(),
            figma_store.display(),
            journal_store.display(),
        ),
    )
    .unwrap();
    let cfg = cfg_path.to_str().unwrap();

    // 5. Run the real gateway as a child process.
    let child = gateway_bin()
        .args(["--config", cfg, "--identity", id, "run"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("run spawns");
    let _child = ChildGuard(child);
    wait_until_listening(gw_port).await;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{gw_port}/mcp");
    let post = |body: Value| {
        let client = client.clone();
        let url = url.clone();
        async move {
            client
                .post(&url)
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
            "params": { "name": tool, "arguments": { "q": "hello" } }
        })
    };

    // 6. The aggregated tools/list names every mapped tool (write too:
    //    refusals must name tools precisely), served by the router.
    let listed = post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools listed")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "brand.read",
            "brand.update",
            "figma.read",
            "journal.write",
            "journal.search"
        ],
        "context tools first, then the native journal tools (lot C2)"
    );

    // 7. Each read is routed to ITS context's upstream — observable on
    //    the wire by each upstream's unique answer.
    let brand = post(call("brand.read", 2)).await;
    assert_eq!(
        brand
            .pointer("/result/content/0/text")
            .and_then(Value::as_str),
        Some("brand-guidelines-v3")
    );
    let figma = post(call("figma.read", 3)).await;
    assert_eq!(
        figma
            .pointer("/result/content/0/text")
            .and_then(Value::as_str),
        Some("figma-frame-42")
    );

    // 8. A write is refused (known, never granted); an unknown tool is
    //    default-denied. Neither reaches ANY upstream.
    for (tool, id) in [("brand.update", 4), ("admin.export", 5)] {
        let denied = post(call(tool, id)).await;
        assert_eq!(
            denied.pointer("/error/code").and_then(Value::as_i64),
            Some(-32001),
            "`{tool}` must be refused"
        );
    }
    {
        let brand_seen = seen_brand.lock().unwrap();
        let figma_seen = seen_figma.lock().unwrap();
        assert_eq!(
            brand_seen.len(),
            1,
            "exactly the covered brand read crossed"
        );
        assert_eq!(
            brand_seen[0]
                .pointer("/params/name")
                .and_then(Value::as_str),
            Some("brand.read")
        );
        assert_eq!(
            figma_seen.len(),
            1,
            "exactly the covered figma read crossed"
        );
        assert_eq!(
            figma_seen[0]
                .pointer("/params/name")
                .and_then(Value::as_str),
            Some("figma.read")
        );
    }

    // 9. Stop the gateway; the story now lives in the files alone.
    drop(_child);

    // Context gammas: the act landed in the covering context ONLY, and
    // the refused write reached the context it aimed at (§3bis.8).
    let brand_gamma = gamma(&brand_store);
    let figma_gamma = gamma(&figma_store);
    let brand_acts = acts_on(&brand_gamma, "x.mcp");
    let figma_acts = acts_on(&figma_gamma, "x.mcp");
    assert_eq!(brand_acts.len(), 1);
    assert_eq!(payload_str(&brand_acts[0], "tool"), Some("brand.read"));
    assert_eq!(figma_acts.len(), 1);
    assert_eq!(payload_str(&figma_acts[0], "tool"), Some("figma.read"));

    let brand_refusals = acts_on(&brand_gamma, "x.gateway");
    assert_eq!(
        brand_refusals.len(),
        1,
        "the aimed write refusal is on record"
    );
    assert_eq!(
        payload_str(&brand_refusals[0], "tool"),
        Some("brand.update")
    );
    assert_eq!(
        payload_str(&brand_refusals[0], "reason"),
        Some("mandate_denied")
    );
    assert!(
        acts_on(&figma_gamma, "x.gateway").is_empty(),
        "no refusal aimed at ui-designer"
    );

    // The journal: one xref per act, joinable both ways, and EVERY
    // refusal (the unknown tool lands here only — no context to blame).
    let journal_gamma = gamma(&journal_store);
    assert!(acts_on(&journal_gamma, "x.mcp").is_empty(), "no act copies");
    let xrefs = acts_on(&journal_gamma, "x.xref");
    assert_eq!(xrefs.len(), 2, "one cross-reference per act");
    for (acts, store) in [(&brand_acts, &brand_store), (&figma_acts, &figma_store)] {
        let did = did_of(store);
        let joined: Vec<_> = xrefs
            .iter()
            .filter(|x| {
                payload_str(x, "ethos_did") == Some(did.as_str())
                    && payload_str(x, "entry_id") == Some(acts[0].id.as_str())
            })
            .collect();
        assert_eq!(joined.len(), 1, "exactly one xref joins the act in {did}");
        assert_eq!(
            payload_str(joined[0], "tool"),
            payload_str(&acts[0], "tool"),
            "the xref names the same tool as the act"
        );
    }
    let journal_refusals = acts_on(&journal_gamma, "x.gateway");
    let refused: Vec<_> = journal_refusals
        .iter()
        .filter_map(|e| payload_str(e, "tool"))
        .collect();
    assert_eq!(
        refused,
        vec!["brand.update", "admin.export"],
        "journal holds ALL refusals"
    );
    let unknown = journal_refusals
        .iter()
        .find(|e| payload_str(e, "tool") == Some("admin.export"))
        .unwrap();
    assert_eq!(payload_str(unknown, "reason"), Some("tool_not_mapped"));

    // 10. Audit-export, real binary, scoped per context: each auditor
    //     seed opens exactly ITS context's action slice.
    let brand_export = run_ok(&[
        "--config",
        cfg,
        "--identity",
        id,
        "audit-export",
        "--auditor-seed-hex",
        &auditor_seeds["company-brand"],
        "--context",
        "company-brand",
    ]);
    let export: Value = serde_json::from_str(&brand_export).expect("valid JSON export");
    let targets: Vec<&str> = export["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|e| e["target"].as_str())
        .collect();
    assert_eq!(targets.iter().filter(|t| **t == "x.mcp").count(), 1);
    assert_eq!(targets.iter().filter(|t| **t == "x.gateway").count(), 1);

    let figma_export = run_ok(&[
        "--config",
        cfg,
        "--identity",
        id,
        "audit-export",
        "--auditor-seed-hex",
        &auditor_seeds["ui-designer"],
        "--context",
        "ui-designer",
    ]);
    let export: Value = serde_json::from_str(&figma_export).expect("valid JSON export");
    assert_eq!(
        export["entries"].as_array().map(Vec::len),
        Some(1),
        "one act, no refusal, in the ui-designer slice"
    );

    // The certificate half still gates the query on the routed path: a
    // wider slice than `read.gamma#kind=action` is refused outright.
    let widened = gateway_bin()
        .args([
            "--config",
            cfg,
            "--identity",
            id,
            "audit-export",
            "--auditor-seed-hex",
            &auditor_seeds["company-brand"],
            "--context",
            "company-brand",
            "--kind",
            "grant",
        ])
        .output()
        .expect("binary runs");
    assert!(!widened.status.success(), "out-of-scope audit must fail");
    assert!(
        String::from_utf8_lossy(&widened.stderr).contains("audit read denied"),
        "the refusal names the denial"
    );

    // The multi shape fails closed on export targeting: an untargeted
    // export and an unknown context are both refused, naming the issue.
    let untargeted = gateway_bin()
        .args([
            "--config",
            cfg,
            "--identity",
            id,
            "audit-export",
            "--auditor-seed-hex",
            &auditor_seeds["company-brand"],
        ])
        .output()
        .expect("binary runs");
    assert!(!untargeted.status.success());
    assert!(
        String::from_utf8_lossy(&untargeted.stderr).contains("--context"),
        "the refusal names the missing flag"
    );
    let unknown_ctx = gateway_bin()
        .args([
            "--config",
            cfg,
            "--identity",
            id,
            "audit-export",
            "--auditor-seed-hex",
            &auditor_seeds["company-brand"],
            "--context",
            "marketing",
        ])
        .output()
        .expect("binary runs");
    assert!(!unknown_ctx.status.success());
    assert!(
        String::from_utf8_lossy(&unknown_ctx.stderr).contains("unknown context"),
        "the refusal names the unknown context"
    );
}

/// The DID anchoring an fs-backed ethos (the xref join key).
fn did_of(root: &std::path::Path) -> String {
    let did: Value =
        serde_json::from_slice(&std::fs::read(root.join("did.json")).unwrap()).unwrap();
    did["id"].as_str().expect("did id").to_owned()
}
