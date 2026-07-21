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
pub mod merge;
pub mod publication;
/// P3 — the wire A.2 client (`RemoteStore`), behind the `remote` feature:
/// the network stays out of the default graph, the core stays pure.
#[cfg(feature = "remote")]
pub mod remote;
pub mod revoke;
pub mod sdk;
pub mod session;
pub mod state;
pub mod structure;
pub mod vault;

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);

fn invalid_path(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn confinement_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.into())
}

fn name_accepted(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn sid_accepted(value: &str) -> bool {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    value.len() == 26 && value.bytes().all(|byte| ALPHABET.contains(&byte))
}

fn hash_accepted(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn short_hash_accepted(value: &str) -> bool {
    value.len() == 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn relative_segments(value: &str) -> io::Result<Vec<&str>> {
    if value.is_empty() || value.starts_with(['/', '\\']) || value.contains(['\\', '\0']) {
        return Err(invalid_path("path must be a non-empty relative POSIX path"));
    }
    let segments: Vec<_> = value.split('/').collect();
    if segments
        .iter()
        .any(|segment| matches!(*segment, "" | "." | ".."))
    {
        return Err(invalid_path(
            "path contains an empty, dot, or parent segment",
        ));
    }
    Ok(segments)
}

/// Validate a user-visible section or folder path before it is resolved.
///
/// Display paths are deliberately narrower than host filesystem paths:
/// lowercase ASCII names separated by `/`, with no empty, dot, parent,
/// absolute, backslash, or NUL form.
pub fn validate_display_path(value: &str) -> io::Result<()> {
    let segments = relative_segments(value)?;
    if segments.iter().all(|segment| name_accepted(segment)) {
        Ok(())
    } else {
        Err(invalid_path("display path contains an unsupported name"))
    }
}

fn manifest_stem_accepted(stem: &str) -> bool {
    let stem = stem.strip_suffix("-alt").unwrap_or(stem);
    if stem.bytes().all(|byte| byte.is_ascii_digit()) && !stem.is_empty() {
        return true;
    }
    if let Some(height) = stem.strip_prefix("tree-") {
        return !height.is_empty() && height.bytes().all(|byte| byte.is_ascii_digit());
    }
    ["index-public-", "index-circle-", "index-self-"]
        .iter()
        .any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|height| {
                !height.is_empty() && height.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
}

fn connector_object_accepted(segments: &[&str]) -> bool {
    if segments.len() < 3
        || !segments[1]
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        || !name_accepted(segments[1])
        || !segments[2..segments.len() - 1]
            .iter()
            .all(|segment| name_accepted(segment))
    {
        return false;
    }
    segments
        .last()
        .and_then(|last| {
            last.strip_suffix(".enc")
                .or_else(|| last.strip_suffix(".json"))
        })
        .is_some_and(name_accepted)
}

/// Validate one canonical Bundle object key.
///
/// This is a closed grammar, not merely a traversal check. A signed
/// manifest therefore cannot make an arbitrary host path into a valid
/// Bundle object.
pub fn validate_store_key(value: &str) -> io::Result<()> {
    let segments = relative_segments(value)?;
    let accepted = matches!(value, "manifest.json" | "did.json")
        || matches!(
            value,
            "e/public/index.json"
                | "e/circle/index.json"
                | "e/self/index.json"
                | "e/public/header.json"
                | "e/circle/header.json"
                | "e/self/header.json"
                | "e/self/root.enc"
                | "e/x/header.json"
                | "gamma/gamma.jsonl"
                | "gateway/state.json"
                | "gateway/keys.json"
        )
        || value
            .strip_prefix("e/public/")
            .and_then(|rest| rest.strip_suffix(".md"))
            .is_some_and(|path| validate_display_path(path).is_ok())
        || (segments.len() == 4
            && segments[0] == "e"
            && matches!(segments[1], "circle" | "self")
            && segments[2] == "blobs"
            && segments[3].strip_suffix(".enc").is_some_and(sid_accepted))
        || (segments.len() == 4
            && segments[0] == "e"
            && matches!(segments[1], "public" | "circle" | "self")
            && segments[2] == "hdr"
            && segments[3].strip_suffix(".json").is_some_and(|stem| {
                stem == "root" || sid_accepted(stem) || short_hash_accepted(stem)
            }))
        || (segments.len() == 4
            && segments[0] == "e"
            && matches!(segments[1], "public" | "circle" | "self")
            && segments[2] == "wraps"
            && segments[3]
                .strip_suffix(".json")
                .is_some_and(short_hash_accepted))
        || (segments.len() == 2
            && segments[0] == "certs"
            && segments[1]
                .strip_suffix(".json")
                .and_then(|stem| stem.strip_prefix("mandate_"))
                .is_some_and(sid_accepted))
        || (segments.len() == 2
            && segments[0] == "gamma"
            && (segments[1] == "gamma.jsonl"
                || segments[1].strip_suffix(".jsonl").is_some_and(|stem| {
                    let bytes = stem.as_bytes();
                    bytes.len() == 7
                        && bytes[0..4].iter().all(u8::is_ascii_digit)
                        && bytes[4] == b'-'
                        && bytes[5..7].iter().all(u8::is_ascii_digit)
                })))
        || (segments.len() == 2
            && segments[0] == "manifests"
            && segments[1]
                .strip_suffix(".json")
                .is_some_and(manifest_stem_accepted))
        || (segments.len() == 2
            && matches!(segments[0], "changesets" | "evidence")
            && segments[1].strip_suffix(".json").is_some_and(hash_accepted))
        || (segments[0] == "x" && connector_object_accepted(&segments))
        || (segments.len() == 4
            && segments[0] == "e"
            && segments[1] == "x"
            && name_accepted(segments[2])
            && matches!(segments[3], "header.json" | "manifest.enc"))
        // Frozen K1-C draft.2 carrier layout. These aliases are an additive,
        // closed grammar; historical `e/*` objects remain byte-identical.
        || (segments.len() == 3
            && segments[0] == "public"
            && segments[1] == "sections"
            && segments[2].strip_suffix(".md").is_some_and(sid_accepted))
        || (segments.len() == 3
            && segments[0] == "circle"
            && segments[1] == "blobs"
            && segments[2].strip_suffix(".json").is_some_and(sid_accepted))
        || matches!(
            value,
            "indices/public.json" | "roots/public.json" | "vault/catalog-pins.json"
        );
    if accepted {
        Ok(())
    } else {
        Err(invalid_path(format!(
            "path is outside the canonical Bundle object grammar: {value}"
        )))
    }
}

fn validate_store_prefix(prefix: &str) -> io::Result<()> {
    if prefix.starts_with(['/', '\\']) || prefix.contains(['\\', '\0']) {
        return Err(invalid_path("store prefix must be relative"));
    }
    if prefix
        .split('/')
        .any(|segment| matches!(segment, "." | ".."))
    {
        return Err(invalid_path(
            "store prefix contains a dot or parent segment",
        ));
    }
    Ok(())
}

/// Minimal object store the bundle is written through. Paths are the
/// bundle-relative file paths of spec §02.3 (`manifest.json`,
/// `e/circle/blobs/<sid>.enc`, `certs/<id>.json`, `gamma/gamma.jsonl`, …).
pub trait Store {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>>;

    /// Return an object only when it fits `maximum`. Backends should override
    /// this when they can reject before allocating the complete object; the
    /// default remains fail-closed for existing bounded transports.
    fn get_bounded(&self, path: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        let bytes = self.get(path)?;
        if bytes.as_ref().is_some_and(|bytes| bytes.len() > maximum) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "store object exceeds the caller's byte bound",
            ));
        }
        Ok(bytes)
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()>;
    fn list(&self, prefix: &str) -> io::Result<Vec<String>>;

    fn delete(&mut self, _path: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "store does not support transactional deletion",
        ))
    }

    fn begin_transaction(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "store does not support transactions",
        ))
    }

    fn commit_transaction(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "store does not support transactions",
        ))
    }

    fn rollback_transaction(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn recover_transaction(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn transaction_active(&self) -> bool {
        false
    }
}

/// In-memory store for tests and vector replay. `Clone` gives the
/// "two copies of a published bundle" fixture of the concurrency pass (§02.6).
#[derive(Debug, Default, Clone)]
pub struct MemStore {
    objects: BTreeMap<String, Vec<u8>>,
    overlay: Option<BTreeMap<String, Vec<u8>>>,
}

impl MemStore {
    fn visible_objects(&self) -> &BTreeMap<String, Vec<u8>> {
        self.overlay.as_ref().unwrap_or(&self.objects)
    }

    fn writable_objects(&mut self) -> &mut BTreeMap<String, Vec<u8>> {
        self.overlay.as_mut().unwrap_or(&mut self.objects)
    }
}

impl Store for MemStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        validate_store_key(path)?;
        Ok(self.visible_objects().get(path).cloned())
    }

    fn get_bounded(&self, path: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        validate_store_key(path)?;
        let bytes = self.visible_objects().get(path);
        if bytes.is_some_and(|bytes| bytes.len() > maximum) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "store object exceeds the caller's byte bound",
            ));
        }
        Ok(bytes.cloned())
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        validate_store_key(path)?;
        self.writable_objects()
            .insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        validate_store_prefix(prefix)?;
        Ok(self
            .visible_objects()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }

    fn delete(&mut self, path: &str) -> io::Result<()> {
        validate_store_key(path)?;
        self.writable_objects().remove(path);
        Ok(())
    }

    fn begin_transaction(&mut self) -> io::Result<()> {
        if self.overlay.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a MemStore transaction is already active",
            ));
        }
        self.overlay = Some(self.objects.clone());
        Ok(())
    }

    fn commit_transaction(&mut self) -> io::Result<()> {
        let overlay = self.overlay.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no MemStore transaction is active")
        })?;
        self.objects = overlay;
        Ok(())
    }

    fn rollback_transaction(&mut self) -> io::Result<()> {
        self.overlay = None;
        Ok(())
    }

    fn recover_transaction(&mut self) -> io::Result<()> {
        self.overlay = None;
        Ok(())
    }

    fn transaction_active(&self) -> bool {
        self.overlay.is_some()
    }
}

/// Filesystem store: the bundle as real files under a root directory.
#[derive(Debug)]
pub struct FsStore {
    pub root: PathBuf,
    transaction: Option<FsTransaction>,
}

#[derive(Debug)]
struct FsTransaction {
    generation: String,
    staging: PathBuf,
}

impl FsStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FsStore {
            root: root.into(),
            transaction: None,
        }
    }

    fn generations_dir(&self) -> io::Result<PathBuf> {
        Ok(self.root.join(".aithos-generations"))
    }

    fn pointer_path(&self) -> PathBuf {
        self.root.join(".aithos-current")
    }

    fn mirror_marker_path(&self) -> PathBuf {
        self.root.join(".aithos-mirror-current")
    }

    fn ensure_plain_directory(path: &Path) -> io::Result<()> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(confinement_error(format!(
                "refusing symlink directory: {}",
                path.display()
            ))),
            Ok(metadata) if !metadata.is_dir() => Err(confinement_error(format!(
                "expected directory: {}",
                path.display()
            ))),
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir_all(path)?;
                let metadata = std::fs::symlink_metadata(path)?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(confinement_error(format!(
                        "directory escaped through a symlink: {}",
                        path.display()
                    )));
                }
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn read_pointer(&self) -> io::Result<Option<String>> {
        self.read_generation_marker(&self.pointer_path())
    }

    fn read_mirror_marker(&self) -> io::Result<Option<String>> {
        self.read_generation_marker(&self.mirror_marker_path())
    }

    fn read_generation_marker(&self, marker: &Path) -> io::Result<Option<String>> {
        let metadata = match std::fs::symlink_metadata(marker) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(confinement_error("transaction pointer is not a plain file"));
        }
        let bytes = std::fs::read(marker)?;
        let generation = std::str::from_utf8(&bytes)
            .map_err(|_| invalid_path("transaction pointer is not UTF-8"))?;
        if !generation.starts_with("generation-")
            || generation.len() > 96
            || !generation
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(invalid_path(
                "transaction pointer contains an invalid generation",
            ));
        }
        Ok(Some(generation.to_owned()))
    }

    fn write_generation_marker(
        &self,
        marker: &Path,
        prefix: &str,
        generation: &str,
    ) -> io::Result<()> {
        let (temporary, mut file) = loop {
            let serial = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
            let temporary = self
                .root
                .join(format!("{prefix}-{}-{serial}", std::process::id()));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };
        use std::io::Write as _;
        if let Err(error) = file
            .write_all(generation.as_bytes())
            .and_then(|()| file.sync_all())
        {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        drop(file);
        if let Err(error) = std::fs::rename(&temporary, marker) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        std::fs::File::open(&self.root)?.sync_all()
    }

    fn canonical_base(&self) -> io::Result<PathBuf> {
        if let Some(transaction) = &self.transaction {
            return Ok(transaction.staging.clone());
        }
        let Some(generation) = self.read_pointer()? else {
            return Ok(self.root.clone());
        };
        let base = self.generations_dir()?.join(generation);
        let metadata = std::fs::symlink_metadata(&base).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "transaction pointer references a missing generation",
                )
            } else {
                error
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(confinement_error(
                "transaction pointer references a non-directory generation",
            ));
        }
        Ok(base)
    }

    fn checked_join(base: &Path, key: &str) -> io::Result<PathBuf> {
        validate_store_key(key)?;
        if let Ok(metadata) = std::fs::symlink_metadata(base) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(confinement_error(format!(
                    "store base is not a plain directory: {}",
                    base.display()
                )));
            }
        }
        let mut path = base.to_path_buf();
        for segment in key.split('/') {
            path.push(segment);
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(confinement_error(format!(
                        "store path crosses a symlink: {}",
                        path.display()
                    )));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(path)
    }

    fn collect_from(base: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
        let metadata = match std::fs::symlink_metadata(dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() {
            return Err(confinement_error(format!(
                "store listing crosses a symlink: {}",
                dir.display()
            )));
        }
        if !metadata.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .map_err(|_| confinement_error("listed object escaped the store base"))?;
            if relative
                .components()
                .next()
                .and_then(|component| component.as_os_str().to_str())
                .is_some_and(|name| name.starts_with(".aithos-"))
            {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(confinement_error(format!(
                    "store listing found a symlink: {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                Self::collect_from(base, &path, out)?;
            } else if metadata.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|_| confinement_error("listed object escaped the store base"))?;
                let key = rel
                    .to_str()
                    .ok_or_else(|| invalid_path("store object path is not UTF-8"))?
                    .replace('\\', "/");
                if key.starts_with(".aithos-") {
                    continue;
                }
                validate_store_key(&key)?;
                out.push(key);
            }
        }
        Ok(())
    }

    fn copy_snapshot(source: &Path, destination: &Path) -> io::Result<()> {
        let mut keys = Vec::new();
        Self::collect_from(source, source, &mut keys)?;
        keys.sort();
        for key in keys {
            let source_file = Self::checked_join(source, &key)?;
            let destination_file = Self::checked_join(destination, &key)?;
            if let Some(parent) = destination_file.parent() {
                Self::ensure_plain_directory(parent)?;
            }
            std::fs::copy(source_file, destination_file)?;
        }
        Ok(())
    }

    fn materialize_compatibility_mirror(&self, active: &Path) -> io::Result<()> {
        if active == self.root {
            return Ok(());
        }
        let mut active_keys = Vec::new();
        Self::collect_from(active, active, &mut active_keys)?;
        active_keys.sort();
        let active_set: std::collections::BTreeSet<_> = active_keys.iter().cloned().collect();

        let mut mirror_keys = Vec::new();
        Self::collect_from(&self.root, &self.root, &mut mirror_keys)?;
        mirror_keys.sort();
        for key in mirror_keys {
            if !active_set.contains(&key) {
                let target = Self::checked_join(&self.root, &key)?;
                match std::fs::remove_file(target) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
        for key in active_keys {
            let source = Self::checked_join(active, &key)?;
            let target = Self::checked_join(&self.root, &key)?;
            if let Some(parent) = target.parent() {
                Self::ensure_plain_directory(parent)?;
            }
            let target = Self::checked_join(&self.root, &key)?;
            std::fs::copy(source, target)?;
        }
        std::fs::File::open(&self.root)?.sync_all()
    }

    fn reconcile_compatibility_mirror(&self, active: &Path) -> io::Result<()> {
        let mut mirror_keys = Vec::new();
        Self::collect_from(&self.root, &self.root, &mut mirror_keys)?;
        mirror_keys.sort();
        let mirror_set: std::collections::BTreeSet<_> = mirror_keys.iter().cloned().collect();

        let mut active_keys = Vec::new();
        Self::collect_from(active, active, &mut active_keys)?;
        active_keys.sort();
        for key in active_keys {
            if !mirror_set.contains(&key) {
                let target = Self::checked_join(active, &key)?;
                match std::fs::remove_file(target) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
        for key in mirror_keys {
            let source = Self::checked_join(&self.root, &key)?;
            let target = Self::checked_join(active, &key)?;
            if let Some(parent) = target.parent() {
                Self::ensure_plain_directory(parent)?;
            }
            let target = Self::checked_join(active, &key)?;
            std::fs::copy(source, target)?;
        }
        Self::sync_tree(active)
    }

    fn sync_tree(dir: &Path) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(confinement_error(format!(
                    "transaction generation contains a symlink: {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                Self::sync_tree(&path)?;
                std::fs::File::open(&path)?.sync_all()?;
            } else if metadata.is_file() {
                std::fs::File::open(&path)?.sync_all()?;
            }
        }
        std::fs::File::open(dir)?.sync_all()
    }

    fn remove_internal_path(path: &Path) -> io::Result<()> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
                std::fs::remove_file(path)
            }
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path),
            Ok(_) => Err(confinement_error("unsupported internal transaction object")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Store for FsStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        let base = self.canonical_base()?;
        let full = Self::checked_join(&base, path)?;
        match std::fs::read(full) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn get_bounded(&self, path: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        use std::io::Read as _;

        let base = self.canonical_base()?;
        let full = Self::checked_join(&base, path)?;
        let file = match std::fs::File::open(full) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let length = file.metadata()?.len();
        if length > maximum as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "store object exceeds the caller's byte bound",
            ));
        }
        let mut bytes = Vec::with_capacity(length as usize);
        file.take((maximum as u64).saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "store object exceeds the caller's byte bound",
            ));
        }
        Ok(Some(bytes))
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        let base = self.canonical_base()?;
        Self::ensure_plain_directory(&base)?;
        let full = Self::checked_join(&base, path)?;
        if let Some(parent) = full.parent() {
            Self::ensure_plain_directory(parent)?;
        }
        if self.transaction.is_none() && base != self.root {
            let mirror = Self::checked_join(&self.root, path)?;
            if let Some(parent) = mirror.parent() {
                Self::ensure_plain_directory(parent)?;
            }
            let mirror = Self::checked_join(&self.root, path)?;
            std::fs::write(mirror, bytes)?;
        }
        let full = Self::checked_join(&base, path)?;
        std::fs::write(full, bytes)
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        validate_store_prefix(prefix)?;
        let base = self.canonical_base()?;
        let mut out = Vec::new();
        Self::collect_from(&base, &base, &mut out)?;
        out.retain(|p| p.starts_with(prefix));
        out.sort();
        Ok(out)
    }

    fn delete(&mut self, path: &str) -> io::Result<()> {
        let base = self.canonical_base()?;
        if self.transaction.is_none() && base != self.root {
            let mirror = Self::checked_join(&self.root, path)?;
            match std::fs::remove_file(mirror) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        let full = Self::checked_join(&base, path)?;
        match std::fs::remove_file(full) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn begin_transaction(&mut self) -> io::Result<()> {
        if self.transaction.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "an FsStore transaction is already active",
            ));
        }
        let source = self.canonical_base()?;
        Self::ensure_plain_directory(&source)?;
        let generations = self.generations_dir()?;
        Self::ensure_plain_directory(&generations)?;
        let (generation, staging) = loop {
            let serial = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
            let generation = format!("generation-{}-{serial}", std::process::id());
            let staging = generations.join(&generation);
            match std::fs::create_dir(&staging) {
                Ok(()) => break (generation, staging),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };
        if let Err(error) = Self::copy_snapshot(&source, &staging) {
            let _ = Self::remove_internal_path(&staging);
            return Err(error);
        }
        self.transaction = Some(FsTransaction {
            generation,
            staging,
        });
        Ok(())
    }

    fn commit_transaction(&mut self) -> io::Result<()> {
        let transaction = self.transaction.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no FsStore transaction is active")
        })?;
        Self::sync_tree(&transaction.staging)?;
        Self::ensure_plain_directory(&self.root)?;
        let pointer = self.pointer_path();
        match std::fs::symlink_metadata(&pointer) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(confinement_error(
                    "transaction pointer cannot be replaced safely",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.write_generation_marker(&pointer, ".aithos-current.tmp", &transaction.generation)?;
        let active = transaction.staging.clone();
        let generation = transaction.generation.clone();
        self.transaction = None;
        self.materialize_compatibility_mirror(&active)?;
        self.write_generation_marker(
            &self.mirror_marker_path(),
            ".aithos-mirror-current.tmp",
            &generation,
        )?;
        Ok(())
    }

    fn rollback_transaction(&mut self) -> io::Result<()> {
        if let Some(transaction) = self.transaction.take() {
            Self::remove_internal_path(&transaction.staging)?;
        }
        Ok(())
    }

    fn recover_transaction(&mut self) -> io::Result<()> {
        self.rollback_transaction()?;
        Self::ensure_plain_directory(&self.root)?;
        let active = self.read_pointer()?;
        let generations = self.generations_dir()?;
        match std::fs::symlink_metadata(&generations) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(confinement_error(
                    "transaction generations root is not a plain directory",
                ));
            }
            Ok(_) => {
                for entry in std::fs::read_dir(&generations)? {
                    let entry = entry?;
                    let name = entry
                        .file_name()
                        .to_str()
                        .ok_or_else(|| invalid_path("generation name is not UTF-8"))?
                        .to_owned();
                    if !name.starts_with("generation-")
                        || !name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    {
                        return Err(confinement_error(
                            "unexpected object in transaction generations root",
                        ));
                    }
                    if active.as_deref() != Some(name.as_str()) {
                        Self::remove_internal_path(&entry.path())?;
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if active.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "transaction pointer references a missing generations root",
                    ));
                }
            }
            Err(error) => return Err(error),
        }
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_name().to_str().is_some_and(|name| {
                name.starts_with(".aithos-current.tmp-")
                    || name.starts_with(".aithos-mirror-current.tmp-")
            }) {
                Self::remove_internal_path(&entry.path())?;
            }
        }
        if let Some(active_generation) = active {
            let canonical = self.canonical_base()?;
            if self.read_mirror_marker()?.as_deref() == Some(active_generation.as_str()) {
                self.reconcile_compatibility_mirror(&canonical)?;
            } else {
                self.materialize_compatibility_mirror(&canonical)?;
                self.write_generation_marker(
                    &self.mirror_marker_path(),
                    ".aithos-mirror-current.tmp",
                    &active_generation,
                )?;
            }
        }
        Ok(())
    }

    fn transaction_active(&self) -> bool {
        self.transaction.is_some()
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
        assert_eq!(s.get_bounded("manifest.json", 2).unwrap().unwrap(), b"{}");
        assert_eq!(
            s.get_bounded("manifest.json", 1).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(s.list("mani").unwrap(), vec!["manifest.json".to_owned()]);
        assert!(s.get("manifests/999.json").unwrap().is_none());
    }
}
