//! CB7 transactional Store and confinement contracts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::{validate_display_path, validate_store_key, FsStore, MemStore, Store};
use aithos_core::error::Error;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::path::Zone;
use serde_json::Value;

const VECTOR_BYTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../vectors/cb2-bundle-boundaries.json"
));
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn vector() -> Value {
    serde_json::from_str(VECTOR_BYTES).expect("CB2 Bundle boundary vector parses")
}

fn snapshot(value: &Value) -> BTreeMap<String, Vec<u8>> {
    value
        .as_object()
        .expect("snapshot object")
        .iter()
        .map(|(path, bytes)| {
            (
                path.clone(),
                bytes.as_str().expect("snapshot bytes").as_bytes().to_vec(),
            )
        })
        .collect()
}

fn read_snapshot(store: &impl Store) -> BTreeMap<String, Vec<u8>> {
    store
        .list("")
        .expect("list canonical snapshot")
        .into_iter()
        .map(|path| {
            let bytes = store
                .get(&path)
                .expect("read canonical object")
                .expect("listed object exists");
            (path, bytes)
        })
        .collect()
}

fn install(store: &mut impl Store, objects: &BTreeMap<String, Vec<u8>>) {
    for (path, bytes) in objects {
        store.put(path, bytes).expect("install fixture object");
    }
}

fn replace_transaction(
    store: &mut impl Store,
    old: &BTreeMap<String, Vec<u8>>,
    new: &BTreeMap<String, Vec<u8>>,
) {
    store.begin_transaction().expect("begin transaction");
    for path in old.keys().filter(|path| !new.contains_key(*path)) {
        store.delete(path).expect("delete candidate object");
    }
    for (path, bytes) in new {
        if old.get(path) != Some(bytes) {
            store.put(path, bytes).expect("stage candidate object");
        }
    }
}

fn owner() -> OwnerKeys {
    let seed = MasterSeed::from_slice(&[0x31; 32]).expect("valid CB7 owner seed");
    OwnerKeys::genesis(&seed)
}

fn initialized_bundle<S: Store>(store: S) -> (Bundle<S>, OwnerKeys, SeqEntropy) {
    let owner = owner();
    let succession = succession_from_entropy([0x47; 32]);
    let mut entropy = SeqEntropy::default();
    let bundle = Bundle::init(
        store,
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T10:00:00Z",
    )
    .expect("initialize transactional CB7 bundle");
    (bundle, owner, entropy)
}

fn add_and_publish<S: Store>(
    bundle: &mut Bundle<S>,
    owner: &OwnerKeys,
    entropy: &mut SeqEntropy,
) -> Result<(), Error> {
    bundle.section_add(
        &SectionSpec {
            zone: Zone::Public,
            folder_path: "projets",
            name: "note",
            title: "CB7",
            tags: &[],
            body: "transactional body",
            now: "2026-07-18T10:01:00Z",
        },
        owner,
        entropy,
    )?;
    bundle.publish(owner, "2026-07-18T10:02:00Z")
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cb7-fsstore-tests")
            });
        std::fs::create_dir_all(&base).expect("create CB7 test base");
        for _ in 0..1024 {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "aithos-cb7-transaction-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create CB7 root {path:?}: {error}"),
            }
        }
        panic!("could not allocate CB7 root")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        let name = self
            .0
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let generations = self
            .0
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".{name}.aithos-generations"));
        let _ = std::fs::remove_dir_all(generations);
    }
}

#[test]
fn cb7_memstore_failure_boundaries_are_byte_exact() {
    let vector = vector();
    let old = snapshot(&vector["transaction"]["old_snapshot"]);
    let new = snapshot(&vector["transaction"]["new_snapshot"]);
    for case in vector["transaction"]["failure_cases"]
        .as_array()
        .expect("failure cases")
        .iter()
        .filter(|case| case["store"] == "MemStore")
    {
        let mut store = MemStore::default();
        install(&mut store, &old);
        replace_transaction(&mut store, &old, &new);
        store
            .rollback_transaction()
            .unwrap_or_else(|error| panic!("{} rollback: {error}", case["id"]));
        assert_eq!(read_snapshot(&store), old, "{}", case["id"]);
    }

    let mut store = MemStore::default();
    install(&mut store, &old);
    replace_transaction(&mut store, &old, &new);
    store.commit_transaction().expect("linearize MemStore");
    assert_eq!(read_snapshot(&store), new);
}

#[test]
fn cb7_fsstore_failure_recovery_and_reopen_are_byte_exact() {
    let vector = vector();
    let old = snapshot(&vector["transaction"]["old_snapshot"]);
    let new = snapshot(&vector["transaction"]["new_snapshot"]);
    for case in vector["transaction"]["failure_cases"]
        .as_array()
        .expect("failure cases")
        .iter()
        .filter(|case| case["store"] == "FsStore")
    {
        let root = TempRoot::new(case["id"].as_str().expect("case id"));
        let mut store = FsStore::new(root.path());
        install(&mut store, &old);
        replace_transaction(&mut store, &old, &new);
        store
            .rollback_transaction()
            .unwrap_or_else(|error| panic!("{} rollback: {error}", case["id"]));
        drop(store);

        let mut reopened = FsStore::new(root.path());
        reopened
            .recover_transaction()
            .unwrap_or_else(|error| panic!("{} recovery: {error}", case["id"]));
        assert_eq!(read_snapshot(&reopened), old, "{}", case["id"]);
    }

    let root = TempRoot::new("prepared-not-linearized");
    let mut prepared = FsStore::new(root.path());
    install(&mut prepared, &old);
    replace_transaction(&mut prepared, &old, &new);
    drop(prepared);
    let mut recovered = FsStore::new(root.path());
    recovered
        .recover_transaction()
        .expect("recover prepared transaction");
    assert_eq!(read_snapshot(&recovered), old);

    replace_transaction(&mut recovered, &old, &new);
    recovered
        .commit_transaction()
        .expect("linearize FsStore generation");
    drop(recovered);
    let mut reopened = FsStore::new(root.path());
    reopened
        .recover_transaction()
        .expect("recover acknowledged-or-lost commit");
    assert_eq!(read_snapshot(&reopened), new);
}

#[test]
fn cb7_bundle_mutation_and_publication_share_one_memstore_linearization() {
    let (mut bundle, owner, mut entropy) = initialized_bundle(MemStore::default());
    let old = read_snapshot(&bundle.store);

    let refused = bundle.transaction(|bundle| {
        bundle.section_add(
            &SectionSpec {
                zone: Zone::Public,
                folder_path: "projets",
                name: "refusee",
                title: "CB7 refusal",
                tags: &[],
                body: "must never become canonical",
                now: "2026-07-18T10:01:00Z",
            },
            &owner,
            &mut entropy,
        )?;
        Err::<(), _>(Error::SealRejected(
            "injected pre-linearization refusal".into(),
        ))
    });
    assert!(refused.is_err());
    assert_eq!(read_snapshot(&bundle.store), old);

    bundle
        .transaction(|bundle| add_and_publish(bundle, &owner, &mut entropy))
        .expect("commit complete mutation and publication");
    assert_ne!(read_snapshot(&bundle.store), old);
    bundle
        .verify()
        .expect("committed MemStore generation verifies");
}

#[test]
fn cb7_bundle_fsstore_commit_reopens_as_one_complete_generation() {
    let root = TempRoot::new("bundle-linearization");
    let (mut bundle, owner, mut entropy) = initialized_bundle(FsStore::new(root.path()));
    let old = read_snapshot(&bundle.store);
    bundle
        .transaction(|bundle| add_and_publish(bundle, &owner, &mut entropy))
        .expect("commit complete FsStore mutation");
    let committed = read_snapshot(&bundle.store);
    assert_ne!(committed, old);
    drop(bundle);

    let reopened = Bundle::open(FsStore::new(root.path())).expect("recover and reopen FsStore");
    assert_eq!(read_snapshot(&reopened.store), committed);
    reopened
        .verify()
        .expect("reopened complete generation verifies");
}

#[test]
fn cb7_vector_paths_reach_the_mandatory_confinement_gate() {
    let vector = vector();
    for case in vector["confinement"]["cases"]
        .as_array()
        .expect("confinement cases")
    {
        let value = case["value"].as_str().expect("path value");
        let result = match case["input_kind"].as_str().expect("input kind") {
            "display_path" => validate_display_path(value),
            "store_key" | "cold_load_key" | "recovery_key" => validate_store_key(value),
            other => panic!("unknown input kind {other}"),
        };
        if case["resolved_outside_root"] == true {
            continue;
        }
        assert_eq!(
            result.is_ok(),
            case["expected"] == "accepted",
            "{}",
            case["id"]
        );
    }
}

#[cfg(unix)]
#[test]
fn cb7_fsstore_rejects_intermediate_and_final_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = TempRoot::new("symlink");
    let outside = TempRoot::new("outside");
    std::fs::create_dir_all(root.path().join("e/circle")).expect("create canonical parent");

    symlink(outside.path(), root.path().join("e/circle/blobs"))
        .expect("install intermediate symlink");
    let mut store = FsStore::new(root.path());
    assert!(store
        .put("e/circle/blobs/01K00000000000000000000081.enc", b"escape",)
        .is_err());
    assert!(!outside
        .path()
        .join("01K00000000000000000000081.enc")
        .exists());

    symlink(
        outside.path().join("manifest.json"),
        root.path().join("manifest.json"),
    )
    .expect("install final symlink");
    assert!(store.get("manifest.json").is_err());
}
