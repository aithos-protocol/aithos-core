//! Bounded OAuth protected-resource and authorization-server discovery.
//!
//! Discovery is profile-driven and never accepts a URL from the browser. The
//! client follows no redirects, caps metadata bytes, pins the configured
//! resource and issuer, and accepts plaintext only for loopback test doubles.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::config::{OAuthClientAuthentication, OAuthEndpointStrategy, UpstreamOAuthConfig};
use crate::{GatewayError, Result};

const METADATA_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_METADATA_BYTES: usize = 64 * 1024;

fn unavailable(reason: &str) -> GatewayError {
    GatewayError::UpstreamOauthUnavailable(reason.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOAuthEndpoints {
    pub issuer: Option<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    pub revocation_endpoint: Option<String>,
    /// Present when AS metadata advertises `jwks_uri` (OLR-4). Never taken
    /// from browser input — only from pinned discovery.
    pub jwks_uri: Option<String>,
}

#[derive(Deserialize)]
struct ProtectedResourceMetadata {
    resource: String,
    #[serde(default)]
    authorization_servers: Vec<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
    #[serde(default)]
    bearer_methods_supported: Vec<String>,
}

#[derive(Deserialize)]
struct AuthorizationServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    registration_endpoint: Option<String>,
    #[serde(default)]
    revocation_endpoint: Option<String>,
    #[serde(default)]
    jwks_uri: Option<String>,
    #[serde(default)]
    code_challenge_methods_supported: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(default)]
    protected_resources: Vec<String>,
}

pub struct OAuthDiscoveryClient {
    http: reqwest::Client,
}

impl OAuthDiscoveryClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(METADATA_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable("cannot build OAuth discovery client"))?;
        Ok(Self { http })
    }

    pub async fn resolve(&self, config: &UpstreamOAuthConfig) -> Result<ResolvedOAuthEndpoints> {
        match &config.endpoints {
            OAuthEndpointStrategy::Static => Ok(ResolvedOAuthEndpoints {
                issuer: config
                    .account_binding
                    .as_ref()
                    .map(|binding| binding.issuer.clone()),
                authorization_endpoint: config.auth_url.clone(),
                token_endpoint: config.token_url.clone(),
                registration_endpoint: None,
                revocation_endpoint: config.revocation_url.clone(),
                jwks_uri: match &config.account_binding.as_ref().map(|b| &b.source) {
                    Some(crate::config::OAuthIdentitySource::IdToken { jwks_uri, .. }) => {
                        Some(jwks_uri.clone())
                    }
                    _ => None,
                },
            }),
            OAuthEndpointStrategy::Discovery {
                protected_resource,
                issuer,
            } => {
                self.resolve_discovery(config, protected_resource, issuer)
                    .await
            }
        }
    }

    async fn resolve_discovery(
        &self,
        config: &UpstreamOAuthConfig,
        protected_resource: &str,
        issuer: &str,
    ) -> Result<ResolvedOAuthEndpoints> {
        validate_https_or_loopback(protected_resource)?;
        validate_https_or_loopback(issuer)?;
        let resource_url = well_known_url(protected_resource, "oauth-protected-resource")?;
        let resource: ProtectedResourceMetadata = self.fetch_json(resource_url).await?;
        if resource.resource != protected_resource
            || resource.authorization_servers.as_slice() != [issuer]
        {
            return Err(unavailable(
                "protected-resource metadata did not match its pinned profile",
            ));
        }
        if !resource.scopes_supported.is_empty()
            && config
                .scopes
                .iter()
                .any(|scope| !resource.scopes_supported.contains(scope))
        {
            return Err(unavailable(
                "protected-resource metadata does not support approved scopes",
            ));
        }
        if !resource.bearer_methods_supported.is_empty()
            && !resource
                .bearer_methods_supported
                .iter()
                .any(|method| method == "header")
        {
            return Err(unavailable(
                "protected resource does not support header bearers",
            ));
        }

        let issuer_url = well_known_url(issuer, "oauth-authorization-server")?;
        let metadata: AuthorizationServerMetadata = self.fetch_json(issuer_url).await?;
        if metadata.issuer != issuer {
            return Err(unavailable("authorization-server metadata issuer mismatch"));
        }
        if !metadata
            .code_challenge_methods_supported
            .iter()
            .any(|method| method == "S256")
        {
            return Err(unavailable("authorization server does not advertise S256"));
        }
        let expected_auth = match config.client_authentication {
            OAuthClientAuthentication::ClientSecretPost => "client_secret_post",
            OAuthClientAuthentication::ClientSecretBasic => "client_secret_basic",
            OAuthClientAuthentication::None => "none",
        };
        if !metadata.token_endpoint_auth_methods_supported.is_empty()
            && !metadata
                .token_endpoint_auth_methods_supported
                .iter()
                .any(|method| method == expected_auth)
        {
            return Err(unavailable(
                "authorization server rejected the pinned client method",
            ));
        }
        if !metadata.protected_resources.is_empty()
            && !metadata
                .protected_resources
                .iter()
                .any(|resource| resource == protected_resource)
        {
            return Err(unavailable(
                "authorization server does not pin the protected resource",
            ));
        }
        if let Some(crate::config::OAuthIdentitySource::IdToken { jwks_uri, .. }) = config
            .account_binding
            .as_ref()
            .map(|binding| &binding.source)
        {
            match metadata.jwks_uri.as_deref() {
                Some(discovered) if discovered == jwks_uri => {}
                Some(_) => {
                    return Err(unavailable(
                        "discovered JWKS endpoint changed its configured pin",
                    ));
                }
                None => {
                    return Err(unavailable(
                        "authorization server did not advertise a JWKS endpoint",
                    ));
                }
            }
        }
        for endpoint in [
            Some(metadata.authorization_endpoint.as_str()),
            Some(metadata.token_endpoint.as_str()),
            metadata.registration_endpoint.as_deref(),
            metadata.revocation_endpoint.as_deref(),
            metadata.jwks_uri.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_https_or_loopback(endpoint)?;
            if !same_origin(issuer, endpoint)? {
                return Err(unavailable(
                    "OAuth metadata endpoint is outside the pinned issuer origin",
                ));
            }
        }
        if !config.auth_url.is_empty() && config.auth_url != metadata.authorization_endpoint {
            return Err(unavailable(
                "discovered authorization endpoint changed its configured pin",
            ));
        }
        if !config.token_url.is_empty() && config.token_url != metadata.token_endpoint {
            return Err(unavailable(
                "discovered token endpoint changed its configured pin",
            ));
        }
        if config
            .revocation_url
            .as_ref()
            .is_some_and(|expected| metadata.revocation_endpoint.as_ref() != Some(expected))
        {
            return Err(unavailable(
                "discovered revocation endpoint changed its configured pin",
            ));
        }
        Ok(ResolvedOAuthEndpoints {
            issuer: Some(issuer.to_owned()),
            authorization_endpoint: metadata.authorization_endpoint,
            token_endpoint: metadata.token_endpoint,
            registration_endpoint: metadata.registration_endpoint,
            revocation_endpoint: metadata.revocation_endpoint,
            jwks_uri: metadata.jwks_uri,
        })
    }

    async fn fetch_json<T: DeserializeOwned>(&self, url: reqwest::Url) -> Result<T> {
        let response = self.http.get(url).send().await.map_err(|error| {
            unavailable(if error.is_timeout() {
                "OAuth metadata request timed out"
            } else if error.is_connect() {
                "OAuth metadata endpoint is unreachable"
            } else {
                "OAuth metadata request failed"
            })
        })?;
        if !response.status().is_success() || response.status().is_redirection() {
            return Err(unavailable("OAuth metadata endpoint refused the request"));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_METADATA_BYTES as u64)
        {
            return Err(unavailable("OAuth metadata response is too large"));
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| unavailable("OAuth metadata body is unavailable"))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_METADATA_BYTES {
                return Err(unavailable("OAuth metadata response is too large"));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| unavailable("OAuth metadata response is malformed"))
    }
}

fn well_known_url(identifier: &str, suffix: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(identifier)
        .map_err(|_| unavailable("OAuth discovery identifier is invalid"))?;
    if parsed.fragment().is_some() {
        return Err(unavailable(
            "OAuth discovery identifier contains a fragment",
        ));
    }
    let mut url = parsed.clone();
    let suffix_path = parsed.path().trim_start_matches('/');
    let path = if suffix_path.is_empty() {
        format!("/.well-known/{suffix}")
    } else {
        format!("/.well-known/{suffix}/{suffix_path}")
    };
    url.set_path(&path);
    Ok(url)
}

fn validate_https_or_loopback(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value).map_err(|_| unavailable("OAuth URL is invalid"))?;
    if url.scheme() == "https" {
        return Ok(());
    }
    let loopback = url.scheme() == "http"
        && url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
    if loopback {
        Ok(())
    } else {
        Err(unavailable("OAuth URL requires HTTPS outside loopback"))
    }
}

fn same_origin(left: &str, right: &str) -> Result<bool> {
    let left = reqwest::Url::parse(left).map_err(|_| unavailable("OAuth issuer URL is invalid"))?;
    let right =
        reqwest::Url::parse(right).map_err(|_| unavailable("OAuth endpoint URL is invalid"))?;
    Ok(left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default())
}
