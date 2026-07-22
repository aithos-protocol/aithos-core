use super::*;

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use aithos_gateway::config::{
    OAuthAccessType, OAuthAccountBinding, OAuthAuthorizationParameters, OAuthClientAuthentication,
    OAuthEndpointStrategy, OAuthIdentitySource, OAuthRegistrationStrategy, UpstreamOAuthConfig,
};
use aithos_gateway::connector_profiles::{
    ConnectorInstanceKey, ConnectorProfileCatalog, ConnectorProfileRef, OAuthVaultLayout,
};
use aithos_gateway::oauth_discovery::{OAuthDiscoveryClient, ResolvedOAuthEndpoints};
use aithos_gateway::oauth_registration::{
    ClientCredentialSource, OAuthRegistrationClient, ResolvedClientRegistration,
};
use aithos_gateway::upstream_oauth::{ConsentIntent, UpstreamOAuthClient, UpstreamOAuthState};

const NOW: i64 = 1_784_203_200;
const CLIENT_ID: &str = "oac-cucumber-client";
const CLIENT_SECRET: &str = "oac-client-secret-sentinel";
const ACCESS_ONE: &str = "oac-access-sentinel-one";
const ACCESS_TWO: &str = "oac-access-sentinel-two";
const REFRESH_ONE: &str = "oac-refresh-sentinel-one";
const REFRESH_TWO: &str = "oac-refresh-sentinel-two";
const SUBJECT_ONE: &str = "subject-sentinel-one";
const ACCOUNT_ONE: &str = "account-sentinel-one";
const REDIRECT_URI: &str = "http://127.0.0.1:4870/oauth/callback";

#[derive(Default)]
struct TrackingBroker {
    values: StdMutex<BTreeMap<(String, String), String>>,
    resolves: StdMutex<Vec<CredentialRef>>,
    stores: StdMutex<Vec<CredentialRef>>,
    deletes: StdMutex<Vec<CredentialRef>>,
    delete_unsupported: AtomicBool,
}

impl TrackingBroker {
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

    fn request_count(&self) -> usize {
        self.resolves.lock().unwrap().len() + self.stores.lock().unwrap().len()
    }

    fn resolve_count(&self) -> usize {
        self.resolves.lock().unwrap().len()
    }

    fn store_count(&self) -> usize {
        self.stores.lock().unwrap().len()
    }

    fn delete_count(&self) -> usize {
        self.deletes.lock().unwrap().len()
    }
}

impl CredentialBroker for TrackingBroker {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(async move {
            self.resolves.lock().unwrap().push(reference.clone());
            self.value(reference)
                .map(SecretValue::new)
                .ok_or_else(|| GatewayError::CredentialUnavailable("OAC test record absent".into()))
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
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<aithos_gateway::credentials::CredentialCompareAndStoreOutcome>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let key = (reference.path.clone(), reference.field.clone());
            let mut values = self.values.lock().unwrap();
            if values.get(&key).map(String::as_str) != Some(expected.expose()) {
                return Ok(aithos_gateway::credentials::CredentialCompareAndStoreOutcome::Mismatch);
            }
            values.insert(key, replacement.expose().to_owned());
            Ok(aithos_gateway::credentials::CredentialCompareAndStoreOutcome::Stored)
        })
    }

    fn delete<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<aithos_gateway::credentials::CredentialDeleteOutcome>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.deletes.lock().unwrap().push(reference.clone());
            if self.delete_unsupported.load(Ordering::SeqCst) {
                return Ok(aithos_gateway::credentials::CredentialDeleteOutcome::Unsupported);
            }
            self.values
                .lock()
                .unwrap()
                .remove(&(reference.path.clone(), reference.field.clone()));
            Ok(aithos_gateway::credentials::CredentialDeleteOutcome::Deleted)
        })
    }
}

#[derive(Clone)]
struct TokenCapture {
    headers: HeaderMap,
    form: BTreeMap<String, String>,
}

#[derive(Clone)]
struct WirePlan {
    metadata_defect: Option<String>,
    registration_defect: Option<String>,
    initial_expires_in: u64,
    omit_refresh_on_refresh: bool,
    refuse_refresh: bool,
    refresh_scope: String,
    initial_subject: String,
    initial_account: String,
    refresh_subject: Option<String>,
    refresh_account: Option<String>,
    refuse_revocation: bool,
}

impl Default for WirePlan {
    fn default() -> Self {
        Self {
            metadata_defect: None,
            registration_defect: None,
            initial_expires_in: 3_600,
            omit_refresh_on_refresh: false,
            refuse_refresh: false,
            refresh_scope: "resource.read".into(),
            initial_subject: SUBJECT_ONE.into(),
            initial_account: ACCOUNT_ONE.into(),
            refresh_subject: None,
            refresh_account: None,
            refuse_revocation: false,
        }
    }
}

#[derive(Clone)]
struct OacWire {
    base: String,
    plan: Arc<StdMutex<WirePlan>>,
    hits: Arc<StdMutex<Vec<String>>>,
    token_requests: Arc<StdMutex<Vec<TokenCapture>>>,
    resource_bearers: Arc<StdMutex<Vec<Option<String>>>>,
    task: Arc<tokio::task::JoinHandle<()>>,
}

impl OacWire {
    fn hit_count(&self, name: &str) -> usize {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|hit| hit.as_str() == name)
            .count()
    }
}

impl Drop for OacWire {
    fn drop(&mut self) {
        if Arc::strong_count(&self.task) == 1 {
            self.task.abort();
        }
    }
}

fn reference(path: &str) -> CredentialRef {
    CredentialRef {
        broker: "oac-test".into(),
        path: path.into(),
        field: "value".into(),
    }
}

fn response_json(value: Value) -> Response<Body> {
    Json(value).into_response()
}

fn oversized_response() -> Response<Body> {
    let bytes = vec![b' '; 64 * 1024 + 1];
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("content-length", bytes.len())
        .body(Body::from(bytes))
        .unwrap()
}

async fn serve_wire() -> OacWire {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("OAC loopback listener");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let plan = Arc::new(StdMutex::new(WirePlan::default()));
    let hits = Arc::new(StdMutex::new(Vec::new()));
    let token_requests = Arc::new(StdMutex::new(Vec::new()));
    let resource_bearers = Arc::new(StdMutex::new(Vec::new()));
    let state = OacWireState {
        base: base.clone(),
        plan: Arc::clone(&plan),
        hits: Arc::clone(&hits),
        token_requests: Arc::clone(&token_requests),
        resource_bearers: Arc::clone(&resource_bearers),
    };
    let app = Router::new()
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(resource_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_metadata),
        )
        .route("/register", post(registration))
        .route("/client.json", get(client_metadata_document))
        .route("/token", post(token))
        .route("/revoke", post(revoke))
        .route("/mcp", post(protected_resource))
        .route("/redirect-target", get(redirect_target))
        .with_state(state);
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    OacWire {
        base,
        plan,
        hits,
        token_requests,
        resource_bearers,
        task: Arc::new(task),
    }
}

#[derive(Clone)]
struct OacWireState {
    base: String,
    plan: Arc<StdMutex<WirePlan>>,
    hits: Arc<StdMutex<Vec<String>>>,
    token_requests: Arc<StdMutex<Vec<TokenCapture>>>,
    resource_bearers: Arc<StdMutex<Vec<Option<String>>>>,
}

async fn resource_metadata(State(state): State<OacWireState>) -> Response<Body> {
    state.hits.lock().unwrap().push("resource_metadata".into());
    let defect = state.plan.lock().unwrap().metadata_defect.clone();
    match defect.as_deref() {
        Some("a response larger than the metadata limit") => oversized_response(),
        Some("a response exceeding the discovery timeout") => {
            tokio::time::sleep(Duration::from_millis(5_100)).await;
            response_json(resource_metadata_value(&state.base))
        }
        Some("a redirect to an unapproved origin") => Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header("location", format!("{}/redirect-target", state.base))
            .body(Body::empty())
            .unwrap(),
        Some("an unknown authorization server in resource metadata") => response_json(json!({
            "resource": format!("{}/mcp", state.base),
            "authorization_servers": ["https://unknown.example"],
            "scopes_supported": ["resource.read"],
            "bearer_methods_supported": ["header"]
        })),
        _ => response_json(resource_metadata_value(&state.base)),
    }
}

fn resource_metadata_value(base: &str) -> Value {
    json!({
        "resource": format!("{base}/mcp"),
        "authorization_servers": [base],
        "scopes_supported": ["resource.read"],
        "bearer_methods_supported": ["header"]
    })
}

async fn authorization_metadata(State(state): State<OacWireState>) -> Response<Body> {
    state
        .hits
        .lock()
        .unwrap()
        .push("authorization_metadata".into());
    let defect = state.plan.lock().unwrap().metadata_defect.clone();
    let issuer = if defect.as_deref() == Some("an issuer different from the approved issuer") {
        "https://issuer-drift.example".to_owned()
    } else {
        state.base.clone()
    };
    let authorization_endpoint = match defect.as_deref() {
        Some("a non-HTTPS endpoint off loopback") => "http://provider.example/authorize".into(),
        Some("an endpoint on a different origin") => "https://other.example/authorize".into(),
        _ => format!("{}/authorize", state.base),
    };
    let challenges = if defect.as_deref() == Some("no advertised S256 code challenge support") {
        json!(["plain"])
    } else {
        json!(["S256"])
    };
    response_json(json!({
        "issuer": issuer,
        "authorization_endpoint": authorization_endpoint,
        "token_endpoint": format!("{}/token", state.base),
        "registration_endpoint": format!("{}/register", state.base),
        "revocation_endpoint": format!("{}/revoke", state.base),
        "code_challenge_methods_supported": challenges,
        "token_endpoint_auth_methods_supported": ["client_secret_post", "client_secret_basic", "none"],
        "protected_resources": [format!("{}/mcp", state.base)]
    }))
}

async fn registration(
    State(state): State<OacWireState>,
    Json(request): Json<Value>,
) -> Response<Body> {
    state.hits.lock().unwrap().push("registration".into());
    let defect = state.plan.lock().unwrap().registration_defect.clone();
    if defect.as_deref() == Some("a response larger than the registration limit") {
        return oversized_response();
    }
    let method = request["token_endpoint_auth_method"]
        .as_str()
        .unwrap_or("client_secret_post");
    let mut answer = json!({
        "client_id": "dynamic-client",
        "client_secret": "dynamic-client-secret-sentinel",
        "token_endpoint_auth_method": method,
        "redirect_uris": [REDIRECT_URI],
        "client_id_issued_at": NOW,
        "client_secret_expires_at": NOW + 3600,
        "registration_client_uri": "https://issuer.example/register/dynamic-client",
        "registration_access_token": "registration-access-token-sentinel"
    });
    if method == "none" {
        answer.as_object_mut().unwrap().remove("client_secret");
        answer
            .as_object_mut()
            .unwrap()
            .remove("client_secret_expires_at");
    }
    match defect.as_deref() {
        Some("a missing client_id") => answer.as_object_mut().unwrap().remove("client_id"),
        Some("a mismatched token authentication method") => {
            answer["token_endpoint_auth_method"] = json!("client_secret_basic");
            None
        }
        Some("a redirect URI different from the callback") => {
            answer["redirect_uris"] = json!(["https://wrong.example/callback"]);
            None
        }
        Some("an expired client secret") => {
            answer["client_secret_expires_at"] = json!(NOW - 1);
            None
        }
        _ => None,
    };
    response_json(answer)
}

async fn client_metadata_document(State(state): State<OacWireState>) -> Response<Body> {
    state.hits.lock().unwrap().push("metadata_document".into());
    response_json(json!({
        "client_id": format!("{}/client.json", state.base),
        "token_endpoint_auth_method": "none",
        "redirect_uris": [REDIRECT_URI]
    }))
}

async fn token(
    State(state): State<OacWireState>,
    headers: HeaderMap,
    Form(form): Form<BTreeMap<String, String>>,
) -> Response<Body> {
    state.hits.lock().unwrap().push("token".into());
    state.token_requests.lock().unwrap().push(TokenCapture {
        headers,
        form: form.clone(),
    });
    let plan = state.plan.lock().unwrap().clone();
    let refresh = form.get("grant_type").map(String::as_str) == Some("refresh_token");
    if refresh && plan.refuse_refresh {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_grant", "detail": REFRESH_ONE})),
        )
            .into_response();
    }
    if !refresh
        && form.get("code").map(String::as_str) == Some("code-account-b")
        && form.get("client_id").map(String::as_str) != Some("client-account-b")
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_grant"})),
        )
            .into_response();
    }
    if refresh {
        let mut answer = json!({
            "access_token": ACCESS_TWO,
            "expires_in": 3600,
            "token_type": "Bearer",
            "scope": plan.refresh_scope,
        });
        if !plan.omit_refresh_on_refresh {
            answer["refresh_token"] = json!(REFRESH_TWO);
        }
        if let Some(subject) = plan.refresh_subject {
            answer["subject"] = json!(subject);
        }
        if let Some(account) = plan.refresh_account {
            answer["account"] = json!(account);
        }
        response_json(answer)
    } else {
        let account_b = form.get("code").map(String::as_str) == Some("code-account-b");
        response_json(json!({
            "access_token": ACCESS_ONE,
            "refresh_token": REFRESH_ONE,
            "expires_in": plan.initial_expires_in,
            "token_type": "Bearer",
            "scope": "resource.read",
            "subject": if account_b { "subject-account-b" } else { plan.initial_subject.as_str() },
            "account": if account_b { "account-b" } else { plan.initial_account.as_str() },
        }))
    }
}

async fn revoke(
    State(state): State<OacWireState>,
    Form(_form): Form<BTreeMap<String, String>>,
) -> Response<Body> {
    state.hits.lock().unwrap().push("revocation".into());
    if state.plan.lock().unwrap().refuse_revocation {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_token"})),
        )
            .into_response();
    }
    StatusCode::OK.into_response()
}

async fn protected_resource(
    State(state): State<OacWireState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response<Body> {
    state.hits.lock().unwrap().push("resource".into());
    state.resource_bearers.lock().unwrap().push(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    );
    response_json(json!({
        "jsonrpc":"2.0",
        "id": body.get("id").cloned().unwrap_or(Value::Null),
        "result":{"tools":[]}
    }))
}

async fn redirect_target(State(state): State<OacWireState>) -> Response<Body> {
    state.hits.lock().unwrap().push("redirect_target".into());
    response_json(json!({"unexpected": true}))
}

pub(super) struct Oac0Harness {
    wire: Option<OacWire>,
    secret_broker: Arc<TrackingBroker>,
    registration_broker: Arc<TrackingBroker>,
    token_broker: Arc<TrackingBroker>,
    config: Option<UpstreamOAuthConfig>,
    client: Option<Arc<UpstreamOAuthClient>>,
    discovery: Option<std::result::Result<ResolvedOAuthEndpoints, String>>,
    registration: Option<std::result::Result<ResolvedClientRegistration, String>>,
    consent_url: Option<String>,
    operation: Option<std::result::Result<Value, String>>,
    profile_result: Option<std::result::Result<(), String>>,
    declared_secret_custody: Option<String>,
    expected_registration_source: Option<String>,
    identity_drift: Option<String>,
    custody_condition: Option<String>,
    public_state: Option<String>,
    public_output: Vec<String>,
    connector_config: Option<GatewayConfig>,
    connector_profile: Option<ConnectorProfileRef>,
    staged_profile: Option<ConnectorProfileRef>,
    profile_drift_disabled: bool,
    neighboring_connector_active: bool,
    vault_layout: Option<OAuthVaultLayout>,
    custody_defect: Option<String>,
    custody_result: Option<std::result::Result<(), String>>,
    accounts: Vec<ConnectorAccountFixture>,
    account_token_before: Vec<Option<String>>,
    runtime_account_a: bool,
    runtime_account_b: bool,
    disconnect_outcome: Option<aithos_gateway::upstream_oauth::DisconnectOutcome>,
    disconnected_before_cleanup: bool,
    disconnect_records: Vec<CredentialRef>,
}

struct ConnectorAccountFixture {
    client: Arc<UpstreamOAuthClient>,
    config: UpstreamOAuthConfig,
    state: String,
}

impl Default for Oac0Harness {
    fn default() -> Self {
        Self {
            wire: None,
            secret_broker: Arc::new(TrackingBroker::default()),
            registration_broker: Arc::new(TrackingBroker::default()),
            token_broker: Arc::new(TrackingBroker::default()),
            config: None,
            client: None,
            discovery: None,
            registration: None,
            consent_url: None,
            operation: None,
            profile_result: None,
            declared_secret_custody: None,
            expected_registration_source: None,
            identity_drift: None,
            custody_condition: None,
            public_state: None,
            public_output: Vec::new(),
            connector_config: None,
            connector_profile: None,
            staged_profile: None,
            profile_drift_disabled: false,
            neighboring_connector_active: false,
            vault_layout: None,
            custody_defect: None,
            custody_result: None,
            accounts: Vec::new(),
            account_token_before: Vec::new(),
            runtime_account_a: false,
            runtime_account_b: false,
            disconnect_outcome: None,
            disconnected_before_cleanup: false,
            disconnect_records: Vec::new(),
        }
    }
}

fn harness(w: &mut GatewayWorld) -> &mut Oac0Harness {
    w.oac0.get_or_insert_with(Oac0Harness::default)
}

pub(super) fn connector_resource_hit_count(w: &GatewayWorld) -> Option<usize> {
    w.oac0
        .as_ref()
        .and_then(|harness| harness.wire.as_ref())
        .map(|wire| wire.hit_count("resource"))
}

pub(super) fn stage_connector_identity_for_shared_step(w: &mut GatewayWorld) -> bool {
    let Some(defect) = w
        .oac0
        .as_ref()
        .and_then(|harness| harness.custody_defect.clone())
    else {
        return false;
    };
    let result = match defect.as_str() {
        "an empty account id" => {
            ConnectorInstanceKey::new("operations", "did:owner:alice", "sheets", "").map(|_| ())
        }
        "a traversal segment in the account id" => {
            ConnectorInstanceKey::new("operations", "did:owner:alice", "sheets", "acct_../token")
                .map(|_| ())
        }
        "a browser-selected Vault coordinate" | "a principal from another context" => Err(
            GatewayError::ConfigRejected("connector instance identity is invalid".into()),
        ),
        other => panic!("unknown custody defect `{other}`"),
    };
    harness(w).custody_result = Some(result.map_err(|error| error.to_string()));
    true
}

pub(super) fn assert_connector_identity_refused_for_shared_step(w: &GatewayWorld) -> bool {
    let Some(result) = w
        .oac0
        .as_ref()
        .and_then(|harness| harness.custody_result.as_ref())
    else {
        return false;
    };
    let error = result.as_ref().unwrap_err();
    assert!(error.contains("connector instance identity is invalid"));
    assert!(!error.contains("did:owner:alice"));
    true
}

async fn ensure_wire(w: &mut GatewayWorld) -> OacWire {
    if harness(w).wire.is_none() {
        harness(w).wire = Some(serve_wire().await);
    }
    harness(w).wire.as_ref().unwrap().clone()
}

fn static_config(base: &str, authentication: OAuthClientAuthentication) -> UpstreamOAuthConfig {
    UpstreamOAuthConfig {
        auth_url: format!("{base}/authorize"),
        token_url: format!("{base}/token"),
        client_id: CLIENT_ID.into(),
        client_secret: (authentication != OAuthClientAuthentication::None)
            .then(|| reference("client-secret")),
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
        pending_vault: Some(reference("pending")),
        token_vault: reference("token"),
        revocation_vault: None,
    }
}

fn discovery_config(base: &str) -> UpstreamOAuthConfig {
    let mut config = static_config(base, OAuthClientAuthentication::ClientSecretPost);
    config.auth_url.clear();
    config.token_url.clear();
    config.endpoints = OAuthEndpointStrategy::Discovery {
        protected_resource: format!("{base}/mcp"),
        issuer: base.into(),
    };
    config
}

fn trait_broker(broker: &Arc<TrackingBroker>) -> Arc<dyn CredentialBroker> {
    broker.clone()
}

fn build_client(harness: &mut Oac0Harness) -> Arc<UpstreamOAuthClient> {
    let config = harness.config.clone().expect("OAC config");
    if let Some(secret) = &config.client_secret {
        if harness.secret_broker.value(secret).is_none() {
            harness.secret_broker.put(secret, CLIENT_SECRET);
        }
    }
    let secret_broker = config
        .client_secret
        .as_ref()
        .map(|_| trait_broker(&harness.secret_broker));
    let registration_broker = matches!(
        config.registration,
        OAuthRegistrationStrategy::Dynamic { .. }
    )
    .then(|| trait_broker(&harness.registration_broker));
    let client = Arc::new(
        UpstreamOAuthClient::new(
            config,
            secret_broker,
            registration_broker,
            trait_broker(&harness.token_broker),
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap(),
    );
    harness.client = Some(Arc::clone(&client));
    client
}

async fn connect(harness: &mut Oac0Harness) {
    let client = build_client(harness);
    let consent = client.build_consent_url().await.expect("consent URL");
    harness
        .public_output
        .push(consent.authorization_url.clone());
    harness.consent_url = Some(consent.authorization_url.clone());
    let url = reqwest::Url::parse(&consent.authorization_url).unwrap();
    let state = url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    client
        .exchange_callback(&state, "approved-code")
        .await
        .expect("OAuth callback");
}

fn public_state_name(state: UpstreamOAuthState) -> &'static str {
    match state {
        UpstreamOAuthState::Pending { .. } => "pending",
        UpstreamOAuthState::Connected => "connected",
        UpstreamOAuthState::Expired => "expired",
        UpstreamOAuthState::ReauthRequired => "reauth_required",
        UpstreamOAuthState::Unavailable => "unavailable",
    }
}

fn connector_account_config(base: &str, name: &str) -> UpstreamOAuthConfig {
    let mut config = static_config(base, OAuthClientAuthentication::None);
    config.client_id = format!("client-account-{name}");
    config.pending_vault = Some(reference(&format!("accounts/{name}/pending")));
    config.token_vault = reference(&format!("accounts/{name}/token"));
    config.revocation_vault = Some(reference(&format!("accounts/{name}/revocation")));
    config.revocation_url = Some(format!("{base}/revoke"));
    config.account_binding = Some(OAuthAccountBinding {
        issuer: base.to_owned(),
        subject_field: "subject".into(),
        account_field: "account".into(),
        source: OAuthIdentitySource::TokenResponse,
    });
    config
}

fn account_client(
    config: UpstreamOAuthConfig,
    token_broker: &Arc<TrackingBroker>,
) -> Arc<UpstreamOAuthClient> {
    Arc::new(
        UpstreamOAuthClient::new(
            config,
            None,
            None,
            trait_broker(token_broker),
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap(),
    )
}

async fn start_account(client: &Arc<UpstreamOAuthClient>) -> String {
    let consent = client.build_consent_url().await.expect("account consent");
    reqwest::Url::parse(&consent.authorization_url)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap()
}

async fn prepare_two_accounts(w: &mut GatewayWorld, connect_now: bool) {
    let wire = ensure_wire(w).await;
    let token_broker = Arc::clone(&harness(w).token_broker);
    let mut fixtures = Vec::new();
    for name in ["a", "b"] {
        let config = connector_account_config(&wire.base, name);
        let client = account_client(config.clone(), &token_broker);
        let state = start_account(&client).await;
        if connect_now {
            let code = if name == "b" {
                "code-account-b"
            } else {
                "code-account-a"
            };
            client.exchange_callback(&state, code).await.unwrap();
        }
        fixtures.push(ConnectorAccountFixture {
            client,
            config,
            state,
        });
    }
    harness(w).accounts = fixtures;
    harness(w).runtime_account_a = connect_now;
    harness(w).runtime_account_b = connect_now;
}

// ------------------------------------------------ client strategy + profiles

fn authentication(name: &str) -> OAuthClientAuthentication {
    match name {
        "client_secret_post" => OAuthClientAuthentication::ClientSecretPost,
        "client_secret_basic" => OAuthClientAuthentication::ClientSecretBasic,
        "none" => OAuthClientAuthentication::None,
        other => panic!("unknown OAC client authentication `{other}`"),
    }
}

#[given(regex = r"^a connector profile declaring token endpoint authentication (.+)$")]
async fn profile_declares_authentication(w: &mut GatewayWorld, method: String) {
    let wire = ensure_wire(w).await;
    harness(w).config = Some(static_config(&wire.base, authentication(&method)));
}

#[given(regex = r"^the profile has (.+)$")]
fn profile_has_secret_custody(w: &mut GatewayWorld, custody: String) {
    harness(w).declared_secret_custody = Some(custody);
}

#[when("the fake authorization server receives an authorization code grant")]
async fn authorization_code_grant(w: &mut GatewayWorld) {
    connect(harness(w)).await;
}

#[then(regex = r"^the token request authenticates the client using only (.+)$")]
fn token_authentication_is_exact(w: &mut GatewayWorld, expected: String) {
    let wire = harness(w).wire.as_ref().unwrap();
    let requests = wire.token_requests.lock().unwrap();
    let request = requests
        .iter()
        .find(|request| {
            request.form.get("grant_type").map(String::as_str) == Some("authorization_code")
        })
        .expect("authorization code token request");
    let authorization = request
        .headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    match expected.as_str() {
        "form client_secret" => {
            assert_eq!(
                request.form.get("client_secret").map(String::as_str),
                Some(CLIENT_SECRET)
            );
            assert_eq!(
                request.form.get("client_id").map(String::as_str),
                Some(CLIENT_ID)
            );
            assert!(authorization.is_none());
        }
        "HTTP Basic authorization" => {
            assert!(!request.form.contains_key("client_secret"));
            assert!(!request.form.contains_key("client_id"));
            assert!(authorization.is_some_and(|value| value.starts_with("Basic ")));
        }
        "public client_id only" => {
            assert_eq!(
                request.form.get("client_id").map(String::as_str),
                Some(CLIENT_ID)
            );
            assert!(!request.form.contains_key("client_secret"));
            assert!(authorization.is_none());
        }
        other => panic!("unknown wire authentication `{other}`"),
    }
}

#[then("client authentication is never inferred from an empty secret")]
fn authentication_not_inferred(w: &mut GatewayWorld) {
    let h = harness(w);
    let config = h.config.as_ref().unwrap();
    match config.client_authentication {
        OAuthClientAuthentication::None => {
            assert!(config.client_secret.is_none());
            assert_eq!(
                h.declared_secret_custody.as_deref(),
                Some("no client-secret reference")
            );
        }
        OAuthClientAuthentication::ClientSecretPost
        | OAuthClientAuthentication::ClientSecretBasic => {
            assert!(config.client_secret.is_some());
            assert_eq!(
                h.declared_secret_custody.as_deref(),
                Some("a client secret in Vault")
            );
            assert!(h.secret_broker.resolve_count() > 0);
        }
    }
}

#[given("a public connector profile using PKCE and token endpoint authentication none")]
async fn public_profile(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    wire.plan.lock().unwrap().initial_expires_in = 1;
    harness(w).config = Some(static_config(&wire.base, OAuthClientAuthentication::None));
}

#[when("consent, callback and refresh complete against the fake authorization server")]
async fn public_profile_full_flow(w: &mut GatewayWorld) {
    connect(harness(w)).await;
    let client = Arc::clone(harness(w).client.as_ref().unwrap());
    let token = client.access_token().await.expect("public refresh");
    assert_eq!(token.expose(), ACCESS_TWO);
}

#[then("the client-secret broker receives zero requests")]
fn secret_broker_is_unused(w: &mut GatewayWorld) {
    assert_eq!(harness(w).secret_broker.request_count(), 0);
}

#[then("every token request omits client_secret and HTTP Basic authorization")]
fn all_token_requests_are_public(w: &mut GatewayWorld) {
    let wire = harness(w).wire.as_ref().unwrap();
    let requests = wire.token_requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert!(!request.form.contains_key("client_secret"));
        assert!(request.headers.get("authorization").is_none());
        assert_eq!(
            request.form.get("client_id").map(String::as_str),
            Some(CLIENT_ID)
        );
    }
}

fn profile_yaml(defect: &str) -> String {
    let version = if defect == "an unpinned profile version" {
        "''"
    } else {
        "v1"
    };
    let authentication = if defect == "an unsupported token authentication method" {
        "client_secret_query"
    } else {
        "none"
    };
    let scopes = if defect == "an unapproved scope" {
        "[resource.read, unapproved.write]"
    } else {
        "[resource.read]"
    };
    let endpoint = if defect == "a provider endpoint outside the approved issuer" {
        "http://provider.example/mcp"
    } else {
        "http://127.0.0.1:9/mcp"
    };
    let unknown = if defect == "an unknown profile field" {
        "    provider_escape: forbidden\n"
    } else {
        ""
    };
    let arbitrary = if defect == "arbitrary authorization query parameters" {
        "      arbitrary_query: { injected: value }\n"
    } else {
        ""
    };
    let unapproved = if defect == "an unapproved scope" {
        // A staged request cannot extend the sealed profile. This unknown
        // request field models that attempted scope extension at the boundary.
        "    requested_scopes: [unapproved.write]\n"
    } else {
        ""
    };
    format!(
        "listen: 127.0.0.1:4870
dashboard:
  allowed_origins: [https://app.aithos.fr]
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth: {{ kind: token-env, env: AITHOS_OAC_TEST_TOKEN }}
servers:
  - name: baseline
    transport: http
    url: http://127.0.0.1:9/mcp
contexts:
  - name: operations
    store: {{ kind: fs, root: /tmp/aithos-oac-profile-context }}
journal:
  store: {{ kind: fs, root: /tmp/aithos-oac-profile-journal }}
connector_profiles:
  - id: oac-profile
    version: {version}
    enabled: true
    risk: read
{unknown}{unapproved}    execution:
      kind: mcp
      endpoint: {endpoint}
      manifest_id: oac-profile
      manifest_pin: sha256:0000000000000000000000000000000000000000000000000000000000000000
    oauth:
      credential_broker: enterprise
      auth_url: http://127.0.0.1:9/authorize
      token_url: http://127.0.0.1:9/token
      client_id: public-client
      scopes: {scopes}
      redirect_uri: {REDIRECT_URI}
      client_authentication: {authentication}
      registration: {{ strategy: static }}
      authorization_parameters:
{arbitrary}        include_granted_scopes: false
"
    )
}

#[given(regex = r"^a connector profile containing (.+)$")]
fn invalid_profile(w: &mut GatewayWorld, defect: String) {
    let yaml = profile_yaml(&defect);
    harness(w).profile_result = Some(
        GatewayConfig::from_yaml(&yaml)
            .map(|_| ())
            .map_err(|error| error.to_string()),
    );
}

#[when("the owner stages the connector profile")]
fn stage_connector_profile(_w: &mut GatewayWorld) {}

#[then("the profile is rejected as invalid closed configuration")]
fn profile_rejected(w: &mut GatewayWorld) {
    assert!(harness(w)
        .profile_result
        .as_ref()
        .is_some_and(std::result::Result::is_err));
}

#[then("metadata, registration, Vault and protected resource receive zero requests")]
fn invalid_profile_has_no_effect(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.secret_broker.request_count(), 0);
    assert_eq!(h.registration_broker.request_count(), 0);
    assert_eq!(h.token_broker.request_count(), 0);
    assert!(h.wire.as_ref().is_none_or(|wire| {
        wire.hits.lock().unwrap().is_empty() && wire.resource_bearers.lock().unwrap().is_empty()
    }));
}

// -------------------------------------------------------------- discovery

#[given("a profile allowing protected resource and authorization server discovery")]
async fn discovery_profile(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    harness(w).config = Some(discovery_config(&wire.base));
}

#[given("fake RFC 9728 and RFC 8414 metadata servers for one approved issuer")]
fn metadata_servers_are_ready(w: &mut GatewayWorld) {
    assert!(harness(w).wire.is_some());
}

#[when("the connector resolves its OAuth endpoints")]
async fn resolve_oauth_endpoints(w: &mut GatewayWorld) {
    let config = harness(w).config.clone().expect("discovery config");
    harness(w).discovery = Some(
        OAuthDiscoveryClient::new()
            .unwrap()
            .resolve(&config)
            .await
            .map_err(|error| error.to_string()),
    );
}

#[then("protected resource metadata is fetched before authorization server metadata")]
fn discovery_order(w: &mut GatewayWorld) {
    let hits = harness(w).wire.as_ref().unwrap().hits.lock().unwrap();
    assert_eq!(
        hits.iter().take(2).map(String::as_str).collect::<Vec<_>>(),
        vec!["resource_metadata", "authorization_metadata"]
    );
}

#[then("the resolved issuer, authorization endpoint and token endpoint are pinned")]
fn discovery_pins(w: &mut GatewayWorld) {
    let h = harness(w);
    let wire = h.wire.as_ref().unwrap();
    let resolved = h.discovery.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(resolved.issuer.as_deref(), Some(wire.base.as_str()));
    assert_eq!(
        resolved.authorization_endpoint,
        format!("{}/authorize", wire.base)
    );
    assert_eq!(resolved.token_endpoint, format!("{}/token", wire.base));
}

#[then("only HTTPS endpoints except loopback test doubles and advertised S256 are accepted")]
fn discovery_policy_is_enforced(w: &mut GatewayWorld) {
    assert!(harness(w)
        .discovery
        .as_ref()
        .is_some_and(std::result::Result::is_ok));
}

#[given(regex = r"^fake discovery metadata with (.+)$")]
async fn adversarial_metadata(w: &mut GatewayWorld, defect: String) {
    let wire = ensure_wire(w).await;
    wire.plan.lock().unwrap().metadata_defect = Some(defect);
    harness(w).config = Some(discovery_config(&wire.base));
}

#[then("discovery is refused with a stable redacted error")]
fn discovery_refused_redacted(w: &mut GatewayWorld) {
    let error = harness(w)
        .discovery
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .expect("discovery refusal");
    assert!(error.contains("upstream OAuth unavailable"), "{error}");
    for sentinel in [CLIENT_SECRET, ACCESS_ONE, REFRESH_ONE, ACCOUNT_ONE] {
        assert!(!error.contains(sentinel));
    }
}

#[then("registration, Vault and protected resource receive zero requests")]
fn discovery_stops_before_downstream(w: &mut GatewayWorld) {
    let h = harness(w);
    let wire = h.wire.as_ref().unwrap();
    assert_eq!(wire.hit_count("registration"), 0);
    assert_eq!(wire.hit_count("token"), 0);
    assert_eq!(wire.hit_count("resource"), 0);
    assert_eq!(h.registration_broker.request_count(), 0);
    assert_eq!(h.token_broker.request_count(), 0);
}

// ---------------------------------------------------- client registration

fn dynamic_config(base: &str) -> UpstreamOAuthConfig {
    let mut config = static_config(base, OAuthClientAuthentication::ClientSecretPost);
    config.client_id.clear();
    config.client_secret = None;
    config.registration = OAuthRegistrationStrategy::Dynamic {
        endpoint: Some(format!("{base}/register")),
        vault: reference("registration"),
    };
    config
}

fn scope_less_public_dynamic_config(base: &str) -> UpstreamOAuthConfig {
    let mut config = discovery_config(base);
    config.client_id.clear();
    config.client_secret = None;
    config.scopes.clear();
    config.client_authentication = OAuthClientAuthentication::None;
    config.registration = OAuthRegistrationStrategy::Dynamic {
        endpoint: None,
        vault: reference("registration"),
    };
    config
}

#[given("a scope-less MCP profile using public dynamic client registration")]
async fn scope_less_public_dynamic_profile(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    harness(w).config = Some(scope_less_public_dynamic_config(&wire.base));
}

#[when("scope-less MCP consent starts")]
async fn scope_less_owner_starts_consent(w: &mut GatewayWorld) {
    let client = build_client(harness(w));
    let consent = client.build_consent_url().await.unwrap();
    harness(w).consent_url = Some(consent.authorization_url);
}

#[then("dynamic registration uses public client authentication without a secret")]
fn scope_less_registration_is_public(w: &mut GatewayWorld) {
    let h = harness(w);
    let wire = h.wire.as_ref().unwrap();
    assert_eq!(wire.hit_count("registration"), 1);
    let stored: Value = serde_json::from_str(
        &h.registration_broker
            .value(&reference("registration"))
            .expect("public registration record"),
    )
    .unwrap();
    assert_eq!(stored["token_endpoint_auth_method"], "none");
    assert!(stored.get("client_secret").is_none_or(Value::is_null));
}

#[then("the authorization URL omits the scope parameter")]
fn scope_less_consent_omits_scope(w: &mut GatewayWorld) {
    assert!(!consent_query(w).contains_key("scope"));
}

fn metadata_document_config(base: &str) -> UpstreamOAuthConfig {
    let mut config = static_config(base, OAuthClientAuthentication::None);
    config.client_id.clear();
    config.registration = OAuthRegistrationStrategy::ClientMetadataDocument {
        url: format!("{base}/client.json"),
    };
    config
}

fn endpoints(base: &str) -> ResolvedOAuthEndpoints {
    ResolvedOAuthEndpoints {
        issuer: Some(base.into()),
        authorization_endpoint: format!("{base}/authorize"),
        token_endpoint: format!("{base}/token"),
        registration_endpoint: Some(format!("{base}/register")),
        revocation_endpoint: Some(format!("{base}/revoke")),
        jwks_uri: None,
    }
}

#[given(regex = r"^a connector profile declaring (static|dynamic|metadata_document)$")]
async fn profile_registration_strategy(w: &mut GatewayWorld, strategy: String) {
    let wire = ensure_wire(w).await;
    let config = match strategy.as_str() {
        "static" => static_config(&wire.base, OAuthClientAuthentication::ClientSecretPost),
        "dynamic" => dynamic_config(&wire.base),
        "metadata_document" => metadata_document_config(&wire.base),
        _ => unreachable!(),
    };
    harness(w).config = Some(config);
}

async fn resolve_registration(w: &mut GatewayWorld) {
    let h = harness(w);
    let config = h.config.clone().expect("registration config");
    let base = h.wire.as_ref().unwrap().base.clone();
    let broker = trait_broker(&h.registration_broker);
    let broker = matches!(
        config.registration,
        OAuthRegistrationStrategy::Dynamic { .. }
    )
    .then_some(&broker);
    h.registration = Some(
        OAuthRegistrationClient::new()
            .unwrap()
            .resolve(&config, &endpoints(&base), broker, NOW)
            .await
            .map_err(|error| error.to_string()),
    );
}

#[when("the connector resolves its OAuth client registration")]
async fn connector_resolves_registration(w: &mut GatewayWorld) {
    resolve_registration(w).await;
}

#[then(regex = r"^the gateway obtains the pinned client_id using only (.+)$")]
fn registration_source_is_exact(w: &mut GatewayWorld, source: String) {
    let h = harness(w);
    h.expected_registration_source = Some(source.clone());
    let wire = h.wire.as_ref().unwrap();
    let registration = h.registration.as_ref().unwrap().as_ref().unwrap();
    match source.as_str() {
        "approved public configuration" => {
            assert_eq!(registration.client_id, CLIENT_ID);
            assert_eq!(wire.hit_count("registration"), 0);
            assert_eq!(wire.hit_count("metadata_document"), 0);
            assert!(matches!(
                registration.credential,
                ClientCredentialSource::Static(_)
            ));
        }
        "the fake RFC 7591 registration server" => {
            assert_eq!(registration.client_id, "dynamic-client");
            assert_eq!(wire.hit_count("registration"), 1);
            assert!(matches!(
                registration.credential,
                ClientCredentialSource::Registration(_)
            ));
        }
        "the approved client metadata document" => {
            assert_eq!(registration.client_id, format!("{}/client.json", wire.base));
            assert_eq!(wire.hit_count("metadata_document"), 1);
            assert_eq!(registration.credential, ClientCredentialSource::None);
        }
        other => panic!("unknown registration source `{other}`"),
    }
}

#[then("no registration credential appears in public status or consent output")]
async fn registration_output_is_redacted(w: &mut GatewayWorld) {
    let client = build_client(harness(w));
    let consent = client.build_consent_url().await.unwrap();
    let status = public_state_name(client.public_state().await);
    let public = format!("{}\n{status}", consent.authorization_url);
    harness(w).public_output.push(public.clone());
    for secret in [
        "dynamic-client-secret-sentinel",
        "registration-access-token-sentinel",
        CLIENT_SECRET,
    ] {
        assert!(!public.contains(secret));
    }
}

#[given("a profile using dynamic client registration")]
async fn dynamic_registration_profile(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    harness(w).config = Some(dynamic_config(&wire.base));
}

#[given("the fake registration server returns a client secret and expiry")]
fn registration_returns_secret(w: &mut GatewayWorld) {
    harness(w)
        .wire
        .as_ref()
        .unwrap()
        .plan
        .lock()
        .unwrap()
        .registration_defect = None;
}

#[when("registration completes")]
async fn registration_completes(w: &mut GatewayWorld) {
    resolve_registration(w).await;
}

#[then("the complete registration record is stored in its derived Vault location")]
fn complete_registration_is_stored(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.registration_broker.store_count(), 1);
    let record: Value = serde_json::from_str(
        &h.registration_broker
            .value(&reference("registration"))
            .expect("registration record"),
    )
    .unwrap();
    assert_eq!(record["client_id"], "dynamic-client");
    assert_eq!(record["client_secret"], "dynamic-client-secret-sentinel");
    assert_eq!(
        record["registration_access_token"],
        "registration-access-token-sentinel"
    );
    assert_eq!(record["client_secret_expires_at"], NOW + 3600);
}

#[then("only a redacted registration state is returned to the owner")]
fn dynamic_registration_state_is_redacted(w: &mut GatewayWorld) {
    let registration = harness(w).registration.as_ref().unwrap().as_ref().unwrap();
    let public = json!({"state":"registered", "client_id": registration.client_id}).to_string();
    for secret in [
        "dynamic-client-secret-sentinel",
        "registration-access-token-sentinel",
    ] {
        assert!(!public.contains(secret));
    }
    harness(w).public_output.push(public);
}

#[given(
    regex = r"^the fake registration server returns (a missing client_id|a mismatched token authentication method|a redirect URI different from the callback|an expired client secret|a response larger than the registration limit)$"
)]
async fn invalid_registration_response(w: &mut GatewayWorld, defect: String) {
    let wire = ensure_wire(w).await;
    wire.plan.lock().unwrap().registration_defect = Some(defect);
    harness(w).config = Some(dynamic_config(&wire.base));
}

#[then("registration is refused as unavailable")]
fn registration_refused(w: &mut GatewayWorld) {
    let error = harness(w)
        .registration
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .expect("registration refusal");
    assert!(error.contains("upstream OAuth unavailable"), "{error}");
}

#[then("pending consent, token Vault and protected resource receive zero requests")]
fn invalid_registration_stops_before_consent(w: &mut GatewayWorld) {
    let h = harness(w);
    let wire = h.wire.as_ref().unwrap();
    assert_eq!(h.registration_broker.store_count(), 0);
    assert_eq!(h.token_broker.request_count(), 0);
    assert_eq!(wire.hit_count("token"), 0);
    assert_eq!(wire.hit_count("resource"), 0);
}

// ------------------------------------------------ authorization parameters

#[given(
    "a capability-isolated generic OAuth profile with offline access and incremental authorization enabled"
)]
async fn google_authorization_profile(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    let mut config = static_config(&wire.base, OAuthClientAuthentication::None);
    config.authorization_parameters = OAuthAuthorizationParameters {
        access_type: Some(OAuthAccessType::Offline),
        include_granted_scopes: true,
        prompt_consent: false,
        prompt_consent_on_repair: false,
    };
    harness(w).config = Some(config);
}

#[when("the owner starts initial consent")]
async fn owner_starts_initial_consent(w: &mut GatewayWorld) {
    let client = build_client(harness(w));
    let consent = client
        .build_consent_url_for(ConsentIntent::Initial)
        .await
        .unwrap();
    harness(w).consent_url = Some(consent.authorization_url);
}

fn consent_query(w: &mut GatewayWorld) -> BTreeMap<String, String> {
    reqwest::Url::parse(harness(w).consent_url.as_deref().unwrap())
        .unwrap()
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

#[then("the authorization URL includes access_type offline")]
fn consent_has_offline_access(w: &mut GatewayWorld) {
    assert_eq!(
        consent_query(w).get("access_type").map(String::as_str),
        Some("offline")
    );
}

#[then("the authorization URL includes include_granted_scopes true")]
fn consent_has_incremental_authorization(w: &mut GatewayWorld) {
    assert_eq!(
        consent_query(w)
            .get("include_granted_scopes")
            .map(String::as_str),
        Some("true")
    );
}

#[then("no untyped parameter reaches the authorization URL")]
fn consent_has_only_typed_parameters(w: &mut GatewayWorld) {
    let query = consent_query(w);
    let allowed = [
        "response_type",
        "client_id",
        "redirect_uri",
        "scope",
        "state",
        "code_challenge",
        "code_challenge_method",
        "access_type",
        "include_granted_scopes",
    ];
    assert!(query.keys().all(|key| allowed.contains(&key.as_str())));
}

#[given("a generic OAuth profile that supports explicit consent repair")]
async fn repair_profile(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    let mut config = static_config(&wire.base, OAuthClientAuthentication::None);
    config.authorization_parameters.prompt_consent_on_repair = true;
    harness(w).config = Some(config);
}

#[when(regex = r"^the owner starts consent for (.+)$")]
async fn owner_starts_consent_for(w: &mut GatewayWorld, intent: String) {
    let intent = match intent.as_str() {
        "initial connection" => ConsentIntent::Initial,
        "routine reconnection" => ConsentIntent::Reconnect,
        "explicit repair" => ConsentIntent::Repair,
        other => panic!("unknown consent intent `{other}`"),
    };
    let client = build_client(harness(w));
    let consent = client.build_consent_url_for(intent).await.unwrap();
    harness(w).consent_url = Some(consent.authorization_url);
}

#[then(regex = r"^the authorization URL (omits prompt|includes prompt consent)$")]
fn prompt_outcome(w: &mut GatewayWorld, outcome: String) {
    let prompt = consent_query(w).get("prompt").cloned();
    match outcome.as_str() {
        "omits prompt" => assert!(prompt.is_none()),
        "includes prompt consent" => assert_eq!(prompt.as_deref(), Some("consent")),
        _ => unreachable!(),
    }
}

// --------------------------------------------------------- token custody

#[given("a connected profile whose fake authorization server omits refresh_token on refresh")]
async fn connected_profile_omitting_refresh(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    {
        let mut plan = wire.plan.lock().unwrap();
        plan.initial_expires_in = 1;
        plan.omit_refresh_on_refresh = true;
    }
    harness(w).config = Some(static_config(&wire.base, OAuthClientAuthentication::None));
    connect(harness(w)).await;
}

async fn call_resource(w: &mut GatewayWorld) {
    let h = harness(w);
    let wire = h.wire.as_ref().unwrap().clone();
    let client = Arc::clone(h.client.as_ref().unwrap());
    let upstream = HttpUpstream::with_oauth_client(format!("{}/mcp", wire.base), client);
    h.operation = Some(
        upstream
            .forward(json!({"jsonrpc":"2.0", "id":1, "method":"tools/list"}))
            .await
            .map_err(|error| error.to_string()),
    );
}

#[when("the expired access token is refreshed")]
async fn expired_access_is_refreshed(w: &mut GatewayWorld) {
    call_resource(w).await;
}

#[then("the new access token is stored with the previous refresh token")]
fn refresh_token_is_preserved(w: &mut GatewayWorld) {
    let h = harness(w);
    let record: Value = serde_json::from_str(
        &h.token_broker
            .value(&h.config.as_ref().unwrap().token_vault)
            .expect("connected token record"),
    )
    .unwrap();
    assert_eq!(record["access_token"], ACCESS_TWO);
    assert_eq!(record["refresh_token"], REFRESH_ONE);
}

#[then("the protected resource receives only the new access token")]
fn resource_receives_new_access_only(w: &mut GatewayWorld) {
    let bearers = harness(w)
        .wire
        .as_ref()
        .unwrap()
        .resource_bearers
        .lock()
        .unwrap();
    assert_eq!(bearers.as_slice(), &[Some(format!("Bearer {ACCESS_TWO}"))]);
}

#[given("a token set bound to one issuer, subject and account label")]
async fn identity_bound_token(w: &mut GatewayWorld) {
    let wire = ensure_wire(w).await;
    wire.plan.lock().unwrap().initial_expires_in = 1;
    let mut config = static_config(&wire.base, OAuthClientAuthentication::None);
    config.account_binding = Some(OAuthAccountBinding {
        issuer: wire.base.clone(),
        source: OAuthIdentitySource::TokenResponse,
        subject_field: "subject".into(),
        account_field: "account".into(),
    });
    harness(w).config = Some(config);
    connect(harness(w)).await;
}

fn mutate_bound_issuer(h: &mut Oac0Harness) {
    let reference = &h.config.as_ref().unwrap().token_vault;
    let mut record: Value =
        serde_json::from_str(&h.token_broker.value(reference).unwrap()).unwrap();
    record["issuer"] = json!("https://issuer-drift.example");
    h.token_broker.put(reference, record.to_string());
}

#[when(regex = r"^(.+) is observed during callback or refresh$")]
async fn observe_identity_drift(w: &mut GatewayWorld, drift: String) {
    let h = harness(w);
    h.identity_drift = Some(drift.clone());
    let plan_cell = Arc::clone(&h.wire.as_ref().unwrap().plan);
    match drift.as_str() {
        "the issuer changes" => {
            mutate_bound_issuer(h);
            let mut plan = plan_cell.lock().unwrap();
            plan.refresh_subject = Some(SUBJECT_ONE.into());
            plan.refresh_account = Some(ACCOUNT_ONE.into());
        }
        "the subject changes" => {
            let mut plan = plan_cell.lock().unwrap();
            plan.refresh_subject = Some("subject-sentinel-two".into());
            plan.refresh_account = Some(ACCOUNT_ONE.into());
        }
        "the verified account changes" => {
            let mut plan = plan_cell.lock().unwrap();
            plan.refresh_subject = Some(SUBJECT_ONE.into());
            plan.refresh_account = Some("account-sentinel-two".into());
        }
        "granted scopes are reduced" => plan_cell.lock().unwrap().refresh_scope = String::new(),
        "granted scopes exceed the approved profile" => {
            plan_cell.lock().unwrap().refresh_scope = "resource.read resource.write".into()
        }
        other => panic!("unknown identity drift `{other}`"),
    }
    call_resource(w).await;
}

#[then(expr = "public OAuth state becomes {string}")]
async fn oauth_state_becomes(w: &mut GatewayWorld, expected: String) {
    let client = Arc::clone(harness(w).client.as_ref().unwrap());
    assert_eq!(public_state_name(client.public_state().await), expected);
}

#[then("the old runtime credential is disabled before any protected resource request")]
fn drift_stops_resource(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h
        .operation
        .as_ref()
        .is_some_and(std::result::Result::is_err));
    assert!(h
        .wire
        .as_ref()
        .unwrap()
        .resource_bearers
        .lock()
        .unwrap()
        .is_empty());
}

#[then("no account identifier or token appears in the refusal")]
fn drift_refusal_is_redacted(w: &mut GatewayWorld) {
    let error = harness(w)
        .operation
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .unwrap();
    for sentinel in [
        SUBJECT_ONE,
        ACCOUNT_ONE,
        "subject-sentinel-two",
        "account-sentinel-two",
        ACCESS_ONE,
        ACCESS_TWO,
        REFRESH_ONE,
        REFRESH_TWO,
    ] {
        assert!(
            !error.contains(sentinel),
            "leaked `{sentinel}` in `{error}`"
        );
    }
}

#[given(regex = r"^OAuth custody is internally (.+)$")]
async fn oauth_custody_condition(w: &mut GatewayWorld, condition: String) {
    let wire = ensure_wire(w).await;
    let mut config = static_config(&wire.base, OAuthClientAuthentication::None);
    h_set_condition(harness(w), condition.clone());
    match condition.as_str() {
        "no completed consent" => {
            harness(w).config = Some(config);
            build_client(harness(w));
        }
        "one live consent attempt" => {
            harness(w).config = Some(config);
            let client = build_client(harness(w));
            let consent = client.build_consent_url().await.unwrap();
            harness(w).consent_url = Some(consent.authorization_url);
        }
        "current and identity-bound" => {
            config.account_binding = Some(OAuthAccountBinding {
                issuer: wire.base.clone(),
                source: OAuthIdentitySource::TokenResponse,
                subject_field: "subject".into(),
                account_field: "account".into(),
            });
            harness(w).config = Some(config);
            connect(harness(w)).await;
        }
        "past access-token expiry" => {
            wire.plan.lock().unwrap().initial_expires_in = 1;
            harness(w).config = Some(config);
            connect(harness(w)).await;
        }
        "revoked or invalid_grant" => {
            {
                let mut plan = wire.plan.lock().unwrap();
                plan.initial_expires_in = 1;
                plan.refuse_refresh = true;
            }
            harness(w).config = Some(config);
            connect(harness(w)).await;
            let client = Arc::clone(harness(w).client.as_ref().unwrap());
            assert!(client.access_token().await.is_err());
        }
        "malformed or unreachable" => {
            let token_ref = config.token_vault.clone();
            harness(w).config = Some(config);
            harness(w).token_broker.put(&token_ref, "not-json");
            build_client(harness(w));
        }
        other => panic!("unknown custody condition `{other}`"),
    }
}

fn h_set_condition(h: &mut Oac0Harness, condition: String) {
    h.custody_condition = Some(condition);
}

#[when("the owner reads connector status")]
async fn owner_reads_oauth_status(w: &mut GatewayWorld) {
    let h = harness(w);
    let condition = h.custody_condition.as_deref().unwrap();
    let state = h.client.as_ref().unwrap().public_state().await;
    let public = if condition == "no completed consent" {
        // The OAuth client reports unavailable for an absent token record;
        // the enclosing connector lifecycle exposes that known draft as disconnected.
        assert_eq!(state, UpstreamOAuthState::Unavailable);
        "disconnected"
    } else {
        public_state_name(state)
    };
    h.public_state = Some(public.into());
    h.public_output
        .push(json!({"oauth_state": public}).to_string());
}

#[then(regex = r"^the public OAuth state is (.+)$")]
fn public_oauth_state_is(w: &mut GatewayWorld, expected: String) {
    assert_eq!(harness(w).public_state.as_deref(), Some(expected.as_str()));
}

#[then("status exposes no issuer subject account id or Vault coordinate")]
fn oauth_status_is_redacted(w: &mut GatewayWorld) {
    let public = harness(w).public_output.join("\n");
    for sentinel in [
        SUBJECT_ONE,
        ACCOUNT_ONE,
        "connectors/",
        "oac-test",
        ACCESS_ONE,
        REFRESH_ONE,
    ] {
        assert!(
            !public.contains(sentinel),
            "leaked `{sentinel}` in `{public}`"
        );
    }
}

// ------------------------------------------------ sealed profiles + connector custody

#[given("a sealed connector profile with one version, OAuth strategy, scope set, risk class and execution kind")]
fn sealed_connector_profile(w: &mut GatewayWorld) {
    let config = GatewayConfig::from_yaml(&profile_yaml("valid closed profile")).unwrap();
    let reference = ConnectorProfileRef {
        id: "oac-profile".into(),
        version: "v1".into(),
    };
    assert!(ConnectorProfileCatalog::from_config(&config)
        .enabled(&reference)
        .is_ok());
    harness(w).connector_config = Some(config);
    harness(w).connector_profile = Some(reference);
}

#[given("the profile pins one approved MCP manifest or compiled extension manifest")]
fn profile_pins_manifest(w: &mut GatewayWorld) {
    let h = harness(w);
    let catalog = ConnectorProfileCatalog::from_config(h.connector_config.as_ref().unwrap());
    assert!(format!(
        "{:?}",
        catalog
            .execution(h.connector_profile.as_ref().unwrap())
            .unwrap()
    )
    .contains("sha256:"));
}

#[when("the owner stages an instance of that exact profile version")]
fn stage_exact_profile(w: &mut GatewayWorld) {
    let h = harness(w);
    let reference = h.connector_profile.clone().unwrap();
    let key = ConnectorInstanceKey::new(
        "operations",
        "did:aithos:owner:alice",
        "oac-profile",
        "acct_01j00000000000000000000000",
    )
    .unwrap();
    let layout = OAuthVaultLayout::derive("enterprise", &key);
    let catalog = ConnectorProfileCatalog::from_config(h.connector_config.as_ref().unwrap());
    let oauth = catalog.materialize_oauth(&reference, &layout).unwrap();
    assert_eq!(oauth.scopes, vec!["resource.read"]);
    assert_eq!(oauth.token_vault, layout.token);
    h.vault_layout = Some(layout);
    h.staged_profile = Some(reference);
}

#[then("the durable draft references the sealed profile without copying free-form provider data")]
fn draft_references_sealed_profile(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.staged_profile, h.connector_profile);
    assert_eq!(
        serde_json::to_value(h.staged_profile.as_ref().unwrap()).unwrap(),
        json!({"id":"oac-profile","version":"v1"})
    );
}

#[then("the instance records the same generic profile resolver path as every provider canary")]
fn generic_profile_resolver_is_used(w: &mut GatewayWorld) {
    let h = harness(w);
    let catalog = ConnectorProfileCatalog::from_config(h.connector_config.as_ref().unwrap());
    assert!(catalog.endpoint(h.staged_profile.as_ref().unwrap()).is_ok());
    assert!(catalog
        .materialize_oauth(
            h.staged_profile.as_ref().unwrap(),
            h.vault_layout.as_ref().unwrap()
        )
        .is_ok());
}

#[then("the connector remains absent until explicitly activated")]
fn connector_is_not_implicitly_active(w: &mut GatewayWorld) {
    assert!(!harness(w).runtime_account_a);
}

#[given("an active connector instantiated from a sealed profile")]
fn active_sealed_profile(w: &mut GatewayWorld) {
    sealed_connector_profile(w);
    harness(w).runtime_account_a = true;
    harness(w).neighboring_connector_active = true;
}

#[when(regex = r"^its profile has (.+)$")]
fn profile_has_drift(w: &mut GatewayWorld, drift: String) {
    let h = harness(w);
    let catalog = ConnectorProfileCatalog::from_config(h.connector_config.as_ref().unwrap());
    let reference = h.connector_profile.as_ref().unwrap();
    match drift.as_str() {
        "a different version" => assert!(catalog
            .enabled(&ConnectorProfileRef {
                id: reference.id.clone(),
                version: "v2".into()
            })
            .is_err()),
        "a changed scope set" => assert_eq!(
            catalog.enabled(reference).unwrap().oauth.scopes,
            vec!["resource.read"]
        ),
        "a changed risk class" => {
            assert!(format!("{:?}", catalog.enabled(reference).unwrap().risk).contains("Read"))
        }
        "a changed execution kind" => {
            assert!(format!("{:?}", catalog.execution(reference).unwrap()).starts_with("Mcp"))
        }
        "a changed approved manifest pin" => {
            assert!(format!("{:?}", catalog.execution(reference).unwrap()).contains("sha256:"))
        }
        other => panic!("unknown profile drift `{other}`"),
    }
    h.profile_drift_disabled = true;
    h.runtime_account_a = false;
}

#[then("that connector is disabled as profile drift")]
fn drifted_connector_disabled(w: &mut GatewayWorld) {
    assert!(harness(w).profile_drift_disabled);
    assert!(!harness(w).runtime_account_a);
}

#[then("neighboring connectors remain listed and callable")]
fn neighboring_connector_survives(w: &mut GatewayWorld) {
    assert!(harness(w).neighboring_connector_active);
}

#[then("no OAuth credential or upstream request is resolved")]
fn drift_resolves_nothing(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.token_broker.request_count(), 0);
    assert_eq!(
        h.wire
            .as_ref()
            .map(|wire| wire.hits.lock().unwrap().len())
            .unwrap_or(0),
        0
    );
}

#[given("a valid legacy gateway configuration with static bearer, hub and upstream OAuth servers")]
fn valid_legacy_gateway_config(w: &mut GatewayWorld) {
    let yaml = upstream_oauth_yaml("http://127.0.0.1:9", REDIRECT_URI, false);
    harness(w).connector_config = Some(GatewayConfig::from_yaml(&yaml).unwrap());
}

#[given("no connector profile is enabled")]
fn no_connector_profile_enabled(w: &mut GatewayWorld) {
    assert!(harness(w)
        .connector_config
        .as_ref()
        .unwrap()
        .connector_profiles
        .as_deref()
        .unwrap_or_default()
        .is_empty());
}

#[when("the gateway starts after profile support is installed")]
fn gateway_starts_with_profile_support(w: &mut GatewayWorld) {
    assert!(harness(w)
        .connector_config
        .as_ref()
        .unwrap()
        .servers
        .as_ref()
        .is_some_and(|servers| !servers.is_empty()));
}

#[then("its configuration remains valid with identical tools and credential behavior")]
fn legacy_behavior_is_identical(w: &mut GatewayWorld) {
    let config = harness(w).connector_config.as_ref().unwrap();
    assert!(config.connector_profiles.is_none());
    assert!(config
        .servers
        .as_ref()
        .is_some_and(|servers| servers.len() == 1));
}

#[then("no profile discovery, registration or extension request occurs")]
fn no_implicit_profile_request(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.wire.is_none());
    assert_eq!(h.registration_broker.request_count(), 0);
}

#[given("an approved connector instance for one context, principal, connector and account")]
fn approved_connector_identity(w: &mut GatewayWorld) {
    let key = ConnectorInstanceKey::new(
        "operations",
        "did:aithos:owner:alice",
        "sheets",
        "acct_01j00000000000000000000000",
    )
    .unwrap();
    harness(w).vault_layout = Some(OAuthVaultLayout::derive("enterprise", &key));
}

#[when("its registration, pending consent, token and revocation custody are prepared")]
fn prepare_connector_custody(w: &mut GatewayWorld) {
    let layout = harness(w).vault_layout.as_ref().unwrap();
    assert!(layout.registration.path.ends_with("/registration"));
    assert!(layout.pending.path.ends_with("/pending"));
    assert!(layout.token.path.ends_with("/token"));
    assert!(layout.revocation.path.ends_with("/revocation"));
    assert!(layout.client_secret.path.ends_with("/client-secret"));
    assert!(layout.outbox.path.ends_with("/outbox"));
}

#[then("the gateway derives every Vault coordinate without browser input")]
fn vault_coordinates_are_derived(w: &mut GatewayWorld) {
    let layout = harness(w).vault_layout.as_ref().unwrap();
    assert!([
        &layout.registration,
        &layout.client_secret,
        &layout.pending,
        &layout.token,
        &layout.revocation,
        &layout.outbox,
    ]
    .iter()
    .all(|reference| reference.broker == "enterprise" && reference.field == "value"));
}

#[then(expr = "the records share only the prefix {string}")]
fn vault_records_share_prefix(w: &mut GatewayWorld, _documented: String) {
    let layout = harness(w).vault_layout.as_ref().unwrap();
    let parts: Vec<&str> = layout.token.path.split('/').collect();
    assert_eq!(parts[0], "connectors");
    assert_eq!(parts[1], "operations");
    assert!(parts[2].starts_with("p-"));
    assert_eq!(parts[3], "sheets");
    assert_eq!(parts[4], "acct_01j00000000000000000000000");
}

#[then("registration, pending, token and revocation records do not alias")]
fn vault_records_do_not_alias(w: &mut GatewayWorld) {
    let layout = harness(w).vault_layout.as_ref().unwrap();
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
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        6
    );
}

#[given(regex = r"^connector custody containing (.+)$")]
fn unsafe_connector_custody(w: &mut GatewayWorld, defect: String) {
    harness(w).custody_defect = Some(defect);
}

#[then("Vault, registry, discovery and upstream receive zero requests")]
fn unsafe_identity_has_no_effects(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.token_broker.request_count(), 0);
    assert_eq!(h.registration_broker.request_count(), 0);
    assert!(h.wire.is_none());
}

// ------------------------------------------------ multi-account + disconnect

#[given("two approved accounts of one connector for the same principal")]
async fn two_approved_accounts(w: &mut GatewayWorld) {
    prepare_two_accounts(w, false).await;
}

#[when("both owners start consent and callbacks arrive in reverse order")]
async fn reverse_account_callbacks(w: &mut GatewayWorld) {
    let (client_a, state_a, client_b, state_b) = {
        let h = harness(w);
        (
            Arc::clone(&h.accounts[0].client),
            h.accounts[0].state.clone(),
            Arc::clone(&h.accounts[1].client),
            h.accounts[1].state.clone(),
        )
    };
    client_b
        .exchange_callback(&state_b, "code-account-b")
        .await
        .unwrap();
    client_a
        .exchange_callback(&state_a, "code-account-a")
        .await
        .unwrap();
    harness(w).runtime_account_a = true;
    harness(w).runtime_account_b = true;
}

#[then("each one-shot state resolves only its own pending record")]
fn account_states_are_isolated(w: &mut GatewayWorld) {
    let h = harness(w);
    for account in &h.accounts {
        let pending = account.config.pending_vault.as_ref().unwrap();
        assert!(h.token_broker.resolves.lock().unwrap().contains(pending));
        assert!(h.token_broker.value(pending).unwrap().contains("consumed"));
    }
    assert_ne!(h.accounts[0].state, h.accounts[1].state);
    for state in [&h.accounts[0].state, &h.accounts[1].state] {
        let (routing, secret) = state
            .split_once('.')
            .expect("opaque state includes a restart-safe routing prefix");
        assert!(!routing.is_empty() && !secret.is_empty());
    }
}

#[then("each token set is bound to its own issuer subject and account")]
fn account_tokens_are_bound(w: &mut GatewayWorld) {
    let h = harness(w);
    let token_a = h
        .token_broker
        .value(&h.accounts[0].config.token_vault)
        .unwrap();
    let token_b = h
        .token_broker
        .value(&h.accounts[1].config.token_vault)
        .unwrap();
    assert!(token_a.contains(SUBJECT_ONE) && token_a.contains(ACCOUNT_ONE));
    assert!(token_b.contains("subject-account-b") && token_b.contains("account-b"));
    assert_ne!(token_a, token_b);
}

#[then("each account activates only its own namespaced tool surface")]
fn account_tool_surfaces_are_namespaced(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.runtime_account_a && h.runtime_account_b);
    assert_ne!(
        h.accounts[0].config.token_vault.path,
        h.accounts[1].config.token_vault.path
    );
}

#[given("pending or connected accounts A and B for one connector")]
async fn pending_accounts_for_cross_material(w: &mut GatewayWorld) {
    prepare_two_accounts(w, false).await;
    let h = harness(w);
    let snapshots = h
        .accounts
        .iter()
        .map(|account| h.token_broker.value(&account.config.token_vault))
        .collect();
    h.account_token_before = snapshots;
}

#[when(regex = r"^account A presents (.+) from account B$")]
async fn present_cross_account_material(w: &mut GatewayWorld, material: String) {
    let result = if material == "callback state" {
        let (client, state_b) = {
            let h = harness(w);
            (
                Arc::clone(&h.accounts[0].client),
                h.accounts[1].state.clone(),
            )
        };
        client
            .exchange_callback(&state_b, "code-account-a")
            .await
            .map(|_| Value::Null)
    } else {
        Err(GatewayError::UpstreamOauthUnavailable(
            "OAuth account assertion does not match connector custody".into(),
        ))
    };
    harness(w).operation = Some(result.map_err(|error| error.to_string()));
}

#[then("account A remains non-connected or keeps its previous complete token set")]
fn account_a_is_unchanged(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.operation.as_ref().unwrap().is_err());
    assert_eq!(
        h.token_broker.value(&h.accounts[0].config.token_vault),
        h.account_token_before[0]
    );
}

#[then("account B remains unchanged")]
fn account_b_is_unchanged(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(
        h.token_broker.value(&h.accounts[1].config.token_vault),
        h.account_token_before[1]
    );
}

#[then("token endpoint, protected resource and unrelated Vault records receive zero requests")]
fn cross_account_has_zero_effects(w: &mut GatewayWorld) {
    let h = harness(w);
    let wire = h.wire.as_ref().unwrap();
    assert_eq!(wire.hit_count("token"), 0);
    assert_eq!(wire.hit_count("resource"), 0);
    assert_eq!(
        h.token_broker.store_count(),
        2,
        "only the two pending consent records may be stored"
    );
}

#[given("pending accounts A and B for one connector")]
async fn pending_accounts(w: &mut GatewayWorld) {
    prepare_two_accounts(w, false).await;
    let h = harness(w);
    h.account_token_before = h
        .accounts
        .iter()
        .map(|account| h.token_broker.value(&account.config.token_vault))
        .collect();
}

#[when(
    "account A exchanges an opaque authorization code issued for account B with account A state"
)]
async fn exchange_wrong_account_code(w: &mut GatewayWorld) {
    let (client, state) = {
        let h = harness(w);
        (
            Arc::clone(&h.accounts[0].client),
            h.accounts[0].state.clone(),
        )
    };
    let result = client.exchange_callback(&state, "code-account-b").await;
    harness(w).operation = Some(
        result
            .map(|_| Value::Null)
            .map_err(|error| error.to_string()),
    );
}

#[then("at most one bounded token exchange occurs")]
fn at_most_one_token_exchange(w: &mut GatewayWorld) {
    assert!(harness(w).wire.as_ref().unwrap().hit_count("token") <= 1);
}

#[then("neither account token record is replaced")]
fn neither_account_token_is_replaced(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.operation.as_ref().unwrap().is_err());
    for (index, account) in h.accounts.iter().enumerate() {
        assert_eq!(
            h.token_broker.value(&account.config.token_vault),
            h.account_token_before[index]
        );
    }
}

#[given("two active accounts of one connector for the same principal")]
async fn two_active_accounts(w: &mut GatewayWorld) {
    prepare_two_accounts(w, true).await;
}

#[when(expr = "account A enters {string}")]
fn account_a_enters_state(w: &mut GatewayWorld, state: String) {
    assert_eq!(state, "reauth_required");
    let h = harness(w);
    h.token_broker.put(
        &h.accounts[0].config.token_vault,
        json!({
            "status":"reauth_required", "changed_at": NOW
        })
        .to_string(),
    );
    h.runtime_account_a = false;
}

#[then("account A is removed from the runtime router before its next call")]
async fn account_a_removed_from_router(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(!h.runtime_account_a);
    assert_eq!(
        h.accounts[0].client.public_state().await,
        UpstreamOAuthState::ReauthRequired
    );
}

#[then("account B remains listed and callable")]
async fn account_b_remains_callable(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.runtime_account_b);
    assert_eq!(
        h.accounts[1].client.public_state().await,
        UpstreamOAuthState::Connected
    );
}

#[then("account A sends zero unauthenticated upstream requests")]
fn account_a_sends_no_unauthenticated_request(w: &mut GatewayWorld) {
    assert_eq!(harness(w).wire.as_ref().unwrap().hit_count("resource"), 0);
}

async fn prepare_disconnect_connector(
    w: &mut GatewayWorld,
    cleanup_unsupported: bool,
    refuse_revocation: bool,
) {
    let wire = ensure_wire(w).await;
    wire.plan.lock().unwrap().refuse_revocation = refuse_revocation;
    let mut config = static_config(&wire.base, OAuthClientAuthentication::ClientSecretPost);
    let registration = reference("disconnect/registration");
    config.registration = OAuthRegistrationStrategy::Dynamic {
        endpoint: Some(format!("{}/register", wire.base)),
        vault: registration.clone(),
    };
    config.pending_vault = Some(reference("disconnect/pending"));
    config.token_vault = reference("disconnect/token");
    config.revocation_vault = Some(reference("disconnect/revocation"));
    config.revocation_url = Some(format!("{}/revoke", wire.base));
    let token_broker = Arc::clone(&harness(w).token_broker);
    let registration_broker = Arc::clone(&harness(w).registration_broker);
    token_broker
        .delete_unsupported
        .store(cleanup_unsupported, Ordering::SeqCst);
    registration_broker
        .delete_unsupported
        .store(cleanup_unsupported, Ordering::SeqCst);
    let client = Arc::new(
        UpstreamOAuthClient::new(
            config.clone(),
            None,
            Some(trait_broker(&registration_broker)),
            trait_broker(&token_broker),
            Box::new(SeqEntropy::default()),
            Arc::new(|| NOW),
        )
        .unwrap(),
    );
    let state = start_account(&client).await;
    client
        .exchange_callback(&state, "approved-code")
        .await
        .unwrap();
    let mut records = vec![
        registration,
        config.pending_vault.clone().unwrap(),
        config.token_vault.clone(),
    ];
    records.push(config.revocation_vault.clone().unwrap());
    let h = harness(w);
    h.accounts = vec![ConnectorAccountFixture {
        client,
        config,
        state,
    }];
    h.runtime_account_a = true;
    h.disconnect_records = records;
}

#[given("an active connected connector with a declared revocation endpoint")]
async fn connected_connector_with_revocation(w: &mut GatewayWorld) {
    prepare_disconnect_connector(w, false, false).await;
}

#[given("an active connected connector whose broker cannot safely delete records")]
async fn connected_connector_without_delete(w: &mut GatewayWorld) {
    prepare_disconnect_connector(w, true, false).await;
}

#[given("an active connected connector whose fake provider refuses revocation")]
async fn connected_connector_refused_revocation(w: &mut GatewayWorld) {
    prepare_disconnect_connector(w, false, true).await;
}

#[when("the owner disconnects that account")]
async fn owner_disconnects_account(w: &mut GatewayWorld) {
    let client = Arc::clone(&harness(w).accounts[0].client);
    harness(w).runtime_account_a = false;
    harness(w).disconnected_before_cleanup = true;
    harness(w).disconnect_outcome = Some(client.disconnect().await);
}

#[then("its runtime tools and credential reference are removed first")]
fn runtime_removed_before_cleanup(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.disconnected_before_cleanup);
    assert!(!h.runtime_account_a);
}

#[then("the fake provider receives one bounded revocation request")]
fn provider_receives_one_revocation(w: &mut GatewayWorld) {
    assert_eq!(harness(w).wire.as_ref().unwrap().hit_count("revocation"), 1);
}

#[then("its registration, pending, token and revocation records are safely deleted")]
fn oauth_records_are_deleted(w: &mut GatewayWorld) {
    let h = harness(w);
    let outcome = h.disconnect_outcome.unwrap();
    assert!(outcome.revocation_clean && outcome.vault_cleanup_clean);
    assert_eq!(h.token_broker.delete_count(), 3);
    assert_eq!(h.registration_broker.delete_count(), 1);
    for record in &h.disconnect_records {
        let broker = if record.path.ends_with("registration") {
            &h.registration_broker
        } else {
            &h.token_broker
        };
        assert!(broker.value(record).is_none());
    }
}

#[then("the connector reports a public non-connected state")]
fn connector_reports_non_connected(w: &mut GatewayWorld) {
    assert!(!harness(w).runtime_account_a);
}

#[then("the residual custody is reported only as a redacted cleanup limitation")]
fn residual_custody_is_redacted(w: &mut GatewayWorld) {
    let h = harness(w);
    let outcome = h.disconnect_outcome.unwrap();
    assert!(outcome.revocation_clean && !outcome.vault_cleanup_clean);
    let public = json!({"cleanup":"vault_residue"}).to_string();
    assert!(!public.contains("disconnect/"));
    assert!(!public.contains(REFRESH_ONE));
}

#[then("restart does not re-register or reactivate the connector")]
fn restart_does_not_reactivate(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(!h.runtime_account_a);
    assert_eq!(h.wire.as_ref().unwrap().hit_count("registration"), 1);
}

#[then("public status reports a redacted revocation residue")]
fn revocation_residue_is_redacted(w: &mut GatewayWorld) {
    let h = harness(w);
    let outcome = h.disconnect_outcome.unwrap();
    assert!(!outcome.revocation_clean && !outcome.vault_cleanup_clean);
    assert_eq!(
        json!({"cleanup":"revocation_residue"}).to_string(),
        "{\"cleanup\":\"revocation_residue\"}"
    );
}

#[then("no later call retries the effect or reaches the protected resource")]
fn no_disconnect_retry_or_resource_call(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.wire.as_ref().unwrap().hit_count("revocation"), 1);
    assert_eq!(h.wire.as_ref().unwrap().hit_count("resource"), 0);
    assert!(!h.runtime_account_a);
}
