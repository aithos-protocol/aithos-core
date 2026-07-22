use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use aithos_gateway::compiled_extensions::{
    GoogleSheetsWriteConfig, GoogleSheetsWriteGuardedUpstream, SHEETS_WRITE_GUARDED_TOOL,
};
use aithos_gateway::config::{
    OAuthAccountBinding, OAuthAuthorizationParameters, OAuthClientAuthentication,
    OAuthEndpointStrategy, OAuthIdentitySource, OAuthRegistrationStrategy, UpstreamOAuthConfig,
};
use aithos_gateway::core_bridge::SeqEntropy;
use aithos_gateway::credentials::{
    CredentialBroker, CredentialCompareAndStoreOutcome, CredentialRef, SecretValue,
};
use aithos_gateway::oauth_discovery::{OAuthDiscoveryClient, ResolvedOAuthEndpoints};
use aithos_gateway::oauth_registration::{ClientCredentialSource, OAuthRegistrationClient};
use aithos_gateway::proxy_mcp::Upstream;
use aithos_gateway::upstream_oauth::{UpstreamOAuthClient, UpstreamOAuthRegistry};
use aithos_gateway::{GatewayError, Result};
use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};

const NOW: i64 = 1_784_203_200;
const CLIENT_ID: &str = "oac-test-client";
const CLIENT_SECRET: &str = "oac-test-client-secret";
const REDIRECT_URI: &str = "http://127.0.0.1:4870/oauth/callback";

#[derive(Default)]
struct MemoryBroker {
    values: Mutex<BTreeMap<(String, String), String>>,
    resolves: Mutex<Vec<CredentialRef>>,
    stores: Mutex<Vec<CredentialRef>>,
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

    fn resolve_count(&self) -> usize {
        self.resolves.lock().unwrap().len()
    }

    fn store_count(&self) -> usize {
        self.stores.lock().unwrap().len()
    }
}

impl CredentialBroker for MemoryBroker {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(async move {
            self.resolves.lock().unwrap().push(reference.clone());
            self.value(reference)
                .map(SecretValue::new)
                .ok_or_else(|| GatewayError::CredentialUnavailable("test record absent".into()))
        })
    }

    fn resolve_optional<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SecretValue>>> + Send + 'a>> {
        Box::pin(async move {
            self.resolves.lock().unwrap().push(reference.clone());
            Ok(self.value(reference).map(SecretValue::new))
        })
    }

    fn store<'a>(
        &'a self,
        reference: &'a CredentialRef,
        value: SecretValue,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.stores.lock().unwrap().push(reference.clone());
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

fn static_config(base: &str, authentication: OAuthClientAuthentication) -> UpstreamOAuthConfig {
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
        protocol_engine: Default::default(),
        registration: OAuthRegistrationStrategy::Static,
        authorization_parameters: OAuthAuthorizationParameters::default(),
        resource: None,
        audience: None,
        revocation_url: None,
        account_binding: None,
        pending_vault: None,
        revocation_vault: None,
        token_vault: credential("token"),
    }
}

fn discovery_config(base: &str) -> UpstreamOAuthConfig {
    let resource = format!("{base}/mcp");
    UpstreamOAuthConfig {
        auth_url: String::new(),
        token_url: String::new(),
        client_id: CLIENT_ID.into(),
        client_secret: Some(credential("client-secret")),
        scopes: vec!["resource.read".into()],
        redirect_uri: REDIRECT_URI.into(),
        endpoints: OAuthEndpointStrategy::Discovery {
            protected_resource: resource,
            issuer: base.into(),
        },
        client_authentication: OAuthClientAuthentication::ClientSecretPost,
        protocol_engine: Default::default(),
        registration: OAuthRegistrationStrategy::Static,
        authorization_parameters: OAuthAuthorizationParameters::default(),
        resource: None,
        audience: None,
        revocation_url: None,
        account_binding: None,
        pending_vault: None,
        revocation_vault: None,
        token_vault: credential("token"),
    }
}

async fn spawn(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, task)
}

#[derive(Clone)]
enum WireAnswer {
    Json(Value),
    Redirect(String),
    Bytes(Vec<u8>),
}

impl WireAnswer {
    fn response(&self) -> Response<Body> {
        match self {
            Self::Json(value) => Json(value.clone()).into_response(),
            Self::Redirect(location) => Response::builder()
                .status(StatusCode::TEMPORARY_REDIRECT)
                .header("location", location)
                .body(Body::empty())
                .unwrap(),
            Self::Bytes(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("content-length", bytes.len())
                .body(Body::from(bytes.clone()))
                .unwrap(),
        }
    }
}

#[derive(Clone)]
struct MetadataState {
    resource: WireAnswer,
    authorization: WireAnswer,
    hits: Arc<Mutex<Vec<&'static str>>>,
}

async fn metadata_server(
    resource: WireAnswer,
    authorization: WireAnswer,
) -> (
    String,
    Arc<Mutex<Vec<&'static str>>>,
    tokio::task::JoinHandle<()>,
) {
    let hits = Arc::new(Mutex::new(Vec::new()));
    let state = MetadataState {
        resource,
        authorization,
        hits: Arc::clone(&hits),
    };
    let app = Router::new()
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("resource");
                state.resource.response()
            }),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("authorization");
                state.authorization.response()
            }),
        )
        .route(
            "/redirect-target",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("redirect-target");
                Json(json!({"unexpected": true})).into_response()
            }),
        )
        .with_state(state);
    let (base, task) = spawn(app).await;
    (base, hits, task)
}

fn resource_metadata(base: &str) -> Value {
    json!({
        "resource": format!("{base}/mcp"),
        "authorization_servers": [base],
        "scopes_supported": ["resource.read"],
        "bearer_methods_supported": ["header"]
    })
}

fn authorization_metadata(base: &str) -> Value {
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
        "revocation_endpoint": format!("{base}/revoke"),
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_post"],
        "protected_resources": [format!("{base}/mcp")]
    })
}

#[tokio::test]
async fn discovery_resolves_pinned_metadata_in_protocol_order() {
    // Bind first so every identifier and endpoint in the served documents is exact.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(Mutex::new(Vec::new()));
    let state = MetadataState {
        resource: WireAnswer::Json(resource_metadata(&base)),
        authorization: WireAnswer::Json(authorization_metadata(&base)),
        hits: Arc::clone(&hits),
    };
    let app = Router::new()
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("resource");
                state.resource.response()
            }),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("authorization");
                state.authorization.response()
            }),
        )
        .with_state(state);
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let resolved = OAuthDiscoveryClient::new()
        .unwrap()
        .resolve(&discovery_config(&base))
        .await
        .unwrap();
    assert_eq!(
        hits.lock().unwrap().as_slice(),
        &["resource", "authorization"]
    );
    assert_eq!(resolved.issuer.as_deref(), Some(base.as_str()));
    assert_eq!(resolved.authorization_endpoint, format!("{base}/authorize"));
    assert_eq!(resolved.token_endpoint, format!("{base}/token"));
    assert_eq!(
        resolved.registration_endpoint,
        Some(format!("{base}/register"))
    );
    assert_eq!(resolved.revocation_endpoint, Some(format!("{base}/revoke")));
    task.abort();
}

#[tokio::test]
async fn discovery_refuses_redirect_without_following_it() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(Mutex::new(Vec::new()));
    let state = MetadataState {
        resource: WireAnswer::Redirect(format!("{base}/redirect-target")),
        authorization: WireAnswer::Json(authorization_metadata(&base)),
        hits: Arc::clone(&hits),
    };
    let app = Router::new()
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("resource");
                state.resource.response()
            }),
        )
        .route(
            "/redirect-target",
            get(|State(state): State<MetadataState>| async move {
                state.hits.lock().unwrap().push("redirect-target");
                Json(json!({"unexpected": true})).into_response()
            }),
        )
        .with_state(state);
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let error = OAuthDiscoveryClient::new()
        .unwrap()
        .resolve(&discovery_config(&base))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("metadata endpoint refused"), "{error}");
    assert_eq!(hits.lock().unwrap().as_slice(), &["resource"]);
    task.abort();
}

#[tokio::test]
async fn discovery_refuses_oversize_metadata_before_authorization_metadata() {
    let oversized = vec![b' '; 64 * 1024 + 1];
    let (base, hits, task) = metadata_server(
        WireAnswer::Bytes(oversized),
        WireAnswer::Json(json!({"unexpected": true})),
    )
    .await;
    let error = OAuthDiscoveryClient::new()
        .unwrap()
        .resolve(&discovery_config(&base))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("too large"), "{error}");
    assert_eq!(hits.lock().unwrap().as_slice(), &["resource"]);
    task.abort();
}

#[tokio::test]
async fn discovery_refuses_issuer_drift_and_missing_s256() {
    for defect in ["issuer", "s256"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut authorization = authorization_metadata(&base);
        if defect == "issuer" {
            authorization["issuer"] = json!("https://issuer-drift.invalid");
        } else {
            authorization["code_challenge_methods_supported"] = json!(["plain"]);
        }
        let state = MetadataState {
            resource: WireAnswer::Json(resource_metadata(&base)),
            authorization: WireAnswer::Json(authorization),
            hits: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                get(|State(state): State<MetadataState>| async move {
                    state.resource.response()
                }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(|State(state): State<MetadataState>| async move {
                    state.authorization.response()
                }),
            )
            .with_state(state);
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let error = OAuthDiscoveryClient::new()
            .unwrap()
            .resolve(&discovery_config(&base))
            .await
            .unwrap_err()
            .to_string();
        if defect == "issuer" {
            assert!(error.contains("issuer mismatch"), "{error}");
        } else {
            assert!(error.contains("does not advertise S256"), "{error}");
        }
        task.abort();
    }
}

#[derive(Clone, Default)]
struct RegistrationWire {
    requests: Arc<Mutex<Vec<Value>>>,
    redirect_hits: Arc<Mutex<usize>>,
}

fn resolved_endpoints(base: &str) -> ResolvedOAuthEndpoints {
    ResolvedOAuthEndpoints {
        issuer: Some(base.into()),
        authorization_endpoint: format!("{base}/authorize"),
        token_endpoint: format!("{base}/token"),
        registration_endpoint: Some(format!("{base}/register")),
        revocation_endpoint: None,
        jwks_uri: None,
    }
}

#[tokio::test]
async fn dynamic_registration_posts_pinned_shape_and_stores_complete_record() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let wire = RegistrationWire::default();
    let app = Router::new()
        .route(
            "/register",
            post(
                |State(wire): State<RegistrationWire>, Json(body): Json<Value>| async move {
                    wire.requests.lock().unwrap().push(body);
                    Json(json!({
                        "client_id": "dynamic-client",
                        "client_secret": "dynamic-secret-sentinel",
                        "token_endpoint_auth_method": "client_secret_post",
                        "redirect_uris": [REDIRECT_URI],
                        "client_id_issued_at": NOW,
                        "client_secret_expires_at": NOW + 3600,
                        "registration_client_uri": "https://issuer.example/register/dynamic-client",
                        "registration_access_token": "registration-access-sentinel"
                    }))
                },
            ),
        )
        .with_state(wire.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let registration_ref = credential("registration");
    let broker = Arc::new(MemoryBroker::default());
    let broker_trait: Arc<dyn CredentialBroker> = broker.clone();
    let mut config = static_config(&base, OAuthClientAuthentication::ClientSecretPost);
    config.client_id.clear();
    config.client_secret = None;
    config.registration = OAuthRegistrationStrategy::Dynamic {
        endpoint: None,
        vault: registration_ref.clone(),
    };

    let registration = OAuthRegistrationClient::new()
        .unwrap()
        .resolve(
            &config,
            &resolved_endpoints(&base),
            Some(&broker_trait),
            NOW,
        )
        .await
        .unwrap();

    assert_eq!(registration.client_id, "dynamic-client");
    assert_eq!(
        registration.credential,
        ClientCredentialSource::Registration(registration_ref.clone())
    );
    assert_eq!(broker.store_count(), 1);
    let stored: Value = serde_json::from_str(&broker.value(&registration_ref).unwrap()).unwrap();
    assert_eq!(stored["client_secret"], "dynamic-secret-sentinel");
    assert_eq!(
        stored["registration_access_token"],
        "registration-access-sentinel"
    );
    let requests = wire.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["redirect_uris"], json!([REDIRECT_URI]));
    assert_eq!(
        requests[0]["token_endpoint_auth_method"],
        "client_secret_post"
    );
    assert_eq!(
        requests[0]["grant_types"],
        json!(["authorization_code", "refresh_token"])
    );
    task.abort();
}

#[tokio::test]
async fn dynamic_registration_refuses_redirect_without_following_it() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let wire = RegistrationWire::default();
    let redirect = format!("{base}/redirect-target");
    let app = Router::new()
        .route(
            "/register",
            post(move || {
                let redirect = redirect.clone();
                async move {
                    Response::builder()
                        .status(StatusCode::TEMPORARY_REDIRECT)
                        .header("location", redirect)
                        .body(Body::empty())
                        .unwrap()
                }
            }),
        )
        .route(
            "/redirect-target",
            post(|State(wire): State<RegistrationWire>| async move {
                *wire.redirect_hits.lock().unwrap() += 1;
                Json(json!({"unexpected": true}))
            }),
        )
        .with_state(wire.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let registration_ref = credential("registration");
    let broker = Arc::new(MemoryBroker::default());
    let broker_trait: Arc<dyn CredentialBroker> = broker.clone();
    let mut config = static_config(&base, OAuthClientAuthentication::ClientSecretPost);
    config.client_id.clear();
    config.client_secret = None;
    config.registration = OAuthRegistrationStrategy::Dynamic {
        endpoint: None,
        vault: registration_ref,
    };
    let error = OAuthRegistrationClient::new()
        .unwrap()
        .resolve(
            &config,
            &resolved_endpoints(&base),
            Some(&broker_trait),
            NOW,
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("refused the client"), "{error}");
    assert_eq!(*wire.redirect_hits.lock().unwrap(), 0);
    assert_eq!(broker.store_count(), 0);
    task.abort();
}

#[tokio::test]
async fn dynamic_registration_refuses_oversize_response_without_storing_it() {
    let app = Router::new().route(
        "/register",
        post(|| async {
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("content-length", 64 * 1024 + 1)
                .body(Body::from(vec![b' '; 64 * 1024 + 1]))
                .unwrap()
        }),
    );
    let (base, task) = spawn(app).await;
    let registration_ref = credential("registration");
    let broker = Arc::new(MemoryBroker::default());
    let broker_trait: Arc<dyn CredentialBroker> = broker.clone();
    let mut config = static_config(&base, OAuthClientAuthentication::ClientSecretPost);
    config.client_id.clear();
    config.client_secret = None;
    config.registration = OAuthRegistrationStrategy::Dynamic {
        endpoint: None,
        vault: registration_ref,
    };
    let error = OAuthRegistrationClient::new()
        .unwrap()
        .resolve(
            &config,
            &resolved_endpoints(&base),
            Some(&broker_trait),
            NOW,
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("too large"), "{error}");
    assert_eq!(broker.store_count(), 0);
    task.abort();
}

#[tokio::test]
async fn client_metadata_document_resolves_its_url_as_public_client_id() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let document_url = format!("{base}/client.json");
    let app = Router::new().route(
        "/client.json",
        get(|| async {
            Json(json!({
                "token_endpoint_auth_method": "none",
                "redirect_uris": [REDIRECT_URI],
                "client_name": "Aithos test client",
                "grant_types": ["authorization_code", "refresh_token"],
                "response_types": ["code"]
            }))
        }),
    );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let broker = Arc::new(MemoryBroker::default());
    let mut config = static_config(&base, OAuthClientAuthentication::None);
    config.client_id.clear();
    config.registration = OAuthRegistrationStrategy::ClientMetadataDocument {
        url: document_url.clone(),
    };
    let registration = OAuthRegistrationClient::new()
        .unwrap()
        .resolve(&config, &resolved_endpoints(&base), None, NOW)
        .await
        .unwrap();
    assert_eq!(registration.client_id, document_url);
    assert_eq!(registration.credential, ClientCredentialSource::None);
    assert_eq!(broker.resolve_count(), 0);
    assert_eq!(broker.store_count(), 0);
    task.abort();
}

#[derive(Clone, Default)]
struct TokenWire {
    grants: Arc<Mutex<Vec<TokenGrant>>>,
}

type TokenGrant = (HeaderMap, BTreeMap<String, String>);

#[tokio::test]
async fn token_endpoint_authentication_is_exact_for_post_basic_and_none() {
    for authentication in [
        OAuthClientAuthentication::ClientSecretPost,
        OAuthClientAuthentication::ClientSecretBasic,
        OAuthClientAuthentication::None,
    ] {
        let wire = TokenWire::default();
        let app = Router::new()
            .route(
                "/token",
                post(
                    |State(wire): State<TokenWire>,
                     headers: HeaderMap,
                     Form(form): Form<BTreeMap<String, String>>| async move {
                        wire.grants.lock().unwrap().push((headers, form));
                        Json(json!({
                            "access_token": "access-sentinel",
                            "refresh_token": "refresh-sentinel",
                            "expires_in": 3600,
                            "token_type": "Bearer",
                            "scope": "resource.read"
                        }))
                    },
                ),
            )
            .with_state(wire.clone());
        let (base, task) = spawn(app).await;
        let secret_broker = Arc::new(MemoryBroker::default());
        let token_broker = Arc::new(MemoryBroker::default());
        let config = static_config(&base, authentication);
        if let Some(reference) = &config.client_secret {
            secret_broker.put(reference, CLIENT_SECRET);
        }
        let secret_trait: Option<Arc<dyn CredentialBroker>> = (authentication
            != OAuthClientAuthentication::None)
            .then(|| secret_broker.clone() as Arc<dyn CredentialBroker>);
        let token_trait: Arc<dyn CredentialBroker> = token_broker.clone();
        let client = UpstreamOAuthClient::new(
            config,
            secret_trait,
            None,
            token_trait,
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap();
        let consent = client.build_consent_url().await.unwrap();
        let consent = reqwest::Url::parse(&consent.authorization_url).unwrap();
        let state = consent
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        client
            .exchange_callback(&state, "approved-code")
            .await
            .unwrap();

        let grants = wire.grants.lock().unwrap();
        assert_eq!(grants.len(), 1);
        let (headers, form) = &grants[0];
        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        match authentication {
            OAuthClientAuthentication::ClientSecretPost => {
                assert_eq!(form.get("client_id").map(String::as_str), Some(CLIENT_ID));
                assert_eq!(
                    form.get("client_secret").map(String::as_str),
                    Some(CLIENT_SECRET)
                );
                assert!(authorization.is_none());
                assert_eq!(secret_broker.resolve_count(), 1);
            }
            OAuthClientAuthentication::ClientSecretBasic => {
                assert!(!form.contains_key("client_id"));
                assert!(!form.contains_key("client_secret"));
                assert_eq!(
                    authorization,
                    Some("Basic b2FjLXRlc3QtY2xpZW50Om9hYy10ZXN0LWNsaWVudC1zZWNyZXQ=")
                );
                assert_eq!(secret_broker.resolve_count(), 1);
            }
            OAuthClientAuthentication::None => {
                assert_eq!(form.get("client_id").map(String::as_str), Some(CLIENT_ID));
                assert!(!form.contains_key("client_secret"));
                assert!(authorization.is_none());
                assert_eq!(secret_broker.resolve_count(), 0);
            }
        }
        task.abort();
    }
}

#[tokio::test]
async fn callback_state_is_one_shot_under_concurrency() {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new().route(
        "/token",
        post({
            let hits = Arc::clone(&hits);
            move || {
                let hits = Arc::clone(&hits);
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                    Json(json!({
                        "access_token": "one-shot-access",
                        "refresh_token": "one-shot-refresh",
                        "expires_in": 3600,
                        "token_type": "Bearer",
                        "scope": "resource.read"
                    }))
                }
            }
        }),
    );
    let (base, task) = spawn(app).await;
    let broker = Arc::new(MemoryBroker::default());
    let first_client = Arc::new(
        UpstreamOAuthClient::new(
            static_config(&base, OAuthClientAuthentication::None),
            None,
            None,
            broker.clone() as Arc<dyn CredentialBroker>,
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap(),
    );
    let second_client = Arc::new(
        UpstreamOAuthClient::new(
            static_config(&base, OAuthClientAuthentication::None),
            None,
            None,
            broker as Arc<dyn CredentialBroker>,
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap(),
    );
    let consent = first_client.build_consent_url().await.unwrap();
    let state = reqwest::Url::parse(&consent.authorization_url)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    let first = {
        let client = Arc::clone(&first_client);
        let state = state.clone();
        tokio::spawn(async move { client.exchange_callback(&state, "code-a").await })
    };
    let second = {
        let client = Arc::clone(&second_client);
        tokio::spawn(async move { client.exchange_callback(&state, "code-b").await })
    };
    let outcomes = [first.await.unwrap(), second.await.unwrap()];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    task.abort();
}

#[tokio::test]
async fn transient_refresh_failure_does_not_destroy_consent() {
    let app = Router::new().route(
        "/token",
        post(|Form(form): Form<BTreeMap<String, String>>| async move {
            if form.get("grant_type").map(String::as_str) == Some("authorization_code") {
                Json(json!({
                    "access_token": "short-access",
                    "refresh_token": "durable-refresh",
                    "expires_in": 1,
                    "token_type": "Bearer",
                    "scope": "resource.read"
                }))
                .into_response()
            } else {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error": "temporarily_unavailable"})),
                )
                    .into_response()
            }
        }),
    );
    let (base, task) = spawn(app).await;
    let now = Arc::new(AtomicI64::new(NOW));
    let clock: Arc<dyn Fn() -> i64 + Send + Sync> = {
        let now = Arc::clone(&now);
        Arc::new(move || now.load(Ordering::SeqCst))
    };
    let broker: Arc<dyn CredentialBroker> = Arc::new(MemoryBroker::default());
    let client = UpstreamOAuthClient::new(
        static_config(&base, OAuthClientAuthentication::None),
        None,
        None,
        broker,
        Box::new(SeqEntropy::default()),
        clock,
    )
    .unwrap();
    let consent = client.build_consent_url().await.unwrap();
    let state = reqwest::Url::parse(&consent.authorization_url)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    client
        .exchange_callback(&state, "approved-code")
        .await
        .unwrap();
    now.store(NOW + 120, Ordering::SeqCst);
    assert!(client.access_token().await.is_err());
    assert_eq!(
        client.public_state().await,
        aithos_gateway::upstream_oauth::UpstreamOAuthState::Expired
    );
    task.abort();
}

#[tokio::test]
async fn pinned_userinfo_binds_the_token_set_to_verified_subject_and_account() {
    let identity_headers = Arc::new(Mutex::new(Vec::<HeaderMap>::new()));
    let captured = Arc::clone(&identity_headers);
    let app = Router::new()
        .route(
            "/token",
            post(|| async {
                Json(json!({
                    "access_token": "identity-access-sentinel",
                    "refresh_token": "identity-refresh-sentinel",
                    "expires_in": 3600,
                    "token_type": "Bearer",
                    "scope": "resource.read"
                }))
            }),
        )
        .route(
            "/userinfo",
            get(move |headers: HeaderMap| {
                let captured = Arc::clone(&captured);
                async move {
                    captured.lock().unwrap().push(headers);
                    Json(json!({"sub": "subject-1", "email": "owner@example.test"}))
                }
            }),
        );
    let (base, task) = spawn(app).await;
    let secret_broker = Arc::new(MemoryBroker::default());
    let token_broker = Arc::new(MemoryBroker::default());
    let mut config = static_config(&base, OAuthClientAuthentication::ClientSecretPost);
    config.account_binding = Some(OAuthAccountBinding {
        issuer: "https://issuer.example.test".into(),
        source: OAuthIdentitySource::UserInfo {
            endpoint: format!("{base}/userinfo"),
        },
        subject_field: "sub".into(),
        account_field: "email".into(),
    });
    secret_broker.put(config.client_secret.as_ref().unwrap(), CLIENT_SECRET);
    let client = UpstreamOAuthClient::new(
        config.clone(),
        Some(secret_broker as Arc<dyn CredentialBroker>),
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
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    client
        .exchange_callback(&state, "identity-code")
        .await
        .unwrap();

    let headers = identity_headers.lock().unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(
        headers[0]
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer identity-access-sentinel")
    );
    let token = token_broker.value(&config.token_vault).unwrap();
    assert!(token.contains("subject-1"));
    assert!(token.contains("owner@example.test"));
    task.abort();
}

#[tokio::test]
async fn callback_routing_survives_registry_restart_without_scanning_neighbor_vaults() {
    let app = Router::new().route(
        "/token",
        post(|| async {
            Json(json!({
                "access_token": "restart-access",
                "refresh_token": "restart-refresh",
                "expires_in": 3600,
                "token_type": "Bearer",
                "scope": "resource.read"
            }))
        }),
    );
    let (base, task) = spawn(app).await;
    let broker = Arc::new(MemoryBroker::default());
    let config = static_config(&base, OAuthClientAuthentication::ClientSecretPost);
    broker.put(config.client_secret.as_ref().unwrap(), CLIENT_SECRET);
    let mut brokers = BTreeMap::<String, Arc<dyn CredentialBroker>>::new();
    brokers.insert("test".into(), broker.clone());

    let before_restart = UpstreamOAuthRegistry::default();
    before_restart
        .upsert("account-a", config.clone(), &brokers)
        .unwrap();
    let consent = before_restart.start("account-a").await.unwrap();
    let state = reqwest::Url::parse(&consent.authorization_url)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();

    let after_restart = UpstreamOAuthRegistry::default();
    after_restart
        .upsert("account-a", config.clone(), &brokers)
        .unwrap();
    let resolves_before = broker.resolve_count();
    after_restart
        .exchange_callback(&state, "restart-code")
        .await
        .unwrap();
    assert_eq!(broker.resolve_count() - resolves_before, 2);
    assert!(broker
        .value(&config.token_vault)
        .unwrap()
        .contains("restart-refresh"));
    task.abort();
}

#[tokio::test]
async fn sheets_guarded_write_is_digest_bound_bounded_and_single_put() {
    let calls = Arc::new(Mutex::new(Vec::<(HeaderMap, Value)>::new()));
    let captured = Arc::clone(&calls);
    let app = Router::new().route(
        "/v4/spreadsheets/{sheet}/values/{range}",
        put(move |headers: HeaderMap, Json(body): Json<Value>| {
            let captured = Arc::clone(&captured);
            async move {
                captured.lock().unwrap().push((headers, body));
                Json(json!({"updatedCells": 4}))
            }
        }),
    );
    let (base, task) = spawn(app).await;
    let token_broker = Arc::new(MemoryBroker::default());
    let config = static_config(&base, OAuthClientAuthentication::ClientSecretPost);
    token_broker.put(
        &config.token_vault,
        json!({
            "status": "connected",
            "access_token": "sheets-write-access",
            "refresh_token": "sheets-write-refresh",
            "expires_at": NOW + 3600,
            "scopes": ["resource.read"]
        })
        .to_string(),
    );
    let client = Arc::new(
        UpstreamOAuthClient::new(
            config,
            None,
            None,
            token_broker as Arc<dyn CredentialBroker>,
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap(),
    );
    let mut allowed = BTreeMap::new();
    allowed.insert(
        "sheet-1".into(),
        std::collections::BTreeSet::from(["Demo!B2:C3".into()]),
    );
    let upstream =
        GoogleSheetsWriteGuardedUpstream::new(GoogleSheetsWriteConfig::new(&base, allowed), client)
            .unwrap();
    let values = json!([["a", 1], [true, null]]);
    let canonical = json!({
        "spreadsheet_id": "sheet-1",
        "range": "Demo!B2:C3",
        "values": values,
    });
    let digest = blake3::hash(&serde_json::to_vec(&canonical).unwrap())
        .to_hex()
        .to_string();
    let call = |range: &str, digest: &str| {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": SHEETS_WRITE_GUARDED_TOOL,
                "arguments": {
                    "spreadsheet_id": "sheet-1",
                    "range": range,
                    "values": [["a", 1], [true, null]],
                    "payload_digest": digest,
                }
            }
        })
    };
    upstream.forward(call("Demo!B2:C3", &digest)).await.unwrap();
    assert!(upstream.forward(call("Demo!B2:D3", &digest)).await.is_err());
    assert!(upstream
        .forward(call("Demo!B2:C3", &"0".repeat(64)))
        .await
        .is_err());
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0]
            .0
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer sheets-write-access")
    );
    assert_eq!(calls[0].1["majorDimension"], "ROWS");
    task.abort();
}
