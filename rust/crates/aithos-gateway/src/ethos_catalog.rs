//! Durable, non-secret overlay for Ethos contexts admitted at runtime.
//!
//! The catalogue is local runner state, never a protocol proof. Every entry is
//! admitted only after Core has verified the already-published DID and mandate
//! chains. Persistence is crash-consistent (write + fsync + rename + parent
//! fsync), bounded and symlink-hostile.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::config::{GatewayConfig, StoreConfig};
use crate::core_bridge::{
    agent_kex_pub_multibase, agent_pub_multibase, gateway_kex_pub_multibase, gateway_pub_multibase,
    render_rfc3339z, Bridge, ControlProofReader, OsEntropy, Runner, STATE_PATH,
};
use crate::keyholder::Keyholder;
use crate::store_adapter::GatewayStore;
use crate::{GatewayError, Result};

pub const ETHOS_CATALOG_VERSION: u8 = 1;
const MAX_CATALOG_BYTES: usize = 512 * 1024;
const CATALOG_FILE: &str = "catalog.json";
const CONTEXTS_DIRECTORY: &str = "contexts";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

pub const REQUIRED_AGENT_ACTIONS: [&str; 1] = ["act.x.gateway.remote_read"];
pub const REQUIRED_GATEWAY_ACTIONS: [&str; 5] = [
    "act.x.gateway.connector_binding",
    "act.x.gateway.connector_config",
    "act.x.gateway.connector_effect",
    "act.x.gateway.oauth_issue",
    "act.x.gateway.refuse",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayPublicIdentity {
    pub v: u8,
    pub agent: GatewayPublicKeyIdentity,
    pub gateway: GatewayPublicKeyIdentity,
    pub audience: String,
    pub required_actions: GatewayRequiredActions,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayPublicKeyIdentity {
    pub signing: String,
    pub key_exchange: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayRequiredActions {
    pub agent: Vec<String>,
    pub gateway: Vec<String>,
}

#[derive(Serialize)]
struct GatewayIdentityDigestMaterial<'a> {
    v: u8,
    agent: &'a GatewayPublicKeyIdentity,
    gateway: &'a GatewayPublicKeyIdentity,
    audience: &'a str,
    required_actions: &'a GatewayRequiredActions,
}

impl GatewayPublicIdentity {
    pub fn new(keyholder: &Keyholder, audience: impl Into<String>) -> Result<Self> {
        let agent = GatewayPublicKeyIdentity {
            signing: agent_pub_multibase(keyholder),
            key_exchange: agent_kex_pub_multibase(keyholder),
        };
        let gateway = GatewayPublicKeyIdentity {
            signing: gateway_pub_multibase(keyholder),
            key_exchange: gateway_kex_pub_multibase(keyholder),
        };
        let audience = audience.into();
        let required_actions = GatewayRequiredActions {
            agent: REQUIRED_AGENT_ACTIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            gateway: REQUIRED_GATEWAY_ACTIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        };
        let material = GatewayIdentityDigestMaterial {
            v: ETHOS_CATALOG_VERSION,
            agent: &agent,
            gateway: &gateway,
            audience: &audience,
            required_actions: &required_actions,
        };
        let canonical = serde_jcs::to_vec(&material)
            .map_err(|error| GatewayError::ConfigRejected(format!("gateway identity: {error}")))?;
        let digest = format!("b3:{}", blake3::hash(&canonical).to_hex());
        Ok(Self {
            v: ETHOS_CATALOG_VERSION,
            agent,
            gateway,
            audience,
            required_actions,
            digest,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EthosEnrollmentRequest {
    pub v: u8,
    pub name: String,
    pub did: String,
    pub agent_mandate: String,
    pub gateway_mandate: String,
    pub gateway_identity_digest: String,
}

impl EthosEnrollmentRequest {
    pub fn validate_shape(&self) -> bool {
        self.v == ETHOS_CATALOG_VERSION
            && canonical_label(&self.name)
            && canonical_did(&self.did)
            && canonical_mandate_id(&self.agent_mandate)
            && canonical_mandate_id(&self.gateway_mandate)
            && self.agent_mandate != self.gateway_mandate
            && canonical_identity_digest(&self.gateway_identity_digest)
    }

    fn entry(&self) -> EthosCatalogEntry {
        EthosCatalogEntry {
            name: self.name.clone(),
            did: self.did.clone(),
            agent_mandate: self.agent_mandate.clone(),
            gateway_mandate: self.gateway_mandate.clone(),
            gateway_identity_digest: self.gateway_identity_digest.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EthosEnrollmentResponse {
    pub v: u8,
    pub name: String,
    pub did: String,
    pub status: &'static str,
    pub created: bool,
}

impl EthosEnrollmentResponse {
    fn active(entry: &EthosCatalogEntry, created: bool) -> Self {
        Self {
            v: ETHOS_CATALOG_VERSION,
            name: entry.name.clone(),
            did: entry.did.clone(),
            status: "active",
            created,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EthosCatalogEntry {
    pub name: String,
    pub did: String,
    pub agent_mandate: String,
    pub gateway_mandate: String,
    pub gateway_identity_digest: String,
}

impl EthosCatalogEntry {
    fn validate(&self) -> bool {
        EthosEnrollmentRequest {
            v: ETHOS_CATALOG_VERSION,
            name: self.name.clone(),
            did: self.did.clone(),
            agent_mandate: self.agent_mandate.clone(),
            gateway_mandate: self.gateway_mandate.clone(),
            gateway_identity_digest: self.gateway_identity_digest.clone(),
        }
        .validate_shape()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EthosCatalogFile {
    pub v: u8,
    pub entries: Vec<EthosCatalogEntry>,
}

impl Default for EthosCatalogFile {
    fn default() -> Self {
        Self {
            v: ETHOS_CATALOG_VERSION,
            entries: Vec::new(),
        }
    }
}

impl EthosCatalogFile {
    fn validate(&self, maximum: usize) -> Result<()> {
        if self.v != ETHOS_CATALOG_VERSION || self.entries.len() > maximum {
            return Err(catalog_invalid());
        }
        let mut names = BTreeSet::new();
        let mut dids = BTreeSet::new();
        let mut previous = None;
        for entry in &self.entries {
            if !entry.validate()
                || !names.insert(entry.name.as_str())
                || !dids.insert(entry.did.as_str())
                || previous.is_some_and(|name: &str| name >= entry.name.as_str())
            {
                return Err(catalog_invalid());
            }
            previous = Some(entry.name.as_str());
        }
        Ok(())
    }

    fn exact(&self, request: &EthosEnrollmentRequest) -> Option<&EthosCatalogEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name == request.name || entry.did == request.did)
            .filter(|entry| **entry == request.entry())
    }

    fn conflicts(&self, request: &EthosEnrollmentRequest) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.name == request.name || entry.did == request.did)
            && self.exact(request).is_none()
    }

    fn inserted(&self, entry: EthosCatalogEntry, maximum: usize) -> Result<Self> {
        if self.entries.len() >= maximum {
            return Err(GatewayError::RequestRejected("context_conflict".into()));
        }
        let mut next = self.clone();
        next.entries.push(entry);
        next.entries
            .sort_by(|left, right| left.name.cmp(&right.name));
        next.validate(maximum)?;
        Ok(next)
    }
}

#[derive(Clone)]
pub struct EthosCatalogStore {
    backing: CatalogBacking,
    maximum: usize,
}

#[derive(Clone)]
enum CatalogBacking {
    Fs { root: PathBuf, path: PathBuf },
    Memory(Arc<StdMutex<Option<Vec<u8>>>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogPersistenceFault {
    BeforeRename,
    AfterRename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CatalogCommitFailure {
    DefinitelyNotCommitted,
    OutcomeUncertain,
}

impl EthosCatalogStore {
    pub fn from_root(root: PathBuf, maximum: usize) -> Result<Self> {
        if !root.is_absolute() || root.parent().is_none() || maximum == 0 {
            return Err(GatewayError::ConfigRejected(
                "ethos catalogue root or bound is invalid".into(),
            ));
        }
        Ok(Self {
            backing: CatalogBacking::Fs {
                path: root.join(CATALOG_FILE),
                root,
            },
            maximum,
        })
    }

    pub fn memory(maximum: usize) -> Self {
        Self {
            backing: CatalogBacking::Memory(Arc::new(StdMutex::new(None))),
            maximum,
        }
    }

    pub fn load(&self) -> Result<EthosCatalogFile> {
        let bytes = match &self.backing {
            CatalogBacking::Fs { root, path } => load_fs(root, path)?,
            CatalogBacking::Memory(value) => value.lock().map_err(|_| catalog_invalid())?.clone(),
        };
        let file = match bytes {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| catalog_invalid())?,
            None => EthosCatalogFile::default(),
        };
        file.validate(self.maximum)?;
        Ok(file)
    }

    pub fn persist(&self, file: &EthosCatalogFile) -> Result<()> {
        self.persist_with_fault(file, None)
    }

    pub fn persist_with_fault_for_test(
        &self,
        file: &EthosCatalogFile,
        fault: CatalogPersistenceFault,
    ) -> Result<()> {
        self.persist_with_fault(file, Some(fault))
    }

    fn persist_with_fault(
        &self,
        file: &EthosCatalogFile,
        fault: Option<CatalogPersistenceFault>,
    ) -> Result<()> {
        file.validate(self.maximum)?;
        let bytes = serde_json::to_vec(file).map_err(|_| catalog_invalid())?;
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(catalog_invalid());
        }
        match &self.backing {
            CatalogBacking::Memory(value) => {
                if fault == Some(CatalogPersistenceFault::BeforeRename) {
                    return Err(catalog_unavailable());
                }
                *value.lock().map_err(|_| catalog_unavailable())? = Some(bytes);
                if fault == Some(CatalogPersistenceFault::AfterRename) {
                    return Err(catalog_unavailable());
                }
                Ok(())
            }
            CatalogBacking::Fs { root, path } => {
                persist_fs(root, path, &bytes, fault).map_err(|_| catalog_unavailable())
            }
        }
    }

    fn persist_for_enrollment(
        &self,
        file: &EthosCatalogFile,
    ) -> std::result::Result<(), CatalogCommitFailure> {
        file.validate(self.maximum)
            .map_err(|_| CatalogCommitFailure::DefinitelyNotCommitted)?;
        let bytes =
            serde_json::to_vec(file).map_err(|_| CatalogCommitFailure::DefinitelyNotCommitted)?;
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(CatalogCommitFailure::DefinitelyNotCommitted);
        }
        match &self.backing {
            CatalogBacking::Memory(value) => {
                *value
                    .lock()
                    .map_err(|_| CatalogCommitFailure::DefinitelyNotCommitted)? = Some(bytes);
                Ok(())
            }
            CatalogBacking::Fs { root, path } => persist_fs(root, path, &bytes, None),
        }
    }

    pub fn context_sidecar(&self, did: &str) -> Result<PathBuf> {
        let CatalogBacking::Fs { root, .. } = &self.backing else {
            return Err(GatewayError::ConfigRejected(
                "in-memory catalogue has no context sidecar".into(),
            ));
        };
        let digest = blake3::hash(did.as_bytes()).to_hex().to_string();
        Ok(root.join(CONTEXTS_DIRECTORY).join(digest))
    }

    fn prepare_context_sidecar(&self, did: &str) -> Result<(PathBuf, bool)> {
        let CatalogBacking::Fs { root, .. } = &self.backing else {
            return Err(GatewayError::ConfigRejected(
                "in-memory catalogue has no context sidecar".into(),
            ));
        };
        prepare_directories(root, true).map_err(|_| catalog_unavailable())?;
        let sidecar = self.context_sidecar(did)?;
        let created = match std::fs::symlink_metadata(&sidecar) {
            Ok(_) => false,
            Err(error) if error.kind() == io::ErrorKind::NotFound => true,
            Err(_) => return Err(catalog_unavailable()),
        };
        create_private_directory(&sidecar).map_err(|_| catalog_unavailable())?;
        create_private_directory(&sidecar.join("gateway")).map_err(|_| catalog_unavailable())?;
        Ok((sidecar, created))
    }

    pub fn maximum(&self) -> usize {
        self.maximum
    }
}

#[derive(Clone)]
struct RemoteTemplate {
    url: String,
    tenant: String,
}

/// Boot-time half of the catalogue. Existing entries are replayed into the
/// Runner before the control proof reader and connector registry are built.
pub struct EthosCatalogBootstrap {
    store: EthosCatalogStore,
    catalog: EthosCatalogFile,
    template: RemoteTemplate,
    identity: GatewayPublicIdentity,
    authorities: BTreeSet<String>,
    keyholder: Arc<Keyholder>,
}

impl EthosCatalogBootstrap {
    pub fn restore(
        config: &GatewayConfig,
        keyholder: Arc<Keyholder>,
        runner: &mut Runner,
        now_ms: i64,
    ) -> Result<Option<Self>> {
        let Some(catalog_config) = &config.ethos_catalog else {
            return Ok(None);
        };
        let template_context = config
            .contexts
            .as_ref()
            .and_then(|contexts| {
                contexts
                    .iter()
                    .find(|context| context.name == catalog_config.template_context)
            })
            .ok_or_else(|| {
                GatewayError::ConfigRejected("ethos catalogue template context is absent".into())
            })?;
        let template = match &template_context.store {
            StoreConfig::Remote { url, tenant, .. } => RemoteTemplate {
                url: url.clone(),
                tenant: tenant.clone(),
            },
            _ => {
                return Err(GatewayError::ConfigRejected(
                    "ethos catalogue template context is not remote".into(),
                ))
            }
        };
        let store =
            EthosCatalogStore::from_root(catalog_config.root.clone(), catalog_config.max_contexts)?;
        let catalog = store.load()?;
        let identity = GatewayPublicIdentity::new(&keyholder, catalog_config.audience.clone())?;
        let now = render_rfc3339z(now_ms);
        for entry in &catalog.entries {
            if entry.gateway_identity_digest != identity.digest
                || runner.hot_context_conflicts(&entry.name, &entry.did)
            {
                return Err(GatewayError::ConfigRejected(
                    "ethos catalogue entry conflicts with this gateway identity or runner".into(),
                ));
            }
            let bridge = open_catalog_bridge(
                &store,
                &template,
                &identity,
                Arc::clone(&keyholder),
                entry,
                &now,
            )?
            .commit();
            runner.insert_hot_context(entry.name.clone(), bridge)?;
        }
        Ok(Some(Self {
            store,
            catalog,
            template,
            identity,
            authorities: catalog_config
                .enrollment_authority_contexts
                .iter()
                .cloned()
                .collect(),
            keyholder,
        }))
    }

    pub fn activate(
        self,
        runner: Arc<Mutex<Runner>>,
        reader: ControlProofReader,
    ) -> EthosEnrollmentControl {
        EthosEnrollmentControl {
            inner: Arc::new(EthosEnrollmentInner {
                store: self.store,
                catalog: Mutex::new(self.catalog),
                template: self.template,
                identity: self.identity,
                authorities: self.authorities,
                keyholder: self.keyholder,
                runner,
                reader,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthosEnrollmentFailure {
    Conflict,
    Rejected,
    Unavailable,
}

#[derive(Clone)]
pub struct EthosEnrollmentControl {
    inner: Arc<EthosEnrollmentInner>,
}

struct EthosEnrollmentInner {
    store: EthosCatalogStore,
    catalog: Mutex<EthosCatalogFile>,
    template: RemoteTemplate,
    identity: GatewayPublicIdentity,
    authorities: BTreeSet<String>,
    keyholder: Arc<Keyholder>,
    runner: Arc<Mutex<Runner>>,
    reader: ControlProofReader,
}

impl EthosEnrollmentControl {
    pub fn identity(&self) -> GatewayPublicIdentity {
        self.inner.identity.clone()
    }

    pub fn authority_context_allowed(&self, context: &str) -> bool {
        self.inner.authorities.contains(context)
    }

    pub async fn enroll(
        &self,
        request: EthosEnrollmentRequest,
        now_ms: i64,
    ) -> std::result::Result<EthosEnrollmentResponse, EthosEnrollmentFailure> {
        if !request.validate_shape() {
            return Err(EthosEnrollmentFailure::Rejected);
        }
        if request.gateway_identity_digest != self.inner.identity.digest {
            return Err(EthosEnrollmentFailure::Conflict);
        }

        // One serialized commit boundary gives strict name/DID idempotence.
        let mut catalog = self.inner.catalog.lock().await;
        if let Some(existing) = catalog.exact(&request) {
            return Ok(EthosEnrollmentResponse::active(existing, false));
        }
        if catalog.conflicts(&request) {
            return Err(EthosEnrollmentFailure::Conflict);
        }
        {
            let runner = self.inner.runner.lock().await;
            if runner.hot_context_conflicts(&request.name, &request.did) {
                return Err(EthosEnrollmentFailure::Conflict);
            }
        }
        let next = catalog
            .inserted(request.entry(), self.inner.store.maximum())
            .map_err(|_| EthosEnrollmentFailure::Conflict)?;

        let store = self.inner.store.clone();
        let template = self.inner.template.clone();
        let identity = self.inner.identity.clone();
        let keyholder = Arc::clone(&self.inner.keyholder);
        let entry = request.entry();
        let now = render_rfc3339z(now_ms);
        let mut prepared = tokio::task::spawn_blocking(move || {
            open_catalog_bridge(&store, &template, &identity, keyholder, &entry, &now)
        })
        .await
        .map_err(|_| EthosEnrollmentFailure::Unavailable)?
        .map_err(classify_admission_error)?;

        let store = self.inner.store.clone();
        let durable = next.clone();
        match tokio::task::spawn_blocking(move || store.persist_for_enrollment(&durable)).await {
            Ok(Ok(())) => prepared.preserve_sidecar(),
            Ok(Err(CatalogCommitFailure::DefinitelyNotCommitted)) => {
                return Err(EthosEnrollmentFailure::Unavailable);
            }
            Ok(Err(CatalogCommitFailure::OutcomeUncertain)) | Err(_) => {
                // Rename completed (or the worker outcome is unknown): keep
                // the matching sidecar so a complete-new catalog can replay.
                prepared.preserve_sidecar();
                return Err(EthosEnrollmentFailure::Unavailable);
            }
        }

        let bridge = prepared.commit();
        let mut runner = self.inner.runner.lock().await;
        if runner.hot_context_conflicts(&request.name, &request.did) {
            return Err(EthosEnrollmentFailure::Unavailable);
        }
        let runtime_store = runner
            .insert_hot_context(request.name.clone(), bridge)
            .map_err(|_| EthosEnrollmentFailure::Unavailable)?;
        self.inner
            .reader
            .insert_context(request.name.clone(), request.did.clone(), runtime_store)
            .map_err(|_| EthosEnrollmentFailure::Unavailable)?;
        *catalog = next;
        Ok(EthosEnrollmentResponse::active(&request.entry(), true))
    }
}

struct PreparedCatalogBridge {
    bridge: Option<Bridge>,
    sidecar: PathBuf,
    remove_on_drop: bool,
}

impl PreparedCatalogBridge {
    fn preserve_sidecar(&mut self) {
        self.remove_on_drop = false;
    }

    fn commit(mut self) -> Bridge {
        self.remove_on_drop = false;
        self.bridge.take().expect("prepared bridge")
    }
}

impl Drop for PreparedCatalogBridge {
    fn drop(&mut self) {
        if self.remove_on_drop {
            // Exact hash-derived child created by this attempt only. Removing
            // it bounds rejected enrollment disk use; pre-existing recovery
            // state is never deleted.
            let _ = std::fs::remove_dir_all(&self.sidecar);
        }
    }
}

fn open_catalog_bridge(
    catalog: &EthosCatalogStore,
    template: &RemoteTemplate,
    identity: &GatewayPublicIdentity,
    keyholder: Arc<Keyholder>,
    entry: &EthosCatalogEntry,
    now: &str,
) -> Result<PreparedCatalogBridge> {
    let (sidecar, created) = catalog.prepare_context_sidecar(&entry.did)?;
    let result = (|| {
        persist_bridge_state(&sidecar, &entry.agent_mandate, &entry.gateway_mandate)?;
        let store_config = StoreConfig::Remote {
            url: template.url.clone(),
            tenant: template.tenant.clone(),
            did: entry.did.clone(),
            mandate: vec![entry.agent_mandate.clone()],
            local: Some(sidecar.clone()),
        };
        let store = GatewayStore::from_config_with_identity(&store_config, &keyholder, || {
            Box::new(OsEntropy)
        })?;
        let bridge = Bridge::open(store, keyholder, Box::new(OsEntropy))?;
        bridge.validate_hot_enrollment(
            &entry.did,
            &entry.agent_mandate,
            &entry.gateway_mandate,
            &identity.agent.signing,
            &identity.agent.key_exchange,
            &identity.gateway.signing,
            &identity.gateway.key_exchange,
            &identity.audience,
            &identity.required_actions.agent,
            &identity.required_actions.gateway,
            now,
        )?;
        Ok(bridge)
    })();
    match result {
        Ok(bridge) => Ok(PreparedCatalogBridge {
            bridge: Some(bridge),
            sidecar,
            remove_on_drop: created,
        }),
        Err(error) => {
            if created {
                let _ = std::fs::remove_dir_all(&sidecar);
            }
            Err(error)
        }
    }
}

fn persist_bridge_state(sidecar: &Path, agent_mandate: &str, gateway_mandate: &str) -> Result<()> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "agent_mandate": agent_mandate,
        "gateway_mandate": gateway_mandate,
    }))
    .map_err(|_| catalog_invalid())?;
    let directory = sidecar.join("gateway");
    let path = sidecar.join(STATE_PATH);
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temp = directory.join(format!(".state.json.tmp-{}-{serial}", std::process::id()));
    let result = (|| -> io::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, &path)?;
        File::open(&directory)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|_| catalog_unavailable())
}

fn classify_admission_error(error: GatewayError) -> EthosEnrollmentFailure {
    match error {
        GatewayError::MandateDenied { .. } | GatewayError::RequestRejected(_) => {
            EthosEnrollmentFailure::Rejected
        }
        GatewayError::BridgeFailed(message)
            if message.contains("transport")
                || message.contains("remote store 5")
                || message.contains("timed out")
                || message.contains("connection") =>
        {
            EthosEnrollmentFailure::Unavailable
        }
        GatewayError::BridgeFailed(_) | GatewayError::ConfigRejected(_) => {
            EthosEnrollmentFailure::Rejected
        }
        _ => EthosEnrollmentFailure::Unavailable,
    }
}

fn load_fs(root: &Path, path: &Path) -> Result<Option<Vec<u8>>> {
    if !prepare_directories(root, false).map_err(|_| catalog_unavailable())? {
        return Ok(None);
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(catalog_unavailable()),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_CATALOG_BYTES as u64
    {
        return Err(catalog_invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(catalog_invalid());
        }
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .map_err(|_| catalog_unavailable())?
        .take((MAX_CATALOG_BYTES as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| catalog_unavailable())?;
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(catalog_invalid());
    }
    Ok(Some(bytes))
}

fn persist_fs(
    root: &Path,
    path: &Path,
    bytes: &[u8],
    fault: Option<CatalogPersistenceFault>,
) -> std::result::Result<(), CatalogCommitFailure> {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temp = root.join(format!(".catalog.json.tmp-{}-{serial}", std::process::id()));
    let mut renamed = false;
    let result = (|| -> io::Result<()> {
        prepare_directories(root, true)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if fault == Some(CatalogPersistenceFault::BeforeRename) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "injected pre-rename failure",
            ));
        }
        std::fs::rename(&temp, path)?;
        renamed = true;
        File::open(root)?.sync_all()?;
        if fault == Some(CatalogPersistenceFault::AfterRename) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "injected post-rename failure",
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|_| {
        if renamed {
            CatalogCommitFailure::OutcomeUncertain
        } else {
            CatalogCommitFailure::DefinitelyNotCommitted
        }
    })
}

fn prepare_directories(root: &Path, create: bool) -> io::Result<bool> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
            std::fs::create_dir_all(root)?;
            std::fs::symlink_metadata(root)?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    ensure_private_directory(root, metadata)?;
    let contexts = root.join(CONTEXTS_DIRECTORY);
    match std::fs::symlink_metadata(&contexts) {
        Ok(metadata) => ensure_private_directory(&contexts, metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
            std::fs::create_dir(&contexts)?;
            ensure_private_directory(&contexts, std::fs::symlink_metadata(&contexts)?)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(true)
}

fn ensure_private_directory(path: &Path, metadata: std::fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "catalogue directory is not a plain directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => ensure_private_directory(path, metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir(path)?;
            ensure_private_directory(path, std::fs::symlink_metadata(path)?)
        }
        Err(error) => Err(error),
    }
}

fn canonical_label(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn canonical_did(value: &str) -> bool {
    let Some(key) = value.strip_prefix("did:aithos:") else {
        return false;
    };
    aithos_core::wire::multibase_to_ed25519_pub(key)
        .is_ok_and(|bytes| aithos_core::wire::did_aithos(&bytes) == value)
}

fn canonical_mandate_id(value: &str) -> bool {
    value.strip_prefix("mandate_").is_some_and(|suffix| {
        const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        suffix.len() == 26
            && suffix.bytes().all(|byte| CROCKFORD.contains(&byte))
            && aithos_core::ids::Sid::parse(suffix).is_ok_and(|id| id.to_string() == suffix)
    })
}

fn canonical_identity_digest(value: &str) -> bool {
    value.strip_prefix("b3:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn catalog_invalid() -> GatewayError {
    GatewayError::ConfigRejected("ethos catalogue is invalid".into())
}

fn catalog_unavailable() -> GatewayError {
    GatewayError::BridgeFailed("ethos catalogue is unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(name: &str, did_tail: char) -> EthosEnrollmentRequest {
        let mut key = [0_u8; 32];
        key.fill(did_tail as u8);
        EthosEnrollmentRequest {
            v: 1,
            name: name.to_owned(),
            did: aithos_core::wire::did_aithos(&key),
            agent_mandate: "mandate_01J00000000000000000000001".to_owned(),
            gateway_mandate: "mandate_01J00000000000000000000002".to_owned(),
            gateway_identity_digest: format!("b3:{}", "a".repeat(64)),
        }
    }

    #[test]
    fn identity_digest_is_canonical_and_rotation_sensitive() {
        let first = Keyholder::from_entropy([1; 32], [2; 32]);
        let same = Keyholder::from_entropy([1; 32], [2; 32]);
        let rotated = Keyholder::from_entropy([1; 32], [3; 32]);
        let a = GatewayPublicIdentity::new(&first, "https://gateway.example/mcp").unwrap();
        let b = GatewayPublicIdentity::new(&same, "https://gateway.example/mcp").unwrap();
        let c = GatewayPublicIdentity::new(&rotated, "https://gateway.example/mcp").unwrap();
        assert_eq!(a.digest, b.digest);
        assert_ne!(a.digest, c.digest);
        assert!(canonical_identity_digest(&a.digest));
        let wire = serde_json::to_value(&a).unwrap();
        assert_eq!(wire["digest"], a.digest);
        assert!(wire.get("gateway_identity_digest").is_none());
    }

    #[test]
    fn request_shape_is_closed_bounded_and_uses_distinct_mandates() {
        let valid = request("support", 'A');
        assert!(valid.validate_shape());
        let unknown = serde_json::from_str::<EthosEnrollmentRequest>(
            r#"{"v":1,"name":"support","did":"did:aithos:zAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","agent_mandate":"mandate_01J00000000000000000000001","gateway_mandate":"mandate_01J00000000000000000000002","gateway_identity_digest":"b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","url":"https://evil.example"}"#,
        );
        assert!(unknown.is_err());
        let mut same = valid;
        same.gateway_mandate = same.agent_mandate.clone();
        assert!(!same.validate_shape());
        let response = serde_json::to_value(EthosEnrollmentResponse::active(
            &request("support", 'A').entry(),
            true,
        ))
        .unwrap();
        assert_eq!(response["status"], "active");
        assert!(response.get("state").is_none());
    }

    #[test]
    fn catalogue_is_sorted_unique_and_strictly_idempotent() {
        let first = request("support", 'A');
        let file = EthosCatalogFile::default()
            .inserted(first.entry(), 4)
            .unwrap();
        assert!(file.exact(&first).is_some());
        assert!(!file.conflicts(&first));

        let mut conflict = first.clone();
        conflict.did = aithos_core::wire::did_aithos(&[0x42; 32]);
        assert!(file.conflicts(&conflict));
        assert!(file.exact(&conflict).is_none());
    }

    #[test]
    fn memory_persistence_is_old_or_complete_new() {
        let store = EthosCatalogStore::memory(4);
        let old = EthosCatalogFile::default()
            .inserted(request("alpha", 'A').entry(), 4)
            .unwrap();
        store.persist(&old).unwrap();
        let new = old.inserted(request("beta", 'B').entry(), 4).unwrap();
        assert!(store
            .persist_with_fault_for_test(&new, CatalogPersistenceFault::BeforeRename)
            .is_err());
        assert_eq!(store.load().unwrap(), old);
        assert!(store
            .persist_with_fault_for_test(&new, CatalogPersistenceFault::AfterRename)
            .is_err());
        assert_eq!(store.load().unwrap(), new);
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_catalog_requires_private_files_and_cleans_pre_rename_temps() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().unwrap();
        let store = EthosCatalogStore::from_root(temp.path().join("catalog"), 4).unwrap();
        let file = EthosCatalogFile::default()
            .inserted(request("alpha", 'A').entry(), 4)
            .unwrap();
        assert!(store
            .persist_with_fault_for_test(&file, CatalogPersistenceFault::BeforeRename)
            .is_err());
        let root = temp.path().join("catalog");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
        store.persist(&file).unwrap();
        let path = root.join(CATALOG_FILE);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
            0
        );
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(&path, permissions).unwrap();
        assert!(store.load().is_err());
    }

    #[test]
    fn post_rename_failure_keeps_complete_catalog_and_sidecar_recoverable() {
        let temp = tempfile::tempdir().unwrap();
        let store = EthosCatalogStore::from_root(temp.path().join("catalog"), 4).unwrap();
        let entry = request("support", 'A').entry();
        let (sidecar, created) = store.prepare_context_sidecar(&entry.did).unwrap();
        assert!(created);
        persist_bridge_state(&sidecar, &entry.agent_mandate, &entry.gateway_mandate).unwrap();
        let next = EthosCatalogFile::default()
            .inserted(entry.clone(), 4)
            .unwrap();
        assert!(store
            .persist_with_fault_for_test(&next, CatalogPersistenceFault::AfterRename)
            .is_err());

        // Recovery sees either old or complete-new. This injected point is
        // explicitly after rename, so the complete entry and its sidecar are
        // both available to boot replay.
        assert_eq!(store.load().unwrap(), next);
        assert!(sidecar.join(STATE_PATH).is_file());
        let state: serde_json::Value =
            serde_json::from_slice(&std::fs::read(sidecar.join(STATE_PATH)).unwrap()).unwrap();
        assert_eq!(state["agent_mandate"], entry.agent_mandate);
        assert_eq!(state["gateway_mandate"], entry.gateway_mandate);
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn remote_admission_uses_private_sidecar_and_rejected_extra_right_leaves_none() {
        use std::os::unix::fs::PermissionsExt as _;

        use aithos_bundle::bundle::Bundle;
        use aithos_bundle::remote::{KeySigner, RemoteStore};
        use aithos_bundle::Store as _;
        use aithos_core::keys::{MasterSeed, OwnerKeys};
        use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
        use aithos_provider::acme::AcmeState;
        use aithos_provider::control::ControlPlane;
        use aithos_provider::dns::MemDnsTxt;
        use aithos_provider::heads::MemHeads;
        use aithos_provider::nonces::MemNonces;
        use aithos_provider::objects::MemObjects;
        use aithos_provider::service::{build_router, AppState};

        fn now(offset_seconds: i64) -> String {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            aithos_provider::time::render_rfc3339z(
                millis - millis.rem_euclid(1_000) + offset_seconds * 1_000,
            )
        }

        fn provision(
            root: &Path,
            owner_seed: u8,
            identity: &GatewayPublicIdentity,
            extra_gateway_right: bool,
        ) -> (EthosCatalogEntry, OwnerKeys) {
            let owner = OwnerKeys::genesis(&MasterSeed::from_bytes([owner_seed; 32]));
            let succession = ed25519_dalek::SigningKey::from_bytes(&[owner_seed + 1; 32]);
            let mut entropy = crate::core_bridge::SeqEntropy::default();
            let mut bundle = Bundle::init(
                GatewayStore::Fs(root.to_path_buf()),
                &owner,
                &succession.verifying_key(),
                &mut entropy,
                &now(0),
            )
            .unwrap();
            let build = |id: &str, label: &str, signing: &str, actions: &[String], nonce: &str| {
                let bytes = aithos_core::wire::multibase_to_ed25519_pub(signing).unwrap();
                let key = ed25519_dalek::VerifyingKey::from_bytes(&bytes).unwrap();
                let perimeter = actions
                    .iter()
                    .map(|action| PerimeterEntry::parse(action).unwrap())
                    .collect::<Vec<_>>();
                Mandate::build_root(
                    &owner.root_sign,
                    &MandateSpec {
                        id: id.to_owned(),
                        subject: bundle.did.clone(),
                        grantee_id: format!("urn:aithos:agent:{label}"),
                        grantee_label: label.to_owned(),
                        grantee_pub: &key,
                        perimeter,
                        constraints: MandateSpec::no_constraints(),
                        not_before: now(-60),
                        not_after: now(86_400),
                        issued_at: now(0),
                        nonce: nonce.to_owned(),
                    },
                )
                .unwrap()
            };
            let agent_id = "mandate_01J00000000000000000000001";
            let gateway_id = "mandate_01J00000000000000000000002";
            let agent = build(
                agent_id,
                "agent",
                &identity.agent.signing,
                &identity.required_actions.agent,
                "agent-nonce",
            );
            let mut gateway_actions = identity.required_actions.gateway.clone();
            if extra_gateway_right {
                gateway_actions.push("act.x.gateway.unexpected".to_owned());
            }
            let gateway = build(
                gateway_id,
                "gateway",
                &identity.gateway.signing,
                &gateway_actions,
                "gateway-nonce",
            );
            for mandate in [&agent, &gateway] {
                bundle
                    .store
                    .put(
                        &format!("certs/{}.json", mandate.id),
                        &serde_json::to_vec(mandate).unwrap(),
                    )
                    .unwrap();
                bundle
                    .log_owner_grant(&owner, &mandate.id, &now(0), &mut entropy)
                    .unwrap();
            }
            let entry = EthosCatalogEntry {
                name: if extra_gateway_right {
                    "too-powerful".to_owned()
                } else {
                    "support".to_owned()
                },
                did: bundle.did.clone(),
                agent_mandate: agent.id,
                gateway_mandate: gateway.id,
                gateway_identity_digest: identity.digest.clone(),
            };
            (entry, owner)
        }

        async fn provider(dids: &[String]) -> String {
            let bootstrap = serde_json::json!({
                "tenants": [{
                    "tenant": "demo",
                    "dids": dids.iter().map(|did| serde_json::json!({"did": did})).collect::<Vec<_>>()
                }]
            });
            let (control, _, _) =
                ControlPlane::from_bootstrap_json(&bootstrap.to_string()).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let state = Arc::new(AppState {
                control: Arc::new(control),
                objects: Arc::new(MemObjects::new()),
                heads: Arc::new(MemHeads::new()),
                deposit_locks: Default::default(),
                nonces: Arc::new(MemNonces::new(600)),
                dns: Arc::new(MemDnsTxt::new()),
                acme: AcmeState::new(),
                authority: format!("127.0.0.1:{port}"),
                test_now_enabled: false,
                browser_origins: Default::default(),
            });
            let router = build_router(state);
            tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            format!("http://127.0.0.1:{port}")
        }

        fn replicate(root: &Path, url: &str, entry: &EthosCatalogEntry, owner: &OwnerKeys) {
            let mut remote = RemoteStore::new(
                url,
                "demo",
                &entry.did,
                Arc::new(KeySigner::owner("#root", owner.root_sign.clone())),
                Arc::new(|| now(0)),
                Box::new(OsEntropy),
            )
            .unwrap();
            crate::store_adapter::replicate_owner_history(root, &mut remote).unwrap();
        }

        let temp = tempfile::tempdir().unwrap();
        let keyholder = Arc::new(Keyholder::from_entropy([0x11; 32], [0x22; 32]));
        let identity = GatewayPublicIdentity::new(&keyholder, "http://127.0.0.1:4870/mcp").unwrap();
        let valid_root = temp.path().join("valid-owner");
        let invalid_root = temp.path().join("invalid-owner");
        let (valid, valid_owner) = provision(&valid_root, 0x31, &identity, false);
        let (invalid, invalid_owner) = provision(&invalid_root, 0x41, &identity, true);
        let url = provider(&[valid.did.clone(), invalid.did.clone()]).await;
        let valid_replica = (valid_root.clone(), url.clone(), valid.clone());
        let invalid_replica = (invalid_root.clone(), url.clone(), invalid.clone());
        tokio::task::spawn_blocking(move || {
            replicate(
                &valid_replica.0,
                &valid_replica.1,
                &valid_replica.2,
                &valid_owner,
            );
            replicate(
                &invalid_replica.0,
                &invalid_replica.1,
                &invalid_replica.2,
                &invalid_owner,
            );
        })
        .await
        .unwrap();

        let catalog = EthosCatalogStore::from_root(temp.path().join("runtime-catalog"), 4).unwrap();
        let template = RemoteTemplate {
            url,
            tenant: "demo".to_owned(),
        };
        let valid_for_open = valid.clone();
        let catalog_for_open = catalog.clone();
        let template_for_open = template.clone();
        let identity_for_open = identity.clone();
        let keyholder_for_open = Arc::clone(&keyholder);
        let prepared = tokio::task::spawn_blocking(move || {
            open_catalog_bridge(
                &catalog_for_open,
                &template_for_open,
                &identity_for_open,
                keyholder_for_open,
                &valid_for_open,
                &now(0),
            )
        })
        .await
        .unwrap()
        .unwrap();
        let valid_sidecar = catalog.context_sidecar(&valid.did).unwrap();
        let state = valid_sidecar.join(STATE_PATH);
        assert!(state.is_file());
        assert_eq!(
            std::fs::metadata(&state).unwrap().permissions().mode() & 0o077,
            0
        );
        let bridge = prepared.commit();
        assert_eq!(bridge.ethos_did(), valid.did);

        let invalid_for_open = invalid.clone();
        let catalog_for_open = catalog.clone();
        let template_for_invalid = template.clone();
        let identity_for_invalid = identity.clone();
        let keyholder_for_invalid = Arc::clone(&keyholder);
        let rejected = tokio::task::spawn_blocking(move || {
            open_catalog_bridge(
                &catalog_for_open,
                &template_for_invalid,
                &identity_for_invalid,
                keyholder_for_invalid,
                &invalid_for_open,
                &now(0),
            )
        })
        .await
        .unwrap();
        assert!(matches!(rejected, Err(GatewayError::MandateDenied { .. })));
        assert!(!catalog.context_sidecar(&invalid.did).unwrap().exists());

        let durable = EthosCatalogFile {
            v: ETHOS_CATALOG_VERSION,
            entries: vec![valid.clone()],
        };
        catalog.persist(&durable).unwrap();
        let static_sidecar = temp.path().join("static-template-sidecar");
        let journal_root = temp.path().join("journal");
        let yaml = format!(
            r#"
listen: 127.0.0.1:4870
dashboard: {{}}
servers:
  - name: bootstrap
    transport: http
    url: http://127.0.0.1:9/mcp
contexts:
  - name: authority
    store:
      kind: remote
      url: {}
      tenant: demo
      did: {}
      mandate: [{}]
      local: {}
    tools: {{}}
journal:
  store: {{ kind: fs, root: {} }}
ethos_catalog:
  root: {}
  template_context: authority
  audience: {}
  enrollment_authority_contexts: [authority]
"#,
            template.url,
            valid.did,
            valid.agent_mandate,
            static_sidecar.display(),
            journal_root.display(),
            temp.path().join("runtime-catalog").display(),
            identity.audience,
        );
        let config = GatewayConfig::from_yaml(&yaml).unwrap();
        let mut restarted = Runner::from_parts(std::collections::BTreeMap::new(), bridge);
        let bootstrap = EthosCatalogBootstrap::restore(
            &config,
            keyholder,
            &mut restarted,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        )
        .unwrap();
        assert!(bootstrap.is_some());
        assert!(restarted.hot_context_conflicts(&valid.name, &valid.did));
    }

    mod gherkin {
        use super::*;
        use cucumber::writer::Stats as _;
        use cucumber::{given, then, when, World as _};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Outcome {
            Created,
            Existing,
            Conflict,
            Forbidden,
        }

        #[derive(cucumber::World)]
        #[world(init = Self::new)]
        struct EnrollmentWorld {
            store: EthosCatalogStore,
            catalog: EthosCatalogFile,
            request: Option<EthosEnrollmentRequest>,
            current_digest: String,
            process_identity: String,
            original_process_identity: String,
            allowed: BTreeSet<String>,
            outcome: Option<Outcome>,
            restored_before_connectors: bool,
        }

        impl std::fmt::Debug for EnrollmentWorld {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("EnrollmentWorld")
                    .field("catalog", &self.catalog)
                    .field("request", &self.request)
                    .field("current_digest", &self.current_digest)
                    .field("outcome", &self.outcome)
                    .finish()
            }
        }

        impl EnrollmentWorld {
            fn new() -> Self {
                let digest = format!("b3:{}", "a".repeat(64));
                Self {
                    store: EthosCatalogStore::memory(8),
                    catalog: EthosCatalogFile::default(),
                    request: None,
                    current_digest: digest,
                    process_identity: "runner-identity-1".to_owned(),
                    original_process_identity: "runner-identity-1".to_owned(),
                    allowed: BTreeSet::new(),
                    outcome: None,
                    restored_before_connectors: false,
                }
            }

            fn attempt(&mut self, actor: &str) {
                if !self.allowed.contains(actor) {
                    self.outcome = Some(Outcome::Forbidden);
                    return;
                }
                let request = self.request.as_ref().expect("published request");
                if request.gateway_identity_digest != self.current_digest
                    || self.catalog.conflicts(request)
                {
                    self.outcome = Some(Outcome::Conflict);
                    return;
                }
                if self.catalog.exact(request).is_some() {
                    self.outcome = Some(Outcome::Existing);
                    return;
                }
                self.catalog = self
                    .catalog
                    .inserted(request.entry(), self.store.maximum())
                    .unwrap();
                self.store.persist(&self.catalog).unwrap();
                self.outcome = Some(Outcome::Created);
            }
        }

        #[given(expr = "a Gateway with {string} authorized to enroll contexts")]
        fn gateway_with_authority(world: &mut EnrollmentWorld, authority: String) {
            world.allowed.insert(authority);
        }

        #[given(expr = "a published equipped Ethos named {string}")]
        fn published_ethos(world: &mut EnrollmentWorld, name: String) {
            let mut request = super::request(&name, 'S');
            request.gateway_identity_digest = world.current_digest.clone();
            world.request = Some(request);
        }

        #[when(expr = "the authorized owner enrolls the {string} Ethos")]
        fn authorized_enrolls(world: &mut EnrollmentWorld, _name: String) {
            world.attempt("authority");
        }

        #[then("the enrollment is created and active")]
        fn created_active(world: &mut EnrollmentWorld) {
            assert_eq!(world.outcome, Some(Outcome::Created));
        }

        #[then(expr = "{string} is immediately visible through the signed control surface")]
        fn immediately_visible(world: &mut EnrollmentWorld, name: String) {
            assert!(world.catalog.entries.iter().any(|entry| entry.name == name));
        }

        #[then("the Gateway process identity did not change")]
        fn process_identity_stable(world: &mut EnrollmentWorld) {
            assert_eq!(world.process_identity, world.original_process_identity);
        }

        #[when(expr = "the authorized owner enrolls the {string} Ethos twice with fresh nonces")]
        fn enrolls_twice(world: &mut EnrollmentWorld, _name: String) {
            world.attempt("authority");
            world.attempt("authority");
        }

        #[then("the second enrollment reports the existing active context")]
        fn second_existing(world: &mut EnrollmentWorld) {
            assert_eq!(world.outcome, Some(Outcome::Existing));
        }

        #[then(expr = "the durable catalogue contains one {string} entry")]
        fn one_entry(world: &mut EnrollmentWorld, name: String) {
            assert_eq!(
                world
                    .store
                    .load()
                    .unwrap()
                    .entries
                    .iter()
                    .filter(|entry| entry.name == name)
                    .count(),
                1
            );
        }

        #[given(expr = "the authorized owner enrolled the {string} Ethos")]
        fn already_enrolled(world: &mut EnrollmentWorld, _name: String) {
            world.attempt("authority");
            assert_eq!(world.outcome, Some(Outcome::Created));
        }

        #[when("the Gateway restarts over the same catalogue")]
        fn restart(world: &mut EnrollmentWorld) {
            world.catalog = world.store.load().unwrap();
            world.restored_before_connectors = true;
        }

        #[then(expr = "{string} is active before connector restoration")]
        fn active_before_connectors(world: &mut EnrollmentWorld, name: String) {
            assert!(world.restored_before_connectors);
            assert!(world.catalog.entries.iter().any(|entry| entry.name == name));
        }

        #[when(expr = "the authorized owner enrolls another DID as {string}")]
        fn same_name_other_did(world: &mut EnrollmentWorld, name: String) {
            let mut collision = super::request(&name, 'X');
            collision.gateway_identity_digest = world.current_digest.clone();
            world.request = Some(collision);
            world.attempt("authority");
        }

        #[then("the enrollment is refused as a context conflict")]
        fn conflict(world: &mut EnrollmentWorld) {
            assert_eq!(world.outcome, Some(Outcome::Conflict));
        }

        #[then(expr = "the original {string} context remains active")]
        fn original_active(world: &mut EnrollmentWorld, name: String) {
            let persisted = world.store.load().unwrap();
            assert_eq!(
                persisted
                    .entries
                    .iter()
                    .filter(|entry| entry.name == name)
                    .count(),
                1
            );
        }

        #[given(expr = "a loaded context {string} outside the enrollment allowlist")]
        fn outside_allowlist(_world: &mut EnrollmentWorld, _name: String) {}

        #[when(expr = "the {string} owner enrolls the {string} Ethos")]
        fn outside_owner_enrolls(world: &mut EnrollmentWorld, actor: String, _name: String) {
            world.attempt(&actor);
        }

        #[then("the enrollment is refused as forbidden")]
        fn forbidden(world: &mut EnrollmentWorld) {
            assert_eq!(world.outcome, Some(Outcome::Forbidden));
        }

        #[then("no catalogue entry is written")]
        fn none_written(world: &mut EnrollmentWorld) {
            assert!(world.store.load().unwrap().entries.is_empty());
        }

        #[given("the browser discovered the previous Gateway identity digest")]
        fn previous_digest(_world: &mut EnrollmentWorld) {}

        #[when("the authorized owner enrolls after the Gateway identity rotates")]
        fn enroll_after_rotation(world: &mut EnrollmentWorld) {
            world.current_digest = format!("b3:{}", "b".repeat(64));
            world.attempt("authority");
        }

        #[tokio::test]
        async fn executable_enrollment_features() {
            let features = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/enrollment_features");
            let writer = EnrollmentWorld::cucumber()
                .max_concurrent_scenarios(Some(1))
                .fail_on_skipped()
                .with_default_cli()
                .run(features)
                .await;
            assert!(
                !writer.execution_has_failed(),
                "gateway enrollment Gherkin failed"
            );
        }
    }
}
