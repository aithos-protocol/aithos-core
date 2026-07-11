//! Store adapter: turns the declared `StoreConfig` into a concrete
//! `aithos_bundle::Store` for the ethos (bundle + gamma).
//!
//! Part of the core-facing seam: like `core_bridge`, this module may
//! import from `aithos-bundle`. Decided 2026-07-10: local disk first,
//! cloud stays structurally possible (the S3 variant parses; the adapter
//! refuses it until the S3 backend exists — fail-closed, not silent).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aithos_bundle::{FsStore, MemStore, Store};

use crate::config::StoreConfig;
use crate::{GatewayError, Result};

/// The stores the gateway can run on today. Cloneable: fs clones reopen
/// the same root, memory clones share the same map (so owner-side setup
/// and runner-side reads in tests see one store, like a real disk).
pub enum GatewayStore {
    Fs(PathBuf),
    /// In-memory store — tests and dry-runs only.
    Mem(Arc<Mutex<MemStore>>),
}

impl Clone for GatewayStore {
    fn clone(&self) -> Self {
        match self {
            GatewayStore::Fs(root) => GatewayStore::Fs(root.clone()),
            GatewayStore::Mem(m) => GatewayStore::Mem(Arc::clone(m)),
        }
    }
}

impl GatewayStore {
    /// Build the store the config asks for; refuse what v1 cannot honour.
    pub fn from_config(cfg: &StoreConfig) -> Result<Self> {
        match cfg {
            StoreConfig::Fs { root } => Ok(GatewayStore::Fs(root.clone())),
            StoreConfig::S3 { .. } => Err(GatewayError::ConfigRejected(
                "store kind `s3` is not available in v1 — use `fs` (cloud lands in a later iteration)".into(),
            )),
        }
    }

    /// In-memory store for tests.
    pub fn in_memory() -> Self {
        GatewayStore::Mem(Arc::new(Mutex::new(MemStore::default())))
    }

    fn fs(root: &PathBuf) -> FsStore {
        FsStore::new(root.clone())
    }
}

impl Store for GatewayStore {
    fn get(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).get(path),
            GatewayStore::Mem(s) => s.lock().expect("store lock").get(path),
        }
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).put(path, bytes),
            GatewayStore::Mem(s) => s.lock().expect("store lock").put(path, bytes),
        }
    }

    fn list(&self, prefix: &str) -> std::io::Result<Vec<String>> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).list(prefix),
            GatewayStore::Mem(s) => s.lock().expect("store lock").list(prefix),
        }
    }
}
