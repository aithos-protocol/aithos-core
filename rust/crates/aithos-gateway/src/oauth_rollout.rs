//! OLR-5 — progressive rollout helpers for upstream OAuth engines.
//!
//! Counters are process-local and deliberately free of tokens, codes,
//! verifiers, subjects and Vault coordinates.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::OAuthProtocolEngine;

#[derive(Debug)]
pub struct UpstreamOAuthRolloutMetrics {
    native_success: AtomicU64,
    native_failure: AtomicU64,
    oauth2_success: AtomicU64,
    oauth2_failure: AtomicU64,
    oidc_success: AtomicU64,
    oidc_failure: AtomicU64,
}

impl Default for UpstreamOAuthRolloutMetrics {
    fn default() -> Self {
        Self {
            native_success: AtomicU64::new(0),
            native_failure: AtomicU64::new(0),
            oauth2_success: AtomicU64::new(0),
            oauth2_failure: AtomicU64::new(0),
            oidc_success: AtomicU64::new(0),
            oidc_failure: AtomicU64::new(0),
        }
    }
}

impl UpstreamOAuthRolloutMetrics {
    pub fn record_token_outcome(&self, engine: OAuthProtocolEngine, ok: bool) {
        match (engine, ok) {
            (OAuthProtocolEngine::Native, true) => {
                self.native_success.fetch_add(1, Ordering::Relaxed);
            }
            (OAuthProtocolEngine::Native, false) => {
                self.native_failure.fetch_add(1, Ordering::Relaxed);
            }
            (OAuthProtocolEngine::Oauth2, true) => {
                self.oauth2_success.fetch_add(1, Ordering::Relaxed);
            }
            (OAuthProtocolEngine::Oauth2, false) => {
                self.oauth2_failure.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn record_oidc_outcome(&self, ok: bool) {
        if ok {
            self.oidc_success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.oidc_failure.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Redacted snapshot safe for status / logs.
    pub fn snapshot(&self) -> UpstreamOAuthRolloutSnapshot {
        UpstreamOAuthRolloutSnapshot {
            native_success: self.native_success.load(Ordering::Relaxed),
            native_failure: self.native_failure.load(Ordering::Relaxed),
            oauth2_success: self.oauth2_success.load(Ordering::Relaxed),
            oauth2_failure: self.oauth2_failure.load(Ordering::Relaxed),
            oidc_success: self.oidc_success.load(Ordering::Relaxed),
            oidc_failure: self.oidc_failure.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct UpstreamOAuthRolloutSnapshot {
    pub native_success: u64,
    pub native_failure: u64,
    pub oauth2_success: u64,
    pub oauth2_failure: u64,
    pub oidc_success: u64,
    pub oidc_failure: u64,
}

pub static ROLLOUT_METRICS: UpstreamOAuthRolloutMetrics = UpstreamOAuthRolloutMetrics {
    native_success: AtomicU64::new(0),
    native_failure: AtomicU64::new(0),
    oauth2_success: AtomicU64::new(0),
    oauth2_failure: AtomicU64::new(0),
    oidc_success: AtomicU64::new(0),
    oidc_failure: AtomicU64::new(0),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_has_no_secret_shaped_fields() {
        let metrics = UpstreamOAuthRolloutMetrics::default();
        metrics.record_token_outcome(OAuthProtocolEngine::Oauth2, true);
        metrics.record_oidc_outcome(false);
        let snap = metrics.snapshot();
        let encoded = serde_json::to_string(&snap).unwrap();
        assert!(encoded.contains("oauth2_success"));
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("secret"));
        assert_eq!(snap.oauth2_success, 1);
        assert_eq!(snap.oidc_failure, 1);
    }
}
