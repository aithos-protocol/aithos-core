//! TLS material for the relay's tunnel door (annexe B.1, jalon M2).
//!
//! The relay terminates exactly ONE TLS: the pod-facing tunnel door, under
//! **its own certificate** (`relay.aithos.fr`, an Aithos secret — never a
//! client key, A3). This module loads that cert+key from PEM the
//! environment supplies (a mounted secret in the task; a self-signed pair
//! in tests) and builds a rustls server config that **requires** ALPN
//! `aithos-tunnel/1`. Public browser TLS is never terminated here — it is
//! piped to the pod (annexe B.3), whose own key stays client-side.
//!
//! Ring is the crypto provider (no aws-lc C build on the relay's hot
//! path). Fail-closed: a config that cannot be built refuses to boot.

use std::io::Cursor;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

use crate::sni::TUNNEL_ALPN;

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("tunnel TLS certificate PEM is empty or unparseable")]
    NoCert,
    #[error("tunnel TLS private key PEM is empty or unparseable")]
    NoKey,
    #[error("tunnel TLS config rejected: {0}")]
    Config(String),
}

/// Parse a PEM certificate chain (leaf first).
pub fn load_cert_chain(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let chain: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut Cursor::new(pem))
        .filter_map(Result::ok)
        .collect();
    if chain.is_empty() {
        return Err(TlsError::NoCert);
    }
    Ok(chain)
}

/// Parse a PEM private key (PKCS#8, PKCS#1 or SEC1 — the first found).
pub fn load_private_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>, TlsError> {
    rustls_pemfile::private_key(&mut Cursor::new(pem))
        .ok()
        .flatten()
        .ok_or(TlsError::NoKey)
}

/// Build the tunnel-door server config: the relay's own cert, ALPN pinned
/// to `aithos-tunnel/1` (a pod that offers no matching ALPN fails the
/// handshake — it never reaches the registration line). Ring provider.
pub fn tunnel_server_config(
    cert_chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
) -> Result<Arc<ServerConfig>, TlsError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsError::Config(e.to_string()))?
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|e| TlsError::Config(e.to_string()))?;
    // The tunnel door speaks exactly one protocol; anything else is refused
    // at the handshake (no application byte is ever read on a bad ALPN).
    config.alpn_protocols = vec![TUNNEL_ALPN.to_vec()];
    Ok(Arc::new(config))
}

/// Convenience: build the tunnel-door config straight from PEM bytes.
pub fn tunnel_server_config_from_pem(
    cert_pem: &[u8],
    key_pem: &[u8],
) -> Result<Arc<ServerConfig>, TlsError> {
    tunnel_server_config(load_cert_chain(cert_pem)?, load_private_key(key_pem)?)
}
