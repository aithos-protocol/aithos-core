//! Deterministic regression tests for publication history cost (lot 0.6).
//!
//! Wall-clock measurements live in the manual probes and Criterion benches.
//! Normal `cargo test` instead checks the operation that caused the historical
//! regression: reading every immutable object from every prior edition.

use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::manifest::{sha256_hex, Manifest};
use aithos_bundle::{MemStore, Store};
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::path::Zone;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::io;

const NOW: &str = "2026-07-09T00:00:00Z";

#[derive(Debug, Default)]
struct ReadCountingStore {
    inner: MemStore,
    immutable_reads: Cell<usize>,
}

impl ReadCountingStore {
    fn count(&self, path: &str) {
        if path.starts_with("manifests/") || path.starts_with("certs/") {
            self.immutable_reads
                .set(self.immutable_reads.get().saturating_add(1));
        }
    }

    fn reset_immutable_reads(&self) {
        self.immutable_reads.set(0);
    }

    fn immutable_reads(&self) -> usize {
        self.immutable_reads.get()
    }
}

impl Store for ReadCountingStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        self.count(path);
        self.inner.get(path)
    }

    fn get_bounded(&self, path: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        self.count(path);
        self.inner.get_bounded(path, maximum)
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.inner.put(path, bytes)
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        self.inner.list(prefix)
    }

    fn delete(&mut self, path: &str) -> io::Result<()> {
        self.inner.delete(path)
    }

    fn begin_transaction(&mut self) -> io::Result<()> {
        self.inner.begin_transaction()
    }

    fn commit_transaction(&mut self) -> io::Result<()> {
        self.inner.commit_transaction()
    }

    fn rollback_transaction(&mut self) -> io::Result<()> {
        self.inner.rollback_transaction()
    }

    fn recover_transaction(&mut self) -> io::Result<()> {
        self.inner.recover_transaction()
    }

    fn transaction_active(&self) -> bool {
        self.inner.transaction_active()
    }
}

fn bundle_with_store<S: Store>(store: S, sections: usize) -> (Bundle<S>, OwnerKeys, SeqEntropy) {
    let owner =
        OwnerKeys::genesis(&MasterSeed::from_slice(&(0u8..32).collect::<Vec<u8>>()).unwrap());
    let succession = succession_from_entropy([9u8; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        store,
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        NOW,
    )
    .unwrap();
    bundle
        .ensure_folder(Zone::Circle, "projets", &owner, &mut entropy)
        .unwrap();
    for index in 0..sections {
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projets",
                    name: &format!("note{index}"),
                    title: "note",
                    tags: &[],
                    body: "Le corps de la note.",
                    now: NOW,
                },
                &owner,
                &mut entropy,
            )
            .unwrap();
    }
    bundle.publish(&owner, NOW).unwrap();
    (bundle, owner, entropy)
}

fn edit_and_publish<S: Store>(
    bundle: &mut Bundle<S>,
    owner: &OwnerKeys,
    entropy: &mut SeqEntropy,
    minute: usize,
) {
    let now = format!("2026-07-09T00:{minute:02}:00Z");
    bundle
        .section_rewrite(
            Zone::Circle,
            "projets/note0",
            &format!("corps modifie {minute}"),
            owner,
            &now,
            entropy,
        )
        .unwrap();
    bundle.publish(owner, &now).unwrap();
}

fn manifest<S: Store>(bundle: &Bundle<S>) -> (Manifest, Vec<u8>) {
    let bytes = bundle.store.get("manifest.json").unwrap().unwrap();
    (serde_json::from_slice(&bytes).unwrap(), bytes)
}

fn full_pin_scan<S: Store>(bundle: &Bundle<S>, height: u64) -> BTreeMap<String, String> {
    let current_history = format!("manifests/{height}.json");
    bundle
        .store
        .list("")
        .unwrap()
        .into_iter()
        .filter(|path| path != "manifest.json" && path != &current_history)
        .map(|path| {
            let bytes = bundle.store.get(&path).unwrap().unwrap();
            (path, sha256_hex(&bytes))
        })
        .collect()
}

#[test]
fn publication_does_not_reread_the_growing_immutable_history() {
    let (mut bundle, owner, mut entropy) = bundle_with_store(ReadCountingStore::default(), 3);

    // Four editions are enough to cross the carry-over threshold. Start the
    // comparison later so both observations use the same optimized path.
    for minute in 1..=8 {
        edit_and_publish(&mut bundle, &owner, &mut entropy, minute);
    }

    bundle.store.reset_immutable_reads();
    edit_and_publish(&mut bundle, &owner, &mut entropy, 9);
    let early_reads = bundle.store.immutable_reads();

    for minute in 10..=34 {
        edit_and_publish(&mut bundle, &owner, &mut entropy, minute);
    }

    bundle.store.reset_immutable_reads();
    edit_and_publish(&mut bundle, &owner, &mut entropy, 35);
    let late_reads = bundle.store.immutable_reads();
    let immutable_objects = bundle
        .store
        .list("")
        .unwrap()
        .into_iter()
        .filter(|path| path.starts_with("manifests/") || path.starts_with("certs/"))
        .count();

    assert_eq!(
        late_reads, early_reads,
        "immutable history reads grew from {early_reads} to {late_reads}"
    );
    assert!(
        late_reads.saturating_mul(10) < immutable_objects,
        "one publication read {late_reads} of {immutable_objects} immutable history objects"
    );
}

#[test]
fn carried_pins_equal_a_full_scan_on_an_append_only_store() {
    let (mut bundle, owner, mut entropy) = bundle_with_store(MemStore::default(), 8);
    for minute in 1..=12 {
        edit_and_publish(&mut bundle, &owner, &mut entropy, minute);
    }

    let (latest, latest_bytes) = manifest(&bundle);
    let history_bytes = bundle
        .store
        .get(&format!("manifests/{}.json", latest.edition.height))
        .unwrap()
        .unwrap();

    assert_eq!(latest.files, full_pin_scan(&bundle, latest.edition.height));
    assert_eq!(latest_bytes, history_bytes);
    bundle.verify().unwrap();
}

#[test]
fn an_overwritten_immutable_object_is_not_reblessed() {
    let (mut bundle, owner, mut entropy) = bundle_with_store(MemStore::default(), 3);
    for minute in 1..=8 {
        edit_and_publish(&mut bundle, &owner, &mut entropy, minute);
    }

    let immutable_path = "manifests/tree-1.json";
    let (before, _) = manifest(&bundle);
    let original_pin = before.files.get(immutable_path).unwrap().clone();
    bundle
        .store
        .put(immutable_path, b"overwritten immutable history")
        .unwrap();

    edit_and_publish(&mut bundle, &owner, &mut entropy, 9);

    let (after, _) = manifest(&bundle);
    assert_eq!(after.files.get(immutable_path), Some(&original_pin));
    assert_ne!(
        after.files.get(immutable_path),
        Some(&sha256_hex(b"overwritten immutable history"))
    );
    let error = bundle.verify().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("pinned file altered: manifests/tree-1.json"),
        "unexpected verification error: {error}"
    );
}
