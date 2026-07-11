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
fn an_obligation_gates_the_action_on_a_signed_receipt() {
    const APPROVER: &str = "b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5";
    let dir = init_bundle();
    // The approver's public key, pinned at grant time.
    let key_out = ac()
        .args([
            "approve",
            "--approver-seed-hex",
            APPROVER,
            "--mandate",
            "-",
            "--key-only",
            "publish",
        ])
        .output()
        .unwrap();
    assert!(key_out.status.success());
    let approver_pub = String::from_utf8(key_out.stdout).unwrap().trim().to_owned();
    assert!(approver_pub.starts_with("z6Mk"), "got: {approver_pub}");
    let obligations = format!(
        r#"[{{"id":"publish-approval","check":"human.approve","attestor":["{approver_pub}"],"applies_to":"act.x.social.publish","verdict":"approve","max_age":"5m"}}]"#
    );
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--obligations-json",
        &obligations,
        "social",
        "*",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    let mandate_id = serde_json::from_slice::<serde_json::Value>(&std::fs::read(&cert).unwrap())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // Without a receipt the in-scope action is refused, fail-closed.
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "social",
        "publish",
        "--args",
        "post hello",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("GammaObligationUnsatisfied"));
    // The approver signs what was shown; the receipt leaks no seed.
    let approve_out = ac()
        .args([
            "approve",
            "--approver-seed-hex",
            APPROVER,
            "--obligation",
            "publish-approval",
            "--mandate",
            &mandate_id,
            "--args",
            "post hello",
            "--presented",
            "rendered: post hello",
            "publish",
        ])
        .output()
        .unwrap();
    assert!(approve_out.status.success());
    let receipt_text = String::from_utf8(approve_out.stdout).unwrap();
    assert!(
        !receipt_text.contains(APPROVER),
        "the approver seed must never leak into the receipt"
    );
    let receipt: serde_json::Value = serde_json::from_str(&receipt_text).unwrap();
    // The receipted action appends; the receipt rides in checks[].
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "social",
        "publish",
        "--args",
        "post hello",
        "--check-json",
        &receipt.to_string(),
    ])
    .assert()
    .success();
    assert!(
        gamma_raw(&dir).contains("publish-approval"),
        "the receipt must be recorded in the entry"
    );
    // A replay on other args dies in the signature.
    ac().args([
        "action",
        "--dir",
        &d(&dir),
        "--cert",
        &cert,
        "--agent-seed-hex",
        AGENT,
        "social",
        "publish",
        "--args",
        "post SOMETHING ELSE",
        "--check-json",
        &receipt.to_string(),
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("GammaObligationUnsatisfied"));
}

#[test]
fn counter_sign_requires_the_owner_co_signature() {
    let dir = init_bundle();
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "--counter-sign",
        "send",
        "gmail",
        "*",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    let mandate_id = serde_json::from_slice::<serde_json::Value>(&std::fs::read(&cert).unwrap())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // Binding action without the owner in the loop: refused.
    ac().args([
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
        "wire the funds",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("GammaObligationUnsatisfied"));
    // A non-binding action under the same mandate rides free.
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
        "just a reply",
    ])
    .assert()
    .success();
    // The owner co-signs (content key, desugared co_sign instance).
    let approve_out = ac()
        .args([
            "approve",
            "--owner-seed-hex",
            OWNER,
            "--mandate",
            &mandate_id,
            "--args",
            "wire the funds",
            "--presented",
            "SEND: wire the funds",
            "send",
        ])
        .output()
        .unwrap();
    assert!(approve_out.status.success());
    let receipt: serde_json::Value =
        serde_json::from_str(&String::from_utf8(approve_out.stdout).unwrap()).unwrap();
    assert_eq!(receipt["obligation"], "co_sign");
    ac().args([
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
        "wire the funds",
        "--check-json",
        &receipt.to_string(),
    ])
    .assert()
    .success();
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

#[test]
fn a_move_rotates_cuts_the_old_parent_and_keeps_the_direct_line() {
    const OLD_PARENT_AGENT: &str =
        "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
    let dir = init_bundle();
    // archives/old/note1 to move, projets as destination.
    for path in ["archives/old/note1", "projets/keep"] {
        ac().args([
            "section-add",
            "--dir",
            &d(&dir),
            "--seed-hex",
            OWNER,
            "circle",
            path,
            "--title",
            "note",
            "--body",
            BODY,
        ])
        .assert()
        .success();
    }
    // One direct line on the moved folder, one grant on the old parent.
    for (seed, folder) in [(AGENT, "archives/old"), (OLD_PARENT_AGENT, "archives")] {
        ac().args([
            "grant",
            "--dir",
            &d(&dir),
            "--seed-hex",
            OWNER,
            "--agent-seed-hex",
            seed,
            folder,
            "--ttl-days",
            "7",
        ])
        .assert()
        .success();
    }
    let old_parent_cert = last_cert(&dir);
    let now = {
        let m: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&old_parent_cert).unwrap()).unwrap();
        m["not_before"].as_str().unwrap().to_owned()
    };
    let direct_cert = {
        let mut certs: Vec<_> = std::fs::read_dir(dir.path().join("certs"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        certs.sort_by_key(|p| std::fs::metadata(p).unwrap().modified().unwrap());
        certs[certs.len() - 2].to_str().unwrap().to_owned()
    };

    ac().args([
        "move",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "archives/old",
        "--under",
        "projets",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("moved archives/old under projets"));

    // The direct line survives — same cert, same keypair, new address.
    ac().args([
        "section-read-agent",
        "--dir",
        &d(&dir),
        "--cert",
        &direct_cert,
        "--agent-seed-hex",
        AGENT,
        "--at",
        &now,
        "projets/old/note1",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(BODY));

    // The old parent's grant no longer covers the moved subtree.
    ac().args([
        "section-read-agent",
        "--dir",
        &d(&dir),
        "--cert",
        &old_parent_cert,
        "--agent-seed-hex",
        OLD_PARENT_AGENT,
        "--at",
        &now,
        "projets/old/note1",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("not covered"));

    // The move republished a verifiable edition and log.
    ac().args(["edition-verify", "--dir", &d(&dir)])
        .assert()
        .success();
    ac().args(["log-verify", "--dir", &d(&dir)])
        .assert()
        .success();
}

/// H2 (spec 07.10): the committed counts trie serves offline count and
/// absence proofs; a counted mandate can never be proven absent.
#[test]
fn log_prove_counts_and_absence_offline() {
    let dir = init_bundle();
    ac().args([
        "grant-act",
        "--dir",
        &d(&dir),
        "--seed-hex",
        OWNER,
        "--agent-seed-hex",
        AGENT,
        "gmail",
        "reply",
    ])
    .assert()
    .success();
    let cert = last_cert(&dir);
    let mandate_id =
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&cert).unwrap())
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
    for i in 1..=2 {
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
    ac().args(["edition-publish", "--dir", &d(&dir), "--seed-hex", OWNER])
        .assert()
        .success();

    // Count proof: two actions, proven against the committed root.
    ac().args(["log-prove", "--dir", &d(&dir), "--mandate", &mandate_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"payload\""))
        .stderr(predicate::str::contains("count proof verifies"))
        .stderr(predicate::str::contains("\"actions\":2"));

    // Absence proof for an id never counted.
    ac().args([
        "log-prove",
        "--dir",
        &d(&dir),
        "--absent",
        "mandate_zzzzzzzzzzzzzzzzzzzzzzzzzz",
    ])
    .assert()
    .success()
    .stderr(predicate::str::contains("was never counted"));

    // A counted mandate cannot be proven absent — fail-closed.
    ac().args(["log-prove", "--dir", &d(&dir), "--absent", &mandate_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("counted"));
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn add_named(dir: &TempDir, path: &str) {
    ac().args([
        "section-add",
        "--dir",
        &d(dir),
        "--seed-hex",
        OWNER,
        "circle",
        path,
        "--title",
        "note",
        "--body",
        BODY,
    ])
    .assert()
    .success();
}

#[test]
fn edition_merge_joins_two_disjoint_copies() {
    // The shared ancestor: one section, one published edition.
    let dir = init_bundle();
    add_circle_section(&dir);
    ac().args(["edition-publish", "--dir", &d(&dir), "--seed-hex", OWNER])
        .assert()
        .success();
    let other = TempDir::new().unwrap();
    copy_tree(dir.path(), other.path());

    // Two disjoint writes, two competing editions at the same height.
    add_named(&dir, "alpha/note-a");
    add_named(&other, "beta/note-b");
    for b in [&dir, &other] {
        ac().args(["edition-publish", "--dir", &d(b), "--seed-hex", OWNER])
            .assert()
            .success();
    }

    // Either party merges; the result verifies and holds both writes.
    ac().args([
        "edition-merge",
        "--dir",
        &d(&dir),
        "--other",
        &d(&other),
        "--seed-hex",
        OWNER,
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("merge edition published"));
    ac().args(["edition-verify", "--dir", &d(&dir)])
        .assert()
        .success();
    ac().args(["log-verify", "--dir", &d(&dir)])
        .assert()
        .success();
    for path in ["alpha/note-a", "beta/note-b"] {
        ac().args([
            "section-read",
            "--dir",
            &d(&dir),
            "circle",
            path,
            "--seed-hex",
            OWNER,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(BODY));
    }
    let manifest = std::fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    assert!(
        manifest.contains("\"merges\""),
        "the merge wire is committed"
    );

    // The same node touched on both sides is a fork — refused. Both copies
    // grant the SAME folder: its header (folded into the node hash) changes
    // on both branches.
    let third = TempDir::new().unwrap();
    copy_tree(dir.path(), third.path());
    for b in [&dir, &third] {
        ac().args([
            "grant",
            "--dir",
            &d(b),
            "--seed-hex",
            OWNER,
            "--agent-seed-hex",
            AGENT,
            "projets",
        ])
        .assert()
        .success();
        ac().args(["edition-publish", "--dir", &d(b), "--seed-hex", OWNER])
            .assert()
            .success();
    }
    ac().args([
        "edition-merge",
        "--dir",
        &d(&dir),
        "--other",
        &d(&third),
        "--seed-hex",
        OWNER,
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("conflict"));
}
