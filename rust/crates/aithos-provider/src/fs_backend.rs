//! Durable local backends used by the real-process E2E harness.
//!
//! These implementations stay behind the production seams and are selected
//! explicitly with the `filesystem` backend plus `AITHOS_STORE_FS_ROOT`.
//! They are not a new wire surface and never contain client secrets.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::heads::{HeadsFuture, HeadsRecord, HeadsTable, HeadsUnavailable};
use crate::nonces::{NonceStore, NonceStoreUnavailable, Reservation, MIN_WINDOW_SECS};
use crate::objects::{ObjectStore, PutOnce, StoreFuture, StoreUnavailable};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn encoded(value: &str) -> String {
    hex::encode(value.as_bytes())
}

fn decoded(value: &str) -> Result<String, ()> {
    String::from_utf8(hex::decode(value).map_err(|_| ())?).map_err(|_| ())
}

fn ensure_directory(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    ensure_directory(parent)?;
    let temporary = parent.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

/// Opaque object bytes stored under traversal-safe hexadecimal names.
pub struct FsObjects {
    root: PathBuf,
}

impl FsObjects {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into().join("objects"),
        }
    }

    fn did_directory(&self, tenant: &str, did: &str) -> PathBuf {
        self.root.join(encoded(tenant)).join(encoded(did))
    }

    fn object_path(&self, tenant: &str, did: &str, chemin: &str) -> PathBuf {
        self.did_directory(tenant, did)
            .join(format!("{}.bin", encoded(chemin)))
    }
}

impl ObjectStore for FsObjects {
    fn get<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
    ) -> StoreFuture<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            match fs::read(self.object_path(tenant, did, chemin)) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
                Err(_) => Err(StoreUnavailable),
            }
        })
    }

    fn put<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, ()> {
        Box::pin(async move {
            atomic_write(&self.object_path(tenant, did, chemin), &bytes)
                .map_err(|_| StoreUnavailable)
        })
    }

    fn put_once<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, PutOnce> {
        Box::pin(async move {
            let path = self.object_path(tenant, did, chemin);
            ensure_directory(path.parent().ok_or(StoreUnavailable)?)
                .map_err(|_| StoreUnavailable)?;
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(&bytes).map_err(|_| StoreUnavailable)?;
                    file.sync_all().map_err(|_| StoreUnavailable)?;
                    Ok(PutOnce::Stored)
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    let stored = fs::read(path).map_err(|_| StoreUnavailable)?;
                    Ok(if stored == bytes {
                        PutOnce::Identical
                    } else {
                        PutOnce::Conflict
                    })
                }
                Err(_) => Err(StoreUnavailable),
            }
        })
    }

    fn list<'a>(&'a self, tenant: &'a str, did: &'a str) -> StoreFuture<'a, Vec<String>> {
        Box::pin(async move {
            let directory = self.did_directory(tenant, did);
            let entries = match fs::read_dir(directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
                Err(_) => return Err(StoreUnavailable),
            };
            let mut paths = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|_| StoreUnavailable)?;
                let name = entry.file_name();
                let name = name.to_str().ok_or(StoreUnavailable)?;
                let stem = name.strip_suffix(".bin").ok_or(StoreUnavailable)?;
                paths.push(decoded(stem).map_err(|_| StoreUnavailable)?);
            }
            paths.sort();
            Ok(paths)
        })
    }
}

#[derive(Serialize, Deserialize)]
struct DiskHeads {
    height: u64,
    manifest_chain_hash: String,
    gamma_head: String,
    gamma_segment: String,
    gamma_segments: Vec<String>,
}

impl From<HeadsRecord> for DiskHeads {
    fn from(record: HeadsRecord) -> Self {
        Self {
            height: record.height,
            manifest_chain_hash: record.manifest_chain_hash,
            gamma_head: record.gamma_head,
            gamma_segment: record.gamma_segment,
            gamma_segments: record.gamma_segments,
        }
    }
}

impl From<DiskHeads> for HeadsRecord {
    fn from(record: DiskHeads) -> Self {
        Self {
            height: record.height,
            manifest_chain_hash: record.manifest_chain_hash,
            gamma_head: record.gamma_head,
            gamma_segment: record.gamma_segment,
            gamma_segments: record.gamma_segments,
        }
    }
}

/// Single-host durable CAS used by the local E2E provider process.
pub struct FsHeads {
    root: PathBuf,
    lock: Mutex<()>,
}

impl FsHeads {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into().join("heads"),
            lock: Mutex::new(()),
        }
    }

    fn record_path(&self, tenant: &str, did: &str) -> PathBuf {
        self.root
            .join(encoded(tenant))
            .join(format!("{}.json", encoded(did)))
    }

    fn read_record(
        &self,
        tenant: &str,
        did: &str,
    ) -> Result<Option<HeadsRecord>, HeadsUnavailable> {
        match fs::read(self.record_path(tenant, did)) {
            Ok(bytes) => serde_json::from_slice::<DiskHeads>(&bytes)
                .map(HeadsRecord::from)
                .map(Some)
                .map_err(|_| HeadsUnavailable),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(_) => Err(HeadsUnavailable),
        }
    }
}

impl HeadsTable for FsHeads {
    fn read<'a>(&'a self, tenant: &'a str, did: &'a str) -> HeadsFuture<'a, Option<HeadsRecord>> {
        Box::pin(async move {
            let _guard = self.lock.lock().map_err(|_| HeadsUnavailable)?;
            self.read_record(tenant, did)
        })
    }

    fn cas<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        expected: Option<&'a HeadsRecord>,
        next: HeadsRecord,
    ) -> HeadsFuture<'a, Result<(), Option<HeadsRecord>>> {
        Box::pin(async move {
            let _guard = self.lock.lock().map_err(|_| HeadsUnavailable)?;
            let current = self.read_record(tenant, did)?;
            if current.as_ref() != expected {
                return Ok(Err(current));
            }
            let bytes = serde_json::to_vec(&DiskHeads::from(next)).map_err(|_| HeadsUnavailable)?;
            atomic_write(&self.record_path(tenant, did), &bytes).map_err(|_| HeadsUnavailable)?;
            Ok(Ok(()))
        })
    }
}

/// Durable insert-if-absent nonce reservations for process restarts.
pub struct FsNonces {
    root: PathBuf,
    window_ms: i64,
    lock: Mutex<()>,
}

impl FsNonces {
    pub fn new(root: impl Into<PathBuf>, window_secs: i64) -> Self {
        Self {
            root: root.into().join("nonces"),
            window_ms: window_secs.max(MIN_WINDOW_SECS) * 1000,
            lock: Mutex::new(()),
        }
    }

    fn reservation_path(&self, key: &str, nonce: &str) -> PathBuf {
        self.root
            .join(encoded(key))
            .join(format!("{}.txt", encoded(nonce)))
    }
}

impl NonceStore for FsNonces {
    fn reserve<'a>(
        &'a self,
        key: &'a str,
        nonce: &'a str,
        now_ms: i64,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Reservation, NonceStoreUnavailable>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let _guard = self.lock.lock().map_err(|_| NonceStoreUnavailable)?;
            let path = self.reservation_path(key, nonce);
            ensure_directory(path.parent().ok_or(NonceStoreUnavailable)?)
                .map_err(|_| NonceStoreUnavailable)?;
            match fs::read_to_string(&path) {
                Ok(expiry) => {
                    let expiry = expiry.parse::<i64>().map_err(|_| NonceStoreUnavailable)?;
                    if expiry >= now_ms {
                        return Ok(Reservation::Replayed);
                    }
                    fs::remove_file(&path).map_err(|_| NonceStoreUnavailable)?;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(_) => return Err(NonceStoreUnavailable),
            }
            let expiry = now_ms + self.window_ms;
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)
                .map_err(|_| NonceStoreUnavailable)?;
            write!(file, "{expiry}").map_err(|_| NonceStoreUnavailable)?;
            file.sync_all().map_err(|_| NonceStoreUnavailable)?;
            Ok(Reservation::Fresh)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aithos-provider-fs-{name}-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn objects_heads_and_nonces_survive_reconstruction() {
        futures::executor::block_on(async {
            let root = root("reopen");
            let objects = FsObjects::new(&root);
            assert_eq!(
                objects
                    .put_once("acme", "did:aithos:test", "manifest.json", b"one".to_vec())
                    .await,
                Ok(PutOnce::Stored)
            );
            let heads = FsHeads::new(&root);
            let record = HeadsRecord {
                height: 1,
                manifest_chain_hash: "abc".into(),
                ..HeadsRecord::default()
            };
            assert_eq!(
                heads
                    .cas("acme", "did:aithos:test", None, record.clone())
                    .await,
                Ok(Ok(()))
            );
            let nonces = FsNonces::new(&root, MIN_WINDOW_SECS);
            assert_eq!(
                nonces.reserve("key", "nonce", 0).await.unwrap(),
                Reservation::Fresh
            );
            drop((objects, heads, nonces));

            assert_eq!(
                FsObjects::new(&root)
                    .get("acme", "did:aithos:test", "manifest.json")
                    .await,
                Ok(Some(b"one".to_vec()))
            );
            assert_eq!(
                FsHeads::new(&root).read("acme", "did:aithos:test").await,
                Ok(Some(record))
            );
            assert_eq!(
                FsNonces::new(&root, MIN_WINDOW_SECS)
                    .reserve("key", "nonce", 1)
                    .await
                    .unwrap(),
                Reservation::Replayed
            );
            let _ = fs::remove_dir_all(root);
        });
    }
}
