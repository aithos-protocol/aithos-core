use aithos_bundle::bundle::Bundle;
use aithos_bundle::{FsStore, Store};
use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::gamma::Entry;
use aithos_core::jcs;
use aithos_core::mandate::Mandate;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const VECTOR_JSON: &str = include_str!("../../../../vectors/cb2-bundle-version-coexistence.json");
const HISTORICAL_EPLUS: &[u8] = include_bytes!("../../../../vectors/eplus-attenuation.json");
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn vector() -> Value {
    serde_json::from_str(VECTOR_JSON).expect("CB2 Bundle coexistence vector parses")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cb2-fsstore-tests")
            });
        std::fs::create_dir_all(&base)
            .unwrap_or_else(|error| panic!("create CB2 FsStore test base {base:?}: {error}"));
        for _ in 0..1024 {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "aithos-cb2-bundle-version-coexistence-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create isolated CB2 FsStore root {path:?}: {error}"),
            }
        }
        panic!("could not allocate an isolated CB2 FsStore root");
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_case<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["negative_cases"]
        .as_array()
        .expect("negative_cases array")
        .iter()
        .find(|case| case["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("missing fixture case {id}"))
}

fn install_and_cold_reopen(v: &Value, section: &Value, label: &str) -> (TempRoot, Bundle<FsStore>) {
    let temp = TempRoot::new(label);
    let mut store = FsStore::new(temp.path());
    let did_path = v["did"]["path"].as_str().expect("did path");
    let did_jcs = v["did"]["jcs"].as_str().expect("did JCS");
    store
        .put(did_path, did_jcs.as_bytes())
        .expect("write exact DID bytes through public Store");

    let names = section["certificate_names"]
        .as_array()
        .expect("certificate_names array");
    for name in names {
        let name = name.as_str().expect("certificate name");
        let record = &v["certificates"][name];
        let id = record["id"].as_str().expect("certificate id");
        let certificate_jcs = record["jcs"].as_str().expect("certificate JCS");
        store
            .put(&format!("certs/{id}.json"), certificate_jcs.as_bytes())
            .expect("write exact certificate bytes through public Store");
    }
    assert_eq!(
        store.list("certs/").expect("list certificate paths").len(),
        names.len(),
        "all selected certificates share this one FsStore"
    );

    let gamma_path = section["gamma_path"].as_str().expect("Gamma path");
    let gamma_jsonl = section["gamma_jsonl"].as_str().expect("Gamma JSONL");
    assert_eq!(
        sha256_hex(gamma_jsonl.as_bytes()),
        section["gamma_sha256"].as_str().expect("Gamma SHA-256")
    );
    store
        .put(gamma_path, gamma_jsonl.as_bytes())
        .expect("write exact Gamma JSONL through public Store");

    let opened = Bundle::open(store).expect("the freshly installed Bundle opens");
    assert_eq!(opened.did, v["did"]["id"].as_str().expect("DID id"));
    assert_eq!(
        opened
            .store
            .list("certs/")
            .expect("list certificates from opened Bundle")
            .len(),
        names.len()
    );
    drop(opened);

    let cold = Bundle::open(FsStore::new(temp.path()))
        .expect("the same FsStore reopens after dropping the first Bundle");
    assert_eq!(cold.did, v["did"]["id"].as_str().expect("DID id"));
    (temp, cold)
}

fn assert_mixed_case_rejected(case_id: &str) {
    let v = vector();
    let section = fixture_case(&v, case_id);
    assert_eq!(section["decision_stage"].as_str(), Some("version"));
    assert_eq!(section["expected"].as_str(), Some("InvalidMandate"));
    let (_temp, cold) = install_and_cold_reopen(&v, section, case_id);
    match cold.gamma_verify() {
        Err(Error::InvalidMandate(_)) => {}
        Err(other) => {
            panic!("{case_id}: expected typed InvalidMandate at version dispatch, got {other:?}")
        }
        Ok(()) => panic!("{case_id}: mixed-version authorized_via was accepted before attenuation"),
    }
}

#[test]
fn cb2_bundle_version_coexistence_vector_and_historical_bytes_are_frozen() {
    let v = vector();
    assert_eq!(
        v["vector"].as_str(),
        Some("CB2-BUNDLE-VERSION-COEXISTENCE-1")
    );
    assert_eq!(
        sha256_hex(HISTORICAL_EPLUS),
        v["historical_inputs"]["eplus_draft1_chain"]["sha256"]
            .as_str()
            .expect("historical E+ SHA-256")
    );

    let did_jcs = v["did"]["jcs"].as_str().expect("DID JCS");
    let did_value: Value = serde_json::from_str(did_jcs).expect("DID JSON");
    assert_eq!(
        jcs::canonicalize(&did_value).expect("canonical DID"),
        did_jcs
    );
    assert_eq!(
        sha256_hex(did_jcs.as_bytes()),
        v["did"]["sha256"].as_str().expect("DID SHA-256")
    );
    let did: DidDocument = serde_json::from_str(did_jcs).expect("typed DID");
    did.verify().expect("independently generated DID verifies");

    for (name, record) in v["certificates"].as_object().expect("certificate object") {
        let certificate_jcs = record["jcs"].as_str().expect("certificate JCS");
        let certificate_value: Value =
            serde_json::from_str(certificate_jcs).expect("certificate JSON");
        assert_eq!(
            jcs::canonicalize(&certificate_value).expect("canonical certificate"),
            certificate_jcs,
            "{name} JCS"
        );
        assert_eq!(
            sha256_hex(certificate_jcs.as_bytes()),
            record["sha256"].as_str().expect("certificate SHA-256"),
            "{name} SHA-256"
        );
        let certificate: Mandate =
            serde_json::from_str(certificate_jcs).expect("typed certificate");
        assert_eq!(certificate.id, record["id"].as_str().expect("id"));
        assert_eq!(
            certificate.version,
            record["version"].as_str().expect("version")
        );
    }

    for line in v["positive"]["gamma_jsonl"]
        .as_str()
        .expect("positive Gamma JSONL")
        .lines()
    {
        let value: Value = serde_json::from_str(line).expect("Gamma JSON line");
        assert_eq!(jcs::canonicalize(&value).expect("canonical Gamma"), line);
        let _: Entry = serde_json::from_str(line).expect("typed Gamma entry");
    }
}

#[test]
fn cb2_homogeneous_draft1_and_draft2_chains_coexist_after_fsstore_reopen() {
    let v = vector();
    let positive = &v["positive"];
    let draft1 = positive["chains"]["draft1"]
        .as_array()
        .expect("draft.1 chain");
    let draft2 = positive["chains"]["draft2"]
        .as_array()
        .expect("draft.2 chain");
    assert!(draft1.iter().all(|name| {
        v["certificates"][name.as_str().expect("draft.1 name")]["version"].as_str()
            == Some("1.0.0-draft.1")
    }));
    assert!(draft2.iter().all(|name| {
        v["certificates"][name.as_str().expect("draft.2 name")]["version"].as_str()
            == Some("1.0.0-draft.2")
    }));
    assert_eq!(
        positive["delegated_entry_ids"]["draft1"]
            .as_array()
            .expect("draft.1 delegated entries")
            .len(),
        2
    );
    assert_eq!(
        positive["delegated_entry_ids"]["draft2"]
            .as_array()
            .expect("draft.2 delegated entries")
            .len(),
        2
    );
    let entries: Vec<Entry> = positive["gamma_jsonl"]
        .as_str()
        .expect("positive Gamma JSONL")
        .lines()
        .map(|line| serde_json::from_str(line).expect("typed positive Gamma entry"))
        .collect();
    for (profile, expected_version) in [("draft1", "1.0.0-draft.1"), ("draft2", "1.0.0-draft.2")] {
        for entry_id in positive["delegated_entry_ids"][profile]
            .as_array()
            .expect("delegated entry id list")
        {
            let entry_id = entry_id.as_str().expect("delegated entry id");
            let entry = entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .unwrap_or_else(|| panic!("missing delegated {profile} entry {entry_id}"));
            let via = entry
                .authorized_via
                .as_ref()
                .unwrap_or_else(|| panic!("{entry_id} has no authorized_via"));
            assert!(!via.is_empty(), "{entry_id} has an empty authorized_via");
            for mandate_id in via {
                let certificate = v["certificates"]
                    .as_object()
                    .expect("certificate object")
                    .values()
                    .find(|record| record["id"].as_str() == Some(mandate_id))
                    .unwrap_or_else(|| panic!("{entry_id} cites absent certificate {mandate_id}"));
                assert_eq!(
                    certificate["version"].as_str(),
                    Some(expected_version),
                    "{entry_id} mixes mandate versions"
                );
            }
        }
    }

    let (_temp, cold) = install_and_cold_reopen(&v, positive, "positive");
    cold.gamma_verify()
        .expect("both homogeneous chains verify from the same cold FsStore/DID");
}

#[test]
fn cb2_mixed_draft1_to_draft2_authorized_via_is_typed_invalid_mandate() {
    assert_mixed_case_rejected("mixed_draft1_to_draft2");
}

#[test]
fn cb2_mixed_draft2_to_draft1_authorized_via_is_typed_invalid_mandate() {
    assert_mixed_case_rejected("mixed_draft2_to_draft1");
}
