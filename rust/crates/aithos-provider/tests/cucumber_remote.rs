//! BDD acceptance harness for `tests/features/remote/` — the P3
//! `RemoteStore` CLIENT contract (aithos-bundle, feature `remote`)
//! exercised against the REAL aithos-provider service bound on a real
//! localhost socket. Nothing on the wire is mocked: the service is the
//! same `build_router`/`AppState` the store harness proves, fronted by
//! a tiny TCP proxy whose only job is FAULT injection (dropped
//! connections, dead listener) for the retry scenarios, plus a counting
//! middleware ("the service saw exactly N requests").
//!
//! Fixtures are the committed vectors (p1 owner keys + mandate, p7 real
//! bundle publications) — never re-invented crypto. The client's
//! signer, clock and nonce entropy are INJECTED (arbitrage ② and the
//! §00 purity rule); the clock is the REAL one here (the service runs
//! on its real clock too — skew ~0, exactly the deployed situation).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use aithos_bundle::entropy::EntropySource;
use aithos_bundle::remote::{EnvelopeSigner, KeySigner, RemoteError, RemoteStore};
use aithos_bundle::Store as _;
use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::{MemObjects, ObjectStore as _};
use aithos_provider::service::{build_router, AppState};
use aithos_provider::time::render_rfc3339z;
use cucumber::{given, then, when, World as _};
use ed25519_dalek::SigningKey;

// ------------------------------------------------------------- fixtures

struct Fixtures {
    tenant: String,
    did: String,
    did_json: String,
    mandate_id: String,
    mandate_jcs: String,
    content_sk: SigningKey,
    agent_sk: SigningKey,
    genesis_manifest: Vec<u8>,
    genesis_head: String,
    successor_manifest: Vec<u8>,
    successor_head: String,
    gamma_entry: Vec<u8>,
    gamma_entry_head: String,
}

fn sk_from_hex(hex_seed: &str) -> SigningKey {
    SigningKey::from_bytes(&hex::decode(hex_seed).unwrap().try_into().unwrap())
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let vectors = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");
        let read = |name: &str| -> serde_json::Value {
            serde_json::from_str(&std::fs::read_to_string(format!("{vectors}/{name}")).unwrap())
                .unwrap()
        };
        let p1 = read("p1-store-envelope.json");
        let a1 = read("a1-genesis.json");
        let p7 = read("p7-store-publication.json");
        let seed: [u8; 32] = hex::decode(a1["seed_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let content_sk = SigningKey::from_bytes(&aithos_core::derive::derive_key(
            aithos_core::derive::CTX_CONTENT_SIGN,
            &seed,
        ));
        let mandate: serde_json::Value =
            serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
        let case = |list: &str, name: &str| -> serde_json::Value {
            p7[list]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap_or_else(|| panic!("p7 case {name}"))
                .clone()
        };
        let genesis = case("manifest_cases", "genesis_publish");
        let successor = case("manifest_cases", "publish_ok");
        let gamma = case("gamma_cases", "append_genesis");
        Fixtures {
            tenant: p1["tenant"].as_str().unwrap().to_owned(),
            did: p1["did"].as_str().unwrap().to_owned(),
            did_json: p1["did_json_jcs"].as_str().unwrap().to_owned(),
            mandate_id: mandate["id"].as_str().unwrap().to_owned(),
            mandate_jcs: p1["mandate_jcs"].as_str().unwrap().to_owned(),
            content_sk,
            agent_sk: sk_from_hex(p1["agent_sk_hex"].as_str().unwrap()),
            genesis_manifest: genesis["body_jcs"].as_str().unwrap().as_bytes().to_vec(),
            genesis_head: genesis["expect"]["new_head"].as_str().unwrap().to_owned(),
            successor_manifest: successor["body_jcs"].as_str().unwrap().as_bytes().to_vec(),
            successor_head: successor["expect"]["new_head"].as_str().unwrap().to_owned(),
            gamma_entry: gamma["entry_jcs"].as_str().unwrap().as_bytes().to_vec(),
            gamma_entry_head: gamma["expect"]["new_head"].as_str().unwrap().to_owned(),
        }
    })
}

/// Deterministic test entropy, SALTED per client — two clients on the
/// same key must never mint the same nonce (A.2 #6 would refuse the
/// second as `nonce_replayed`).
struct SaltedEntropy {
    salt: u64,
    counter: u64,
}

impl SaltedEntropy {
    fn fresh() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            salt: NEXT.fetch_add(1, Ordering::SeqCst),
            counter: 0,
        }
    }
}

impl EntropySource for SaltedEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        let mut out = Vec::new();
        while out.len() < buf.len() {
            self.counter += 1;
            out.extend_from_slice(
                blake3::hash(format!("remote-bdd-{}-{}", self.salt, self.counter).as_bytes())
                    .as_bytes(),
            );
        }
        buf.copy_from_slice(&out[..buf.len()]);
    }
}

// ------------------------------------------------------------ the wire

/// Fault-injection controls of the TCP proxy fronting the service.
#[derive(Default)]
struct ProxyCtl {
    drop_next: AtomicU32,
    dead: AtomicBool,
}

struct Wire {
    proxy_url: String,
    objects: Arc<MemObjects>,
    #[allow(dead_code)]
    heads: Arc<MemHeads>,
    /// Requests the SERVICE actually saw, by absolute path.
    counters: Arc<Mutex<HashMap<String, u32>>>,
    ctl: Arc<ProxyCtl>,
}

async fn boot_wire() -> Wire {
    let f = fixtures();
    // The enrollment bootstrap: tenant + DID + did.json + the p1 cert —
    // exactly the vectors_replay `case_bootstrap` shape.
    let bootstrap = serde_json::json!({
        "tenants": [{
            "tenant": f.tenant,
            "dids": [{
                "did": f.did,
                "did_json": f.did_json,
                "objects": [{
                    "key": format!("certs/{}.json", f.mandate_id),
                    "utf8": f.mandate_jcs,
                }],
            }],
        }],
    });
    let (control, preloads, _head_seeds) =
        ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("fixture bootstrap");
    let objects = Arc::new(MemObjects::new());
    for (tenant, did, key, bytes) in preloads {
        objects.put(&tenant, &did, &key, bytes).await.unwrap();
    }
    let heads = Arc::new(MemHeads::new());

    // Bind the service, then the proxy in front of it.
    let service_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let service_addr = service_listener.local_addr().unwrap();
    let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    let state = Arc::new(AppState {
        control: Arc::new(control),
        objects: objects.clone(),
        heads: heads.clone(),
        deposit_locks: Default::default(),
        nonces: Arc::new(MemNonces::new(600)),
        dns: Arc::new(MemDnsTxt::new()),
        acme: AcmeState::new(),
        // The authority the CLIENT addresses (A.2 host = request
        // authority): the proxy's socket.
        authority: format!("127.0.0.1:{}", proxy_addr.port()),
        browser_origins: Arc::default(),
        test_now_enabled: false,
    });

    // Counting middleware: every request the service actually receives.
    let counters: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    let router = build_router(state).layer(axum::middleware::from_fn_with_state(
        counters.clone(),
        |axum::extract::State(counters): axum::extract::State<Arc<Mutex<HashMap<String, u32>>>>,
         request: axum::extract::Request,
         next: axum::middleware::Next| async move {
            *counters
                .lock()
                .expect("counters")
                .entry(format!("{} {}", request.method(), request.uri().path()))
                .or_insert(0) += 1;
            next.run(request).await
        },
    ));
    tokio::spawn(async move {
        axum::serve(service_listener, router).await.ok();
    });

    // The fault-injection proxy: transparent byte pipe, or a dropped
    // connection when the scenario armed one.
    let ctl = Arc::new(ProxyCtl::default());
    let proxy_ctl = ctl.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut inbound, _)) = proxy_listener.accept().await else {
                break;
            };
            if proxy_ctl.dead.load(Ordering::SeqCst) {
                drop(inbound); // dead service: accepted then reset
                continue;
            }
            let armed = proxy_ctl
                .drop_next
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok();
            if armed {
                drop(inbound); // injected transient fault
                continue;
            }
            tokio::spawn(async move {
                if let Ok(mut outbound) = tokio::net::TcpStream::connect(service_addr).await {
                    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                }
            });
        }
    });

    Wire {
        proxy_url: format!("http://127.0.0.1:{}", proxy_addr.port()),
        objects,
        heads,
        counters,
        ctl,
    }
}

// --------------------------------------------------------------- world

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct RemoteClientWorld {
    wire: Option<Wire>,
    signer: Option<Arc<dyn EnvelopeSigner>>,
    client: Option<Arc<Mutex<RemoteStore>>>,
    /// get() results, in call order.
    gets: Vec<std::io::Result<Option<Vec<u8>>>>,
    /// The bytes a Given stored (the "exact stored bytes" oracle).
    stored: Option<Vec<u8>>,
    stored_paths: Vec<String>,
    put_result: Option<std::io::Result<()>>,
    put_body: Option<Vec<u8>>,
    list_result: Option<std::io::Result<Vec<String>>>,
    publish_acks: Vec<Result<aithos_bundle::remote::Ack, RemoteError>>,
    append_result: Option<Result<aithos_bundle::remote::Ack, RemoteError>>,
    /// The relative path the counter Thens inspect.
    counted_path: Option<String>,
    /// The last `/batch` or `/sync` pack (P4).
    pack: Option<Result<Vec<aithos_bundle::remote::PackPart>, RemoteError>>,
}

impl std::fmt::Debug for RemoteClientWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RemoteClientWorld")
    }
}

fn real_now() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    // Whole-second render, the wire's RFC 3339 Z convention.
    render_rfc3339z(ms - ms.rem_euclid(1000))
}

impl RemoteClientWorld {
    fn new() -> Self {
        Self {
            wire: None,
            signer: None,
            client: None,
            gets: Vec::new(),
            stored: None,
            stored_paths: Vec::new(),
            put_result: None,
            put_body: None,
            list_result: None,
            publish_acks: Vec::new(),
            append_result: None,
            counted_path: None,
            pack: None,
        }
    }

    fn wire(&self) -> &Wire {
        self.wire.as_ref().expect("the service is listening")
    }

    fn build_client(&self, signer: Arc<dyn EnvelopeSigner>) -> RemoteStore {
        let f = fixtures();
        RemoteStore::new(
            &self.wire().proxy_url,
            &f.tenant,
            &f.did,
            signer,
            Arc::new(real_now),
            Box::new(SaltedEntropy::fresh()),
        )
        .expect("client builds")
        // The test sleeper RECORDS the backoff (the tap) without
        // actually waiting — the schedule is asserted, not endured.
        .with_sleeper(Arc::new(|_| {}))
    }

    fn client(&self) -> Arc<Mutex<RemoteStore>> {
        self.client.clone().expect("a client is configured")
    }

    async fn client_get(&mut self, relative: &str) {
        let client = self.client();
        let relative = relative.to_owned();
        let result =
            tokio::task::spawn_blocking(move || client.lock().expect("client").get(&relative))
                .await
                .expect("join");
        self.gets.push(result);
    }

    fn abs(&self, relative: &str) -> String {
        let f = fixtures();
        format!("/t/{}/{}/{relative}", f.tenant, f.did)
    }

    /// GET hits the service saw for a relative path (the counter Thens
    /// are all about reads — a publish PUT on the same path never mixes).
    fn count_of(&self, relative: &str) -> u32 {
        let key = format!("GET {}", self.abs(relative));
        *self
            .wire()
            .counters
            .lock()
            .expect("counters")
            .get(&key)
            .unwrap_or(&0)
    }

    fn last_wire_error(&self) -> RemoteError {
        let err = match self.gets.last() {
            Some(Err(e)) => e,
            other => panic!("expected the last get to fail, got {other:?}"),
        };
        err.get_ref()
            .and_then(|inner| inner.downcast_ref::<RemoteError>())
            .expect("a typed RemoteError rides the io::Error")
            .clone()
    }
}

// ---------------------------------------------------------- background

#[given(expr = "the real store service listens on a local socket")]
async fn service_listens(world: &mut RemoteClientWorld) {
    world.wire = Some(boot_wire().await);
}

#[given(expr = "the tenant {string} is enrolled and bound to the vector DID")]
async fn tenant_enrolled(world: &mut RemoteClientWorld, tenant: String) {
    assert_eq!(tenant, fixtures().tenant, "fixture is the committed vector");
    let _ = world.wire();
}

#[given(expr = "the vector did.json is stored for that DID")]
async fn did_json_stored(world: &mut RemoteClientWorld) {
    let f = fixtures();
    let stored = world
        .wire()
        .objects
        .get(&f.tenant, &f.did, "did.json")
        .await
        .unwrap()
        .expect("did.json preloaded by the enrollment bootstrap");
    assert_eq!(stored, f.did_json.as_bytes());
}

#[given(expr = "an owner content signer from the p1 vectors is injected")]
async fn owner_signer(world: &mut RemoteClientWorld) {
    world.signer = Some(Arc::new(KeySigner::owner(
        "#content",
        fixtures().content_sk.clone(),
    )));
}

#[given(expr = "a RemoteStore client points at the service for tenant {string} and the vector DID")]
async fn client_points(world: &mut RemoteClientWorld, tenant: String) {
    assert_eq!(tenant, fixtures().tenant);
    let signer = world.signer.clone().expect("a signer is injected");
    world.client = Some(Arc::new(Mutex::new(world.build_client(signer))));
}

// ------------------------------------------------------------- givens

#[given(expr = "the artifact {string} is stored with body {string}")]
async fn artifact_stored(world: &mut RemoteClientWorld, relative: String, body: String) {
    let f = fixtures();
    world
        .wire()
        .objects
        .put(&f.tenant, &f.did, &relative, body.clone().into_bytes())
        .await
        .unwrap();
    world.stored = Some(body.into_bytes());
}

#[given(expr = "the artifact {string} is stored with 4096 opaque bytes")]
async fn artifact_stored_opaque(world: &mut RemoteClientWorld, relative: String) {
    let f = fixtures();
    let body = vec![0xABu8; 4096];
    world
        .wire()
        .objects
        .put(&f.tenant, &f.did, &relative, body.clone())
        .await
        .unwrap();
    world.stored = Some(body);
}

#[given(expr = "{int} artifacts are stored under {string}")]
async fn artifacts_stored_under(world: &mut RemoteClientWorld, n: u64, prefix: String) {
    let f = fixtures();
    for i in 0..n {
        let key = format!("{prefix}note-{i}.md");
        world
            .wire()
            .objects
            .put(&f.tenant, &f.did, &key, format!("note {i}").into_bytes())
            .await
            .unwrap();
        world.stored_paths.push(key);
    }
    world.stored_paths.sort();
}

#[given(expr = "the p1 mandate cert is stored by the enrollment")]
async fn cert_stored(world: &mut RemoteClientWorld) {
    let f = fixtures();
    let key = format!("certs/{}.json", f.mandate_id);
    let stored = world
        .wire()
        .objects
        .get(&f.tenant, &f.did, &key)
        .await
        .unwrap()
        .expect("the enrollment bootstrap stored the p1 cert");
    world.stored = Some(stored);
}

#[given(expr = "a p7 genesis publication package is loaded")]
async fn p7_loaded(_world: &mut RemoteClientWorld) {
    let f = fixtures();
    assert!(!f.genesis_manifest.is_empty());
    assert!(!f.successor_manifest.is_empty());
}

#[given(expr = "the client published the genesis manifest")]
async fn client_published_genesis(world: &mut RemoteClientWorld) {
    let client = world.client();
    let bytes = fixtures().genesis_manifest.clone();
    let ack = tokio::task::spawn_blocking(move || {
        client.lock().expect("client").publish_manifest(&bytes)
    })
    .await
    .expect("join")
    .expect("genesis publish accepted");
    assert_eq!(ack.head, fixtures().genesis_head, "vector head, byte-exact");
    world.publish_acks.push(Ok(ack));
}

#[given(expr = "another writer already advanced the manifest head on the service")]
async fn competitor_publishes(world: &mut RemoteClientWorld) {
    // A REAL second client (its own tracked state) wins the race on the
    // wire — nothing is seeded behind the service's back.
    let competitor = world.build_client(Arc::new(KeySigner::owner(
        "#content",
        fixtures().content_sk.clone(),
    )));
    let bytes = fixtures().successor_manifest.clone();
    let ack = tokio::task::spawn_blocking(move || competitor.publish_manifest(&bytes))
        .await
        .expect("join")
        .expect("the competitor's publish is accepted");
    assert_eq!(ack.head, fixtures().successor_head);
}

#[given(expr = "the client knows the stored gamma head is empty")]
async fn gamma_head_empty(world: &mut RemoteClientWorld) {
    let client = world.client();
    let heads = tokio::task::spawn_blocking(move || client.lock().expect("client").heads())
        .await
        .expect("join")
        .expect("/heads serves");
    assert!(heads.gamma.is_none(), "no gamma appended yet");
}

#[given(expr = "another writer already appended to the gamma on the service")]
async fn competitor_appends(world: &mut RemoteClientWorld) {
    let competitor = world.build_client(Arc::new(KeySigner::owner(
        "#content",
        fixtures().content_sk.clone(),
    )));
    let entry = fixtures().gamma_entry.clone();
    let ack = tokio::task::spawn_blocking(move || competitor.append_gamma(&entry))
        .await
        .expect("join")
        .expect("the competitor's append is accepted");
    assert_eq!(ack.head, fixtures().gamma_entry_head);
}

#[given(expr = "the service drops the next {int} connections")]
async fn drops_next(world: &mut RemoteClientWorld, n: u64) {
    world.wire().ctl.drop_next.store(n as u32, Ordering::SeqCst);
}

#[given(expr = "the service stops listening")]
async fn service_dies(world: &mut RemoteClientWorld) {
    world.wire().ctl.dead.store(true, Ordering::SeqCst);
}

// -------------------------------------------------------------- whens

#[when(expr = "the client calls get on {string}")]
async fn client_get_once(world: &mut RemoteClientWorld, relative: String) {
    world.counted_path = Some(relative.clone());
    world.client_get(&relative).await;
}

#[when(expr = "the client calls get on {string} twice")]
async fn client_get_twice(world: &mut RemoteClientWorld, relative: String) {
    world.counted_path = Some(relative.clone());
    world.client_get(&relative).await;
    world.client_get(&relative).await;
}

#[when(expr = "the client calls get on that blob twice")]
async fn client_get_blob_twice(world: &mut RemoteClientWorld) {
    let relative = "e/circle/blobs/01000000000000000000000000.enc".to_owned();
    world.counted_path = Some(relative.clone());
    world.client_get(&relative).await;
    world.client_get(&relative).await;
}

#[when(expr = "the client calls get on the p1 cert path twice")]
async fn client_get_cert_twice(world: &mut RemoteClientWorld) {
    let relative = format!("certs/{}.json", fixtures().mandate_id);
    world.counted_path = Some(relative.clone());
    world.client_get(&relative).await;
    world.client_get(&relative).await;
}

#[when(expr = "the client calls put on {string} with 4096 opaque bytes")]
async fn client_put_opaque(world: &mut RemoteClientWorld, relative: String) {
    let body = vec![0xABu8; 4096];
    world.put_body = Some(body.clone());
    world.counted_path = Some(relative.clone());
    let client = world.client();
    let result =
        tokio::task::spawn_blocking(move || client.lock().expect("client").put(&relative, &body))
            .await
            .expect("join");
    world.put_result = Some(result);
}

#[when(expr = "the client calls get on {string} with a signer the chain does not cover")]
async fn client_get_uncovered(world: &mut RemoteClientWorld, relative: String) {
    // The p1 mandated agent: a VALID chain whose perimeter
    // (read.circle / append.circle / act.x.gmail.reply) does not cover
    // the path — the refusal is `not_covered`, a 403 verdict.
    let f = fixtures();
    let signer = Arc::new(KeySigner::mandated(
        f.agent_sk.clone(),
        vec![f.mandate_id.clone()],
    ));
    world.client = Some(Arc::new(Mutex::new(world.build_client(signer))));
    world.counted_path = Some(relative.clone());
    world.client_get(&relative).await;
}

#[when(expr = "the client calls list on {string}")]
async fn client_list(world: &mut RemoteClientWorld, prefix: String) {
    let client = world.client();
    let result = tokio::task::spawn_blocking(move || client.lock().expect("client").list(&prefix))
        .await
        .expect("join");
    world.list_result = Some(result);
}

#[when(expr = "the client publishes the genesis manifest")]
async fn client_publishes_genesis(world: &mut RemoteClientWorld) {
    let client = world.client();
    let bytes = fixtures().genesis_manifest.clone();
    let ack = tokio::task::spawn_blocking(move || {
        client.lock().expect("client").publish_manifest(&bytes)
    })
    .await
    .expect("join");
    world.publish_acks.push(ack);
}

#[when(expr = "the client publishes the successor manifest from p7")]
async fn client_publishes_successor(world: &mut RemoteClientWorld) {
    let client = world.client();
    let bytes = fixtures().successor_manifest.clone();
    let ack = tokio::task::spawn_blocking(move || {
        client.lock().expect("client").publish_manifest(&bytes)
    })
    .await
    .expect("join");
    world.publish_acks.push(ack);
}

#[when(expr = "the client appends a signed gamma entry from the p1 fixtures")]
async fn client_appends(world: &mut RemoteClientWorld) {
    let client = world.client();
    let entry = fixtures().gamma_entry.clone();
    let result =
        tokio::task::spawn_blocking(move || client.lock().expect("client").append_gamma(&entry))
            .await
            .expect("join");
    world.append_result = Some(result);
}

// -------------------------------------------------------------- thens

#[then(expr = "the call returns the exact stored bytes")]
async fn returns_stored(world: &mut RemoteClientWorld) {
    let expected = world.stored.clone().expect("a stored oracle");
    match world.gets.last() {
        Some(Ok(Some(bytes))) => assert_eq!(bytes, &expected, "byte-exact"),
        other => panic!("expected the stored bytes, got {other:?}"),
    }
}

#[then(expr = "both calls return the exact stored bytes")]
async fn both_return_stored(world: &mut RemoteClientWorld) {
    let expected = world
        .stored
        .clone()
        .or(world.put_body.clone())
        .expect("an oracle");
    assert!(world.gets.len() >= 2, "two calls were made");
    for result in world.gets.iter().rev().take(2) {
        match result {
            Ok(Some(bytes)) => assert_eq!(bytes, &expected, "byte-exact"),
            other => panic!("expected the stored bytes, got {other:?}"),
        }
    }
}

#[then(expr = "both calls succeed")]
async fn both_succeed(world: &mut RemoteClientWorld) {
    assert!(world.gets.len() >= 2);
    for result in world.gets.iter().rev().take(2) {
        assert!(matches!(result, Ok(Some(_))), "both calls served");
    }
}

#[then(expr = "the request carried an X-Aithos-Auth envelope naming key {string}")]
async fn envelope_names_key(world: &mut RemoteClientWorld, key: String) {
    let envelope = world
        .client()
        .lock()
        .expect("client")
        .last_envelope()
        .expect("an envelope was sent");
    assert_eq!(envelope["key"].as_str(), Some(key.as_str()));
    assert_eq!(envelope["v"].as_i64(), Some(1));
    assert!(
        envelope["signature"]["value"]
            .as_str()
            .is_some_and(|s| s.len() == 128),
        "a filled ed25519 signature"
    );
}

#[then(expr = "the deposit is accepted by the real service")]
async fn deposit_accepted(world: &mut RemoteClientWorld) {
    match &world.put_result {
        Some(Ok(())) => {}
        other => panic!("expected an accepted deposit, got {other:?}"),
    }
    // The service actually stored the exact bytes.
    let f = fixtures();
    let relative = world.counted_path.clone().expect("a counted path");
    let stored = world
        .wire()
        .objects
        .get(&f.tenant, &f.did, &relative)
        .await
        .unwrap()
        .expect("the deposit landed");
    assert_eq!(&stored, world.put_body.as_ref().expect("a put body"));
}

#[then(expr = "the envelope body_b3 equals the BLAKE3 of the sent body")]
async fn envelope_body_b3(world: &mut RemoteClientWorld) {
    let envelope = world
        .client()
        .lock()
        .expect("client")
        .last_envelope()
        .expect("an envelope was sent");
    let expected = blake3::hash(world.put_body.as_ref().expect("a put body"))
        .to_hex()
        .to_string();
    assert_eq!(envelope["body_b3"].as_str(), Some(expected.as_str()));
}

#[then(expr = "the two envelopes carry distinct nonces")]
async fn distinct_nonces(world: &mut RemoteClientWorld) {
    let envelopes = world.client().lock().expect("client").sent_envelopes();
    assert!(envelopes.len() >= 2);
    let nonce = |e: &serde_json::Value| e["nonce"].as_str().unwrap().to_owned();
    let last = nonce(&envelopes[envelopes.len() - 1]);
    let previous = nonce(&envelopes[envelopes.len() - 2]);
    assert_ne!(last, previous, "a nonce is never reused");
}

#[then(expr = "the call fails with a not_covered store error")]
async fn fails_not_covered(world: &mut RemoteClientWorld) {
    match world.last_wire_error() {
        RemoteError::Wire { status, code, .. } => {
            assert_eq!((status, code.as_str()), (403, "not_covered"));
        }
        other => panic!("expected a wire verdict, got {other:?}"),
    }
}

#[then(expr = "no bytes are returned")]
async fn no_bytes(world: &mut RemoteClientWorld) {
    assert!(matches!(world.gets.last(), Some(Err(_))), "no silent bytes");
}

#[then(expr = "the listing returns the {int} paths in wire order")]
async fn listing_returns(world: &mut RemoteClientWorld, n: u64) {
    let paths = match &world.list_result {
        Some(Ok(paths)) => paths.clone(),
        other => panic!("expected a listing, got {other:?}"),
    };
    assert_eq!(paths.len() as u64, n);
    assert_eq!(paths, world.stored_paths, "the wire's own order");
}

#[then(expr = "the publish is accepted with height {int}")]
async fn publish_accepted(world: &mut RemoteClientWorld, height: u64) {
    match world.publish_acks.last() {
        Some(Ok(ack)) => assert_eq!(ack.height, Some(height)),
        other => panic!("expected an accepted publish, got {other:?}"),
    }
}

#[then(expr = "the client's tracked manifest head equals the head the service returned")]
async fn tracked_equals_returned(world: &mut RemoteClientWorld) {
    let returned = match world.publish_acks.last() {
        Some(Ok(ack)) => ack.head.clone(),
        other => panic!("expected an accepted publish, got {other:?}"),
    };
    let tracked = world
        .client()
        .lock()
        .expect("client")
        .tracked_manifest_head();
    assert_eq!(tracked.as_deref(), Some(returned.as_str()));
}

#[then(expr = "the If-Head sent equals the head returned by the genesis publish")]
async fn if_head_pins_genesis(world: &mut RemoteClientWorld) {
    let requests = world.client().lock().expect("client").sent_requests();
    let last_publish = requests
        .iter()
        .rev()
        .find(|r| r.method == "PUT" && r.path.ends_with("/manifest.json"))
        .expect("a publish was sent");
    assert_eq!(
        last_publish.if_head.as_deref(),
        Some(fixtures().genesis_head.as_str()),
        "the tracked head rode If-Head"
    );
}

#[then(expr = "the publish fails with a cas_mismatch carrying the current head")]
async fn publish_cas_mismatch(world: &mut RemoteClientWorld) {
    match world.publish_acks.last() {
        Some(Err(RemoteError::Wire {
            status, code, head, ..
        })) => {
            assert_eq!((*status, code.as_str()), (409, "cas_mismatch"));
            assert_eq!(head.as_deref(), Some(fixtures().successor_head.as_str()));
        }
        other => panic!("expected a cas_mismatch, got {other:?}"),
    }
}

#[then(expr = "the client's tracked manifest head now equals the served head")]
async fn tracked_adopts_served(world: &mut RemoteClientWorld) {
    let tracked = world
        .client()
        .lock()
        .expect("client")
        .tracked_manifest_head();
    assert_eq!(
        tracked.as_deref(),
        Some(fixtures().successor_head.as_str()),
        "the 409 head IS the rebase input"
    );
}

#[then(expr = "the append is accepted")]
async fn append_accepted(world: &mut RemoteClientWorld) {
    match &world.append_result {
        Some(Ok(ack)) => assert_eq!(ack.head, fixtures().gamma_entry_head, "vector head"),
        other => panic!("expected an accepted append, got {other:?}"),
    }
}

#[then(expr = "the tracked gamma head equals the head the service returned")]
async fn tracked_gamma_equals(world: &mut RemoteClientWorld) {
    let tracked = world.client().lock().expect("client").tracked_gamma_head();
    assert_eq!(
        tracked.as_deref(),
        Some(fixtures().gamma_entry_head.as_str())
    );
}

#[then(expr = "the append fails with a cas_mismatch carrying the current head")]
async fn append_cas_mismatch(world: &mut RemoteClientWorld) {
    match &world.append_result {
        Some(Err(RemoteError::Wire {
            status, code, head, ..
        })) => {
            assert_eq!((*status, code.as_str()), (409, "cas_mismatch"));
            assert_eq!(head.as_deref(), Some(fixtures().gamma_entry_head.as_str()));
        }
        other => panic!("expected a cas_mismatch, got {other:?}"),
    }
}

#[then(expr = "the client waited a backoff between attempts")]
async fn waited_backoff(world: &mut RemoteClientWorld) {
    let waits = world.client().lock().expect("client").backoff_waits();
    assert!(waits.len() >= 2, "one wait per dropped connection");
    assert!(waits[1] > waits[0], "the backoff grows: {waits:?}");
}

#[then(expr = "the call fails with a transport store error after the bounded retries")]
async fn fails_transport(world: &mut RemoteClientWorld) {
    match world.last_wire_error() {
        RemoteError::Transport(_) => {}
        other => panic!("expected a transport error, got {other:?}"),
    }
    let waits = world.client().lock().expect("client").backoff_waits();
    assert_eq!(
        waits.len(),
        3,
        "exactly max_retries backoffs, then the error"
    );
}

#[then(expr = "the service saw exactly {int} request for that path")]
#[then(expr = "the service saw exactly {int} requests for that path")]
async fn service_saw(world: &mut RemoteClientWorld, n: u64) {
    let relative = world.counted_path.clone().expect("a counted path");
    assert_eq!(
        world.count_of(&relative) as u64,
        n,
        "wire hits for {relative}"
    );
}

#[then(expr = "the second request carried If-None-Match and was answered 304")]
async fn second_was_304(world: &mut RemoteClientWorld) {
    let requests = world.client().lock().expect("client").sent_requests();
    let last = requests.last().expect("requests were sent");
    assert!(
        last.if_none_match.is_some(),
        "the cached strong ETag rode If-None-Match"
    );
    assert_eq!(last.status, 304, "the wire revalidated");
}

// ------------------------------------------------------- P4: batch/sync

#[given(expr = "the store holds the published p7 editions at heights 1 and 2")]
async fn published_p7_editions(world: &mut RemoteClientWorld) {
    // By the WIRE: two owner publishes — the A.5 slots (manifests/1.json)
    // are written by the SERVER on accept, exactly the deployed shape.
    let client = world.client();
    let (genesis, successor) = {
        let f = fixtures();
        (f.genesis_manifest.clone(), f.successor_manifest.clone())
    };
    tokio::task::spawn_blocking(move || {
        let client = client.lock().expect("client");
        client
            .publish_manifest(&genesis)
            .expect("genesis publish accepted");
        client
            .publish_manifest(&successor)
            .expect("successor publish accepted");
    })
    .await
    .expect("join");
}

#[given(expr = "the edition slot 1 is purged server-side")]
async fn purge_edition_slot(world: &mut RemoteClientWorld) {
    // The §8 GC in miniature: the held slot is gone, the tip stays. The
    // memory backend has no delete (the wire never deletes) — the purge
    // is simulated the way the store harness does it: rebuild the state
    // WITHOUT the slot, on a fresh wire, then re-point the client.
    let f = fixtures();
    let wire = boot_wire().await;
    wire.objects
        .put(
            &f.tenant,
            &f.did,
            "manifest.json",
            f.successor_manifest.clone(),
        )
        .await
        .expect("seed tip");
    world.wire = Some(wire);
    let signer = world.signer.clone().expect("a signer is configured");
    world.client = Some(Arc::new(Mutex::new(world.build_client(signer))));
}

#[when(expr = "the client calls get_many on {string}, {string} and {string}")]
async fn client_get_many_three(world: &mut RemoteClientWorld, a: String, b: String, c: String) {
    let client = world.client();
    let paths = vec![a, b, c];
    let pack = tokio::task::spawn_blocking(move || client.lock().expect("client").get_many(&paths))
        .await
        .expect("join");
    world.pack = Some(pack);
}

#[when(expr = "the mandated client calls get_many on {string} and {string}")]
async fn mandated_get_many_two(world: &mut RemoteClientWorld, a: String, b: String) {
    let f = fixtures();
    let signer = Arc::new(KeySigner::mandated(
        f.agent_sk.clone(),
        vec![f.mandate_id.clone()],
    ));
    world.client = Some(Arc::new(Mutex::new(world.build_client(signer))));
    let client = world.client();
    let paths = vec![a, b];
    let pack = tokio::task::spawn_blocking(move || client.lock().expect("client").get_many(&paths))
        .await
        .expect("join");
    world.pack = Some(pack);
}

#[when(expr = "the client calls sync with have_edition {int}")]
async fn client_sync(world: &mut RemoteClientWorld, have: u64) {
    let client = world.client();
    let pack = tokio::task::spawn_blocking(move || client.lock().expect("client").sync(have))
        .await
        .expect("join");
    world.pack = Some(pack);
}

impl RemoteClientWorld {
    fn pack_parts(&self) -> &Vec<aithos_bundle::remote::PackPart> {
        match self.pack.as_ref() {
            Some(Ok(parts)) => parts,
            other => panic!("expected a parsed pack, got {other:?}"),
        }
    }
}

#[then(expr = "the pack carries {int} parts in request order")]
async fn pack_carries(world: &mut RemoteClientWorld, n: usize) {
    assert_eq!(world.pack_parts().len(), n, "pack size");
}

#[then(expr = "pack part {int} is {int} with body {string}")]
async fn pack_part_with_body(world: &mut RemoteClientWorld, i: usize, status: u16, body: String) {
    let part = &world.pack_parts()[i - 1];
    assert_eq!(part.status, status, "part {i} status");
    assert_eq!(
        part.bytes.as_deref(),
        Some(body.as_bytes()),
        "part {i} bytes"
    );
}

#[then(expr = "pack part {int} is {int} without a body")]
async fn pack_part_no_body(world: &mut RemoteClientWorld, i: usize, status: u16) {
    let part = &world.pack_parts()[i - 1];
    assert_eq!(part.status, status, "part {i} status");
    assert!(part.bytes.is_none(), "a non-200 part never carries bytes");
}

#[then(expr = "the service saw exactly {int} POST request for {string}")]
async fn service_saw_post(world: &mut RemoteClientWorld, n: u64, route: String) {
    let key = format!("POST {}", world.abs(&route));
    let seen = *world
        .wire()
        .counters
        .lock()
        .expect("counters")
        .get(&key)
        .unwrap_or(&0);
    assert_eq!(u64::from(seen), n, "wire hits for POST {route}");
}

#[then(expr = "the pack's first part is {string} with the successor bytes")]
async fn pack_first_manifest(world: &mut RemoteClientWorld, path: String) {
    let f = fixtures();
    let part = world.pack_parts().first().expect("a first part");
    assert_eq!(part.path, path, "manifest first (frozen p9 rule)");
    assert_eq!(part.status, 200);
    assert_eq!(
        part.bytes.as_deref(),
        Some(f.successor_manifest.as_slice()),
        "the tip manifest bytes"
    );
}

#[then(expr = "the pack lists the changed paths of the p7 edition diff in lexicographic order")]
async fn pack_lists_diff(world: &mut RemoteClientWorld) {
    let f = fixtures();
    let held: serde_json::Value = serde_json::from_slice(&f.genesis_manifest).unwrap();
    let tip: serde_json::Value = serde_json::from_slice(&f.successor_manifest).unwrap();
    let held_files = held["files"].as_object().unwrap();
    let mut want: Vec<String> = tip["files"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, v)| held_files.get(*k) != Some(v))
        .map(|(k, _)| k.clone())
        .collect();
    want.sort();
    let got: Vec<String> = world.pack_parts()[1..]
        .iter()
        .map(|p| p.path.clone())
        .collect();
    assert_eq!(got, want, "the lexicographic diff, manifest excluded");
}

#[then(expr = "the {string} pack part is {int}")]
async fn pack_named_part(world: &mut RemoteClientWorld, path: String, status: u16) {
    let part = world
        .pack_parts()
        .iter()
        .find(|p| p.path == path)
        .unwrap_or_else(|| panic!("part {path} in the pack"));
    assert_eq!(part.status, status, "part {path} status");
}

#[then(expr = "the absent diff parts answer 404 without a body")]
async fn pack_absent_parts(world: &mut RemoteClientWorld) {
    let absent: Vec<_> = world.pack_parts()[1..]
        .iter()
        .filter(|p| p.status != 200)
        .collect();
    assert!(!absent.is_empty(), "the p7 diff has undeposited sidecars");
    for part in absent {
        assert_eq!(part.status, 404, "absence is typed per part: {}", part.path);
        assert!(part.bytes.is_none());
    }
}

#[then(expr = "the sync call fails with a {int} edition_gone store error")]
async fn sync_fails_gone(world: &mut RemoteClientWorld, status: u16) {
    match world.pack.as_ref() {
        Some(Err(RemoteError::Wire {
            status: got, code, ..
        })) => {
            assert_eq!((*got, code.as_str()), (status, "edition_gone"));
        }
        other => panic!("expected the typed 410, got {other:?}"),
    }
}

// ----------------------------------------------------------------- main

#[tokio::main]
async fn main() {
    RemoteClientWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit("tests/features/remote")
        .await;
}
