use super::*;

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, AtomicUsize};

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};

use aithos_gateway::connectors::{
    ApprovedManifestRef, ConnectorControl, ConnectorFailure, ConnectorOAuthDescriptor,
    ConnectorRegistryStore, ConnectorStageRequest, ConnectorTransport, PersistenceFault,
};
use aithos_gateway::core_bridge::{approved_manifest_catalog_digest, owner_grant_connector_config};
use aithos_gateway::policy::hub_exposed_name;
use aithos_gateway::proxy_mcp::{DynamicUpstreams, McpRouter};

const CONNECTOR: &str = "calendar-safe";
const CONNECTOR_B: &str = "crm-safe";
const CONTEXT: &str = "operations";
const NEIGHBOR_CONTEXT: &str = "finance";
const CLIENT_SECRET: &str = "g7b-client-secret-sentinel";
const ACCESS_ONE: &str = "g7b-access-token-sentinel-one";
const ACCESS_TWO: &str = "g7b-access-token-sentinel-two";
const REFRESH_ONE: &str = "g7b-refresh-token-sentinel-one";
const REFRESH_TWO: &str = "g7b-refresh-token-sentinel-two";
const CALLBACK_CODE: &str = "g7b-approved-code-sentinel";
const INTERNAL_SENTINEL: &str = "g7b-internal-failure-sentinel";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ApprovalPlacement {
    Exact,
    Missing,
    AnotherId,
    AnotherContext,
}

#[derive(Default)]
struct TrackingVault {
    values: StdMutex<BTreeMap<(String, String), String>>,
    resolves: StdMutex<Vec<(String, String)>>,
    stores: StdMutex<Vec<(String, String)>>,
    fail_all: AtomicBool,
    fail_paths: StdMutex<BTreeSet<String>>,
}

impl TrackingVault {
    fn value(&self, path: &str) -> Option<String> {
        self.values
            .lock()
            .unwrap()
            .get(&(path.to_owned(), "value".to_owned()))
            .cloned()
    }

    fn put(&self, path: &str, value: impl Into<String>) {
        self.values
            .lock()
            .unwrap()
            .insert((path.to_owned(), "value".to_owned()), value.into());
    }

    fn remove(&self, path: &str) {
        self.values
            .lock()
            .unwrap()
            .remove(&(path.to_owned(), "value".to_owned()));
    }

    fn clear_counts(&self) {
        self.resolves.lock().unwrap().clear();
        self.stores.lock().unwrap().clear();
    }

    fn request_count(&self) -> usize {
        self.resolves.lock().unwrap().len() + self.stores.lock().unwrap().len()
    }

    fn store_count_for(&self, path: &str) -> usize {
        self.stores
            .lock()
            .unwrap()
            .iter()
            .filter(|(candidate, field)| candidate == path && field == "value")
            .count()
    }

    fn fail_path(&self, path: &str) {
        self.fail_paths.lock().unwrap().insert(path.to_owned());
    }
}

impl CredentialBroker for TrackingVault {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(async move {
            self.resolves
                .lock()
                .unwrap()
                .push((reference.path.clone(), reference.field.clone()));
            if self.fail_all.load(Ordering::SeqCst)
                || self.fail_paths.lock().unwrap().contains(&reference.path)
            {
                return Err(GatewayError::CredentialUnavailable(
                    INTERNAL_SENTINEL.to_owned(),
                ));
            }
            self.values
                .lock()
                .unwrap()
                .get(&(reference.path.clone(), reference.field.clone()))
                .cloned()
                .map(SecretValue::new)
                .ok_or_else(|| GatewayError::CredentialUnavailable(INTERNAL_SENTINEL.to_owned()))
        })
    }

    fn store<'a>(
        &'a self,
        reference: &'a CredentialRef,
        value: SecretValue,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.stores
                .lock()
                .unwrap()
                .push((reference.path.clone(), reference.field.clone()));
            if self.fail_all.load(Ordering::SeqCst)
                || self.fail_paths.lock().unwrap().contains(&reference.path)
            {
                return Err(GatewayError::CredentialUnavailable(
                    INTERNAL_SENTINEL.to_owned(),
                ));
            }
            self.values.lock().unwrap().insert(
                (reference.path.clone(), reference.field.clone()),
                value.expose().to_owned(),
            );
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
            self.resolves
                .lock()
                .unwrap()
                .push((reference.path.clone(), reference.field.clone()));
            self.stores
                .lock()
                .unwrap()
                .push((reference.path.clone(), reference.field.clone()));
            if self.fail_all.load(Ordering::SeqCst)
                || self.fail_paths.lock().unwrap().contains(&reference.path)
            {
                return Err(GatewayError::CredentialUnavailable(
                    INTERNAL_SENTINEL.to_owned(),
                ));
            }
            let key = (reference.path.clone(), reference.field.clone());
            let mut values = self.values.lock().unwrap();
            if values.get(&key).map(String::as_str) != Some(expected.expose()) {
                return Ok(CredentialCompareAndStoreOutcome::Mismatch);
            }
            values.insert(key, replacement.expose().to_owned());
            Ok(CredentialCompareAndStoreOutcome::Stored)
        })
    }

    fn readiness<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = CredentialBrokerReadiness> + Send + 'a>> {
        Box::pin(async move {
            if self.fail_all.load(Ordering::SeqCst) {
                CredentialBrokerReadiness::Unavailable
            } else {
                CredentialBrokerReadiness::Ready
            }
        })
    }
}

#[derive(Clone)]
struct ConnectorWire {
    catalogue: Arc<StdMutex<Vec<Value>>>,
    token_grants: Arc<StdMutex<Vec<BTreeMap<String, String>>>>,
    mcp_requests: Arc<StdMutex<Vec<Value>>>,
    bearers: Arc<StdMutex<Vec<Option<String>>>>,
    gamma_counts_at_call: Arc<StdMutex<Vec<usize>>>,
    gamma_store: Arc<StdMutex<Option<GatewayStore>>>,
    refuse_refresh: Arc<AtomicBool>,
    initial_expires_in: Arc<AtomicU64>,
}

impl ConnectorWire {
    fn new(catalogue: Vec<Value>) -> Self {
        Self {
            catalogue: Arc::new(StdMutex::new(catalogue)),
            token_grants: Arc::default(),
            mcp_requests: Arc::default(),
            bearers: Arc::default(),
            gamma_counts_at_call: Arc::default(),
            gamma_store: Arc::default(),
            refuse_refresh: Arc::new(AtomicBool::new(false)),
            initial_expires_in: Arc::new(AtomicU64::new(3_600)),
        }
    }

    fn clear_mcp(&self) {
        self.mcp_requests.lock().unwrap().clear();
        self.bearers.lock().unwrap().clear();
        self.gamma_counts_at_call.lock().unwrap().clear();
    }

    fn refresh_count(&self) -> usize {
        self.token_grants
            .lock()
            .unwrap()
            .iter()
            .filter(|grant| grant.get("grant_type").map(String::as_str) == Some("refresh_token"))
            .count()
    }

    fn list_count(&self) -> usize {
        self.mcp_requests
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["method"] == "tools/list")
            .count()
    }

    fn call_count(&self) -> usize {
        self.mcp_requests
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["method"] == "tools/call")
            .count()
    }
}

async fn serve_connector_wire(wire: ConnectorWire) -> (String, tokio::task::JoinHandle<()>) {
    let app =
        Router::new()
            .route(
                "/token",
                post(
                    |State(state): State<ConnectorWire>,
                     Form(form): Form<BTreeMap<String, String>>| async move {
                        state.token_grants.lock().unwrap().push(form.clone());
                        let refresh =
                            form.get("grant_type").map(String::as_str) == Some("refresh_token");
                        if refresh && state.refuse_refresh.load(Ordering::SeqCst) {
                            return (
                                axum::http::StatusCode::BAD_REQUEST,
                                Json(json!({
                                    "error": "invalid_grant",
                                    "detail": INTERNAL_SENTINEL,
                                })),
                            );
                        }
                        let expires_in = if refresh {
                            3_600
                        } else {
                            state.initial_expires_in.load(Ordering::SeqCst)
                        };
                        let body = if refresh {
                            json!({
                                "access_token": ACCESS_TWO,
                                "refresh_token": REFRESH_TWO,
                                "expires_in": expires_in,
                                "token_type": "Bearer",
                                "scope": "calendar.read",
                            })
                        } else {
                            json!({
                                "access_token": ACCESS_ONE,
                                "refresh_token": REFRESH_ONE,
                                "expires_in": expires_in,
                                "token_type": "Bearer",
                                "scope": "calendar.read",
                            })
                        };
                        (axum::http::StatusCode::OK, Json(body))
                    },
                ),
            )
            .route(
                "/mcp",
                post(
                    |State(state): State<ConnectorWire>,
                     headers: HeaderMap,
                     Json(body): Json<Value>| async move {
                        state.mcp_requests.lock().unwrap().push(body.clone());
                        state.bearers.lock().unwrap().push(
                            headers
                                .get("authorization")
                                .and_then(|value| value.to_str().ok())
                                .map(str::to_owned),
                        );
                        let id = body.get("id").cloned().unwrap_or(Value::Null);
                        if body["method"] == "tools/call" {
                            let count = state
                                .gamma_store
                                .lock()
                                .unwrap()
                                .as_ref()
                                .and_then(|store| gamma_view(store.clone()).ok())
                                .map_or(0, |entries| {
                                    entries
                                        .iter()
                                        .filter(|entry| entry.kind == "action")
                                        .count()
                                });
                            state.gamma_counts_at_call.lock().unwrap().push(count);
                        }
                        if headers.get("authorization").is_none() {
                            return Json(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": { "code": -32001, "message": "unauthorized" },
                            }));
                        }
                        if body["method"] == "tools/list" {
                            return Json(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "tools": state.catalogue.lock().unwrap().clone() },
                            }));
                        }
                        Json(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": "connector-ok" }],
                                "isError": false,
                            },
                        }))
                    },
                ),
            )
            .with_state(wire);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, task)
}

fn approved_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "events.list",
            "description": "List events in one approved calendar",
            "inputSchema": {
                "type": "object",
                "properties": { "calendar_id": { "type": "string" } },
                "required": ["calendar_id"],
                "additionalProperties": false,
            },
        }),
        json!({
            "name": "events.delete",
            "description": "Delete one neighboring event",
            "inputSchema": {
                "type": "object",
                "properties": { "event_id": { "type": "string" } },
                "required": ["event_id"],
                "additionalProperties": false,
            },
        }),
    ]
}

async fn approved_manifest(id: &str) -> ApprovedManifest {
    let proposed = discover_server(id, &FakeMcp::advertising(approved_tools()))
        .await
        .unwrap();
    approve_manifest(
        &proposed,
        &BTreeMap::from([
            (
                "events.list".to_owned(),
                ToolApproval::granted(ToolAccess::Read).with_bounds(vec![ArgumentBound::OneOf {
                    field: "calendar_id".to_owned(),
                    values: vec!["primary".to_owned()],
                }]),
            ),
            (
                "events.delete".to_owned(),
                ToolApproval::denied(ToolAccess::Write),
            ),
        ]),
    )
    .unwrap()
}

struct G7bControlAuthority {
    signing: SigningKey,
    key: String,
    mandate: String,
}

pub(super) struct G7bHarness {
    _scratch: tempfile::TempDir,
    config: GatewayConfig,
    runner: Arc<Mutex<Runner>>,
    router: Arc<McpRouter<HttpUpstream>>,
    control: Arc<ConnectorControl>,
    oauth: Arc<UpstreamOAuthRegistry>,
    dynamic: DynamicUpstreams,
    vault: Arc<TrackingVault>,
    wire: ConnectorWire,
    base: String,
    host: String,
    callback_url: String,
    client: reqwest::Client,
    owner: SigningKey,
    config_authority: G7bControlAuthority,
    descriptors: BTreeMap<String, ConnectorStageRequest>,
    context_store: GatewayStore,
    journal_store: GatewayStore,
    registry_path: std::path::PathBuf,
    servers: Vec<tokio::task::JoinHandle<()>>,
    nonce: AtomicUsize,
    last: Option<HttpCapture>,
    captures: Vec<HttpCapture>,
    defect: Option<String>,
    pending_body: Option<Vec<u8>>,
    pending_state: Option<String>,
    facts: BTreeMap<String, Value>,
    public_blobs: Vec<String>,
    atomic_reads: Vec<Vec<u8>>,
}

impl Drop for G7bHarness {
    fn drop(&mut self) {
        for task in &self.servers {
            task.abort();
        }
    }
}

impl G7bHarness {
    async fn exact() -> Self {
        Self::provision(&[CONNECTOR], ApprovalPlacement::Exact).await
    }

    async fn two() -> Self {
        Self::provision(&[CONNECTOR, CONNECTOR_B], ApprovalPlacement::Exact).await
    }

    async fn provision(ids: &[&str], placement: ApprovalPlacement) -> Self {
        let wire = ConnectorWire::new(approved_tools());
        let (upstream_base, upstream_task) = serve_connector_wire(wire.clone()).await;
        let callback_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let callback_url = format!(
            "http://{}/oauth/callback",
            callback_listener.local_addr().unwrap()
        );
        let scratch = tempfile::tempdir().unwrap();
        let context_root = scratch.path().join("operations");
        let neighbor_root = scratch.path().join("finance");
        let journal_root = scratch.path().join("journal");
        let store = |root: &std::path::Path| {
            GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                root: root.to_owned(),
            })
            .unwrap()
        };
        let context_store = store(&context_root);
        let neighbor_store = store(&neighbor_root);
        let journal_store = store(&journal_root);
        let master = [0x47; 32];
        let keyholder = Keyholder::from_entropy([0x51; 32], [0x52; 32]);
        let agent_pub = agent_pub_multibase(&keyholder);
        let gateway_pub = gateway_pub_multibase(&keyholder);
        let mut entropy = SeqEntropy::default();
        owner_init_context(
            &master,
            CONTEXT,
            context_store.clone(),
            CONTROL_NOW,
            &mut entropy,
        )
        .unwrap();

        let mut manifests = BTreeMap::new();
        for id in ids {
            manifests.insert((*id).to_owned(), approved_manifest(id).await);
        }
        match placement {
            ApprovalPlacement::Exact => {
                let owned: Vec<_> = manifests.values().cloned().collect();
                aithos_gateway::core_bridge::owner_enroll_servers(
                    &master,
                    CONTEXT,
                    &agent_pub,
                    &gateway_pub,
                    &owned,
                    context_store.clone(),
                    &GatewayWorld::window(),
                    CONTROL_NOW,
                    &mut entropy,
                )
                .unwrap();
            }
            ApprovalPlacement::Missing | ApprovalPlacement::AnotherId => {
                owner_grant_context(
                    &master,
                    CONTEXT,
                    &agent_pub,
                    &gateway_pub,
                    &[],
                    context_store.clone(),
                    &GatewayWorld::window(),
                    CONTROL_NOW,
                    &mut entropy,
                )
                .unwrap();
                if placement == ApprovalPlacement::AnotherId {
                    let other = approved_manifest("calendar-other").await;
                    owner_enroll_server(
                        &master,
                        CONTEXT,
                        &agent_pub,
                        &gateway_pub,
                        &other,
                        context_store.clone(),
                        &GatewayWorld::window(),
                        CONTROL_NOW,
                        &mut entropy,
                    )
                    .unwrap();
                }
            }
            ApprovalPlacement::AnotherContext => {
                owner_grant_context(
                    &master,
                    CONTEXT,
                    &agent_pub,
                    &gateway_pub,
                    &[],
                    context_store.clone(),
                    &GatewayWorld::window(),
                    CONTROL_NOW,
                    &mut entropy,
                )
                .unwrap();
                owner_init_context(
                    &master,
                    NEIGHBOR_CONTEXT,
                    neighbor_store.clone(),
                    CONTROL_NOW,
                    &mut entropy,
                )
                .unwrap();
                let owned: Vec<_> = ids
                    .iter()
                    .map(|id| manifests.get(*id).unwrap().clone())
                    .collect();
                aithos_gateway::core_bridge::owner_enroll_servers(
                    &master,
                    NEIGHBOR_CONTEXT,
                    &agent_pub,
                    &gateway_pub,
                    &owned,
                    neighbor_store.clone(),
                    &GatewayWorld::window(),
                    CONTROL_NOW,
                    &mut entropy,
                )
                .unwrap();
            }
        }
        let config_grant = owner_grant_connector_config(
            &master,
            CONTEXT,
            CONNECTOR,
            context_store.clone(),
            &GatewayWorld::window(),
            CONTROL_NOW,
            &mut entropy,
        )
        .unwrap();
        owner_init_journal(
            &master,
            "g7b-agent",
            &agent_pub,
            &gateway_pub,
            None,
            journal_store.clone(),
            &GatewayWorld::window(),
            CONTROL_NOW,
            &mut entropy,
        )
        .unwrap();

        let quote =
            |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
        let servers = ids
            .iter()
            .map(|id| {
                format!(
                    "  - name: {id}\n    transport: http\n    url: {upstream_base}/mcp\n    oauth:\n      auth_url: {upstream_base}/authorize\n      token_url: {upstream_base}/token\n      client_id: aithos-enterprise\n      client_secret: {{ broker: enterprise, path: template/{id}/client, field: value }}\n      scopes: [calendar.read]\n      redirect_uri: {callback_url}\n      token_vault: {{ broker: enterprise, path: template/{id}/token, field: value }}\n"
                )
            })
            .collect::<String>();
        let contexts = if placement == ApprovalPlacement::AnotherContext {
            format!(
                "  - name: {CONTEXT}\n    store: {{ kind: fs, root: {} }}\n  - name: {NEIGHBOR_CONTEXT}\n    store: {{ kind: fs, root: {} }}\n",
                quote(&context_root),
                quote(&neighbor_root)
            )
        } else {
            format!(
                "  - name: {CONTEXT}\n    store: {{ kind: fs, root: {} }}\n",
                quote(&context_root)
            )
        };
        let yaml = format!(
            "listen: 127.0.0.1:4870\ndashboard:\n  allowed_origins: [{CONTROL_ORIGIN}]\ncredential_brokers:\n  enterprise:\n    kind: vault-kv2\n    address: http://127.0.0.1:8200\n    mount: secret\n    auth: {{ kind: token-env, env: AITHOS_G7B_TEST_VAULT_TOKEN }}\nservers:\n{servers}contexts:\n{contexts}journal:\n  store: {{ kind: fs, root: {} }}\n",
            quote(&journal_root)
        );
        let config = GatewayConfig::from_yaml(&yaml).unwrap();
        let runner = Arc::new(Mutex::new(
            Runner::open(&config, keyholder, || Box::new(SeqEntropy::default())).unwrap(),
        ));
        if placement == ApprovalPlacement::Exact {
            for id in ids {
                runner
                    .lock()
                    .await
                    .approved_connector(CONTEXT, id)
                    .unwrap_or_else(|error| {
                        panic!("sealed approval `{id}` did not reopen: {error}")
                    });
            }
        }
        let vault = Arc::new(TrackingVault::default());
        let brokers: BTreeMap<String, Arc<dyn CredentialBroker>> = BTreeMap::from([(
            "enterprise".to_owned(),
            vault.clone() as Arc<dyn CredentialBroker>,
        )]);
        let oauth = Arc::new(UpstreamOAuthRegistry::from_config(&config, &brokers).unwrap());
        let dynamic = empty_dynamic_upstreams();
        let control = Arc::new(
            ConnectorControl::from_config(
                &config,
                Arc::clone(&runner),
                Arc::clone(&dynamic),
                Arc::clone(&oauth),
                brokers.clone(),
            )
            .unwrap()
            .with_clock(Arc::new(|| CONTROL_NOW.to_owned())),
        );
        let callback_app = upstream_oauth::router(Arc::clone(&oauth));
        let callback_task = tokio::spawn(async move {
            axum::serve(callback_listener, callback_app).await.unwrap();
        });

        let control_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = control_listener.local_addr().unwrap();
        let host = address.to_string();
        let mut proof_stores = BTreeMap::from([(CONTEXT.to_owned(), context_store.clone())]);
        if placement == ApprovalPlacement::AnotherContext {
            proof_stores.insert(NEIGHBOR_CONTEXT.to_owned(), neighbor_store);
        }
        let reader = ControlProofReader::from_stores(proof_stores).unwrap();
        let dashboard = DashboardConfig {
            allowed_origins: vec![CONTROL_ORIGIN.to_owned()],
        };
        let state = ControlState::new(
            reader,
            &dashboard,
            [host.clone()],
            RelayHealth::new(RelayReadiness::Ready),
            brokers,
        )
        .unwrap()
        .with_clock(Arc::new(|| CONTROL_NOW_MS))
        .with_connectors(Arc::clone(&control));
        let control_task = tokio::spawn(async move {
            axum::serve(control_listener, control::router(Arc::new(state)))
                .await
                .unwrap();
        });
        let owner_master =
            aithos_core::derive::derive_key("aithos-gw/v1/context/operations", &master);
        let owner = aithos_core::keys::OwnerKeys::genesis(
            &aithos_core::keys::MasterSeed::from_bytes(owner_master),
        )
        .content_sign;
        let config_seed: [u8; 32] = hex::decode(config_grant.seed_hex)
            .unwrap()
            .try_into()
            .unwrap();
        let config_signing = SigningKey::from_bytes(&config_seed);
        let config_key =
            aithos_core::wire::ed25519_pub_to_multibase(&config_signing.verifying_key().to_bytes());
        let router = Arc::new(McpRouter::<HttpUpstream> {
            runner: Arc::clone(&runner),
            upstreams: BTreeMap::new(),
            dynamic_upstreams: Arc::clone(&dynamic),
            clock: Arc::new(|| CONTROL_NOW.to_owned()),
            session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
            oauth: None,
        });
        *wire.gamma_store.lock().unwrap() = Some(context_store.clone());
        let descriptors = ids
            .iter()
            .map(|id| {
                let manifest = manifests.get(*id).unwrap();
                let digest = approved_manifest_catalog_digest(manifest).unwrap();
                (
                    (*id).to_owned(),
                    ConnectorStageRequest {
                        v: 1,
                        id: (*id).to_owned(),
                        context: CONTEXT.to_owned(),
                        endpoint: format!("{upstream_base}/mcp"),
                        transport: ConnectorTransport::StreamableHttp,
                        oauth: ConnectorOAuthDescriptor {
                            authorization_endpoint: format!("{upstream_base}/authorize"),
                            token_endpoint: format!("{upstream_base}/token"),
                            client_id: "aithos-enterprise".to_owned(),
                            scopes: vec!["calendar.read".to_owned()],
                            redirect_uri: callback_url.clone(),
                            client_secret_record: format!("{id}-client"),
                            pending_record: format!("{id}-pending"),
                            token_record: format!("{id}-token"),
                        },
                        approved_manifest: ApprovedManifestRef {
                            id: (*id).to_owned(),
                            pin: digest,
                        },
                    },
                )
            })
            .collect();
        Self {
            _scratch: scratch,
            config,
            runner,
            router,
            control,
            oauth,
            dynamic,
            vault,
            wire,
            base: format!("http://{address}"),
            host,
            callback_url,
            client: reqwest::Client::new(),
            owner,
            config_authority: G7bControlAuthority {
                signing: config_signing,
                key: config_key,
                mandate: config_grant.mandate,
            },
            descriptors,
            context_store,
            journal_store,
            registry_path: journal_root
                .join(".aithos-sidecar")
                .join("gateway/connectors.json"),
            servers: vec![upstream_task, callback_task, control_task],
            nonce: AtomicUsize::new(1),
            last: None,
            captures: Vec::new(),
            defect: None,
            pending_body: None,
            pending_state: None,
            facts: BTreeMap::new(),
            public_blobs: Vec::new(),
            atomic_reads: Vec::new(),
        }
    }

    fn descriptor(&self, id: &str) -> ConnectorStageRequest {
        self.descriptors.get(id).unwrap().clone()
    }

    fn next_nonce(&self, prefix: &str) -> String {
        format!("{prefix}-{:016}", self.nonce.fetch_add(1, Ordering::SeqCst))
    }

    fn signed_header(&self, method: &str, path: &str, body: &[u8], owner: bool) -> String {
        let (signing, key, mandates) = if owner {
            (&self.owner, "#content", Vec::new())
        } else {
            (
                &self.config_authority.signing,
                self.config_authority.key.as_str(),
                vec![self.config_authority.mandate.clone()],
            )
        };
        a2_header_value(
            &a2_sign_envelope(
                A2Envelope {
                    v: 1,
                    host: self.host.clone(),
                    method: method.to_owned(),
                    path: path.to_owned(),
                    body_b3: if body.is_empty() {
                        String::new()
                    } else {
                        blake3::hash(body).to_hex().to_string()
                    },
                    at: CONTROL_NOW.to_owned(),
                    nonce: self.next_nonce("g7b"),
                    mandate: mandates,
                    key: key.to_owned(),
                    signature: A2EnvelopeSignature {
                        alg: "ed25519".to_owned(),
                        value: String::new(),
                    },
                },
                signing,
            )
            .unwrap(),
        )
        .unwrap()
    }

    async fn send(&mut self, method: &str, path: &str, body: Vec<u8>, owner: bool) -> HttpCapture {
        let auth = self.signed_header(method, path, &body, owner);
        let request = self
            .client
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
                format!("{}{}", self.base, path),
            )
            .header("Origin", CONTROL_ORIGIN)
            .header("X-Aithos-Auth", auth);
        let response = if body.is_empty() {
            request.send().await.unwrap()
        } else {
            request
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .unwrap()
        };
        let capture = control_capture(response).await;
        self.public_blobs.push(capture.text());
        self.captures.push(capture.clone());
        self.last = Some(capture.clone());
        capture
    }

    async fn stage_request(
        &mut self,
        id: &str,
        descriptor: ConnectorStageRequest,
        owner: bool,
    ) -> HttpCapture {
        self.send(
            "POST",
            &format!("/control/v1/connectors/{id}/stage"),
            serde_json::to_vec(&descriptor).unwrap(),
            owner,
        )
        .await
    }

    async fn stage(&mut self, id: &str) -> HttpCapture {
        self.stage_request(id, self.descriptor(id), false).await
    }

    async fn secret(&mut self, id: &str) -> HttpCapture {
        self.send(
            "PUT",
            &format!("/control/v1/connectors/{id}/client-secret"),
            serde_json::to_vec(&json!({ "client_secret": CLIENT_SECRET })).unwrap(),
            false,
        )
        .await
    }

    async fn oauth_start(&mut self, id: &str) -> HttpCapture {
        self.send(
            "POST",
            &format!("/control/v1/connectors/{id}/oauth/start"),
            Vec::new(),
            false,
        )
        .await
    }

    async fn oauth_status(&mut self, id: &str, owner: bool) -> HttpCapture {
        self.send(
            "GET",
            &format!("/control/v1/connectors/{id}/oauth/status"),
            Vec::new(),
            owner,
        )
        .await
    }

    async fn activate(&mut self, id: &str) -> HttpCapture {
        self.send(
            "POST",
            &format!("/control/v1/connectors/{id}/activate"),
            Vec::new(),
            false,
        )
        .await
    }

    async fn delete(&mut self, id: &str) -> HttpCapture {
        self.send(
            "DELETE",
            &format!("/control/v1/connectors/{id}/draft"),
            Vec::new(),
            false,
        )
        .await
    }

    async fn list(&mut self) -> HttpCapture {
        self.send("GET", "/control/v1/connectors", Vec::new(), true)
            .await
    }

    async fn connect(&mut self, id: &str) {
        assert_eq!(self.stage(id).await.status, 201);
        assert_eq!(self.secret(id).await.status, 200);
        let state = self.begin_pending(id).await;
        let response = self
            .client
            .get(&self.callback_url)
            .query(&[("code", CALLBACK_CODE), ("state", state.as_str())])
            .send()
            .await
            .unwrap();
        let callback = control_capture(response).await;
        assert_eq!(callback.status, 200, "{}", callback.text());
        self.public_blobs.push(callback.text());
        self.captures.push(callback);
    }

    async fn begin_pending(&mut self, id: &str) -> String {
        let start = self.oauth_start(id).await;
        assert_eq!(start.status, 200, "{}", start.text());
        let consent =
            reqwest::Url::parse(start.json()["authorization_url"].as_str().unwrap()).unwrap();
        let state = consent
            .query_pairs()
            .find(|(name, _)| name == "state")
            .map(|(_, value)| value.into_owned())
            .unwrap();
        self.pending_state = Some(state.clone());
        state
    }

    async fn callback(&mut self, query: &[(&str, &str)]) -> HttpCapture {
        let response = self
            .client
            .get(&self.callback_url)
            .query(query)
            .send()
            .await
            .unwrap();
        let capture = control_capture(response).await;
        self.public_blobs.push(capture.text());
        self.captures.push(capture.clone());
        self.last = Some(capture.clone());
        capture
    }

    fn expire_token(&self, id: &str) {
        let path = Self::token_path(id);
        let mut record: Value = serde_json::from_str(&self.vault.value(&path).unwrap()).unwrap();
        record["expires_at"] = Value::from(0);
        self.vault
            .put(&path, serde_json::to_string(&record).unwrap());
    }

    async fn connect_and_activate(&mut self, id: &str) {
        self.connect(id).await;
        let activated = self.activate(id).await;
        assert_eq!(activated.status, 200, "{}", activated.text());
    }

    async fn connect_as_owner(&mut self, id: &str) {
        let descriptor = self.descriptor(id);
        assert_eq!(self.stage_request(id, descriptor, true).await.status, 201);
        assert_eq!(
            self.send(
                "PUT",
                &format!("/control/v1/connectors/{id}/client-secret"),
                serde_json::to_vec(&json!({ "client_secret": CLIENT_SECRET })).unwrap(),
                true,
            )
            .await
            .status,
            200
        );
        let start = self
            .send(
                "POST",
                &format!("/control/v1/connectors/{id}/oauth/start"),
                Vec::new(),
                true,
            )
            .await;
        assert_eq!(start.status, 200, "{}", start.text());
        let consent =
            reqwest::Url::parse(start.json()["authorization_url"].as_str().unwrap()).unwrap();
        let state = consent
            .query_pairs()
            .find(|(name, _)| name == "state")
            .map(|(_, value)| value.into_owned())
            .unwrap();
        assert_eq!(
            self.callback(&[("code", CALLBACK_CODE), ("state", state.as_str())])
                .await
                .status,
            200
        );
    }

    async fn activate_as_owner(&mut self, id: &str) -> HttpCapture {
        self.send(
            "POST",
            &format!("/control/v1/connectors/{id}/activate"),
            Vec::new(),
            true,
        )
        .await
    }

    fn pending_path(id: &str) -> String {
        format!("aithos/connectors/{id}/{id}-pending")
    }

    fn token_path(id: &str) -> String {
        format!("aithos/connectors/{id}/{id}-token")
    }

    fn secret_path(id: &str) -> String {
        format!("aithos/connectors/{id}/{id}-client")
    }

    fn registry_text(&self) -> String {
        std::fs::read_to_string(&self.registry_path).unwrap()
    }

    async fn tools_list(&self) -> Value {
        process_multi(
            &self.router,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        )
        .await
    }

    async fn tool_call(&self, name: &str, arguments: Value) -> Value {
        process_multi(
            &self.router,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments },
            }),
        )
        .await
    }

    fn public_surface(&self) -> String {
        let mut blobs = self.public_blobs.clone();
        if self.registry_path.exists() {
            blobs.push(self.registry_text());
        }
        for store in [&self.context_store, &self.journal_store] {
            for path in store.list("").unwrap() {
                if let Some(bytes) = store.get(&path).unwrap() {
                    blobs.push(String::from_utf8_lossy(&bytes).into_owned());
                }
            }
        }
        blobs.join("\n")
    }
}

fn harness(w: &GatewayWorld) -> &G7bHarness {
    w.g7b.as_ref().expect("G7b harness")
}

fn harness_mut(w: &mut GatewayWorld) -> &mut G7bHarness {
    w.g7b.as_mut().expect("G7b harness")
}

#[given(regex = r"^a signed config request containing (.+)$")]
async fn g7b_invalid_descriptor(w: &mut GatewayWorld, defect: String) {
    let mut h = G7bHarness::exact().await;
    let mut body = serde_json::to_value(h.descriptor(CONNECTOR)).unwrap();
    match defect.as_str() {
        "an invalid connector id" => body["id"] = Value::String("INVALID/ID".to_owned()),
        "a browser-selected Vault path" => {
            body["oauth"]["vault_path"] = Value::String("root/browser-selected".to_owned())
        }
        "a non-HTTPS non-loopback endpoint" => {
            body["endpoint"] = Value::String("http://mcp.evil.example/mcp".to_owned())
        }
        "scopes outside the approved set" => {
            body["oauth"]["scopes"] = json!(["calendar.read", "calendar.write"])
        }
        "an unknown JSON field" => body["surprise"] = Value::Bool(true),
        "an unsupported transport" => body["transport"] = Value::String("stdio".to_owned()),
        "a redirect URI different from callback" => {
            body["oauth"]["redirect_uri"] =
                Value::String("https://neighbor.example/oauth/callback".to_owned())
        }
        other => panic!("unknown input defect: {other}"),
    }
    h.defect = Some(defect);
    h.pending_body = Some(serde_json::to_vec(&body).unwrap());
    w.g7b = Some(h);
}

#[when("the owner stages the connector instance")]
async fn g7b_owner_stages_invalid(w: &mut GatewayWorld) {
    if oac0_steps::stage_connector_identity_for_shared_step(w) {
        return;
    }
    let h = harness_mut(w);
    let body = h.pending_body.clone().unwrap();
    h.send(
        "POST",
        &format!("/control/v1/connectors/{CONNECTOR}/stage"),
        body,
        true,
    )
    .await;
}

#[then("staging is refused with a stable redacted error")]
fn g7b_invalid_stage_is_stable(w: &mut GatewayWorld) {
    if oac0_steps::assert_connector_identity_refused_for_shared_step(w) {
        return;
    }
    let response = harness(w).last.as_ref().unwrap();
    assert_eq!(response.status, 403);
    assert_eq!(
        response.json(),
        json!({ "error": "connector_not_approved" })
    );
    assert!(!response.text().contains(INTERNAL_SENTINEL));
}

#[then("Vault, the local registry and upstream receive zero requests")]
fn g7b_invalid_stage_has_zero_effect(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.vault.request_count(), 0);
    assert!(!h.registry_path.exists());
    assert!(h.wire.token_grants.lock().unwrap().is_empty());
    assert!(h.wire.mcp_requests.lock().unwrap().is_empty());
}

#[given(regex = r"^the requested connector has (.+)$")]
async fn g7b_approval_defect(w: &mut GatewayWorld, defect: String) {
    let placement = match defect.as_str() {
        "no sealed manifest" => ApprovalPlacement::Missing,
        "a mismatched manifest pin" => ApprovalPlacement::Exact,
        "a manifest for another id" => ApprovalPlacement::AnotherId,
        "a manifest in another context" => ApprovalPlacement::AnotherContext,
        other => panic!("unknown approval defect: {other}"),
    };
    let mut h = G7bHarness::provision(&[CONNECTOR], placement).await;
    let mut descriptor = h.descriptor(CONNECTOR);
    if defect == "a mismatched manifest pin" {
        descriptor.approved_manifest.pin = "sha256:mismatched".to_owned();
    }
    h.defect = Some(defect);
    h.pending_body = Some(serde_json::to_vec(&descriptor).unwrap());
    w.g7b = Some(h);
}

#[when("a correctly mandated config authority stages it")]
async fn g7b_config_authority_stages_defect(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let body = h.pending_body.clone().unwrap();
    h.send(
        "POST",
        &format!("/control/v1/connectors/{CONNECTOR}/stage"),
        body,
        false,
    )
    .await;
}

#[then("the connector is refused as not approved")]
fn g7b_not_approved(w: &mut GatewayWorld) {
    let response = harness(w).last.as_ref().unwrap();
    assert_eq!(response.status, 403, "{}", response.text());
    assert_eq!(response.json()["error"], "connector_not_approved");
}

#[then("no draft, secret record or upstream request is created")]
fn g7b_not_approved_has_zero_effect(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(!h.registry_path.exists());
    assert!(h.vault.values.lock().unwrap().is_empty());
    assert!(h.wire.mcp_requests.lock().unwrap().is_empty());
    assert!(h.wire.token_grants.lock().unwrap().is_empty());
}

#[given("a sealed approved manifest and the exact connector config mandate")]
async fn g7b_valid_descriptor(w: &mut GatewayWorld) {
    w.g7b = Some(G7bHarness::exact().await);
}

#[when("the instance descriptor is staged")]
async fn g7b_stage_valid_descriptor(w: &mut GatewayWorld) {
    let response = harness_mut(w).stage(CONNECTOR).await;
    assert_eq!(response.status, 201, "{}", response.text());
}

#[then(expr = "a versioned non-secret draft is atomically persisted in {string}")]
fn g7b_draft_is_durable(w: &mut GatewayWorld, suffix: String) {
    let h = harness(w);
    assert!(h.registry_path.ends_with(suffix));
    let registry: Value = serde_json::from_str(&h.registry_text()).unwrap();
    assert_eq!(registry["v"], 1);
    assert_eq!(registry["connectors"].as_array().unwrap().len(), 1);
    let draft = &registry["connectors"][0];
    assert_eq!(draft["id"], CONNECTOR);
    assert_eq!(draft["state"], "draft");
    assert_eq!(draft["active"], false);
    for secret in [
        CLIENT_SECRET,
        ACCESS_ONE,
        ACCESS_TWO,
        REFRESH_ONE,
        REFRESH_TWO,
    ] {
        assert!(!h.registry_text().contains(secret));
    }
}

#[then("the connector is not visible in runtime tools")]
async fn g7b_draft_not_in_runtime(w: &mut GatewayWorld) {
    let tools = harness(w).tools_list().await;
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(!names.iter().any(|name| name.starts_with("calendar_safe__")));
}

#[then("no gateway registry record is sent to RemoteStore")]
fn g7b_sidecar_is_local_only(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(matches!(
        h.config.journal.as_ref().unwrap().store,
        aithos_gateway::config::StoreConfig::Fs { .. }
    ));
    assert!(h.registry_path.exists());
}

#[given("an approved inactive connector draft")]
async fn g7b_inactive_draft(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    assert_eq!(h.stage(CONNECTOR).await.status, 201);
    w.g7b = Some(h);
}

#[when("the browser sends a bounded client secret over gateway public TLS")]
async fn g7b_browser_sets_secret(w: &mut GatewayWorld) {
    let response = harness_mut(w).secret(CONNECTOR).await;
    assert_eq!(response.status, 200, "{}", response.text());
    harness_mut(w)
        .facts
        .insert("secret_scope_ended".to_owned(), Value::Bool(true));
}

#[then("the gateway writes that secret exactly once to its derived Vault record")]
fn g7b_secret_written_once(w: &mut GatewayWorld) {
    let h = harness(w);
    let path = G7bHarness::secret_path(CONNECTOR);
    assert_eq!(h.vault.store_count_for(&path), 1);
    assert_eq!(h.vault.value(&path).as_deref(), Some(CLIENT_SECRET));
    assert!(!path.contains("browser"));
}

#[then("the Rust secret buffer is immediately zeroized")]
fn g7b_secret_buffer_scope_ended(w: &mut GatewayWorld) {
    // At this point both non-cloneable wrappers (`ClientSecretBody` then
    // `SecretValue`) have left their lexical scopes. Their Drop impls are the
    // production zeroization boundary; only the broker's Vault value remains.
    assert_eq!(
        harness(w).facts.get("secret_scope_ended"),
        Some(&Value::Bool(true))
    );
}

#[then("the connector remains inactive")]
async fn g7b_connector_remains_inactive(w: &mut GatewayWorld) {
    let response = harness_mut(w).list().await;
    let item = &response.json()["items"][0];
    assert_eq!(item["active"], false);
    assert_eq!(item["state"], "disconnected");
}

#[then("the secret sentinel is absent from responses, registry, proof, logs and upstream")]
fn g7b_secret_is_vault_only(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(!h.public_surface().contains(CLIENT_SECRET));
    assert!(
        !serde_json::to_string(&*h.wire.mcp_requests.lock().unwrap())
            .unwrap()
            .contains(CLIENT_SECRET)
    );
    assert_eq!(
        h.vault
            .value(&G7bHarness::secret_path(CONNECTOR))
            .as_deref(),
        Some(CLIENT_SECRET)
    );
}

#[given("an approved inactive connector draft and an unavailable Vault")]
async fn g7b_draft_vault_unavailable(w: &mut GatewayWorld) {
    g7b_inactive_draft(w).await;
    harness(w).vault.fail_all.store(true, Ordering::SeqCst);
    let before = harness(w).tools_list().await;
    harness_mut(w).facts.insert("tools_before".into(), before);
}

#[when("the browser submits its client secret")]
async fn g7b_browser_submits_secret(w: &mut GatewayWorld) {
    harness_mut(w).secret(CONNECTOR).await;
}

#[then("the request fails as secret unavailable")]
fn g7b_secret_unavailable(w: &mut GatewayWorld) {
    let response = harness(w).last.as_ref().unwrap();
    assert_eq!(response.status, 503);
    assert_eq!(response.json(), json!({ "error": "secret_unavailable" }));
    assert!(!response.text().contains(INTERNAL_SENTINEL));
}

#[then("the draft remains disconnected and the runtime router is unchanged")]
async fn g7b_vault_failure_is_inactive(w: &mut GatewayWorld) {
    let after = harness(w).tools_list().await;
    assert_eq!(harness(w).facts.get("tools_before").unwrap(), &after);
    let registry: Value = serde_json::from_str(&harness(w).registry_text()).unwrap();
    assert_eq!(registry["connectors"][0]["active"], false);
    assert!(matches!(
        registry["connectors"][0]["state"].as_str(),
        Some("draft" | "disconnected")
    ));
}

#[given("an inactive draft whose broker cannot safely delete its Vault records")]
async fn g7b_residual_vault_draft(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    assert_eq!(h.stage(CONNECTOR).await.status, 201);
    assert_eq!(h.secret(CONNECTOR).await.status, 200);
    w.g7b = Some(h);
}

#[when("a valid config authority deletes the draft")]
async fn g7b_delete_draft(w: &mut GatewayWorld) {
    harness_mut(w).delete(CONNECTOR).await;
}

#[then("every runtime reference is disabled")]
async fn g7b_deleted_runtime_disabled(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(!h.dynamic.read().unwrap().contains_key(CONNECTOR));
    assert!(h.oauth.get(CONNECTOR).is_none());
    let tools = h.tools_list().await;
    assert!(!tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == hub_exposed_name(CONNECTOR, "events.list")));
}

#[then("the residual Vault record is reported only as a non-secret cleanup limitation")]
fn g7b_delete_reports_cleanup_limitation(w: &mut GatewayWorld) {
    let h = harness(w);
    let response = h.last.as_ref().unwrap();
    assert_eq!(response.status, 204);
    assert_eq!(
        response.header("x-aithos-cleanup-limitation"),
        Some("vault-records-retained")
    );
    assert_eq!(
        response.header("access-control-expose-headers"),
        Some("X-Aithos-Cleanup-Limitation")
    );
    assert_eq!(
        h.vault
            .value(&G7bHarness::secret_path(CONNECTOR))
            .as_deref(),
        Some(CLIENT_SECRET)
    );
    assert!(!response.text().contains(CLIENT_SECRET));
}

#[given("an approved draft with its client secret in Vault")]
async fn g7b_draft_with_secret(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    assert_eq!(h.stage(CONNECTOR).await.status, 201);
    assert_eq!(h.secret(CONNECTOR).await.status, 200);
    h.vault.clear_counts();
    w.g7b = Some(h);
}

#[when("the browser starts upstream OAuth")]
async fn g7b_start_oauth(w: &mut GatewayWorld) {
    harness_mut(w).oauth_start(CONNECTOR).await;
}

#[then("the existing upstream OAuth registry returns only consent URL and expiry")]
fn g7b_oauth_start_is_public_only(w: &mut GatewayWorld) {
    let response = harness(w).last.as_ref().unwrap();
    assert_eq!(response.status, 200, "{}", response.text());
    let body = response.json();
    let keys: BTreeSet<_> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["authorization_url", "expires_at", "v"])
    );
    assert_eq!(body["v"], 1);
    assert!(reqwest::Url::parse(body["authorization_url"].as_str().unwrap()).is_ok());
    assert!(body["expires_at"].as_str().unwrap().ends_with('Z'));
}

#[then("PKCE verifier and state live only in Vault")]
fn g7b_pending_custody_is_vault_only(w: &mut GatewayWorld) {
    let h = harness(w);
    let pending = h.vault.value(&G7bHarness::pending_path(CONNECTOR)).unwrap();
    let record: Value = serde_json::from_str(&pending).unwrap();
    assert_eq!(record["status"], "pending");
    assert!(record["state"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(record["code_verifier"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(h.vault.value(&G7bHarness::token_path(CONNECTOR)).is_none());
    let public = h.public_surface();
    assert!(!public.contains(record["code_verifier"].as_str().unwrap()));
}

#[then("the response is no-store and contains no secret or token")]
fn g7b_oauth_start_is_no_store(w: &mut GatewayWorld) {
    let response = harness(w).last.as_ref().unwrap();
    assert_eq!(response.header("cache-control"), Some("no-store"));
    for secret in [
        CLIENT_SECRET,
        ACCESS_ONE,
        ACCESS_TWO,
        REFRESH_ONE,
        REFRESH_TWO,
    ] {
        assert!(!response.text().contains(secret));
    }
}

#[given("a pending upstream OAuth attempt in Vault")]
async fn g7b_pending_attempt(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    assert_eq!(h.stage(CONNECTOR).await.status, 201);
    assert_eq!(h.secret(CONNECTOR).await.status, 200);
    let state = h.begin_pending(CONNECTOR).await;
    assert!(h
        .vault
        .value(&G7bHarness::pending_path(CONNECTOR))
        .is_some());
    h.pending_state = Some(state);
    w.g7b = Some(h);
}

#[when("the callback carries the approved code and matching one-shot state")]
async fn g7b_valid_callback(w: &mut GatewayWorld) {
    let state = harness(w).pending_state.clone().unwrap();
    let callback = harness_mut(w)
        .callback(&[("code", CALLBACK_CODE), ("state", state.as_str())])
        .await;
    harness_mut(w)
        .facts
        .insert("callback_text".into(), Value::String(callback.text()));
}

#[then("the existing upstream OAuth registry stores the token set in Vault")]
fn g7b_token_set_is_in_vault(w: &mut GatewayWorld) {
    let h = harness(w);
    let connected = h.vault.value(&G7bHarness::token_path(CONNECTOR)).unwrap();
    let record: Value = serde_json::from_str(&connected).unwrap();
    assert_eq!(record["status"], "connected");
    assert_eq!(record["access_token"], ACCESS_ONE);
    assert_eq!(record["refresh_token"], REFRESH_ONE);
    assert_eq!(record["scopes"], json!(["calendar.read"]));
    let pending: Value =
        serde_json::from_str(&h.vault.value(&G7bHarness::pending_path(CONNECTOR)).unwrap())
            .unwrap();
    assert_eq!(pending["status"], "consumed");
}

#[then(expr = "public status becomes {string}")]
async fn g7b_public_status(w: &mut GatewayWorld, expected: String) {
    let response = harness_mut(w).oauth_status(CONNECTOR, false).await;
    assert_eq!(response.status, 200, "{}", response.text());
    assert_eq!(response.json()["state"], expected);
    assert!(!response.text().contains(CLIENT_SECRET));
    for token in [ACCESS_ONE, ACCESS_TWO, REFRESH_ONE, REFRESH_TWO] {
        assert!(!response.text().contains(token));
    }
}

#[then("the callback redirect carries only a generic outcome")]
fn g7b_callback_is_generic(w: &mut GatewayWorld) {
    let text = harness(w)
        .facts
        .get("callback_text")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(
        text,
        "OAuth connection established. You may close this window."
    );
    for secret in [
        CALLBACK_CODE,
        CLIENT_SECRET,
        ACCESS_ONE,
        ACCESS_TWO,
        REFRESH_ONE,
        REFRESH_TWO,
    ] {
        assert!(!text.contains(secret));
    }
}

#[when(
    regex = r"^the callback carries (provider denial|a replayed callback|the wrong state|an expired attempt)$"
)]
async fn g7b_invalid_callback(w: &mut GatewayWorld, defect: String) {
    let state = harness(w).pending_state.clone().unwrap();
    if defect == "a replayed callback" {
        harness(w).vault.put(
            &G7bHarness::pending_path(CONNECTOR),
            serde_json::to_string(&json!({
                "status": "consumed",
                "consumed_at": 1_784_203_200_i64,
            }))
            .unwrap(),
        );
    } else if defect == "an expired attempt" {
        let path = G7bHarness::pending_path(CONNECTOR);
        let mut pending: Value =
            serde_json::from_str(&harness(w).vault.value(&path).unwrap()).unwrap();
        pending["created_at"] = Value::from(0);
        harness(w)
            .vault
            .put(&path, serde_json::to_string(&pending).unwrap());
    }
    let capture = match defect.as_str() {
        "provider denial" => {
            harness_mut(w)
                .callback(&[("error", "access_denied"), ("state", state.as_str())])
                .await
        }
        "a replayed callback" | "an expired attempt" => {
            harness_mut(w)
                .callback(&[("code", CALLBACK_CODE), ("state", state.as_str())])
                .await
        }
        "the wrong state" => {
            harness_mut(w)
                .callback(&[("code", CALLBACK_CODE), ("state", "wrong-state-sentinel")])
                .await
        }
        other => panic!("unknown callback defect: {other}"),
    };
    assert_eq!(capture.status, 400);
    let index = harness(w).captures.len() - 1;
    harness_mut(w)
        .facts
        .insert("invalid_callback_index".into(), Value::from(index as u64));
    harness_mut(w).defect = Some(defect);
}

#[then("OAuth remains fail-closed with a public non-connected state")]
async fn g7b_invalid_callback_stays_closed(w: &mut GatewayWorld) {
    assert!(!harness(w).oauth.is_connected(CONNECTOR).await);
    let response = harness_mut(w).oauth_status(CONNECTOR, false).await;
    assert_eq!(response.status, 200);
    assert!(matches!(
        response.json()["state"].as_str(),
        Some("pending" | "expired" | "unavailable")
    ));
    assert_eq!(response.json()["active"], false);
}

#[then("no code, state, verifier or token appears in any public output")]
fn g7b_invalid_callback_is_redacted(w: &mut GatewayWorld) {
    let h = harness(w);
    let callback_index = h.facts["invalid_callback_index"].as_u64().unwrap() as usize;
    let callback = &h.captures[callback_index];
    let status = h.last.as_ref().unwrap();
    let public = format!("{}\n{}", callback.text(), status.text());
    let pending = h.vault.value(&G7bHarness::pending_path(CONNECTOR));
    let pending: Value = pending
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(Value::Null);
    let mut forbidden = vec![
        CALLBACK_CODE,
        ACCESS_ONE,
        ACCESS_TWO,
        REFRESH_ONE,
        REFRESH_TWO,
        "wrong-state-sentinel",
    ];
    if let Some(state) = pending["state"].as_str() {
        forbidden.push(state);
    }
    if let Some(verifier) = pending["code_verifier"].as_str() {
        forbidden.push(verifier);
    }
    for secret in forbidden {
        assert!(
            !public.contains(secret),
            "public callback/status leaked {secret}"
        );
    }
}

#[given("a connected connector with an expired access token and valid refresh token")]
async fn g7b_expired_connected(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    h.connect(CONNECTOR).await;
    h.expire_token(CONNECTOR);
    h.wire.clear_mcp();
    w.g7b = Some(h);
}

#[given("a connected connector whose refresh is refused")]
async fn g7b_refused_refresh(w: &mut GatewayWorld) {
    g7b_expired_connected(w).await;
    harness(w).wire.refuse_refresh.store(true, Ordering::SeqCst);
}

#[when("activation requests authenticated discovery")]
async fn g7b_activation_discovery(w: &mut GatewayWorld) {
    let response = harness_mut(w).activate(CONNECTOR).await;
    harness_mut(w)
        .facts
        .insert("activation_status".into(), Value::from(response.status));
    harness_mut(w)
        .facts
        .insert("activation_body".into(), response.json());
}

#[then("the existing upstream OAuth registry performs one refresh")]
fn g7b_one_refresh(w: &mut GatewayWorld) {
    assert_eq!(harness(w).wire.refresh_count(), 1);
}

#[then("discovery receives only the rotated bearer")]
fn g7b_discovery_uses_rotated_bearer(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.wire.list_count(), 1);
    let bearers = h.wire.bearers.lock().unwrap();
    assert!(!bearers.is_empty());
    let expected = format!("Bearer {ACCESS_TWO}");
    assert!(bearers
        .iter()
        .all(|bearer| bearer.as_deref() == Some(expected.as_str())));
    assert!(!serde_json::to_string(&*bearers)
        .unwrap()
        .contains(ACCESS_ONE));
}

#[then("the protected MCP receives zero requests")]
fn g7b_protected_mcp_not_called(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.wire.mcp_requests.lock().unwrap().len(), 0);
    assert_eq!(h.wire.refresh_count(), 1);
    assert_eq!(h.facts["activation_status"], 503);
    assert_eq!(h.facts["activation_body"]["error"], "oauth_unavailable");
}

#[given("a connected approved connector whose live tools match every approved pin")]
async fn g7b_connected_matching_catalogue(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    h.connect(CONNECTOR).await;
    let before = h.tools_list().await;
    h.facts.insert("tools_before".into(), before);
    h.wire.clear_mcp();
    h.vault.clear_counts();
    w.g7b = Some(h);
}

#[when("a valid config authority activates it")]
async fn g7b_config_authority_activates(w: &mut GatewayWorld) {
    harness_mut(w).activate(CONNECTOR).await;
}

#[then("discovery runs once with the Vault bearer")]
fn g7b_discovery_once(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.wire.list_count(), 1);
    let bearers = h.wire.bearers.lock().unwrap();
    assert_eq!(bearers.len(), 1);
    let expected = format!("Bearer {ACCESS_ONE}");
    assert_eq!(bearers[0].as_deref(), Some(expected.as_str()));
    assert!(h.vault.request_count() >= 1);
}

#[then("a complete registry record is atomically persisted")]
fn g7b_active_registry_complete(w: &mut GatewayWorld) {
    let h = harness(w);
    let registry: Value = serde_json::from_str(&h.registry_text()).unwrap();
    let connector = &registry["connectors"][0];
    assert_eq!(connector["state"], "connected");
    assert_eq!(connector["active"], true);
    assert!(connector["live_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert_eq!(connector["approved_manifest"]["id"], CONNECTOR);
    assert!(h.dynamic.read().unwrap().contains_key(CONNECTOR));
}

#[then("the approved tools become visible without restarting the gateway")]
async fn g7b_tools_hot_visible(w: &mut GatewayWorld) {
    let h = harness(w);
    let tools = h.tools_list().await;
    let names: BTreeSet<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(hub_exposed_name(CONNECTOR, "events.list").as_str()));
    assert!(!names.contains(hub_exposed_name(CONNECTOR, "events.delete").as_str()));
}

#[given(regex = r"^a connected approved connector with (.+)$")]
async fn g7b_catalogue_drift(w: &mut GatewayWorld, drift: String) {
    let mut h = G7bHarness::exact().await;
    h.connect(CONNECTOR).await;
    let before = h.tools_list().await;
    h.facts.insert("tools_before".into(), before);
    let mut catalogue = approved_tools();
    match drift.as_str() {
        "an added tool" => catalogue.push(json!({
            "name": "events.export",
            "description": "Unexpected export",
            "inputSchema": { "type": "object", "additionalProperties": false },
        })),
        "a removed tool" => catalogue.retain(|tool| tool["name"] != "events.delete"),
        "a modified input schema" => {
            catalogue[0]["inputSchema"]["properties"]["calendar_id"]["type"] =
                Value::String("array".to_owned());
        }
        "a modified digest" => {
            catalogue[0]["description"] = Value::String("Drifted description".to_owned());
        }
        other => panic!("unknown catalogue drift: {other}"),
    }
    *h.wire.catalogue.lock().unwrap() = catalogue;
    h.wire.clear_mcp();
    h.defect = Some(drift);
    w.g7b = Some(h);
}

#[then("activation is refused as manifest drift")]
fn g7b_activation_drift_refused(w: &mut GatewayWorld) {
    let response = harness(w).last.as_ref().unwrap();
    assert_eq!(response.status, 409, "{}", response.text());
    assert_eq!(response.json(), json!({ "error": "manifest_drift" }));
    assert!(!response.text().contains(INTERNAL_SENTINEL));
}

#[then("the connector exposes zero tools and the previous runtime remains intact")]
async fn g7b_drift_does_not_swap_runtime(w: &mut GatewayWorld) {
    let h = harness(w);
    let after = h.tools_list().await;
    assert_eq!(h.facts.get("tools_before"), Some(&after));
    assert!(!after["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == hub_exposed_name(CONNECTOR, "events.list")));
    assert!(!h.dynamic.read().unwrap().contains_key(CONNECTOR));
}

#[given("one active registry and one validated replacement")]
async fn g7b_atomic_registry_fixture(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    h.connect_and_activate(CONNECTOR).await;
    let old = std::fs::read(&h.registry_path).unwrap();
    let mut replacement: Value = serde_json::from_slice(&old).unwrap();
    replacement["connectors"][0]["id"] = Value::String(CONNECTOR_B.to_owned());
    replacement["connectors"][0]["approved_manifest"]["id"] = Value::String(CONNECTOR_B.to_owned());
    let replacement = serde_json::to_vec(&replacement).unwrap();
    h.facts.insert(
        "old_registry".into(),
        Value::String(String::from_utf8(old).unwrap()),
    );
    h.facts.insert(
        "replacement_registry".into(),
        Value::String(String::from_utf8(replacement).unwrap()),
    );
    w.g7b = Some(h);
}

#[when("the process crashes at each atomic persistence boundary")]
async fn g7b_inject_atomic_crashes(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let store =
        ConnectorRegistryStore::from_store_config(&h.config.journal.as_ref().unwrap().store)
            .unwrap();
    let replacement = h.facts["replacement_registry"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec();
    assert!(store
        .persist_with_fault_for_test(&replacement, PersistenceFault::BeforeRename)
        .is_err());
    h.atomic_reads
        .push(std::fs::read(&h.registry_path).unwrap());
    assert!(store
        .persist_with_fault_for_test(&replacement, PersistenceFault::AfterRename)
        .is_err());
    h.atomic_reads
        .push(std::fs::read(&h.registry_path).unwrap());
    let before_invalid = std::fs::read(&h.registry_path).unwrap();
    assert!(store
        .persist_with_fault_for_test(
            br#"{"v":1,"connectors":[{"active":true}]}"#,
            PersistenceFault::BeforeRename
        )
        .is_err());
    h.facts.insert(
        "invalid_preserved".into(),
        Value::Bool(std::fs::read(&h.registry_path).unwrap() == before_invalid),
    );
}

#[then("restart reads either the old registry or the complete replacement")]
fn g7b_atomic_restart_reads_complete(w: &mut GatewayWorld) {
    let h = harness(w);
    let old: Value = serde_json::from_str(h.facts["old_registry"].as_str().unwrap()).unwrap();
    let replacement: Value =
        serde_json::from_str(h.facts["replacement_registry"].as_str().unwrap()).unwrap();
    assert_eq!(h.atomic_reads.len(), 2);
    for bytes in &h.atomic_reads {
        let observed: Value = serde_json::from_slice(bytes).unwrap();
        assert!(observed == old || observed == replacement);
    }
    assert_eq!(
        serde_json::from_slice::<Value>(&h.atomic_reads[0]).unwrap(),
        old
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&h.atomic_reads[1]).unwrap(),
        replacement
    );
}

#[then("no partial JSON or half-active connector is accepted")]
fn g7b_atomic_registry_rejects_partial(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.facts.get("invalid_preserved"), Some(&Value::Bool(true)));
    for bytes in &h.atomic_reads {
        let value: Value = serde_json::from_slice(bytes).unwrap();
        for connector in value["connectors"].as_array().unwrap() {
            if connector["active"] == true {
                assert_eq!(connector["state"], "connected");
                assert!(connector["live_digest"].is_string());
            }
        }
    }
}

#[given("two persisted active connectors with sealed approved pins")]
async fn g7b_two_active_persisted(w: &mut GatewayWorld) {
    let mut h = G7bHarness::two().await;
    h.connect_as_owner(CONNECTOR).await;
    assert_eq!(h.activate_as_owner(CONNECTOR).await.status, 200);
    h.connect_as_owner(CONNECTOR_B).await;
    assert_eq!(h.activate_as_owner(CONNECTOR_B).await.status, 200);
    w.g7b = Some(h);
}

#[given("only one still has valid OAuth custody")]
fn g7b_one_connector_loses_custody(w: &mut GatewayWorld) {
    harness(w).vault.remove(&G7bHarness::token_path(CONNECTOR));
}

#[when("the gateway restarts")]
async fn g7b_restore_connectors(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    {
        let mut runner = h.runner.lock().await;
        runner.remove_hot_connector(CONNECTOR);
        runner.remove_hot_connector(CONNECTOR_B);
    }
    h.dynamic.write().unwrap().clear();
    h.control.restore().await.unwrap();
}

#[then("the healthy connector returns active")]
async fn g7b_healthy_restored(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let list = h.list().await;
    let items = list.json()["items"].as_array().unwrap().clone();
    let healthy = items.iter().find(|item| item["id"] == CONNECTOR_B).unwrap();
    assert_eq!(healthy["active"], true);
    let tools = h.tools_list().await;
    assert!(tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == hub_exposed_name(CONNECTOR_B, "events.list")));
}

#[then("the unhealthy connector fails closed without disabling its neighbor")]
async fn g7b_unhealthy_restore_isolated(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let list = h.list().await;
    let items = list.json()["items"].as_array().unwrap().clone();
    let unhealthy = items.iter().find(|item| item["id"] == CONNECTOR).unwrap();
    let healthy = items.iter().find(|item| item["id"] == CONNECTOR_B).unwrap();
    assert_eq!(unhealthy["active"], false);
    assert_eq!(unhealthy["state"], "unavailable");
    assert_eq!(healthy["active"], true);
    assert!(!h.dynamic.read().unwrap().contains_key(CONNECTOR));
    assert!(h.dynamic.read().unwrap().contains_key(CONNECTOR_B));
}

#[given("one hot-activated connector")]
async fn g7b_one_hot_connector(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    h.connect_and_activate(CONNECTOR).await;
    h.vault.clear_counts();
    h.wire.clear_mcp();
    w.g7b = Some(h);
}

#[when("an agent calls tools/list repeatedly")]
async fn g7b_repeated_tools_list(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let first = h.tools_list().await;
    let second = h.tools_list().await;
    let third = h.tools_list().await;
    h.facts.insert("list_first".into(), first);
    h.facts.insert("list_second".into(), second);
    h.facts.insert("list_third".into(), third);
}

#[then("the approved runtime view is returned from memory")]
fn g7b_list_is_memory_view(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.facts["list_first"], h.facts["list_second"]);
    assert_eq!(h.facts["list_second"], h.facts["list_third"]);
    assert!(h.facts["list_first"]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == hub_exposed_name(CONNECTOR, "events.list")));
}

#[then("Vault and every upstream receive zero list requests")]
fn g7b_list_has_no_io(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.vault.request_count(), 0);
    assert_eq!(h.wire.list_count(), 0);
    assert!(h.wire.mcp_requests.lock().unwrap().is_empty());
}

#[given("active connectors A and B")]
async fn g7b_active_a_and_b(w: &mut GatewayWorld) {
    g7b_two_active_persisted(w).await;
    harness(w).vault.clear_counts();
    harness(w).wire.clear_mcp();
}

#[when("connector A becomes OAuth unavailable")]
async fn g7b_a_oauth_unavailable(w: &mut GatewayWorld) {
    let token_path = G7bHarness::token_path(CONNECTOR);
    harness(w).vault.fail_path(&token_path);
    let a = hub_exposed_name(CONNECTOR, "events.list");
    let b = hub_exposed_name(CONNECTOR_B, "events.list");
    let a_result = harness(w)
        .tool_call(&a, json!({ "calendar_id": "primary" }))
        .await;
    let b_result = harness(w)
        .tool_call(&b, json!({ "calendar_id": "primary" }))
        .await;
    let listed = harness(w).tools_list().await;
    let h = harness_mut(w);
    h.facts.insert("a_result".into(), a_result);
    h.facts.insert("b_result".into(), b_result);
    h.facts.insert("isolation_list".into(), listed);
}

#[then("connector B remains listed and callable")]
fn g7b_b_survives_a_failure(w: &mut GatewayWorld) {
    let h = harness(w);
    assert!(h.facts["a_result"].get("error").is_some());
    assert_eq!(
        h.facts["b_result"]["result"]["content"][0]["text"],
        "connector-ok"
    );
    assert!(h.facts["isolation_list"]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == hub_exposed_name(CONNECTOR_B, "events.list")));
}

#[then("connector A sends zero unauthenticated upstream requests")]
fn g7b_a_never_sends_unauthenticated(w: &mut GatewayWorld) {
    let h = harness(w);
    let requests = h.wire.mcp_requests.lock().unwrap();
    let bearers = h.wire.bearers.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "only B should reach the shared fake wire"
    );
    assert_eq!(bearers.len(), 1);
    assert!(bearers[0].is_some());
    assert!(requests
        .iter()
        .all(|body| body["params"]["name"] == "events.list"));
}

#[given("a hot-activated connector and a current mandate for one approved safe capability")]
#[given("a hot-activated connector and a mandate for one approved safe capability")]
async fn g7b_hot_safe_mandate(w: &mut GatewayWorld) {
    g7b_one_hot_connector(w).await;
    let action_count = gamma_view(harness(w).context_store.clone())
        .unwrap()
        .iter()
        .filter(|entry| entry.kind == "action")
        .count();
    harness_mut(w).facts.insert(
        "action_count_before_call".into(),
        Value::from(action_count as u64),
    );
}

#[when("the agent calls that exact capability")]
async fn g7b_call_safe_capability(w: &mut GatewayWorld) {
    let tool = hub_exposed_name(CONNECTOR, "events.list");
    let args = json!({ "calendar_id": "primary" });
    let response = harness(w).tool_call(&tool, args.clone()).await;
    let h = harness_mut(w);
    h.facts.insert("call_args".into(), args);
    h.facts.insert("call_response".into(), response);
}

#[then("the act is durably logged before the upstream receives it")]
fn g7b_act_precedes_relay(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(
        h.facts["call_response"]["result"]["content"][0]["text"],
        "connector-ok"
    );
    let before = h.facts["action_count_before_call"].as_u64().unwrap() as usize;
    let at_wire = h.wire.gamma_counts_at_call.lock().unwrap();
    assert_eq!(at_wire.len(), 1);
    assert!(
        at_wire[0] > before,
        "the context act must exist at wire receipt"
    );
}

#[then("only that connector receives the original bounded arguments")]
fn g7b_exact_args_relayed(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.wire.call_count(), 1);
    let requests = h.wire.mcp_requests.lock().unwrap();
    let call = requests
        .iter()
        .find(|body| body["method"] == "tools/call")
        .unwrap();
    assert_eq!(call["params"]["name"], "events.list");
    assert_eq!(call["params"]["arguments"], h.facts["call_args"]);
}

#[when("the agent calls a neighboring unmandated capability")]
async fn g7b_call_neighbor(w: &mut GatewayWorld) {
    let tool = hub_exposed_name(CONNECTOR, "events.delete");
    let response = harness(w)
        .tool_call(&tool, json!({ "event_id": "neighbor-event" }))
        .await;
    harness_mut(w)
        .facts
        .insert("neighbor_response".into(), response);
}

#[then("authority is denied before Vault and upstream")]
fn g7b_neighbor_denied_before_custody(w: &mut GatewayWorld) {
    let h = harness(w);
    assert_eq!(h.vault.request_count(), 0);
    assert!(h.wire.mcp_requests.lock().unwrap().is_empty());
    assert_eq!(
        h.facts["neighbor_response"]["error"]["code"],
        POLICY_DENIED_CODE
    );
    let message = h.facts["neighbor_response"]["error"]["message"]
        .as_str()
        .unwrap();
    assert!(message.contains("outside the granted perimeter"));
}

#[then("a redacted governance refusal is logged")]
fn g7b_neighbor_refusal_logged(w: &mut GatewayWorld) {
    let h = harness(w);
    for store in [&h.context_store, &h.journal_store] {
        let entries = gamma_view(store.clone()).unwrap();
        assert!(entries.iter().any(|entry| {
            entry.kind == "action"
                && entry
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("action"))
                    .and_then(Value::as_str)
                    == Some("refuse")
        }));
        let blob = serde_json::to_string(&entries).unwrap();
        for secret in [CLIENT_SECRET, ACCESS_ONE, REFRESH_ONE] {
            assert!(!blob.contains(secret));
        }
    }
}

#[given("every internal connector failure contains distinct secret sentinels")]
async fn g7b_internal_failure_sentinels(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    h.facts.insert(
        "failure_sentinels".into(),
        json!([INTERNAL_SENTINEL, CLIENT_SECRET, ACCESS_ONE, REFRESH_ONE]),
    );
    w.g7b = Some(h);
}

#[when("each failure crosses the control API")]
async fn g7b_failures_cross_api(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let bad = serde_json::to_vec(&json!({
        "v": 1,
        "id": CONNECTOR,
        "context": CONTEXT,
        "sentinel": INTERNAL_SENTINEL,
    }))
    .unwrap();
    h.send(
        "POST",
        &format!("/control/v1/connectors/{CONNECTOR}/stage"),
        bad,
        true,
    )
    .await;
    assert_eq!(h.stage(CONNECTOR).await.status, 201);
    h.vault.fail_all.store(true, Ordering::SeqCst);
    h.secret(CONNECTOR).await;
    h.vault.fail_all.store(false, Ordering::SeqCst);
    assert_eq!(h.secret(CONNECTOR).await.status, 200);
    h.activate(CONNECTOR).await;
    let state = h.begin_pending(CONNECTOR).await;
    assert_eq!(
        h.callback(&[("code", CALLBACK_CODE), ("state", state.as_str())])
            .await
            .status,
        200
    );
    h.wire.catalogue.lock().unwrap().push(json!({
        "name": "sentinel.extra",
        "description": INTERNAL_SENTINEL,
        "inputSchema": { "type": "object", "additionalProperties": false },
    }));
    h.activate(CONNECTOR).await;
    let direct_codes = [
        ConnectorFailure::NotApproved,
        ConnectorFailure::SecretUnavailable,
        ConnectorFailure::OauthPending,
        ConnectorFailure::OauthDenied,
        ConnectorFailure::OauthUnavailable,
        ConnectorFailure::ManifestDrift,
        ConnectorFailure::ActivationFailed,
        ConnectorFailure::UpstreamDenied,
    ]
    .map(|failure| failure.code())
    .to_vec();
    h.facts
        .insert("direct_failure_codes".into(), json!(direct_codes));
}

#[then("its public code belongs to the documented finite error set")]
fn g7b_errors_are_finite(w: &mut GatewayWorld) {
    let h = harness(w);
    let finite: BTreeSet<_> = [
        "gateway_offline",
        "origin_denied",
        "authority_denied",
        "connector_not_approved",
        "secret_unavailable",
        "oauth_pending",
        "oauth_denied",
        "oauth_unavailable",
        "manifest_drift",
        "activation_failed",
        "upstream_denied",
    ]
    .into_iter()
    .collect();
    let api_codes: Vec<_> = h
        .captures
        .iter()
        .filter_map(|capture| capture.json()["error"].as_str().map(str::to_owned))
        .collect();
    assert!(
        api_codes.len() >= 4,
        "expected several independent API failures"
    );
    assert!(api_codes.iter().all(|code| finite.contains(code.as_str())));
    for code in h.facts["direct_failure_codes"].as_array().unwrap() {
        assert!(finite.contains(code.as_str().unwrap()));
    }
}

#[then("no sentinel appears in body, headers, URL, registry, proof or logs")]
fn g7b_failure_sentinels_are_redacted(w: &mut GatewayWorld) {
    let h = harness(w);
    let mut public = h.public_surface();
    public.push_str(&h.base);
    public.push_str(&h.callback_url);
    for capture in &h.captures {
        public.push_str(&capture.text());
        public.push_str(&serde_json::to_string(&capture.headers).unwrap());
    }
    for sentinel in h.facts["failure_sentinels"].as_array().unwrap() {
        assert!(!public.contains(sentinel.as_str().unwrap()));
    }
}

#[given("a connected connector with secret and tokens in Vault")]
async fn g7b_connected_for_control_reads(w: &mut GatewayWorld) {
    let mut h = G7bHarness::exact().await;
    h.connect(CONNECTOR).await;
    h.facts.insert(
        "sensitive_capture_start".into(),
        Value::from(h.captures.len() as u64),
    );
    w.g7b = Some(h);
}

#[when("owner and connector config authorities read every permitted control route")]
async fn g7b_read_all_permitted_control(w: &mut GatewayWorld) {
    let h = harness_mut(w);
    let owner_routes = [
        "/control/v1/status",
        "/control/v1/contexts",
        "/control/v1/connectors",
        "/control/v1/contexts/operations/certs?limit=64",
        "/control/v1/contexts/operations/gamma?kind=action&limit=64",
        "/control/v1/contexts/operations/heads",
        "/control/v1/connectors/calendar-safe/oauth/status",
    ];
    for route in owner_routes {
        let response = h.send("GET", route, Vec::new(), true).await;
        assert_eq!(
            response.status,
            200,
            "owner route {route}: {}",
            response.text()
        );
    }
    let delegated = h
        .send(
            "GET",
            "/control/v1/connectors/calendar-safe/oauth/status",
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(delegated.status, 200, "{}", delegated.text());
}

#[then("no response contains a client secret, token, Vault reference or MCP payload")]
fn g7b_control_reads_are_redacted(w: &mut GatewayWorld) {
    let h = harness(w);
    let start = h.facts["sensitive_capture_start"].as_u64().unwrap() as usize;
    let public = h.captures[start..]
        .iter()
        .map(|capture| {
            format!(
                "{}\n{}",
                capture.text(),
                serde_json::to_string(&capture.headers).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        CLIENT_SECRET,
        ACCESS_ONE,
        ACCESS_TWO,
        REFRESH_ONE,
        REFRESH_TWO,
        "aithos/connectors/",
        "connector-ok",
        "events.list\"",
    ] {
        assert!(
            !public.contains(forbidden),
            "control response leaked {forbidden}"
        );
    }
}

#[then("every sensitive response is no-store")]
fn g7b_sensitive_responses_no_store(w: &mut GatewayWorld) {
    let h = harness(w);
    let start = h.facts["sensitive_capture_start"].as_u64().unwrap() as usize;
    assert!(!h.captures[start..].is_empty());
    for capture in &h.captures[start..] {
        assert_eq!(capture.header("cache-control"), Some("no-store"));
    }
}
