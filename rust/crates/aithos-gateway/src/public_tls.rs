//! G1b public TLS custody and delegated ACME DNS-01.
//!
//! Public ClientHello bytes arrive inside one yamux stream. The relay sees
//! only TLS records: certificate keys, ACME account credentials and HTTP
//! plaintext stay in this process and in a private local cache.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::future::BoxFuture;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, ClientConfig, ClientConnection, ServerConfig, ServerConnection};
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};
use tokio_rustls::TlsAcceptor;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};
use zeroize::Zeroizing;

use crate::core_bridge::gateway_acme_authorization_header;
use crate::keyholder::Keyholder;
use crate::{GatewayError, Result};

const PUBLIC_TLS_ALPN: &[u8] = b"http/1.1";
const PUBLIC_TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const ACME_CALL_TIMEOUT: Duration = Duration::from_secs(15);
const ACME_DNS_WAIT: Duration = Duration::from_secs(30);
const ACME_POLL_WAIT: Duration = Duration::from_secs(2);
const ACME_READY_POLLS: usize = 60;
const ACME_ISSUANCE_POLLS: usize = 60;
const ACME_ORDER_DEADLINE: Duration = Duration::from_secs(8 * 60);
const RENEW_BEFORE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MAX_CERT_BYTES: u64 = 256 * 1024;
const MAX_KEY_BYTES: u64 = 64 * 1024;
const MAX_ACCOUNT_BYTES: u64 = 128 * 1024;
const MAX_POINTER_BYTES: u64 = 4 * 1024;
const CACHE_VERSION: u8 = 1;
const STORE_WIRE_VERSION: &str = "1.0.0-draft.1";

fn tls_error(code: &'static str) -> GatewayError {
    GatewayError::RelayUnavailable(code.into())
}

/// One hot-swappable public TLS endpoint. Clones share the same current
/// `ServerConfig`; an in-flight connection keeps the generation it began
/// with while new streams immediately observe a successful renewal.
#[derive(Clone)]
pub struct PublicTlsAcceptor {
    current: watch::Receiver<Arc<ServerConfig>>,
}

/// The write half of [`PublicTlsAcceptor`], kept by the certificate manager.
pub struct PublicTlsActivator {
    current: watch::Sender<Arc<ServerConfig>>,
}

pub fn public_tls_slot(initial: Arc<ServerConfig>) -> (PublicTlsActivator, PublicTlsAcceptor) {
    let (current, receiver) = watch::channel(initial);
    (
        PublicTlsActivator { current },
        PublicTlsAcceptor { current: receiver },
    )
}

impl PublicTlsActivator {
    pub fn replace(&self, next: Arc<ServerConfig>) {
        self.current.send_replace(next);
    }

    pub fn current(&self) -> Arc<ServerConfig> {
        self.current.borrow().clone()
    }
}

impl PublicTlsAcceptor {
    /// Terminate public TLS inside the gateway. No plaintext is exposed
    /// until the bounded handshake has completed successfully.
    pub async fn accept(
        &self,
        stream: yamux::Stream,
    ) -> Result<tokio_rustls::server::TlsStream<Compat<yamux::Stream>>> {
        let acceptor = TlsAcceptor::from(self.current.borrow().clone());
        tokio::time::timeout(
            PUBLIC_TLS_HANDSHAKE_TIMEOUT,
            acceptor.accept(stream.compat()),
        )
        .await
        .map_err(|_| tls_error("public_tls_handshake_timeout"))?
        .map_err(|_| tls_error("public_tls_handshake_failed"))
    }
}

struct LoadedMaterial {
    config: Arc<ServerConfig>,
    chain: Vec<CertificateDer<'static>>,
}

fn load_material(chain_pem: &[u8], key_pem: &[u8]) -> Result<LoadedMaterial> {
    let chain = rustls_pemfile::certs(&mut Cursor::new(chain_pem))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|_| tls_error("public_tls_certificate_invalid"))?;
    if chain.is_empty() {
        return Err(tls_error("public_tls_certificate_invalid"));
    }
    let key = rustls_pemfile::private_key(&mut Cursor::new(key_pem))
        .map_err(|_| tls_error("public_tls_private_key_invalid"))?
        .ok_or_else(|| tls_error("public_tls_private_key_invalid"))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| tls_error("public_tls_configuration_invalid"))?
        .with_no_client_auth()
        .with_single_cert(chain.clone(), key)
        .map_err(|_| tls_error("public_tls_key_mismatch"))?;
    config.alpn_protocols = vec![PUBLIC_TLS_ALPN.to_vec()];
    Ok(LoadedMaterial {
        config: Arc::new(config),
        chain,
    })
}

#[derive(Debug)]
struct FixedTime(UnixTime);

impl rustls::time_provider::TimeProvider for FixedTime {
    fn current_time(&self) -> Option<UnixTime> {
        Some(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CertificateValidity {
    Valid,
    Expired,
}

fn validate_material_at(
    material: &LoadedMaterial,
    hostname: &str,
    at: UnixTime,
) -> Result<CertificateValidity> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(
            material
                .chain
                .last()
                .expect("load_material rejects an empty chain")
                .clone(),
        )
        .map_err(|_| tls_error("public_tls_chain_invalid"))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let client_config = ClientConfig::builder_with_details(provider, Arc::new(FixedTime(at)))
        .with_safe_default_protocol_versions()
        .map_err(|_| tls_error("public_tls_configuration_invalid"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = ServerName::try_from(hostname.to_owned())
        .map_err(|_| tls_error("public_tls_hostname_invalid"))?;
    let mut client = ClientConnection::new(Arc::new(client_config), server_name)
        .map_err(|_| tls_error("public_tls_validation_failed"))?;
    let mut server = ServerConnection::new(Arc::clone(&material.config))
        .map_err(|_| tls_error("public_tls_validation_failed"))?;

    for _ in 0..64 {
        if client.wants_write() {
            let mut bytes = Vec::new();
            client
                .write_tls(&mut bytes)
                .map_err(|_| tls_error("public_tls_validation_failed"))?;
            server
                .read_tls(&mut Cursor::new(bytes))
                .map_err(|_| tls_error("public_tls_validation_failed"))?;
            server
                .process_new_packets()
                .map_err(|_| tls_error("public_tls_validation_failed"))?;
        }
        if server.wants_write() {
            let mut bytes = Vec::new();
            server
                .write_tls(&mut bytes)
                .map_err(|_| tls_error("public_tls_validation_failed"))?;
            client
                .read_tls(&mut Cursor::new(bytes))
                .map_err(|_| tls_error("public_tls_validation_failed"))?;
            match client.process_new_packets() {
                Ok(_) => {}
                Err(rustls::Error::InvalidCertificate(
                    CertificateError::Expired | CertificateError::ExpiredContext { .. },
                )) => return Ok(CertificateValidity::Expired),
                Err(_) => return Err(tls_error("public_tls_certificate_invalid")),
            }
        }
        if !client.is_handshaking() && !server.is_handshaking() {
            return Ok(CertificateValidity::Valid);
        }
    }
    Err(tls_error("public_tls_validation_failed"))
}

fn renewal_horizon(now: UnixTime) -> Result<UnixTime> {
    let seconds = now
        .as_secs()
        .checked_add(RENEW_BEFORE.as_secs())
        .ok_or_else(|| tls_error("public_tls_clock_invalid"))?;
    Ok(UnixTime::since_unix_epoch(Duration::from_secs(seconds)))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivePointer {
    version: u8,
    hostname: String,
    generation: String,
}

#[derive(Clone)]
pub struct SecureTlsCache {
    root: PathBuf,
}

enum CacheState {
    Missing,
    Expired,
    Current {
        material: LoadedMaterial,
        renew: bool,
    },
}

impl SecureTlsCache {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let cache = Self { root: path.into() };
        ensure_private_directory(&cache.root)?;
        Ok(cache)
    }

    /// Import one externally obtained PEM pair through the same validation
    /// and atomic generation swap used by ACME. This is also the bounded
    /// enterprise migration seam; raw key bytes are never retained here.
    pub fn install(
        &self,
        hostname: &str,
        chain_pem: &[u8],
        key_pem: &[u8],
        now: UnixTime,
    ) -> Result<Arc<ServerConfig>> {
        self.activate(hostname, chain_pem, key_pem, now)
            .map(|material| material.config)
    }

    fn state(&self, hostname: &str, now: UnixTime) -> Result<CacheState> {
        validate_private_directory(&self.root)?;
        let pointer_path = self.root.join("active.json");
        let Some(pointer_bytes) = read_private_optional(&pointer_path, MAX_POINTER_BYTES)? else {
            return Ok(CacheState::Missing);
        };
        let pointer: ActivePointer = serde_json::from_slice(&pointer_bytes)
            .map_err(|_| tls_error("public_tls_cache_pointer_invalid"))?;
        if pointer.version != CACHE_VERSION
            || pointer.hostname != hostname
            || !valid_generation(&pointer.generation)
        {
            return Err(tls_error("public_tls_cache_pointer_invalid"));
        }
        let generation = self.root.join(&pointer.generation);
        validate_private_directory(&generation)?;
        let chain = read_private_required(&generation.join("cert.pem"), MAX_CERT_BYTES)?;
        let key = Zeroizing::new(read_private_required(
            &generation.join("key.pem"),
            MAX_KEY_BYTES,
        )?);
        let material = load_material(&chain, &key)?;
        match validate_material_at(&material, hostname, now)? {
            CertificateValidity::Expired => Ok(CacheState::Expired),
            CertificateValidity::Valid => {
                let renew = matches!(
                    validate_material_at(&material, hostname, renewal_horizon(now)?)?,
                    CertificateValidity::Expired
                );
                Ok(CacheState::Current { material, renew })
            }
        }
    }

    fn load_account(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        read_private_optional(&self.root.join("account.json"), MAX_ACCOUNT_BYTES)
            .map(|value| value.map(Zeroizing::new))
    }

    fn store_account(&self, account: &[u8]) -> Result<()> {
        if account.len() as u64 > MAX_ACCOUNT_BYTES {
            return Err(tls_error("acme_account_cache_invalid"));
        }
        write_private_atomic(&self.root, "account.json", account)
    }

    fn activate(
        &self,
        hostname: &str,
        chain_pem: &[u8],
        key_pem: &[u8],
        now: UnixTime,
    ) -> Result<LoadedMaterial> {
        if chain_pem.len() as u64 > MAX_CERT_BYTES || key_pem.len() as u64 > MAX_KEY_BYTES {
            return Err(tls_error("public_tls_material_too_large"));
        }
        let generation_name = create_generation(&self.root)?;
        let generation = self.root.join(&generation_name);
        write_private_new(&generation.join("cert.pem"), chain_pem)?;
        write_private_new(&generation.join("key.pem"), key_pem)?;
        sync_directory(&generation)?;

        let candidate = load_material(chain_pem, key_pem)?;
        if validate_material_at(&candidate, hostname, now)? != CertificateValidity::Valid {
            return Err(tls_error("public_tls_certificate_not_current"));
        }
        let pointer = ActivePointer {
            version: CACHE_VERSION,
            hostname: hostname.to_owned(),
            generation: generation_name,
        };
        let pointer = serde_json::to_vec(&pointer)
            .map_err(|_| tls_error("public_tls_cache_pointer_invalid"))?;
        write_private_atomic(&self.root, "active.json", &pointer)?;
        sync_directory(&self.root)?;
        Ok(candidate)
    }
}

fn valid_generation(value: &str) -> bool {
    value.starts_with("gen-")
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_generation(root: &Path) -> Result<String> {
    for _ in 0..32 {
        let counter = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| tls_error("public_tls_clock_invalid"))?
            .as_nanos();
        let name = format!("gen-{stamp:x}-{}-{counter:x}", std::process::id());
        let path = root.join(&name);
        match create_private_directory(&path) {
            Ok(()) => return Ok(name),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(tls_error("public_tls_cache_write_failed")),
        }
    }
    Err(tls_error("public_tls_cache_write_failed"))
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(path)
}

#[cfg(not(unix))]
fn create_private_directory(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "private filesystem modes unavailable",
    ))
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => validate_private_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|_| tls_error("public_tls_cache_create_failed"))?;
            set_private_directory_mode(path)?;
            validate_private_directory(path)
        }
        Err(_) => Err(tls_error("public_tls_cache_unavailable")),
    }
}

fn validate_private_directory(path: &Path) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| tls_error("public_tls_cache_unavailable"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(tls_error("public_tls_cache_invalid"));
    }
    validate_private_mode(&metadata)
}

#[cfg(unix)]
fn set_private_directory_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| tls_error("public_tls_cache_create_failed"))
}

#[cfg(not(unix))]
fn set_private_directory_mode(_path: &Path) -> Result<()> {
    Err(tls_error("public_tls_private_mode_unsupported"))
}

#[cfg(unix)]
fn validate_private_mode(metadata: &fs::Metadata) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(tls_error("public_tls_private_mode_required"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_private_mode(_metadata: &fs::Metadata) -> Result<()> {
    Err(tls_error("public_tls_private_mode_unsupported"))
}

fn read_private_optional(path: &Path, max: u64) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(tls_error("public_tls_cache_read_failed")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max {
        return Err(tls_error("public_tls_cache_invalid"));
    }
    validate_private_mode(&metadata)?;
    let file = File::open(path).map_err(|_| tls_error("public_tls_cache_read_failed"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| tls_error("public_tls_cache_read_failed"))?;
    if bytes.len() as u64 > max {
        return Err(tls_error("public_tls_cache_invalid"));
    }
    Ok(Some(bytes))
}

fn read_private_required(path: &Path, max: u64) -> Result<Vec<u8>> {
    read_private_optional(path, max)?.ok_or_else(|| tls_error("public_tls_cache_incomplete"))
}

#[cfg(unix)]
fn private_options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    options
}

#[cfg(not(unix))]
fn private_options() -> OpenOptions {
    OpenOptions::new()
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = private_options()
        .open(path)
        .map_err(|_| tls_error("public_tls_cache_write_failed"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| tls_error("public_tls_cache_write_failed"))
}

fn write_private_atomic(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let counter = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| tls_error("public_tls_clock_invalid"))?
        .as_nanos();
    let temporary = root.join(format!(".tmp-{stamp:x}-{}-{counter:x}", std::process::id()));
    write_private_new(&temporary, bytes)?;
    fs::rename(&temporary, root.join(name))
        .map_err(|_| tls_error("public_tls_cache_write_failed"))?;
    sync_directory(root)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| tls_error("public_tls_cache_sync_failed"))
}

/// Load an explicit enterprise PEM pair after checking private modes,
/// key correspondence, SAN and current validity.
pub fn load_private_pem(
    cert_file: &Path,
    key_file: &Path,
    hostname: &str,
    now: UnixTime,
) -> Result<Arc<ServerConfig>> {
    let chain = read_private_required(cert_file, MAX_CERT_BYTES)?;
    let key = Zeroizing::new(read_private_required(key_file, MAX_KEY_BYTES)?);
    let material = load_material(&chain, &key)?;
    if validate_material_at(&material, hostname, now)? != CertificateValidity::Valid {
        return Err(tls_error("public_tls_certificate_not_current"));
    }
    Ok(material.config)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateSource {
    Cache,
    Renewed,
    RetainedAfterRenewalFailure,
}

pub struct CertificateLease {
    pub config: Arc<ServerConfig>,
    pub source: CertificateSource,
}

/// Issuance result. Its custom Debug deliberately omits all PEM and ACME
/// account bytes.
pub struct IssuedCertificate {
    pub chain_pem: Vec<u8>,
    pub private_key_pem: Zeroizing<Vec<u8>>,
    pub account_record: Option<Zeroizing<Vec<u8>>>,
}

impl fmt::Debug for IssuedCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedCertificate")
            .field("chain_bytes", &self.chain_pem.len())
            .field("private_key", &"[REDACTED]")
            .field(
                "account_record",
                &self.account_record.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

pub trait CertificateIssuer: Send + Sync {
    fn issue<'a>(
        &'a self,
        hostname: &'a str,
        account_record: Option<Zeroizing<Vec<u8>>>,
    ) -> BoxFuture<'a, Result<IssuedCertificate>>;
}

/// Single-flight cache/renewal manager. A failed renewal keeps a certificate
/// that is still valid now; missing/expired material remains fail-closed.
pub struct AcmeCertificateManager<I> {
    cache: SecureTlsCache,
    issuer: I,
    lock: Mutex<()>,
}

impl<I> AcmeCertificateManager<I>
where
    I: CertificateIssuer,
{
    pub fn new(cache: SecureTlsCache, issuer: I) -> Self {
        Self {
            cache,
            issuer,
            lock: Mutex::new(()),
        }
    }

    pub async fn ensure(&self, hostname: &str, now: UnixTime) -> Result<CertificateLease> {
        let _guard = self.lock.lock().await;
        let current = self.cache.state(hostname, now)?;
        if let CacheState::Current {
            material,
            renew: false,
        } = current
        {
            return Ok(CertificateLease {
                config: material.config,
                source: CertificateSource::Cache,
            });
        }

        let retained = match current {
            CacheState::Current { material, .. } => Some(material.config),
            CacheState::Missing | CacheState::Expired => None,
        };
        let account = self.cache.load_account()?;
        let issued = match self.issuer.issue(hostname, account).await {
            Ok(issued) => issued,
            Err(error) => {
                return retained.map_or(Err(error), |config| {
                    Ok(CertificateLease {
                        config,
                        source: CertificateSource::RetainedAfterRenewalFailure,
                    })
                });
            }
        };
        if let Some(account) = &issued.account_record {
            if let Err(error) = self.cache.store_account(account) {
                return retained.map_or(Err(error), |config| {
                    Ok(CertificateLease {
                        config,
                        source: CertificateSource::RetainedAfterRenewalFailure,
                    })
                });
            }
        }
        match self
            .cache
            .activate(hostname, &issued.chain_pem, &issued.private_key_pem, now)
        {
            Ok(material) => Ok(CertificateLease {
                config: material.config,
                source: CertificateSource::Renewed,
            }),
            Err(error) => retained.map_or(Err(error), |config| {
                Ok(CertificateLease {
                    config,
                    source: CertificateSource::RetainedAfterRenewalFailure,
                })
            }),
        }
    }
}

#[derive(Clone)]
pub struct AcmeTxtClient {
    base: String,
    authority: String,
    identity: Arc<Keyholder>,
    clock: Arc<dyn Fn() -> String + Send + Sync>,
    nonce: Arc<dyn Fn() -> String + Send + Sync>,
    http: reqwest::Client,
}

struct SignedAcmeRequest {
    url: String,
    body: Vec<u8>,
    authorization: String,
}

impl AcmeTxtClient {
    pub fn new(
        store_url: &str,
        identity: Arc<Keyholder>,
        clock: Arc<dyn Fn() -> String + Send + Sync>,
        nonce: Arc<dyn Fn() -> String + Send + Sync>,
    ) -> Result<Self> {
        let parsed =
            reqwest::Url::parse(store_url).map_err(|_| tls_error("acme_store_url_invalid"))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(tls_error("acme_store_url_invalid"));
        }
        let host = parsed
            .host_str()
            .expect("validated URL host")
            .to_ascii_lowercase();
        if parsed.scheme() == "http"
            && host != "localhost"
            && !host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
        {
            return Err(tls_error("acme_store_url_invalid"));
        }
        let authority_host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host
        };
        let authority = match (parsed.scheme(), parsed.port()) {
            ("https", Some(443)) | ("http", Some(80)) | (_, None) => authority_host,
            (_, Some(port)) => format!("{authority_host}:{port}"),
        };
        let base = store_url.trim_end_matches('/').to_owned();
        let http = reqwest::Client::builder()
            .timeout(ACME_CALL_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| tls_error("acme_store_client_failed"))?;
        Ok(Self {
            base,
            authority,
            identity,
            clock,
            nonce,
            http,
        })
    }

    fn signed_request(
        &self,
        method: &str,
        hostname: &str,
        value: &str,
    ) -> Result<SignedAcmeRequest> {
        if !valid_dns_name(hostname)
            || value.is_empty()
            || value.len() > 255
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(tls_error("acme_challenge_invalid"));
        }
        let body = serde_jcs::to_vec(&serde_json::json!({
            "hostname": hostname,
            "value": value,
        }))
        .map_err(|_| tls_error("acme_challenge_encode_failed"))?;
        let authorization = gateway_acme_authorization_header(
            &self.identity,
            &self.authority,
            method,
            &body,
            &(self.clock)(),
            &(self.nonce)(),
        )?;
        Ok(SignedAcmeRequest {
            url: format!("{}/acme/txt", self.base),
            body,
            authorization,
        })
    }

    async fn effect(&self, method: reqwest::Method, hostname: &str, value: &str) -> Result<()> {
        let request = self.signed_request(method.as_str(), hostname, value)?;
        let response = self
            .http
            .request(method, request.url)
            .header("x-aithos-auth", request.authorization)
            .header("x-aithos-store", STORE_WIRE_VERSION)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request.body)
            .send()
            .await
            .map_err(|_| tls_error("acme_store_unavailable"))?;
        if response.status() != reqwest::StatusCode::NO_CONTENT
            || response
                .headers()
                .get("x-aithos-store")
                .and_then(|value| value.to_str().ok())
                != Some(STORE_WIRE_VERSION)
        {
            return Err(tls_error("acme_store_refused"));
        }
        Ok(())
    }

    pub async fn present_txt(&self, hostname: &str, value: &str) -> Result<()> {
        self.effect(reqwest::Method::PUT, hostname, value).await
    }

    pub async fn retire_txt(&self, hostname: &str, value: &str) -> Result<()> {
        self.effect(reqwest::Method::DELETE, hostname, value).await
    }
}

fn valid_dns_name(name: &str) -> bool {
    name.len() <= 253
        && name.contains('.')
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountRecord {
    version: u8,
    directory: String,
    credentials: instant_acme::AccountCredentials,
}

pub struct InstantAcmeIssuer {
    directory: String,
    dns: AcmeTxtClient,
    cache: SecureTlsCache,
    dns_wait: Duration,
    poll_wait: Duration,
}

impl InstantAcmeIssuer {
    pub fn new(directory: impl Into<String>, dns: AcmeTxtClient, cache: SecureTlsCache) -> Self {
        Self {
            directory: directory.into(),
            dns,
            cache,
            dns_wait: ACME_DNS_WAIT,
            poll_wait: ACME_POLL_WAIT,
        }
    }

    async fn bounded<T, E>(
        future: impl Future<Output = std::result::Result<T, E>>,
        code: &'static str,
    ) -> Result<T> {
        tokio::time::timeout(ACME_CALL_TIMEOUT, future)
            .await
            .map_err(|_| tls_error("acme_call_timeout"))?
            .map_err(|_| tls_error(code))
    }

    async fn account(
        &self,
        cached: Option<Zeroizing<Vec<u8>>>,
    ) -> Result<(instant_acme::Account, Option<Zeroizing<Vec<u8>>>)> {
        if let Some(cached) = cached {
            let record: AccountRecord = serde_json::from_slice(&cached)
                .map_err(|_| tls_error("acme_account_cache_invalid"))?;
            if record.version != CACHE_VERSION || record.directory != self.directory {
                return Err(tls_error("acme_account_cache_invalid"));
            }
            let account = Self::bounded(
                instant_acme::Account::from_credentials(record.credentials),
                "acme_account_restore_failed",
            )
            .await?;
            return Ok((account, None));
        }

        let (account, credentials) = Self::bounded(
            instant_acme::Account::create(
                &instant_acme::NewAccount {
                    contact: &[],
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                &self.directory,
                None,
            ),
            "acme_account_create_failed",
        )
        .await?;
        let record = AccountRecord {
            version: CACHE_VERSION,
            directory: self.directory.clone(),
            credentials,
        };
        let bytes = Zeroizing::new(
            serde_json::to_vec(&record).map_err(|_| tls_error("acme_account_cache_invalid"))?,
        );
        self.cache.store_account(&bytes)?;
        Ok((account, None))
    }

    async fn issue_inner(
        &self,
        hostname: &str,
        account_record: Option<Zeroizing<Vec<u8>>>,
    ) -> Result<IssuedCertificate> {
        if !valid_dns_name(hostname) {
            return Err(tls_error("acme_hostname_invalid"));
        }
        let (account, fresh_account) = self.account(account_record).await?;
        let identifiers = [instant_acme::Identifier::Dns(hostname.to_owned())];
        let mut order = Self::bounded(
            account.new_order(&instant_acme::NewOrder {
                identifiers: &identifiers,
            }),
            "acme_order_create_failed",
        )
        .await?;
        let authorizations =
            Self::bounded(order.authorizations(), "acme_authorization_failed").await?;
        if authorizations.len() != 1
            || authorizations[0].identifier != instant_acme::Identifier::Dns(hostname.to_owned())
        {
            return Err(tls_error("acme_authorization_invalid"));
        }

        let mut posed = Vec::new();
        let result = tokio::time::timeout(ACME_ORDER_DEADLINE, async {
            for authorization in &authorizations {
                match authorization.status {
                    instant_acme::AuthorizationStatus::Valid => continue,
                    instant_acme::AuthorizationStatus::Pending => {}
                    _ => return Err(tls_error("acme_authorization_invalid")),
                }
                let challenge = authorization
                    .challenges
                    .iter()
                    .find(|challenge| challenge.r#type == instant_acme::ChallengeType::Dns01)
                    .ok_or_else(|| tls_error("acme_dns01_unavailable"))?;
                let value = order.key_authorization(challenge).dns_value();
                self.dns.present_txt(hostname, &value).await?;
                posed.push(value);
                tokio::time::sleep(self.dns_wait).await;
                Self::bounded(
                    order.set_challenge_ready(&challenge.url),
                    "acme_challenge_ready_failed",
                )
                .await?;
            }

            let mut ready = false;
            for _ in 0..ACME_READY_POLLS {
                let state = Self::bounded(order.refresh(), "acme_order_refresh_failed").await?;
                match state.status {
                    instant_acme::OrderStatus::Ready => {
                        ready = true;
                        break;
                    }
                    instant_acme::OrderStatus::Pending => {
                        tokio::time::sleep(self.poll_wait).await;
                    }
                    _ => return Err(tls_error("acme_order_invalid")),
                }
            }
            if !ready {
                return Err(tls_error("acme_order_ready_timeout"));
            }

            let mut parameters = rcgen::CertificateParams::new(vec![hostname.to_owned()])
                .map_err(|_| tls_error("acme_csr_failed"))?;
            parameters.distinguished_name = rcgen::DistinguishedName::new();
            let key_pair =
                rcgen::KeyPair::generate().map_err(|_| tls_error("acme_keygen_failed"))?;
            let request = parameters
                .serialize_request(&key_pair)
                .map_err(|_| tls_error("acme_csr_failed"))?;
            Self::bounded(order.finalize(request.der()), "acme_finalize_failed").await?;

            let mut chain = None;
            for _ in 0..ACME_ISSUANCE_POLLS {
                let state = Self::bounded(order.refresh(), "acme_order_refresh_failed").await?;
                match state.status {
                    instant_acme::OrderStatus::Valid => {
                        chain =
                            Self::bounded(order.certificate(), "acme_certificate_failed").await?;
                        if chain.is_some() {
                            break;
                        }
                    }
                    instant_acme::OrderStatus::Processing => {}
                    _ => return Err(tls_error("acme_order_invalid")),
                }
                tokio::time::sleep(self.poll_wait).await;
            }
            let chain = chain.ok_or_else(|| tls_error("acme_certificate_timeout"))?;
            Ok(IssuedCertificate {
                chain_pem: chain.into_bytes(),
                private_key_pem: Zeroizing::new(key_pair.serialize_pem().into_bytes()),
                account_record: fresh_account,
            })
        })
        .await
        .map_err(|_| tls_error("acme_order_deadline_exceeded"))
        .and_then(|result| result);

        for value in &posed {
            let _ =
                tokio::time::timeout(ACME_CALL_TIMEOUT, self.dns.retire_txt(hostname, value)).await;
        }
        result
    }
}

impl CertificateIssuer for InstantAcmeIssuer {
    fn issue<'a>(
        &'a self,
        hostname: &'a str,
        account_record: Option<Zeroizing<Vec<u8>>>,
    ) -> BoxFuture<'a, Result<IssuedCertificate>> {
        Box::pin(self.issue_inner(hostname, account_record))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    const HOSTNAME: &str = "demo.mcp.aithos.fr";
    const NOW: u64 = 1_774_224_000; // 2026-03-22, inside every test certificate below.

    fn identity() -> Arc<Keyholder> {
        Arc::new(Keyholder::from_entropy([0x42; 32], [0x51; 32]))
    }

    fn certificate(
        not_before: (i32, u8, u8),
        not_after: (i32, u8, u8),
    ) -> (Vec<u8>, Zeroizing<Vec<u8>>) {
        let mut parameters = rcgen::CertificateParams::new(vec![HOSTNAME.to_owned()]).unwrap();
        parameters.distinguished_name = rcgen::DistinguishedName::new();
        parameters.not_before = rcgen::date_time_ymd(not_before.0, not_before.1, not_before.2);
        parameters.not_after = rcgen::date_time_ymd(not_after.0, not_after.1, not_after.2);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = parameters.self_signed(&key).unwrap();
        (
            cert.pem().into_bytes(),
            Zeroizing::new(key.serialize_pem().into_bytes()),
        )
    }

    fn now() -> UnixTime {
        UnixTime::since_unix_epoch(Duration::from_secs(NOW))
    }

    #[test]
    fn b5_authorization_is_byte_exact_to_p6_put_and_delete() {
        let vectors: serde_json::Value =
            serde_json::from_str(include_str!("../../../../vectors/p6-acme-txt.json")).unwrap();
        for name in ["accept_put_ok", "accept_delete_ok"] {
            let case = vectors["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["name"] == name)
                .unwrap();
            let at = case["envelope"]["at"].as_str().unwrap().to_owned();
            let nonce = case["envelope"]["nonce"].as_str().unwrap().to_owned();
            let client = AcmeTxtClient::new(
                "https://store.aithos.fr",
                identity(),
                Arc::new(move || at.clone()),
                Arc::new(move || nonce.clone()),
            )
            .unwrap();
            let body: serde_json::Value =
                serde_json::from_str(case["request_body_utf8"].as_str().unwrap()).unwrap();
            let request = client
                .signed_request(
                    case["method"].as_str().unwrap(),
                    body["hostname"].as_str().unwrap(),
                    body["value"].as_str().unwrap(),
                )
                .unwrap();
            assert_eq!(
                request.body,
                case["request_body_utf8"].as_str().unwrap().as_bytes()
            );
            assert_eq!(
                request.authorization,
                case["x_aithos_auth"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn cache_rejects_permissive_modes_and_never_follows_a_symlink() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let temporary = tempfile::tempdir().unwrap();
        let permissive = temporary.path().join("permissive");
        fs::create_dir(&permissive).unwrap();
        fs::set_permissions(&permissive, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            SecureTlsCache::open(&permissive),
            Err(GatewayError::RelayUnavailable(reason))
                if reason == "public_tls_private_mode_required"
        ));

        let target = temporary.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        let linked = temporary.path().join("linked");
        symlink(&target, &linked).unwrap();
        assert!(matches!(
            SecureTlsCache::open(&linked),
            Err(GatewayError::RelayUnavailable(reason))
                if reason == "public_tls_cache_invalid"
        ));
    }

    #[test]
    fn complete_generation_is_validated_before_atomic_activation() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = SecureTlsCache::open(temporary.path().join("tls")).unwrap();
        let (old_cert, old_key) = certificate((2026, 1, 1), (2027, 1, 1));
        cache
            .activate(HOSTNAME, &old_cert, &old_key, now())
            .unwrap();
        let before = fs::read(cache.root.join("active.json")).unwrap();

        let (new_cert, _new_key) = certificate((2026, 1, 1), (2028, 1, 1));
        let (_, foreign_key) = certificate((2026, 1, 1), (2028, 1, 1));
        assert!(cache
            .activate(HOSTNAME, &new_cert, &foreign_key, now())
            .is_err());
        assert_eq!(fs::read(cache.root.join("active.json")).unwrap(), before);
        assert!(matches!(
            cache.state(HOSTNAME, now()).unwrap(),
            CacheState::Current { .. }
        ));
    }

    struct ScriptedIssuer {
        calls: AtomicUsize,
        outcome: std::sync::Mutex<Option<Result<IssuedCertificate>>>,
    }

    impl CertificateIssuer for ScriptedIssuer {
        fn issue<'a>(
            &'a self,
            _hostname: &'a str,
            _account_record: Option<Zeroizing<Vec<u8>>>,
        ) -> BoxFuture<'a, Result<IssuedCertificate>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let result = self.outcome.lock().unwrap().take().unwrap();
            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn failed_renewal_retains_the_still_valid_generation() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = SecureTlsCache::open(temporary.path().join("tls")).unwrap();
        let (cert, key) = certificate((2026, 1, 1), (2026, 4, 1));
        cache.activate(HOSTNAME, &cert, &key, now()).unwrap();
        let pointer = fs::read(cache.root.join("active.json")).unwrap();
        let issuer = ScriptedIssuer {
            calls: AtomicUsize::new(0),
            outcome: std::sync::Mutex::new(Some(Err(tls_error("scripted_renewal_failure")))),
        };
        let manager = AcmeCertificateManager::new(cache.clone(), issuer);
        let lease = manager.ensure(HOSTNAME, now()).await.unwrap();
        assert_eq!(lease.source, CertificateSource::RetainedAfterRenewalFailure);
        assert_eq!(
            validate_material_at(
                &LoadedMaterial {
                    config: lease.config,
                    chain: rustls_pemfile::certs(&mut Cursor::new(cert))
                        .collect::<std::io::Result<Vec<_>>>()
                        .unwrap(),
                },
                HOSTNAME,
                now(),
            )
            .unwrap(),
            CertificateValidity::Valid
        );
        assert_eq!(fs::read(cache.root.join("active.json")).unwrap(), pointer);
        assert_eq!(manager.issuer.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn valid_long_lived_cache_never_calls_the_issuer() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = SecureTlsCache::open(temporary.path().join("tls")).unwrap();
        let (cert, key) = certificate((2026, 1, 1), (2027, 4, 1));
        cache.activate(HOSTNAME, &cert, &key, now()).unwrap();
        let issuer = ScriptedIssuer {
            calls: AtomicUsize::new(0),
            outcome: std::sync::Mutex::new(Some(Err(tls_error("must_not_issue")))),
        };
        let manager = AcmeCertificateManager::new(cache, issuer);
        let lease = manager.ensure(HOSTNAME, now()).await.unwrap();
        assert_eq!(lease.source, CertificateSource::Cache);
        assert_eq!(manager.issuer.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn successful_renewal_swaps_one_complete_valid_generation() {
        let temporary = tempfile::tempdir().unwrap();
        let cache = SecureTlsCache::open(temporary.path().join("tls")).unwrap();
        let (old_cert, old_key) = certificate((2026, 1, 1), (2026, 4, 1));
        cache
            .activate(HOSTNAME, &old_cert, &old_key, now())
            .unwrap();
        let before = fs::read(cache.root.join("active.json")).unwrap();
        let (new_cert, new_key) = certificate((2026, 1, 1), (2028, 1, 1));
        let issuer = ScriptedIssuer {
            calls: AtomicUsize::new(0),
            outcome: std::sync::Mutex::new(Some(Ok(IssuedCertificate {
                chain_pem: new_cert,
                private_key_pem: new_key,
                account_record: Some(Zeroizing::new(br#"{"version":1}"#.to_vec())),
            }))),
        };
        let manager = AcmeCertificateManager::new(cache.clone(), issuer);
        let lease = manager.ensure(HOSTNAME, now()).await.unwrap();
        assert_eq!(lease.source, CertificateSource::Renewed);
        assert_ne!(fs::read(cache.root.join("active.json")).unwrap(), before);
        assert!(matches!(
            cache.state(HOSTNAME, now()).unwrap(),
            CacheState::Current { renew: false, .. }
        ));
        assert_eq!(manager.issuer.calls.load(Ordering::SeqCst), 1);
    }
}
