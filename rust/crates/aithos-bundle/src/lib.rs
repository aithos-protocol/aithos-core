#![forbid(unsafe_code)]
//! # aithos-bundle
//!
//! Bundle layout (spec §02.3) and storage. This is the only crate in the
//! workspace allowed to touch I/O; `aithos-core` stays pure. Backends
//! implement [`Store`]; `fs` ships here, `s3` will live behind a feature.

use std::collections::BTreeMap;
use std::io;

/// Minimal object store the bundle is written through. Paths are the
/// bundle-relative file paths of spec §02.3 (`manifest.json`,
/// `e/circle/blobs/<sid>.enc`, `certs/<id>.json`, `gamma/gamma.jsonl`, …).
pub trait Store {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>>;
    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()>;
    fn list(&self, prefix: &str) -> io::Result<Vec<String>>;
}

/// In-memory store for tests and vector replay.
#[derive(Default)]
pub struct MemStore {
    objects: BTreeMap<String, Vec<u8>>,
}

impl Store for MemStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        Ok(self.objects.get(path).cloned())
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.objects.insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        Ok(self
            .objects
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memstore_roundtrip() {
        let mut s = MemStore::default();
        s.put("manifest.json", b"{}").unwrap();
        assert_eq!(s.get("manifest.json").unwrap().unwrap(), b"{}");
        assert_eq!(s.list("mani").unwrap(), vec!["manifest.json".to_owned()]);
        assert!(s.get("absent").unwrap().is_none());
    }
}
