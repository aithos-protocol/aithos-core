//! Process-wide rustls bootstrap.
//!
//! A workspace build can enable both `ring` (gateway, ACME and relay) and
//! `aws-lc-rs` (the provider's AWS SDK graph) on the same rustls crate. In
//! that shape rustls deliberately refuses to guess a process default. Every
//! gateway binary therefore calls [`install_ring_provider`] before it parses
//! configuration or constructs anything that may build a TLS client.

use std::sync::OnceLock;

const ALREADY_SELECTED: &str =
    "another rustls CryptoProvider was selected before gateway bootstrap";

static INSTALL_RING: OnceLock<std::result::Result<(), &'static str>> = OnceLock::new();

/// Install the gateway's process-wide rustls provider exactly once.
///
/// Repeated calls through this function are idempotent. A provider installed
/// by some other code first is a startup error: accepting it would make the
/// production crypto choice depend on link order or an earlier TLS caller.
pub fn install_ring_provider() -> std::result::Result<(), &'static str> {
    *INSTALL_RING.get_or_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| ALREADY_SELECTED)
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_is_selected_explicitly_and_bootstrap_is_idempotent() {
        install_ring_provider().expect("ring provider installs first");
        install_ring_provider().expect("gateway bootstrap is idempotent");
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}
