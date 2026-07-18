//! Replay of `vectors/p1-store-envelope.json` against the REAL
//! `aithos-store-api` binary — child process, real TCP socket (the
//! `e2e_http.rs` pattern of the gateway).
//!
//! Two modes, one contract:
//!
//! 1. **Byte-exact** (always runs): the four P1-gated cases of the lot —
//!    `accept_put_owner_root`, `reject_clock_skew_301s`,
//!    `reject_nonce_replayed`, `reject_signature_invalid` — are sent with
//!    the committed `x_aithos_auth` header bytes and the committed
//!    `server_now` as the injected test instant. The remaining p1 cases
//!    need the P2 chain machinery; until then the skeleton must stay
//!    FAIL-CLOSED on them: never a 2xx, never a 5xx — asserted here so a
//!    regression that silently accepts an unverified chain cannot land.
//!
//! 2. **Deployed** (opt-in): `AITHOS_REPLAY_URL=https://store.dev.… cargo
//!    test -p aithos-provider --test vectors_replay -- --nocapture`
//!    re-signs the same four case SEMANTICS against the live endpoint —
//!    fresh instants and nonces under the committed vector keys (the a1
//!    seed and `agent_sk_hex` ARE the fixture; no other key exists). The
//!    live service runs without any test clock: time-relative cases are
//!    re-anchored to the wall clock with the exact committed deltas
//!    (301 s), which is the durable way to replay frozen instants against
//!    a real clock. This is the P1 gate evidence.
//!
//! The HTTP client here is deliberately minimal (HTTP/1.1 over a
//! `TcpStream`): the byte-exact mode must control the `Host` header
//! (`store.aithos.fr`) while dialing 127.0.0.1 — no client library
//! ambiguity allowed between what is signed and what is sent.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use aithos_provider::envelope::{header_value, sign_envelope, Envelope, EnvelopeSignature};
use ed25519_dalek::SigningKey;
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

/// Spawn the real binary against the vector bootstrap: tenant, DID and
/// did.json come from the committed p1 file, nothing else.
fn spawn_store(p1: &Value, tmp: &tempfile::TempDir) -> (ChildGuard, u16) {
    let bootstrap = serde_json::json!({
        "tenants": [{
            "tenant": p1["tenant"],
            "suspended": false,
            "dids": [{"did": p1["did"], "did_json": p1["did_json_jcs"]}],
        }]
    });
    let bootstrap_path = tmp.path().join("bootstrap.json");
    std::fs::write(&bootstrap_path, bootstrap.to_string()).unwrap();

    let port = free_port();
    let child = Command::new(assert_cmd::cargo::cargo_bin("aithos-store-api"))
        .env("AITHOS_STORE_LISTEN", format!("127.0.0.1:{port}"))
        .env("AITHOS_STORE_AUTHORITY", "store.aithos.fr")
        .env("AITHOS_STORE_BOOTSTRAP", &bootstrap_path)
        .env("AITHOS_STORE_NONCE_BACKEND", "memory")
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
    host: &str,
    auth: Option<&str>,
    test_now: Option<&str>,
    body: &[u8],
) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut request = format!("{method} {target} HTTP/1.1\r\nhost: {host}\r\n");
    if let Some(auth) = auth {
        request.push_str(&format!("x-aithos-auth: {auth}\r\n"));
    }
    if let Some(now) = test_now {
        request.push_str(&format!("x-aithos-test-now: {now}\r\n"));
    }
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

fn case<'a>(p1: &'a Value, name: &str) -> &'a Value {
    p1["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("case {name} missing from p1"))
}

fn send_case(p1: &Value, port: u16, name: &str) -> Reply {
    let case = case(p1, name);
    exchange(
        port,
        case["envelope"]["method"].as_str().unwrap(),
        case["envelope"]["path"].as_str().unwrap(),
        "store.aithos.fr",
        Some(case["x_aithos_auth"].as_str().unwrap()),
        Some(case["server_now"].as_str().unwrap()),
        case["request_body_utf8"].as_str().unwrap().as_bytes(),
    )
}

fn assert_error(reply: &Reply, status: u16, code: &str, name: &str) {
    assert_eq!(reply.status, status, "{name}: status ({:?})", reply.body);
    assert_eq!(
        reply.body["error"].as_str(),
        Some(code),
        "{name}: registry code"
    );
}

// ----------------------------------------------------- byte-exact mode

#[test]
fn p1_cases_replay_byte_exact_against_the_real_binary() {
    let p1 = load("p1-store-envelope.json");
    let tmp = tempfile::tempdir().unwrap();
    let (_guard, port) = spawn_store(&p1, &tmp);

    // The four cases gated on P1 (HANDOFF-PROVIDER-AWS, lot P1) — the
    // committed header bytes, the committed instants.
    let accept = send_case(&p1, port, "accept_put_owner_root");
    assert!(
        (200..300).contains(&accept.status),
        "accept_put_owner_root: expected acceptance, got {} ({:?})",
        accept.status,
        accept.body
    );
    // The accepted artifact is really there: anonymous GET (A2 exception).
    let served = exchange(
        port,
        "GET",
        case(&p1, "accept_put_owner_root")["envelope"]["path"]
            .as_str()
            .unwrap(),
        "store.aithos.fr",
        None,
        None,
        b"",
    );
    assert_eq!(served.status, 200, "the hello serves back anonymously");

    assert_error(
        &send_case(&p1, port, "reject_clock_skew_301s"),
        401,
        "clock_skew",
        "reject_clock_skew_301s",
    );
    assert_error(
        &send_case(&p1, port, "reject_signature_invalid"),
        401,
        "signature_invalid",
        "reject_signature_invalid",
    );

    // The replay pair: the first presentation (accept_get_mandated's
    // envelope) is refused at #9 in P1 — chain machinery pending — but
    // its nonce burned at #6, BEFORE the refusal (annexe A.2: réservation
    // avant tout effet de bord). The second presentation must answer
    // nonce_replayed, byte-exact as committed.
    let first = send_case(&p1, port, "accept_get_mandated");
    assert_error(
        &first,
        403,
        "chain_invalid",
        "accept_get_mandated (P1 defer)",
    );
    assert_error(
        &send_case(&p1, port, "reject_nonce_replayed"),
        401,
        "nonce_replayed",
        "reject_nonce_replayed",
    );

    // Every case P2 will turn green must already be FAIL-CLOSED: refused
    // 4xx — never accepted, never a crash. The exact verdicts (window,
    // covers, revocation, leaf) arrive with the chain machinery.
    for name in [
        "accept_skew_boundary_300s",
        "reject_window_expired",
        "reject_not_covered",
        "reject_chain_revoked",
        "reject_key_leaf_mismatch",
    ] {
        let reply = send_case(&p1, port, name);
        assert!(
            (400..500).contains(&reply.status),
            "{name}: P1 must fail closed, got {} ({:?})",
            reply.status,
            reply.body
        );
        eprintln!("deferred to P2 (fail-closed now: {}): {name}", reply.status);
    }
}

// ------------------------------------------------------- deployed mode

fn owner_root_key() -> SigningKey {
    let a1 = load("a1-genesis.json");
    let seed: [u8; 32] = hex::decode(a1["seed_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    SigningKey::from_bytes(&aithos_core::derive::derive_key(
        aithos_core::derive::CTX_ROOT_SIGN,
        &seed,
    ))
}

fn now_zulu(offset_secs: i64) -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    aithos_provider::time::render_rfc3339z(ms + offset_secs * 1000)
}

#[allow(clippy::too_many_arguments)]
fn signed_header(
    key: &SigningKey,
    fragment: &str,
    host: &str,
    method: &str,
    path: &str,
    body: &[u8],
    at: &str,
    nonce: &str,
) -> String {
    let envelope = Envelope {
        v: 1,
        host: host.to_owned(),
        method: method.to_owned(),
        path: path.to_owned(),
        body_b3: if body.is_empty() {
            String::new()
        } else {
            blake3::hash(body).to_hex().to_string()
        },
        at: at.to_owned(),
        nonce: nonce.to_owned(),
        mandate: vec![],
        key: fragment.to_owned(),
        signature: EnvelopeSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    header_value(&sign_envelope(envelope, key).unwrap()).unwrap()
}

/// The P1 gate evidence: the four case semantics against the DEPLOYED dev
/// endpoint, re-signed fresh under the committed vector keys. Opt-in:
/// `AITHOS_REPLAY_URL=https://store.dev.aithos.fr`.
#[tokio::test]
async fn p1_semantics_replay_against_the_deployed_endpoint() {
    let Ok(base) = std::env::var("AITHOS_REPLAY_URL") else {
        eprintln!("AITHOS_REPLAY_URL unset: deployed replay skipped (local byte-exact ran)");
        return;
    };
    let base = base.trim_end_matches('/').to_owned();
    let authority = base
        .strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))
        .expect("AITHOS_REPLAY_URL is a URL")
        .to_ascii_lowercase();
    let p1 = load("p1-store-envelope.json");
    let tenant = p1["tenant"].as_str().unwrap();
    let did = p1["did"].as_str().unwrap();
    let root = owner_root_key();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let client = reqwest::Client::new();
    let path = format!("/t/{tenant}/{did}/e/public/hello.md");
    let url = format!("{base}{path}");
    let body = b"# hello\n";

    // 1. accept_put_owner_root, live: fresh instant, fresh nonce.
    let auth = signed_header(
        &root,
        "#root",
        &authority,
        "PUT",
        &path,
        body,
        &now_zulu(0),
        &format!("replay-{stamp}-put1"),
    );
    let reply = client
        .put(&url)
        .header("x-aithos-auth", auth)
        .body(body.to_vec())
        .send()
        .await
        .expect("deployed endpoint reachable");
    assert!(
        reply.status().is_success(),
        "hello PUT refused: {} {}",
        reply.status(),
        reply.text().await.unwrap_or_default()
    );

    // 2. reject_clock_skew_301s, live: the committed delta, wall-anchored.
    let auth = signed_header(
        &root,
        "#root",
        &authority,
        "PUT",
        &path,
        body,
        &now_zulu(-301),
        &format!("replay-{stamp}-skew"),
    );
    let reply = client
        .put(&url)
        .header("x-aithos-auth", auth)
        .body(body.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(reply.status().as_u16(), 401, "clock_skew status");
    assert_eq!(
        reply.json::<Value>().await.unwrap()["error"],
        "clock_skew",
        "clock_skew code"
    );

    // 3. reject_nonce_replayed, live: same (key, nonce) twice.
    let auth = signed_header(
        &root,
        "#root",
        &authority,
        "GET",
        &format!("/t/{tenant}/{did}/did.json"),
        b"",
        &now_zulu(0),
        &format!("replay-{stamp}-nonce"),
    );
    let get_url = format!("{base}/t/{tenant}/{did}/did.json");
    let first = client
        .get(&get_url)
        .header("x-aithos-auth", &auth)
        .send()
        .await
        .unwrap();
    assert!(first.status().is_success(), "first presentation accepted");
    let second = client
        .get(&get_url)
        .header("x-aithos-auth", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status().as_u16(), 401, "nonce_replayed status");
    assert_eq!(
        second.json::<Value>().await.unwrap()["error"],
        "nonce_replayed",
        "nonce_replayed code"
    );

    // 4. reject_signature_invalid, live: last byte flipped.
    let mut envelope: Envelope = {
        use base64::Engine as _;
        let auth = signed_header(
            &root,
            "#root",
            &authority,
            "GET",
            &format!("/t/{tenant}/{did}/did.json"),
            b"",
            &now_zulu(0),
            &format!("replay-{stamp}-sig"),
        );
        serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(auth)
                .unwrap(),
        )
        .unwrap()
    };
    let tail = if envelope.signature.value.ends_with("00") {
        "ff"
    } else {
        "00"
    };
    let len = envelope.signature.value.len();
    envelope.signature.value.truncate(len - 2);
    envelope.signature.value.push_str(tail);
    let reply = client
        .get(&get_url)
        .header("x-aithos-auth", header_value(&envelope).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(reply.status().as_u16(), 401, "signature_invalid status");
    assert_eq!(
        reply.json::<Value>().await.unwrap()["error"],
        "signature_invalid",
        "signature_invalid code"
    );

    eprintln!("deployed replay GREEN against {base} — P1 gate evidence");
}
