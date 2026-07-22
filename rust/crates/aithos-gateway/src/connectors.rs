//! G7b pre-approved connector bindings.
//!
//! The browser supplies a closed, public descriptor. A static `servers[]`
//! entry is the approved public OAuth/endpoint template, while the sealed H3
//! manifest in the named context remains the sole source of tools, schemas,
//! grants and pins. Secrets and OAuth custody stay behind broker references
//! derived here; the sidecar persists non-secret state only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zeroize::Zeroize;

use crate::config::{GatewayConfig, ServerConfig, StoreConfig, UpstreamOAuthConfig};
use crate::core_bridge::{proposed_manifest_catalog_digest, Runner};
use crate::credentials::{CredentialBroker, CredentialRef, SecretValue};
use crate::hub::discover_server;
use crate::policy::valid_server_name;
use crate::proxy_mcp::{DynamicUpstream, DynamicUpstreams, HttpUpstream};
use crate::upstream_oauth::{ConsentStart, UpstreamOAuthRegistry, UpstreamOAuthState};
use crate::{GatewayError, Result};

const REGISTRY_VERSION: u8 = 1;
const MAX_CONNECTORS: usize = 64;
const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
const MAX_ID_BYTES: usize = 64;
const MAX_URL_BYTES: usize = 2_048;
const MAX_CLIENT_ID_BYTES: usize = 512;
const MAX_SCOPES: usize = 64;
const MAX_SCOPE_BYTES: usize = 256;
const MAX_CLIENT_SECRET_BYTES: usize = 4_096;
const VAULT_RECORD_FIELD: &str = "value";
const VAULT_RECORD_PREFIX: &str = "aithos/connectors";
const SIDECAR_DIRECTORY: &str = ".aithos-sidecar";
const REGISTRY_RELATIVE_PATH: &str = "gateway/connectors.json";

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorState {
    Draft,
    SecretMissing,
    Disconnected,
    Pending,
    Connected,
    Expired,
    Drifted,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedManifestRef {
    pub id: String,
    pub pin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorOAuthDescriptor {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    pub client_secret_record: String,
    pub pending_record: String,
    pub token_record: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorTransport {
    #[serde(rename = "streamable-http")]
    StreamableHttp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorStageRequest {
    pub v: u8,
    pub id: String,
    pub context: String,
    pub endpoint: String,
    pub transport: ConnectorTransport,
    pub oauth: ConnectorOAuthDescriptor,
    pub approved_manifest: ApprovedManifestRef,
}

/// Parsed separately from every other DTO so no derive can ever print or
/// clone the secret. The string zeroizes even when validation refuses it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientSecretBody {
    client_secret: String,
}

impl ClientSecretBody {
    fn take(&mut self) -> String {
        std::mem::take(&mut self.client_secret)
    }

    fn valid(&self) -> bool {
        !self.client_secret.is_empty()
            && self.client_secret.len() <= MAX_CLIENT_SECRET_BYTES
            && !self.client_secret.contains('\0')
    }
}

impl Drop for ClientSecretBody {
    fn drop(&mut self) {
        self.client_secret.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorView {
    pub v: u8,
    pub id: String,
    pub context: String,
    pub endpoint: String,
    pub transport: ConnectorTransport,
    pub state: ConnectorState,
    pub active: bool,
    pub approved_manifest: ApprovedManifestRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorPage {
    pub v: u8,
    pub items: Vec<ConnectorView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorOAuthStatus {
    pub v: u8,
    pub id: String,
    pub state: ConnectorState,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorOAuthStart {
    pub v: u8,
    pub authorization_url: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorActivation {
    pub v: u8,
    pub connector: ConnectorView,
    pub approved_digest: String,
    pub live_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorFailure {
    NotApproved,
    SecretUnavailable,
    OauthPending,
    OauthDenied,
    OauthUnavailable,
    ManifestDrift,
    ActivationFailed,
    UpstreamDenied,
}

impl ConnectorFailure {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotApproved => "connector_not_approved",
            Self::SecretUnavailable => "secret_unavailable",
            Self::OauthPending => "oauth_pending",
            Self::OauthDenied => "oauth_denied",
            Self::OauthUnavailable => "oauth_unavailable",
            Self::ManifestDrift => "manifest_drift",
            Self::ActivationFailed => "activation_failed",
            Self::UpstreamDenied => "upstream_denied",
        }
    }
}

pub type ConnectorResult<T> = std::result::Result<T, ConnectorFailure>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedConnector {
    id: String,
    context: String,
    endpoint: String,
    transport: ConnectorTransport,
    oauth: ConnectorOAuthDescriptor,
    approved_manifest: ApprovedManifestRef,
    state: ConnectorState,
    active: bool,
    secret_stored: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live_digest: Option<String>,
}

impl PersistedConnector {
    fn view(&self) -> ConnectorView {
        ConnectorView {
            v: REGISTRY_VERSION,
            id: self.id.clone(),
            context: self.context.clone(),
            endpoint: self.endpoint.clone(),
            transport: self.transport,
            state: self.state,
            active: self.active,
            approved_manifest: self.approved_manifest.clone(),
            live_digest: self.live_digest.clone(),
        }
    }

    fn oauth_status(&self) -> ConnectorOAuthStatus {
        ConnectorOAuthStatus {
            v: REGISTRY_VERSION,
            id: self.id.clone(),
            state: self.state,
            active: self.active,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    v: u8,
    connectors: Vec<PersistedConnector>,
}

impl Default for RegistryFile {
    fn default() -> Self {
        Self {
            v: REGISTRY_VERSION,
            connectors: Vec::new(),
        }
    }
}

impl RegistryFile {
    fn validate(&self) -> ConnectorResult<()> {
        if self.v != REGISTRY_VERSION || self.connectors.len() > MAX_CONNECTORS {
            return Err(ConnectorFailure::ActivationFailed);
        }
        let mut ids = BTreeSet::new();
        for connector in &self.connectors {
            if !ids.insert(connector.id.as_str())
                || !valid_id(&connector.id)
                || !valid_id(&connector.context)
                || !valid_public_descriptor(connector)
                || connector.approved_manifest.id != connector.id
                || (connector.active
                    && (connector.state != ConnectorState::Connected
                        || connector.live_digest.is_none()))
            {
                return Err(ConnectorFailure::ActivationFailed);
            }
        }
        Ok(())
    }

    fn get(&self, context: &str, id: &str) -> ConnectorResult<&PersistedConnector> {
        self.connectors
            .iter()
            .find(|connector| connector.id == id && connector.context == context)
            .ok_or(ConnectorFailure::NotApproved)
    }

    fn replace(&mut self, connector: PersistedConnector) {
        self.connectors.retain(|current| current.id != connector.id);
        self.connectors.push(connector);
        self.connectors
            .sort_by(|left, right| left.id.cmp(&right.id));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceFault {
    BeforeRename,
    AfterRename,
}

#[derive(Clone)]
pub struct ConnectorRegistryStore {
    backing: RegistryBacking,
}

#[derive(Clone)]
enum RegistryBacking {
    Fs { root: PathBuf, path: PathBuf },
    Memory(Arc<StdMutex<Option<Vec<u8>>>>),
}

impl ConnectorRegistryStore {
    pub fn memory() -> Self {
        Self {
            backing: RegistryBacking::Memory(Arc::new(StdMutex::new(None))),
        }
    }

    pub fn from_store_config(config: &StoreConfig) -> Result<Self> {
        let root = match config {
            StoreConfig::Fs { root } | StoreConfig::Replicated { root, .. } => root.clone(),
            StoreConfig::Remote {
                local: Some(root), ..
            } => root.clone(),
            StoreConfig::Remote { local: None, .. } => {
                return Err(GatewayError::ConfigRejected(
                    "dashboard connectors need a durable local RemoteStore sidecar".into(),
                ))
            }
            StoreConfig::S3 { .. } => {
                return Err(GatewayError::ConfigRejected(
                    "dashboard connectors cannot use the unavailable s3 store".into(),
                ))
            }
        };
        let path = root.join(SIDECAR_DIRECTORY).join(REGISTRY_RELATIVE_PATH);
        Ok(Self {
            backing: RegistryBacking::Fs { root, path },
        })
    }

    fn load(&self) -> ConnectorResult<RegistryFile> {
        let bytes = match &self.backing {
            RegistryBacking::Fs { root, path } => {
                if !prepare_registry_directories(root, false)
                    .map_err(|_| ConnectorFailure::ActivationFailed)?
                {
                    None
                } else {
                    let metadata = match std::fs::symlink_metadata(path) {
                        Ok(metadata) => metadata,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            return Ok(RegistryFile::default())
                        }
                        Err(_) => return Err(ConnectorFailure::ActivationFailed),
                    };
                    if metadata.file_type().is_symlink() || !metadata.is_file() {
                        return Err(ConnectorFailure::ActivationFailed);
                    }
                    let file = File::open(path).map_err(|_| ConnectorFailure::ActivationFailed)?;
                    let length = file
                        .metadata()
                        .map_err(|_| ConnectorFailure::ActivationFailed)?
                        .len();
                    if length > MAX_REGISTRY_BYTES as u64 {
                        return Err(ConnectorFailure::ActivationFailed);
                    }
                    let mut bytes = Vec::with_capacity(length as usize);
                    file.take((MAX_REGISTRY_BYTES as u64).saturating_add(1))
                        .read_to_end(&mut bytes)
                        .map_err(|_| ConnectorFailure::ActivationFailed)?;
                    if bytes.len() > MAX_REGISTRY_BYTES {
                        return Err(ConnectorFailure::ActivationFailed);
                    }
                    Some(bytes)
                }
            }
            RegistryBacking::Memory(value) => value
                .lock()
                .map_err(|_| ConnectorFailure::ActivationFailed)?
                .clone(),
        };
        let file = match bytes {
            Some(bytes) => {
                serde_json::from_slice(&bytes).map_err(|_| ConnectorFailure::ActivationFailed)?
            }
            None => RegistryFile::default(),
        };
        file.validate()?;
        Ok(file)
    }

    fn persist(&self, file: &RegistryFile) -> ConnectorResult<()> {
        self.persist_with_fault(file, None)
    }

    pub fn persist_with_fault_for_test(
        &self,
        json: &[u8],
        fault: PersistenceFault,
    ) -> ConnectorResult<()> {
        let candidate: RegistryFile =
            serde_json::from_slice(json).map_err(|_| ConnectorFailure::ActivationFailed)?;
        candidate.validate()?;
        self.persist_with_fault(&candidate, Some(fault))
    }

    fn persist_with_fault(
        &self,
        file: &RegistryFile,
        fault: Option<PersistenceFault>,
    ) -> ConnectorResult<()> {
        file.validate()?;
        let bytes = serde_json::to_vec(file).map_err(|_| ConnectorFailure::ActivationFailed)?;
        if bytes.len() > MAX_REGISTRY_BYTES {
            return Err(ConnectorFailure::ActivationFailed);
        }
        match &self.backing {
            RegistryBacking::Memory(value) => {
                if fault == Some(PersistenceFault::BeforeRename) {
                    return Err(ConnectorFailure::ActivationFailed);
                }
                *value
                    .lock()
                    .map_err(|_| ConnectorFailure::ActivationFailed)? = Some(bytes);
                if fault == Some(PersistenceFault::AfterRename) {
                    return Err(ConnectorFailure::ActivationFailed);
                }
                Ok(())
            }
            RegistryBacking::Fs { root, path } => persist_fs(root, path, &bytes, fault)
                .map_err(|_| ConnectorFailure::ActivationFailed),
        }
    }
}

fn persist_fs(
    root: &Path,
    path: &Path,
    bytes: &[u8],
    fault: Option<PersistenceFault>,
) -> io::Result<()> {
    if !prepare_registry_directories(root, true)? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "registry directory was not created",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "registry has no parent"))?;
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".connectors.json.tmp-{}-{serial}",
        std::process::id()
    ));
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
    if fault == Some(PersistenceFault::BeforeRename) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "injected pre-rename crash",
        ));
    }
    std::fs::rename(&temp, path)?;
    File::open(parent)?.sync_all()?;
    if fault == Some(PersistenceFault::AfterRename) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "injected post-rename crash",
        ));
    }
    Ok(())
}

/// Validate the configured store root and then walk the two sidecar
/// components one at a time. `create_dir_all` on the full path would follow a
/// malicious `.aithos-sidecar` symlink before the final-directory check.
fn prepare_registry_directories(root: &Path, create: bool) -> io::Result<bool> {
    let root_metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
            std::fs::create_dir_all(root)?;
            std::fs::symlink_metadata(root)?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "registry root is not a plain directory",
        ));
    }
    let sidecar = root.join(SIDECAR_DIRECTORY);
    let gateway = sidecar.join("gateway");
    for directory in [&sidecar, &gateway] {
        match std::fs::symlink_metadata(directory) {
            Ok(_) => ensure_private_directory(directory)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                std::fs::create_dir(directory)?;
                ensure_private_directory(directory)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

fn ensure_private_directory(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "registry parent is not a plain directory",
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

#[derive(Clone)]
struct ConnectorTemplate {
    server: ServerConfig,
    oauth: UpstreamOAuthConfig,
}

pub struct ConnectorControl {
    runner: Arc<Mutex<Runner>>,
    dynamic_upstreams: DynamicUpstreams,
    oauth: Arc<UpstreamOAuthRegistry>,
    brokers: Arc<BTreeMap<String, Arc<dyn CredentialBroker>>>,
    templates: BTreeMap<String, ConnectorTemplate>,
    store: ConnectorRegistryStore,
    registry: StdMutex<RegistryFile>,
    operation: Mutex<()>,
    clock: Arc<dyn Fn() -> String + Send + Sync>,
}

impl ConnectorControl {
    pub fn from_config(
        config: &GatewayConfig,
        runner: Arc<Mutex<Runner>>,
        dynamic_upstreams: DynamicUpstreams,
        oauth: Arc<UpstreamOAuthRegistry>,
        brokers: BTreeMap<String, Arc<dyn CredentialBroker>>,
    ) -> Result<Self> {
        let journal = config.journal.as_ref().ok_or_else(|| {
            GatewayError::ConfigRejected("dashboard connectors need a journal sidecar".into())
        })?;
        let store = ConnectorRegistryStore::from_store_config(&journal.store)?;
        let templates = connector_templates(config)?;
        Self::new(
            store,
            runner,
            dynamic_upstreams,
            oauth,
            brokers,
            templates,
            Arc::new(system_now_rfc3339),
        )
    }

    /// Deterministic clock injection for acceptance/runtime composition.
    /// Production construction keeps the system clock selected above.
    pub fn with_clock(mut self, clock: Arc<dyn Fn() -> String + Send + Sync>) -> Self {
        self.clock = clock;
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        store: ConnectorRegistryStore,
        runner: Arc<Mutex<Runner>>,
        dynamic_upstreams: DynamicUpstreams,
        oauth: Arc<UpstreamOAuthRegistry>,
        brokers: BTreeMap<String, Arc<dyn CredentialBroker>>,
        templates: BTreeMap<String, ConnectorTemplate>,
        clock: Arc<dyn Fn() -> String + Send + Sync>,
    ) -> Result<Self> {
        let registry = store.load().map_err(|_| {
            GatewayError::ConfigRejected("connector registry is invalid or unavailable".into())
        })?;
        let control = Self {
            runner,
            dynamic_upstreams,
            oauth,
            brokers: Arc::new(brokers),
            templates,
            store,
            registry: StdMutex::new(registry),
            operation: Mutex::new(()),
            clock,
        };
        control.register_persisted_oauth()?;
        Ok(control)
    }

    fn register_persisted_oauth(&self) -> Result<()> {
        let connectors = self
            .registry
            .lock()
            .map_err(|_| GatewayError::ConfigRejected("connector registry lock failed".into()))?
            .connectors
            .clone();
        for connector in connectors {
            let template = self.templates.get(&connector.id).ok_or_else(|| {
                GatewayError::ConfigRejected(format!(
                    "connector `{}` no longer has an approved server template",
                    connector.id
                ))
            })?;
            let config = self.oauth_config(&connector, template);
            self.oauth.upsert(&connector.id, config, &self.brokers)?;
        }
        Ok(())
    }

    pub fn parse_stage(bytes: &[u8]) -> ConnectorResult<ConnectorStageRequest> {
        serde_json::from_slice(bytes).map_err(|_| ConnectorFailure::NotApproved)
    }

    pub fn parse_client_secret(bytes: &[u8]) -> ConnectorResult<ClientSecretBody> {
        let value: ClientSecretBody =
            serde_json::from_slice(bytes).map_err(|_| ConnectorFailure::SecretUnavailable)?;
        if value.valid() {
            Ok(value)
        } else {
            Err(ConnectorFailure::SecretUnavailable)
        }
    }

    pub fn list(&self, context: &str) -> ConnectorResult<ConnectorPage> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        Ok(ConnectorPage {
            v: REGISTRY_VERSION,
            items: registry
                .connectors
                .iter()
                .filter(|connector| connector.context == context)
                .map(PersistedConnector::view)
                .collect(),
            next_cursor: None,
        })
    }

    pub async fn stage(
        &self,
        principal_context: &str,
        path_id: &str,
        request: ConnectorStageRequest,
    ) -> ConnectorResult<ConnectorView> {
        let _operation = self.operation.lock().await;
        self.validate_stage(principal_context, path_id, &request)?;
        let template = self
            .templates
            .get(path_id)
            .ok_or(ConnectorFailure::NotApproved)?
            .clone();
        let (manifest, digest) = self
            .runner
            .lock()
            .await
            .approved_connector(principal_context, path_id)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        if manifest.server != request.approved_manifest.id
            || digest != request.approved_manifest.pin
        {
            return Err(ConnectorFailure::NotApproved);
        }
        let mut next = self.registry_snapshot()?;
        if let Some(current) = next
            .connectors
            .iter()
            .find(|connector| connector.id == path_id)
        {
            if current.context != principal_context {
                return Err(ConnectorFailure::NotApproved);
            }
            if current.active {
                return Err(ConnectorFailure::ActivationFailed);
            }
        }
        let connector = PersistedConnector {
            id: request.id,
            context: request.context,
            endpoint: request.endpoint,
            transport: request.transport,
            oauth: request.oauth,
            approved_manifest: request.approved_manifest,
            state: ConnectorState::Draft,
            active: false,
            secret_stored: false,
            live_digest: None,
        };
        // Validate the future OAuth client before the governance intent and
        // durable mutation. Broker lookup is local and secret-free.
        let oauth_config = self.oauth_config(&connector, &template);
        ensure_oauth_brokers(&oauth_config, &self.brokers)?;
        self.runner
            .lock()
            .await
            .record_connector_config(principal_context, path_id, "stage", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        next.replace(connector.clone());
        self.commit_registry(next)?;
        self.oauth
            .upsert(path_id, oauth_config, &self.brokers)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        Ok(connector.view())
    }

    pub async fn set_client_secret(
        &self,
        context: &str,
        id: &str,
        mut body: ClientSecretBody,
    ) -> ConnectorResult<ConnectorOAuthStatus> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        if connector.active {
            return Err(ConnectorFailure::ActivationFailed);
        }
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "client_secret", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let template = self
            .templates
            .get(id)
            .ok_or(ConnectorFailure::NotApproved)?;
        let config = self.oauth_config(&connector, template);
        let broker = self
            .brokers
            .get(&config.client_secret.broker)
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        broker
            .store(&config.client_secret, SecretValue::new(body.take()))
            .await
            .map_err(|_| ConnectorFailure::SecretUnavailable)?;
        connector.secret_stored = true;
        connector.state = ConnectorState::Disconnected;
        connector.active = false;
        next.replace(connector.clone());
        self.commit_registry(next)?;
        Ok(connector.oauth_status())
    }

    pub async fn start_oauth(
        &self,
        context: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorOAuthStart> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        if !connector.secret_stored || connector.active {
            return Err(ConnectorFailure::SecretUnavailable);
        }
        let template = self
            .templates
            .get(id)
            .ok_or(ConnectorFailure::NotApproved)?;
        let config = self.oauth_config(&connector, template);
        let secret_broker = self
            .brokers
            .get(&config.client_secret.broker)
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        let secret = secret_broker
            .resolve(&config.client_secret)
            .await
            .map_err(|_| ConnectorFailure::SecretUnavailable)?;
        drop(secret);
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "oauth_start", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let ConsentStart {
            authorization_url,
            expires_at,
        } = self
            .oauth
            .start(id)
            .await
            .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        connector.state = ConnectorState::Pending;
        next.replace(connector);
        self.commit_registry(next)?;
        Ok(ConnectorOAuthStart {
            v: REGISTRY_VERSION,
            authorization_url,
            expires_at: epoch_rfc3339(expires_at),
        })
    }

    pub async fn oauth_status(
        &self,
        context: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorOAuthStatus> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        let observed = if !connector.secret_stored {
            ConnectorState::SecretMissing
        } else {
            match self.oauth.public_state(id).await {
                UpstreamOAuthState::Pending { .. } => ConnectorState::Pending,
                UpstreamOAuthState::Connected => ConnectorState::Connected,
                UpstreamOAuthState::Expired => ConnectorState::Expired,
                UpstreamOAuthState::Unavailable => ConnectorState::Unavailable,
            }
        };
        // A failed authenticated discovery/refresh is sticky until a later
        // activation succeeds. Merely observing the still-present expired
        // token record must not turn an unavailable connector back into a
        // publicly connected one.
        connector.state = if connector.state == ConnectorState::Unavailable
            && matches!(
                observed,
                ConnectorState::Connected | ConnectorState::Expired
            ) {
            ConnectorState::Unavailable
        } else {
            observed
        };
        if connector.state != ConnectorState::Connected && connector.active {
            connector.active = false;
            connector.live_digest = None;
            self.disable_runtime(id).await;
        }
        next.replace(connector.clone());
        self.commit_registry(next)?;
        Ok(connector.oauth_status())
    }

    pub async fn activate(&self, context: &str, id: &str) -> ConnectorResult<ConnectorActivation> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        if !connector.secret_stored {
            return Err(ConnectorFailure::SecretUnavailable);
        }
        self.templates
            .get(id)
            .ok_or(ConnectorFailure::NotApproved)?;
        let (manifest, approved_digest) = self
            .runner
            .lock()
            .await
            .approved_connector(context, id)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        if connector.approved_manifest.id != manifest.server
            || connector.approved_manifest.pin != approved_digest
        {
            return Err(ConnectorFailure::NotApproved);
        }
        let client = self
            .oauth
            .get(id)
            .ok_or(ConnectorFailure::OauthUnavailable)?;
        let upstream = HttpUpstream::with_oauth_client(connector.endpoint.clone(), client);
        let observed = match discover_server(id, &upstream).await {
            Ok(observed) => observed,
            Err(GatewayError::UpstreamOauthUnavailable(_))
            | Err(GatewayError::CredentialUnavailable(_)) => {
                connector.state = ConnectorState::Unavailable;
                connector.active = false;
                connector.live_digest = None;
                next.replace(connector);
                self.disable_runtime(id).await;
                let _ = self.commit_registry(next);
                return Err(ConnectorFailure::OauthUnavailable);
            }
            Err(_) => {
                connector.state = ConnectorState::Unavailable;
                connector.active = false;
                connector.live_digest = None;
                next.replace(connector);
                self.disable_runtime(id).await;
                let _ = self.commit_registry(next);
                return Err(ConnectorFailure::UpstreamDenied);
            }
        };
        let live_digest = match proposed_manifest_catalog_digest(&observed) {
            Ok(digest) => digest,
            Err(_) => {
                connector.state = ConnectorState::Drifted;
                connector.active = false;
                connector.live_digest = None;
                next.replace(connector);
                self.disable_runtime(id).await;
                let _ = self.commit_registry(next);
                return Err(ConnectorFailure::ManifestDrift);
            }
        };
        let expected: BTreeMap<_, _> = manifest
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.pin_sha256.as_str()))
            .collect();
        let actual: BTreeMap<_, _> = observed
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.pin_sha256.as_str()))
            .collect();
        if expected != actual || approved_digest != live_digest {
            connector.state = ConnectorState::Drifted;
            connector.active = false;
            connector.live_digest = Some(live_digest);
            next.replace(connector);
            self.disable_runtime(id).await;
            let _ = self.commit_registry(next);
            return Err(ConnectorFailure::ManifestDrift);
        }
        let mut runner = self.runner.lock().await;
        runner
            .validate_hot_connector(context, id, &manifest)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        runner
            .record_connector_config(context, id, "activate", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        connector.state = ConnectorState::Connected;
        connector.active = true;
        connector.live_digest = Some(live_digest.clone());
        next.replace(connector.clone());
        self.commit_registry(next)?;
        let installed_upstream = match self.dynamic_upstreams.write() {
            Ok(mut upstreams) => {
                upstreams.insert(id.to_owned(), DynamicUpstream::new(upstream));
                true
            }
            Err(_) => false,
        };
        let installed_tools =
            installed_upstream && runner.install_hot_connector(context, id, &manifest).is_ok();
        if !installed_tools {
            runner.remove_hot_connector(id);
            if let Ok(mut upstreams) = self.dynamic_upstreams.write() {
                upstreams.remove(id);
            }
            connector.state = ConnectorState::Unavailable;
            connector.active = false;
            connector.live_digest = None;
            let mut closed = self.registry_snapshot()?;
            closed.replace(connector);
            // Keep the live view fail-closed even if the rollback itself
            // cannot reach disk. A complete active disk record is still
            // independently revalidated on restart; no half JSON is written.
            let _ = self.store.persist(&closed);
            *self
                .registry
                .lock()
                .map_err(|_| ConnectorFailure::ActivationFailed)? = closed;
            return Err(ConnectorFailure::ActivationFailed);
        }
        Ok(ConnectorActivation {
            v: REGISTRY_VERSION,
            connector: connector.view(),
            approved_digest,
            live_digest,
        })
    }

    pub async fn delete_draft(&self, context: &str, id: &str) -> ConnectorResult<()> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        next.get(context, id)?;
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "delete_draft", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        next.connectors
            .retain(|connector| !(connector.id == id && connector.context == context));
        self.commit_registry(next)?;
        self.disable_runtime(id).await;
        self.oauth.remove(id);
        Ok(())
    }

    /// Revalidate each active record independently at startup. Corrupt
    /// registry syntax fails construction; connector-local custody/drift
    /// failures close that connector and never prevent a healthy neighbor.
    pub async fn restore(&self) -> ConnectorResult<()> {
        let _operation = self.operation.lock().await;
        let active = self
            .registry_snapshot()?
            .connectors
            .into_iter()
            .filter(|connector| connector.active)
            .collect::<Vec<_>>();
        for connector in active {
            if self.restore_one(&connector).await.is_err() {
                let mut next = self.registry_snapshot()?;
                if let Ok(current) = next.get(&connector.context, &connector.id) {
                    let mut closed = current.clone();
                    closed.active = false;
                    closed.state = ConnectorState::Unavailable;
                    closed.live_digest = None;
                    next.replace(closed);
                    let _ = self.commit_registry(next);
                }
                self.disable_runtime(&connector.id).await;
            }
        }
        Ok(())
    }

    async fn restore_one(&self, connector: &PersistedConnector) -> ConnectorResult<()> {
        let (manifest, approved_digest) = self
            .runner
            .lock()
            .await
            .approved_connector(&connector.context, &connector.id)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        if connector.approved_manifest.pin != approved_digest {
            return Err(ConnectorFailure::ManifestDrift);
        }
        let client = self
            .oauth
            .get(&connector.id)
            .ok_or(ConnectorFailure::OauthUnavailable)?;
        let upstream = HttpUpstream::with_oauth_client(connector.endpoint.clone(), client);
        let observed = discover_server(&connector.id, &upstream)
            .await
            .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        let live_digest = proposed_manifest_catalog_digest(&observed)
            .map_err(|_| ConnectorFailure::ManifestDrift)?;
        if live_digest != approved_digest {
            return Err(ConnectorFailure::ManifestDrift);
        }
        self.dynamic_upstreams
            .write()
            .map_err(|_| ConnectorFailure::ActivationFailed)?
            .insert(connector.id.clone(), DynamicUpstream::new(upstream));
        self.runner
            .lock()
            .await
            .install_hot_connector(&connector.context, &connector.id, &manifest)
            .map_err(|_| ConnectorFailure::ActivationFailed)
    }

    async fn disable_runtime(&self, id: &str) {
        self.runner.lock().await.remove_hot_connector(id);
        if let Ok(mut upstreams) = self.dynamic_upstreams.write() {
            upstreams.remove(id);
        }
    }

    fn registry_snapshot(&self) -> ConnectorResult<RegistryFile> {
        self.registry
            .lock()
            .map_err(|_| ConnectorFailure::ActivationFailed)
            .map(|registry| registry.clone())
    }

    fn commit_registry(&self, next: RegistryFile) -> ConnectorResult<()> {
        self.store.persist(&next)?;
        *self
            .registry
            .lock()
            .map_err(|_| ConnectorFailure::ActivationFailed)? = next;
        Ok(())
    }

    fn validate_stage(
        &self,
        principal_context: &str,
        path_id: &str,
        request: &ConnectorStageRequest,
    ) -> ConnectorResult<()> {
        if request.v != REGISTRY_VERSION
            || request.id != path_id
            || request.context != principal_context
            || !valid_id(path_id)
            || !valid_id(&request.context)
            || request.approved_manifest.id != path_id
            || request.approved_manifest.pin.len() > 128
            || !request.approved_manifest.pin.starts_with("sha256:")
            || !valid_oauth_descriptor(&request.oauth)
        {
            return Err(ConnectorFailure::NotApproved);
        }
        let template = self
            .templates
            .get(path_id)
            .ok_or(ConnectorFailure::NotApproved)?;
        if request.endpoint != template.server.url
            || request.oauth.authorization_endpoint != template.oauth.auth_url
            || request.oauth.token_endpoint != template.oauth.token_url
            || request.oauth.client_id != template.oauth.client_id
            || request.oauth.scopes != template.oauth.scopes
            || request.oauth.redirect_uri != template.oauth.redirect_uri
        {
            return Err(ConnectorFailure::NotApproved);
        }
        Ok(())
    }

    fn oauth_config(
        &self,
        connector: &PersistedConnector,
        template: &ConnectorTemplate,
    ) -> UpstreamOAuthConfig {
        let client_secret = derived_reference(
            &template.oauth.client_secret.broker,
            &connector.id,
            &connector.oauth.client_secret_record,
        );
        let pending = derived_reference(
            &template.oauth.token_vault.broker,
            &connector.id,
            &connector.oauth.pending_record,
        );
        let token = derived_reference(
            &template.oauth.token_vault.broker,
            &connector.id,
            &connector.oauth.token_record,
        );
        UpstreamOAuthConfig {
            auth_url: connector.oauth.authorization_endpoint.clone(),
            token_url: connector.oauth.token_endpoint.clone(),
            client_id: connector.oauth.client_id.clone(),
            client_secret,
            scopes: connector.oauth.scopes.clone(),
            redirect_uri: connector.oauth.redirect_uri.clone(),
            pending_vault: Some(pending),
            token_vault: token,
        }
    }
}

fn connector_templates(config: &GatewayConfig) -> Result<BTreeMap<String, ConnectorTemplate>> {
    let mut templates = BTreeMap::new();
    for server in config.servers.as_deref().unwrap_or_default() {
        let Some(oauth) = &server.oauth else {
            continue;
        };
        templates.insert(
            server.name.clone(),
            ConnectorTemplate {
                server: server.clone(),
                oauth: oauth.clone(),
            },
        );
    }
    Ok(templates)
}

fn ensure_oauth_brokers(
    config: &UpstreamOAuthConfig,
    brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
) -> ConnectorResult<()> {
    let pending = config
        .pending_vault
        .as_ref()
        .ok_or(ConnectorFailure::ActivationFailed)?;
    if brokers.contains_key(&config.client_secret.broker)
        && brokers.contains_key(&pending.broker)
        && brokers.contains_key(&config.token_vault.broker)
    {
        Ok(())
    } else {
        Err(ConnectorFailure::ActivationFailed)
    }
}

fn derived_reference(broker: &str, connector: &str, record: &str) -> CredentialRef {
    CredentialRef {
        broker: broker.to_owned(),
        path: format!("{VAULT_RECORD_PREFIX}/{connector}/{record}"),
        field: VAULT_RECORD_FIELD.to_owned(),
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && valid_server_name(value)
        && !matches!(value, "journal" | "gateway" | "briefing" | "ethos")
}

fn valid_oauth_descriptor(oauth: &ConnectorOAuthDescriptor) -> bool {
    let urls = [
        oauth.authorization_endpoint.as_str(),
        oauth.token_endpoint.as_str(),
        oauth.redirect_uri.as_str(),
    ];
    urls.iter()
        .all(|url| !url.is_empty() && url.len() <= MAX_URL_BYTES)
        && !oauth.client_id.is_empty()
        && oauth.client_id.len() <= MAX_CLIENT_ID_BYTES
        && !oauth.scopes.is_empty()
        && oauth.scopes.len() <= MAX_SCOPES
        && oauth
            .scopes
            .iter()
            .all(|scope| !scope.is_empty() && scope.len() <= MAX_SCOPE_BYTES)
        && oauth.scopes.iter().collect::<BTreeSet<_>>().len() == oauth.scopes.len()
        && [
            oauth.client_secret_record.as_str(),
            oauth.pending_record.as_str(),
            oauth.token_record.as_str(),
        ]
        .iter()
        .all(|record| valid_id(record))
        && BTreeSet::from([
            oauth.client_secret_record.as_str(),
            oauth.pending_record.as_str(),
            oauth.token_record.as_str(),
        ])
        .len()
            == 3
}

fn valid_public_descriptor(connector: &PersistedConnector) -> bool {
    connector.endpoint.len() <= MAX_URL_BYTES
        && !connector.endpoint.is_empty()
        && valid_oauth_descriptor(&connector.oauth)
        && connector.approved_manifest.pin.len() <= 128
        && connector.approved_manifest.pin.starts_with("sha256:")
}

fn system_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn system_now_rfc3339() -> String {
    epoch_rfc3339(system_epoch())
}

fn epoch_rfc3339(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let remaining = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remaining / 3_600,
        (remaining % 3_600) / 60,
        remaining % 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (y + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(id: &str) -> RegistryFile {
        RegistryFile {
            v: 1,
            connectors: vec![PersistedConnector {
                id: id.to_owned(),
                context: "operations".into(),
                endpoint: "https://mcp.example.test/mcp".into(),
                transport: ConnectorTransport::StreamableHttp,
                oauth: ConnectorOAuthDescriptor {
                    authorization_endpoint: "https://as.example.test/authorize".into(),
                    token_endpoint: "https://as.example.test/token".into(),
                    client_id: "aithos-enterprise".into(),
                    scopes: vec!["calendar.read".into()],
                    redirect_uri: "https://acme.mcp.aithos.fr/oauth/callback".into(),
                    client_secret_record: "calendar-client".into(),
                    pending_record: "calendar-pending".into(),
                    token_record: "calendar-token".into(),
                },
                approved_manifest: ApprovedManifestRef {
                    id: id.to_owned(),
                    pin: "sha256:approved".into(),
                },
                state: ConnectorState::Draft,
                active: false,
                secret_stored: false,
                live_digest: None,
            }],
        }
    }

    #[test]
    fn memory_persistence_fault_is_old_or_complete_new() {
        let store = ConnectorRegistryStore::memory();
        store.persist(&registry("calendar-safe")).unwrap();
        let replacement = serde_json::to_vec(&registry("crm-safe")).unwrap();
        assert!(store
            .persist_with_fault_for_test(&replacement, PersistenceFault::BeforeRename)
            .is_err());
        assert_eq!(store.load().unwrap(), registry("calendar-safe"));
        assert!(store
            .persist_with_fault_for_test(&replacement, PersistenceFault::AfterRename)
            .is_err());
        assert_eq!(store.load().unwrap(), registry("crm-safe"));
    }

    #[test]
    fn secret_body_is_closed_and_bounded() {
        assert!(ConnectorControl::parse_client_secret(br#"{"client_secret":"ok"}"#).is_ok());
        assert!(ConnectorControl::parse_client_secret(
            br#"{"client_secret":"ok","path":"vault/root"}"#
        )
        .is_err());
    }

    #[test]
    fn epoch_rendering_is_stable() {
        assert_eq!(epoch_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_rfc3339(1_784_203_200), "2026-07-16T12:00:00Z");
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_registry_refuses_sidecar_and_file_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("journal");
        let outside = directory.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, root.join(SIDECAR_DIRECTORY)).unwrap();
        let config = StoreConfig::Fs { root: root.clone() };
        let store = ConnectorRegistryStore::from_store_config(&config).unwrap();
        assert_eq!(
            store.persist(&registry("calendar-safe")),
            Err(ConnectorFailure::ActivationFailed)
        );
        assert!(!outside.join("gateway").exists());

        std::fs::remove_file(root.join(SIDECAR_DIRECTORY)).unwrap();
        store.persist(&registry("calendar-safe")).unwrap();
        let path = root.join(SIDECAR_DIRECTORY).join(REGISTRY_RELATIVE_PATH);
        std::fs::remove_file(&path).unwrap();
        let outside_file = outside.join("registry.json");
        std::fs::write(&outside_file, br#"{"v":1,"connectors":[]}"#).unwrap();
        symlink(&outside_file, &path).unwrap();
        assert_eq!(store.load(), Err(ConnectorFailure::ActivationFailed));
    }
}
