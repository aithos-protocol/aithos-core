//! CLI surface tests (decided 2026-07-10, EXECUTION-PLAN §4/§Sécurité de
//! surface): run the REAL binary against a disposable bundle and assert
//! stdout/stderr/exit codes. The Gherkin suite owns the protocol logic;
//! this file owns the surface — critical paths, plus the two invariants
//! the core cannot see:
//!
//!   1. the agent key is held by the CLI/container, never by the LLM —
//!      no secret ever appears in any output or certificate;
//!   2. the kind is imposed by the operation, never chosen by the caller.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

const OWNER: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const SUCCESSION: &str = "0909090909090909090909090909090909090909090909090909090909090909";
const AGENT: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const BODY: &str = "corps secret ultra-prive";

fn ac() -> Command {
    Command::cargo_bin("aithos-core").expect("binary builds")
}

fn init_bundle() -> TempDir {
    let dir = TempDir::new().unwrap();
    ac().args([
        "init",
        "--seed-hex",
        OWNER,
        "--succession-seed-hex",
        SUCCESSION,
        "--dir",
        dir.path().to_str().unwrap(),
    ])
    .assert()
    .success();
    dir
}

fn d(dir: &TempDir) -> String {
    dir.path().to_str().unwrap().to_owned()
}

/// Latest certificate file in the bundle.
fn last_cert(dir: &TempDir) -> String {
    let mut certs: Vec<_> = std::fs::read_dir(dir.path().join("certs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    certs.sort_by_key(|p| std::fs::metadata(p).unwrap().modified().unwrap());
    certs.last().unwrap().to_str().unwrap().to_owned()
}

/// Every gamma segment concatenated, as text.
fn gamma_raw(dir: &TempDir) -> String {
    let mut out = String::new();
    if let Ok(rd) = std::fs::read_dir(dir.path().join("gamma")) {
        for e in rd {
            out.push_str(&std::fs::read_to_string(e.unwrap().path()).unwrap());
        }
    }
    out
}

fn add_circle_section(dir: &TempDir) {
    ac().args([
        "section-add",
        "--dir",
        &d(dir),
        "--seed-hex",
        OWNER,
        "circle",
        "projets/note1",
        "--title",
        "note",
        "--tags",
        "toto",
        "--body",
        BODY,
    ])
    .assert()
    .success();
}

// ------------------------------------------------------- critical paths ---

#[test]
fn init_creates_a_verifiable_bundle() {
    let dir = init_bundle();
    assert!(dir.path().join("did.json").exists());
    assert!(dir.path().join("e/x/header.json").exists(), "vault root");
    ac().args(["edition-verify", "--dir", &d(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
    ac().args(["log-verify", "--dir", &d(&dir)])
        .assert()
        .success();
}

#[test]
fn a_circle_mutation_is_logged_sealed_and_reads_back() {
    let dir = init_bundle();
    add_circle_section(&dir);
    // Canonical kind, sealed target, and the body NEVER on the wire.
    ac().args(["log-show", "--dir", &d(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("section.add"))
        .stdout(predicate::str::contains("(sealed)"))
        .stdout(predicate::str::contains(BODY).not());
    assert!(
        !gamma_raw(&dir).contains(BODY),
        "the body must never reach the log in clear"
    );
    ac().args([
        "section-read",
        "--dir",
        &d(&dir),
        "circle",
        "projets/note1",
        "--seed-hex",
        OWNER,
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(BODY));
}

#[test]
fn editions_pin_the_log_and_tampering_fails_closed() {
    let dir = init_bundle();
    add_circle_section(&dir);
    ac().args(["edition-publish", "--dir", &d(&dir), "--seed-hex", OWNER])
        .assert()
        .success();
    ac().args(["edition-verify", "--dir", &d(&dir)])
        .assert()
        .success();
    // Flip one hex char inside a signature: everything must fail closed.
    let seg = std::fs::read_dir(dir.path().join("gamma"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = std::fs::read(&seg).unwrap();
    let i = bytes.windows(9).position(|w| w == b"\"value\":\"").unwrap() + 9;
    bytes[i] = if bytes[i] == b'0' { b'1' } else { b'0' };
    std::fs::write(&seg, bytes).unwrap();
    ac().args(["log-verify", "--dir", &d(&dir)])
        .assert()
        .failure();
    ac().args(["edition-verify", "--dir", &d(&dir)])
        .assert()
        .failure();
}

#[test]
fn a_mandate_gates_the_agent_read_and_expires() {
    let dir = init_bundle();
    add_circle_section(&dir);
    ac().args([
        "grant",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "projets",
        "--tag",
        "toto",
        "--ttl-days",
        "7",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    let now = {
        // The CLI's own clock format is RFC 3339 Z; "now" read back from
        // the freshly minted certificate keeps the test clock-free.
        let m: serde_json::Value = serde_json::from_slice(&std::fs::read(&cert).unwrap()).unwrap();
        m["not_before"].as_str().unwrap().to_owned()
    };
    ac().args([
        "mandate-verify",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--at",
        &now,
    ])
    .assert()
    .success();
    ac().args([
        "section-read-agent",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "--at",
        &now,
        "projets/note1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(BODY));
    ac().args([
        "mandate-verify",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--at",
        "2099-01-01T00:00:00Z",
    ])
    .assert()
    .failure();
}

#[test]
fn the_budget_refuses_the_action_after_the_last_one() {
    let dir = init_bundle();
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--max-actions",
        "3",
        "gmail",
        "reply",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    for i in 1..=3 {
        ac().args([
            "action",
            "--dir",
            &d(&dir),
            "--cert",
            &cert,
            "--agent-seed-hex",
            AGENT,
            "gmail",
            "reply",
            "--args",
            &format!("mail {i}"),
        ])
        .assert()
        .success();
    }
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "gmail",
        "reply",
        "--args",
        "mail 4",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("GammaBudgetExhausted"));
}

#[test]
fn owner_silence_suspends_and_the_next_beacon_resumes() {
    let dir = init_bundle();
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--heartbeat-every",
        "1s",
        "--heartbeat-grace",
        "1s",
        "gmail",
        "*",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    let beacon = |seq: &str| {
        ac().args([
            "heartbeat",
            "--dir",
            &d(&dir),
            "--seed-hex",
            OWNER,
            "--seq",
            seq,
        ])
        .assert()
        .success();
    };
    let act = |args: &str| {
        let mut cmd = ac();
        cmd.args([
            "action",
            "--dir",
            &d(&dir),
            "--cert",
            &cert,
            "--agent-seed-hex",
            AGENT,
            "gmail",
            "send",
            "--args",
            args,
        ]);
        cmd
    };
    beacon("1");
    act("vivant").assert().success();
    std::thread::sleep(std::time::Duration::from_secs(3));
    act("trop tard")
        .assert()
        .failure()
        .stderr(predicate::str::contains("GammaHeartbeatStale"));
    beacon("2");
    act("reprise").assert().success();
}

#[test]
fn budget_profiles_meter_inferences_and_models() {
    let dir = init_bundle();
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--budgets-json",
        r#"[{"id":"gemma","models":["gemma"],"token_budget":25000}]"#,
        "gmail",
        "*",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    let infer = |tin: &str, tout: &str| {
        let mut cmd = ac();
        cmd.args([
            "inference",
            "--dir",
            &d(&dir),
            "--cert",
            &cert,
            "--agent-seed-hex",
            AGENT,
            "--tokens-in",
            tin,
            "--tokens-out",
            tout,
            "--budget-ref",
            "gemma",
            "prov",
            "gemma",
        ]);
        cmd
    };
    infer("11000", "1000").assert().success();
    infer("8000", "1000").assert().success();
    infer("4900", "100")
        .assert()
        .failure()
        .stderr(predicate::str::contains("token budget 25000 spent"));
    // Model allow-list.
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "--budget-ref",
        "gemma",
        "--model",
        "gpt-oss",
        "--tokens",
        "10",
        "gmail",
        "reply",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("not allowed"));
    // Citing is mandatory once budgets exist.
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "gmail",
        "reply",
        "--args",
        "x",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("budget_ref"));
}

#[test]
fn absolute_windows_gate_the_action() {
    let dir = init_bundle();
    let grant = |windows: &str, label: &str| {
        ac().args([
            "grant-act",
            "--dir",
            &d(&dir),
            "--seed-hex",
            OWNER,
            "--agent-seed-hex",
            AGENT,
            "--label",
            label,
            "--windows-json",
            windows,
            "gmail",
            "reply",
        ])
        .assert()
        .success();
        last_cert(&dir)
    };
    let act = |cert: &str| {
        let mut cmd = ac();
        cmd.args([
            "action",
            "--dir",
            &d(&dir),
            "--cert",
            cert,
            "--agent-seed-hex",
            AGENT,
            "gmail",
            "reply",
            "--args",
            "x",
        ]);
        cmd
    };
    // A window closed years ago refuses.
    let past = grant(
        r#"[{"anchor":"2020-01-01T00:00:00Z","duration":"1h"}]"#,
        "past",
    );
    act(&past)
        .assert()
        .failure()
        .stderr(predicate::str::contains("outside every active window"));
    // A window opened in 2020 and wide enough to cover today admits.
    let open = grant(
        r#"[{"anchor":"2020-01-01T00:00:00Z","duration":"100000d"}]"#,
        "open",
    );
    act(&open).assert().success();
}

#[test]
fn sealed_args_audit_roundtrip_and_disk_opacity() {
    let dir = init_bundle();
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--audit",
        "gmail",
        "reply",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "--args-json",
        r#"{"recipient":"alice@example.com","subject":"re: devis"}"#,
        "gmail",
        "reply",
    ])
    .assert()
    .success();
    // Opaque on disk and in the skeleton; recovered by the audit.
    assert!(!gamma_raw(&dir).contains("alice@example.com"));
    ac().args(["log-show", "--dir", &d(&dir)])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice").not());
    ac().args(["log-audit", "--dir", &d(&dir), "--seed-hex", OWNER])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice@example.com"))
        .stdout(predicate::str::contains("all consistent"));
}

// -------------------------------------------------- surface invariants ---

/// EXECUTION-PLAN §Sécurité de surface: the kind is imposed by the
/// operation — every verb writes its canonical kind, and no verb lets the
/// caller pick one.
#[test]
fn kinds_are_imposed_by_the_operation() {
    let dir = init_bundle();
    add_circle_section(&dir);
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "gmail",
        "*",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "gmail",
        "reply",
        "--args",
        "x",
    ])
    .assert()
    .success();
    ac().args([
        "inference",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "--tokens-in",
        "10",
        "--tokens-out",
        "1",
        "prov",
        "gemma",
    ])
    .assert()
    .success();
    ac().args(["heartbeat", "--dir", &d(&dir), "--seed-hex", OWNER])
        .assert()
        .success();

    let kinds: Vec<String> = gamma_raw(&dir)
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        kinds,
        ["section.add", "grant", "action", "inference", "heartbeat"],
        "each verb must write exactly its canonical kind, in order"
    );
    // And no surface exists to choose a kind: the flag is rejected.
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "--kind",
        "heartbeat",
        "gmail",
        "reply",
    ])
    .assert()
    .failure();
}

/// EXECUTION-PLAN §Sécurité de surface: the agent key lives in the
/// CLI/container — no secret seed ever reaches stdout, stderr, or any
/// artifact written for third parties (certificates).
#[test]
fn secrets_never_leak_into_outputs_or_certificates() {
    let dir = init_bundle();
    add_circle_section(&dir);
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--max-actions",
        "2",
        "gmail",
        "reply",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);

    let no_secret = |out: &[u8]| {
        let s = String::from_utf8_lossy(out);
        assert!(!s.contains(OWNER), "owner seed leaked");
        assert!(!s.contains(AGENT), "agent seed leaked");
    };
    for args in [
        vec!["log-show", "--dir", &d(&dir)],
        vec![
            "log-query",
            "--dir",
            &d(&dir),
            "--seed-hex",
            OWNER,
            "--kind",
            "action",
        ],
        vec![
            "action",
            "--dir",
            &d(&dir),
            "--cert",
            &cert,
            "--agent-seed-hex",
            AGENT,
            "gmail",
            "reply",
            "--args",
            "x",
        ],
    ] {
        let out = ac().args(&args).output().unwrap();
        no_secret(&out.stdout);
        no_secret(&out.stderr);
    }
    // Certificates carry public keys only.
    let cert_text = std::fs::read_to_string(&cert).unwrap();
    assert!(!cert_text.contains(AGENT) && !cert_text.contains(OWNER));
    // The log itself never holds a seed either.
    let log = gamma_raw(&dir);
    assert!(!log.contains(AGENT) && !log.contains(OWNER));
}

// ------------------------------------------------ fail-closed surfaces ---

#[test]
fn invalid_inputs_fail_closed() {
    let dir = init_bundle();
    // Malformed seed.
    ac().args([
        "section-add",
        "--dir",
        &d(&dir),
        "--seed-hex",
        "beef",
        "circle",
        "x",
        "--body",
        "b",
    ])
    .assert()
    .failure();
    // Unknown zone.
    ac().args([
        "section-add",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "nowhere",
        "x",
        "--body",
        "b",
    ])
    .assert()
    .failure();
    // Garbage budgets JSON.
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--budgets-json",
        "not json",
        "gmail",
        "reply",
    ])
    .assert()
    .failure();
    // Missing certificate file.
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        "/nonexistent/cert.json",
        "--agent-seed-hex",
        AGENT,
        "gmail",
        "reply",
    ])
    .assert()
    .failure();
    // No bundle at all.
    ac().args(["log-verify", "--dir", "/nonexistent-bundle"])
        .assert()
        .failure();
}
