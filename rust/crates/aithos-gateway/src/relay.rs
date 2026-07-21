//! G1 outbound relay client: TLS to the existing C2 tunnel door, one
//! byte-exact B.2 registration line, then yamux in server mode.
//!
//! This module is deliberately independent from `aithos-provider` in the
//! production graph. Compatibility is locked by the committed P3 vector
//! and by dev-only tests through the provider verifier. It introduces no
//! protocol grammar and never handles public HTTP bytes itself.

use std::future::{poll_fn, Future};
use std::io::Cursor;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use rustls::pki_types::{CertificateDer, ServerName};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_rustls::TlsConnector;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::config::{RelayConfig, RelayReconnectConfig};
use crate::core_bridge::gateway_tunnel_registration_line;
use crate::keyholder::Keyholder;
use crate::{GatewayError, Result};

pub const TUNNEL_WIRE_VERSION: &str = "1.0.0-draft.1";
pub const TUNNEL_ALPN: &[u8] = b"aithos-tunnel/1";
pub const MAX_REGISTRATION_BYTES: usize = 4 * 1024;
pub const MAX_ANSWER_BYTES: usize = 4 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

const KEEPALIVE_IDLE_SECS: u64 = 30;
const KEEPALIVE_INTERVAL_SECS: u64 = 10;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const KEEPALIVE_RETRIES: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Registration {
    #[serde(rename = "aithos-tunnel")]
    version: String,
    tenant: String,
    hostname: String,
    gateway_pub: String,
    at: String,
    nonce: String,
    signature: RegistrationSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RegistrationSignature {
    alg: String,
    value: String,
}

/// The sole bounded signing entry point used by `core_bridge`. P3 fixes
/// the canonical bytes and signature convention independently.
pub(crate) fn registration_line_with_key(
    tenant: &str,
    hostname: &str,
    gateway_pub: &str,
    at: &str,
    nonce: &str,
    key: &SigningKey,
) -> Result<Vec<u8>> {
    if tenant.is_empty()
        || hostname.is_empty()
        || gateway_pub.is_empty()
        || at.is_empty()
        || nonce.is_empty()
        || nonce.len() > 64
        || nonce.contains('\n')
    {
        return Err(GatewayError::RelayUnavailable(
            "registration_input_invalid".into(),
        ));
    }
    let mut registration = Registration {
        version: TUNNEL_WIRE_VERSION.into(),
        tenant: tenant.into(),
        hostname: hostname.into(),
        gateway_pub: gateway_pub.into(),
        at: at.into(),
        nonce: nonce.into(),
        signature: RegistrationSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let unsigned = serde_jcs::to_vec(&registration)
        .map_err(|_| GatewayError::RelayUnavailable("registration_encode_failed".into()))?;
    registration.signature.value = hex::encode(key.sign(&unsigned).to_bytes());
    let mut line = serde_jcs::to_vec(&registration)
        .map_err(|_| GatewayError::RelayUnavailable("registration_encode_failed".into()))?;
    line.push(b'\n');
    if line.len() > MAX_REGISTRATION_BYTES {
        return Err(GatewayError::RelayUnavailable(
            "registration_too_large".into(),
        ));
    }
    Ok(line)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RelayAnswer {
    Accepted(AcceptedAnswer),
    Refused(RefusedAnswer),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptedAnswer {
    #[serde(rename = "aithos-tunnel")]
    version: String,
    ok: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefusedAnswer {
    ok: bool,
    error: String,
}

fn parse_answer(line: &[u8]) -> Result<()> {
    let answer: RelayAnswer = serde_json::from_slice(line)
        .map_err(|_| GatewayError::RelayUnavailable("answer_invalid".into()))?;
    match answer {
        RelayAnswer::Accepted(answer) if answer.ok && answer.version == TUNNEL_WIRE_VERSION => {
            Ok(())
        }
        RelayAnswer::Refused(answer) if !answer.ok && is_refusal_code(&answer.error) => Err(
            GatewayError::RelayUnavailable(format!("registration_refused:{}", answer.error)),
        ),
        _ => Err(GatewayError::RelayUnavailable("answer_invalid".into())),
    }
}

fn is_refusal_code(code: &str) -> bool {
    matches!(
        code,
        "envelope_invalid"
            | "clock_skew"
            | "nonce_replayed"
            | "signature_invalid"
            | "mapping_mismatch"
            | "suspended"
            | "rate_limited"
            | "unavailable"
    )
}

async fn read_answer_line<S>(stream: &mut S) -> Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut line = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    loop {
        let read = stream
            .read(&mut byte)
            .await
            .map_err(|_| GatewayError::RelayUnavailable("answer_io_failed".into()))?;
        if read == 0 {
            return Err(GatewayError::RelayUnavailable("answer_truncated".into()));
        }
        if byte[0] == b'\n' {
            return Ok(line);
        }
        line.push(byte[0]);
        if line.len() > MAX_ANSWER_BYTES {
            return Err(GatewayError::RelayUnavailable("answer_too_large".into()));
        }
    }
}

fn enable_keepalive(stream: &TcpStream) -> std::io::Result<()> {
    let socket = socket2::SockRef::from(stream);
    #[allow(unused_mut)]
    let mut keepalive = socket2::TcpKeepalive::new()
        .with_time(Duration::from_secs(KEEPALIVE_IDLE_SECS))
        .with_interval(Duration::from_secs(KEEPALIVE_INTERVAL_SECS));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        keepalive = keepalive.with_retries(KEEPALIVE_RETRIES);
    }
    socket.set_tcp_keepalive(&keepalive)
}

type RelayTransport = tokio_util::compat::Compat<tokio_rustls::client::TlsStream<TcpStream>>;

/// One accepted C2 tunnel. Each inbound yamux stream is one still-encrypted
/// public TLS connection; G1b terminates that TLS on the client.
pub struct RelaySession {
    connection: yamux::Connection<RelayTransport>,
}

impl RelaySession {
    pub async fn accept(&mut self) -> Result<Option<yamux::Stream>> {
        match poll_fn(|context| self.connection.poll_next_inbound(context)).await {
            Some(Ok(stream)) => Ok(Some(stream)),
            Some(Err(_)) => Err(GatewayError::RelayUnavailable("mux_closed".into())),
            None => Ok(None),
        }
    }

    pub async fn close(&mut self) {
        let _ = poll_fn(|context| self.connection.poll_close(context)).await;
    }
}

/// TLS dialer with immutable roots. Production loads the OS/container CA
/// bundle; tests inject one local CA without weakening verification.
#[derive(Clone)]
pub struct RelayClient {
    config: RelayConfig,
    endpoint_host: String,
    endpoint_port: u16,
    tls: Arc<rustls::ClientConfig>,
}

impl RelayClient {
    pub fn from_system_roots(config: RelayConfig) -> Result<Self> {
        let roots = load_system_roots()?;
        Self::with_root_certificates(config, roots)
    }

    pub fn with_root_certificates(
        config: RelayConfig,
        certificates: Vec<CertificateDer<'static>>,
    ) -> Result<Self> {
        let endpoint = reqwest::Url::parse(&config.endpoint)
            .map_err(|_| GatewayError::RelayUnavailable("endpoint_invalid".into()))?;
        let endpoint_host = endpoint
            .host_str()
            .ok_or_else(|| GatewayError::RelayUnavailable("endpoint_invalid".into()))?
            .to_owned();
        let endpoint_port = endpoint
            .port_or_known_default()
            .ok_or_else(|| GatewayError::RelayUnavailable("endpoint_invalid".into()))?;
        if certificates.is_empty() {
            return Err(GatewayError::RelayUnavailable("roots_unavailable".into()));
        }
        let mut roots = rustls::RootCertStore::empty();
        for certificate in certificates {
            roots
                .add(certificate)
                .map_err(|_| GatewayError::RelayUnavailable("roots_invalid".into()))?;
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_| GatewayError::RelayUnavailable("tls_config_invalid".into()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        tls.alpn_protocols = vec![TUNNEL_ALPN.to_vec()];
        Ok(Self {
            config,
            endpoint_host,
            endpoint_port,
            tls: Arc::new(tls),
        })
    }

    pub fn config(&self) -> &RelayConfig {
        &self.config
    }

    /// One bounded attempt. A rejected B.2 answer returns before yamux is
    /// constructed, so a refusal can never emit an application frame.
    pub async fn connect(
        &self,
        identity: &Keyholder,
        at: &str,
        nonce: &str,
    ) -> Result<RelaySession> {
        // Runtime guidance is ≥16 chars/≥96 bits; the P3 compatibility
        // builder remains able to replay its older 10-character nonce.
        if !(16..=64).contains(&nonce.len()) {
            return Err(GatewayError::RelayUnavailable("nonce_invalid".into()));
        }
        let tcp = tokio::time::timeout(
            IO_TIMEOUT,
            TcpStream::connect((self.endpoint_host.as_str(), self.endpoint_port)),
        )
        .await
        .map_err(|_| GatewayError::RelayUnavailable("tcp_timeout".into()))?
        .map_err(|_| GatewayError::RelayUnavailable("tcp_unavailable".into()))?;
        // Keepalive is liveness hygiene, not an authorization gate.
        let _ = enable_keepalive(&tcp);

        let server_name = ServerName::try_from(self.config.tunnel_name.clone())
            .map_err(|_| GatewayError::RelayUnavailable("tunnel_name_invalid".into()))?;
        let connector = TlsConnector::from(Arc::clone(&self.tls));
        let mut tls = tokio::time::timeout(IO_TIMEOUT, connector.connect(server_name, tcp))
            .await
            .map_err(|_| GatewayError::RelayUnavailable("tls_timeout".into()))?
            .map_err(|_| GatewayError::RelayUnavailable("tls_refused".into()))?;
        if tls.get_ref().1.alpn_protocol() != Some(TUNNEL_ALPN) {
            return Err(GatewayError::RelayUnavailable("alpn_refused".into()));
        }

        let line = gateway_tunnel_registration_line(
            identity,
            &self.config.tenant,
            &self.config.hostname,
            at,
            nonce,
        )?;
        tokio::time::timeout(IO_TIMEOUT, async {
            tls.write_all(&line).await?;
            tls.flush().await
        })
        .await
        .map_err(|_| GatewayError::RelayUnavailable("registration_timeout".into()))?
        .map_err(|_| GatewayError::RelayUnavailable("registration_io_failed".into()))?;

        let answer = tokio::time::timeout(IO_TIMEOUT, read_answer_line(&mut tls))
            .await
            .map_err(|_| GatewayError::RelayUnavailable("answer_timeout".into()))??;
        parse_answer(&answer)?;

        let connection =
            yamux::Connection::new(tls.compat(), yamux::Config::default(), yamux::Mode::Server);
        Ok(RelaySession { connection })
    }

    /// Reconnect on refusal, EOF or GoAway with fresh caller-provided
    /// clock/nonce inputs. Shutdown closes the mux and aborts stream tasks.
    pub async fn run<Handler, HandlerFuture>(
        &self,
        identity: Arc<Keyholder>,
        inputs: RelayInputs,
        health: RelayHealth,
        mut shutdown: watch::Receiver<bool>,
        handler: Handler,
    ) -> Result<()>
    where
        Handler: Fn(yamux::Stream) -> HandlerFuture + Clone + Send + Sync + 'static,
        HandlerFuture: Future<Output = ()> + Send + 'static,
    {
        let backoff = ReconnectBackoff::new(self.config.reconnect.clone());
        let mut attempt = 0u32;
        loop {
            if *shutdown.borrow() {
                health.set(RelayReadiness::Disabled);
                return Ok(());
            }
            health.set(RelayReadiness::Connecting);
            let at = (inputs.clock)();
            let nonce = (inputs.nonce)();
            if let Ok(mut session) = self.connect(&identity, &at, &nonce).await {
                health.set(RelayReadiness::Ready);
                attempt = 0;
                let mut handlers = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() {
                                session.close().await;
                                handlers.abort_all();
                                while handlers.join_next().await.is_some() {}
                                health.set(RelayReadiness::Disabled);
                                return Ok(());
                            }
                        }
                        stream = session.accept() => {
                            match stream {
                                Ok(Some(stream)) => {
                                    let serve = handler.clone();
                                    handlers.spawn(serve(stream));
                                }
                                Ok(None) | Err(_) => break,
                            }
                        }
                        completed = handlers.join_next(), if !handlers.is_empty() => {
                            let _ = completed;
                        }
                    }
                }
                handlers.abort_all();
                while handlers.join_next().await.is_some() {}
            }

            health.set(RelayReadiness::Unavailable);
            let delay = backoff.delay(attempt, (inputs.jitter)());
            attempt = attempt.saturating_add(1);
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        health.set(RelayReadiness::Disabled);
                        return Ok(());
                    }
                }
            }
        }
    }
}

fn load_system_roots() -> Result<Vec<CertificateDer<'static>>> {
    let bundle = std::fs::read("/etc/ssl/certs/ca-certificates.crt")
        .or_else(|_| std::fs::read("/etc/ssl/cert.pem"))
        .map_err(|_| GatewayError::RelayUnavailable("roots_unavailable".into()))?;
    rustls_pemfile::certs(&mut Cursor::new(bundle))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| GatewayError::RelayUnavailable("roots_invalid".into()))
}

#[derive(Clone)]
pub struct RelayInputs {
    pub clock: Arc<dyn Fn() -> String + Send + Sync>,
    pub nonce: Arc<dyn Fn() -> String + Send + Sync>,
    pub jitter: Arc<dyn Fn() -> u64 + Send + Sync>,
}

#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    config: RelayReconnectConfig,
}

impl ReconnectBackoff {
    pub fn new(config: RelayReconnectConfig) -> Self {
        Self { config }
    }

    /// Exponential delay with injected symmetric jitter. The final value,
    /// not only the nominal one, is capped by `max_ms`.
    pub fn delay(&self, attempt: u32, sample: u64) -> Duration {
        let multiplier = 1u64.checked_shl(attempt.min(63)).unwrap_or(u64::MAX);
        let nominal = self
            .config
            .base_ms
            .saturating_mul(multiplier)
            .min(self.config.max_ms);
        let radius = nominal.saturating_mul(u64::from(self.config.jitter_percent)) / 100;
        let span = radius.saturating_mul(2).saturating_add(1);
        let offset = sample % span;
        let jittered = nominal.saturating_sub(radius).saturating_add(offset);
        Duration::from_millis(jittered.min(self.config.max_ms))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RelayReadiness {
    Disabled = 0,
    Connecting = 1,
    Ready = 2,
    Unavailable = 3,
}

#[derive(Clone)]
pub struct RelayHealth(Arc<AtomicU8>);

impl RelayHealth {
    pub fn new(initial: RelayReadiness) -> Self {
        Self(Arc::new(AtomicU8::new(initial as u8)))
    }

    pub fn get(&self) -> RelayReadiness {
        match self.0.load(Ordering::Acquire) {
            0 => RelayReadiness::Disabled,
            1 => RelayReadiness::Connecting,
            2 => RelayReadiness::Ready,
            _ => RelayReadiness::Unavailable,
        }
    }

    fn set(&self, readiness: RelayReadiness) {
        self.0.store(readiness as u8, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_is_byte_exact_to_p3() {
        let vector: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../vectors/p3-tunnel-register.json"
        )))
        .unwrap();
        let key = Keyholder::from_entropy([0x42; 32], [0x51; 32]);
        let line = gateway_tunnel_registration_line(
            &key,
            "acme",
            "demo.mcp.aithos.fr",
            "2026-07-16T12:00:00Z",
            "p0-t-ok-01",
        )
        .unwrap();
        assert_eq!(
            line,
            vector["cases"][0]["line"].as_str().unwrap().as_bytes()
        );
    }

    #[test]
    fn relay_answer_is_closed_and_finite() {
        assert!(parse_answer(br#"{"aithos-tunnel":"1.0.0-draft.1","ok":true}"#).is_ok());
        assert!(matches!(
            parse_answer(br#"{"ok":false,"error":"mapping_mismatch"}"#),
            Err(GatewayError::RelayUnavailable(reason))
                if reason == "registration_refused:mapping_mismatch"
        ));
        for invalid in [
            br#"{"aithos-tunnel":"1.0.0-draft.1","ok":true,"extra":1}"#.as_slice(),
            br#"{"ok":false,"error":"new_protocol_code"}"#.as_slice(),
            br#"{"ok":true}"#.as_slice(),
        ] {
            assert!(parse_answer(invalid).is_err());
        }
    }

    #[test]
    fn reconnect_backoff_is_exponential_jittered_and_capped() {
        let backoff = ReconnectBackoff::new(RelayReconnectConfig {
            base_ms: 1_000,
            max_ms: 60_000,
            jitter_percent: 20,
        });
        assert_eq!(backoff.delay(0, 200).as_millis(), 1_000);
        assert_eq!(backoff.delay(1, 400).as_millis(), 2_000);
        assert_eq!(backoff.delay(2, 800).as_millis(), 4_000);
        assert!((800..=1_200).contains(&backoff.delay(0, u64::MAX).as_millis()));
        assert!(backoff.delay(63, u64::MAX) <= Duration::from_secs(60));
    }
}
