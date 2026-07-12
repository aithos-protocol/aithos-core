//! Owner-side surface tests on the real binary (`assert_cmd`) — the
//! Phase B provisioning path as the ENTERPRISE runs it, where the master
//! seed lives and the runner is only a pair of public keys.
//!
//! Invariants covered here (lot 4):
//! - the three owner commands write the mandate certificates into the
//!   ethos store, and each certificate names EXACTLY the public key the
//!   owner granted to (the runner key never left the runner);
//! - every grant is logged in the ethos gamma (issuance is never silent);
//! - the console shows NO seed material — not the master seed handed in,
//!   not the runner seeds (which the owner side never even holds) — with
//!   ONE exception: the auditor seed, printed once at grant time next to
//!   the store-it-cold warning;
//! - `--master-seed-hex` on the command line prints the DEV ONLY warning;
//! - malformed inputs fail closed (exit 2), never half-provision.

use assert_cmd::Command;
use predicates::prelude::*;

/// A dev master seed (32 bytes hex). Its VALUE must never echo back.
const MASTER: &str = "9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c5b4a39281706f5e4d3c2b1a0";

fn gateway() -> Command {
    Command::cargo_bin("aithos-gateway").unwrap()
}

/// `keygen` on the real binary: returns the published public halves and
/// the identity file path (whose seeds must never surface again).
fn keygen(dir: &std::path::Path) -> (String, String, std::path::PathBuf) {
    let id_path = dir.join("agent.id");
    let out = gateway()
        .args(["--identity", id_path.to_str().unwrap(), "keygen"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    (
        line_value(&stdout, "agent_pub: "),
        line_value(&stdout, "gateway_pub: "),
        id_path,
    )
}

fn line_value(stdout: &str, prefix: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("`{prefix}` not printed"))
        .to_owned()
}

/// stdout ‖ stderr of a finished command, for no-leak assertions.
fn console(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The seeds persisted at birth — the exact strings that must never
/// appear on any owner-side console.
fn runner_seeds(id_path: &std::path::Path) -> Vec<String> {
    let keys: serde_json::Value = serde_json::from_slice(&std::fs::read(id_path).unwrap()).unwrap();
    ["agent_seed_hex", "gateway_seed_hex"]
        .iter()
        .map(|k| keys[k].as_str().unwrap().to_owned())
        .collect()
}

/// The full JSON of a stored mandate certificate.
fn cert_json(store_root: &std::path::Path, mandate_id: &str) -> serde_json::Value {
    let path = store_root.join("certs").join(format!("{mandate_id}.json"));
    serde_json::from_slice(
        &std::fs::read(&path).unwrap_or_else(|e| panic!("cert {} unreadable: {e}", path.display())),
    )
    .unwrap()
}

/// The grantee pubkey (multibase) named by a stored certificate.
fn cert_grantee(store_root: &std::path::Path, mandate_id: &str) -> String {
    cert_json(store_root, mandate_id)["grantee"]["pubkey"]
        .as_str()
        .expect("grantee pubkey")
        .to_owned()
}

/// Clear headers of every gamma entry in a store (kind, target).
fn gamma_kinds(store_root: &std::path::Path) -> Vec<(String, Option<String>)> {
    let mut lines = Vec::new();
    let dir = store_root.join("gamma");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("gamma dir {} unreadable: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for f in files {
        for line in std::fs::read_to_string(f).unwrap().lines() {
            let e: serde_json::Value = serde_json::from_str(line).unwrap();
            lines.push((
                e["kind"].as_str().unwrap_or_default().to_owned(),
                e["target"].as_str().map(str::to_owned),
            ));
        }
    }
    lines
}

fn assert_no_seed_leak(printed: &str, id_path: &std::path::Path) {
    assert!(
        !printed.contains(MASTER),
        "the master seed echoed back to the console"
    );
    for seed in runner_seeds(id_path) {
        assert!(
            !printed.contains(&seed),
            "a runner seed surfaced on the owner console"
        );
    }
}

#[test]
fn owner_init_journal_writes_certs_to_the_granted_pubkeys_and_logs_the_grants() {
    let tmp = tempfile::tempdir().unwrap();
    let (agent_pub, gateway_pub, id_path) = keygen(tmp.path());
    let store = tmp.path().join("journal");

    let out = gateway()
        .args([
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
            store.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("journal_did: "))
        .stderr(predicate::str::contains("DEV ONLY"))
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let agent_mandate = line_value(&stdout, "agent_mandate: ");
    let gateway_mandate = line_value(&stdout, "gateway_mandate: ");
    let memory_mandate = line_value(&stdout, "memory_mandate: ");

    // The certs are on disk and name EXACTLY the keys the owner granted
    // to — the runner's seeds never travelled, only these pubkeys did.
    // The memory pen (lot C2) targets the SAME agent key with its own
    // certificate: append on the circle shelf, nothing wider.
    assert_eq!(cert_grantee(&store, &agent_mandate), agent_pub);
    assert_eq!(cert_grantee(&store, &gateway_mandate), gateway_pub);
    assert_eq!(cert_grantee(&store, &memory_mandate), agent_pub);
    let pen = cert_json(&store, &memory_mandate);
    let perimeter = pen["perimeter"][0].as_str().expect("a perimeter entry");
    assert!(
        perimeter.starts_with("append.circle#dir="),
        "the memory pen is append on the circle shelf, got `{perimeter}`"
    );

    // Every grant is logged (issuance is never silent) and names its
    // mandate; the journal grants no auditor (no audit grant to log).
    let grants: Vec<_> = gamma_kinds(&store)
        .into_iter()
        .filter(|(k, _)| k == "grant")
        .collect();
    assert_eq!(
        grants.len(),
        3,
        "three logged grants: xref pen + gateway pen + memory pen"
    );
    let targets: Vec<_> = grants.iter().filter_map(|(_, t)| t.as_deref()).collect();
    assert!(targets.contains(&agent_mandate.as_str()));
    assert!(targets.contains(&gateway_mandate.as_str()));
    assert!(targets.contains(&memory_mandate.as_str()));

    // No auditor on a journal, and no seed anywhere on this console.
    assert!(!stdout.contains("auditor_seed_hex"));
    assert!(
        !store.join("gateway/keys.json").exists(),
        "owner-side stores must never hold runner keys"
    );
    assert_no_seed_leak(&console(&out), &id_path);
}

#[test]
fn owner_grant_context_prints_the_auditor_seed_once_and_no_other_secret() {
    let tmp = tempfile::tempdir().unwrap();
    let (agent_pub, gateway_pub, id_path) = keygen(tmp.path());
    let store = tmp.path().join("brand");

    let init = gateway()
        .args([
            "owner-init-context",
            "--master-seed-hex",
            MASTER,
            "--label",
            "company-brand",
            "--store-root",
            store.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("context_did: "))
        .stderr(predicate::str::contains("DEV ONLY"))
        .get_output()
        .clone();
    assert_no_seed_leak(&console(&init), &id_path);
    let context_did = line_value(&String::from_utf8_lossy(&init.stdout), "context_did: ");

    let out = gateway()
        .args([
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
            "--read",
            "brand.guidelines",
            "--store-root",
            store.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("STORE the auditor seed COLD"))
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(line_value(&stdout, "context_did: "), context_did);
    let agent_mandate = line_value(&stdout, "agent_mandate: ");
    let gateway_mandate = line_value(&stdout, "gateway_mandate: ");
    let auditor_mandate = line_value(&stdout, "auditor_mandate: ");
    let auditor_seed = line_value(&stdout, "auditor_seed_hex: ");

    // The ONE permitted secret: the auditor seed, printed once, valid.
    assert_eq!(
        stdout.matches(&auditor_seed).count(),
        1,
        "the auditor seed appears exactly once"
    );
    assert_eq!(hex::decode(&auditor_seed).unwrap().len(), 32);

    // Certs written towards the granted pubkeys; the auditor cert names
    // a key that is NEITHER runner key (freshly minted for the auditor).
    assert_eq!(cert_grantee(&store, &agent_mandate), agent_pub);
    assert_eq!(cert_grantee(&store, &gateway_mandate), gateway_pub);
    let auditor_grantee = cert_grantee(&store, &auditor_mandate);
    assert_ne!(auditor_grantee, agent_pub);
    assert_ne!(auditor_grantee, gateway_pub);

    // Three logged grants on top of the context genesis.
    let grants = gamma_kinds(&store)
        .into_iter()
        .filter(|(k, _)| k == "grant")
        .count();
    assert_eq!(grants, 3, "agent + gateway + auditor grants are logged");

    // No other secret on the console: master seed and runner seeds stay out.
    assert_no_seed_leak(&console(&out), &id_path);
    assert!(
        !store.join("gateway/keys.json").exists(),
        "owner-side stores must never hold runner keys"
    );
}

#[test]
fn owner_init_journal_with_token_budget_mints_the_inference_pen() {
    let tmp = tempfile::tempdir().unwrap();
    let (agent_pub, gateway_pub, id_path) = keygen(tmp.path());
    let store = tmp.path().join("journal");

    let out = gateway()
        .args([
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
            store.to_str().unwrap(),
            "--token-budget",
            "750",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("inference_mandate: "))
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let agent_mandate = line_value(&stdout, "agent_mandate: ");
    let inference_mandate = line_value(&stdout, "inference_mandate: ");

    // The pen is a THIRD mandate towards the SAME agent key, carrying the
    // token budget as a profile constraint — separate on purpose, so the
    // xref pen never has to cite a budget.
    assert_ne!(inference_mandate, agent_mandate);
    assert_eq!(cert_grantee(&store, &inference_mandate), agent_pub);
    let pen = cert_json(&store, &inference_mandate);
    assert_eq!(
        pen["constraints"]["budgets"],
        serde_json::json!([{ "id": "llm", "token_budget": 750 }]),
        "the certificate carries the budget the owner set"
    );
    assert!(
        cert_json(&store, &agent_mandate)["constraints"]["budgets"].is_null(),
        "the xref pen stays budget-free"
    );

    // Four logged grants: xref pen + gateway pen + memory pen (lot C2)
    // + inference pen — no issuance is ever silent.
    let grants: Vec<_> = gamma_kinds(&store)
        .into_iter()
        .filter(|(k, _)| k == "grant")
        .collect();
    assert_eq!(
        grants.len(),
        4,
        "agent + gateway + memory + inference grants logged"
    );
    let targets: Vec<_> = grants.iter().filter_map(|(_, t)| t.as_deref()).collect();
    assert!(targets.contains(&inference_mandate.as_str()));

    // The budget number may echo; seed material still never does.
    assert_no_seed_leak(&console(&out), &id_path);

    // Without the flag: no pen, no `inference_mandate` line — the
    // no-pen-no-LLM refusal is contracted at library level.
    let bare = tmp.path().join("journal-bare");
    let out2 = gateway()
        .args([
            "owner-init-journal",
            "--master-seed-hex",
            MASTER,
            "--agent-label",
            "agent-8",
            "--agent-pub",
            &agent_pub,
            "--gateway-pub",
            &gateway_pub,
            "--store-root",
            bare.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(
        !String::from_utf8_lossy(&out2.stdout).contains("inference_mandate"),
        "no budget, no pen, no fourth line"
    );
}

#[test]
fn owner_commands_fail_closed_on_malformed_inputs() {
    let tmp = tempfile::tempdir().unwrap();
    let (agent_pub, gateway_pub, _id) = keygen(tmp.path());
    let store = tmp.path().join("ctx");

    // A malformed master seed provisions nothing.
    gateway()
        .args([
            "owner-init-journal",
            "--master-seed-hex",
            "zz",
            "--agent-label",
            "agent-7",
            "--agent-pub",
            &agent_pub,
            "--gateway-pub",
            &gateway_pub,
            "--store-root",
            tmp.path().join("j").to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("want 32 hex bytes"));
    assert!(
        !tmp.path().join("j").exists()
            || std::fs::read_dir(tmp.path().join("j"))
                .unwrap()
                .next()
                .is_none()
    );

    // Granting into a store that was never initialised is refused.
    gateway()
        .args([
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
            store.to_str().unwrap(),
        ])
        .assert()
        .code(2);

    // A malformed grantee pubkey is refused after a real init.
    gateway()
        .args([
            "owner-init-context",
            "--master-seed-hex",
            MASTER,
            "--label",
            "company-brand",
            "--store-root",
            store.to_str().unwrap(),
        ])
        .assert()
        .success();
    gateway()
        .args([
            "owner-grant-context",
            "--master-seed-hex",
            MASTER,
            "--label",
            "company-brand",
            "--agent-pub",
            "not-a-key",
            "--gateway-pub",
            &gateway_pub,
            "--read",
            "brand.read",
            "--store-root",
            store.to_str().unwrap(),
        ])
        .assert()
        .code(2);
}
