//! Replay of `vectors/p6-acme-txt.json` against the REAL
//! `aithos-store-api` binary — child process, real TCP socket, the
//! committed header bytes and the committed instants (the
//! `vectors_replay.rs` pattern, applied to the B.5 surface).
//!
//! The cases are a STATEFUL sequence per plane: nonces burn across
//! cases, the rolling-hour PUT budget fills and frees, DELETE retires
//! what an earlier PUT posed. The normal plane replays in committed
//! order against one process; each suspended plane (binding, tenant)
//! replays against its own process whose bootstrap carries the flag.
//! The DNS backend is `memory` — no AWS anywhere near this test; the
//! wire verdicts ARE the assertion (the DNS effects are asserted by the
//! BDD suite against the inspectable memory backend).

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");

fn load(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{VECTORS}/{name}")).unwrap())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

// ------------------------------------------------------------- harness

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

fn wait_until_listening(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "aithos-store-api never started listening"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The store bootstrap of one plane: the p1 tenant fixture + the p6
/// tunnel mappings, with the plane's suspension flag.
fn bootstrap_for(p1: &Value, p6: &Value, plane: &str) -> Value {
    let mut tunnels = Vec::new();
    for (i, mapping) in p6["control_plane_mappings"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        tunnels.push(serde_json::json!({
            "gateway_pub": mapping["gateway_pub"],
            "tenant": mapping["tenant"],
            "hostname": mapping["hostname"],
            "suspended": plane == "suspended_binding" && i == 0,
        }));
    }
    serde_json::json!({
        "tenants": [{
            "tenant": p1["tenant"],
            "suspended": plane == "suspended_tenant",
            "dids": [{"did": p1["did"], "did_json": p1["did_json_jcs"]}],
        }],
        "tunnels": tunnels,
    })
}

fn spawn_store(bootstrap: &Value, tmp: &tempfile::TempDir, tag: &str) -> (ChildGuard, u16) {
    let bootstrap_path = tmp.path().join(format!("bootstrap-{tag}.json"));
    std::fs::write(&bootstrap_path, bootstrap.to_string()).unwrap();
    let port = free_port();
    let child = Command::new(assert_cmd::cargo::cargo_bin("aithos-store-api"))
        .env("AITHOS_STORE_LISTEN", format!("127.0.0.1:{port}"))
        .env("AITHOS_STORE_AUTHORITY", "store.aithos.fr")
        .env("AITHOS_STORE_BOOTSTRAP", &bootstrap_path)
        .env("AITHOS_STORE_NONCE_BACKEND", "memory")
        .env("AITHOS_STORE_DNS_BACKEND", "memory")
        .env("AITHOS_STORE_TEST_NOW", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("binary spawns");
    let guard = ChildGuard(child);
    wait_until_listening(port);
    (guard, port)
}

struct Reply {
    status: u16,
    body: Value,
}

/// Minimal HTTP/1.1 exchange with full control of every header byte.
fn exchange(
    port: u16,
    method: &str,
    target: &str,
    auth: Option<&str>,
    test_now: &str,
    body: &[u8],
) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut request = format!("{method} {target} HTTP/1.1\r\nhost: store.aithos.fr\r\n");
    if let Some(auth) = auth {
        request.push_str(&format!("x-aithos-auth: {auth}\r\n"));
    }
    request.push_str(&format!("x-aithos-test-now: {test_now}\r\n"));
    request.push_str(&format!(
        "content-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    ));
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8_lossy(&raw);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("unparsable response: {text}"));
    let body_text = text.split("\r\n\r\n").nth(1).unwrap_or("");
    let body = serde_json::from_str(body_text).unwrap_or(Value::Null);
    Reply { status, body }
}

fn send_case(port: u16, case: &Value) -> Reply {
    exchange(
        port,
        case["method"].as_str().unwrap(),
        "/acme/txt",
        case["x_aithos_auth"].as_str(),
        case["server_now"].as_str().unwrap(),
        case["request_body_utf8"].as_str().unwrap().as_bytes(),
    )
}

fn assert_case(reply: &Reply, case: &Value) {
    let name = case["name"].as_str().unwrap();
    let want_status = case["expect"]["status"].as_u64().unwrap() as u16;
    assert_eq!(
        reply.status, want_status,
        "{name}: status ({:?})",
        reply.body
    );
    if let Some(code) = case["expect"]["error"].as_str() {
        assert_eq!(reply.body["error"].as_str(), Some(code), "{name}: code");
    } else {
        // 204: no body, nothing but the effect (asserted by the BDD).
        assert_eq!(reply.body, Value::Null, "{name}: acceptance has no body");
    }
}

// ----------------------------------------------------- byte-exact mode

/// The normal plane: every committed case in committed order against ONE
/// process — the sequence IS the fixture (nonces, rate window, DNS
/// state).
#[test]
fn p6_normal_plane_replays_byte_exact_against_the_real_binary() {
    let p1 = load("p1-store-envelope.json");
    let p6 = load("p6-acme-txt.json");
    let tmp = tempfile::tempdir().unwrap();
    let (_guard, port) = spawn_store(&bootstrap_for(&p1, &p6, "normal"), &tmp, "normal");

    let mut replayed = 0;
    for case in p6["cases"].as_array().unwrap() {
        if case["plane"] != "normal" {
            continue;
        }
        let reply = send_case(port, case);
        assert_case(&reply, case);
        replayed += 1;
    }
    assert!(replayed >= 30, "the committed sequence replays whole");
}

/// Each suspension plane runs against its own bootstrap — the same
/// committed header bytes, the control plane flag is the only variable.
#[test]
fn p6_suspension_planes_replay_byte_exact() {
    let p1 = load("p1-store-envelope.json");
    let p6 = load("p6-acme-txt.json");
    let tmp = tempfile::tempdir().unwrap();
    for plane in ["suspended_binding", "suspended_tenant"] {
        let (_guard, port) = spawn_store(&bootstrap_for(&p1, &p6, plane), &tmp, plane);
        let mut replayed = 0;
        for case in p6["cases"].as_array().unwrap() {
            if case["plane"] != plane {
                continue;
            }
            let reply = send_case(port, case);
            assert_case(&reply, case);
            replayed += 1;
        }
        assert_eq!(replayed, 1, "{plane}: exactly the committed case");
    }
}
