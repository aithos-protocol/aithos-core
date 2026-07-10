//! Surface tests on the real binary (`assert_cmd`), mirroring the core's
//! `cli_surface.rs` discipline: the security-bearing invariants of the
//! CLI are real tests, not a manual checklist.
//!
//! Invariants covered here:
//! - onboarding hands the operator the cold seeds ONCE and the endpoint,
//!   but never the gateway-held (agent / gateway) seeds;
//! - a malformed or unknown config fails closed (exit 2), it is never
//!   guessed at;
//! - the audit export honours the auditor's scope end to end, and an
//!   out-of-scope query is refused.

use assert_cmd::Command;
use predicates::prelude::*;

fn write_config(dir: &std::path::Path) -> std::path::PathBuf {
    let store = dir.join("ethos");
    let cfg = format!(
        "listen: 127.0.0.1:0\nupstream_mcp: http://127.0.0.1:4124/mcp\nstore:\n  kind: fs\n  root: {}\ntools:\n  user.read: read\n  user.update: write\n",
        store.display()
    );
    let path = dir.join("gateway.yaml");
    std::fs::write(&path, cfg).unwrap();
    path
}

fn gateway() -> Command {
    Command::cargo_bin("aithos-gateway").unwrap()
}

#[test]
fn onboard_prints_endpoint_and_cold_seeds_but_never_gateway_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(tmp.path());

    let out = gateway()
        .args(["--config", cfg.to_str().unwrap(), "onboard"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "agent_endpoint: http://127.0.0.1:0/mcp",
        ))
        .stdout(predicate::str::contains("owner_seed_hex:"))
        .stdout(predicate::str::contains("auditor_seed_hex:"))
        .get_output()
        .clone();

    // The gateway-held seeds persist for `run` but must never surface.
    let keys: serde_json::Value =
        serde_json::from_slice(&std::fs::read(tmp.path().join("ethos/gateway/keys.json")).unwrap())
            .unwrap();
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for k in ["agent_seed_hex", "gateway_seed_hex"] {
        let seed = keys[k].as_str().unwrap();
        assert!(!printed.contains(seed), "{k} leaked to the console");
    }
}

#[test]
fn unknown_config_keys_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(tmp.path());
    let mut text = std::fs::read_to_string(&cfg).unwrap();
    text.push_str("surprise: true\n");
    std::fs::write(&cfg, text).unwrap();

    gateway()
        .args(["--config", cfg.to_str().unwrap(), "onboard"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("config rejected"));
}

#[test]
fn audit_export_honours_the_auditor_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(tmp.path());

    let out = gateway()
        .args(["--config", cfg.to_str().unwrap(), "onboard"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let seed = stdout
        .lines()
        .find_map(|l| l.strip_prefix("auditor_seed_hex: "))
        .expect("auditor seed printed once")
        .to_owned();

    // In scope (kind=action): a valid, empty slice — no act has happened.
    let export = gateway()
        .args([
            "--config",
            cfg.to_str().unwrap(),
            "audit-export",
            "--auditor-seed-hex",
            &seed,
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let json: serde_json::Value =
        serde_json::from_slice(&export.stdout).expect("export is valid JSON");
    assert_eq!(json["entries"], serde_json::json!([]));

    // Out of scope (kind=grant): refused by the certificate half.
    gateway()
        .args([
            "--config",
            cfg.to_str().unwrap(),
            "audit-export",
            "--auditor-seed-hex",
            &seed,
            "--kind",
            "grant",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("audit read denied"));
}
