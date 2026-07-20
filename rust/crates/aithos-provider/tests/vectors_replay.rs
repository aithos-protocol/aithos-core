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

/// Spawn the real binary against an explicit bootstrap document.
fn spawn_store_with(bootstrap: &Value, tmp: &tempfile::TempDir) -> (ChildGuard, u16) {
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

/// Spawn the real binary against the vector bootstrap: tenant, DID and
/// did.json come from the committed p1 file, nothing else.
fn spawn_store(p1: &Value, tmp: &tempfile::TempDir) -> (ChildGuard, u16) {
    // The chain state the P2 checks read, straight from the committed
    // vector: the mandate cert (#7/#9) and the FULL gamma log at the
    // did.json's own `revocations` pointer. Seeding the post-revoke
    // superset is correct for every case: revocation is forward-only,
    // evaluated at each case's `server_now` (the revoke at 13:00Z bites
    // only the 13:05Z case). The covered blob makes the mandated accepts
    // servable.
    let mandate: Value = serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
    let did_doc: Value = serde_json::from_str(p1["did_json_jcs"].as_str().unwrap()).unwrap();
    let gamma_log = p1["gamma_states"]["post_revoke"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let bootstrap = serde_json::json!({
        "tenants": [{
            "tenant": p1["tenant"],
            "suspended": false,
            "dids": [{
                "did": p1["did"],
                "did_json": p1["did_json_jcs"],
                "objects": [
                    {"key": format!("certs/{}.json", mandate["id"].as_str().unwrap()),
                     "utf8": p1["mandate_jcs"]},
                    {"key": did_doc["revocations"], "utf8": gamma_log},
                    {"key": "e/circle/blobs/01000000000000000000000000.enc",
                     "utf8": "opaque-p1-circle-blob"},
                ],
            }],
        }]
    });
    spawn_store_with(&bootstrap, tmp)
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

    // The mandated accept + the replay pair (gate 3, P2): the chain
    // machinery is live — the first presentation is ACCEPTED byte-exact,
    // its nonce burned at #6; the second presentation still answers
    // nonce_replayed on the same committed bytes.
    let first = send_case(&p1, port, "accept_get_mandated");
    assert!(
        (200..300).contains(&first.status),
        "accept_get_mandated: expected acceptance, got {} ({:?})",
        first.status,
        first.body
    );
    assert_error(
        &send_case(&p1, port, "reject_nonce_replayed"),
        401,
        "nonce_replayed",
        "reject_nonce_replayed",
    );

    // The five P1-deferred cases, byte-exact at their committed verdicts
    // (gate 3): the exact window/covers/revocation/leaf refusals of the
    // frozen vector, under the committed header bytes and instants.
    let accept_skew = send_case(&p1, port, "accept_skew_boundary_300s");
    assert!(
        (200..300).contains(&accept_skew.status),
        "accept_skew_boundary_300s: expected acceptance, got {} ({:?})",
        accept_skew.status,
        accept_skew.body
    );
    assert_error(
        &send_case(&p1, port, "reject_window_expired"),
        403,
        "chain_invalid",
        "reject_window_expired",
    );
    assert_error(
        &send_case(&p1, port, "reject_not_covered"),
        403,
        "not_covered",
        "reject_not_covered",
    );
    assert_error(
        &send_case(&p1, port, "reject_chain_revoked"),
        403,
        "chain_revoked",
        "reject_chain_revoked",
    );
    assert_error(
        &send_case(&p1, port, "reject_key_leaf_mismatch"),
        403,
        "chain_invalid",
        "reject_key_leaf_mismatch",
    );
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

// ------------------------------------------- p2/p7 CAS + deposit replay
//
// The A.4/A.5 layer of the gate contrat P2: every p2 (frozen, lot P0) and
// p7 case is a PER-STATE contract — `state_heads`/`state_head` names the
// A.5 heads-table tuple the server holds, `state_objects` the stored
// artifacts the check needs. Each case therefore replays against a FRESH
// child seeded through the bootstrap (the gate-3 `objects` pattern plus
// the étape-4 `heads` seed). Layer rule (the vectors' own description):
// cases carry no envelope — the harness signs with the COMMITTED keys (a1
// seed → owner, p1 `agent_sk_hex` → grantee, cb2 grantee seed → delegate;
// no other key exists in the fixture). Assertions are byte-exact on the
// wire facts the vectors freeze: status, registry code, `head`/`height`
// on `cas_mismatch`, the closed `reason` on `artifact_invalid`, and the
// accepted `new_head`/`new_height`.

fn p1_agent_key(p1: &Value) -> SigningKey {
    let seed: [u8; 32] = hex::decode(p1["agent_sk_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    SigningKey::from_bytes(&seed)
}

fn cb2_seed(cb2: &Value, name: &str) -> SigningKey {
    let seed: [u8; 32] = hex::decode(
        cb2["deterministic_private_seed_hex"][name]
            .as_str()
            .unwrap(),
    )
    .unwrap()
    .try_into()
    .unwrap();
    SigningKey::from_bytes(&seed)
}

/// The cb2 subject's did.json, synthesized from the COMMITTED cb2 seeds
/// (did:key-style: the DID literal IS the root key). A control-plane
/// enrollment fixture — the vector's CAS layer never names it, A.2 #1/#7
/// presuppose it (« l'enrôlement P7 précède toujours »).
fn synth_cb2_did_json(cb2: &Value) -> String {
    let root = cb2_seed(cb2, "root");
    let owner = aithos_core::keys::OwnerKeys {
        content_sign: cb2_seed(cb2, "content"),
        owner_kex: aithos_core::keys::grantee_kex_secret(&root),
        root_sign: root,
    };
    let succession = SigningKey::from_bytes(&[0xbb; 32]);
    let subject = cb2["context"]["subject"].as_str().unwrap();
    let doc = aithos_core::did::DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec![format!("https://store.aithos.fr/t/acme/{subject}")],
        "gamma/gamma.jsonl".into(),
    )
    .unwrap();
    assert_eq!(doc.id, subject, "cb2 seed anchors the DID literal");
    serde_jcs::to_string(&doc).unwrap()
}

/// One case's bootstrap: the p1 binding + mandate cert (enrollment
/// fixtures), the case's `state_objects`, and the heads seed. `state` is
/// the normalized A.5 tuple (`{height?, manifest?, gamma?}` or null) —
/// p7 carries it verbatim (`state_heads`), p2's bare `state_head` string
/// is normalized by its own replay (the manifest height is recovered
/// from the frozen `manifests` table of the vector).
fn case_bootstrap(
    p1: &Value,
    case: &Value,
    cb2_did_json: Option<&str>,
    tenant: &str,
    state: &Value,
) -> Value {
    let mandate: Value = serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
    let heads = if state.is_null() {
        Value::Null
    } else {
        let mut heads = serde_json::Map::new();
        if let Some(height) = state.get("height") {
            heads.insert("height".into(), height.clone());
            heads.insert("manifest".into(), state["manifest"].clone());
        }
        if let Some(gamma) = state.get("gamma") {
            heads.insert("gamma".into(), gamma.clone());
        }
        Value::Object(heads)
    };
    let state_objects = case
        .get("state_objects")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let objects_of = |entries: Vec<(String, Value)>| {
        entries
            .into_iter()
            .map(|(key, utf8)| serde_json::json!({"key": key, "utf8": utf8}))
            .collect::<Vec<_>>()
    };
    let p1_cert = (
        format!("certs/{}.json", mandate["id"].as_str().unwrap()),
        p1["mandate_jcs"].clone(),
    );
    let mut dids = Vec::new();
    match (
        case.get("subject_did").and_then(Value::as_str),
        cb2_did_json,
    ) {
        (Some(subject), Some(did_json)) => {
            dids.push(serde_json::json!({
                "did": p1["did"], "did_json": p1["did_json_jcs"],
                "objects": objects_of(vec![p1_cert]),
            }));
            let mut cb2 = serde_json::json!({
                "did": subject, "did_json": did_json,
                "objects": objects_of(
                    state_objects.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            });
            if !heads.is_null() {
                cb2["heads"] = heads;
            }
            dids.push(cb2);
        }
        _ => {
            let mut entries = vec![p1_cert];
            entries.extend(state_objects.iter().map(|(k, v)| (k.clone(), v.clone())));
            let mut own = serde_json::json!({
                "did": p1["did"], "did_json": p1["did_json_jcs"],
                "objects": objects_of(entries),
            });
            if !heads.is_null() {
                own["heads"] = heads;
            }
            dids.push(own);
        }
    }
    serde_json::json!({"tenants": [{"tenant": tenant, "dids": dids}]})
}

struct CaseSigner<'a> {
    key: String,
    sk: &'a SigningKey,
    mandate: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn replay_case(
    p1: &Value,
    case: &Value,
    cb2_did_json: Option<&str>,
    tenant: &str,
    method: &str,
    path: &str,
    body_field: &str,
    at: &str,
    signer: &CaseSigner<'_>,
    nonce: &str,
) {
    let name = case["name"].as_str().unwrap();
    let body = case[body_field].as_str().unwrap().as_bytes().to_vec();
    let envelope = Envelope {
        v: 1,
        host: "store.aithos.fr".into(),
        method: method.to_owned(),
        path: path.to_owned(),
        body_b3: blake3::hash(&body).to_hex().to_string(),
        at: at.to_owned(),
        nonce: nonce.to_owned(),
        mandate: signer.mandate.clone(),
        key: signer.key.clone(),
        signature: EnvelopeSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let auth = header_value(&sign_envelope(envelope, signer.sk).unwrap()).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let state = case.get("state_heads").cloned().unwrap_or(Value::Null);
    let state = if state.is_null() {
        normalized_p2_state(case)
    } else {
        state
    };
    let bootstrap = case_bootstrap(p1, case, cb2_did_json, tenant, &state);
    let (_guard, port) = spawn_store_with(&bootstrap, &tmp);
    let reply = exchange_with_if_head(
        port,
        method,
        path,
        "store.aithos.fr",
        Some(&auth),
        Some(at),
        case.get("if_head").and_then(Value::as_str),
        &body,
    );

    let expect = &case["expect"];
    if expect["status"] == "accept" {
        assert!(
            (200..300).contains(&reply.status),
            "{name}: expected acceptance, got {} ({:?})",
            reply.status,
            reply.body
        );
        if let Some(head) = expect.get("new_head") {
            assert_eq!(&reply.body["head"], head, "{name}: accepted head");
        }
        if let Some(height) = expect.get("new_height") {
            assert_eq!(&reply.body["height"], height, "{name}: accepted height");
        }
    } else {
        assert_eq!(
            reply.status,
            expect["status"].as_u64().unwrap() as u16,
            "{name}: status ({:?})",
            reply.body
        );
        assert_eq!(
            reply.body["error"], expect["error"],
            "{name}: registry code"
        );
        for extra in ["head", "height", "reason"] {
            if let Some(want) = expect.get(extra) {
                assert_eq!(&reply.body[extra], want, "{name}: {extra}");
            }
        }
    }
}

/// `exchange` plus the A.5 `If-Head` header.
#[allow(clippy::too_many_arguments)]
fn exchange_with_if_head(
    port: u16,
    method: &str,
    target: &str,
    host: &str,
    auth: Option<&str>,
    test_now: Option<&str>,
    if_head: Option<&str>,
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
    if let Some(head) = if_head {
        request.push_str(&format!("if-head: {head}\r\n"));
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

/// p2's bare `state_head` string, normalized to the A.5 tuple: a gamma
/// case seeds `gamma`; a manifest case recovers its height from the
/// frozen `manifests` table (chain hash → edition height).
fn normalized_p2_state(case: &Value) -> Value {
    let Some(head) = case.get("state_head").and_then(Value::as_str) else {
        return Value::Null;
    };
    if case.get("entry_jcs").is_some() {
        return serde_json::json!({"gamma": head});
    }
    let p2 = load("p2-store-cas.json");
    let manifests = p2["manifests"].as_object().unwrap();
    for stem in ["m1", "m2", "m2b"] {
        if head
            == format!(
                "sha256:{}",
                manifests[&format!("{stem}_chain_hash")].as_str().unwrap()
            )
        {
            let manifest: Value =
                serde_json::from_str(manifests[&format!("{stem}_jcs")].as_str().unwrap()).unwrap();
            return serde_json::json!({
                "height": manifest["edition"]["height"],
                "manifest": head,
            });
        }
    }
    panic!("p2 state head {head} matches no frozen manifest");
}

#[test]
fn p2_cases_replay_wire_exact_against_the_real_binary() {
    let p1 = load("p1-store-envelope.json");
    let p2 = load("p2-store-cas.json");
    let tenant = p2["tenant"].as_str().unwrap();
    let did = p2["did"].as_str().unwrap();
    let root = owner_root_key();
    let mut nonce = 0;
    for case in p2["manifest_cases"].as_array().unwrap() {
        nonce += 1;
        let at = serde_json::from_str::<Value>(case["body_jcs"].as_str().unwrap()).unwrap()
            ["edition"]["created_at"]
            .as_str()
            .unwrap()
            .to_owned();
        replay_case(
            &p1,
            case,
            None,
            tenant,
            "PUT",
            &format!("/t/{tenant}/{did}/manifest.json"),
            "body_jcs",
            &at,
            &CaseSigner {
                key: "#root".into(),
                sk: &root,
                mandate: vec![],
            },
            &format!("p2-replay-m{nonce:04}"),
        );
    }
    for case in p2["gamma_cases"].as_array().unwrap() {
        nonce += 1;
        let at = serde_json::from_str::<Value>(case["entry_jcs"].as_str().unwrap()).unwrap()["at"]
            .as_str()
            .unwrap()
            .to_owned();
        replay_case(
            &p1,
            case,
            None,
            tenant,
            "POST",
            &format!("/t/{tenant}/{did}/gamma"),
            "entry_jcs",
            &at,
            &CaseSigner {
                key: "#root".into(),
                sk: &root,
                mandate: vec![],
            },
            &format!("p2-replay-g{nonce:04}"),
        );
    }
}

#[test]
fn p7_cases_replay_wire_exact_against_the_real_binary() {
    let p1 = load("p1-store-envelope.json");
    let p7 = load("p7-store-publication.json");
    let cb2 = load("cb2-draft2-carriers.json");
    let cb2_did_json = synth_cb2_did_json(&cb2);
    let tenant = p7["tenant"].as_str().unwrap();
    let did = p7["did"].as_str().unwrap();
    let root = owner_root_key();
    let agent = p1_agent_key(&p1);
    let agent_key = {
        let mandate: Value = serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
        mandate["grantee"]["pubkey"].as_str().unwrap().to_owned()
    };
    let mandate_id = {
        let mandate: Value = serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
        mandate["id"].as_str().unwrap().to_owned()
    };
    let cb2_grantee = cb2_seed(&cb2, "grantee");
    let mut nonce = 0;
    for case in p7["manifest_cases"].as_array().unwrap() {
        nonce += 1;
        let subject = case
            .get("subject_did")
            .and_then(Value::as_str)
            .unwrap_or(did);
        let at = serde_json::from_str::<Value>(case["body_jcs"].as_str().unwrap()).unwrap()
            ["edition"]["created_at"]
            .as_str()
            .unwrap()
            .to_owned();
        let signer = if case["signer"] == "cb2_grantee" {
            CaseSigner {
                key: cb2["context"]["grantee_key"].as_str().unwrap().into(),
                sk: &cb2_grantee,
                mandate: case["mandate_chain"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().to_owned())
                    .collect(),
            }
        } else {
            CaseSigner {
                key: "#root".into(),
                sk: &root,
                mandate: vec![],
            }
        };
        replay_case(
            &p1,
            case,
            Some(&cb2_did_json),
            tenant,
            "PUT",
            &format!("/t/{tenant}/{subject}/manifest.json"),
            "body_jcs",
            &at,
            &signer,
            &format!("p7-replay-m{nonce:04}"),
        );
    }
    for case in p7["cert_cases"].as_array().unwrap() {
        nonce += 1;
        replay_case(
            &p1,
            case,
            None,
            tenant,
            "PUT",
            &format!("/t/{tenant}/{did}/{}", case["path"].as_str().unwrap()),
            "body_jcs",
            "2026-07-19T12:00:00Z",
            &CaseSigner {
                key: "#root".into(),
                sk: &root,
                mandate: vec![],
            },
            &format!("p7-replay-c{nonce:04}"),
        );
    }
    for case in p7["gamma_cases"].as_array().unwrap() {
        nonce += 1;
        replay_case(
            &p1,
            case,
            None,
            tenant,
            "POST",
            &format!("/t/{tenant}/{did}/gamma"),
            "entry_jcs",
            "2026-07-19T12:00:00Z",
            &CaseSigner {
                key: agent_key.clone(),
                sk: &agent,
                mandate: vec![mandate_id.clone()],
            },
            &format!("p7-replay-g{nonce:04}"),
        );
    }
}

// ------------------------------------------------------- p9 wire replay

/// Raw exchange keeping the exact response bytes — the p9 steps assert
/// multipart packs and byte-exact object bodies, not only JSON.
#[allow(clippy::too_many_arguments)]
fn exchange_raw(
    port: u16,
    method: &str,
    target: &str,
    auth: Option<&str>,
    test_now: &str,
    if_head: Option<&str>,
    body: &[u8],
) -> (u16, String, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut request = format!("{method} {target} HTTP/1.1\r\nhost: store.aithos.fr\r\n");
    if let Some(auth) = auth {
        request.push_str(&format!("x-aithos-auth: {auth}\r\n"));
    }
    request.push_str(&format!("x-aithos-test-now: {test_now}\r\n"));
    if let Some(head) = if_head {
        request.push_str(&format!("if-head: {head}\r\n"));
    }
    request.push_str(&format!(
        "content-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    ));
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response head");
    let head_text = String::from_utf8_lossy(&raw[..split]).to_string();
    let status: u16 = head_text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("unparsable response: {head_text}"));
    (status, head_text, raw[split + 4..].to_vec())
}

/// The byte-exact multipart split of the p9 driver: delimiter is
/// `\r\n--boundary`, so part bodies keep their own trailing newlines.
fn split_multipart(head_text: &str, body: &[u8]) -> Vec<(String, u16, Vec<u8>)> {
    let boundary = head_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim().eq_ignore_ascii_case("content-type"))
                .then(|| {
                    value
                        .split_once("boundary=")
                        .map(|(_, b)| b.trim().to_owned())
                })
                .flatten()
        })
        .expect("a multipart content-type");
    let delim = format!("--{boundary}");
    assert!(body.starts_with(format!("{delim}\r\n").as_bytes()));
    let sep = format!("\r\n{delim}");
    let mut rest = &body[delim.len() + 2..];
    let mut parts = Vec::new();
    loop {
        let next = rest
            .windows(sep.len())
            .position(|w| w == sep.as_bytes())
            .expect("closing boundary");
        let chunk = &rest[..next];
        let header_end = chunk
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("part headers");
        let headers = String::from_utf8_lossy(&chunk[..header_end]).to_string();
        let mut location = String::new();
        let mut status = 0u16;
        for line in headers.lines() {
            if let Some((name, value)) = line.split_once(':') {
                match name.trim().to_ascii_lowercase().as_str() {
                    "content-location" => location = value.trim().to_owned(),
                    "x-aithos-status" => status = value.trim().parse().unwrap(),
                    _ => {}
                }
            }
        }
        parts.push((location, status, chunk[header_end + 4..].to_vec()));
        rest = &rest[next + sep.len()..];
        if rest.starts_with(b"--") {
            break;
        }
        rest = &rest[2..];
    }
    parts
}

#[test]
fn p9_cases_replay_wire_exact_against_the_real_binary() {
    let p9 = load("p9-store-reads.json");
    let bundle = load("p7-bundle-packages.json");
    let p1 = load("p1-store-envelope.json");
    let base_objects = bundle["packages"][p9["base_package"].as_str().unwrap()]["objects"]
        .as_object()
        .unwrap();
    let tenant = p9["tenant"].as_str().unwrap();
    let did = p9["did"].as_str().unwrap();
    let at = p9["at"].as_str().unwrap();
    let root = owner_root_key();
    let agent = p1_agent_key(&p1);
    let mandate: Value = serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
    let agent_key = mandate["grantee"]["pubkey"].as_str().unwrap().to_owned();
    let mandate_id = mandate["id"].as_str().unwrap().to_owned();
    let genesis = &p9["fixtures"]["genesis"];
    let genesis_seed: [u8; 32] = hex::decode(genesis["root_seed_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let genesis_sk = SigningKey::from_bytes(&genesis_seed);
    let mut nonce = 0u64;

    for case in p9["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let state = &case["state"];
        // The case's frozen object state (the p9 driver's bootstrap rule).
        let mut objects: std::collections::BTreeMap<String, String> = Default::default();
        if state["use_base_objects"] == Value::Bool(true) {
            for (key, value) in base_objects {
                objects.insert(key.clone(), value["utf8"].as_str().unwrap().to_owned());
            }
        }
        if let Some(extra) = state["extra_objects"].as_object() {
            for (key, value) in extra {
                objects.insert(key.clone(), value.as_str().unwrap().to_owned());
            }
        }
        if let Some(drop) = state["drop_objects"].as_array() {
            for key in drop {
                objects.remove(key.as_str().unwrap());
            }
        }
        let did_json = objects
            .remove("did.json")
            .unwrap_or_else(|| p1["did_json_jcs"].as_str().unwrap().to_owned());
        let mut entries = vec![serde_json::json!({
            "key": format!("certs/{mandate_id}.json"),
            "utf8": p1["mandate_jcs"],
        })];
        for (key, utf8) in &objects {
            entries.push(serde_json::json!({"key": key, "utf8": utf8}));
        }
        let mut own = serde_json::json!({
            "did": did, "did_json": did_json, "objects": entries,
        });
        if !state["heads"].is_null() {
            own["heads"] = state["heads"].clone();
        }
        let mut dids = vec![own];
        if let Some(bind) = state["bind_did"].as_str() {
            dids.push(serde_json::json!({"did": bind}));
        }
        let bootstrap = serde_json::json!({"tenants": [{"tenant": tenant, "dids": dids}]});

        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            nonce += 1;
            let step_did = match (case["group"].as_str(), state["bind_did"].as_str()) {
                (Some("did"), Some(bind)) => bind,
                _ => did,
            };
            let base = format!("/t/{tenant}/{step_did}");
            let rel = step["path_rel"].as_str().unwrap();
            let query = step["query"].as_str().unwrap_or("");
            let path = if rel.is_empty() {
                format!("{base}{query}")
            } else {
                format!("{base}/{rel}{query}")
            };
            let method = step["method"].as_str().unwrap();
            let body = step["body_utf8"].as_str().unwrap_or("").as_bytes().to_vec();
            let auth = match step["signer"].as_str().unwrap() {
                "anonymous" => None,
                signer => {
                    let (key, sk, chain): (String, &SigningKey, Vec<String>) = match signer {
                        "owner_root" | "genesis_foreign_doc" => ("#root".into(), &root, vec![]),
                        "grantee" => (agent_key.clone(), &agent, vec![mandate_id.clone()]),
                        "genesis_owner" => ("#root".into(), &genesis_sk, vec![]),
                        "genesis_wrong_signer" => ("#root".into(), &agent, vec![]),
                        other => panic!("unknown signer {other}"),
                    };
                    let envelope = Envelope {
                        v: 1,
                        host: "store.aithos.fr".into(),
                        method: method.to_owned(),
                        path: path.clone(),
                        body_b3: if body.is_empty() {
                            String::new()
                        } else {
                            blake3::hash(&body).to_hex().to_string()
                        },
                        at: at.to_owned(),
                        nonce: format!("p9-replay-{nonce:04}"),
                        mandate: chain,
                        key,
                        signature: EnvelopeSignature {
                            alg: "ed25519".into(),
                            value: String::new(),
                        },
                    };
                    Some(header_value(&sign_envelope(envelope, sk).unwrap()).unwrap())
                }
            };
            let tmp = tempfile::tempdir().unwrap();
            let (_guard, port) = spawn_store_with(&bootstrap, &tmp);
            let (status, head_text, raw_body) = exchange_raw(
                port,
                method,
                &path,
                auth.as_deref(),
                at,
                step["if_head"].as_str(),
                &body,
            );
            let label = format!("{name}#{}", index + 1);
            let expect = &step["expect"];
            if expect["status"] == "accept" {
                assert_eq!(
                    status,
                    expect["code"].as_u64().unwrap() as u16,
                    "{label}: accept status ({})",
                    String::from_utf8_lossy(&raw_body)
                );
                if let Some(want) = expect.get("json") {
                    let got: Value = serde_json::from_slice(&raw_body)
                        .unwrap_or_else(|_| panic!("{label}: a JSON accept body"));
                    assert_eq!(&got, want, "{label}: accept JSON body");
                }
                if let Some(want) = expect["body_utf8"].as_str() {
                    assert_eq!(raw_body, want.as_bytes(), "{label}: byte-exact object body");
                }
                if let Some(want_parts) = expect["parts"].as_array() {
                    let parts = split_multipart(&head_text, &raw_body);
                    assert_eq!(parts.len(), want_parts.len(), "{label}: part count");
                    for (got, want) in parts.iter().zip(want_parts) {
                        assert!(
                            got.0.ends_with(want["path"].as_str().unwrap()),
                            "{label}: part location {}",
                            got.0
                        );
                        assert_eq!(
                            got.1,
                            want["part_status"].as_u64().unwrap() as u16,
                            "{label}: part status for {}",
                            got.0
                        );
                        if let Some(body_want) = want["body_utf8"].as_str() {
                            assert_eq!(
                                got.2,
                                body_want.as_bytes(),
                                "{label}: part bytes for {}",
                                got.0
                            );
                        } else {
                            assert!(got.2.is_empty(), "{label}: no body on {}", got.0);
                        }
                    }
                }
            } else {
                assert_eq!(
                    status,
                    expect["status"].as_u64().unwrap() as u16,
                    "{label}: status ({})",
                    String::from_utf8_lossy(&raw_body)
                );
                let got: Value = serde_json::from_slice(&raw_body).unwrap_or(Value::Null);
                assert_eq!(got["error"], expect["error"], "{label}: registry code");
                for extra in ["reason", "head", "height"] {
                    if let Some(want) = expect.get(extra) {
                        assert_eq!(&got[extra], want, "{label}: {extra}");
                    }
                }
            }
        }
    }
}
