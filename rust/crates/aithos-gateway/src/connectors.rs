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
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zeroize::Zeroize;

use crate::compiled_extensions::{
    compiled_manifest, ApprovalReview, ApprovalView, CompiledExtensionUpstream,
    GmailSendGuardedUpstream, GmailSendPolicy, GoogleSheetsReadConfig, GoogleSheetsWriteConfig,
};
use crate::config::{
    CompiledConnectorAdapter, CompiledConnectorSettings, ConnectorExecutionProfile, GatewayConfig,
    OAuthClientAuthentication, ServerConfig, StoreConfig, UpstreamOAuthConfig,
};
use crate::connector_profiles::{
    ConnectorInstanceKey, ConnectorProfileCatalog, ConnectorProfileRef, OAuthVaultLayout,
};
use crate::core_bridge::{proposed_manifest_catalog_digest, ConnectorEffectProof, Runner};
use crate::credentials::{CredentialBroker, CredentialRef, SecretValue};
use crate::hub::discover_server;
use crate::hub::{ApprovedManifest, ProposedManifest};
use crate::policy::{hub_exposed_name, valid_server_name};
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
const VAULT_BEARER_RECORD: &str = "bearer";
const SIDECAR_DIRECTORY: &str = ".aithos-sidecar";
const REGISTRY_RELATIVE_PATH: &str = "gateway/connectors.json";
const DEFAULT_CREDENTIAL_BROKER: &str = "enterprise";

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

fn legacy_principal() -> String {
    "legacy".to_owned()
}

fn legacy_account() -> String {
    "default".to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorState {
    Draft,
    SecretMissing,
    Disconnected,
    Pending,
    Connected,
    Expired,
    ReauthRequired,
    Drifted,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorCleanupState {
    #[default]
    Clean,
    VaultResidue,
    RevocationResidue,
    VaultAndRevocationResidue,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorProfileStageRequest {
    pub v: u8,
    pub id: String,
    pub context: String,
    pub profile: ConnectorProfileRef,
}

/// Dynamic bearer / credential-only MCP staging (no OAuth template).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorCredentialStageRequest {
    pub v: u8,
    pub id: String,
    pub context: String,
    pub endpoint: String,
    pub transport: ConnectorTransport,
    pub auth: ConnectorCredentialAuth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorCredentialAuth {
    Bearer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum ConnectorAuthKind {
    #[default]
    Oauth,
    Bearer,
}

/// Parsed separately from every other DTO so no derive can ever print or
/// clone the secret. The string zeroizes even when validation refuses it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientSecretBody {
    client_secret: String,
}

/// Bearer token body for credential-stage connectors. Zeroized on drop.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BearerSecretBody {
    bearer_token: String,
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

impl BearerSecretBody {
    fn take(&mut self) -> String {
        std::mem::take(&mut self.bearer_token)
    }

    fn valid(&self) -> bool {
        !self.bearer_token.is_empty()
            && self.bearer_token.len() <= MAX_CLIENT_SECRET_BYTES
            && !self.bearer_token.contains('\0')
    }
}

impl Drop for BearerSecretBody {
    fn drop(&mut self) {
        self.bearer_token.zeroize();
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
    pub cleanup: ConnectorCleanupState,
    pub approved_manifest: ApprovedManifestRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<ConnectorProfileRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
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
    #[serde(default)]
    auth: ConnectorAuthKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oauth: Option<ConnectorOAuthDescriptor>,
    approved_manifest: ApprovedManifestRef,
    #[serde(default)]
    profile: Option<ConnectorProfileRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile_pin: Option<String>,
    #[serde(default = "legacy_principal")]
    principal: String,
    #[serde(default = "legacy_account")]
    account: String,
    state: ConnectorState,
    active: bool,
    #[serde(default)]
    cleanup: ConnectorCleanupState,
    secret_stored: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live_digest: Option<String>,
}

impl PersistedConnector {
    fn is_bearer(&self) -> bool {
        self.auth == ConnectorAuthKind::Bearer
    }

    fn view(&self) -> ConnectorView {
        ConnectorView {
            v: REGISTRY_VERSION,
            id: self.id.clone(),
            context: self.context.clone(),
            endpoint: self.endpoint.clone(),
            transport: self.transport,
            state: self.state,
            active: self.active,
            cleanup: self.cleanup,
            approved_manifest: self.approved_manifest.clone(),
            profile: self.profile.clone(),
            account_id: self.profile.as_ref().map(|_| self.account.clone()),
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
                || (connector.profile.is_none() && connector.approved_manifest.id != connector.id)
                || (connector.profile.is_some()
                    && ConnectorInstanceKey::new(
                        &connector.context,
                        &connector.principal,
                        &connector.id,
                        &connector.account,
                    )
                    .is_err())
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
    profiles: ConnectorProfileCatalog,
    gmail_extensions: StdRwLock<BTreeMap<String, GmailSendGuardedUpstream>>,
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
        let profiles = ConnectorProfileCatalog::from_config(config);
        Self::new(
            store,
            runner,
            dynamic_upstreams,
            oauth,
            brokers,
            templates,
            profiles,
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
        profiles: ConnectorProfileCatalog,
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
            profiles,
            gmail_extensions: StdRwLock::new(BTreeMap::new()),
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
            if connector.is_bearer() {
                continue;
            }
            let config = if connector.profile.is_some() {
                match self.profile_oauth_config(&connector) {
                    Ok(config) => config,
                    Err(_) => {
                        let mut registry = self.registry.lock().map_err(|_| {
                            GatewayError::ConfigRejected("connector registry lock failed".into())
                        })?;
                        let mut closed = connector.clone();
                        closed.active = false;
                        closed.state = ConnectorState::Drifted;
                        closed.live_digest = None;
                        registry.replace(closed);
                        let _ = self.store.persist(&registry);
                        continue;
                    }
                }
            } else {
                let template = self.templates.get(&connector.id).ok_or_else(|| {
                    GatewayError::ConfigRejected(format!(
                        "connector `{}` no longer has an approved server template",
                        connector.id
                    ))
                })?;
                self.oauth_config(&connector, template)
            };
            self.oauth.upsert(&connector.id, config, &self.brokers)?;
        }
        Ok(())
    }

    pub fn parse_stage(bytes: &[u8]) -> ConnectorResult<ConnectorStageRequest> {
        serde_json::from_slice(bytes).map_err(|_| ConnectorFailure::NotApproved)
    }

    pub fn parse_profile_stage(bytes: &[u8]) -> ConnectorResult<ConnectorProfileStageRequest> {
        serde_json::from_slice(bytes).map_err(|_| ConnectorFailure::NotApproved)
    }

    pub fn parse_credential_stage(
        bytes: &[u8],
    ) -> ConnectorResult<ConnectorCredentialStageRequest> {
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

    pub fn parse_bearer_secret(bytes: &[u8]) -> ConnectorResult<BearerSecretBody> {
        let value: BearerSecretBody =
            serde_json::from_slice(bytes).map_err(|_| ConnectorFailure::SecretUnavailable)?;
        if value.valid() {
            Ok(value)
        } else {
            Err(ConnectorFailure::SecretUnavailable)
        }
    }

    pub fn list(&self, context: &str, principal_id: &str) -> ConnectorResult<ConnectorPage> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        Ok(ConnectorPage {
            v: REGISTRY_VERSION,
            items: registry
                .connectors
                .iter()
                .filter(|connector| {
                    connector.context == context
                        && (connector.profile.is_none() || connector.principal == principal_id)
                })
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
            auth: ConnectorAuthKind::Oauth,
            oauth: Some(request.oauth),
            approved_manifest: request.approved_manifest,
            profile: None,
            profile_pin: None,
            principal: legacy_principal(),
            account: legacy_account(),
            state: ConnectorState::Draft,
            active: false,
            cleanup: ConnectorCleanupState::Clean,
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

    pub async fn stage_profile(
        &self,
        principal_context: &str,
        principal_id: &str,
        path_id: &str,
        request: ConnectorProfileStageRequest,
    ) -> ConnectorResult<ConnectorView> {
        let _operation = self.operation.lock().await;
        if request.v != REGISTRY_VERSION
            || request.id != path_id
            || request.context != principal_context
            || !valid_id(path_id)
        {
            return Err(ConnectorFailure::NotApproved);
        }
        let profile = self
            .profiles
            .enabled(&request.profile)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        let profile_pin = self
            .profiles
            .pin(&request.profile)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        let (endpoint, manifest_id, manifest_pin) = match &profile.execution {
            ConnectorExecutionProfile::Mcp {
                endpoint,
                manifest_id,
                manifest_pin,
            } => (endpoint.clone(), manifest_id.clone(), manifest_pin.clone()),
            ConnectorExecutionProfile::CompiledRest {
                api_base_url,
                manifest_id,
                manifest_pin,
                ..
            } => (
                api_base_url.clone(),
                manifest_id.clone(),
                manifest_pin.clone(),
            ),
        };
        let (_, sealed_digest) = self
            .runner
            .lock()
            .await
            .approved_connector(principal_context, &manifest_id)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        if sealed_digest != manifest_pin {
            return Err(ConnectorFailure::NotApproved);
        }
        let mut next = self.registry_snapshot()?;
        if next
            .connectors
            .iter()
            .any(|connector| connector.id == path_id)
        {
            return Err(ConnectorFailure::NotApproved);
        }
        let account = ConnectorInstanceKey::issue_account_id();
        let key = ConnectorInstanceKey::new(principal_context, principal_id, path_id, &account)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        let layout = OAuthVaultLayout::derive(&profile.oauth.credential_broker, &key);
        let config = self
            .profiles
            .materialize_oauth(&request.profile, &layout)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        ensure_oauth_brokers(&config, &self.brokers)?;
        let secret_stored = profile.oauth.client_authentication == OAuthClientAuthentication::None
            || !matches!(
                profile.oauth.registration,
                crate::config::ConnectorProfileRegistration::Static
            );
        let connector = PersistedConnector {
            id: request.id,
            context: request.context,
            endpoint,
            transport: ConnectorTransport::StreamableHttp,
            auth: ConnectorAuthKind::Oauth,
            oauth: Some(ConnectorOAuthDescriptor {
                authorization_endpoint: profile.oauth.auth_url.clone(),
                token_endpoint: profile.oauth.token_url.clone(),
                client_id: profile.oauth.client_id.clone(),
                scopes: profile.oauth.scopes.clone(),
                redirect_uri: profile.oauth.redirect_uri.clone(),
                client_secret_record: "client-secret".into(),
                pending_record: "pending".into(),
                token_record: "token".into(),
            }),
            approved_manifest: ApprovedManifestRef {
                id: manifest_id,
                pin: manifest_pin,
            },
            profile: Some(request.profile),
            profile_pin: Some(profile_pin),
            principal: principal_id.to_owned(),
            account,
            state: if secret_stored {
                ConnectorState::Disconnected
            } else {
                ConnectorState::SecretMissing
            },
            active: false,
            cleanup: ConnectorCleanupState::Clean,
            secret_stored,
            live_digest: None,
        };
        self.runner
            .lock()
            .await
            .record_connector_config(principal_context, path_id, "stage_profile", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        self.oauth
            .upsert(path_id, config, &self.brokers)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        next.replace(connector.clone());
        self.commit_registry(next)?;
        Ok(connector.view())
    }

    pub async fn stage_credential(
        &self,
        principal_context: &str,
        path_id: &str,
        request: ConnectorCredentialStageRequest,
    ) -> ConnectorResult<ConnectorView> {
        let _operation = self.operation.lock().await;
        self.validate_credential_stage(principal_context, path_id, &request)?;
        // OAuth-template ids stay on the governed OAuth stage path.
        if self.templates.contains_key(path_id) {
            return Err(ConnectorFailure::NotApproved);
        }
        let mut next = self.registry_snapshot()?;
        if let Some(current) = next
            .connectors
            .iter()
            .find(|connector| connector.id == path_id)
        {
            if current.context != principal_context || !current.is_bearer() {
                return Err(ConnectorFailure::NotApproved);
            }
            if current.active {
                return Err(ConnectorFailure::ActivationFailed);
            }
        }
        let broker = credential_broker_name(&self.brokers)?;
        // Touch the broker map early so a misconfigured gateway fails before
        // the durable draft is written.
        if !self.brokers.contains_key(&broker) {
            return Err(ConnectorFailure::SecretUnavailable);
        }
        let connector = PersistedConnector {
            id: request.id,
            context: request.context,
            endpoint: request.endpoint,
            transport: request.transport,
            auth: ConnectorAuthKind::Bearer,
            oauth: None,
            approved_manifest: ApprovedManifestRef {
                id: path_id.to_owned(),
                // Filled on first successful activate (owner TOFU digest).
                pin: String::new(),
            },
            profile: None,
            profile_pin: None,
            principal: legacy_principal(),
            account: legacy_account(),
            state: ConnectorState::Draft,
            active: false,
            cleanup: ConnectorCleanupState::Clean,
            secret_stored: false,
            live_digest: None,
        };
        self.runner
            .lock()
            .await
            .record_connector_config(
                principal_context,
                path_id,
                "credential-stage",
                &(self.clock)(),
            )
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        next.replace(connector.clone());
        self.commit_registry(next)?;
        Ok(connector.view())
    }

    pub async fn set_client_secret(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
        mut body: ClientSecretBody,
    ) -> ConnectorResult<ConnectorOAuthStatus> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if connector.is_bearer() {
            return Err(ConnectorFailure::NotApproved);
        }
        if connector.active {
            return Err(ConnectorFailure::ActivationFailed);
        }
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "client_secret", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let config = self.runtime_oauth_config(&connector)?;
        if self.oauth.get(id).is_none() {
            self.oauth
                .upsert(id, config.clone(), &self.brokers)
                .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        }
        let reference = config
            .client_secret
            .as_ref()
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        let broker = self
            .brokers
            .get(&reference.broker)
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        broker
            .store(reference, SecretValue::new(body.take()))
            .await
            .map_err(|_| ConnectorFailure::SecretUnavailable)?;
        connector.secret_stored = true;
        connector.state = ConnectorState::Disconnected;
        connector.active = false;
        next.replace(connector.clone());
        self.commit_registry(next)?;
        Ok(connector.oauth_status())
    }

    pub async fn set_bearer_secret(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
        mut body: BearerSecretBody,
    ) -> ConnectorResult<ConnectorOAuthStatus> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if !connector.is_bearer() {
            return Err(ConnectorFailure::NotApproved);
        }
        if connector.active {
            return Err(ConnectorFailure::ActivationFailed);
        }
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "bearer_secret", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let reference = self.bearer_credential_ref(id)?;
        let broker = self
            .brokers
            .get(&reference.broker)
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        broker
            .store(&reference, SecretValue::new(body.take()))
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
        principal_id: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorOAuthStart> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if connector.is_bearer() {
            return Err(ConnectorFailure::NotApproved);
        }
        if !connector.secret_stored || connector.active {
            return Err(ConnectorFailure::SecretUnavailable);
        }
        let config = self.runtime_oauth_config(&connector)?;
        if self.oauth.get(id).is_none() {
            self.oauth
                .upsert(id, config.clone(), &self.brokers)
                .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        }
        if let Some(reference) = config.client_secret.as_ref() {
            let secret_broker = self
                .brokers
                .get(&reference.broker)
                .ok_or(ConnectorFailure::SecretUnavailable)?;
            let secret = secret_broker
                .resolve(reference)
                .await
                .map_err(|_| ConnectorFailure::SecretUnavailable)?;
            drop(secret);
        }
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "oauth_start", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let ConsentStart {
            authorization_url,
            expires_at,
            ..
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
        principal_id: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorOAuthStatus> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if connector.is_bearer() {
            return Ok(connector.oauth_status());
        }
        let observed = if !connector.secret_stored {
            ConnectorState::SecretMissing
        } else {
            match self.oauth.public_state(id).await {
                UpstreamOAuthState::Pending { .. } => ConnectorState::Pending,
                UpstreamOAuthState::Connected => ConnectorState::Connected,
                UpstreamOAuthState::Expired => ConnectorState::Expired,
                UpstreamOAuthState::ReauthRequired => ConnectorState::ReauthRequired,
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

    pub async fn activate(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorActivation> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if !connector.secret_stored {
            return Err(ConnectorFailure::SecretUnavailable);
        }
        if connector.is_bearer() {
            return self
                .activate_bearer(context, id, next, connector)
                .await;
        }
        connector.state = match self.oauth.public_state(id).await {
            UpstreamOAuthState::Pending { .. } => ConnectorState::Pending,
            UpstreamOAuthState::Connected => ConnectorState::Connected,
            UpstreamOAuthState::Expired => ConnectorState::Expired,
            UpstreamOAuthState::ReauthRequired => ConnectorState::ReauthRequired,
            UpstreamOAuthState::Unavailable => ConnectorState::Unavailable,
        };
        if !matches!(
            connector.state,
            ConnectorState::Connected | ConnectorState::Expired
        ) {
            return Err(ConnectorFailure::OauthUnavailable);
        }
        if let Some(profile) = &connector.profile {
            let current_pin = self
                .profiles
                .pin(profile)
                .map_err(|_| ConnectorFailure::NotApproved)?;
            if connector.profile_pin.as_deref() != Some(current_pin.as_str()) {
                return Err(ConnectorFailure::ManifestDrift);
            }
        } else {
            self.templates
                .get(id)
                .ok_or(ConnectorFailure::NotApproved)?;
        }
        let (manifest, approved_digest) = self
            .runner
            .lock()
            .await
            .approved_connector(context, &connector.approved_manifest.id)
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
        let execution = connector
            .profile
            .as_ref()
            .map(|profile| self.profiles.execution(profile))
            .transpose()
            .map_err(|_| ConnectorFailure::NotApproved)?;
        let (observed, runtime_upstream, gmail_runtime) = match execution {
            Some(ConnectorExecutionProfile::CompiledRest {
                adapter, settings, ..
            }) => {
                let observed = compiled_manifest(&connector.approved_manifest.id, *adapter)
                    .map_err(|_| ConnectorFailure::NotApproved)?;
                let upstream = match settings {
                    CompiledConnectorSettings::GoogleSheetsRead {
                        allowed_ranges,
                        max_response_bytes,
                    } => {
                        let mut config = GoogleSheetsReadConfig::new(
                            connector.endpoint.clone(),
                            allowed_ranges
                                .iter()
                                .map(|(spreadsheet, ranges)| {
                                    (spreadsheet.clone(), ranges.iter().cloned().collect())
                                })
                                .collect(),
                        );
                        config.max_response_bytes = *max_response_bytes;
                        CompiledExtensionUpstream::google_sheets_read(config, Arc::clone(&client))
                    }
                    CompiledConnectorSettings::GmailSendGuarded {
                        allowed_recipients,
                        allowed_domains,
                        max_recipients,
                        max_subject_bytes,
                        max_body_bytes,
                        approval_ttl_seconds,
                    } => {
                        let mut policy = GmailSendPolicy::new(connector.endpoint.clone());
                        policy.allowed_recipients = allowed_recipients.iter().cloned().collect();
                        policy.allowed_domains = allowed_domains.iter().cloned().collect();
                        policy.max_recipients = *max_recipients;
                        policy.max_subject_bytes = *max_subject_bytes;
                        policy.max_body_bytes = *max_body_bytes;
                        policy.approval_ttl_seconds = *approval_ttl_seconds;
                        let (broker, reference) = self.compiled_outbox_storage(&connector)?;
                        CompiledExtensionUpstream::gmail_send_guarded_durable(
                            policy,
                            Arc::clone(&client),
                            broker,
                            reference,
                        )
                    }
                    CompiledConnectorSettings::GoogleSheetsWriteGuarded {
                        allowed_ranges,
                        max_cells,
                        max_request_bytes,
                    } => {
                        let mut config = GoogleSheetsWriteConfig::new(
                            connector.endpoint.clone(),
                            allowed_ranges
                                .iter()
                                .map(|(spreadsheet, ranges)| {
                                    (spreadsheet.clone(), ranges.iter().cloned().collect())
                                })
                                .collect(),
                        );
                        config.max_cells = *max_cells;
                        config.max_request_bytes = *max_request_bytes;
                        CompiledExtensionUpstream::google_sheets_write_guarded(
                            config,
                            Arc::clone(&client),
                        )
                    }
                }
                .map_err(|_| ConnectorFailure::NotApproved)?;
                upstream
                    .hydrate()
                    .await
                    .map_err(|_| ConnectorFailure::ActivationFailed)?;
                let gmail = upstream.gmail().cloned();
                (observed, DynamicUpstream::new(upstream), gmail)
            }
            Some(ConnectorExecutionProfile::Mcp { .. }) | None => {
                let upstream = HttpUpstream::with_oauth_client(
                    connector.endpoint.clone(),
                    Arc::clone(&client),
                );
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
                (observed, DynamicUpstream::new(upstream), None)
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
        let runtime_manifest = instance_manifest(&connector, &manifest);
        let mut runner = self.runner.lock().await;
        runner
            .validate_hot_connector(context, id, &runtime_manifest)
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
                upstreams.insert(id.to_owned(), runtime_upstream);
                true
            }
            Err(_) => false,
        };
        let installed_gmail = gmail_runtime.is_none()
            || self.gmail_extensions.write().is_ok_and(|mut extensions| {
                extensions.insert(
                    id.to_owned(),
                    gmail_runtime.expect("Gmail runtime was checked"),
                );
                true
            });
        let installed_tools = installed_upstream
            && installed_gmail
            && runner
                .install_hot_connector(context, id, &runtime_manifest)
                .is_ok();
        if !installed_tools {
            runner.remove_hot_connector(id);
            if let Ok(mut upstreams) = self.dynamic_upstreams.write() {
                upstreams.remove(id);
            }
            if let Ok(mut extensions) = self.gmail_extensions.write() {
                extensions.remove(id);
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

    pub async fn delete_draft(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorCleanupState> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if connector.active {
            return Err(ConnectorFailure::ActivationFailed);
        }
        if connector.is_bearer() {
            self.runner
                .lock()
                .await
                .record_connector_config(context, id, "delete_draft", &(self.clock)())
                .map_err(|_| ConnectorFailure::ActivationFailed)?;
            self.disable_runtime(id).await;
            let vault_clean = self.delete_bearer_secret(id).await;
            next.connectors
                .retain(|candidate| !(candidate.id == id && candidate.context == context));
            self.commit_registry(next)?;
            return Ok(if vault_clean {
                ConnectorCleanupState::Clean
            } else {
                ConnectorCleanupState::VaultResidue
            });
        }
        let config = self
            .runtime_oauth_config(&connector)
            .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        if self.oauth.get(id).is_none() {
            self.oauth
                .upsert(id, config.clone(), &self.brokers)
                .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        }
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "delete_draft", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        self.disable_runtime(id).await;
        let outcome = self
            .oauth
            .disconnect(id)
            .await
            .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        let mut vault_clean = outcome.vault_cleanup_clean;
        if outcome.revocation_clean {
            if let Some(reference) = &config.client_secret {
                vault_clean &= matches!(
                    self.brokers
                        .get(&reference.broker)
                        .ok_or(ConnectorFailure::SecretUnavailable)?
                        .delete(reference)
                        .await,
                    Ok(crate::credentials::CredentialDeleteOutcome::Deleted)
                );
            }
        }
        if connector.profile.is_some() && outcome.revocation_clean {
            vault_clean &= match self.compiled_outbox_storage(&connector) {
                Ok((broker, reference)) => matches!(
                    broker.delete(&reference).await,
                    Ok(crate::credentials::CredentialDeleteOutcome::Deleted)
                ),
                Err(_) => false,
            };
        }
        let cleanup = match (vault_clean, outcome.revocation_clean) {
            (true, true) => ConnectorCleanupState::Clean,
            (false, true) => ConnectorCleanupState::VaultResidue,
            (true, false) => ConnectorCleanupState::RevocationResidue,
            (false, false) => ConnectorCleanupState::VaultAndRevocationResidue,
        };
        if outcome.revocation_clean {
            next.connectors
                .retain(|candidate| !(candidate.id == id && candidate.context == context));
        } else {
            // Keep the derived account/profile metadata as a fail-closed
            // tombstone so a later DELETE can retry provider revocation.
            connector.active = false;
            connector.state = ConnectorState::Disconnected;
            connector.live_digest = None;
            connector.cleanup = cleanup;
            next.replace(connector);
        }
        self.commit_registry(next)?;
        Ok(cleanup)
    }

    pub async fn disconnect(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
    ) -> ConnectorResult<ConnectorView> {
        let _operation = self.operation.lock().await;
        let mut next = self.registry_snapshot()?;
        let mut connector = next.get(context, id)?.clone();
        ensure_principal_access(&connector, principal_id)?;
        if connector.is_bearer() {
            self.runner
                .lock()
                .await
                .record_connector_config(context, id, "disconnect", &(self.clock)())
                .map_err(|_| ConnectorFailure::ActivationFailed)?;
            self.disable_runtime(id).await;
            connector.active = false;
            connector.state = ConnectorState::Disconnected;
            connector.live_digest = None;
            connector.cleanup = ConnectorCleanupState::Clean;
            next.replace(connector.clone());
            self.commit_registry(next)?;
            return Ok(connector.view());
        }
        if self.oauth.get(id).is_none() {
            let config = self
                .runtime_oauth_config(&connector)
                .map_err(|_| ConnectorFailure::OauthUnavailable)?;
            self.oauth
                .upsert(id, config, &self.brokers)
                .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        }
        self.runner
            .lock()
            .await
            .record_connector_config(context, id, "disconnect", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        self.disable_runtime(id).await;
        connector.active = false;
        connector.state = ConnectorState::Disconnected;
        connector.live_digest = None;
        connector.cleanup = ConnectorCleanupState::Clean;
        next.replace(connector.clone());
        self.commit_registry(next)?;
        let outcome = self
            .oauth
            .disconnect(id)
            .await
            .map_err(|_| ConnectorFailure::OauthUnavailable)?;
        let outbox_cleanup_clean = if connector.profile.is_some() {
            match self.compiled_outbox_storage(&connector) {
                Ok((broker, reference)) => matches!(
                    broker.delete(&reference).await,
                    Ok(crate::credentials::CredentialDeleteOutcome::Deleted)
                ),
                Err(_) => false,
            }
        } else {
            true
        };
        let vault_cleanup_clean = outcome.vault_cleanup_clean && outbox_cleanup_clean;
        connector.cleanup = match (vault_cleanup_clean, outcome.revocation_clean) {
            (true, true) => ConnectorCleanupState::Clean,
            (false, true) => ConnectorCleanupState::VaultResidue,
            (true, false) => ConnectorCleanupState::RevocationResidue,
            (false, false) => ConnectorCleanupState::VaultAndRevocationResidue,
        };
        let mut final_state = self.registry_snapshot()?;
        final_state.replace(connector.clone());
        self.commit_registry(final_state)?;
        Ok(connector.view())
    }

    pub async fn gmail_review(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
        approval_id: &str,
    ) -> ConnectorResult<ApprovalReview> {
        self.gmail_extension(context, principal_id, id)?
            .owner_review(approval_id)
            .await
            .map_err(|_| ConnectorFailure::OauthDenied)
    }

    pub async fn gmail_approve(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
        approval_id: &str,
    ) -> ConnectorResult<ApprovalView> {
        let _operation = self.operation.lock().await;
        let extension = self.gmail_extension(context, principal_id, id)?;
        let approval = extension
            .approval_status(approval_id)
            .await
            .map_err(|_| ConnectorFailure::OauthDenied)?;
        self.runner
            .lock()
            .await
            .record_connector_effect(
                context,
                id,
                &ConnectorEffectProof {
                    event: "gmail_approve",
                    approval_id,
                    payload_digest: &approval.payload_digest,
                    message_id: None,
                },
                &(self.clock)(),
            )
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        extension
            .owner_approve(approval_id, principal_id)
            .await
            .map_err(|_| ConnectorFailure::OauthDenied)
    }

    pub async fn gmail_deny(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
        approval_id: &str,
    ) -> ConnectorResult<ApprovalView> {
        let _operation = self.operation.lock().await;
        let extension = self.gmail_extension(context, principal_id, id)?;
        let approval = extension
            .approval_status(approval_id)
            .await
            .map_err(|_| ConnectorFailure::OauthDenied)?;
        self.runner
            .lock()
            .await
            .record_connector_effect(
                context,
                id,
                &ConnectorEffectProof {
                    event: "gmail_deny",
                    approval_id,
                    payload_digest: &approval.payload_digest,
                    message_id: None,
                },
                &(self.clock)(),
            )
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        extension
            .owner_deny(approval_id, principal_id)
            .await
            .map_err(|_| ConnectorFailure::OauthDenied)
    }

    pub async fn gmail_dispatch(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
        approval_id: &str,
    ) -> ConnectorResult<ApprovalView> {
        let _operation = self.operation.lock().await;
        let extension = self.gmail_extension(context, principal_id, id)?;
        let approval = extension
            .approval_status(approval_id)
            .await
            .map_err(|_| ConnectorFailure::OauthDenied)?;
        self.runner
            .lock()
            .await
            .record_connector_effect(
                context,
                id,
                &ConnectorEffectProof {
                    event: "gmail_dispatch",
                    approval_id,
                    payload_digest: &approval.payload_digest,
                    message_id: None,
                },
                &(self.clock)(),
            )
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let outcome = extension
            .owner_dispatch(approval_id)
            .await
            .map_err(|_| ConnectorFailure::UpstreamDenied)?;
        self.runner
            .lock()
            .await
            .record_connector_effect(
                context,
                id,
                &ConnectorEffectProof {
                    event: "gmail_dispatched",
                    approval_id,
                    payload_digest: &outcome.payload_digest,
                    message_id: outcome.message_id.as_deref(),
                },
                &(self.clock)(),
            )
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        Ok(outcome)
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
        if connector.is_bearer() {
            return self.restore_bearer(connector).await;
        }
        if let Some(profile) = &connector.profile {
            let current_pin = self
                .profiles
                .pin(profile)
                .map_err(|_| ConnectorFailure::NotApproved)?;
            if connector.profile_pin.as_deref() != Some(current_pin.as_str()) {
                return Err(ConnectorFailure::ManifestDrift);
            }
        }
        let (manifest, approved_digest) = self
            .runner
            .lock()
            .await
            .approved_connector(&connector.context, &connector.approved_manifest.id)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        if connector.approved_manifest.pin != approved_digest {
            return Err(ConnectorFailure::ManifestDrift);
        }
        let client = self
            .oauth
            .get(&connector.id)
            .ok_or(ConnectorFailure::OauthUnavailable)?;
        let execution = connector
            .profile
            .as_ref()
            .map(|profile| self.profiles.execution(profile))
            .transpose()
            .map_err(|_| ConnectorFailure::NotApproved)?;
        let (observed, runtime_upstream, gmail_runtime) = match execution {
            Some(ConnectorExecutionProfile::CompiledRest {
                adapter, settings, ..
            }) => {
                let storage = self.compiled_outbox_storage(connector).ok();
                build_compiled_runtime(connector, *adapter, settings, client, storage).await?
            }
            Some(ConnectorExecutionProfile::Mcp { .. }) | None => {
                let upstream = HttpUpstream::with_oauth_client(connector.endpoint.clone(), client);
                let observed = discover_server(&connector.id, &upstream)
                    .await
                    .map_err(|_| ConnectorFailure::OauthUnavailable)?;
                (observed, DynamicUpstream::new(upstream), None)
            }
        };
        let live_digest = proposed_manifest_catalog_digest(&observed)
            .map_err(|_| ConnectorFailure::ManifestDrift)?;
        if live_digest != approved_digest {
            return Err(ConnectorFailure::ManifestDrift);
        }
        self.dynamic_upstreams
            .write()
            .map_err(|_| ConnectorFailure::ActivationFailed)?
            .insert(connector.id.clone(), runtime_upstream);
        let runtime_manifest = instance_manifest(connector, &manifest);
        self.runner
            .lock()
            .await
            .install_hot_connector(&connector.context, &connector.id, &runtime_manifest)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        if let Some(gmail) = gmail_runtime {
            self.gmail_extensions
                .write()
                .map_err(|_| ConnectorFailure::ActivationFailed)?
                .insert(connector.id.clone(), gmail);
        }
        Ok(())
    }

    async fn activate_bearer(
        &self,
        context: &str,
        id: &str,
        mut next: RegistryFile,
        mut connector: PersistedConnector,
    ) -> ConnectorResult<ConnectorActivation> {
        let upstream = self.bearer_upstream(&connector)?;
        let observed = match discover_server(id, &upstream).await {
            Ok(observed) => observed,
            Err(GatewayError::CredentialUnavailable(_)) => {
                connector.state = ConnectorState::Unavailable;
                connector.active = false;
                connector.live_digest = None;
                next.replace(connector);
                self.disable_runtime(id).await;
                let _ = self.commit_registry(next);
                return Err(ConnectorFailure::SecretUnavailable);
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
        // First activation seals the live catalogue (owner TOFU). Later
        // activations and restores refuse drift against that pin.
        if !connector.approved_manifest.pin.is_empty()
            && connector.approved_manifest.pin != live_digest
        {
            connector.state = ConnectorState::Drifted;
            connector.active = false;
            connector.live_digest = Some(live_digest);
            next.replace(connector);
            self.disable_runtime(id).await;
            let _ = self.commit_registry(next);
            return Err(ConnectorFailure::ManifestDrift);
        }
        let approved = approve_discovered_bearer(&observed)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        let approved_digest = live_digest.clone();
        let mut runner = self.runner.lock().await;
        runner
            .validate_hot_connector(context, id, &approved)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        runner
            .record_connector_config(context, id, "activate", &(self.clock)())
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        connector.approved_manifest.pin = approved_digest.clone();
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
        let installed_tools = installed_upstream
            && runner
                .install_hot_connector(context, id, &approved)
                .is_ok();
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

    async fn restore_bearer(&self, connector: &PersistedConnector) -> ConnectorResult<()> {
        if connector.approved_manifest.pin.is_empty() || !connector.secret_stored {
            return Err(ConnectorFailure::NotApproved);
        }
        let upstream = self.bearer_upstream(connector)?;
        let observed = discover_server(&connector.id, &upstream)
            .await
            .map_err(|_| ConnectorFailure::SecretUnavailable)?;
        let live_digest = proposed_manifest_catalog_digest(&observed)
            .map_err(|_| ConnectorFailure::ManifestDrift)?;
        if live_digest != connector.approved_manifest.pin {
            return Err(ConnectorFailure::ManifestDrift);
        }
        let approved = approve_discovered_bearer(&observed)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        self.dynamic_upstreams
            .write()
            .map_err(|_| ConnectorFailure::ActivationFailed)?
            .insert(connector.id.clone(), DynamicUpstream::new(upstream));
        self.runner
            .lock()
            .await
            .install_hot_connector(&connector.context, &connector.id, &approved)
            .map_err(|_| ConnectorFailure::ActivationFailed)?;
        Ok(())
    }

    fn bearer_upstream(&self, connector: &PersistedConnector) -> ConnectorResult<HttpUpstream> {
        let reference = self.bearer_credential_ref(&connector.id)?;
        let broker = self
            .brokers
            .get(&reference.broker)
            .cloned()
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        Ok(HttpUpstream::with_credential(
            connector.endpoint.clone(),
            broker,
            reference,
        ))
    }

    fn bearer_credential_ref(&self, connector_id: &str) -> ConnectorResult<CredentialRef> {
        let broker = credential_broker_name(&self.brokers)?;
        Ok(derived_reference(
            &broker,
            connector_id,
            VAULT_BEARER_RECORD,
        ))
    }

    async fn delete_bearer_secret(&self, connector_id: &str) -> bool {
        let Ok(reference) = self.bearer_credential_ref(connector_id) else {
            return false;
        };
        let Some(broker) = self.brokers.get(&reference.broker) else {
            return false;
        };
        matches!(
            broker.delete(&reference).await,
            Ok(crate::credentials::CredentialDeleteOutcome::Deleted)
        )
    }

    fn validate_credential_stage(
        &self,
        principal_context: &str,
        path_id: &str,
        request: &ConnectorCredentialStageRequest,
    ) -> ConnectorResult<()> {
        if request.v != REGISTRY_VERSION
            || request.id != path_id
            || request.context != principal_context
            || !matches!(request.auth, ConnectorCredentialAuth::Bearer)
            || !valid_id(path_id)
            || !valid_id(&request.context)
            || !valid_public_endpoint(&request.endpoint)
        {
            return Err(ConnectorFailure::NotApproved);
        }
        Ok(())
    }

    async fn disable_runtime(&self, id: &str) {
        self.runner.lock().await.remove_hot_connector(id);
        if let Ok(mut upstreams) = self.dynamic_upstreams.write() {
            upstreams.remove(id);
        }
        if let Ok(mut gmail) = self.gmail_extensions.write() {
            gmail.remove(id);
        }
    }

    fn gmail_extension(
        &self,
        context: &str,
        principal_id: &str,
        id: &str,
    ) -> ConnectorResult<GmailSendGuardedUpstream> {
        let registry = self.registry_snapshot()?;
        let connector = registry.get(context, id)?;
        ensure_principal_access(connector, principal_id)?;
        if !connector.active {
            return Err(ConnectorFailure::OauthUnavailable);
        }
        self.gmail_extensions
            .read()
            .map_err(|_| ConnectorFailure::ActivationFailed)?
            .get(id)
            .cloned()
            .ok_or(ConnectorFailure::NotApproved)
    }

    fn compiled_outbox_storage(
        &self,
        connector: &PersistedConnector,
    ) -> ConnectorResult<(Arc<dyn CredentialBroker>, CredentialRef)> {
        let profile = connector
            .profile
            .as_ref()
            .ok_or(ConnectorFailure::NotApproved)?;
        let configured = self
            .profiles
            .enabled(profile)
            .map_err(|_| ConnectorFailure::NotApproved)?;
        let key = ConnectorInstanceKey::new(
            &connector.context,
            &connector.principal,
            &connector.id,
            &connector.account,
        )
        .map_err(|_| ConnectorFailure::NotApproved)?;
        let layout = OAuthVaultLayout::derive(&configured.oauth.credential_broker, &key);
        let broker = self
            .brokers
            .get(&layout.outbox.broker)
            .cloned()
            .ok_or(ConnectorFailure::SecretUnavailable)?;
        Ok((broker, layout.outbox))
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
        let oauth = connector
            .oauth
            .as_ref()
            .expect("OAuth connectors persist an oauth descriptor");
        let client_secret = template.oauth.client_secret.as_ref().map(|reference| {
            derived_reference(
                &reference.broker,
                &connector.id,
                &oauth.client_secret_record,
            )
        });
        let pending = derived_reference(
            &template.oauth.token_vault.broker,
            &connector.id,
            &oauth.pending_record,
        );
        let token = derived_reference(
            &template.oauth.token_vault.broker,
            &connector.id,
            &oauth.token_record,
        );
        UpstreamOAuthConfig {
            auth_url: oauth.authorization_endpoint.clone(),
            token_url: oauth.token_endpoint.clone(),
            client_id: oauth.client_id.clone(),
            client_secret,
            scopes: oauth.scopes.clone(),
            redirect_uri: oauth.redirect_uri.clone(),
            endpoints: template.oauth.endpoints.clone(),
            client_authentication: template.oauth.client_authentication,
            registration: template.oauth.registration.clone(),
            authorization_parameters: template.oauth.authorization_parameters.clone(),
            resource: template.oauth.resource.clone(),
            audience: template.oauth.audience.clone(),
            revocation_url: template.oauth.revocation_url.clone(),
            account_binding: template.oauth.account_binding.clone(),
            pending_vault: Some(pending),
            revocation_vault: None,
            token_vault: token,
        }
    }

    fn profile_oauth_config(&self, connector: &PersistedConnector) -> Result<UpstreamOAuthConfig> {
        let profile = connector.profile.as_ref().ok_or_else(|| {
            GatewayError::ConfigRejected("connector has no profile reference".into())
        })?;
        let current_pin = self.profiles.pin(profile)?;
        if connector.profile_pin.as_deref() != Some(current_pin.as_str()) {
            return Err(GatewayError::ConfigRejected(
                "connector profile content drifted under its pinned version".into(),
            ));
        }
        let key = ConnectorInstanceKey::new(
            &connector.context,
            &connector.principal,
            &connector.id,
            &connector.account,
        )?;
        let declared = self.profiles.enabled(profile)?;
        let layout = OAuthVaultLayout::derive(&declared.oauth.credential_broker, &key);
        self.profiles.materialize_oauth(profile, &layout)
    }

    fn runtime_oauth_config(
        &self,
        connector: &PersistedConnector,
    ) -> ConnectorResult<UpstreamOAuthConfig> {
        if connector.profile.is_some() {
            self.profile_oauth_config(connector)
                .map_err(|_| ConnectorFailure::NotApproved)
        } else {
            let template = self
                .templates
                .get(&connector.id)
                .ok_or(ConnectorFailure::NotApproved)?;
            Ok(self.oauth_config(connector, template))
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
    if config
        .client_secret
        .as_ref()
        .is_none_or(|secret| brokers.contains_key(&secret.broker))
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
    if connector.endpoint.len() > MAX_URL_BYTES || connector.endpoint.is_empty() {
        return false;
    }
    if connector.is_bearer() {
        return connector.oauth.is_none()
            && connector.profile.is_none()
            && valid_public_endpoint(&connector.endpoint)
            && connector.approved_manifest.pin.len() <= 128
            && (connector.approved_manifest.pin.is_empty()
                || connector.approved_manifest.pin.starts_with("sha256:"));
    }
    connector
        .oauth
        .as_ref()
        .is_some_and(valid_oauth_descriptor)
        && connector.approved_manifest.pin.len() <= 128
        && connector.approved_manifest.pin.starts_with("sha256:")
}

fn credential_broker_name(
    brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
) -> ConnectorResult<String> {
    if brokers.contains_key(DEFAULT_CREDENTIAL_BROKER) {
        return Ok(DEFAULT_CREDENTIAL_BROKER.to_owned());
    }
    if brokers.len() == 1 {
        return Ok(brokers.keys().next().expect("len checked").clone());
    }
    Err(ConnectorFailure::SecretUnavailable)
}

fn valid_public_endpoint(endpoint: &str) -> bool {
    if endpoint.is_empty() || endpoint.len() > MAX_URL_BYTES || endpoint.contains('\0') {
        return false;
    }
    if let Some(rest) = endpoint.strip_prefix("https://") {
        return rest.split(['/', '?']).next().is_some_and(|host| !host.is_empty());
    }
    endpoint
        .strip_prefix("http://")
        .and_then(|rest| rest.split(['/', ':', '?']).next())
        .is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        })
}

fn approve_discovered_bearer(observed: &ProposedManifest) -> Result<ApprovedManifest> {
    use crate::config::ToolAccess;
    use crate::hub::{approve_manifest, ToolApproval};

    let approvals = observed
        .tools
        .iter()
        .map(|tool| {
            (
                tool.name.clone(),
                // Owner pasted the bearer for this exact endpoint: grant the
                // live catalogue on first activate (TOFU). Risk class is write
                // so mandate checks stay explicit for powerful tools.
                ToolApproval::granted(ToolAccess::Write),
            )
        })
        .collect();
    approve_manifest(observed, &approvals)
}

fn ensure_principal_access(
    connector: &PersistedConnector,
    principal_id: &str,
) -> ConnectorResult<()> {
    if connector.profile.is_some() && connector.principal != principal_id {
        Err(ConnectorFailure::NotApproved)
    } else {
        Ok(())
    }
}

async fn build_compiled_runtime(
    connector: &PersistedConnector,
    adapter: CompiledConnectorAdapter,
    settings: &CompiledConnectorSettings,
    client: Arc<crate::upstream_oauth::UpstreamOAuthClient>,
    outbox_storage: Option<(Arc<dyn CredentialBroker>, CredentialRef)>,
) -> ConnectorResult<(
    ProposedManifest,
    DynamicUpstream,
    Option<GmailSendGuardedUpstream>,
)> {
    let observed = compiled_manifest(&connector.approved_manifest.id, adapter)
        .map_err(|_| ConnectorFailure::NotApproved)?;
    let upstream = match settings {
        CompiledConnectorSettings::GoogleSheetsRead {
            allowed_ranges,
            max_response_bytes,
        } => {
            let mut config = GoogleSheetsReadConfig::new(
                connector.endpoint.clone(),
                allowed_ranges
                    .iter()
                    .map(|(spreadsheet, ranges)| {
                        (spreadsheet.clone(), ranges.iter().cloned().collect())
                    })
                    .collect(),
            );
            config.max_response_bytes = *max_response_bytes;
            CompiledExtensionUpstream::google_sheets_read(config, client)
        }
        CompiledConnectorSettings::GmailSendGuarded {
            allowed_recipients,
            allowed_domains,
            max_recipients,
            max_subject_bytes,
            max_body_bytes,
            approval_ttl_seconds,
        } => {
            let mut policy = GmailSendPolicy::new(connector.endpoint.clone());
            policy.allowed_recipients = allowed_recipients.iter().cloned().collect();
            policy.allowed_domains = allowed_domains.iter().cloned().collect();
            policy.max_recipients = *max_recipients;
            policy.max_subject_bytes = *max_subject_bytes;
            policy.max_body_bytes = *max_body_bytes;
            policy.approval_ttl_seconds = *approval_ttl_seconds;
            let (broker, reference) = outbox_storage.ok_or(ConnectorFailure::SecretUnavailable)?;
            CompiledExtensionUpstream::gmail_send_guarded_durable(policy, client, broker, reference)
        }
        CompiledConnectorSettings::GoogleSheetsWriteGuarded {
            allowed_ranges,
            max_cells,
            max_request_bytes,
        } => {
            let mut config = GoogleSheetsWriteConfig::new(
                connector.endpoint.clone(),
                allowed_ranges
                    .iter()
                    .map(|(spreadsheet, ranges)| {
                        (spreadsheet.clone(), ranges.iter().cloned().collect())
                    })
                    .collect(),
            );
            config.max_cells = *max_cells;
            config.max_request_bytes = *max_request_bytes;
            CompiledExtensionUpstream::google_sheets_write_guarded(config, client)
        }
    }
    .map_err(|_| ConnectorFailure::NotApproved)?;
    upstream
        .hydrate()
        .await
        .map_err(|_| ConnectorFailure::ActivationFailed)?;
    let gmail = upstream.gmail().cloned();
    Ok((observed, DynamicUpstream::new(upstream), gmail))
}

fn instance_manifest(
    connector: &PersistedConnector,
    manifest: &ApprovedManifest,
) -> ApprovedManifest {
    if connector.profile.is_none() || connector.id == manifest.server {
        return manifest.clone();
    }
    let mut runtime = manifest.clone();
    runtime.server = connector.id.clone();
    for tool in &mut runtime.tools {
        tool.exposed_name = hub_exposed_name(&connector.id, &tool.name);
    }
    runtime
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
                auth: ConnectorAuthKind::Oauth,
                oauth: Some(ConnectorOAuthDescriptor {
                    authorization_endpoint: "https://as.example.test/authorize".into(),
                    token_endpoint: "https://as.example.test/token".into(),
                    client_id: "aithos-enterprise".into(),
                    scopes: vec!["calendar.read".into()],
                    redirect_uri: "https://acme.mcp.aithos.fr/oauth/callback".into(),
                    client_secret_record: "calendar-client".into(),
                    pending_record: "calendar-pending".into(),
                    token_record: "calendar-token".into(),
                }),
                approved_manifest: ApprovedManifestRef {
                    id: id.to_owned(),
                    pin: "sha256:approved".into(),
                },
                profile: None,
                profile_pin: None,
                principal: legacy_principal(),
                account: legacy_account(),
                state: ConnectorState::Draft,
                active: false,
                cleanup: ConnectorCleanupState::Clean,
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
        assert!(ConnectorControl::parse_bearer_secret(br#"{"bearer_token":"ok"}"#).is_ok());
        assert!(ConnectorControl::parse_bearer_secret(
            br#"{"bearer_token":"ok","path":"vault/root"}"#
        )
        .is_err());
        assert!(ConnectorControl::parse_credential_stage(
            br#"{"v":1,"id":"notes","context":"travail","endpoint":"https://mcp.example/mcp","transport":"streamable-http","auth":"bearer"}"#
        )
        .is_ok());
        assert!(ConnectorControl::parse_credential_stage(
            br#"{"v":1,"id":"notes","context":"travail","endpoint":"http://evil.example/mcp","transport":"streamable-http","auth":"bearer"}"#
        )
        .is_ok());
    }

    #[test]
    fn bearer_endpoints_accept_https_and_loopback_only() {
        assert!(valid_public_endpoint("https://mcp.example/mcp"));
        assert!(valid_public_endpoint("http://127.0.0.1:9/mcp"));
        assert!(valid_public_endpoint("http://localhost/mcp"));
        assert!(!valid_public_endpoint("http://mcp.evil.example/mcp"));
        assert!(!valid_public_endpoint("ftp://127.0.0.1/mcp"));
        assert!(!valid_public_endpoint(""));
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
