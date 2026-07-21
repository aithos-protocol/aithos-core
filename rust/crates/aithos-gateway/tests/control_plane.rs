use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_gateway::config::DashboardConfig;
use aithos_gateway::control::{self, ControlState};
use aithos_gateway::core_bridge::{
    agent_pub_multibase, gateway_pub_multibase, owner_grant_context, owner_init_context,
    owner_revoke_mandate_id, Bridge, ControlProofReader, MandateWindow, RawStore, SeqEntropy,
};
use aithos_gateway::credentials::{
    CredentialBroker, CredentialBrokerReadiness, CredentialRef, SecretValue,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::relay::{RelayHealth, RelayReadiness};
use aithos_gateway::store_adapter::GatewayStore;
use aithos_provider::envelope::{header_value, sign_envelope, Envelope, EnvelopeSignature};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::SigningKey;
use serde_json::Value;

const ORIGIN: &str = "https://app.aithos.fr";
const NOW: &str = "2026-07-16T12:00:00Z";
const NOW_MS: i64 = 1_784_203_200_000;

struct ReadyBroker;

impl CredentialBroker for ReadyBroker {
    fn resolve<'a>(
        &'a self,
        _reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = aithos_gateway::Result<SecretValue>> + Send + 'a>> {
        Box::pin(async {
            Err(aithos_gateway::GatewayError::CredentialUnavailable(
                "test broker has no readable secret".into(),
            ))
        })
    }

    fn readiness<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = CredentialBrokerReadiness> + Send + 'a>> {
        Box::pin(async { CredentialBrokerReadiness::Ready })
    }
}

struct Fixture {
    base: String,
    host: String,
    client: reqwest::Client,
    owner: OwnerKeys,
    auditor: SigningKey,
    auditor_mandate: String,
    company_store: GatewayStore,
    master: [u8; 32],
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Fixture {
    async fn new() -> Self {
        let master = [7u8; 32];
        let keyholder = Arc::new(Keyholder::from_entropy([0x31; 32], [0x41; 32]));
        let agent_pub = agent_pub_multibase(&keyholder);
        let gateway_pub = gateway_pub_multibase(&keyholder);
        let window = MandateWindow {
            not_before: "2026-07-16T11:00:00Z".to_owned(),
            not_after: "2026-07-16T12:01:00Z".to_owned(),
        };
        let company_store = GatewayStore::in_memory();
        let neighbor_store = GatewayStore::in_memory();
        let mut entropy = SeqEntropy::default();
        owner_init_context(
            &master,
            "company-brand",
            company_store.clone(),
            NOW,
            &mut entropy,
        )
        .unwrap();
        let equipped = owner_grant_context(
            &master,
            "company-brand",
            &agent_pub,
            &gateway_pub,
            &["brand.read".to_owned()],
            company_store.clone(),
            &window,
            NOW,
            &mut entropy,
        )
        .unwrap();
        owner_init_context(
            &master,
            "ui-designer",
            neighbor_store.clone(),
            NOW,
            &mut entropy,
        )
        .unwrap();
        owner_grant_context(
            &master,
            "ui-designer",
            &agent_pub,
            &gateway_pub,
            &["figma.read".to_owned()],
            neighbor_store.clone(),
            &window,
            NOW,
            &mut entropy,
        )
        .unwrap();

        let mut bridge = Bridge::open(
            company_store.clone(),
            Arc::clone(&keyholder),
            Box::new(SeqEntropy::default()),
        )
        .unwrap();
        bridge
            .record_act("brand.read", &serde_json::json!({ "q": "safe" }), NOW)
            .unwrap();

        let stores = BTreeMap::from([
            ("company-brand".to_owned(), company_store.clone()),
            ("ui-designer".to_owned(), neighbor_store),
        ]);
        let reader = ControlProofReader::from_stores(stores).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let host = address.to_string();
        let dashboard = DashboardConfig {
            allowed_origins: vec![ORIGIN.to_owned()],
        };
        let brokers: BTreeMap<String, Arc<dyn CredentialBroker>> =
            BTreeMap::from([("enterprise".to_owned(), Arc::new(ReadyBroker) as _)]);
        let state = ControlState::new(
            reader,
            &dashboard,
            [host.clone()],
            RelayHealth::new(RelayReadiness::Ready),
            brokers,
        )
        .unwrap()
        .with_clock(Arc::new(|| NOW_MS));
        let app = control::router(Arc::new(state));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let owner_seed =
            aithos_core::derive::derive_key("aithos-gw/v1/context/company-brand", &master);
        let owner = OwnerKeys::genesis(&MasterSeed::from_bytes(owner_seed));
        let auditor_seed: [u8; 32] = hex::decode(equipped.auditor_seed_hex.unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        Self {
            base: format!("http://{address}"),
            host,
            client: reqwest::Client::new(),
            owner,
            auditor: SigningKey::from_bytes(&auditor_seed),
            auditor_mandate: equipped.auditor_mandate.unwrap(),
            company_store,
            master,
            server,
        }
    }

    #[allow(clippy::too_many_arguments)] // mirrors every closed A.2 request fact
    fn envelope(
        &self,
        key: &SigningKey,
        key_name: &str,
        mandates: Vec<String>,
        method: &str,
        path: &str,
        body: &[u8],
        at: &str,
        nonce: &str,
    ) -> String {
        header_value(
            &sign_envelope(
                Envelope {
                    v: 1,
                    host: self.host.clone(),
                    method: method.to_owned(),
                    path: path.to_owned(),
                    body_b3: if body.is_empty() {
                        String::new()
                    } else {
                        blake3::hash(body).to_hex().to_string()
                    },
                    at: at.to_owned(),
                    nonce: nonce.to_owned(),
                    mandate: mandates,
                    key: key_name.to_owned(),
                    signature: EnvelopeSignature {
                        alg: "ed25519".to_owned(),
                        value: String::new(),
                    },
                },
                key,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn owner_header(&self, path: &str, nonce: &str) -> String {
        self.envelope(
            &self.owner.content_sign,
            "#content",
            Vec::new(),
            "GET",
            path,
            &[],
            NOW,
            nonce,
        )
    }

    fn auditor_header(&self, path: &str, at: &str, nonce: &str) -> String {
        self.envelope(
            &self.auditor,
            &aithos_core::wire::ed25519_pub_to_multibase(&self.auditor.verifying_key().to_bytes()),
            vec![self.auditor_mandate.clone()],
            "GET",
            path,
            &[],
            at,
            nonce,
        )
    }

    async fn get(&self, path: &str, auth: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{path}", self.base))
            .header("Origin", ORIGIN)
            .header("X-Aithos-Auth", auth)
            .send()
            .await
            .unwrap()
    }
}

fn snapshot(store: &GatewayStore) -> BTreeMap<String, Vec<u8>> {
    store
        .list("")
        .unwrap()
        .into_iter()
        .map(|path| {
            let bytes = store.get(&path).unwrap().unwrap();
            (path, bytes)
        })
        .collect()
}

#[tokio::test]
async fn signed_control_surface_is_exact_scoped_and_redacted() {
    let fixture = Fixture::new().await;

    let preflight = fixture
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/control/v1/status", fixture.base),
        )
        .header("Origin", ORIGIN)
        .header("Access-Control-Request-Method", "GET")
        .header("Access-Control-Request-Headers", "X-Aithos-Auth")
        .send()
        .await
        .unwrap();
    assert_eq!(preflight.status(), 204);
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        ORIGIN
    );
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-methods")
            .unwrap(),
        "GET"
    );
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-headers")
            .unwrap(),
        "X-Aithos-Auth"
    );
    assert!(preflight
        .headers()
        .get("access-control-allow-credentials")
        .is_none());

    let neighbor = fixture
        .client
        .get(format!("{}/control/v1/status", fixture.base))
        .header("Origin", "https://neighbor.app.aithos.fr")
        .send()
        .await
        .unwrap();
    assert_eq!(neighbor.status(), 403);
    assert!(neighbor
        .headers()
        .get("access-control-allow-origin")
        .is_none());
    assert_eq!(neighbor.headers().get("cache-control").unwrap(), "no-store");
    assert_eq!(neighbor.headers().get("vary").unwrap(), "Origin");

    let mut duplicate_origins = reqwest::header::HeaderMap::new();
    duplicate_origins.append(
        reqwest::header::ORIGIN,
        reqwest::header::HeaderValue::from_static(ORIGIN),
    );
    duplicate_origins.append(
        reqwest::header::ORIGIN,
        reqwest::header::HeaderValue::from_static(ORIGIN),
    );
    let duplicate_origin = fixture
        .client
        .get(format!("{}/control/v1/status", fixture.base))
        .headers(duplicate_origins)
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_origin.status(), 403);
    assert!(duplicate_origin
        .headers()
        .get("access-control-allow-origin")
        .is_none());

    let duplicated_auth = fixture.owner_header("/control/v1/status", "duplicated-auth-01");
    let auth_name = reqwest::header::HeaderName::from_static("x-aithos-auth");
    let auth_value = reqwest::header::HeaderValue::from_str(&duplicated_auth).unwrap();
    let mut duplicate_auth_headers = reqwest::header::HeaderMap::new();
    duplicate_auth_headers.append(
        reqwest::header::ORIGIN,
        reqwest::header::HeaderValue::from_static(ORIGIN),
    );
    duplicate_auth_headers.append(auth_name.clone(), auth_value.clone());
    duplicate_auth_headers.append(auth_name, auth_value);
    let duplicate_auth = fixture
        .client
        .get(format!("{}/control/v1/status", fixture.base))
        .headers(duplicate_auth_headers)
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_auth.status(), 401);

    let forbidden_body = br#"{"unexpected":true}"#;
    let signed_body = fixture.envelope(
        &fixture.owner.content_sign,
        "#content",
        Vec::new(),
        "GET",
        "/control/v1/status",
        forbidden_body,
        NOW,
        "bodyless-get-01",
    );
    let bodyful_get = fixture
        .client
        .get(format!("{}/control/v1/status", fixture.base))
        .header("Origin", ORIGIN)
        .header("X-Aithos-Auth", signed_body)
        .body(forbidden_body.as_slice())
        .send()
        .await
        .unwrap();
    assert_eq!(bodyful_get.status(), 401);

    let unsigned = fixture
        .client
        .get(format!("{}/control/v1/status", fixture.base))
        .header("Origin", ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(unsigned.status(), 401);

    let status = fixture
        .get(
            "/control/v1/status",
            &fixture.owner_header("/control/v1/status", "owner-status-01"),
        )
        .await;
    assert_eq!(status.status(), 200);
    assert_eq!(status.headers().get("cache-control").unwrap(), "no-store");
    let status: Value = status.json().await.unwrap();
    assert_eq!(status["process"], "ready");
    assert_eq!(status["vault"], "ready");
    assert_eq!(status["relay"], "ready");
    let rendered = status.to_string();
    for sentinel in [
        "/private/customer",
        "VAULT_TOKEN",
        "access-token",
        "mcp-argument",
    ] {
        assert!(!rendered.contains(sentinel));
    }

    let contexts = fixture
        .get(
            "/control/v1/contexts",
            &fixture.owner_header("/control/v1/contexts", "owner-contexts-01"),
        )
        .await;
    assert_eq!(contexts.status(), 200);
    let contexts: Value = contexts.json().await.unwrap();
    assert_eq!(contexts["items"].as_array().unwrap().len(), 1);
    assert_eq!(contexts["items"][0]["name"], "company-brand");
    let did_raw = URL_SAFE_NO_PAD
        .decode(
            contexts["items"][0]["did_document"]["bytes_b64"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let did: Value = serde_json::from_slice(&did_raw).unwrap();
    assert_eq!(did["id"], contexts["items"][0]["did"]);

    let gamma_path = "/control/v1/contexts/company-brand/gamma?kind=action&limit=10";
    let gamma = fixture
        .get(
            gamma_path,
            &fixture.auditor_header(gamma_path, NOW, "auditor-gamma-01"),
        )
        .await;
    assert_eq!(gamma.status(), 200);
    let gamma: Value = gamma.json().await.unwrap();
    let items = gamma["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let entry_raw = URL_SAFE_NO_PAD
        .decode(items[0]["bytes_b64"].as_str().unwrap())
        .unwrap();
    let entry: Value = serde_json::from_slice(&entry_raw).unwrap();
    assert_eq!(entry["kind"], "action");
    assert!(entry.get("signature").is_some());

    let certs_path = "/control/v1/contexts/company-brand/certs?limit=10";
    let certs = fixture
        .get(
            certs_path,
            &fixture.auditor_header(certs_path, NOW, "auditor-certs-01"),
        )
        .await;
    assert_eq!(certs.status(), 200);
    let certs: Value = certs.json().await.unwrap();
    assert!(!certs["items"].as_array().unwrap().is_empty());
    let visible_certificates: BTreeSet<String> = certs["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["path"].as_str().unwrap().to_owned())
        .collect();
    assert!(visible_certificates.contains(&format!("certs/{}.json", fixture.auditor_mandate)));
    let all_certificates: BTreeSet<String> = fixture
        .company_store
        .list("certs/")
        .unwrap()
        .into_iter()
        .collect();
    assert!(visible_certificates.is_subset(&all_certificates));
    assert!(
        visible_certificates.len() < all_certificates.len(),
        "an auditor must not receive an unrelated certificate"
    );

    let heads_path = "/control/v1/contexts/company-brand/heads";
    let heads = fixture
        .get(
            heads_path,
            &fixture.auditor_header(heads_path, NOW, "auditor-heads-01"),
        )
        .await;
    assert_eq!(heads.status(), 200);
    let heads: Value = heads.json().await.unwrap();
    assert!(
        heads["manifest"].is_null(),
        "auditor never receives the wider manifest"
    );
    assert!(heads["gamma_tail"].is_object());

    let before_invalid = snapshot(&fixture.company_store);
    let neighboring_path = "/control/v1/contexts/company-brand/gamma?kind=grant";
    let neighboring = fixture
        .get(
            neighboring_path,
            &fixture.auditor_header(neighboring_path, NOW, "neighboring-right-01"),
        )
        .await;
    assert_eq!(neighboring.status(), 401);

    let skewed = fixture
        .get(
            gamma_path,
            &fixture.auditor_header(gamma_path, "2026-07-16T11:50:00Z", "auditor-skew-01"),
        )
        .await;
    assert_eq!(skewed.status(), 401);

    let expired = fixture
        .get(
            gamma_path,
            &fixture.auditor_header(gamma_path, "2026-07-16T12:02:00Z", "auditor-expired-01"),
        )
        .await;
    assert_eq!(expired.status(), 401);

    let wrong = SigningKey::from_bytes(&[0x99; 32]);
    let false_signature = fixture.envelope(
        &wrong,
        "#content",
        Vec::new(),
        "GET",
        "/control/v1/status",
        &[],
        NOW,
        "false-signature-01",
    );
    assert_eq!(
        fixture
            .get("/control/v1/status", &false_signature)
            .await
            .status(),
        401
    );

    let modified_path_header = fixture.owner_header("/control/v1/status", "changed-path-01");
    assert_eq!(
        fixture
            .get("/control/v1/contexts", &modified_path_header)
            .await
            .status(),
        401
    );

    let replay_header = fixture.owner_header("/control/v1/status", "replay-01");
    assert_eq!(
        fixture
            .get("/control/v1/status", &replay_header)
            .await
            .status(),
        200
    );
    assert_eq!(
        fixture
            .get("/control/v1/status", &replay_header)
            .await
            .status(),
        401
    );
    assert_eq!(snapshot(&fixture.company_store), before_invalid);

    let mut entropy = SeqEntropy::default();
    owner_revoke_mandate_id(
        &fixture.master,
        "company-brand",
        &fixture.auditor_mandate,
        "test revocation",
        fixture.company_store.clone(),
        NOW,
        &mut entropy,
    )
    .unwrap();
    let revoked = fixture
        .get(
            gamma_path,
            &fixture.auditor_header(gamma_path, NOW, "auditor-revoked-01"),
        )
        .await;
    assert_eq!(revoked.status(), 401);
}
