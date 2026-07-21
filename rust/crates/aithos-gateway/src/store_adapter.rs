//! Store adapter: turns the declared `StoreConfig` into a concrete
//! `aithos_bundle::Store` for the ethos (bundle + gamma).
//!
//! Part of the core-facing seam: like `core_bridge`, this module may
//! import from `aithos-bundle`. Decided 2026-07-10: local disk first,
//! cloud stays structurally possible (the S3 variant parses; the adapter
//! refuses it until the S3 backend exists — fail-closed, not silent).
//!
//! P3 (2026-07-21, arbitrages Mathieu ①②④): the cloud landed as the
//! wire A.2 client (`aithos_bundle::remote`, feature `remote`):
//! - `remote { url, tenant, did, mandate }` — mode B, provider-primary
//!   (INFRA-PROVIDER §3.5): every read/write speaks the signed wire; a
//!   provider outage FAILS CLOSED (the journal's discipline);
//! - `replicated { root, url, … }` — mode A, local-primary: fs answers,
//!   the provider receives an asynchronous post-publish replication —
//!   a provider outage never blocks the agent.
//!
//! The envelope signer is INJECTED from the runner's keyholder (the
//! agent leaf key + the configured mandate chain) — never a key in the
//! config, never an owner key in the gateway (the doctrine line: owner
//! keys live client-side; the gateway holds agent+gateway seeds only).
//! The `s3` refusal stays as-is.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aithos_bundle::entropy::EntropySource;
use aithos_bundle::remote::{KeySigner, RemoteError, RemoteStore, SharedRemoteStore};
use aithos_bundle::{FsStore, MemStore, Store};

use crate::config::StoreConfig;
use crate::keyholder::Keyholder;
use crate::{GatewayError, Result};

/// The stores the gateway can run on today. Cloneable: fs clones reopen
/// the same root, memory clones share the same map (so owner-side setup
/// and runner-side reads in tests see one store, like a real disk);
/// remote clones share ONE wire identity (tracked heads + cache), and a
/// replicated clone shares the same replication state.
pub enum GatewayStore {
    Fs(PathBuf),
    /// In-memory store — tests and dry-runs only.
    Mem(Arc<Mutex<MemStore>>),
    /// P3 mode B — provider-primary over the wire A.2, with a LOCAL
    /// sidecar for the runner/derived keys the wire deliberately
    /// excludes (arbitrage hybride 2026-07-21: gateway/**, manifests/*
    /// never leave the pod; everything protocolar rides the wire).
    Remote {
        remote: SharedRemoteStore,
        sidecar: Sidecar,
    },
    /// P3 mode A — fs primary + asynchronous post-publish replication.
    Replicated {
        root: PathBuf,
        remote: SharedRemoteStore,
        replication: Arc<ReplicationState>,
    },
}

impl Clone for GatewayStore {
    fn clone(&self) -> Self {
        match self {
            GatewayStore::Fs(root) => GatewayStore::Fs(root.clone()),
            GatewayStore::Mem(m) => GatewayStore::Mem(Arc::clone(m)),
            GatewayStore::Remote { remote, sidecar } => GatewayStore::Remote {
                remote: remote.clone(),
                sidecar: sidecar.clone(),
            },
            GatewayStore::Replicated {
                root,
                remote,
                replication,
            } => GatewayStore::Replicated {
                root: root.clone(),
                remote: remote.clone(),
                replication: Arc::clone(replication),
            },
        }
    }
}

/// The mode-B sidecar: where the runner-local and derived keys live
/// (never the wire). Fs when the config names a local root, memory
/// otherwise (an ephemeral runner re-derives them).
#[derive(Clone)]
pub enum Sidecar {
    Fs(PathBuf),
    Mem(Arc<Mutex<MemStore>>),
}

impl Sidecar {
    fn get(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            Sidecar::Fs(root) => FsStore::new(root.clone()).get(path),
            Sidecar::Mem(m) => m.lock().expect("sidecar").get(path),
        }
    }
    fn get_bounded(&self, path: &str, maximum: usize) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            Sidecar::Fs(root) => FsStore::new(root.clone()).get_bounded(path, maximum),
            Sidecar::Mem(m) => m.lock().expect("sidecar").get_bounded(path, maximum),
        }
    }
    fn put(&self, path: &str, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Sidecar::Fs(root) => FsStore::new(root.clone()).put(path, bytes),
            Sidecar::Mem(m) => m.lock().expect("sidecar").put(path, bytes),
        }
    }
    fn list(&self, prefix: &str) -> std::io::Result<Vec<String>> {
        match self {
            Sidecar::Fs(root) => FsStore::new(root.clone()).list(prefix),
            Sidecar::Mem(m) => m.lock().expect("sidecar").list(prefix),
        }
    }
}

/// The wire A.1 exclusions the pod keeps to itself: runner state and
/// derived caches (micro-redline P3 kept them OUT deliberately —
/// `gateway/**` is the runner's, `manifests/*` are server-written slots
/// or re-derivable trees/indexes).
fn sidecar_key(path: &str) -> bool {
    path.starts_with("gateway/") || path.starts_with("manifests/")
}

/// Mode-A bookkeeping: which paths changed since the last replication
/// sweep, and the in-flight background sweep (joined by tests and by
/// the drain path — errors are LOGGED, never propagated: the primary
/// already answered, that is what mode A means).
#[derive(Default)]
pub struct ReplicationState {
    dirty: Mutex<Vec<String>>,
    inflight: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl GatewayStore {
    /// Build the store the config asks for; refuse what v1 cannot honour.
    /// The remote kinds need the runner identity — this constructor
    /// refuses them fail-closed (owner CLI paths are fs-only by design).
    pub fn from_config(cfg: &StoreConfig) -> Result<Self> {
        match cfg {
            StoreConfig::Fs { root } => Ok(GatewayStore::Fs(root.clone())),
            StoreConfig::S3 { .. } => Err(GatewayError::ConfigRejected(
                "store kind `s3` is not available in v1 — use `fs` (cloud lands in a later iteration)".into(),
            )),
            StoreConfig::Remote { .. } | StoreConfig::Replicated { .. } => {
                Err(GatewayError::ConfigRejected(
                    "a remote store needs the runner identity — this path builds stores without a keyholder (owner flows stay local)".into(),
                ))
            }
        }
    }

    /// Build any configured store, remote kinds included: the envelope
    /// signer is the AGENT leaf from the keyholder plus the configured
    /// mandate chain (arbitrage ②: a seam, never a key in the config).
    pub fn from_config_with_identity(
        cfg: &StoreConfig,
        keyholder: &Keyholder,
        entropy: impl FnOnce() -> Box<dyn EntropySource + Send>,
    ) -> Result<Self> {
        match cfg {
            StoreConfig::Fs { .. } | StoreConfig::S3 { .. } => Self::from_config(cfg),
            StoreConfig::Remote {
                url,
                tenant,
                did,
                mandate,
                local,
            } => Ok(GatewayStore::Remote {
                remote: Self::remote_client(
                    url,
                    tenant,
                    did,
                    mandate.clone(),
                    keyholder,
                    entropy(),
                )?,
                sidecar: match local {
                    Some(root) => Sidecar::Fs(root.clone()),
                    None => Sidecar::Mem(Arc::new(Mutex::new(MemStore::default()))),
                },
            }),
            StoreConfig::Replicated {
                root,
                url,
                tenant,
                did,
                mandate,
            } => Ok(GatewayStore::Replicated {
                root: root.clone(),
                remote: Self::remote_client(
                    url,
                    tenant,
                    did,
                    mandate.clone(),
                    keyholder,
                    entropy(),
                )?,
                replication: Arc::new(ReplicationState::default()),
            }),
        }
    }

    fn remote_client(
        url: &str,
        tenant: &str,
        did: &str,
        mandate: Vec<String>,
        keyholder: &Keyholder,
        entropy: Box<dyn EntropySource + Send>,
    ) -> Result<SharedRemoteStore> {
        let agent_sk = ed25519_dalek::SigningKey::from_bytes(keyholder.agent_seed());
        let signer = Arc::new(KeySigner::mandated(agent_sk, mandate));
        let store = RemoteStore::new(
            url,
            tenant,
            did,
            signer,
            Arc::new(system_now_rfc3339),
            entropy,
        )
        .map_err(|e| GatewayError::ConfigRejected(format!("remote store: {e}")))?;
        Ok(SharedRemoteStore::new(store))
    }

    /// In-memory store for tests.
    pub fn in_memory() -> Self {
        GatewayStore::Mem(Arc::new(Mutex::new(MemStore::default())))
    }

    fn fs(root: &std::path::Path) -> FsStore {
        FsStore::new(root.to_path_buf())
    }

    /// Mode A: run one FULL replication sweep NOW, synchronously —
    /// did.json first, artifacts next, gamma segments as replica PUTs,
    /// the manifest publish LAST (the A.5 order: sidecars land before
    /// the publish that pins them). Errors surface to the caller: this
    /// entry point is the deliberate sweep (seeding, drain, tests) —
    /// only the post-publish HOOK is fire-and-forget.
    pub fn replicate_now(&self) -> std::io::Result<()> {
        let GatewayStore::Replicated { root, remote, .. } = self else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "replicate_now: not a replicated store",
            ));
        };
        replicate_paths(&Self::fs(root), remote, None)
    }

    /// Mode A: wait for the in-flight post-publish sweep, if any.
    pub fn join_replication(&self) {
        if let GatewayStore::Replicated { replication, .. } = self {
            let handle = replication.inflight.lock().expect("replication").take();
            if let Some(handle) = handle {
                let _ = handle.join();
            }
        }
    }
}

/// Replicate `paths` (or, when `None`, every path of the primary) onto
/// the remote, in the wire's publication order.
fn replicate_paths(
    primary: &FsStore,
    remote: &SharedRemoteStore,
    paths: Option<Vec<String>>,
) -> std::io::Result<()> {
    let mut all = match paths {
        Some(paths) => paths,
        None => primary.list("")?,
    };
    all.sort();
    all.dedup();
    let priority = |p: &str| -> u8 {
        match p {
            "did.json" => 0,
            p if p.starts_with("gamma/") => 2,
            "manifest.json" => 3,
            _ => 1,
        }
    };
    all.sort_by_key(|p| priority(p));
    let mut remote = remote.clone();
    for path in all {
        let Some(bytes) = primary.get(&path)? else {
            continue; // deleted between the snapshot and the sweep
        };
        if path.starts_with("gamma/") {
            // Prime the client's diff base: a single appended entry then
            // rides POST /gamma (the delegated-writable hot path) instead
            // of the owner-only segment replica PUT.
            let _ = remote.get(&path);
        }
        remote.put(&path, &bytes)?;
    }
    Ok(())
}

/// Result of one deliberate owner-side history replay. This promotes the
/// P3 DEMO-LEA seeding seam from the test harness to operator tooling.
/// Counts are safe to print; no key or payload is ever returned.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OwnerReplicationReport {
    pub protocol_objects: usize,
    pub editions: usize,
    pub gamma_segments: usize,
    pub unchanged: usize,
}

/// Replay one locally provisioned owner store through the provider wire.
///
/// Ordering is contractual: `did.json` first, protocol objects next,
/// gamma segments after their diff base is primed, then every saved
/// edition in ascending height order. Runner-only state (`gateway/**`)
/// and local edition slots (`manifests/**`) never leave the machine.
/// The supplied client must carry an owner signer.
pub fn replicate_owner_history(
    local_root: &std::path::Path,
    remote: &mut RemoteStore,
) -> std::io::Result<OwnerReplicationReport> {
    let primary = FsStore::new(local_root.to_path_buf());
    let mut paths = primary.list("")?;
    paths.sort();
    paths.dedup();
    if !paths.iter().any(|path| path == "did.json") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "owner replication: local store has no did.json",
        ));
    }

    let priority = |path: &str| -> u8 {
        match path {
            "did.json" => 0,
            path if path.starts_with("gamma/") => 2,
            "manifest.json" => 3,
            _ => 1,
        }
    };
    paths.sort_by_key(|path| priority(path));
    let mut heights: Vec<u64> = paths
        .iter()
        .filter_map(|path| {
            path.strip_prefix("manifests/")?
                .strip_suffix(".json")?
                .parse()
                .ok()
        })
        .collect();
    heights.sort_unstable();
    heights.dedup();

    // A fresh DID cannot authenticate a GET before its self-signed
    // genesis document exists. An existing DID, however, must never be
    // re-deposited: compare its parsed document and skip it.
    let did_doc = primary.get("did.json")?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "owner replication: local store has no did.json",
        )
    })?;
    let mut report = OwnerReplicationReport {
        protocol_objects: 0,
        editions: 0,
        gamma_segments: 0,
        unchanged: 0,
    };
    match remote.get("did.json") {
        Ok(Some(existing)) => {
            let local_value =
                serde_json::from_slice::<serde_json::Value>(&did_doc).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "owner replication: local did.json is not JSON",
                    )
                })?;
            let remote_value =
                serde_json::from_slice::<serde_json::Value>(&existing).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "owner replication: remote did.json is not JSON",
                    )
                })?;
            if local_value != remote_value {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "owner replication: remote did.json differs from local",
                ));
            }
            report.unchanged += 1;
        }
        Ok(None) => {
            remote.put("did.json", &did_doc).map_err(|error| {
                std::io::Error::new(error.kind(), format!("owner replication did.json: {error}"))
            })?;
            report.protocol_objects += 1;
        }
        Err(error)
            if matches!(
                error
                    .get_ref()
                    .and_then(|source| source.downcast_ref::<RemoteError>()),
                Some(RemoteError::Wire { status: 403, code, .. }) if code == "chain_invalid"
            ) =>
        {
            remote.put("did.json", &did_doc).map_err(|put_error| {
                std::io::Error::new(
                    put_error.kind(),
                    format!("owner replication did.json: {put_error}"),
                )
            })?;
            report.protocol_objects += 1;
        }
        Err(error) => return Err(error),
    }

    let remote_manifest = remote.get("manifest.json")?;
    let manifest_height = |bytes: &[u8]| -> std::io::Result<u64> {
        serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|value| value.pointer("/edition/height")?.as_u64())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "owner replication: manifest has no edition.height",
                )
            })
    };
    let remote_height = remote_manifest
        .as_deref()
        .map(manifest_height)
        .transpose()?;
    if let (Some(remote_height), Some(local_height)) = (remote_height, heights.last().copied()) {
        if remote_height > local_height {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "owner replication: remote edition {remote_height} is ahead of local {local_height}"
                ),
            ));
        }
    }

    for path in paths {
        if sidecar_key(&path) {
            continue;
        }
        if path == "did.json" {
            continue;
        }
        let Some(bytes) = primary.get(&path)? else {
            continue;
        };
        if path.starts_with("gamma/") && remote.get(&path)?.as_deref() == Some(bytes.as_slice()) {
            report.unchanged += 1;
            continue;
        }
        if path == "manifest.json" {
            if heights.is_empty() {
                if remote_manifest.as_deref() == Some(bytes.as_slice()) {
                    report.unchanged += 1;
                } else {
                    remote.put("manifest.json", &bytes).map_err(|error| {
                        std::io::Error::new(
                            error.kind(),
                            format!("owner replication manifest.json: {error}"),
                        )
                    })?;
                    report.editions += 1;
                }
            } else {
                for height in heights
                    .iter()
                    .filter(|height| remote_height.is_none_or(|remote| **height > remote))
                {
                    let slot = primary
                        .get(&format!("manifests/{height}.json"))?
                        .ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("owner replication: missing edition slot {height}"),
                            )
                        })?;
                    remote.put("manifest.json", &slot).map_err(|error| {
                        std::io::Error::new(
                            error.kind(),
                            format!("owner replication edition {height}: {error}"),
                        )
                    })?;
                    report.editions += 1;
                }
            }
            continue;
        }
        if remote.get(&path)?.as_deref() == Some(bytes.as_slice()) {
            report.unchanged += 1;
            continue;
        }
        remote.put(&path, &bytes).map_err(|error| {
            std::io::Error::new(error.kind(), format!("owner replication {path}: {error}"))
        })?;
        report.protocol_objects += 1;
        if path.starts_with("gamma/") {
            report.gamma_segments += 1;
        }
    }
    Ok(report)
}

/// UTC now, RFC 3339 Zulu, whole seconds — the envelope `at` (A.2). The
/// gateway binary runs on the real clock; tests inject nothing here
/// because the counterparty (the real service) runs on ITS real clock
/// too — skew ~0 is exactly the deployed situation.
fn system_now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days — exact for the proleptic
    // Gregorian calendar.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod / 60) % 60,
        tod % 60
    )
}

impl Store for GatewayStore {
    fn get(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).get(path),
            GatewayStore::Mem(s) => s.lock().expect("store lock").get(path),
            GatewayStore::Remote { remote, sidecar } => {
                if sidecar_key(path) {
                    sidecar.get(path)
                } else {
                    remote.get(path)
                }
            }
            // Mode A: the PRIMARY answers reads — that is the point.
            GatewayStore::Replicated { root, .. } => Self::fs(root).get(path),
        }
    }

    fn get_bounded(&self, path: &str, maximum: usize) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).get_bounded(path, maximum),
            GatewayStore::Mem(store) => {
                store.lock().expect("store lock").get_bounded(path, maximum)
            }
            GatewayStore::Remote { remote, sidecar } => {
                if sidecar_key(path) {
                    sidecar.get_bounded(path, maximum)
                } else {
                    remote.get_bounded(path, maximum)
                }
            }
            GatewayStore::Replicated { root, .. } => Self::fs(root).get_bounded(path, maximum),
        }
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).put(path, bytes),
            GatewayStore::Mem(s) => s.lock().expect("store lock").put(path, bytes),
            GatewayStore::Remote { remote, sidecar } => {
                if sidecar_key(path) {
                    sidecar.put(path, bytes)
                } else {
                    remote.put(path, bytes)
                }
            }
            GatewayStore::Replicated {
                root,
                remote,
                replication,
            } => {
                // Primary first — mode A never blocks on the provider.
                Self::fs(root).put(path, bytes)?;
                replication
                    .dirty
                    .lock()
                    .expect("replication")
                    .push(path.to_owned());
                if path == "manifest.json" || path.starts_with("gamma/") {
                    // The asynchronous replication (§3.5): post-publish,
                    // and post-append (a journal-shaped ethos may never
                    // publish between beats — its appends must still
                    // reach the provider and its witness hook):
                    // snapshot the dirty set, sweep it in the background,
                    // log-and-carry-on on failure (the primary answered).
                    let batch: Vec<String> = {
                        let mut dirty = replication.dirty.lock().expect("replication");
                        std::mem::take(&mut *dirty)
                    };
                    let primary = Self::fs(root);
                    let remote = remote.clone();
                    let handle = std::thread::spawn(move || {
                        if let Err(e) = replicate_paths(&primary, &remote, Some(batch)) {
                            // Mode A by design: the primary answered; the
                            // sweep failure is operational noise, retried
                            // by the next publish or a deliberate sweep.
                            eprintln!("mode A replication sweep failed (primary unaffected): {e}");
                        }
                    });
                    let previous = replication
                        .inflight
                        .lock()
                        .expect("replication")
                        .replace(handle);
                    if let Some(previous) = previous {
                        let _ = previous.join();
                    }
                }
                Ok(())
            }
        }
    }

    fn list(&self, prefix: &str) -> std::io::Result<Vec<String>> {
        match self {
            GatewayStore::Fs(root) => Self::fs(root).list(prefix),
            GatewayStore::Mem(s) => s.lock().expect("store lock").list(prefix),
            GatewayStore::Remote { remote, sidecar } => {
                // Both worlds under one prefix: the wire's listing plus
                // the pod's own keys, deduplicated, lexicographic.
                let mut all = remote.list(prefix)?;
                all.extend(sidecar.list(prefix)?);
                all.sort();
                all.dedup();
                Ok(all)
            }
            GatewayStore::Replicated { root, .. } => Self::fs(root).list(prefix),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_still_refused_and_remote_needs_identity() {
        let s3 = StoreConfig::S3 {
            bucket: "b".into(),
            prefix: None,
        };
        assert!(GatewayStore::from_config(&s3).is_err(), "s3 stays refused");
        let remote = StoreConfig::Remote {
            url: "https://store.aithos.fr".into(),
            tenant: "acme".into(),
            did: "did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr".into(),
            mandate: vec![],
            local: None,
        };
        let refused = GatewayStore::from_config(&remote);
        assert!(
            refused.is_err(),
            "the identity-less path refuses remote fail-closed"
        );
    }

    #[test]
    fn remote_builds_with_identity() {
        let keyholder = Keyholder::from_entropy([7u8; 32], [8u8; 32]);
        let remote = StoreConfig::Remote {
            url: "https://store.aithos.fr".into(),
            tenant: "acme".into(),
            did: "did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr".into(),
            mandate: vec!["mandate_0000000000000000000000P0M1".into()],
            local: None,
        };
        let built = GatewayStore::from_config_with_identity(&remote, &keyholder, || {
            Box::new(aithos_bundle::entropy::SeqEntropy::default())
        });
        assert!(built.is_ok(), "remote builds from the keyholder seam");
        assert!(matches!(built.unwrap(), GatewayStore::Remote { .. }));
    }

    #[test]
    fn now_renders_rfc3339_zulu() {
        let now = system_now_rfc3339();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z') && now.contains('T'));
        assert!(now.starts_with("20"), "{now}");
    }
}
