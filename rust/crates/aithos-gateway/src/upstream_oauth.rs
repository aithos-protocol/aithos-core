//! OAuth 2.1 client custody for protected upstream MCP servers.
//!
//! The public config contains URLs, client id, scopes and Vault references.
//! PKCE verifier, CSRF state, client secret, access token and refresh token
//! never cross the gateway boundary. The token record is resolved for every
//! call, refreshed under a per-client lock, and failures happen before any
//! upstream request is sent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::config::{GatewayConfig, UpstreamOAuthConfig};
use crate::core_bridge::{EntropySource, OsEntropy};
use crate::credentials::{CredentialBroker, CredentialRef, SecretValue};
use crate::oauth::{b64url_encode, s256_challenge};
use crate::{GatewayError, Result};

const TOKEN_TIMEOUT: Duration = Duration::from_secs(10);
const EXPIRY_SKEW_SECS: i64 = 30;
const PENDING_TTL_SECS: i64 = 600;

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

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum VaultRecord {
    Pending {
        state: String,
        code_verifier: String,
        created_at: i64,
    },
    Connected {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
        scopes: Vec<String>,
    },
}

impl Drop for VaultRecord {
    fn drop(&mut self) {
        match self {
            Self::Pending {
                state,
                code_verifier,
                ..
            } => {
                state.zeroize();
                code_verifier.zeroize();
            }
            Self::Connected {
                access_token,
                refresh_token,
                scopes,
                ..
            } => {
                access_token.zeroize();
                refresh_token.zeroize();
                scopes.zeroize();
            }
        }
    }
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
    token_type: String,
    #[serde(default)]
    scope: Option<String>,
}

impl Drop for TokenAnswer {
    fn drop(&mut self) {
        self.access_token.zeroize();
        if let Some(refresh) = &mut self.refresh_token {
            refresh.zeroize();
        }
    }
}

pub struct ConsentStart {
    pub authorization_url: String,
}

/// One configured OAuth client. Clone/share through `Arc`; the refresh lock
/// prevents two simultaneous expired calls from rotating the same token set.
pub struct UpstreamOAuthClient {
    config: UpstreamOAuthConfig,
    client_secret_broker: Arc<dyn CredentialBroker>,
    token_broker: Arc<dyn CredentialBroker>,
    http: reqwest::Client,
    entropy: Mutex<Box<dyn EntropySource + Send>>,
    refresh_lock: tokio::sync::Mutex<()>,
    clock: OAuthClock,
}

impl UpstreamOAuthClient {
    pub fn new(
        config: UpstreamOAuthConfig,
        client_secret_broker: Arc<dyn CredentialBroker>,
        token_broker: Arc<dyn CredentialBroker>,
        entropy: Box<dyn EntropySource + Send>,
        clock: OAuthClock,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(TOKEN_TIMEOUT)
            .build()
            .map_err(|_| unavailable("cannot build OAuth HTTP client"))?;
        Ok(Self {
            config,
            client_secret_broker,
            token_broker,
            http,
            entropy: Mutex::new(entropy),
            refresh_lock: tokio::sync::Mutex::new(()),
            clock,
        })
    }

    pub fn config(&self) -> &UpstreamOAuthConfig {
        &self.config
    }

    async fn read_record(&self) -> Result<VaultRecord> {
        let value = self
            .token_broker
            .resolve(&self.config.token_vault)
            .await
            .map_err(|_| unavailable("OAuth token record is unavailable"))?;
        serde_json::from_str(value.expose())
            .map_err(|_| unavailable("OAuth token record is malformed"))
    }

    async fn write_record(&self, record: &VaultRecord) -> Result<()> {
        let encoded = serde_json::to_string(record)
            .map_err(|_| unavailable("cannot encode OAuth token record"))?;
        self.token_broker
            .store(&self.config.token_vault, SecretValue::new(encoded))
            .await
            .map_err(|_| unavailable("cannot persist OAuth token record"))
    }

    /// Persist a fresh PKCE verifier and state before returning the consent
    /// URL. Re-running deliberately invalidates the previous pending flow.
    pub async fn build_consent_url(&self) -> Result<ConsentStart> {
        let (verifier, state) = {
            let mut entropy = self
                .entropy
                .lock()
                .map_err(|_| unavailable("OAuth entropy lock is unavailable"))?;
            (b64url_encode(&entropy.e32()), b64url_encode(&entropy.e32()))
        };
        let challenge = s256_challenge(&verifier);
        let pending = VaultRecord::Pending {
            state,
            code_verifier: verifier,
            created_at: (self.clock)(),
        };
        self.write_record(&pending).await?;
        let state = match &pending {
            VaultRecord::Pending { state, .. } => state,
            VaultRecord::Connected { .. } => unreachable!(),
        };
        let mut url = reqwest::Url::parse(&self.config.auth_url)
            .map_err(|_| unavailable("authorization URL is invalid"))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("scope", &self.config.scopes.join(" "))
            .append_pair("state", state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(ConsentStart {
            authorization_url: url.into(),
        })
    }

    async fn pending_matches(&self, state: &str) -> bool {
        matches!(
            self.read_record().await,
            Ok(VaultRecord::Pending { state: ref expected, .. }) if expected == state
        )
    }

    async fn token_request(&self, mut form: Vec<(&'static str, String)>) -> Result<TokenAnswer> {
        let response = self
            .http
            .post(&self.config.token_url)
            .form(&form)
            .send()
            .await;
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
        if !response.status().is_success() {
            return Err(unavailable("OAuth token endpoint refused the grant"));
        }
        let answer: TokenAnswer = response
            .json()
            .await
            .map_err(|_| unavailable("OAuth token answer is malformed"))?;
        if answer.access_token.is_empty()
            || !answer.token_type.eq_ignore_ascii_case("bearer")
            || answer.expires_in == 0
        {
            return Err(unavailable("OAuth token answer is incomplete"));
        }
        Ok(answer)
    }

    fn scopes_from_answer(&self, answer: &TokenAnswer, fallback: &[String]) -> Result<Vec<String>> {
        let scopes: Vec<String> = answer.scope.as_deref().map_or_else(
            || fallback.to_vec(),
            |scope| scope.split_whitespace().map(str::to_owned).collect(),
        );
        if self
            .config
            .scopes
            .iter()
            .any(|required| !scopes.contains(required))
        {
            return Err(unavailable("OAuth token answer narrowed required scopes"));
        }
        Ok(scopes)
    }

    /// Exchange one callback only when its CSRF state matches the pending
    /// Vault record. The code and verifier are never returned or logged.
    pub async fn exchange_callback(&self, state: &str, code: &str) -> Result<()> {
        if state.is_empty() || code.is_empty() {
            return Err(unavailable("OAuth callback is incomplete"));
        }
        let record = self.read_record().await?;
        let (expected_state, verifier, created_at) = match &record {
            VaultRecord::Pending {
                state,
                code_verifier,
                created_at,
            } => (state, code_verifier, *created_at),
            VaultRecord::Connected { .. } => {
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
        let client_secret = self
            .client_secret_broker
            .resolve(&self.config.client_secret)
            .await
            .map_err(|_| unavailable("OAuth client secret is unavailable"))?;
        let answer = self
            .token_request(vec![
                ("grant_type", "authorization_code".into()),
                ("code", code.into()),
                ("redirect_uri", self.config.redirect_uri.clone()),
                ("client_id", self.config.client_id.clone()),
                ("client_secret", client_secret.expose().into()),
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
        let connected = VaultRecord::Connected {
            access_token: answer.access_token.clone(),
            refresh_token,
            expires_at: (self.clock)().saturating_add(answer.expires_in as i64),
            scopes,
        };
        self.write_record(&connected).await
    }

    pub async fn is_connected(&self) -> bool {
        matches!(self.read_record().await, Ok(VaultRecord::Connected { .. }))
    }

    /// Resolve a usable access token at the last possible moment. Expiry or
    /// any malformed/missing record refreshes or refuses before the resource
    /// request builder can be sent.
    pub async fn access_token(&self) -> Result<SecretValue> {
        let _guard = self.refresh_lock.lock().await;
        let record = self.read_record().await?;
        let (access_token, refresh_token, expires_at, scopes) = match &record {
            VaultRecord::Connected {
                access_token,
                refresh_token,
                expires_at,
                scopes,
            } => (access_token, refresh_token, *expires_at, scopes),
            VaultRecord::Pending { .. } => {
                return Err(unavailable("OAuth consent is not complete"));
            }
        };
        if expires_at > (self.clock)().saturating_add(EXPIRY_SKEW_SECS) {
            return Ok(SecretValue::new(access_token.clone()));
        }

        let client_secret = self
            .client_secret_broker
            .resolve(&self.config.client_secret)
            .await
            .map_err(|_| unavailable("OAuth client secret is unavailable"))?;
        let answer = self
            .token_request(vec![
                ("grant_type", "refresh_token".into()),
                ("refresh_token", refresh_token.clone()),
                ("client_id", self.config.client_id.clone()),
                ("client_secret", client_secret.expose().into()),
            ])
            .await?;
        let next_refresh = answer
            .refresh_token
            .as_ref()
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| refresh_token.clone());
        let next_access = answer.access_token.clone();
        let next_scopes = self.scopes_from_answer(&answer, scopes)?;
        let connected = VaultRecord::Connected {
            access_token: next_access.clone(),
            refresh_token: next_refresh,
            expires_at: (self.clock)().saturating_add(answer.expires_in as i64),
            scopes: next_scopes,
        };
        self.write_record(&connected).await?;
        Ok(SecretValue::new(next_access))
    }
}

/// All configured upstream OAuth clients, shared by callback routing, owner
/// onboarding and the HTTP upstream authorization seam.
#[derive(Default)]
pub struct UpstreamOAuthRegistry {
    clients: BTreeMap<String, Arc<UpstreamOAuthClient>>,
}

impl UpstreamOAuthRegistry {
    pub fn from_config(
        cfg: &GatewayConfig,
        brokers: &BTreeMap<String, Arc<dyn CredentialBroker>>,
    ) -> Result<Self> {
        let mut clients = BTreeMap::new();
        for server in cfg.servers.as_deref().unwrap_or_default() {
            let Some(oauth) = &server.oauth else {
                continue;
            };
            let secret_broker = broker_for(brokers, &oauth.client_secret)?;
            let token_broker = broker_for(brokers, &oauth.token_vault)?;
            clients.insert(
                server.name.clone(),
                Arc::new(UpstreamOAuthClient::new(
                    oauth.clone(),
                    secret_broker,
                    token_broker,
                    Box::new(OsEntropy),
                    Arc::new(system_epoch),
                )?),
            );
        }
        Ok(Self { clients })
    }

    pub fn get(&self, server: &str) -> Option<Arc<UpstreamOAuthClient>> {
        self.clients.get(server).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn server_names(&self) -> impl Iterator<Item = &str> {
        self.clients.keys().map(String::as_str)
    }

    pub async fn disconnected_server_names(&self) -> std::collections::BTreeSet<String> {
        let mut disconnected = std::collections::BTreeSet::new();
        for (name, client) in &self.clients {
            if !client.is_connected().await {
                disconnected.insert(name.clone());
            }
        }
        disconnected
    }

    pub async fn start(&self, server: &str) -> Result<ConsentStart> {
        self.clients
            .get(server)
            .ok_or_else(|| unavailable("unknown OAuth upstream server"))?
            .build_consent_url()
            .await
    }

    pub async fn is_connected(&self, server: &str) -> bool {
        match self.clients.get(server) {
            Some(client) => client.is_connected().await,
            None => false,
        }
    }

    pub async fn exchange_callback(&self, state: &str, code: &str) -> Result<()> {
        for client in self.clients.values() {
            if client.pending_matches(state).await {
                return client.exchange_callback(state, code).await;
            }
        }
        Err(unavailable("OAuth callback state is unknown"))
    }
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
