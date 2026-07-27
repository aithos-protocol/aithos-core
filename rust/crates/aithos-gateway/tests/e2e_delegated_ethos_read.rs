//! Lot 1 E2E — the delegated ethos reading surface: a session whose
//! chain carries zone rights sees `ethos.list` / `ethos.read` /
//! `ethos.context` in ITS context only, reads public clear and circle
//! sealed (journalized under the session chain), and loses the whole
//! surface hot on revocation. A session without zone rights sees no
//! ethos tool and every direct call fails closed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use aithos_bundle::Store;
use aithos_core::gamma::Entry;
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry};
use aithos_gateway::config::{GatewayConfig, StoreConfig};
use aithos_gateway::core_bridge::{
    agent_pub_multibase, gamma_view, gateway_kex_pub_multibase, gateway_pub_multibase,
    owner_add_section, owner_deliver_circle_line, owner_grant_context,
    owner_grant_session_delegate, owner_init_context, owner_init_journal, owner_revoke_mandate_id,
    MandateWindow, Runner, SeqEntropy,
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

const NOW: &str = "2026-07-24T12:00:00Z";
const SESSION_END: &str = "2026-07-24T20:00:00Z";
const CALLBACK: &str = "http://127.0.0.1:19411/callback";
const AS_ISSUER: &str = "http://127.0.0.1:4871";
const RESOURCE: &str = "http://127.0.0.1:4871/mcp";
const MASTER: [u8; 32] = [0x35; 32];

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
                "content": [{ "type": "text", "text": "upstream" }],
                "isError": false,
            }
        }))
    }
}

struct IssuedSession {
    access_token: String,
    parent_id: String,
    leaf_id: String,
}

/// Run the full G4/G5 ceremony against the parent whose perimeter
/// matches `parent_marker`, binding `leaf_perimeter` onto the leaf.
#[allow(clippy::too_many_arguments)]
async fn issue_session(
    auth: &AuthServer,
    runner: &Arc<Mutex<Runner>>,
    client_id: &str,
    delegate: &SigningKey,
    parent_marker: Option<&str>,
    leaf_perimeter: Vec<PerimeterEntry>,
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
        .find(|parent| {
            parent.context == "ventes"
                && match parent_marker {
                    Some(marker) => parent.perimeter.iter().any(|entry| entry == marker),
                    None => parent
                        .perimeter
                        .iter()
                        .all(|entry| entry.starts_with("act.") || entry.starts_with("issue")),
                }
        })
        .expect("a matching issuing parent");
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
            id: format!("mandate_01J000000000000000000000E{sequence}"),
            subject: parent.subject.clone(),
            grantee_id: format!("urn:aithos:agent:session-{sequence}"),
            grantee_label: format!("session {sequence}"),
            grantee_pub: &gateway,
            perimeter: leaf_perimeter,
            constraints,
            not_before: NOW.to_owned(),
            not_after: SESSION_END.to_owned(),
            issued_at: "2026-07-24T11:59:59Z".to_owned(),
            nonce: format!("{sequence:02x}").repeat(16),
        },
    )
    .unwrap();
    let leaf_value = serde_json::to_value(&leaf).unwrap();
    let unsigned_grant = runner
        .lock()
        .await
        .prepare_session_grant(
            "ventes",
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
        "ventes",
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
            "ventes",
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
            "ventes",
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
    }
}

async fn post_mcp(base: &str, token: &str, body: Value) -> Value {
    reqwest::Client::new()
        .post(format!("{base}/mcp"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn tool_names(body: &Value) -> Vec<String> {
    body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
        .collect()
}

/// Mirror the public-only carrier produced by the current browser SDK:
/// K1-C keeps path/SID/hash in `indices/public.json` and stores the body
/// by SID, without the historical clear tree index.
fn convert_public_section_to_k1c(store: &mut GatewayStore, root: &std::path::Path, path: &str) {
    let legacy: Value = serde_json::from_slice(
        &store
            .get("e/public/index.json")
            .unwrap()
            .expect("legacy public index"),
    )
    .unwrap();
    let row = legacy["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == path.rsplit('/').next().unwrap())
        .unwrap();
    let sid = row["sid"].as_str().unwrap();
    let body_hash = format!("sha256:{}", row["blob_sha"].as_str().unwrap());
    let legacy_body = format!("e/public/{path}.md");
    let body = store
        .get(&legacy_body)
        .unwrap()
        .expect("legacy public body");
    store
        .put(&format!("public/sections/{sid}.md"), &body)
        .unwrap();
    store
        .put(
            "indices/public.json",
            &serde_json::to_vec_pretty(&json!({
                "sections": [{
                    "sid": sid,
                    "path": path,
                    "body_hash": body_hash,
                }]
            }))
            .unwrap(),
        )
        .unwrap();
    std::fs::remove_file(root.join(&legacy_body)).unwrap();
    std::fs::remove_file(root.join("e/public/index.json")).unwrap();
}

struct Env {
    _scratch: tempfile::TempDir,
    context_store: GatewayStore,
    runner: Arc<Mutex<Runner>>,
    auth: Arc<AuthServer>,
    client_id: String,
    delegate: SigningKey,
    writer_delegate: SigningKey,
    owner_entropy: SeqEntropy,
    upstream: RecordingUpstream,
    issuer: String,
    routing_handle: tokio::task::JoinHandle<()>,
}

/// One fully provisioned gateway + AS + HTTP router: parents A (reads),
/// B (act only) and C (writer, second delegate), content in both zones,
/// the circle line delivered to the gateway key.
async fn build_env(ethos_backend: Arc<aithos_gateway::ethos_backend::EthosBackend>) -> Env {
    let scratch = tempfile::tempdir().unwrap();
    let context_root = scratch.path().join("ventes");
    let journal_root = scratch.path().join("journal");
    let context_store = GatewayStore::from_config(&StoreConfig::Fs {
        root: context_root.clone(),
    })
    .unwrap();
    let journal_store = GatewayStore::from_config(&StoreConfig::Fs {
        root: journal_root.clone(),
    })
    .unwrap();
    let delegate_seed = [0x51; 32];
    let gateway_seed = [0x52; 32];
    let delegate = SigningKey::from_bytes(&delegate_seed);
    // A second delegate carries the writer sessions: the AS caps ACTIVE
    // sessions per delegate, and this test needs four live ones.
    let writer_delegate = SigningKey::from_bytes(&[0x61; 32]);
    let writer_delegate_pub =
        aithos_core::wire::ed25519_pub_to_multibase(&writer_delegate.verifying_key().to_bytes());
    let keyholder = Keyholder::from_entropy(delegate_seed, gateway_seed);
    let delegate_pub = agent_pub_multibase(&keyholder);
    let gateway_pub = gateway_pub_multibase(&keyholder);
    let gateway_kex_pub = gateway_kex_pub_multibase(&keyholder);
    let mut owner_entropy = SeqEntropy::default();
    let setup_at = "2026-07-24T10:00:00Z";
    owner_init_context(
        &MASTER,
        "ventes",
        context_store.clone(),
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    let window = MandateWindow {
        not_before: setup_at.to_owned(),
        not_after: "2026-07-25T00:00:00Z".to_owned(),
    };
    owner_grant_context(
        &MASTER,
        "ventes",
        &delegate_pub,
        &gateway_pub,
        &["issues.list".to_owned()],
        context_store.clone(),
        &window,
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    // The governed content: one clear public section, one sealed circle
    // section (outside the briefing shelves).
    owner_add_section(
        &MASTER,
        "ventes",
        "public",
        "produits/catalogue",
        "Catalogue 2026 — offre publique.",
        context_store.clone(),
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    owner_add_section(
        &MASTER,
        "ventes",
        "circle",
        "notes/marge",
        "Marge de négociation max 8%.",
        context_store.clone(),
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    // `self` is never delegable: the gesture itself refuses it.
    let refused = owner_grant_session_delegate(
        &MASTER,
        "ventes",
        &delegate_pub,
        RESOURCE,
        &["read.self".to_owned()],
        context_store.clone(),
        &window,
        setup_at,
        &mut owner_entropy,
    );
    assert!(refused.is_err(), "read.self must be refused at the gesture");
    // Parent A: connector action + BOTH zone reads. Parent B: action only.
    owner_grant_session_delegate(
        &MASTER,
        "ventes",
        &delegate_pub,
        RESOURCE,
        &[
            "issues.list".to_owned(),
            "read.public".to_owned(),
            "read.circle".to_owned(),
        ],
        context_store.clone(),
        &window,
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    owner_grant_session_delegate(
        &MASTER,
        "ventes",
        &delegate_pub,
        RESOURCE,
        &["issues.list".to_owned()],
        context_store.clone(),
        &window,
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    // Parent C: the writer authority — read + full write on circle, plus
    // an append on public (the gateway must refuse public mutations even
    // when the certificate would allow them: circle only this pass).
    owner_grant_session_delegate(
        &MASTER,
        "ventes",
        &writer_delegate_pub,
        RESOURCE,
        &[
            "read.circle".to_owned(),
            "write.circle".to_owned(),
            "append.public".to_owned(),
        ],
        context_store.clone(),
        &window,
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    // The physics half for circle: the zone line goes to the GATEWAY
    // key — the session leaf grantee that will open the bodies.
    owner_deliver_circle_line(
        &MASTER,
        "ventes",
        &gateway_pub,
        context_store.clone(),
        setup_at,
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
        setup_at,
        &mut owner_entropy,
    )
    .unwrap();
    let quote = |path: &std::path::Path| serde_json::to_string(&path.to_string_lossy()).unwrap();
    let config = GatewayConfig::from_yaml(&format!(
        "listen: 127.0.0.1:0\ncontexts:\n  - name: ventes\n    store: {{ kind: fs, root: {} }}\n    upstream_mcp: http://127.0.0.1:9/mcp\n    tools:\n      issues.list: read\njournal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(&context_root),
        quote(&journal_root),
    ))
    .unwrap();
    let runner = Arc::new(Mutex::new(
        Runner::open(&config, keyholder, || Box::new(SeqEntropy::default())).unwrap(),
    ));
    convert_public_section_to_k1c(
        &mut context_store.clone(),
        &context_root,
        "produits/catalogue",
    );

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}");
    let auth = Arc::new(AuthServer::new_production_with_state(
        AdapterKey::from_seed([0x53; 32]),
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
    let upstream = RecordingUpstream::default();
    let routing = Arc::new(McpRouter {
        runner: Arc::clone(&runner),
        upstreams: BTreeMap::from([("ventes".to_owned(), upstream.clone())]),
        ethos_backend,
        dynamic_upstreams: empty_dynamic_upstreams(),
        clock: Arc::new(|| NOW.to_owned()),
        session_entropy: StdMutex::new(Box::new(SeqEntropy::default())),
        oauth: Some(Arc::clone(&auth)),
        browser_origins: Arc::new(BTreeSet::new()),
    });
    let app = router_multi(Arc::clone(&routing)).merge(router_oauth(routing));
    let routing_handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Env {
        _scratch: scratch,
        context_store,
        runner,
        auth,
        client_id,
        delegate,
        writer_delegate,
        owner_entropy,
        upstream,
        issuer,
        routing_handle,
    }
}

#[tokio::test]
async fn delegated_ethos_read_surface_matches_the_session_chain_exactly() {
    let env = build_env(aithos_gateway::ethos_backend::client_shadow_ethos_backend()).await;
    let issuer = env.issuer.clone();
    let ethos_session = issue_session(
        &env.auth,
        &env.runner,
        &env.client_id,
        &env.delegate,
        Some("read.public"),
        vec![
            PerimeterEntry::parse(&op_for_tool("issues.list")).unwrap(),
            PerimeterEntry::parse("read.public").unwrap(),
            PerimeterEntry::parse("read.circle").unwrap(),
        ],
        1,
    )
    .await;
    let act_session = issue_session(
        &env.auth,
        &env.runner,
        &env.client_id,
        &env.delegate,
        None,
        vec![PerimeterEntry::parse(&op_for_tool("issues.list")).unwrap()],
        2,
    )
    .await;
    let context_store = env.context_store.clone();
    let upstream = env.upstream.clone();
    let _ = &upstream;

    // --- surface: the covered session lists the native read tools ---
    let listed = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    let names = tool_names(&listed);
    for expected in ["issues.list", "ethos.read", "ethos.list", "ethos.context"] {
        assert!(names.contains(&expected.to_owned()), "missing {expected}");
    }
    // The descriptions name the session's context and zones only.
    let read_tool = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "ethos.read")
        .unwrap();
    assert!(read_tool["description"]
        .as_str()
        .unwrap()
        .contains("ventes: public, circle"));

    // --- anti over-exposure: the act-only session sees NO ethos tool ---
    let listed_act = post_mcp(
        &issuer,
        &act_session.access_token,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    let act_names = tool_names(&listed_act);
    assert_eq!(act_names, vec!["issues.list"]);

    // --- initialize mirrors the same surface, per session ---
    let init = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({ "jsonrpc": "2.0", "id": 3, "method": "initialize" }),
    )
    .await;
    assert!(init["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("ventes: public, circle"));
    let init_act = post_mcp(
        &issuer,
        &act_session.access_token,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "initialize" }),
    )
    .await;
    assert!(init_act["result"].get("instructions").is_none());

    // --- ethos.list: the covered skeleton, this context only ---
    let skeleton = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "ethos.list", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(skeleton["result"]["isError"], false, "{skeleton}");
    let text = skeleton["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("produits/catalogue"));
    assert!(text.contains("notes/marge"));
    assert!(text.contains("\"context\":\"ventes\"") || text.contains("\"context\": \"ventes\""));

    // --- ethos.read public: clear body ---
    let public_read = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "ethos.read",
                        "arguments": { "zone": "public", "path": "produits/catalogue" } }
        }),
    )
    .await;
    assert_eq!(public_read["result"]["isError"], false);
    assert!(public_read["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Catalogue 2026"));

    // --- ethos.read circle: sealed body under the session chain ---
    let circle_read = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": "ethos.read",
                        "arguments": { "zone": "circle", "path": "notes/marge" } }
        }),
    )
    .await;
    assert_eq!(circle_read["result"]["isError"], false, "{circle_read}");
    assert!(circle_read["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Marge de négociation max 8%"));
    // The sealed read is journalized under the SESSION chain.
    let read_entry = gamma_view(context_store.clone())
        .unwrap()
        .into_iter()
        .rfind(|entry| entry.kind == "ethos.read")
        .expect("the delegated circle read is on the record");
    assert_eq!(
        read_entry.authorized_via.unwrap(),
        vec![
            ethos_session.parent_id.clone(),
            ethos_session.leaf_id.clone()
        ]
    );

    // --- cross-context refusal ---
    let crossed = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 8, "method": "tools/call",
            "params": { "name": "ethos.read",
                        "arguments": { "context": "autre", "zone": "public",
                                       "path": "produits/catalogue" } }
        }),
    )
    .await;
    assert!(crossed["error"]["message"]
        .as_str()
        .unwrap()
        .contains("differs from delegated context"));

    // --- self refusal ---
    let self_read = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 9, "method": "tools/call",
            "params": { "name": "ethos.read",
                        "arguments": { "zone": "self", "path": "notes/marge" } }
        }),
    )
    .await;
    assert!(self_read["error"]["message"]
        .as_str()
        .unwrap()
        .contains("read.self"));

    // --- visibility is not authorization: uncovered direct calls fail closed ---
    for (id_num, arguments) in [
        (
            10,
            json!({ "zone": "public", "path": "produits/catalogue" }),
        ),
        (11, json!({ "zone": "circle", "path": "notes/marge" })),
    ] {
        let denied = post_mcp(
            &issuer,
            &act_session.access_token,
            json!({
                "jsonrpc": "2.0", "id": id_num, "method": "tools/call",
                "params": { "name": "ethos.read", "arguments": arguments }
            }),
        )
        .await;
        assert!(
            denied.get("error").is_some(),
            "uncovered ethos.read must refuse: {denied}"
        );
    }
    let denied_list = post_mcp(
        &issuer,
        &act_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 12, "method": "tools/call",
            "params": { "name": "ethos.list", "arguments": {} }
        }),
    )
    .await;
    // An uncovered ethos.list is not an error but serves an EMPTY skeleton.
    assert_eq!(denied_list["result"]["isError"], false);
    assert!(denied_list["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("\"contexts\":[]"));

    env.routing_handle.abort();
}

#[tokio::test]
async fn delegated_ethos_mutations_are_bounded_by_the_session_verbs() {
    let mut env = build_env(aithos_gateway::ethos_backend::client_provider_ethos_backend()).await;
    let issuer = env.issuer.clone();
    // Contributor: append only — create and edit, never delete.
    let writer_session = issue_session(
        &env.auth,
        &env.runner,
        &env.client_id,
        &env.writer_delegate,
        Some("write.circle"),
        vec![
            PerimeterEntry::parse("read.circle").unwrap(),
            PerimeterEntry::parse("append.circle").unwrap(),
        ],
        3,
    )
    .await;
    // Full writer: write covers delete (§04.2 — said in clear by the UI).
    let full_session = issue_session(
        &env.auth,
        &env.runner,
        &env.client_id,
        &env.writer_delegate,
        Some("write.circle"),
        vec![
            PerimeterEntry::parse("read.circle").unwrap(),
            PerimeterEntry::parse("write.circle").unwrap(),
            PerimeterEntry::parse("append.public").unwrap(),
        ],
        4,
    )
    .await;
    // A read-only session for the adversarial checks.
    let ethos_session = issue_session(
        &env.auth,
        &env.runner,
        &env.client_id,
        &env.delegate,
        Some("read.public"),
        vec![
            PerimeterEntry::parse("read.public").unwrap(),
            PerimeterEntry::parse("read.circle").unwrap(),
        ],
        5,
    )
    .await;
    let context_store = env.context_store.clone();
    let upstream = env.upstream.clone();

    // ==================== lot 4: delegated mutations ====================

    // Surfaces: the read-only session sees NO write tool; the contributor
    // sees create+edit; the full writer also sees delete.
    let read_names = tool_names(
        &post_mcp(
            &issuer,
            &ethos_session.access_token,
            json!({ "jsonrpc": "2.0", "id": 20, "method": "tools/list" }),
        )
        .await,
    );
    assert!(!read_names
        .iter()
        .any(|name| name.starts_with("ethos.c") && name != "ethos.context"));
    assert!(!read_names.contains(&"ethos.edit".to_owned()));
    assert!(!read_names.contains(&"ethos.delete".to_owned()));
    let writer_names = tool_names(
        &post_mcp(
            &issuer,
            &writer_session.access_token,
            json!({ "jsonrpc": "2.0", "id": 21, "method": "tools/list" }),
        )
        .await,
    );
    for expected in ["ethos.create", "ethos.edit"] {
        assert!(
            writer_names.contains(&expected.to_owned()),
            "missing {expected}"
        );
    }
    assert!(!writer_names.contains(&"ethos.delete".to_owned()));
    let full_names = tool_names(
        &post_mcp(
            &issuer,
            &full_session.access_token,
            json!({ "jsonrpc": "2.0", "id": 22, "method": "tools/list" }),
        )
        .await,
    );
    assert!(full_names.contains(&"ethos.delete".to_owned()));

    // Create under the contributor session.
    let created = post_mcp(
        &issuer,
        &writer_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 23, "method": "tools/call",
            "params": { "name": "ethos.create",
                        "arguments": { "zone": "circle", "folder": "notes",
                                       "name": "compte-rendu", "body": "CR délégué." } }
        }),
    )
    .await;
    assert_eq!(created["result"]["isError"], false, "{created}");
    let created_payload: Value =
        serde_json::from_str(created["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let created_digest = created_payload["digest"].as_str().unwrap().to_owned();
    // The delegated mutation is journalized under the SESSION chain.
    let add_entry = gamma_view(context_store.clone())
        .unwrap()
        .into_iter()
        .rfind(|entry| entry.kind == "section.add")
        .expect("delegated creation on the record");
    assert_eq!(
        add_entry.authorized_via.unwrap(),
        vec![
            writer_session.parent_id.clone(),
            writer_session.leaf_id.clone()
        ]
    );

    // Edit with the fresh digest succeeds; a stale digest refuses.
    let edited = post_mcp(
        &issuer,
        &writer_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 24, "method": "tools/call",
            "params": { "name": "ethos.edit",
                        "arguments": { "zone": "circle", "path": "notes/compte-rendu",
                                       "body": "CR délégué, corrigé.",
                                       "expected_digest": created_digest } }
        }),
    )
    .await;
    assert_eq!(edited["result"]["isError"], false, "{edited}");
    let stale = post_mcp(
        &issuer,
        &writer_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 25, "method": "tools/call",
            "params": { "name": "ethos.edit",
                        "arguments": { "zone": "circle", "path": "notes/compte-rendu",
                                       "body": "écrasement concurrent",
                                       "expected_digest": created_digest } }
        }),
    )
    .await;
    assert!(stale["error"]["message"]
        .as_str()
        .unwrap()
        .contains("stale precondition"));

    // Adversarial: delete without the `delete` verb; write on a zone the
    // session can only read; public mutation despite an append.public
    // certificate; self; read-only session creating.
    let no_delete = post_mcp(
        &issuer,
        &writer_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 26, "method": "tools/call",
            "params": { "name": "ethos.delete",
                        "arguments": { "zone": "circle", "path": "notes/compte-rendu" } }
        }),
    )
    .await;
    assert!(
        no_delete.get("error").is_some(),
        "delete without verb must refuse"
    );
    let read_only_create = post_mcp(
        &issuer,
        &ethos_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 27, "method": "tools/call",
            "params": { "name": "ethos.create",
                        "arguments": { "zone": "circle", "folder": "notes",
                                       "name": "intrusion", "body": "x" } }
        }),
    )
    .await;
    assert!(
        read_only_create.get("error").is_some(),
        "read-only create must refuse"
    );
    let public_write = post_mcp(
        &issuer,
        &full_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 28, "method": "tools/call",
            "params": { "name": "ethos.create",
                        "arguments": { "zone": "public", "folder": "produits",
                                       "name": "intrus", "body": "x" } }
        }),
    )
    .await;
    assert!(public_write["error"]["message"]
        .as_str()
        .unwrap()
        .contains("circle only"));
    let self_write = post_mcp(
        &issuer,
        &full_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 29, "method": "tools/call",
            "params": { "name": "ethos.create",
                        "arguments": { "zone": "self", "name": "intrus", "body": "x" } }
        }),
    )
    .await;
    assert!(
        self_write.get("error").is_some(),
        "self mutation must refuse"
    );

    // Delete under the full session: stale digest refuses, fresh one lands,
    // and the row is gone from the skeleton afterwards.
    let reread: Value = serde_json::from_str(
        post_mcp(
            &issuer,
            &full_session.access_token,
            json!({
                "jsonrpc": "2.0", "id": 30, "method": "tools/call",
                "params": { "name": "ethos.read",
                            "arguments": { "zone": "circle", "path": "notes/compte-rendu" } }
            }),
        )
        .await["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(reread["text"], "CR délégué, corrigé.");
    let fresh_digest = reread["digest"].as_str().unwrap().to_owned();
    let stale_delete = post_mcp(
        &issuer,
        &full_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 31, "method": "tools/call",
            "params": { "name": "ethos.delete",
                        "arguments": { "zone": "circle", "path": "notes/compte-rendu",
                                       "expected_digest": created_digest } }
        }),
    )
    .await;
    assert!(stale_delete["error"]["message"]
        .as_str()
        .unwrap()
        .contains("stale precondition"));
    let deleted = post_mcp(
        &issuer,
        &full_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 32, "method": "tools/call",
            "params": { "name": "ethos.delete",
                        "arguments": { "zone": "circle", "path": "notes/compte-rendu",
                                       "expected_digest": fresh_digest } }
        }),
    )
    .await;
    assert_eq!(deleted["result"]["isError"], false, "{deleted}");
    let delete_entry = gamma_view(context_store.clone())
        .unwrap()
        .into_iter()
        .rfind(|entry| entry.kind == "section.delete")
        .expect("delegated deletion on the record");
    assert_eq!(
        delete_entry.authorized_via.unwrap(),
        vec![full_session.parent_id.clone(), full_session.leaf_id.clone()]
    );
    let skeleton_after = post_mcp(
        &issuer,
        &full_session.access_token,
        json!({
            "jsonrpc": "2.0", "id": 33, "method": "tools/call",
            "params": { "name": "ethos.list", "arguments": {} }
        }),
    )
    .await;
    assert!(!skeleton_after["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("compte-rendu"));

    // No upstream was ever touched by native reads or writes.
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 0);

    // --- revocation drops the whole surface hot, no restart ---
    owner_revoke_mandate_id(
        &MASTER,
        "ventes",
        &ethos_session.leaf_id,
        "session closed",
        context_store,
        NOW,
        &mut env.owner_entropy,
    )
    .unwrap();
    let revoked = reqwest::Client::new()
        .post(format!("{issuer}/mcp"))
        .bearer_auth(&ethos_session.access_token)
        .json(&json!({ "jsonrpc": "2.0", "id": 13, "method": "tools/list" }))
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), 401);
}
