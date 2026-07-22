//! Closed OAuth client-registration strategies.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::config::{OAuthClientAuthentication, OAuthRegistrationStrategy, UpstreamOAuthConfig};
use crate::credentials::{CredentialBroker, CredentialRef, SecretValue};
use crate::oauth_discovery::ResolvedOAuthEndpoints;
use crate::{GatewayError, Result};

const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REGISTRATION_BYTES: usize = 64 * 1024;

fn unavailable(reason: &str) -> GatewayError {
    GatewayError::UpstreamOauthUnavailable(reason.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCredentialSource {
    None,
    Static(CredentialRef),
    Registration(CredentialRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedClientRegistration {
    pub client_id: String,
    pub credential: ClientCredentialSource,
}

#[derive(Serialize, Deserialize)]
struct RegistrationRecord {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: Option<String>,
    token_endpoint_auth_method: String,
    redirect_uris: Vec<String>,
    #[serde(default)]
    client_id_issued_at: Option<i64>,
    #[serde(default)]
    client_secret_expires_at: Option<i64>,
    #[serde(default)]
    registration_client_uri: Option<String>,
    #[serde(default)]
    registration_access_token: Option<String>,
}

impl Drop for RegistrationRecord {
    fn drop(&mut self) {
        self.client_id.zeroize();
        self.client_secret.zeroize();
        self.registration_access_token.zeroize();
    }
}

#[derive(Serialize)]
struct RegistrationRequest<'a> {
    client_name: &'a str,
    redirect_uris: [&'a str; 1],
    token_endpoint_auth_method: &'static str,
    grant_types: [&'static str; 2],
    response_types: [&'static str; 1],
}

pub struct OAuthRegistrationClient {
    http: reqwest::Client,
}

impl OAuthRegistrationClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(REGISTRATION_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable("cannot build OAuth registration client"))?;
        Ok(Self { http })
    }

    pub async fn resolve(
        &self,
        config: &UpstreamOAuthConfig,
        endpoints: &ResolvedOAuthEndpoints,
        registration_broker: Option<&Arc<dyn CredentialBroker>>,
        now: i64,
    ) -> Result<ResolvedClientRegistration> {
        match &config.registration {
            OAuthRegistrationStrategy::Static => Ok(ResolvedClientRegistration {
                client_id: config.client_id.clone(),
                credential: config
                    .client_secret
                    .clone()
                    .map_or(ClientCredentialSource::None, ClientCredentialSource::Static),
            }),
            OAuthRegistrationStrategy::Dynamic { endpoint, vault } => {
                let broker = registration_broker
                    .ok_or_else(|| unavailable("OAuth registration Vault is unavailable"))?;
                if let Some(value) = broker
                    .resolve_optional(vault)
                    .await
                    .map_err(|_| unavailable("OAuth registration record is unavailable"))?
                {
                    let record: RegistrationRecord = serde_json::from_str(value.expose())
                        .map_err(|_| unavailable("OAuth registration record is malformed"))?;
                    self.validate_record(config, &record, now)?;
                    return Ok(ResolvedClientRegistration {
                        client_id: record.client_id.clone(),
                        credential: if record.client_secret.is_some() {
                            ClientCredentialSource::Registration(vault.clone())
                        } else {
                            ClientCredentialSource::None
                        },
                    });
                }
                let endpoint = endpoint
                    .as_ref()
                    .or(endpoints.registration_endpoint.as_ref())
                    .ok_or_else(|| {
                        unavailable("authorization server has no registration endpoint")
                    })?;
                let request = RegistrationRequest {
                    client_name: "Aithos Gateway",
                    redirect_uris: [&config.redirect_uri],
                    token_endpoint_auth_method: auth_method(config.client_authentication),
                    grant_types: ["authorization_code", "refresh_token"],
                    response_types: ["code"],
                };
                let response = self
                    .http
                    .post(endpoint)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|error| {
                        unavailable(if error.is_timeout() {
                            "OAuth registration timed out"
                        } else if error.is_connect() {
                            "OAuth registration endpoint is unreachable"
                        } else {
                            "OAuth registration failed"
                        })
                    })?;
                if !response.status().is_success() || response.status().is_redirection() {
                    return Err(unavailable(
                        "OAuth registration endpoint refused the client",
                    ));
                }
                let bytes = bounded_bytes(response).await?;
                let record: RegistrationRecord = serde_json::from_slice(&bytes)
                    .map_err(|_| unavailable("OAuth registration response is malformed"))?;
                self.validate_record(config, &record, now)?;
                let client_id = record.client_id.clone();
                let has_secret = record.client_secret.is_some();
                let encoded = serde_json::to_string(&record)
                    .map_err(|_| unavailable("cannot encode OAuth registration record"))?;
                broker
                    .store(vault, SecretValue::new(encoded))
                    .await
                    .map_err(|_| unavailable("cannot persist OAuth registration record"))?;
                Ok(ResolvedClientRegistration {
                    client_id,
                    credential: if has_secret {
                        ClientCredentialSource::Registration(vault.clone())
                    } else {
                        ClientCredentialSource::None
                    },
                })
            }
            OAuthRegistrationStrategy::ClientMetadataDocument { url } => {
                let response = self.http.get(url).send().await.map_err(|error| {
                    unavailable(if error.is_timeout() {
                        "OAuth client metadata request timed out"
                    } else if error.is_connect() {
                        "OAuth client metadata document is unreachable"
                    } else {
                        "OAuth client metadata request failed"
                    })
                })?;
                if !response.status().is_success() || response.status().is_redirection() {
                    return Err(unavailable("OAuth client metadata document was refused"));
                }
                let bytes = bounded_bytes(response).await?;
                let mut record: RegistrationRecord = serde_json::from_slice(&bytes)
                    .map_err(|_| unavailable("OAuth client metadata document is malformed"))?;
                if (!record.client_id.is_empty() && record.client_id != *url)
                    || record.client_secret.is_some()
                {
                    return Err(unavailable(
                        "OAuth client metadata document changed its public id",
                    ));
                }
                // In CIMD the document URL is the client_id; the metadata
                // document is not required to repeat a `client_id` member.
                record.client_id = url.clone();
                self.validate_record(config, &record, now)?;
                Ok(ResolvedClientRegistration {
                    client_id: url.clone(),
                    credential: ClientCredentialSource::None,
                })
            }
        }
    }

    fn validate_record(
        &self,
        config: &UpstreamOAuthConfig,
        record: &RegistrationRecord,
        now: i64,
    ) -> Result<()> {
        if record.client_id.is_empty()
            || record.token_endpoint_auth_method != auth_method(config.client_authentication)
            || record.redirect_uris.as_slice() != [config.redirect_uri.as_str()]
            || record
                .client_secret_expires_at
                .is_some_and(|expiry| expiry != 0 && expiry <= now)
            || (config.client_authentication == OAuthClientAuthentication::None
                && record.client_secret.is_some())
            || (config.client_authentication != OAuthClientAuthentication::None
                && record.client_secret.as_deref().is_none_or(str::is_empty))
        {
            return Err(unavailable(
                "OAuth client registration did not match its pinned profile",
            ));
        }
        Ok(())
    }
}

pub async fn resolve_registration_secret(
    broker: &Arc<dyn CredentialBroker>,
    reference: &CredentialRef,
) -> Result<SecretValue> {
    let value = broker
        .resolve(reference)
        .await
        .map_err(|_| unavailable("OAuth registration record is unavailable"))?;
    let record: RegistrationRecord = serde_json::from_str(value.expose())
        .map_err(|_| unavailable("OAuth registration record is malformed"))?;
    record
        .client_secret
        .as_ref()
        .filter(|secret| !secret.is_empty())
        .cloned()
        .map(SecretValue::new)
        .ok_or_else(|| unavailable("OAuth registration has no client secret"))
}

fn auth_method(method: OAuthClientAuthentication) -> &'static str {
    match method {
        OAuthClientAuthentication::ClientSecretPost => "client_secret_post",
        OAuthClientAuthentication::ClientSecretBasic => "client_secret_basic",
        OAuthClientAuthentication::None => "none",
    }
}

async fn bounded_bytes(mut response: reqwest::Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_REGISTRATION_BYTES as u64)
    {
        return Err(unavailable("OAuth registration response is too large"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| unavailable("OAuth registration response body is unavailable"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_REGISTRATION_BYTES {
            return Err(unavailable("OAuth registration response is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
