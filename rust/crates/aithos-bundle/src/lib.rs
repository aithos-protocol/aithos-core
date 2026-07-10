#![forbid(unsafe_code)]
//! # aithos-bundle
//!
//! Bundle layout (spec §02.3) and storage. This is the only crate in the
//! workspace allowed to touch I/O; `aithos-core` stays pure. Backends
//! implement [`Store`]; `fs` ships here, `s3` will live behind a feature.

pub mod bundle;
pub mod entropy;
pub mod grants;
pub mod log;
pub mod manifest;
pub mod revoke;

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

/// Minimal object store the bundle is written through. Paths are the
/// bundle-relative file paths of spec §02.3 (`manifest.json`,
/// `e/circle/blobs/<sid>.enc`, `certs/<id>.json`, `gamma/gamma.jsonl`, …).
pub trait Store {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>>;
    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()>;
    fn list(&self, prefix: &str) -> io::Result<Vec<String>>;
}

/// In-memory store for tests and vector replay.
#[derive(Debug, Default)]
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

/// Filesystem store: the bundle as real files under a root directory.
#[derive(Debug)]
pub struct FsStore {
    pub root: PathBuf,
}

impl FsStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FsStore { root: root.into() }
    }

    fn collect(&self, dir: &std::path::Path, out: &mut Vec<String>) -> io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                self.collect(&path, out)?;
            } else if let Ok(rel) = path.strip_prefix(&self.root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
}

impl Store for FsStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(self.root.join(path)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        let full = self.root.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, bytes)
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        let mut out = Vec::new();
        self.collect(&self.root.clone(), &mut out)?;
        out.retain(|p| p.starts_with(prefix));
        out.sort();
        Ok(out)
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
