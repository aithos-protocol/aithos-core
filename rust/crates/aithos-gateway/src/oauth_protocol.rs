//! Upstream OAuth protocol engines (OLR seam).
//!
//! Custody, discovery, DCR, typed profile parameters and Vault remain in
//! [`crate::upstream_oauth`]. This module only owns token-endpoint exchange
//! and refresh (Authorization Code + PKCE verifier / refresh_token grants).

use std::collections::BTreeMap;
#[cfg(feature = "olr-oauth-libs")]
use std::future::Future;
#[cfg(feature = "olr-oauth-libs")]
use std::pin::Pin;
#[cfg(feature = "olr-oauth-libs")]
use std::time::Duration;

#[cfg(feature = "olr-oauth-libs")]
use oauth2::basic::{
    BasicErrorResponse, BasicErrorResponseType, BasicRevocationErrorResponse, BasicTokenType,
};
#[cfg(feature = "olr-oauth-libs")]
use oauth2::{
    AuthType, AuthorizationCode, Client, ClientId, ClientSecret, EmptyExtraTokenFields,
    EndpointNotSet, EndpointSet, ExtraTokenFields, PkceCodeVerifier, RedirectUrl, RefreshToken,
    RequestTokenError, StandardRevocableToken, StandardTokenIntrospectionResponse,
    StandardTokenResponse, TokenResponse, TokenUrl,
};
use serde::Deserialize;
#[cfg(feature = "olr-oauth-libs")]
use serde::Serialize;
use zeroize::Zeroize;

use crate::config::{OAuthClientAuthentication, OAuthProtocolEngine};
use crate::credentials::SecretValue;
use crate::{GatewayError, Result};

#[cfg(feature = "olr-oauth-libs")]
const TOKEN_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OAUTH_RESPONSE_BYTES: usize = 64 * 1024;

fn unavailable(reason: &str) -> GatewayError {
    GatewayError::UpstreamOauthUnavailable(reason.to_owned())
}

fn zeroize_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => text.zeroize(),
        serde_json::Value::Array(values) => {
            for value in values {
                zeroize_json(value);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                zeroize_json(value);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

/// Effective engine after config + optional process override.
pub fn resolve_protocol_engine(configured: OAuthProtocolEngine) -> OAuthProtocolEngine {
    match std::env::var("AITHOS_UPSTREAM_OAUTH_ENGINE")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "oauth2" | "lib" | "library" => OAuthProtocolEngine::Oauth2,
        "native" | "legacy" | "house" => OAuthProtocolEngine::Native,
        _ => configured,
    }
}

#[derive(Debug, Clone)]
pub struct TokenEndpointContext {
    pub token_url: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub client_authentication: OAuthClientAuthentication,
}

#[derive(Deserialize)]
pub struct ProtocolTokenAnswer {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub token_type: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

impl Drop for ProtocolTokenAnswer {
    fn drop(&mut self) {
        self.access_token.zeroize();
        if let Some(refresh) = &mut self.refresh_token {
            refresh.zeroize();
        }
        for value in self.additional.values_mut() {
            zeroize_json(value);
        }
    }
}

fn validate_token_answer(answer: ProtocolTokenAnswer) -> Result<ProtocolTokenAnswer> {
    if answer.access_token.is_empty()
        || !answer.token_type.eq_ignore_ascii_case("bearer")
        || answer.expires_in == 0
    {
        return Err(unavailable("OAuth token answer is incomplete"));
    }
    Ok(answer)
}

async fn bounded_response_bytes(response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    use futures::StreamExt;
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| unavailable("OAuth token endpoint is temporarily unavailable"))?;
        if out.len().saturating_add(chunk.len()) > limit {
            return Err(unavailable("OAuth token answer is malformed"));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Native (historic) token HTTP — preserved as the default engine.
pub async fn native_token_request(
    http: &reqwest::Client,
    ctx: &TokenEndpointContext,
    client_secret: Option<&SecretValue>,
    mut form: Vec<(&'static str, String)>,
) -> Result<ProtocolTokenAnswer> {
    let mut request = http.post(&ctx.token_url);
    match ctx.client_authentication {
        OAuthClientAuthentication::None => {
            form.push(("client_id", ctx.client_id.clone()));
        }
        OAuthClientAuthentication::ClientSecretPost => {
            let secret =
                client_secret.ok_or_else(|| unavailable("OAuth client secret is unavailable"))?;
            form.push(("client_id", ctx.client_id.clone()));
            form.push(("client_secret", secret.expose().into()));
        }
        OAuthClientAuthentication::ClientSecretBasic => {
            let secret =
                client_secret.ok_or_else(|| unavailable("OAuth client secret is unavailable"))?;
            request = request.basic_auth(&ctx.client_id, Some(secret.expose()));
        }
    }
    let response = request.form(&form).send().await;
    for (_, value) in &mut form {
        value.zeroize();
    }
    let response = response.map_err(|e| {
        unavailable(if e.is_timeout() {
            "OAuth token endpoint timed out"
        } else if e.is_connect() {
            "OAuth token endpoint is unreachable"
        } else {
            "OAuth token exchange failed"
        })
    })?;
    let status = response.status();
    if !status.is_success() {
        let invalid_grant = if status == reqwest::StatusCode::BAD_REQUEST {
            bounded_response_bytes(response, MAX_OAUTH_RESPONSE_BYTES)
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice::<OAuthErrorAnswer>(&bytes).ok())
                .is_some_and(|answer| answer.error == "invalid_grant")
        } else {
            false
        };
        return Err(unavailable(if invalid_grant {
            "OAuth token endpoint refused the grant"
        } else {
            "OAuth token endpoint is temporarily unavailable"
        }));
    }
    let bytes = bounded_response_bytes(response, MAX_OAUTH_RESPONSE_BYTES).await?;
    let answer: ProtocolTokenAnswer = serde_json::from_slice(&bytes)
        .map_err(|_| unavailable("OAuth token answer is malformed"))?;
    validate_token_answer(answer)
}

#[derive(Deserialize)]
struct OAuthErrorAnswer {
    error: String,
}

/// reqwest client that refuses redirects and caps body size before oauth2 parses.
#[cfg(feature = "olr-oauth-libs")]
struct BoundedAsyncHttpClient {
    inner: reqwest::Client,
}

#[cfg(feature = "olr-oauth-libs")]
impl BoundedAsyncHttpClient {
    fn new() -> Result<Self> {
        let inner = reqwest::Client::builder()
            .timeout(TOKEN_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable("cannot build OAuth HTTP client"))?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "olr-oauth-libs")]
impl<'c> oauth2::AsyncHttpClient<'c> for BoundedAsyncHttpClient {
    type Error = oauth2::HttpClientError<reqwest::Error>;
    type Future = Pin<
        Box<
            dyn Future<Output = std::result::Result<oauth2::HttpResponse, Self::Error>>
                + Send
                + Sync
                + 'c,
        >,
    >;

    fn call(&'c self, request: oauth2::HttpRequest) -> Self::Future {
        Box::pin(async move {
            let response = self
                .inner
                .execute(request.try_into().map_err(Box::new)?)
                .await
                .map_err(Box::new)?;
            let status = response.status();
            let headers = response.headers().clone();
            let mut body = Vec::new();
            let mut stream = response.bytes_stream();
            use futures::StreamExt;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(Box::new)?;
                if body.len().saturating_add(chunk.len()) > MAX_OAUTH_RESPONSE_BYTES {
                    return Err(oauth2::HttpClientError::Other(
                        "OAuth token answer exceeded size limit".into(),
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            let mut builder = oauth2::http::Response::builder().status(status);
            for (name, value) in headers.iter() {
                builder = builder.header(name, value);
            }
            builder.body(body).map_err(oauth2::HttpClientError::Http)
        })
    }
}

#[cfg(feature = "olr-oauth-libs")]
fn oauth2_auth_type(method: OAuthClientAuthentication) -> AuthType {
    match method {
        OAuthClientAuthentication::ClientSecretBasic => AuthType::BasicAuth,
        OAuthClientAuthentication::ClientSecretPost | OAuthClientAuthentication::None => {
            AuthType::RequestBody
        }
    }
}

#[cfg(feature = "olr-oauth-libs")]
fn map_oauth2_token_error<RE>(
    err: RequestTokenError<RE, oauth2::basic::BasicErrorResponse>,
) -> GatewayError
where
    RE: std::error::Error + 'static,
{
    match err {
        RequestTokenError::ServerResponse(response) => {
            if response.error() == &BasicErrorResponseType::InvalidGrant {
                unavailable("OAuth token endpoint refused the grant")
            } else {
                unavailable("OAuth token endpoint is temporarily unavailable")
            }
        }
        RequestTokenError::Request(e) => {
            let text = e.to_string();
            if text.contains("timed out") || text.contains("timeout") {
                unavailable("OAuth token endpoint timed out")
            } else if text.contains("size limit") {
                unavailable("OAuth token answer is malformed")
            } else if text.contains("connect") || text.contains("dns") {
                unavailable("OAuth token endpoint is unreachable")
            } else {
                unavailable("OAuth token exchange failed")
            }
        }
        RequestTokenError::Parse(_, _) => unavailable("OAuth token answer is malformed"),
        RequestTokenError::Other(_) => {
            unavailable("OAuth token endpoint is temporarily unavailable")
        }
    }
}

#[cfg(feature = "olr-oauth-libs")]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
struct AithosExtraTokenFields {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(flatten)]
    rest: BTreeMap<String, serde_json::Value>,
}

#[cfg(feature = "olr-oauth-libs")]
impl ExtraTokenFields for AithosExtraTokenFields {}

#[cfg(feature = "olr-oauth-libs")]
impl Drop for AithosExtraTokenFields {
    fn drop(&mut self) {
        if let Some(token) = &mut self.id_token {
            token.zeroize();
        }
        for value in self.rest.values_mut() {
            zeroize_json(value);
        }
    }
}

#[cfg(feature = "olr-oauth-libs")]
type AithosTokenResponse = StandardTokenResponse<AithosExtraTokenFields, BasicTokenType>;
#[cfg(feature = "olr-oauth-libs")]
type AithosOAuthClient = Client<
    BasicErrorResponse,
    AithosTokenResponse,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

#[cfg(feature = "olr-oauth-libs")]
fn token_type_label(token_type: &BasicTokenType) -> String {
    match token_type {
        BasicTokenType::Bearer => "Bearer".to_owned(),
        BasicTokenType::Mac => "mac".to_owned(),
        BasicTokenType::Extension(name) => name.clone(),
    }
}

#[cfg(feature = "olr-oauth-libs")]
fn aithos_token_to_answer(token: AithosTokenResponse) -> Result<ProtocolTokenAnswer> {
    let expires_in = token
        .expires_in()
        .map(|d| d.as_secs())
        .ok_or_else(|| unavailable("OAuth token answer is incomplete"))?;
    let extra = token.extra_fields();
    let mut additional = extra.rest.clone();
    if let Some(id_token) = &extra.id_token {
        additional.insert(
            "id_token".into(),
            serde_json::Value::String(id_token.clone()),
        );
    }
    validate_token_answer(ProtocolTokenAnswer {
        access_token: token.access_token().secret().to_owned(),
        refresh_token: token.refresh_token().map(|t| t.secret().to_owned()),
        expires_in,
        token_type: token_type_label(token.token_type()),
        scope: token.scopes().map(|scopes| {
            scopes
                .iter()
                .map(|scope| scope.as_ref().to_owned())
                .collect::<Vec<_>>()
                .join(" ")
        }),
        additional,
    })
}

#[cfg(feature = "olr-oauth-libs")]
fn build_oauth2_client(
    ctx: &TokenEndpointContext,
    client_secret: Option<&SecretValue>,
) -> Result<AithosOAuthClient> {
    let mut client = Client::<
        BasicErrorResponse,
        AithosTokenResponse,
        StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
        StandardRevocableToken,
        BasicRevocationErrorResponse,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
        EndpointNotSet,
    >::new(ClientId::new(ctx.client_id.clone()));
    if matches!(
        ctx.client_authentication,
        OAuthClientAuthentication::ClientSecretPost | OAuthClientAuthentication::ClientSecretBasic
    ) {
        let secret =
            client_secret.ok_or_else(|| unavailable("OAuth client secret is unavailable"))?;
        client = client.set_client_secret(ClientSecret::new(secret.expose().into()));
    }
    Ok(client
        .set_token_uri(
            TokenUrl::new(ctx.token_url.clone())
                .map_err(|_| unavailable("token URL is invalid"))?,
        )
        .set_redirect_uri(
            RedirectUrl::new(ctx.redirect_uri.clone())
                .map_err(|_| unavailable("redirect URI is invalid"))?,
        )
        .set_auth_type(oauth2_auth_type(ctx.client_authentication)))
}

/// Authorization-code exchange via the `oauth2` crate (OLR-2).
#[cfg(feature = "olr-oauth-libs")]
pub async fn oauth2_exchange_code(
    ctx: &TokenEndpointContext,
    client_secret: Option<&SecretValue>,
    code: &str,
    code_verifier: &str,
) -> Result<ProtocolTokenAnswer> {
    let client = build_oauth2_client(ctx, client_secret)?;
    let http = BoundedAsyncHttpClient::new()?;
    let token = client
        .exchange_code(AuthorizationCode::new(code.to_owned()))
        .set_pkce_verifier(PkceCodeVerifier::new(code_verifier.to_owned()))
        .request_async(&http)
        .await
        .map_err(map_oauth2_token_error)?;
    aithos_token_to_answer(token)
}

/// Refresh-token grant via the `oauth2` crate (OLR-2).
#[cfg(feature = "olr-oauth-libs")]
pub async fn oauth2_refresh(
    ctx: &TokenEndpointContext,
    client_secret: Option<&SecretValue>,
    refresh_token: &str,
) -> Result<ProtocolTokenAnswer> {
    let client = build_oauth2_client(ctx, client_secret)?;
    let http = BoundedAsyncHttpClient::new()?;
    let token = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_owned()))
        .request_async(&http)
        .await
        .map_err(map_oauth2_token_error)?;
    aithos_token_to_answer(token)
}

/// Build a PKCE S256 challenge from an existing verifier (Aithos entropy).
#[cfg(feature = "olr-oauth-libs")]
pub fn pkce_s256_challenge(verifier: &str) -> String {
    oauth2::PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
        verifier.to_owned(),
    ))
    .as_str()
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OAuthProtocolEngine;

    #[test]
    fn env_override_selects_oauth2() {
        std::env::set_var("AITHOS_UPSTREAM_OAUTH_ENGINE", "oauth2");
        assert_eq!(
            resolve_protocol_engine(OAuthProtocolEngine::Native),
            OAuthProtocolEngine::Oauth2
        );
        std::env::set_var("AITHOS_UPSTREAM_OAUTH_ENGINE", "native");
        assert_eq!(
            resolve_protocol_engine(OAuthProtocolEngine::Oauth2),
            OAuthProtocolEngine::Native
        );
        std::env::remove_var("AITHOS_UPSTREAM_OAUTH_ENGINE");
        assert_eq!(
            resolve_protocol_engine(OAuthProtocolEngine::Native),
            OAuthProtocolEngine::Native
        );
    }

    #[test]
    #[cfg(feature = "olr-oauth-libs")]
    fn pkce_challenge_is_s256_stable() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = pkce_s256_challenge(verifier);
        // RFC 7636 appendix B
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
}
