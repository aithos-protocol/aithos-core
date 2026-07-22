//! Local E2E for the OLR oauth2 upstream protocol engine.
//!
//! Spins a fake Authorization Server + protected resource on loopback,
//! runs consent → callback → resource call → refresh with
//! `protocol_engine: oauth2`, and asserts custody + wire shape.
//!
//! Run:
//! ```bash
//! cd rust && cargo test -p aithos-gateway --test olr_oauth2_e2e -- --nocapture
//! ```

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

use aithos_gateway::config::{
    OAuthAuthorizationParameters, OAuthClientAuthentication, OAuthEndpointStrategy,
    OAuthProtocolEngine, OAuthRegistrationStrategy, UpstreamOAuthConfig,
};
use aithos_gateway::core_bridge::SeqEntropy;
use aithos_gateway::credentials::{
    CredentialBroker, CredentialCompareAndStoreOutcome, CredentialRef, SecretValue,
};
use aithos_gateway::upstream_oauth::{UpstreamOAuthClient, UpstreamOAuthState};
use aithos_gateway::{GatewayError, Result};

const NOW: i64 = 1_784_203_200;
const CLIENT_ID: &str = "olr-e2e-client";
const CLIENT_SECRET: &str = "olr-e2e-client-secret";
const REDIRECT_URI: &str = "http://127.0.0.1:4870/oauth/callback";
const ACCESS_1: &str = "olr-access-one";
const ACCESS_2: &str = "olr-access-two";
const REFRESH_1: &str = "olr-refresh-one";
const REFRESH_2: &str = "olr-refresh-two";

type RecordedGrant = (HeaderMap, BTreeMap<String, String>);

#[derive(Default)]
struct MemoryBroker {
    values: Mutex<BTreeMap<(String, String), String>>,
}

impl MemoryBroker {
    fn put(&self, reference: &CredentialRef, value: impl Into<String>) {
        self.values.lock().unwrap().insert(
            (reference.path.clone(), reference.field.clone()),
            value.into(),
        );
    }

    fn value(&self, reference: &CredentialRef) -> Option<String> {
        self.values
            .lock()
            .unwrap()
            .get(&(reference.path.clone(), reference.field.clone()))
            .cloned()
    }
}

impl CredentialBroker for MemoryBroker {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(async move {
            self.value(reference)
                .map(SecretValue::new)
                .ok_or_else(|| GatewayError::CredentialUnavailable("e2e record absent".into()))
        })
    }

    fn resolve_optional<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SecretValue>>> + Send + 'a>> {
        Box::pin(async move { Ok(self.value(reference).map(SecretValue::new)) })
    }

    fn store<'a>(
        &'a self,
        reference: &'a CredentialRef,
        value: SecretValue,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.put(reference, value.expose());
            Ok(())
        })
    }

    fn compare_and_store<'a>(
        &'a self,
        reference: &'a CredentialRef,
        expected: SecretValue,
        replacement: SecretValue,
    ) -> Pin<Box<dyn Future<Output = Result<CredentialCompareAndStoreOutcome>> + Send + 'a>> {
        Box::pin(async move {
            let key = (reference.path.clone(), reference.field.clone());
            let mut values = self.values.lock().unwrap();
            if values.get(&key).map(String::as_str) != Some(expected.expose()) {
                return Ok(CredentialCompareAndStoreOutcome::Mismatch);
            }
            values.insert(key, replacement.expose().to_owned());
            Ok(CredentialCompareAndStoreOutcome::Stored)
        })
    }
}

fn credential(path: &str) -> CredentialRef {
    CredentialRef {
        broker: "test".into(),
        path: path.into(),
        field: "value".into(),
    }
}

fn oauth2_config(base: &str, authentication: OAuthClientAuthentication) -> UpstreamOAuthConfig {
    UpstreamOAuthConfig {
        auth_url: format!("{base}/authorize"),
        token_url: format!("{base}/token"),
        client_id: CLIENT_ID.into(),
        client_secret: (authentication != OAuthClientAuthentication::None)
            .then(|| credential("client-secret")),
        scopes: vec!["resource.read".into()],
        redirect_uri: REDIRECT_URI.into(),
        endpoints: OAuthEndpointStrategy::Static,
        client_authentication: authentication,
        protocol_engine: OAuthProtocolEngine::Oauth2,
        registration: OAuthRegistrationStrategy::Static,
        authorization_parameters: OAuthAuthorizationParameters::default(),
        resource: None,
        audience: None,
        revocation_url: None,
        account_binding: None,
        pending_vault: Some(credential("pending")),
        revocation_vault: None,
        token_vault: credential("token"),
    }
}

#[derive(Clone, Default)]
struct FakeAs {
    grants: Arc<Mutex<Vec<RecordedGrant>>>,
    resource_bearers: Arc<Mutex<Vec<Option<String>>>>,
    refuse_refresh: Arc<Mutex<bool>>,
    initial_expires_in: Arc<Mutex<u64>>,
}

async fn spawn_fake_as(state: FakeAs) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/token",
            post(
                |State(state): State<FakeAs>,
                 headers: HeaderMap,
                 Form(form): Form<BTreeMap<String, String>>| async move {
                    state
                        .grants
                        .lock()
                        .unwrap()
                        .push((headers, form.clone()));
                    let grant = form.get("grant_type").map(String::as_str);
                    if grant == Some("refresh_token") && *state.refuse_refresh.lock().unwrap() {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(json!({
                                "error": "invalid_grant",
                                "adversarial": [CLIENT_SECRET, REFRESH_1, ACCESS_1]
                            })),
                        );
                    }
                    let body = if grant == Some("refresh_token") {
                        json!({
                            "access_token": ACCESS_2,
                            "refresh_token": REFRESH_2,
                            "expires_in": 3600,
                            "token_type": "Bearer",
                            "scope": "resource.read"
                        })
                    } else {
                        json!({
                            "access_token": ACCESS_1,
                            "refresh_token": REFRESH_1,
                            "expires_in": *state.initial_expires_in.lock().unwrap(),
                            "token_type": "Bearer",
                            "scope": "resource.read"
                        })
                    };
                    (axum::http::StatusCode::OK, Json(body))
                },
            ),
        )
        .route(
            "/mcp",
            post(
                |State(state): State<FakeAs>, headers: HeaderMap, Json(_body): Json<Value>| async move {
                    state.resource_bearers.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned),
                    );
                    Json(json!({"ok": true}))
                },
            ),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Give the listener a tick.
    tokio::time::sleep(Duration::from_millis(20)).await;
    (base, task)
}

async fn build_client(
    base: &str,
    authentication: OAuthClientAuthentication,
    secret_broker: &Arc<MemoryBroker>,
    token_broker: &Arc<MemoryBroker>,
) -> UpstreamOAuthClient {
    let config = oauth2_config(base, authentication);
    if let Some(reference) = &config.client_secret {
        secret_broker.put(reference, CLIENT_SECRET);
    }
    let secret_trait: Option<Arc<dyn CredentialBroker>> = (authentication
        != OAuthClientAuthentication::None)
        .then(|| secret_broker.clone() as Arc<dyn CredentialBroker>);
    UpstreamOAuthClient::new(
        config,
        secret_trait,
        None,
        token_broker.clone() as Arc<dyn CredentialBroker>,
        Box::new(SeqEntropy::default()),
        Arc::new(|| NOW),
    )
    .unwrap()
}

#[tokio::test]
async fn e2e_oauth2_consent_callback_resource_and_refresh() {
    let fake = FakeAs {
        initial_expires_in: Arc::new(Mutex::new(1)), // force refresh on next access
        ..FakeAs::default()
    };
    let (base, _task) = spawn_fake_as(fake.clone()).await;
    let secret_broker = Arc::new(MemoryBroker::default());
    let token_broker = Arc::new(MemoryBroker::default());
    let client = build_client(
        &base,
        OAuthClientAuthentication::ClientSecretPost,
        &secret_broker,
        &token_broker,
    )
    .await;

    let consent = client.build_consent_url().await.expect("consent");
    let url = reqwest::Url::parse(&consent.authorization_url).unwrap();
    let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert!(query.contains_key("code_challenge"));
    assert!(query.contains_key("state"));
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("resource.read")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some(REDIRECT_URI)
    );
    let state = query.get("state").cloned().unwrap();

    // Pending lives only in Vault.
    let pending = token_broker
        .value(&credential("pending"))
        .expect("pending vault record");
    assert!(pending.contains("code_verifier"));
    assert!(!consent.authorization_url.contains("code_verifier"));

    client
        .exchange_callback(&state, "approved-code")
        .await
        .expect("callback via oauth2 engine");

    let connected = token_broker
        .value(&credential("token"))
        .expect("connected vault record");
    assert!(connected.contains(ACCESS_1));
    assert!(connected.contains(REFRESH_1));
    assert_eq!(client.public_state().await, UpstreamOAuthState::Expired);

    // First grant shape: authorization_code + PKCE + client_secret_post.
    {
        let grants = fake.grants.lock().unwrap();
        assert_eq!(grants.len(), 1);
        let (headers, form) = &grants[0];
        assert_eq!(
            form.get("grant_type").map(String::as_str),
            Some("authorization_code")
        );
        assert_eq!(form.get("code").map(String::as_str), Some("approved-code"));
        assert!(form.contains_key("code_verifier"));
        assert_eq!(form.get("client_id").map(String::as_str), Some(CLIENT_ID));
        assert_eq!(
            form.get("client_secret").map(String::as_str),
            Some(CLIENT_SECRET)
        );
        assert!(headers.get("authorization").is_none());
    }

    // Expired access → oauth2 refresh → rotated custody.
    let access = client.access_token().await.expect("refresh via oauth2");
    assert_eq!(access.expose(), ACCESS_2);
    let rotated = token_broker.value(&credential("token")).unwrap();
    assert!(rotated.contains(ACCESS_2));
    assert!(rotated.contains(REFRESH_2));
    assert!(!rotated.contains(ACCESS_1));
    assert_eq!(client.public_state().await, UpstreamOAuthState::Connected);

    {
        let grants = fake.grants.lock().unwrap();
        assert_eq!(grants.len(), 2);
        let (_, form) = &grants[1];
        assert_eq!(
            form.get("grant_type").map(String::as_str),
            Some("refresh_token")
        );
        assert_eq!(
            form.get("refresh_token").map(String::as_str),
            Some(REFRESH_1)
        );
    }

    // Resource call uses the rotated bearer only.
    let http = reqwest::Client::new();
    let token = client.access_token().await.unwrap();
    let _ = http
        .post(format!("{base}/mcp"))
        .bearer_auth(token.expose())
        .json(&json!({"hello": "world"}))
        .send()
        .await
        .unwrap();
    let bearers = fake.resource_bearers.lock().unwrap();
    assert_eq!(bearers.len(), 1);
    assert_eq!(
        bearers[0].as_deref(),
        Some(format!("Bearer {ACCESS_2}").as_str())
    );
}

#[tokio::test]
async fn e2e_oauth2_public_client_and_failed_refresh_is_fail_closed() {
    let fake = FakeAs {
        initial_expires_in: Arc::new(Mutex::new(1)),
        refuse_refresh: Arc::new(Mutex::new(true)),
        ..FakeAs::default()
    };
    let (base, _task) = spawn_fake_as(fake.clone()).await;
    let secret_broker = Arc::new(MemoryBroker::default());
    let token_broker = Arc::new(MemoryBroker::default());
    let client = build_client(
        &base,
        OAuthClientAuthentication::None,
        &secret_broker,
        &token_broker,
    )
    .await;

    let consent = client.build_consent_url().await.unwrap();
    let state = reqwest::Url::parse(&consent.authorization_url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    client
        .exchange_callback(&state, "approved-code")
        .await
        .unwrap();

    {
        let grants = fake.grants.lock().unwrap();
        let (_, form) = &grants[0];
        assert_eq!(form.get("client_id").map(String::as_str), Some(CLIENT_ID));
        assert!(!form.contains_key("client_secret"));
    }

    let err = match client.access_token().await {
        Ok(_) => panic!("refresh must fail closed"),
        Err(error) => error,
    };
    let text = format!("{err:?}");
    assert!(text.contains("refused the grant") || text.contains("unavailable"));
    assert!(!text.contains(ACCESS_1));
    assert!(!text.contains(REFRESH_1));
    assert!(!text.contains(CLIENT_SECRET));
    assert!(fake.resource_bearers.lock().unwrap().is_empty());
    assert_eq!(
        client.public_state().await,
        UpstreamOAuthState::ReauthRequired
    );
}

#[tokio::test]
async fn e2e_oauth2_engine_via_env_override() {
    std::env::set_var("AITHOS_UPSTREAM_OAUTH_ENGINE", "oauth2");
    let fake = FakeAs {
        initial_expires_in: Arc::new(Mutex::new(3600)),
        ..FakeAs::default()
    };
    let (base, _task) = spawn_fake_as(fake.clone()).await;
    let secret_broker = Arc::new(MemoryBroker::default());
    let token_broker = Arc::new(MemoryBroker::default());

    // Config says native, env forces oauth2.
    let mut config = oauth2_config(&base, OAuthClientAuthentication::ClientSecretBasic);
    config.protocol_engine = OAuthProtocolEngine::Native;
    secret_broker.put(config.client_secret.as_ref().unwrap(), CLIENT_SECRET);
    let client = UpstreamOAuthClient::new(
        config,
        Some(secret_broker.clone() as Arc<dyn CredentialBroker>),
        None,
        token_broker.clone() as Arc<dyn CredentialBroker>,
        Box::new(SeqEntropy::default()),
        Arc::new(|| NOW),
    )
    .unwrap();

    let consent = client.build_consent_url().await.unwrap();
    let state = reqwest::Url::parse(&consent.authorization_url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    client
        .exchange_callback(&state, "approved-code")
        .await
        .unwrap();

    let grants = fake.grants.lock().unwrap();
    assert_eq!(grants.len(), 1);
    let (headers, form) = &grants[0];
    assert!(!form.contains_key("client_secret"));
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(authorization.starts_with("Basic "));
    std::env::remove_var("AITHOS_UPSTREAM_OAUTH_ENGINE");
}

#[test]
fn unit_pkce_and_engine_resolution() {
    // Re-export coverage through the public module surface.
    assert_eq!(
        aithos_gateway::oauth_protocol::pkce_s256_challenge(
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        ),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}
