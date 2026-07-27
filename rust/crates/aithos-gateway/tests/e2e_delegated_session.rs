//! End-to-end G4/G5 hot-path proof: OAuth selects a durable session, while
//! its non-root mandate chain, external Gamma grant and SC1 double proof
//! authorize every MCP operation before the upstream is touched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use aithos_core::gamma::Entry;
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
use aithos_gateway::config::{GatewayConfig, StoreConfig};
use aithos_gateway::core_bridge::{
    agent_pub_multibase, gamma_view, gateway_kex_pub_multibase, gateway_pub_multibase,
    owner_grant_briefing, owner_grant_context, owner_grant_session_delegate, owner_init_context,
    owner_init_journal, owner_revoke_mandate_id, owner_set_briefing, MandateWindow, Runner,
    SeqEntropy,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::oauth::{
    build_ceremony_challenge, s256_challenge, AdapterKey, AuthServer, AuthorizeOutcome,
    AuthorizeRequest, CeremonyProof,
};
use aithos_gateway::oauth_state::MemoryAsStateStore;
use aithos_gateway::policy::op_for_tool;
use aithos_gateway::proxy_mcp::{
    empty_dynamic_upstreams, router_multi, router_oauth, McpRouter, Upstream,
};
use aithos_gateway::store_adapter::GatewayStore;
use aithos_gateway::Result;

const NOW: &str = "2026-07-22T12:00:00Z";
const SESSION_END: &str = "2026-07-22T20:00:00Z";
const CALLBACK: &str = "http://127.0.0.1:19410/callback";
const AS_ISSUER: &str = "http://127.0.0.1:4870";
const RESOURCE: &str = "http://127.0.0.1:4870/mcp";
const DASHBOARD_ORIGIN: &str = "https://app.aithos.fr";
const MASTER: [u8; 32] = [0x31; 32];

#[derive(Clone, Default)]
struct RecordingUpstream {
    calls: Arc<AtomicUsize>,
}

impl Upstream for RecordingUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({
            "jsonrpc": "2.0",
            "id": body["id"],
            "result": {
                "content": [{ "type": "text", "text": "safe upstream read" }],
                "isError": false,
            }
        }))
    }
}

struct IssuedSession {
    access_token: String,
    parent_id: String,
    leaf_id: String,
    session_pub: String,
}

#[allow(clippy::too_many_arguments)]
async fn issue_session(
    auth: &AuthServer,
    runner: &Arc<Mutex<Runner>>,
    client_id: &str,
    delegate: &SigningKey,
    tool: &str,
    sequence: u8,
) -> IssuedSession {
    let verifier = format!("g4-verifier-{sequence}-{}", "x".repeat(43));
    let request = AuthorizeRequest {
        client_id: client_id.to_owned(),
        redirect_uri: CALLBACK.to_owned(),
        response_type: "code".to_owned(),
        code_challenge: Some(s256_challenge(&verifier)),
        code_challenge_method: Some("S256".to_owned()),
        resource: Some(auth.resource().to_owned()),
        scope: Some("mcp".to_owned()),
        state: Some(format!("state-{sequence}")),
    };
    let pending = match auth.authorize_at(&request, NOW) {
        AuthorizeOutcome::Ceremony { pending, .. } => pending,
        _ => panic!("production authorization must create a ceremony"),
    };
    let delegate_pub =
        aithos_core::wire::ed25519_pub_to_multibase(&delegate.verifying_key().to_bytes());
    let preparation = auth
        .prepare_ceremony(&pending.transaction_id, &delegate_pub, NOW)
        .unwrap();
    let parent_view = runner
        .lock()
        .await
        .eligible_session_parents(&delegate_pub, RESOURCE, NOW)
        .into_iter()
        .find(|parent| parent.context == "finance")
        .expect("delegate has one issuing parent");
    let parent: Mandate =
        serde_json::from_value(parent_view.chain.last().unwrap().clone()).unwrap();
    let gateway_bytes =
        aithos_core::wire::multibase_to_ed25519_pub(&preparation.gateway_pub).unwrap();
    let gateway = ed25519_dalek::VerifyingKey::from_bytes(&gateway_bytes).unwrap();
    let mut constraints = parent.constraints.clone();
    constraints["session_bind"] = Value::String(preparation.session_pub.clone());
    let leaf = Mandate::build_sub(
        &parent,
        delegate,
        &MandateSpec {
            id: format!("mandate_01J0000000000000000000007{sequence}"),
            subject: parent.subject.clone(),
            grantee_id: format!("urn:aithos:agent:session-{sequence}"),
            grantee_label: format!("session {sequence}"),
            grantee_pub: &gateway,
            perimeter: vec![PerimeterEntry::parse(&op_for_tool(tool)).unwrap()],
            constraints,
            not_before: NOW.to_owned(),
            not_after: SESSION_END.to_owned(),
            issued_at: "2026-07-22T11:59:59Z".to_owned(),
            nonce: format!("{sequence:02x}").repeat(16),
        },
    )
    .unwrap();
    let leaf_value = serde_json::to_value(&leaf).unwrap();
    let unsigned_grant = runner
        .lock()
        .await
        .prepare_session_grant(
            "finance",
            &parent.id,
            &delegate_pub,
            &preparation.gateway_pub,
            &preparation.gateway_kex_pub,
            &preparation.session_pub,
            &preparation.resource,
            &leaf_value,
            NOW,
        )
        .unwrap();
    let mut grant: Entry = serde_json::from_value(unsigned_grant).unwrap();
    let grant_preimage = serde_jcs::to_vec(&grant).unwrap();
    grant.signature.value = hex::encode(delegate.sign(&grant_preimage).to_bytes());
    let grant_value = serde_json::to_value(&grant).unwrap();
    let challenge = build_ceremony_challenge(
        &preparation,
        "finance",
        &parent.id,
        &leaf_value,
        &grant_value,
    )
    .unwrap();
    let proof = CeremonyProof {
        version: "1.0.0".to_owned(),
        digest: challenge.digest,
        delegate_pub: delegate_pub.clone(),
        sig: hex::encode(delegate.sign(&challenge.signing_preimage).to_bytes()),
    };
    let reserved = auth
        .reserve_ceremony_completion(
            &pending.transaction_id,
            "finance",
            &parent.id,
            &leaf_value,
            &grant_value,
            &proof,
            NOW,
        )
        .unwrap();
    let authority = runner
        .lock()
        .await
        .activate_session_leaf(
            "finance",
            &parent.id,
            &delegate_pub,
            &preparation.gateway_pub,
            &preparation.gateway_kex_pub,
            &preparation.session_pub,
            &preparation.resource,
            &leaf_value,
            &grant_value,
            NOW,
        )
        .unwrap();
    let location = auth.finalize_ceremony(reserved, &authority, NOW).unwrap();
    let code = location
        .split(['?', '&'])
        .find_map(|part| part.strip_prefix("code="))
        .unwrap();
    let (grant, _) = auth
        .exchange_code(code, &verifier, auth.resource(), CALLBACK, None, NOW)
        .unwrap();
    IssuedSession {
        access_token: grant.access_token,
        parent_id: parent.id,
        leaf_id: leaf.id,
        session_pub: preparation.session_pub,
    }
}

async fn post_mcp(base: &str, token: &str, body: Value) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn delegated_sessions_isolate_surface_log_full_chain_and_cut_on_revocation() {
    let scratch = tempfile::tempdir().unwrap();
    let context_root = scratch.path().join("finance");
    let journal_root = scratch.path().join("journal");
    let context_store = GatewayStore::from_config(&StoreConfig::Fs {
        root: context_root.clone(),
    })
    .unwrap();
    let journal_store = GatewayStore::from_config(&StoreConfig::Fs {
        root: journal_root.clone(),
    })
    .unwrap();
    let delegate_seed = [0x41; 32];
    let gateway_seed = [0x42; 32];
    let delegate = SigningKey::from_bytes(&delegate_seed);
    let keyholder = Keyholder::from_entropy(delegate_seed, gateway_seed);
    let delegate_pub = agent_pub_multibase(&keyholder);
    let gateway_pub = gateway_pub_multibase(&keyholder);
    let gateway_kex_pub = gateway_kex_pub_multibase(&keyholder);
    let mut owner_entropy = SeqEntropy::default();
    owner_init_context(
        &MASTER,
        "finance",
        context_store.clone(),
        "2026-07-22T10:00:00Z",
        &mut owner_entropy,
    )
    .unwrap();
    let window = MandateWindow {
        not_before: "2026-07-22T10:00:00Z".to_owned(),
        not_after: "2026-07-23T00:00:00Z".to_owned(),
    };
    owner_grant_context(
        &MASTER,
        "finance",
        &delegate_pub,
        &gateway_pub,
        &["issues.list".to_owned(), "issues.create".to_owned()],
        context_store.clone(),
        &window,
        "2026-07-22T10:00:00Z",
        &mut owner_entropy,
    )
    .unwrap();
    owner_grant_briefing(
        &MASTER,
        "finance",
        &delegate_pub,
        context_store.clone(),
        &window,
        "2026-07-22T10:00:00Z",
        &mut owner_entropy,
    )
    .unwrap();
    owner_set_briefing(
        &MASTER,
        "finance",
        "circle",
        "Session directive",
        "Read-only delegated work only.",
        context_store.clone(),
        "2026-07-22T10:00:00Z",
        &mut owner_entropy,
    )
    .unwrap();
    owner_grant_session_delegate(
        &MASTER,
        "finance",
        &delegate_pub,
        RESOURCE,
        &[
            "issues.list".to_owned(),
            "issues.create".to_owned(),
            "briefing.read".to_owned(),
        ],
        context_store.clone(),
        &window,
        "2026-07-22T10:00:00Z",
        &mut owner_entropy,
    )
    .unwrap();
    owner_init_journal(
        &MASTER,
        "delegate",
        &delegate_pub,
        &gateway_pub,
        None,
        journal_store,
        &window,
        "2026-07-22T10:00:00Z",
        &mut owner_entropy,
    )
    .unwrap();
    let quote = |path: &std::path::Path| serde_json::to_string(&path.to_string_lossy()).unwrap();
    let config = GatewayConfig::from_yaml(&format!(
        "listen: 127.0.0.1:0\ncontexts:\n  - name: finance\n    store: {{ kind: fs, root: {} }}\n    upstream_mcp: http://127.0.0.1:9/mcp\n    tools:\n      issues.list: read\n      issues.create: read\njournal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(&context_root),
        quote(&journal_root),
    ))
    .unwrap();
    let runner = Arc::new(Mutex::new(
        Runner::open(&config, keyholder, || Box::new(SeqEntropy::default())).unwrap(),
    ));
    assert_eq!(
        runner
            .lock()
            .await
            .eligible_session_parents(&delegate_pub, RESOURCE, NOW)
            .len(),
        1
    );
    assert!(runner
        .lock()
        .await
        .eligible_session_parents(&delegate_pub, "https://gateway-b.example/mcp", NOW)
        .is_empty());

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}");
    let auth = Arc::new(AuthServer::new_production_with_state(
        AdapterKey::from_seed([0x43; 32]),
        AS_ISSUER,
        15 * 60,
        8 * 60 * 60,
        vec![CALLBACK.to_owned()],
        Box::new(SeqEntropy::default()),
        Arc::new(MemoryAsStateStore::default()),
        gateway_pub,
        gateway_kex_pub,
    ));
    let client_id = auth
        .register(&json!({ "redirect_uris": [CALLBACK] }))
        .unwrap()["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let list_session = issue_session(&auth, &runner, &client_id, &delegate, "issues.list", 1).await;
    let create_session =
        issue_session(&auth, &runner, &client_id, &delegate, "issues.create", 2).await;
    let briefing_session =
        issue_session(&auth, &runner, &client_id, &delegate, "briefing.read", 3).await;
    let upstream = RecordingUpstream::default();
    let routing = Arc::new(McpRouter {
        runner,
        upstreams: BTreeMap::from([("finance".to_owned(), upstream.clone())]),
        // Run the generic connector regression under the opt-in Ethos
        // backend: exact-name routing must leave every non-Ethos byte on the
        // historical path.
        ethos_backend: aithos_gateway::ethos_backend::client_provider_ethos_backend(),
        dynamic_upstreams: empty_dynamic_upstreams(),
        clock: Arc::new(|| NOW.to_owned()),
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: Some(auth),
        browser_origins: Arc::new(BTreeSet::from([DASHBOARD_ORIGIN.to_owned()])),
    });
    let app = router_multi(Arc::clone(&routing)).merge(router_oauth(routing));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let http = reqwest::Client::new();
    for (path, method, headers) in [
        ("/ceremony/prepare", "POST", "Accept, Content-Type"),
        (
            "/mcp",
            "POST",
            "Accept, Authorization, Content-Type, MCP-Protocol-Version, MCP-Session-Id",
        ),
    ] {
        let response = http
            .request(reqwest::Method::OPTIONS, format!("{issuer}{path}"))
            .header("Origin", DASHBOARD_ORIGIN)
            .header("Access-Control-Request-Method", method)
            .header("Access-Control-Request-Headers", headers)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 204, "preflight {path}");
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|value| value.to_str().ok()),
            Some(DASHBOARD_ORIGIN)
        );
        assert!(response
            .headers()
            .get("access-control-allow-credentials")
            .is_none());
    }
    for (origin, headers) in [
        (
            "https://neighbor.aithos.fr",
            "Accept, Authorization, Content-Type, MCP-Protocol-Version, MCP-Session-Id",
        ),
        (
            DASHBOARD_ORIGIN,
            "Accept, Authorization, Content-Type, MCP-Protocol-Version, MCP-Session-Id, X-Extra",
        ),
    ] {
        let response = http
            .request(reqwest::Method::OPTIONS, format!("{issuer}/mcp"))
            .header("Origin", origin)
            .header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", headers)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
        assert!(response
            .headers()
            .get("access-control-allow-origin")
            .is_none());
    }
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 0);
    let oauth_refusal = http
        .post(format!("{issuer}/mcp"))
        .header("Origin", DASHBOARD_ORIGIN)
        .json(&json!({"jsonrpc":"2.0","id":0,"method":"initialize"}))
        .send()
        .await
        .unwrap();
    assert_eq!(oauth_refusal.status(), 401);
    assert_eq!(
        oauth_refusal
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some(DASHBOARD_ORIGIN)
    );
    assert!(oauth_refusal
        .headers()
        .get("access-control-expose-headers")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("WWW-Authenticate")));

    for (session, expected, absent) in [
        (&list_session, "issues.list", "issues.create"),
        (&create_session, "issues.create", "issues.list"),
    ] {
        let response = post_mcp(
            &issuer,
            &session.access_token,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        )
        .await;
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        let names = body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&expected));
        assert!(!names.contains(&absent));
    }

    let briefing_list = post_mcp(
        &issuer,
        &briefing_session.access_token,
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/list" }),
    )
    .await;
    assert_eq!(briefing_list.status(), 200);
    let briefing_list_body = briefing_list.json::<Value>().await.unwrap();
    let briefing_names = briefing_list_body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(briefing_names, vec!["briefing.read"]);

    let briefing = post_mcp(
        &issuer,
        &briefing_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "briefing.read", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(briefing.status(), 200);
    let briefing_body = briefing.json::<Value>().await.unwrap();
    assert_eq!(briefing_body["result"]["isError"], false);
    assert!(briefing_body["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Read-only delegated work only."));

    let denied = post_mcp(
        &issuer,
        &list_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "issues.create", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(denied.status(), 200);
    assert_eq!(
        denied.json::<Value>().await.unwrap()["error"]["code"],
        -32001
    );
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 0);

    let allowed = post_mcp(
        &issuer,
        &list_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "issues.list", "arguments": { "state": "open" } }
        }),
    )
    .await;
    assert_eq!(allowed.status(), 200);
    assert_eq!(
        allowed.json::<Value>().await.unwrap()["result"]["isError"],
        false
    );
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 1);

    let action = gamma_view(context_store.clone())
        .unwrap()
        .into_iter()
        .rfind(|entry| entry.kind == "action")
        .expect("delegated action is logged");
    assert_eq!(
        action.authorized_via.unwrap(),
        vec![list_session.parent_id.clone(), list_session.leaf_id.clone()]
    );
    let payload = action.payload.unwrap();
    assert_eq!(payload["session"]["key"], list_session.session_pub);
    assert_eq!(payload["session"]["mandate_id"], list_session.leaf_id);
    assert!(payload["operation_ref"]["commitment"].is_string());
    assert!(payload.to_string().find("seed").is_none());

    owner_revoke_mandate_id(
        &MASTER,
        "finance",
        &list_session.leaf_id,
        "session disconnected",
        context_store,
        NOW,
        &mut owner_entropy,
    )
    .unwrap();
    let revoked = post_mcp(
        &issuer,
        &list_session.access_token,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list" }),
    )
    .await;
    assert_eq!(revoked.status(), 401);
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 1);
}
