//! OAuth 2.1 client custody for protected upstream MCP servers.
//!
//! The public config contains URLs, client id, scopes and Vault references.
//! PKCE verifier, CSRF state, client secret, access token and refresh token
//! never cross the gateway boundary. The token record is resolved for every
//! call, refreshed under a per-client lock, and failures happen before any
//! upstream request is sent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::config::{
    GatewayConfig, OAuthAccessType, OAuthClientAuthentication, OAuthIdentitySource,
    OAuthProtocolEngine, OAuthRegistrationStrategy, UpstreamOAuthConfig,
};
use crate::core_bridge::{EntropySource, OsEntropy};
use crate::credentials::{
    CredentialBroker, CredentialCompareAndStoreOutcome, CredentialRef, SecretValue,
};
use crate::oauth::{b64url_encode, s256_challenge};
use crate::oauth_discovery::{OAuthDiscoveryClient, ResolvedOAuthEndpoints};
#[cfg(feature = "olr-oauth-libs")]
use crate::oauth_oidc::{self, OidcNoncePolicy, OidcValidationRequest};
use crate::oauth_protocol::{
    native_token_request, resolve_protocol_engine, ProtocolTokenAnswer, TokenEndpointContext,
};
#[cfg(feature = "olr-oauth-libs")]
use crate::oauth_protocol::{oauth2_exchange_code, oauth2_refresh};
use crate::oauth_registration::{
    resolve_registration_secret, ClientCredentialSource, OAuthRegistrationClient,
    ResolvedClientRegistration,
};
use crate::oauth_rollout::ROLLOUT_METRICS;
use crate::{GatewayError, Result};

const TOKEN_TIMEOUT: Duration = Duration::from_secs(10);
const EXPIRY_SKEW_SECS: i64 = 30;
const PENDING_TTL_SECS: i64 = 600;
/// Longer than the token endpoint timeout. A crashed claimant can be
/// recovered after this lease, while two live gateway instances never send
/// the same refresh token concurrently.
const REFRESH_LEASE_SECS: i64 = 30;
const MAX_OAUTH_RESPONSE_BYTES: usize = 64 * 1024;

pub type OAuthClock = Arc<dyn Fn() -> i64 + Send + Sync>;

fn system_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

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

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum VaultRecord {
    Pending {
        state: String,
        code_verifier: String,
        created_at: i64,
        /// OIDC nonce when account binding uses an ID Token (OLR-3).
        #[serde(default)]
        nonce: Option<String>,
    },
    Connected {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
        scopes: Vec<String>,
        #[serde(default)]
        issuer: Option<String>,
        #[serde(default)]
        subject: Option<String>,
        #[serde(default)]
        account: Option<String>,
    },
    /// A CAS-owned refresh lease. The previous token set remains encrypted in
    /// Vault for deterministic recovery, but no caller may use it while this
    /// state is present.
    Refreshing {
        started_at: i64,
        access_token: String,
        refresh_token: String,
        expires_at: i64,
        scopes: Vec<String>,
        #[serde(default)]
        issuer: Option<String>,
        #[serde(default)]
        subject: Option<String>,
        #[serde(default)]
        account: Option<String>,
    },
    ReauthRequired {
        changed_at: i64,
    },
    /// A distinct pending record cannot be deleted through the current
    /// broker seam. Replacing it with this non-secret tombstone prevents
    /// replay without inventing secret deletion-by-overwrite.
    Consumed {
        consumed_at: i64,
    },
}

impl Drop for VaultRecord {
    fn drop(&mut self) {
        match self {
            Self::Pending {
                state,
                code_verifier,
                nonce,
                ..
            } => {
                state.zeroize();
                code_verifier.zeroize();
                nonce.zeroize();
            }
            Self::Connected {
                access_token,
                refresh_token,
                scopes,
                issuer,
                subject,
                account,
                ..
            }
            | Self::Refreshing {
                access_token,
                refresh_token,
                scopes,
                issuer,
                subject,
                account,
                ..
            } => {
                access_token.zeroize();
                refresh_token.zeroize();
                scopes.zeroize();
                issuer.zeroize();
                subject.zeroize();
                account.zeroize();
            }
            Self::Consumed { .. } | Self::ReauthRequired { .. } => {}
        }
    }
}

pub struct ConsentStart {
    pub authorization_url: String,
    pub expires_at: i64,
    state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentIntent {
    Initial,
    Reconnect,
    Repair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamOAuthState {
    Pending { expires_at: i64 },
    Connected,
    Expired,
    ReauthRequired,
    Unavailable,
}

#[derive(Debug, Clone, Copy)]
enum IdentityNoncePolicy<'a> {
    AuthorizationCode(&'a str),
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisconnectOutcome {
    pub revocation_clean: bool,
    pub vault_cleanup_clean: bool,
}

/// One configured OAuth client. Clone/share through `Arc`; the refresh lock
/// prevents two simultaneous expired calls from rotating the same token set.
pub struct UpstreamOAuthClient {
    config: UpstreamOAuthConfig,
    client_secret_broker: Option<Arc<dyn CredentialBroker>>,
    registration_broker: Option<Arc<dyn CredentialBroker>>,
    token_broker: Arc<dyn CredentialBroker>,
    http: reqwest::Client,
    resolved: tokio::sync::RwLock<Option<(ResolvedOAuthEndpoints, ResolvedClientRegistration)>>,
    entropy: Mutex<Box<dyn EntropySource + Send>>,
    /// Serializes every mutation of pending/token/revocation custody. This is
    /// deliberately wider than refresh: callback replay, consent restart and
    /// disconnect must not race a refresh into resurrecting authority.
    lifecycle_lock: tokio::sync::Mutex<()>,
    clock: OAuthClock,
}

impl UpstreamOAuthClient {
    pub fn new(
        config: UpstreamOAuthConfig,
        client_secret_broker: Option<Arc<dyn CredentialBroker>>,
        registration_broker: Option<Arc<dyn CredentialBroker>>,
        token_broker: Arc<dyn CredentialBroker>,
        entropy: Box<dyn EntropySource + Send>,
        clock: OAuthClock,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(TOKEN_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable("cannot build OAuth HTTP client"))?;
        Ok(Self {
            config,
            client_secret_broker,
            registration_broker,
            token_broker,
            http,
            resolved: tokio::sync::RwLock::new(None),
            entropy: Mutex::new(entropy),
            lifecycle_lock: tokio::sync::Mutex::new(()),
            clock,
        })
    }

    pub fn config(&self) -> &UpstreamOAuthConfig {
        &self.config
    }

    fn pending_vault(&self) -> &CredentialRef {
        self.config
            .pending_vault
            .as_ref()
            .unwrap_or(&self.config.token_vault)
    }

    async fn read_record_with_encoding(
        &self,
        reference: &CredentialRef,
    ) -> Result<(VaultRecord, SecretValue)> {
        let value = self
            .token_broker
            .resolve(reference)
            .await
            .map_err(|_| unavailable("OAuth token record is unavailable"))?;
        let record = serde_json::from_str(value.expose())
            .map_err(|_| unavailable("OAuth token record is malformed"))?;
        Ok((record, value))
    }

    async fn read_record(&self, reference: &CredentialRef) -> Result<VaultRecord> {
        self.read_record_with_encoding(reference)
            .await
            .map(|(record, _encoding)| record)
    }

    async fn write_record(&self, reference: &CredentialRef, record: &VaultRecord) -> Result<()> {
        let encoded = serde_json::to_string(record)
            .map_err(|_| unavailable("cannot encode OAuth token record"))?;
        self.token_broker
            .store(reference, SecretValue::new(encoded))
            .await
            .map_err(|_| unavailable("cannot persist OAuth token record"))
    }

    async fn consume_pending_record(&self, expected: SecretValue) -> Result<()> {
        let consumed = serde_json::to_string(&VaultRecord::Consumed {
            consumed_at: (self.clock)(),
        })
        .map_err(|_| unavailable("cannot encode OAuth consumed record"))?;
        match self
            .token_broker
            .compare_and_store(self.pending_vault(), expected, SecretValue::new(consumed))
            .await
            .map_err(|_| unavailable("cannot consume OAuth callback state"))?
        {
            CredentialCompareAndStoreOutcome::Stored => Ok(()),
            CredentialCompareAndStoreOutcome::Mismatch => {
                Err(unavailable("no OAuth consent is pending"))
            }
            CredentialCompareAndStoreOutcome::Unsupported => Err(unavailable(
                "OAuth token Vault does not support atomic callback state",
            )),
        }
    }

    async fn replace_token_record(
        &self,
        expected: &VaultRecord,
        replacement: &VaultRecord,
        mismatch: &'static str,
    ) -> Result<()> {
        let expected = serde_json::to_string(expected)
            .map_err(|_| unavailable("cannot encode OAuth token state"))?;
        self.replace_encoded_token_record(SecretValue::new(expected), replacement, mismatch)
            .await
    }

    async fn replace_encoded_token_record(
        &self,
        expected: SecretValue,
        replacement: &VaultRecord,
        mismatch: &'static str,
    ) -> Result<()> {
        let replacement = serde_json::to_string(replacement)
            .map_err(|_| unavailable("cannot encode OAuth token state"))?;
        match self
            .token_broker
            .compare_and_store(
                &self.config.token_vault,
                expected,
                SecretValue::new(replacement),
            )
            .await
            .map_err(|_| unavailable("cannot update OAuth token state atomically"))?
        {
            CredentialCompareAndStoreOutcome::Stored => Ok(()),
            CredentialCompareAndStoreOutcome::Mismatch => Err(unavailable(mismatch)),
            CredentialCompareAndStoreOutcome::Unsupported => Err(unavailable(
                "OAuth token Vault does not support atomic refresh state",
            )),
        }
    }

    async fn claim_refresh(
        &self,
        observed: &VaultRecord,
        observed_encoding: SecretValue,
    ) -> Result<(VaultRecord, VaultRecord)> {
        let now = (self.clock)();
        let previous = match observed {
            VaultRecord::Connected {
                access_token,
                refresh_token,
                expires_at,
                scopes,
                issuer,
                subject,
                account,
            }
            | VaultRecord::Refreshing {
                access_token,
                refresh_token,
                expires_at,
                scopes,
                issuer,
                subject,
                account,
                ..
            } => VaultRecord::Connected {
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                expires_at: *expires_at,
                scopes: scopes.clone(),
                issuer: issuer.clone(),
                subject: subject.clone(),
                account: account.clone(),
            },
            VaultRecord::Pending { .. } | VaultRecord::Consumed { .. } => {
                return Err(unavailable("OAuth consent is not complete"));
            }
            VaultRecord::ReauthRequired { .. } => {
                return Err(unavailable("OAuth consent must be renewed"));
            }
        };
        if let VaultRecord::Refreshing { started_at, .. } = observed {
            let age = now.saturating_sub(*started_at);
            if age <= REFRESH_LEASE_SECS {
                return Err(unavailable("OAuth token refresh is already in progress"));
            }
        }
        let claimed = match &previous {
            VaultRecord::Connected {
                access_token,
                refresh_token,
                expires_at,
                scopes,
                issuer,
                subject,
                account,
            } => VaultRecord::Refreshing {
                started_at: now,
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                expires_at: *expires_at,
                scopes: scopes.clone(),
                issuer: issuer.clone(),
                subject: subject.clone(),
                account: account.clone(),
            },
            VaultRecord::Pending { .. }
            | VaultRecord::Refreshing { .. }
            | VaultRecord::Consumed { .. }
            | VaultRecord::ReauthRequired { .. } => unreachable!(),
        };
        self.replace_encoded_token_record(
            observed_encoding,
            &claimed,
            "OAuth token refresh was claimed concurrently",
        )
        .await?;
        Ok((previous, claimed))
    }

    async fn write_revocation_marker(&self, reference: &CredentialRef) -> Result<()> {
        let encoded = serde_json::to_string(&serde_json::json!({
            "v": 1,
            "state": "pending",
            "started_at": (self.clock)(),
        }))
        .map_err(|_| unavailable("cannot encode OAuth revocation marker"))?;
        self.token_broker
            .store(reference, SecretValue::new(encoded))
            .await
            .map_err(|_| unavailable("cannot persist OAuth revocation marker"))
    }

    async fn resolve_runtime(
        &self,
    ) -> Result<(ResolvedOAuthEndpoints, ResolvedClientRegistration)> {
        if let Some(resolved) = self.resolved.read().await.clone() {
            return Ok(resolved);
        }
        let endpoints = OAuthDiscoveryClient::new()?.resolve(&self.config).await?;
        let registration = OAuthRegistrationClient::new()?
            .resolve(
                &self.config,
                &endpoints,
                self.registration_broker.as_ref(),
                (self.clock)(),
            )
            .await?;
        let resolved = (endpoints, registration);
        *self.resolved.write().await = Some(resolved.clone());
        Ok(resolved)
    }

    /// Persist a fresh PKCE verifier and state before returning the consent
    /// URL. Re-running deliberately invalidates the previous pending flow.
    pub async fn build_consent_url(&self) -> Result<ConsentStart> {
        self.build_consent_url_for(ConsentIntent::Initial).await
    }

    pub async fn build_consent_url_for(&self, intent: ConsentIntent) -> Result<ConsentStart> {
        let _lifecycle = self.lifecycle_lock.lock().await;
        let (endpoints, registration) = self.resolve_runtime().await?;
        let oidc_binding = matches!(
            self.config
                .account_binding
                .as_ref()
                .map(|binding| &binding.source),
            Some(OAuthIdentitySource::IdToken { .. })
        );
        let (verifier, random_state, nonce) = {
            let mut entropy = self
                .entropy
                .lock()
                .map_err(|_| unavailable("OAuth entropy lock is unavailable"))?;
            let verifier = b64url_encode(&entropy.e32());
            let random_state = b64url_encode(&entropy.e32());
            let nonce = oidc_binding.then(|| b64url_encode(&entropy.e32()));
            (verifier, random_state, nonce)
        };
        let state = format!(
            "{}.{}",
            callback_route_key(&self.config.token_vault),
            random_state
        );
        let challenge = s256_challenge(&verifier);
        let pending = VaultRecord::Pending {
            state,
            code_verifier: verifier,
            created_at: (self.clock)(),
            nonce: nonce.clone(),
        };
        self.write_record(self.pending_vault(), &pending).await?;
        let state = match &pending {
            VaultRecord::Pending { state, .. } => state,
            VaultRecord::Connected { .. }
            | VaultRecord::Refreshing { .. }
            | VaultRecord::Consumed { .. }
            | VaultRecord::ReauthRequired { .. } => unreachable!(),
        };
        let mut url = reqwest::Url::parse(&endpoints.authorization_endpoint)
            .map_err(|_| unavailable("authorization URL is invalid"))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &registration.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri);
        if !self.config.scopes.is_empty() {
            url.query_pairs_mut()
                .append_pair("scope", &self.config.scopes.join(" "));
        }
        url.query_pairs_mut()
            .append_pair("state", state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        if let Some(nonce) = &nonce {
            url.query_pairs_mut().append_pair("nonce", nonce);
        }
        if matches!(
            self.config.authorization_parameters.access_type,
            Some(OAuthAccessType::Offline)
        ) {
            url.query_pairs_mut().append_pair("access_type", "offline");
        }
        if self.config.authorization_parameters.include_granted_scopes {
            url.query_pairs_mut()
                .append_pair("include_granted_scopes", "true");
        }
        if self.config.authorization_parameters.prompt_consent
            || (intent == ConsentIntent::Repair
                && self
                    .config
                    .authorization_parameters
                    .prompt_consent_on_repair)
        {
            url.query_pairs_mut().append_pair("prompt", "consent");
        }
        if let Some(resource) = &self.config.resource {
            url.query_pairs_mut().append_pair("resource", resource);
        }
        if let Some(audience) = &self.config.audience {
            url.query_pairs_mut().append_pair("audience", audience);
        }
        Ok(ConsentStart {
            authorization_url: url.into(),
            expires_at: (self.clock)().saturating_add(PENDING_TTL_SECS),
            state: state.clone(),
        })
    }

    async fn token_request(
        &self,
        mut form: Vec<(&'static str, String)>,
    ) -> Result<ProtocolTokenAnswer> {
        let engine = resolve_protocol_engine(self.config.protocol_engine);
        let result = self.token_request_inner(&form).await;
        for (_, value) in &mut form {
            value.zeroize();
        }
        ROLLOUT_METRICS.record_token_outcome(engine, result.is_ok());
        result
    }

    async fn token_request_inner(
        &self,
        form: &[(&'static str, String)],
    ) -> Result<ProtocolTokenAnswer> {
        let (endpoints, registration) = self.resolve_runtime().await?;
        let ctx = TokenEndpointContext {
            token_url: endpoints.token_endpoint.clone(),
            client_id: registration.client_id.clone(),
            redirect_uri: self.config.redirect_uri.clone(),
            client_authentication: self.config.client_authentication,
        };
        let secret = match self.config.client_authentication {
            OAuthClientAuthentication::None => None,
            OAuthClientAuthentication::ClientSecretPost
            | OAuthClientAuthentication::ClientSecretBasic => {
                Some(self.resolve_client_secret(&registration.credential).await?)
            }
        };
        match resolve_protocol_engine(self.config.protocol_engine) {
            OAuthProtocolEngine::Native => {
                native_token_request(&self.http, &ctx, secret.as_ref(), form.to_vec()).await
            }
            OAuthProtocolEngine::Oauth2 => {
                #[cfg(not(feature = "olr-oauth-libs"))]
                {
                    Err(unavailable(
                        "OAuth2 protocol engine is unavailable in this build",
                    ))
                }
                #[cfg(feature = "olr-oauth-libs")]
                {
                    let grant = form
                        .iter()
                        .find(|(key, _)| *key == "grant_type")
                        .map(|(_, value)| value.as_str())
                        .unwrap_or_default();
                    match grant {
                        "authorization_code" => {
                            let code = form
                                .iter()
                                .find(|(key, _)| *key == "code")
                                .map(|(_, value)| value.as_str())
                                .ok_or_else(|| unavailable("OAuth callback is incomplete"))?;
                            let verifier = form
                                .iter()
                                .find(|(key, _)| *key == "code_verifier")
                                .map(|(_, value)| value.as_str())
                                .ok_or_else(|| unavailable("OAuth callback is incomplete"))?;
                            oauth2_exchange_code(&ctx, secret.as_ref(), code, verifier).await
                        }
                        "refresh_token" => {
                            let refresh = form
                                .iter()
                                .find(|(key, _)| *key == "refresh_token")
                                .map(|(_, value)| value.as_str())
                                .ok_or_else(|| unavailable("OAuth consent must be renewed"))?;
                            oauth2_refresh(&ctx, secret.as_ref(), refresh).await
                        }
                        _ => Err(unavailable("OAuth token exchange failed")),
                    }
                }
            }
        }
    }

    async fn resolve_client_secret(&self, source: &ClientCredentialSource) -> Result<SecretValue> {
        match source {
            ClientCredentialSource::Static(reference) => self
                .client_secret_broker
                .as_ref()
                .ok_or_else(|| unavailable("OAuth client secret broker is unavailable"))?
                .resolve(reference)
                .await
                .map_err(|_| unavailable("OAuth client secret is unavailable")),
            ClientCredentialSource::Registration(reference) => {
                let broker = self
                    .registration_broker
                    .as_ref()
                    .ok_or_else(|| unavailable("OAuth registration Vault is unavailable"))?;
                resolve_registration_secret(broker, reference).await
            }
            ClientCredentialSource::None => {
                Err(unavailable("OAuth client authentication has no secret"))
            }
        }
    }

    fn scopes_from_answer(
        &self,
        answer: &ProtocolTokenAnswer,
        fallback: &[String],
    ) -> Result<Vec<String>> {
        let scopes: Vec<String> = answer.scope.as_deref().map_or_else(
            || fallback.to_vec(),
            |scope| scope.split_whitespace().map(str::to_owned).collect(),
        );
        let expected = self
            .config
            .scopes
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let observed = scopes.iter().collect::<std::collections::BTreeSet<_>>();
        if observed != expected {
            return Err(unavailable("OAuth token answer changed approved scopes"));
        }
        Ok(scopes)
    }

    async fn identity_values(
        &self,
        answer: &ProtocolTokenAnswer,
        oidc_nonce: Option<IdentityNoncePolicy<'_>>,
    ) -> Result<Option<BTreeMap<String, serde_json::Value>>> {
        let Some(binding) = &self.config.account_binding else {
            return Ok(None);
        };
        match &binding.source {
            OAuthIdentitySource::TokenResponse => Ok(Some(answer.additional.clone())),
            OAuthIdentitySource::UserInfo { endpoint } => {
                let response = self
                    .http
                    .get(endpoint)
                    .bearer_auth(&answer.access_token)
                    .send()
                    .await
                    .map_err(|_| unavailable("OAuth identity endpoint is unavailable"))?;
                if !response.status().is_success() || response.status().is_redirection() {
                    return Err(unavailable("OAuth identity endpoint refused the token"));
                }
                let bytes = bounded_response_bytes(response, MAX_OAUTH_RESPONSE_BYTES).await?;
                serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|_| unavailable("OAuth identity response is malformed"))
            }
            OAuthIdentitySource::IdToken { audience, .. } => {
                #[cfg(not(feature = "olr-oauth-libs"))]
                {
                    if let Some(IdentityNoncePolicy::AuthorizationCode(nonce)) = oidc_nonce {
                        let _ = nonce;
                    }
                    let _ = (answer, audience);
                    Err(unavailable("OIDC validation is unavailable in this build"))
                }
                #[cfg(feature = "olr-oauth-libs")]
                {
                    let id_token = answer
                        .additional
                        .get("id_token")
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| unavailable("OAuth token answer has no ID token"))?;
                    let (endpoints, registration) = self.resolve_runtime().await?;
                    let jwks_uri = endpoints
                        .jwks_uri
                        .as_deref()
                        .ok_or_else(|| unavailable("OIDC JWKS endpoint is unavailable"))?;
                    let audience = audience
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .unwrap_or(registration.client_id.as_str());
                    let validated = oauth_oidc::validate_id_token(OidcValidationRequest {
                        id_token,
                        issuer: binding.issuer.as_str(),
                        audience,
                        jwks_uri,
                        nonce: match oidc_nonce
                            .ok_or_else(|| unavailable("OIDC nonce policy is missing"))?
                        {
                            IdentityNoncePolicy::AuthorizationCode(nonce) => {
                                OidcNoncePolicy::AuthorizationCode(nonce)
                            }
                            IdentityNoncePolicy::Refresh => OidcNoncePolicy::Refresh,
                        },
                    })
                    .await;
                    let ok = validated.is_ok();
                    ROLLOUT_METRICS.record_oidc_outcome(ok);
                    validated.map(Some)
                }
            }
        }
    }

    async fn identity_from_answer(
        &self,
        answer: &ProtocolTokenAnswer,
        oidc_nonce: Option<&str>,
    ) -> Result<Option<(String, String, String)>> {
        let Some(binding) = &self.config.account_binding else {
            return Ok(None);
        };
        let mut values = self
            .identity_values(
                answer,
                oidc_nonce.map(IdentityNoncePolicy::AuthorizationCode),
            )
            .await?
            .ok_or_else(|| unavailable("OAuth identity response is absent"))?;
        let subject = values
            .get(&binding.subject_field)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unavailable("OAuth token answer has no verified subject"))?
            .to_owned();
        let account = values
            .get(&binding.account_field)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unavailable("OAuth token answer has no verified account"))?
            .to_owned();
        for value in values.values_mut() {
            zeroize_json(value);
        }
        Ok(Some((binding.issuer.clone(), subject, account)))
    }

    async fn identity_from_refresh(
        &self,
        answer: &ProtocolTokenAnswer,
        issuer: &Option<String>,
        subject: &Option<String>,
        account: &Option<String>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        let Some(binding) = &self.config.account_binding else {
            return Ok((issuer.clone(), subject.clone(), account.clone()));
        };
        // Refresh responses often omit id_token; keep the bound identity when
        // the IdToken source yields no fresh claims.
        if matches!(binding.source, OAuthIdentitySource::IdToken { .. })
            && answer
                .additional
                .get("id_token")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Ok((issuer.clone(), subject.clone(), account.clone()));
        }
        let mut values = self
            .identity_values(answer, Some(IdentityNoncePolicy::Refresh))
            .await?
            .unwrap_or_default();
        let next_subject = values
            .get(&binding.subject_field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let next_account = values
            .get(&binding.account_field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let response_omitted_identity = matches!(
            binding.source,
            OAuthIdentitySource::TokenResponse | OAuthIdentitySource::IdToken { .. }
        ) && next_subject.is_none()
            && next_account.is_none();
        for value in values.values_mut() {
            zeroize_json(value);
        }
        if response_omitted_identity {
            return Ok((issuer.clone(), subject.clone(), account.clone()));
        }
        if issuer.as_deref() != Some(binding.issuer.as_str())
            || next_subject.as_deref() != subject.as_deref()
            || next_account.as_deref() != account.as_deref()
        {
            return Err(unavailable("OAuth refresh changed the bound account"));
        }
        Ok((issuer.clone(), subject.clone(), account.clone()))
    }

    /// Exchange one callback only when its CSRF state matches the pending
    /// Vault record. The code and verifier are never returned or logged.
    pub async fn exchange_callback(&self, state: &str, code: &str) -> Result<()> {
        let _lifecycle = self.lifecycle_lock.lock().await;
        if state.is_empty() || code.is_empty() {
            return Err(unavailable("OAuth callback is incomplete"));
        }
        if self.config.pending_vault.is_some()
            && matches!(
                self.read_record(&self.config.token_vault).await,
                Ok(VaultRecord::Connected { .. } | VaultRecord::Refreshing { .. })
            )
        {
            return Err(unavailable("no OAuth consent is pending"));
        }
        let (record, record_encoding) =
            self.read_record_with_encoding(self.pending_vault()).await?;
        let (expected_state, verifier, created_at, oidc_nonce) = match &record {
            VaultRecord::Pending {
                state,
                code_verifier,
                created_at,
                nonce,
            } => (state, code_verifier, *created_at, nonce.clone()),
            VaultRecord::Connected { .. } => {
                return Err(unavailable("no OAuth consent is pending"));
            }
            VaultRecord::Refreshing { .. } => {
                return Err(unavailable("no OAuth consent is pending"));
            }
            VaultRecord::Consumed { .. } => {
                return Err(unavailable("no OAuth consent is pending"));
            }
            VaultRecord::ReauthRequired { .. } => {
                return Err(unavailable("no OAuth consent is pending"));
            }
        };
        if expected_state != state {
            return Err(unavailable("OAuth callback state mismatch"));
        }
        let age = (self.clock)().saturating_sub(created_at);
        if !(0..=PENDING_TTL_SECS).contains(&age) {
            return Err(unavailable("OAuth consent has expired"));
        }
        // Consume the state durably before contacting the token endpoint.
        // Vault CAS makes this one-shot across gateway processes, not merely
        // across tasks sharing this client's lifecycle mutex.
        self.consume_pending_record(record_encoding).await?;
        let answer = self
            .token_request(vec![
                ("grant_type", "authorization_code".into()),
                ("code", code.into()),
                ("redirect_uri", self.config.redirect_uri.clone()),
                ("code_verifier", verifier.clone()),
            ])
            .await?;
        let refresh_token = answer
            .refresh_token
            .as_ref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unavailable("OAuth token answer has no refresh token"))?
            .clone();
        let scopes = self.scopes_from_answer(&answer, &self.config.scopes)?;
        let identity = self
            .identity_from_answer(&answer, oidc_nonce.as_deref())
            .await?;
        let connected = VaultRecord::Connected {
            access_token: answer.access_token.clone(),
            refresh_token,
            expires_at: (self.clock)().saturating_add(answer.expires_in as i64),
            scopes,
            issuer: identity.as_ref().map(|value| value.0.clone()),
            subject: identity.as_ref().map(|value| value.1.clone()),
            account: identity.map(|value| value.2),
        };
        self.write_record(&self.config.token_vault, &connected)
            .await?;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        matches!(
            self.read_record(&self.config.token_vault).await,
            Ok(VaultRecord::Connected { .. } | VaultRecord::Refreshing { .. })
        )
    }

    pub async fn public_state(&self) -> UpstreamOAuthState {
        match self.read_record(&self.config.token_vault).await {
            Ok(VaultRecord::Connected { expires_at, .. }) => {
                return if expires_at > (self.clock)().saturating_add(EXPIRY_SKEW_SECS) {
                    UpstreamOAuthState::Connected
                } else {
                    UpstreamOAuthState::Expired
                }
            }
            Ok(VaultRecord::Refreshing { .. }) => return UpstreamOAuthState::Expired,
            Ok(VaultRecord::Pending { created_at, .. }) if self.config.pending_vault.is_none() => {
                let expires_at = created_at.saturating_add(PENDING_TTL_SECS);
                return if (self.clock)() <= expires_at {
                    UpstreamOAuthState::Pending { expires_at }
                } else {
                    UpstreamOAuthState::Expired
                };
            }
            Ok(VaultRecord::ReauthRequired { .. }) => return UpstreamOAuthState::ReauthRequired,
            Ok(VaultRecord::Consumed { .. }) | Ok(VaultRecord::Pending { .. }) | Err(_) => {}
        }
        match self.read_record(self.pending_vault()).await {
            Ok(VaultRecord::Pending { created_at, .. }) => {
                let expires_at = created_at.saturating_add(PENDING_TTL_SECS);
                if (self.clock)() <= expires_at {
                    UpstreamOAuthState::Pending { expires_at }
                } else {
                    UpstreamOAuthState::Expired
                }
            }
            _ => UpstreamOAuthState::Unavailable,
        }
    }

    /// Resolve a usable access token at the last possible moment. Expiry or
    /// any malformed/missing record refreshes or refuses before the resource
    /// request builder can be sent.
    pub async fn access_token(&self) -> Result<SecretValue> {
        let _lifecycle = self.lifecycle_lock.lock().await;
        let (record, record_encoding) = self
            .read_record_with_encoding(&self.config.token_vault)
            .await?;
        if let VaultRecord::Connected {
            access_token,
            expires_at,
            ..
        } = &record
        {
            if *expires_at > (self.clock)().saturating_add(EXPIRY_SKEW_SECS) {
                return Ok(SecretValue::new(access_token.clone()));
            }
        }
        let (previous, claimed) = self.claim_refresh(&record, record_encoding).await?;
        let VaultRecord::Connected {
            refresh_token,
            scopes,
            issuer,
            subject,
            account,
            ..
        } = &previous
        else {
            unreachable!()
        };

        let answer = match self
            .token_request(vec![
                ("grant_type", "refresh_token".into()),
                ("refresh_token", refresh_token.clone()),
            ])
            .await
        {
            Ok(answer) => answer,
            Err(error) => {
                let invalid_grant = matches!(
                    &error,
                    GatewayError::UpstreamOauthUnavailable(reason)
                        if reason == "OAuth token endpoint refused the grant"
                );
                let replacement = if invalid_grant {
                    VaultRecord::ReauthRequired {
                        changed_at: (self.clock)(),
                    }
                } else {
                    previous.clone()
                };
                self.replace_token_record(
                    &claimed,
                    &replacement,
                    "OAuth token refresh state changed before failure recovery",
                )
                .await?;
                return Err(error);
            }
        };
        let next_refresh = answer
            .refresh_token
            .as_ref()
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| refresh_token.clone());
        let next_access = answer.access_token.clone();
        let next_scopes = self.scopes_from_answer(&answer, scopes);
        let next_identity = self
            .identity_from_refresh(&answer, issuer, subject, account)
            .await;
        let (next_scopes, next_identity) = match (next_scopes, next_identity) {
            (Ok(scopes), Ok(identity)) => (scopes, identity),
            (Err(error), _) | (_, Err(error)) => {
                self.replace_token_record(
                    &claimed,
                    &VaultRecord::ReauthRequired {
                        changed_at: (self.clock)(),
                    },
                    "OAuth token refresh state changed before refusal",
                )
                .await?;
                return Err(error);
            }
        };
        let connected = VaultRecord::Connected {
            access_token: next_access.clone(),
            refresh_token: next_refresh,
            expires_at: (self.clock)().saturating_add(answer.expires_in as i64),
            scopes: next_scopes,
            issuer: next_identity.0,
            subject: next_identity.1,
            account: next_identity.2,
        };
        self.replace_token_record(
            &claimed,
            &connected,
            "OAuth token refresh state changed before commit",
        )
        .await?;
        Ok(SecretValue::new(next_access))
    }

    /// Revoke best-effort and remove every account-specific OAuth record.
    /// Callers must remove runtime authority before entering this method.
    pub async fn disconnect(&self) -> DisconnectOutcome {
        let _lifecycle = self.lifecycle_lock.lock().await;
        let record = self.read_record(&self.config.token_vault).await.ok();
        let mut revocation_clean = true;
        let mut vault_cleanup_clean = true;
        if let Some(VaultRecord::Connected { refresh_token, .. }) = &record {
            if let Some(marker) = &self.config.revocation_vault {
                if self.write_revocation_marker(marker).await.is_err() {
                    vault_cleanup_clean = false;
                }
            }
            revocation_clean = self.revoke(refresh_token).await.is_ok();
        }

        let mut records = Vec::new();
        if let Some(pending) = &self.config.pending_vault {
            if pending != &self.config.token_vault {
                records.push((&self.token_broker, pending));
            }
        }
        if revocation_clean {
            records.push((&self.token_broker, &self.config.token_vault));
            if let Some(marker) = &self.config.revocation_vault {
                records.push((&self.token_broker, marker));
            }
        } else {
            // Preserve the token and dynamic registration needed to retry
            // provider revocation; runtime authority is already removed.
            vault_cleanup_clean = false;
        }
        for (broker, reference) in records {
            match broker.delete(reference).await {
                Ok(crate::credentials::CredentialDeleteOutcome::Deleted) => {}
                Ok(crate::credentials::CredentialDeleteOutcome::Unsupported) | Err(_) => {
                    vault_cleanup_clean = false
                }
            }
        }
        if revocation_clean {
            if let OAuthRegistrationStrategy::Dynamic { vault, .. } = &self.config.registration {
                match &self.registration_broker {
                    Some(broker) => match broker.delete(vault).await {
                        Ok(crate::credentials::CredentialDeleteOutcome::Deleted) => {}
                        Ok(crate::credentials::CredentialDeleteOutcome::Unsupported) | Err(_) => {
                            vault_cleanup_clean = false
                        }
                    },
                    None => vault_cleanup_clean = false,
                }
            }
        }
        *self.resolved.write().await = None;
        DisconnectOutcome {
            revocation_clean,
            vault_cleanup_clean,
        }
    }

    async fn revoke(&self, refresh_token: &str) -> Result<()> {
        let (endpoints, registration) = self.resolve_runtime().await?;
        let Some(endpoint) = endpoints.revocation_endpoint else {
            return Ok(());
        };
        let mut form = vec![
            ("token", refresh_token.to_owned()),
            ("token_type_hint", "refresh_token".to_owned()),
        ];
        let mut request = self.http.post(endpoint);
        let mut secret = None;
        match self.config.client_authentication {
            OAuthClientAuthentication::None => {
                form.push(("client_id", registration.client_id));
            }
            OAuthClientAuthentication::ClientSecretPost => {
                secret = Some(self.resolve_client_secret(&registration.credential).await?);
                form.push(("client_id", registration.client_id));
                form.push((
                    "client_secret",
                    secret
                        .as_ref()
                        .expect("secret was assigned")
                        .expose()
                        .into(),
                ));
            }
            OAuthClientAuthentication::ClientSecretBasic => {
                secret = Some(self.resolve_client_secret(&registration.credential).await?);
                request = request.basic_auth(
                    registration.client_id,
                    Some(secret.as_ref().expect("secret was assigned").expose()),
                );
            }
        }
        let response = request.form(&form).send().await;
        for (_, value) in &mut form {
            value.zeroize();
        }
        drop(secret);
        let response = response.map_err(|_| unavailable("OAuth revocation request failed"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(unavailable("OAuth revocation endpoint refused the token"))
        }
    }
}

async fn bounded_response_bytes(
    mut response: reqwest::Response,
    maximum: usize,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(unavailable("OAuth response is too large"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| unavailable("OAuth response body is unavailable"))?
    {
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err(unavailable("OAuth response is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// All configured upstream OAuth clients, shared by callback routing, owner
/// onboarding and the HTTP upstream authorization seam.
pub struct UpstreamOAuthRegistry {
    clients: RwLock<BTreeMap<String, Arc<UpstreamOAuthClient>>>,
    callback_routes: RwLock<BTreeMap<String, String>>,
    pending_routes: RwLock<BTreeMap<String, String>>,
}

impl Default for UpstreamOAuthRegistry {
    fn default() -> Self {
        Self {
            clients: RwLock::new(BTreeMap::new()),
            callback_routes: RwLock::new(BTreeMap::new()),
            pending_routes: RwLock::new(BTreeMap::new()),
        }
    }
}

impl UpstreamOAuthRegistry {
    pub fn from_config(
        cfg: &GatewayConfig,
        brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
    ) -> Result<Self> {
        let mut clients = BTreeMap::new();
        let mut callback_routes = BTreeMap::new();
        for server in cfg.servers.as_deref().unwrap_or_default() {
            let Some(oauth) = &server.oauth else {
                continue;
            };
            let secret_broker = oauth
                .client_secret
                .as_ref()
                .map(|reference| broker_for(brokers, reference))
                .transpose()?;
            let token_broker = broker_for(brokers, &oauth.token_vault)?;
            let registration_broker = match &oauth.registration {
                OAuthRegistrationStrategy::Dynamic { vault, .. } => {
                    Some(broker_for(brokers, vault)?)
                }
                OAuthRegistrationStrategy::Static
                | OAuthRegistrationStrategy::ClientMetadataDocument { .. } => None,
            };
            clients.insert(
                server.name.clone(),
                Arc::new(UpstreamOAuthClient::new(
                    oauth.clone(),
                    secret_broker,
                    registration_broker,
                    token_broker,
                    Box::new(OsEntropy),
                    Arc::new(system_epoch),
                )?),
            );
            if callback_routes
                .insert(callback_route_key(&oauth.token_vault), server.name.clone())
                .is_some()
            {
                return Err(unavailable("OAuth callback routes are not isolated"));
            }
        }
        Ok(Self {
            clients: RwLock::new(clients),
            callback_routes: RwLock::new(callback_routes),
            pending_routes: RwLock::new(BTreeMap::new()),
        })
    }

    pub fn get(&self, server: &str) -> Option<Arc<UpstreamOAuthClient>> {
        self.clients.read().ok()?.get(server).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.clients
            .read()
            .map_or(true, |clients| clients.is_empty())
    }

    pub fn server_names(&self) -> Vec<String> {
        self.clients
            .read()
            .map_or_else(|_| Vec::new(), |clients| clients.keys().cloned().collect())
    }

    pub fn upsert(
        &self,
        server: &str,
        config: UpstreamOAuthConfig,
        brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
    ) -> Result<Arc<UpstreamOAuthClient>> {
        let route = callback_route_key(&config.token_vault);
        {
            let routes = self
                .callback_routes
                .read()
                .map_err(|_| unavailable("OAuth callback router is unavailable"))?;
            if routes
                .get(&route)
                .is_some_and(|routed_server| routed_server != server)
            {
                return Err(unavailable("OAuth callback routes are not isolated"));
            }
        }
        let secret_broker = config
            .client_secret
            .as_ref()
            .map(|reference| broker_for(brokers, reference))
            .transpose()?;
        let token_broker = broker_for(brokers, &config.token_vault)?;
        let registration_broker = match &config.registration {
            OAuthRegistrationStrategy::Dynamic { vault, .. } => Some(broker_for(brokers, vault)?),
            OAuthRegistrationStrategy::Static
            | OAuthRegistrationStrategy::ClientMetadataDocument { .. } => None,
        };
        let client = Arc::new(UpstreamOAuthClient::new(
            config,
            secret_broker,
            registration_broker,
            token_broker,
            Box::new(OsEntropy),
            Arc::new(system_epoch),
        )?);
        if let Some(previous) = self.get(server) {
            let previous_route = callback_route_key(&previous.config().token_vault);
            if let Ok(mut routes) = self.callback_routes.write() {
                routes.remove(&previous_route);
            }
            if let Ok(mut pending) = self.pending_routes.write() {
                pending.retain(|_, routed_server| routed_server != server);
            }
        }
        self.clients
            .write()
            .map_err(|_| unavailable("OAuth registry is unavailable"))?
            .insert(server.to_owned(), Arc::clone(&client));
        self.callback_routes
            .write()
            .map_err(|_| unavailable("OAuth callback router is unavailable"))?
            .insert(route, server.to_owned());
        Ok(client)
    }

    pub fn remove(&self, server: &str) {
        let removed = if let Ok(mut clients) = self.clients.write() {
            clients.remove(server)
        } else {
            None
        };
        if let Some(client) = removed {
            let route = callback_route_key(&client.config().token_vault);
            if let Ok(mut routes) = self.callback_routes.write() {
                routes.remove(&route);
            }
        }
        if let Ok(mut routes) = self.pending_routes.write() {
            routes.retain(|_, routed_server| routed_server != server);
        }
    }

    pub async fn disconnect(&self, server: &str) -> Result<DisconnectOutcome> {
        let client = self
            .get(server)
            .ok_or_else(|| unavailable("unknown OAuth upstream server"))?;
        let outcome = client.disconnect().await;
        self.remove(server);
        Ok(outcome)
    }

    pub async fn disconnected_server_names(&self) -> std::collections::BTreeSet<String> {
        let mut disconnected = std::collections::BTreeSet::new();
        let clients = self.clients.read().map_or_else(
            |_| Vec::new(),
            |clients| {
                clients
                    .iter()
                    .map(|(name, client)| (name.clone(), Arc::clone(client)))
                    .collect()
            },
        );
        for (name, client) in clients {
            if !client.is_connected().await {
                disconnected.insert(name);
            }
        }
        disconnected
    }

    pub async fn start(&self, server: &str) -> Result<ConsentStart> {
        self.start_with_intent(server, ConsentIntent::Initial).await
    }

    pub async fn start_with_intent(
        &self,
        server: &str,
        intent: ConsentIntent,
    ) -> Result<ConsentStart> {
        let start = self
            .get(server)
            .ok_or_else(|| unavailable("unknown OAuth upstream server"))?
            .build_consent_url_for(intent)
            .await?;
        let mut routes = self
            .pending_routes
            .write()
            .map_err(|_| unavailable("OAuth callback router is unavailable"))?;
        routes.retain(|_, routed_server| routed_server != server);
        routes.insert(start.state.clone(), server.to_owned());
        drop(routes);
        Ok(start)
    }

    pub async fn is_connected(&self, server: &str) -> bool {
        match self.get(server) {
            Some(client) => client.is_connected().await,
            None => false,
        }
    }

    pub async fn public_state(&self, server: &str) -> UpstreamOAuthState {
        match self.get(server) {
            Some(client) => client.public_state().await,
            None => UpstreamOAuthState::Unavailable,
        }
    }

    pub async fn exchange_callback(&self, state: &str, code: &str) -> Result<()> {
        let pending_server = self
            .pending_routes
            .write()
            .map_err(|_| unavailable("OAuth callback router is unavailable"))?
            .remove(state);
        let server = match pending_server {
            Some(server) => server,
            None => {
                let route = state
                    .split_once('.')
                    .map(|(route, _)| route)
                    .ok_or_else(|| unavailable("OAuth callback state is unknown"))?;
                self.callback_routes
                    .read()
                    .map_err(|_| unavailable("OAuth callback router is unavailable"))?
                    .get(route)
                    .cloned()
                    .ok_or_else(|| unavailable("OAuth callback state is unknown"))?
            }
        };
        self.get(&server)
            .ok_or_else(|| unavailable("unknown OAuth upstream server"))?
            .exchange_callback(state, code)
            .await
    }
}

fn callback_route_key(reference: &CredentialRef) -> String {
    let mut input = String::with_capacity(
        reference.broker.len() + reference.path.len() + reference.field.len() + 2,
    );
    input.push_str(&reference.broker);
    input.push('\0');
    input.push_str(&reference.path);
    input.push('\0');
    input.push_str(&reference.field);
    let digest = blake3::hash(input.as_bytes());
    b64url_encode(&digest.as_bytes()[..16])
}

fn broker_for(
    brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
    reference: &CredentialRef,
) -> Result<Arc<dyn CredentialBroker>> {
    brokers
        .get(&reference.broker)
        .cloned()
        .ok_or_else(|| unavailable("OAuth Vault broker is unavailable"))
}

#[derive(Deserialize)]
struct CallbackQuery {
    #[serde(default)]
    code: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    error: Option<String>,
}

impl Drop for CallbackQuery {
    fn drop(&mut self) {
        self.code.zeroize();
        self.state.zeroize();
        if let Some(error) = &mut self.error {
            error.zeroize();
        }
    }
}

async fn callback(
    State(registry): State<Arc<UpstreamOAuthRegistry>>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if query.error.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Html("OAuth consent was refused. You may close this window."),
        )
            .into_response();
    }
    match registry.exchange_callback(&query.state, &query.code).await {
        Ok(()) => (
            StatusCode::OK,
            Html("OAuth connection established. You may close this window."),
        )
            .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html("OAuth callback refused. Restart the owner connection flow."),
        )
            .into_response(),
    }
}

pub fn router(registry: Arc<UpstreamOAuthRegistry>) -> Router {
    Router::new()
        .route("/oauth/callback", get(callback))
        .with_state(registry)
}
