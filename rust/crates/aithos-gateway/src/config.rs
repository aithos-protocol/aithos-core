//! Onboarding configuration: what the enterprise declares before plugging
//! an agent in. Parsed fail-closed: unknown keys, unknown access levels or
//! an unusable store are rejected outright, never guessed at.
//!
//! Decided 2026-07-10: YAML whitelist. Example:
//!
//! ```yaml
//! listen: 127.0.0.1:4870
//! upstream_mcp: http://127.0.0.1:4124/mcp
//! store:
//!   kind: fs
//!   root: /var/lib/aithos
//! tools:
//!   user.read: read
//!   user.update: write
//! ```
//!
//! Semantics: `read` tools are covered by the generated read-only mandate;
//! `write` tools are known but *not* granted (so refusals name the tool
//! precisely); anything absent from `tools` is denied by default.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::{GatewayError, Result};

/// Access level the enterprise assigns to an MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolAccess {
    /// Covered by the read-only mandate: relayed and logged.
    Read,
    /// Known but outside the read-only mandate: refused and logged.
    Write,
}

/// Tool name → access level. BTreeMap for deterministic iteration
/// (mandate generation must be reproducible).
pub type ToolMap = BTreeMap<String, ToolAccess>;

/// Where the ethos (bundle + gamma) lives. Both variants parse from day
/// one — decided 2026-07-10: local disk first, cloud must stay possible.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum StoreConfig {
    /// Local filesystem (v1 default).
    Fs { root: PathBuf },
    /// S3-compatible object store (accepted by the parser, refused by the
    /// v1 adapter — see `store_adapter`).
    S3 {
        bucket: String,
        #[serde(default)]
        prefix: Option<String>,
    },
}

/// The whole gateway configuration file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfig {
    /// Agent-facing bind address (the pod-internal endpoint).
    pub listen: String,
    /// The real MCP server the gateway relays to.
    pub upstream_mcp: String,
    /// Where the ethos lives.
    pub store: StoreConfig,
    /// The enterprise tool whitelist. Empty map = everything denied.
    #[serde(default)]
    pub tools: ToolMap,
}

impl GatewayConfig {
    /// Parse and validate a YAML config. Any ambiguity is a rejection.
    pub fn from_yaml(text: &str) -> Result<Self> {
        let cfg: GatewayConfig = serde_yaml::from_str(text)
            .map_err(|e| GatewayError::ConfigRejected(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        if self.listen.trim().is_empty() {
            return Err(GatewayError::ConfigRejected("`listen` is empty".into()));
        }
        if !(self.upstream_mcp.starts_with("http://")
            || self.upstream_mcp.starts_with("https://"))
        {
            return Err(GatewayError::ConfigRejected(format!(
                "`upstream_mcp` must be an http(s) URL, got `{}`",
                self.upstream_mcp
            )));
        }
        if let StoreConfig::Fs { root } = &self.store {
            if root.as_os_str().is_empty() {
                return Err(GatewayError::ConfigRejected("`store.root` is empty".into()));
            }
        }
        for tool in self.tools.keys() {
            if tool.trim().is_empty() {
                return Err(GatewayError::ConfigRejected(
                    "empty tool name in `tools`".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "\
listen: 127.0.0.1:4870
upstream_mcp: http://127.0.0.1:4124/mcp
store:
  kind: fs
  root: /var/lib/aithos
tools:
  user.read: read
  user.update: write
";

    #[test]
    fn parses_a_valid_config() {
        let cfg = GatewayConfig::from_yaml(GOOD).unwrap();
        assert_eq!(cfg.tools.get("user.read"), Some(&ToolAccess::Read));
        assert_eq!(cfg.tools.get("user.update"), Some(&ToolAccess::Write));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let text = format!("{GOOD}surprise: true\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn unknown_access_level_is_rejected() {
        let text = GOOD.replace("read\n", "admin\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn non_http_upstream_is_rejected() {
        let text = GOOD.replace("http://127.0.0.1:4124/mcp", "ftp://x");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }
}
