//! Reproducible CLI assets for the documented Léa demonstration.
//!
//! This module deliberately generates configuration coordinates only:
//! no bearer, Vault token, owner seed or auditor seed can enter the YAML.

use serde_json::json;

use crate::config::GatewayConfig;
use crate::{GatewayError, Result};

#[derive(Debug, Clone)]
pub struct DemoLeaConfigInput {
    pub listen: String,
    pub vault_address: String,
    pub provider_url: String,
    pub tenant: String,
    pub notion_url: String,
    pub gmail_url: String,
    pub calendar_url: String,
    pub context_root: String,
    pub context_did: String,
    pub context_mandate: String,
    pub journal_sidecar: String,
    pub journal_did: String,
    pub journal_mandate: String,
}

/// Render the provider-backed Léa topology used by the P3 gate:
/// `ventes` local-primary/replicated (mode A), journal provider-primary
/// with a local sidecar (mode B), and three distinct brokered upstreams.
pub fn render_provider_config(input: &DemoLeaConfigInput) -> Result<String> {
    let value = json!({
        "listen": input.listen,
        "credential_brokers": {
            "enterprise": {
                "kind": "vault-kv2",
                "address": input.vault_address,
                "mount": "secret",
                "auth": { "kind": "token-env", "env": "AITHOS_VAULT_TOKEN" }
            }
        },
        "servers": [
            {
                "name": "notion", "transport": "http", "url": input.notion_url,
                "credential": { "broker": "enterprise", "path": "aithos/mcp/notion", "field": "token" }
            },
            {
                "name": "gmail", "transport": "http", "url": input.gmail_url,
                "credential": { "broker": "enterprise", "path": "aithos/mcp/gmail", "field": "token" }
            },
            {
                "name": "calendar", "transport": "http", "url": input.calendar_url,
                "credential": { "broker": "enterprise", "path": "aithos/mcp/calendar", "field": "token" }
            }
        ],
        "contexts": [{
            "name": "ventes",
            "store": {
                "kind": "replicated",
                "root": input.context_root,
                "url": input.provider_url,
                "tenant": input.tenant,
                "did": input.context_did,
                "mandate": [input.context_mandate]
            },
            "tools": {
                "notion__query_database": { "server": "notion", "tool": "query_database", "access": "read", "granted": true },
                "notion__create_page": { "server": "notion", "tool": "create_page", "access": "write", "granted": false },
                "gmail__search_emails": { "server": "gmail", "tool": "search_emails", "access": "read", "granted": true },
                "gmail__send_email": { "server": "gmail", "tool": "send_email", "access": "write", "granted": true },
                "gmail__delete_email": { "server": "gmail", "tool": "delete_email", "access": "write", "granted": false },
                "calendar__list_events": { "server": "calendar", "tool": "list_events", "access": "read", "granted": true },
                "calendar__create_event": { "server": "calendar", "tool": "create_event", "access": "write", "granted": true }
            }
        }],
        "journal": {
            "store": {
                "kind": "remote",
                "url": input.provider_url,
                "tenant": input.tenant,
                "did": input.journal_did,
                "mandate": [input.journal_mandate],
                "local": input.journal_sidecar
            }
        }
    });
    let yaml = serde_yaml::to_string(&value)
        .map_err(|e| GatewayError::ConfigRejected(format!("demo config render: {e}")))?;
    GatewayConfig::from_yaml(&yaml)?;
    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> DemoLeaConfigInput {
        DemoLeaConfigInput {
            listen: "127.0.0.1:4890".into(),
            vault_address: "http://127.0.0.1:8200".into(),
            provider_url: "https://store.aithos.fr".into(),
            tenant: "demo-lea-20260721".into(),
            notion_url: "http://127.0.0.1:9201/mcp".into(),
            gmail_url: "http://127.0.0.1:9202/mcp".into(),
            calendar_url: "http://127.0.0.1:9203/mcp".into(),
            context_root: "/tmp/aithos-lea-demo/ventes".into(),
            context_did: "did:aithos:z6Mktestcontext".into(),
            context_mandate: "m_context".into(),
            journal_sidecar: "/tmp/aithos-lea-demo/journal".into(),
            journal_did: "did:aithos:z6Mktestjournal".into(),
            journal_mandate: "m_memory".into(),
        }
    }

    #[test]
    fn provider_demo_config_is_parseable_and_contains_references_only() {
        let yaml = render_provider_config(&input()).unwrap();
        let parsed = GatewayConfig::from_yaml(&yaml).unwrap();
        assert_eq!(parsed.listen, "127.0.0.1:4890");
        assert!(yaml.contains("kind: replicated"));
        assert!(yaml.contains("kind: remote"));
        assert!(yaml.contains("AITHOS_VAULT_TOKEN"));
        for forbidden in ["vault-root-token", "notion-secret", "gmail-secret"] {
            assert!(!yaml.contains(forbidden));
        }
    }
}
