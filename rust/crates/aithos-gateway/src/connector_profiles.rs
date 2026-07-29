//! Closed connector profiles and deterministic per-account Vault layout.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::config::{
    ConnectorExecutionProfile, ConnectorProfileConfig, ConnectorProfileRegistration, GatewayConfig,
    OAuthClientAuthentication, OAuthRegistrationStrategy, UpstreamOAuthConfig,
};
use crate::credentials::CredentialRef;
use crate::{GatewayError, Result};

const VAULT_FIELD: &str = "value";
static NEXT_ACCOUNT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorProfileRef {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorInstanceKey {
    pub context: String,
    pub principal: String,
    pub connector: String,
    pub account: String,
}

impl ConnectorInstanceKey {
    pub fn new(context: &str, principal: &str, connector: &str, account: &str) -> Result<Self> {
        if !valid_segment(context)
            || principal.is_empty()
            || principal.len() > 512
            || !valid_segment(connector)
            || !account.starts_with("acct_")
            || !valid_segment(account)
        {
            return Err(GatewayError::ConfigRejected(
                "connector instance identity is invalid".into(),
            ));
        }
        Ok(Self {
            context: context.to_owned(),
            principal: principal.to_owned(),
            connector: connector.to_owned(),
            account: account.to_owned(),
        })
    }

    pub fn issue_account_id() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let serial = NEXT_ACCOUNT.fetch_add(1, Ordering::Relaxed);
        let material = format!("{now}:{}:{serial}", std::process::id());
        let digest = blake3::hash(material.as_bytes()).to_hex();
        format!("acct_{}", &digest.as_str()[..26])
    }

    fn principal_segment(&self) -> String {
        format!("p-{}", blake3::hash(self.principal.as_bytes()).to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthVaultLayout {
    pub registration: CredentialRef,
    pub client_secret: CredentialRef,
    pub pending: CredentialRef,
    pub token: CredentialRef,
    pub revocation: CredentialRef,
    pub outbox: CredentialRef,
}

impl OAuthVaultLayout {
    pub fn derive(broker: &str, key: &ConnectorInstanceKey) -> Self {
        let prefix = format!(
            "connectors/{}/{}/{}/{}",
            key.context,
            key.principal_segment(),
            key.connector,
            key.account
        );
        let reference = |record: &str| CredentialRef {
            broker: broker.to_owned(),
            path: format!("{prefix}/{record}"),
            field: VAULT_FIELD.to_owned(),
        };
        Self {
            registration: reference("registration"),
            client_secret: reference("client-secret"),
            pending: reference("pending"),
            token: reference("token"),
            revocation: reference("revocation"),
            outbox: reference("outbox"),
        }
    }
}

#[derive(Clone)]
pub struct ConnectorProfileCatalog {
    profiles: BTreeMap<(String, String), ConnectorProfileConfig>,
}

impl ConnectorProfileCatalog {
    pub fn from_config(config: &GatewayConfig) -> Self {
        let profiles = config
            .connector_profiles
            .as_deref()
            .unwrap_or_default()
            .iter()
            .cloned()
            .map(|profile| ((profile.id.clone(), profile.version.clone()), profile))
            .collect();
        Self { profiles }
    }

    pub fn enabled(&self, reference: &ConnectorProfileRef) -> Result<&ConnectorProfileConfig> {
        self.profiles
            .get(&(reference.id.clone(), reference.version.clone()))
            .filter(|profile| profile.enabled)
            .ok_or_else(|| GatewayError::ConfigRejected("connector profile is not enabled".into()))
    }

    /// Immutable content pin for a versioned profile. Reusing an id/version
    /// with changed scopes, endpoints, risk or bounds is drift, even when the
    /// compiled/MCP manifest itself did not change.
    pub fn pin(&self, reference: &ConnectorProfileRef) -> Result<String> {
        let profile = self.enabled(reference)?;
        let canonical = serde_jcs::to_vec(profile).map_err(|_| {
            GatewayError::ConfigRejected("connector profile cannot be canonicalized".into())
        })?;
        Ok(format!("b3:{}", blake3::hash(&canonical).to_hex()))
    }

    pub fn materialize_oauth(
        &self,
        reference: &ConnectorProfileRef,
        layout: &OAuthVaultLayout,
    ) -> Result<UpstreamOAuthConfig> {
        let profile = self.enabled(reference)?;
        let oauth = &profile.oauth;
        let registration = match &oauth.registration {
            ConnectorProfileRegistration::Static => OAuthRegistrationStrategy::Static,
            ConnectorProfileRegistration::Dynamic { endpoint } => {
                OAuthRegistrationStrategy::Dynamic {
                    endpoint: endpoint.clone(),
                    vault: layout.registration.clone(),
                }
            }
            ConnectorProfileRegistration::ClientMetadataDocument { url } => {
                OAuthRegistrationStrategy::ClientMetadataDocument { url: url.clone() }
            }
        };
        let client_secret = if matches!(registration, OAuthRegistrationStrategy::Static)
            && oauth.client_authentication != OAuthClientAuthentication::None
        {
            Some(layout.client_secret.clone())
        } else {
            None
        };
        Ok(UpstreamOAuthConfig {
            auth_url: oauth.auth_url.clone(),
            token_url: oauth.token_url.clone(),
            client_id: oauth.client_id.clone(),
            client_secret,
            scopes: oauth.scopes.clone(),
            redirect_uri: oauth.redirect_uri.clone(),
            endpoints: oauth.endpoints.clone(),
            client_authentication: oauth.client_authentication,
            protocol_engine: oauth.protocol_engine,
            registration,
            authorization_parameters: oauth.authorization_parameters.clone(),
            resource: oauth.resource.clone(),
            audience: oauth.audience.clone(),
            revocation_url: oauth.revocation_url.clone(),
            account_binding: oauth.account_binding.clone(),
            pending_vault: Some(layout.pending.clone()),
            revocation_vault: Some(layout.revocation.clone()),
            token_vault: layout.token.clone(),
        })
    }

    pub fn endpoint(&self, reference: &ConnectorProfileRef) -> Result<&str> {
        match &self.enabled(reference)?.execution {
            ConnectorExecutionProfile::Mcp { endpoint, .. } => Ok(endpoint),
            ConnectorExecutionProfile::CompiledRest { api_base_url, .. } => Ok(api_base_url),
        }
    }

    pub fn execution(&self, reference: &ConnectorProfileRef) -> Result<&ConnectorExecutionProfile> {
        Ok(&self.enabled(reference)?.execution)
    }
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !matches!(value, "." | "..")
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ConnectorProfileOAuth, ConnectorRiskClass, OAuthAuthorizationParameters,
        OAuthEndpointStrategy, OAuthProtocolEngine,
    };

    #[test]
    fn vault_layout_is_composite_and_non_aliasing() {
        let key = ConnectorInstanceKey::new(
            "sales",
            "did:aithos:owner:alice",
            "notion-alice",
            "acct_01j00000000000000000000000",
        )
        .unwrap();
        let layout = OAuthVaultLayout::derive("enterprise", &key);
        let paths = [
            &layout.registration.path,
            &layout.client_secret.path,
            &layout.pending.path,
            &layout.token.path,
            &layout.revocation.path,
            &layout.outbox.path,
        ];
        assert_eq!(
            paths
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            6
        );
        assert!(paths
            .iter()
            .all(|path| path.starts_with("connectors/sales/p-")));
    }

    #[test]
    fn profile_pin_covers_scopes_and_not_only_id_version() {
        let make_profile = |scope: &str| ConnectorProfileConfig {
            id: "notion-read".into(),
            version: "1".into(),
            enabled: true,
            risk: ConnectorRiskClass::Read,
            execution: ConnectorExecutionProfile::Mcp {
                endpoint: "https://mcp.example.test/mcp".into(),
                manifest_id: "notion-read".into(),
                manifest_pin: "sha256:manifest".into(),
            },
            oauth: ConnectorProfileOAuth {
                credential_broker: "enterprise".into(),
                auth_url: "https://as.example.test/authorize".into(),
                token_url: "https://as.example.test/token".into(),
                client_id: "client".into(),
                scopes: vec![scope.into()],
                redirect_uri: "https://gateway.example.test/oauth/callback".into(),
                endpoints: OAuthEndpointStrategy::Static,
                client_authentication: OAuthClientAuthentication::None,
                protocol_engine: OAuthProtocolEngine::Oauth2,
                registration: ConnectorProfileRegistration::Static,
                authorization_parameters: OAuthAuthorizationParameters::default(),
                resource: None,
                audience: None,
                revocation_url: None,
                account_binding: None,
            },
        };
        let reference = ConnectorProfileRef {
            id: "notion-read".into(),
            version: "1".into(),
        };
        let first = ConnectorProfileCatalog {
            profiles: BTreeMap::from([(
                ("notion-read".into(), "1".into()),
                make_profile("read.one"),
            )]),
        };
        let drifted = ConnectorProfileCatalog {
            profiles: BTreeMap::from([(
                ("notion-read".into(), "1".into()),
                make_profile("read.two"),
            )]),
        };
        assert_ne!(
            first.pin(&reference).unwrap(),
            drifted.pin(&reference).unwrap()
        );
        let key = ConnectorInstanceKey::new(
            "sales",
            "did:aithos:owner:alice",
            "notion-alice",
            "acct_01j00000000000000000000000",
        )
        .unwrap();
        let layout = OAuthVaultLayout::derive("enterprise", &key);
        assert_eq!(
            first
                .materialize_oauth(&reference, &layout)
                .unwrap()
                .protocol_engine,
            OAuthProtocolEngine::Oauth2
        );
    }
}
