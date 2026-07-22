//! OLR-3 E2E — OIDC ID Token binding against a fake IdP + JWKS.
//!
//! Run:
//! ```bash
//! cargo test -p aithos-gateway --test olr_oidc_e2e -- --nocapture
//! ```

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Form, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration as ChronoDuration, Utc};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreRsaPrivateSigningKey,
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, EndUserEmail, IssuerUrl, JsonWebKeyId, Nonce,
    PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use serde_json::json;

use aithos_gateway::config::{
    OAuthAccountBinding, OAuthAuthorizationParameters, OAuthClientAuthentication,
    OAuthEndpointStrategy, OAuthIdentitySource, OAuthProtocolEngine, OAuthRegistrationStrategy,
    UpstreamOAuthConfig,
};
use aithos_gateway::core_bridge::SeqEntropy;
use aithos_gateway::credentials::{
    CredentialBroker, CredentialCompareAndStoreOutcome, CredentialRef, SecretValue,
};
use aithos_gateway::upstream_oauth::{UpstreamOAuthClient, UpstreamOAuthState};
use aithos_gateway::{GatewayError, Result};

const NOW: i64 = 1_784_203_200;
const CLIENT_ID: &str = "olr-oidc-client";
const REDIRECT_URI: &str = "http://127.0.0.1:4870/oauth/callback";
const SUBJECT: &str = "subject-oidc-1";
const EMAIL: &str = "owner@example.test";
const RSA_PEM: &str = include_str!("fixtures/olr_oidc_rsa.pem");

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

fn signing_key() -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(RSA_PEM, Some(JsonWebKeyId::new("olr-test-key".into())))
        .expect("test RSA key")
}

fn jwks_json() -> String {
    let jwks = CoreJsonWebKeySet::new(vec![signing_key().as_verification_key()]);
    serde_json::to_string(&jwks).unwrap()
}

fn mint_id_token(issuer: &str, nonce: Option<&str>) -> String {
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(issuer.to_owned()).unwrap(),
        vec![Audience::new(CLIENT_ID.into())],
        Utc::now() + ChronoDuration::seconds(300),
        Utc::now(),
        StandardClaims::new(SubjectIdentifier::new(SUBJECT.into()))
            .set_email(Some(EndUserEmail::new(EMAIL.into()))),
        EmptyAdditionalClaims {},
    );
    let claims = match nonce {
        Some(nonce) => claims.set_nonce(Some(Nonce::new(nonce.to_owned()))),
        None => claims,
    };
    let id_token = CoreIdToken::new(
        claims,
        &signing_key(),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .expect("sign id_token");
    id_token.to_string()
}

#[derive(Clone)]
struct FakeOidc {
    issuer: String,
    nonces: Arc<Mutex<Vec<String>>>,
    jwks: String,
}

async fn spawn_fake_oidc() -> (String, FakeOidc, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = FakeOidc {
        issuer: base.clone(),
        nonces: Arc::new(Mutex::new(Vec::new())),
        jwks: jwks_json(),
    };
    let app = Router::new()
        .route(
            "/jwks",
            get(|State(state): State<FakeOidc>| async move {
                (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    state.jwks,
                )
            }),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(|State(state): State<FakeOidc>| async move {
                Json(json!({
                    "issuer": state.issuer,
                    "authorization_endpoint": format!("{}/authorize", state.issuer),
                    "token_endpoint": format!("{}/token", state.issuer),
                    "jwks_uri": format!("{}/jwks", state.issuer),
                    "code_challenge_methods_supported": ["S256"],
                    "token_endpoint_auth_methods_supported": ["none"],
                    "extra_vendor_field": {"ok": true}
                }))
            }),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(|State(state): State<FakeOidc>| async move {
                Json(json!({
                    "resource": format!("{}/mcp", state.issuer),
                    "authorization_servers": [state.issuer],
                    "bearer_methods_supported": ["header"]
                }))
            }),
        )
        .route(
            "/token",
            post(
                 |State(state): State<FakeOidc>, Form(form): Form<BTreeMap<String, String>>| async move {
                    let is_refresh = form.get("grant_type").map(String::as_str)
                        == Some("refresh_token");
                    let nonce = (!is_refresh).then(|| {
                        state
                            .nonces
                            .lock()
                            .unwrap()
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "missing-nonce".into())
                    });
                    let id_token = mint_id_token(&state.issuer, nonce.as_deref());
                    Json(json!({
                        "access_token": if is_refresh { "oidc-access-refreshed" } else { "oidc-access" },
                        "refresh_token": "oidc-refresh",
                        "expires_in": if is_refresh { 3600 } else { 1 },
                        "token_type": "Bearer",
                        "scope": "openid email",
                        "id_token": id_token,
                        "grant_type_echo": form.get("grant_type")
                    }))
                },
            ),
        )
        .with_state(state.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    (base, state, task)
}

fn oidc_config(base: &str, engine: OAuthProtocolEngine) -> UpstreamOAuthConfig {
    UpstreamOAuthConfig {
        auth_url: format!("{base}/authorize"),
        token_url: format!("{base}/token"),
        client_id: CLIENT_ID.into(),
        client_secret: None,
        scopes: vec!["openid".into(), "email".into()],
        redirect_uri: REDIRECT_URI.into(),
        endpoints: OAuthEndpointStrategy::Static,
        client_authentication: OAuthClientAuthentication::None,
        protocol_engine: engine,
        registration: OAuthRegistrationStrategy::Static,
        authorization_parameters: OAuthAuthorizationParameters::default(),
        resource: None,
        audience: None,
        revocation_url: None,
        account_binding: Some(OAuthAccountBinding {
            issuer: base.to_owned(),
            source: OAuthIdentitySource::IdToken {
                jwks_uri: format!("{base}/jwks"),
                audience: Some(CLIENT_ID.into()),
            },
            subject_field: "sub".into(),
            account_field: "email".into(),
        }),
        pending_vault: Some(credential("pending")),
        revocation_vault: None,
        token_vault: credential("token"),
    }
}

#[tokio::test]
async fn e2e_oidc_id_token_binding_with_oauth2_engine() {
    let (base, fake, _task) = spawn_fake_oidc().await;
    let token_broker = Arc::new(MemoryBroker::default());
    let client = UpstreamOAuthClient::new(
        oidc_config(&base, OAuthProtocolEngine::Oauth2),
        None,
        None,
        token_broker.clone() as Arc<dyn CredentialBroker>,
        Box::new(SeqEntropy::default()),
        Arc::new(|| NOW),
    )
    .unwrap();

    let consent = client.build_consent_url().await.unwrap();
    let url = reqwest::Url::parse(&consent.authorization_url).unwrap();
    let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    let nonce = query.get("nonce").cloned().expect("nonce on authorize URL");
    fake.nonces.lock().unwrap().push(nonce.clone());
    let state = query.get("state").cloned().unwrap();

    client
        .exchange_callback(&state, "oidc-code")
        .await
        .expect("OIDC callback");

    let connected = token_broker.value(&credential("token")).unwrap();
    assert!(connected.contains(SUBJECT));
    assert!(connected.contains(EMAIL));
    assert_eq!(client.public_state().await, UpstreamOAuthState::Expired);

    // A refresh response may contain a new ID Token without a nonce because
    // it is not tied to a new browser authorization request.
    let refreshed = client.access_token().await.expect("OIDC refresh ID token");
    assert_eq!(refreshed.expose(), "oidc-access-refreshed");
    assert_eq!(client.public_state().await, UpstreamOAuthState::Connected);

    // Hostile: wrong nonce must fail closed.
    let token_broker2 = Arc::new(MemoryBroker::default());
    let client2 = UpstreamOAuthClient::new(
        oidc_config(&base, OAuthProtocolEngine::Oauth2),
        None,
        None,
        token_broker2 as Arc<dyn CredentialBroker>,
        Box::new(SeqEntropy::default()),
        Arc::new(|| NOW),
    )
    .unwrap();
    let consent2 = client2.build_consent_url().await.unwrap();
    let url2 = reqwest::Url::parse(&consent2.authorization_url).unwrap();
    let state2 = url2
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    // Serve an id_token minted for a different nonce than the pending one.
    fake.nonces.lock().unwrap().push("attacker-nonce".into());
    let err = client2
        .exchange_callback(&state2, "oidc-code")
        .await
        .expect_err("nonce mismatch");
    assert!(
        format!("{err:?}").contains("verification failed")
            || format!("{err:?}").contains("unavailable")
    );
}

#[tokio::test]
async fn e2e_discovery_captures_jwks_uri_and_ignores_vendor_fields() {
    let (base, _fake, _task) = spawn_fake_oidc().await;
    let config = UpstreamOAuthConfig {
        auth_url: String::new(),
        token_url: String::new(),
        client_id: CLIENT_ID.into(),
        client_secret: None,
        scopes: vec![],
        redirect_uri: REDIRECT_URI.into(),
        endpoints: OAuthEndpointStrategy::Discovery {
            protected_resource: format!("{base}/mcp"),
            issuer: base.clone(),
        },
        client_authentication: OAuthClientAuthentication::None,
        protocol_engine: OAuthProtocolEngine::Native,
        registration: OAuthRegistrationStrategy::Static,
        authorization_parameters: OAuthAuthorizationParameters::default(),
        resource: None,
        audience: None,
        revocation_url: None,
        account_binding: Some(OAuthAccountBinding {
            issuer: base.clone(),
            source: OAuthIdentitySource::IdToken {
                jwks_uri: format!("{base}/jwks"),
                audience: Some(CLIENT_ID.into()),
            },
            subject_field: "sub".into(),
            account_field: "email".into(),
        }),
        pending_vault: None,
        revocation_vault: None,
        token_vault: credential("token"),
    };
    let resolved = aithos_gateway::oauth_discovery::OAuthDiscoveryClient::new()
        .unwrap()
        .resolve(&config)
        .await
        .expect("discovery");
    assert_eq!(
        resolved.jwks_uri.as_deref(),
        Some(format!("{base}/jwks").as_str())
    );
    assert_eq!(resolved.issuer.as_deref(), Some(base.as_str()));

    let mut mismatched = oidc_config(&base, OAuthProtocolEngine::Oauth2);
    mismatched.auth_url.clear();
    mismatched.token_url.clear();
    mismatched.endpoints = OAuthEndpointStrategy::Discovery {
        protected_resource: format!("{base}/mcp"),
        issuer: base.clone(),
    };
    if let Some(OAuthAccountBinding {
        source: OAuthIdentitySource::IdToken { jwks_uri, .. },
        ..
    }) = &mut mismatched.account_binding
    {
        *jwks_uri = format!("{base}/different-jwks");
    }
    let err = aithos_gateway::oauth_discovery::OAuthDiscoveryClient::new()
        .unwrap()
        .resolve(&mismatched)
        .await
        .expect_err("JWKS pin drift");
    assert!(format!("{err:?}").contains("JWKS endpoint changed"));
}
