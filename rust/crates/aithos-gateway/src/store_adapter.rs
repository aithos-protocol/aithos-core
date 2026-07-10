//! Store adapter: turns the declared `StoreConfig` into a concrete
//! `aithos_bundle::Store` for the ethos (bundle + gamma).
//!
//! Part of the core-facing seam: like `core_bridge`, this module may
//! import from `aithos-bundle`. Decided 2026-07-10: local disk first,
//! cloud stays structurally possible (the S3 variant parses; the adapter
//! refuses it until the S3 backend exists — fail-closed, not silent).

use aithos_bundle::{FsStore, MemStore, Store};

use crate::config::StoreConfig;
use crate::{GatewayError, Result};

/// The stores the gateway can run on today.
pub enum GatewayStore {
    Fs(FsStore),
    /// In-memory store — tests and dry-runs only.
    Mem(MemStore),
}

impl GatewayStore {
    /// Build the store the config asks for; refuse what v1 cannot honour.
    pub fn from_config(cfg: &StoreConfig) -> Result<Self> {
        match cfg {
            StoreConfig::Fs { root } => Ok(GatewayStore::Fs(FsStore::new(root.clone()))),
            StoreConfig::S3 { .. } => Err(GatewayError::ConfigRejected(
                "store kind `s3` is not available in v1 — use `fs` (cloud lands in a later iteration)".into(),
            )),
        }
    }

    /// In-memory store for tests.
    pub fn in_memory() -> Self {
        GatewayStore::Mem(MemStore::default())
    }
}

impl Store for GatewayStore {
    fn get(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            GatewayStore::Fs(s) => s.get(path),
            GatewayStore::Mem(s) => s.get(path),
        }
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            GatewayStore::Fs(s) => s.put(path, bytes),
            GatewayStore::Mem(s) => s.put(path, bytes),
        }
    }

    fn list(&self, prefix: &str) -> std::io::Result<Vec<String>> {
        match self {
            GatewayStore::Fs(s) => s.list(prefix),
            GatewayStore::Mem(s) => s.list(prefix),
        }
    }
}
