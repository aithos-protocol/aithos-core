//! BDD acceptance harness for `tests/features/store-hello.feature` —
//! the A.2 verification order, case by case, against the REAL axum
//! surface (`build_router`), driven in-process.
//!
//! Fixtures are the committed conformance vectors, never re-invented:
//! the owner keys re-derive from the a1 seed, the DID/tenant/mandate come
//! from p1, and the clock is the injected test instant (`server_now` is
//! an input, A.2's replayability property). The log-discipline scenario
//! captures the service's real `tracing` output and asserts the A.8
//! register: no path, no body, no envelope material — ever.

use std::sync::{Arc, Mutex, OnceLock};

use aithos_provider::acme::AcmeState;
use aithos_provider::control::ControlPlane;
use aithos_provider::dns::MemDnsTxt;
use aithos_provider::envelope::{header_value, sign_envelope, Envelope, EnvelopeSignature};
use aithos_provider::heads::MemHeads;
use aithos_provider::nonces::MemNonces;
use aithos_provider::objects::{MemObjects, ObjectStore};
use aithos_provider::service::{build_router, AppState};
use aithos_provider::time::{parse_rfc3339z_ms, render_rfc3339z};
use axum::body::Body;
use axum::http::{header, Request};
use cucumber::{given, then, when, World as _};
use ed25519_dalek::SigningKey;
use tower::ServiceExt as _;

// ------------------------------------------------------------ log capture

static LOG_BUFFER: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();

#[derive(Clone)]
struct BufMake(Arc<Mutex<Vec<u8>>>);

struct BufWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for BufWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufMake {
    type Writer = BufWriter;
    fn make_writer(&'a self) -> BufWriter {
        BufWriter(self.0.clone())
    }
}

// ------------------------------------------------------------- fixtures

struct Fixtures {
    tenant: String,
    did: String,
    did_json: String,
    mandate_id: String,
    mandate_jcs: String,
    /// The p1 gamma fixtures, seeded at the did.json's own `revocations`
    /// key: the granted state and the post-revoke state (forward-only cut).
    gamma_post_grant: Vec<String>,
    gamma_post_revoke: Vec<String>,
    revocations_key: String,
    root_sk: SigningKey,
    agent_sk: SigningKey,
    /// B.5 fixtures — the committed p6 vector's gateway identities.
    gateway_sk: SigningKey,
    gateway_pub: String,
    stranger_sk: SigningKey,
    demo_hostname: String,
    rate_hostname: String,
    rate_gateway_pub: String,
}

fn sk_from_hex(hex_seed: &str) -> SigningKey {
    SigningKey::from_bytes(&hex::decode(hex_seed).unwrap().try_into().unwrap())
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let vectors = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");
        let p1: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{vectors}/p1-store-envelope.json")).unwrap(),
        )
        .unwrap();
        let a1: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{vectors}/a1-genesis.json")).unwrap(),
        )
        .unwrap();
        let p6: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{vectors}/p6-acme-txt.json")).unwrap(),
        )
        .unwrap();
        let seed: [u8; 32] = hex::decode(a1["seed_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let root_sk = SigningKey::from_bytes(&aithos_core::derive::derive_key(
            aithos_core::derive::CTX_ROOT_SIGN,
            &seed,
        ));
        let agent_sk = sk_from_hex(p1["agent_sk_hex"].as_str().unwrap());
        let mandate: serde_json::Value =
            serde_json::from_str(p1["mandate_jcs"].as_str().unwrap()).unwrap();
        let mappings = p6["control_plane_mappings"].as_array().unwrap();
        let lines = |state: &str| -> Vec<String> {
            p1["gamma_states"][state]
                .as_array()
                .unwrap()
                .iter()
                .map(|line| line.as_str().unwrap().to_owned())
                .collect()
        };
        let did_doc: serde_json::Value =
            serde_json::from_str(p1["did_json_jcs"].as_str().unwrap()).unwrap();
        Fixtures {
            tenant: p1["tenant"].as_str().unwrap().to_owned(),
            did: p1["did"].as_str().unwrap().to_owned(),
            did_json: p1["did_json_jcs"].as_str().unwrap().to_owned(),
            mandate_id: mandate["id"].as_str().unwrap().to_owned(),
            mandate_jcs: p1["mandate_jcs"].as_str().unwrap().to_owned(),
            gamma_post_grant: lines("post_grant"),
            gamma_post_revoke: lines("post_revoke"),
            revocations_key: did_doc["revocations"].as_str().unwrap().to_owned(),
            root_sk,
            agent_sk,
            gateway_sk: sk_from_hex(p6["gateway_sk_hex"].as_str().unwrap()),
            gateway_pub: mappings[0]["gateway_pub"].as_str().unwrap().to_owned(),
            stranger_sk: sk_from_hex(p6["stranger_gateway_sk_hex"].as_str().unwrap()),
            demo_hostname: mappings[0]["hostname"].as_str().unwrap().to_owned(),
            rate_hostname: mappings[1]["hostname"].as_str().unwrap().to_owned(),
            rate_gateway_pub: mappings[1]["gateway_pub"].as_str().unwrap().to_owned(),
        }
    })
}

// --------------------------------------------------------------- world

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct StoreWorld {
    state: Arc<AppState>,
    /// Concrete handle on the memory DNS backend so the B.5 effects are
    /// asserted, never assumed (same Arc `state.dns` erases).
    dns: Arc<MemDnsTxt>,
    /// Concrete handle on the memory heads table (A.5) — the CAS givens
    /// seed it and the Thens assert it (same Arc `state.heads` erases).
    heads: Arc<MemHeads>,
    authority: String,
    now: String,
    nonce_counter: u64,
    last: Option<Answer>,
    previous: Option<Answer>,
    last_mandated: Option<Pending>,
    last_acme: Option<Pending>,
    /// The artifact the scenario loaded for the next deposit (a frozen
    /// p7 body), and the seeded heads the "stored … head" steps name.
    loaded_body: Option<Vec<u8>>,
    loaded_expect: Option<serde_json::Value>,
    stored_manifest_head: Option<String>,
    stored_gamma_head: Option<String>,
    /// The previous listing page (étape 5 — the pagination Then compares
    /// continuation against it).
    prev_list_paths: Option<Vec<String>>,
    log_mark: usize,
}

struct Answer {
    status: u16,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

/// A request about to fire: the ACTUAL wire facts (which scenarios
/// deliberately desynchronize from what the envelope signed).
struct Pending {
    method: String,
    path: String,
    body: Vec<u8>,
    header: Option<String>,
    version_header: Option<String>,
    /// The A.5 CAS header, verbatim (`None` = absent — `428` where CAS
    /// is mandatory).
    if_head: Option<String>,
}

impl std::fmt::Debug for StoreWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoreWorld")
    }
}

/// Which control-plane suspension the scenario asked for (B.5 givens).
#[derive(Clone, Copy, PartialEq)]
enum Suspension {
    None,
    Binding,
    Tenant,
}

impl StoreWorld {
    fn new() -> Self {
        let f = fixtures();
        let authority = "store.aithos.fr".to_owned();
        let (state, dns, heads) = Self::state(&authority, true, Suspension::None);
        Self {
            state,
            dns,
            heads,
            authority,
            now: "2026-07-16T12:00:00Z".into(),
            nonce_counter: 0,
            last: None,
            previous: None,
            last_mandated: None,
            last_acme: None,
            loaded_body: None,
            loaded_expect: None,
            stored_manifest_head: None,
            stored_gamma_head: None,
            prev_list_paths: None,
            log_mark: 0,
        }
        .tap(|_| {
            let _ = f;
        })
    }

    fn tap(self, f: impl FnOnce(&Self)) -> Self {
        f(&self);
        self
    }

    #[allow(clippy::type_complexity)]
    fn state(
        authority: &str,
        with_did_json: bool,
        suspension: Suspension,
    ) -> (Arc<AppState>, Arc<MemDnsTxt>, Arc<MemHeads>) {
        let f = fixtures();
        let bootstrap = serde_json::json!({
            "tenants": [{
                "tenant": f.tenant,
                "suspended": suspension == Suspension::Tenant,
                "dids": [{"did": f.did, "did_json": f.did_json}],
            }],
            // The B.5 authority: the committed p6 mappings (demo + rate).
            "tunnels": [
                {"gateway_pub": f.gateway_pub, "tenant": f.tenant,
                 "hostname": f.demo_hostname,
                 "suspended": suspension == Suspension::Binding},
                {"gateway_pub": f.rate_gateway_pub, "tenant": f.tenant,
                 "hostname": f.rate_hostname},
            ],
        });
        let (control, preloads, head_seeds) =
            ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("fixture bootstrap");
        let objects = Arc::new(MemObjects::new());
        if with_did_json {
            for (tenant, did, key, bytes) in preloads {
                futures::executor::block_on(objects.put(&tenant, &did, &key, bytes));
            }
        }
        let heads = Arc::new(MemHeads::new());
        for (tenant, did, record) in head_seeds {
            heads.seed(&tenant, &did, record);
        }
        let dns = Arc::new(MemDnsTxt::new());
        let state = Arc::new(AppState {
            control,
            objects,
            heads: heads.clone(),
            deposit_locks: Default::default(),
            nonces: Arc::new(MemNonces::new(600)),
            dns: dns.clone(),
            acme: AcmeState::new(),
            authority: authority.to_owned(),
            test_now_enabled: true,
        });
        (state, dns, heads)
    }

    fn fresh_nonce(&mut self) -> String {
        self.nonce_counter += 1;
        format!("bdd-nonce-{:012}", self.nonce_counter)
    }

    fn abs(&self, relative: &str) -> String {
        format!("/t/{}/{}/{relative}", fixtures().tenant, fixtures().did)
    }

    /// Build a signed envelope. The `signed_*` facts are what the envelope
    /// claims; the request itself may send different facts.
    #[allow(clippy::too_many_arguments)]
    fn envelope(
        &mut self,
        key: &str,
        signer: &SigningKey,
        method: &str,
        path: &str,
        body: &[u8],
        at: Option<&str>,
        nonce: Option<&str>,
        mandate: Vec<String>,
        host: Option<&str>,
    ) -> Envelope {
        let envelope = Envelope {
            v: 1,
            host: host.unwrap_or(&self.authority).to_owned(),
            method: method.to_owned(),
            path: path.to_owned(),
            body_b3: if body.is_empty() {
                String::new()
            } else {
                blake3::hash(body).to_hex().to_string()
            },
            at: at.unwrap_or(&self.now).to_owned(),
            nonce: nonce
                .map(str::to_owned)
                .unwrap_or_else(|| self.fresh_nonce()),
            mandate,
            key: key.to_owned(),
            signature: EnvelopeSignature {
                alg: "ed25519".into(),
                value: String::new(),
            },
        };
        sign_envelope(envelope, signer).expect("sign")
    }

    fn owner_pending(&mut self, method: &str, relative: &str, body: &[u8]) -> Pending {
        let path = self.abs(relative);
        let envelope = self.envelope(
            "#root",
            &fixtures().root_sk.clone(),
            method,
            &path,
            body,
            None,
            None,
            vec![],
            None,
        );
        Pending {
            method: method.to_owned(),
            path,
            body: body.to_vec(),
            header: Some(header_value(&envelope).unwrap()),
            version_header: None,
            if_head: None,
        }
    }

    fn mandated_pending(&mut self, relative: &str, nonce: Option<&str>) -> Pending {
        let f = fixtures();
        let path = self.abs(relative);
        let key =
            aithos_core::wire::ed25519_pub_to_multibase(&f.agent_sk.verifying_key().to_bytes());
        let envelope = self.envelope(
            &key,
            &f.agent_sk.clone(),
            "GET",
            &path,
            b"",
            None,
            nonce,
            vec![f.mandate_id.clone()],
            None,
        );
        Pending {
            method: "GET".into(),
            path,
            body: vec![],
            header: Some(header_value(&envelope).unwrap()),
            version_header: None,
            if_head: None,
        }
    }

    async fn fire(&mut self, pending: &Pending) {
        self.log_mark = LOG_BUFFER
            .get()
            .map(|b| b.lock().expect("log buffer").len())
            .unwrap_or(0);
        let mut request = Request::builder()
            .method(pending.method.as_str())
            .uri(&pending.path)
            .header(header::HOST, &self.authority)
            .header("x-aithos-test-now", &self.now);
        if let Some(h) = &pending.header {
            request = request.header("x-aithos-auth", h);
        }
        if let Some(head) = &pending.if_head {
            request = request.header("if-head", head);
        }
        if let Some(v) = &pending.version_header {
            request = request.header("x-aithos-store", v);
        }
        let request = request
            .body(Body::from(pending.body.clone()))
            .expect("request");
        let response = build_router(self.state.clone())
            .oneshot(request)
            .await
            .expect("infallible");
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body")
            .to_vec();
        self.previous = self.last.take();
        self.last = Some(Answer {
            status,
            headers,
            body,
        });
    }

    fn last(&self) -> &Answer {
        self.last.as_ref().expect("a request was fired")
    }

    fn assert_error(&self, answer: &Answer, status: u16, code: &str) {
        assert_eq!(
            answer.status,
            status,
            "status (body: {})",
            String::from_utf8_lossy(&answer.body)
        );
        let body: serde_json::Value =
            serde_json::from_slice(&answer.body).expect("A.7 error body is JSON");
        assert_eq!(body["error"].as_str(), Some(code), "registry code");
        assert!(
            body["at"].as_str().is_some_and(|s| s.ends_with('Z')),
            "error body carries `at` (now serveur)"
        );
    }
}

// ---------------------------------------------------------- background

#[given(expr = "the tenant {string} is enrolled and bound to the vector DID")]
async fn tenant_enrolled(_world: &mut StoreWorld, tenant: String) {
    assert_eq!(tenant, fixtures().tenant, "fixture is the committed vector");
}

#[given(expr = "the vector did.json is stored for that DID")]
async fn did_json_stored(world: &mut StoreWorld) {
    let f = fixtures();
    let stored = world
        .state
        .objects
        .get(&f.tenant, &f.did, "did.json")
        .await
        .expect("preloaded");
    assert_eq!(stored, f.did_json.as_bytes());
}

#[given(expr = "the service authority is {string}")]
async fn service_authority(world: &mut StoreWorld, authority: String) {
    assert_eq!(world.authority, authority, "fixture authority");
}

// ------------------------------------------------------------- givens

#[given(expr = "the server clock reads {string}")]
async fn clock_reads(world: &mut StoreWorld, now: String) {
    world.now = now;
}

#[given(expr = "a mandated GET with nonce {string} was refused after the nonce check")]
async fn mandated_refused(world: &mut StoreWorld, nonce: String) {
    let pending = world.mandated_pending(
        "e/circle/blobs/01000000000000000000000000.enc",
        Some(&nonce),
    );
    world.fire(&pending).await;
    let answer = world.last();
    // Refused at #9 (P1 fail-closed chain), which is PAST the #6
    // reservation: the nonce is burned.
    world.assert_error(answer, 403, "chain_invalid");
    world.last_mandated = Some(pending);
}

#[given(expr = "the did.json of the bound DID is absent from the store")]
async fn did_json_absent(world: &mut StoreWorld) {
    let (state, dns, heads) = StoreWorld::state(&world.authority, false, Suspension::None);
    world.state = state;
    world.dns = dns;
    world.heads = heads;
}

#[given(expr = "an owner-signed PUT stored {string} at relative path {string}")]
async fn owner_put_stored(world: &mut StoreWorld, body: String, relative: String) {
    let pending = world.owner_pending("PUT", &relative, body.as_bytes());
    world.fire(&pending).await;
    assert!(world.last().status < 300, "the PUT must be accepted");
}

// -------------------------------------------------------------- whens

#[when(expr = "an unsigned GET arrives for path {string}")]
async fn unsigned_get_abs(world: &mut StoreWorld, path: String) {
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: None,
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an unsigned GET arrives for relative path {string}")]
async fn unsigned_get_rel(world: &mut StoreWorld, relative: String) {
    let path = world.abs(&relative);
    unsigned_get_abs(world, path).await;
}

#[when(
    expr = "an unsigned GET arrives for relative path {string} with header {string} equal to {string}"
)]
async fn unsigned_get_with_header(
    world: &mut StoreWorld,
    relative: String,
    name: String,
    value: String,
) {
    assert_eq!(name, "X-Aithos-Store", "the only negotiated header");
    let pending = Pending {
        method: "GET".into(),
        path: world.abs(&relative),
        body: vec![],
        header: None,
        version_header: Some(value),
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed PUT arrives for relative path {string} with body {string}")]
async fn owner_put(world: &mut StoreWorld, relative: String, body: String) {
    let pending = world.owner_pending("PUT", &relative, body.as_bytes());
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives for relative path {string}")]
async fn owner_get(world: &mut StoreWorld, relative: String) {
    let pending = world.owner_pending("GET", &relative, b"");
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives for tenant {string} and relative path {string}")]
async fn owner_get_other_tenant(world: &mut StoreWorld, tenant: String, relative: String) {
    let f = fixtures();
    let path = format!("/t/{tenant}/{}/{relative}", f.did);
    let envelope = world.envelope(
        "#root",
        &f.root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET for an unbound DID arrives with a valid envelope")]
async fn owner_get_unbound_did(world: &mut StoreWorld) {
    let f = fixtures();
    // A perfectly valid DID that the control plane never enrolled.
    let stranger = SigningKey::from_bytes(&[0x51; 32]);
    let did = aithos_core::wire::did_aithos(&stranger.verifying_key().to_bytes());
    let path = format!("/t/{}/{did}/manifest.json", f.tenant);
    let envelope = world.envelope(
        "#root",
        &stranger,
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a GET arrives with header value {string}")]
async fn get_with_raw_header(world: &mut StoreWorld, value: String) {
    let pending = Pending {
        method: "GET".into(),
        path: world.abs("manifest.json"),
        body: vec![],
        header: Some(value),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a GET arrives with a {int}-byte header value")]
async fn get_with_huge_header(world: &mut StoreWorld, size: usize) {
    get_with_raw_header(world, "A".repeat(size)).await;
}

#[when(expr = "an owner-signed GET arrives whose envelope carries an extra field {string}")]
async fn owner_get_extra_field(world: &mut StoreWorld, field: String) {
    let path = world.abs("manifest.json");
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let mut value: serde_json::Value =
        serde_json::from_str(&serde_jcs::to_string(&envelope).unwrap()).unwrap();
    value[field] = serde_json::json!("x");
    let header = base64_url(serde_jcs::to_string(&value).unwrap().as_bytes());
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives re-encoded with spaces between JSON tokens")]
async fn owner_get_non_canonical(world: &mut StoreWorld) {
    let path = world.abs("manifest.json");
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    // Same JSON document, non-JCS bytes: pretty-printed.
    let value: serde_json::Value =
        serde_json::from_str(&serde_jcs::to_string(&envelope).unwrap()).unwrap();
    let header = base64_url(serde_json::to_string_pretty(&value).unwrap().as_bytes());
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives whose envelope carries v {int}")]
async fn owner_get_wrong_v(world: &mut StoreWorld, v: u8) {
    let path = world.abs("manifest.json");
    let mut envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    envelope.v = v;
    let envelope = sign_envelope(envelope, &fixtures().root_sk.clone()).unwrap();
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives whose envelope names host {string}")]
async fn owner_get_wrong_host(world: &mut StoreWorld, host: String) {
    let path = world.abs("manifest.json");
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        Some(&host),
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a GET request carries an envelope signed for method {string}")]
async fn get_with_envelope_for_method(world: &mut StoreWorld, method: String) {
    let path = world.abs("manifest.json");
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        &method,
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(
    expr = "a GET for relative path {string} carries an envelope signed for relative path {string}"
)]
async fn get_with_envelope_for_path(world: &mut StoreWorld, actual: String, signed: String) {
    let actual_path = world.abs(&actual);
    let signed_path = world.abs(&signed);
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &signed_path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path: actual_path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(
    expr = "an owner-signed PUT for relative path {string} signs body {string} but sends body {string}"
)]
async fn owner_put_tampered_body(
    world: &mut StoreWorld,
    relative: String,
    signed: String,
    sent: String,
) {
    let path = world.abs(&relative);
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "PUT",
        &path,
        signed.as_bytes(),
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "PUT".into(),
        path,
        body: sent.into_bytes(),
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives carrying an unexpected body {string}")]
async fn owner_get_with_body(world: &mut StoreWorld, body: String) {
    let path = world.abs("manifest.json");
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: body.into_bytes(),
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET for relative path {string} is signed at {string}")]
async fn owner_get_signed_at(world: &mut StoreWorld, relative: String, at: String) {
    let path = world.abs(&relative);
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        Some(&at),
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET with nonce {string} is presented twice")]
async fn owner_get_twice(world: &mut StoreWorld, nonce: String) {
    let path = world.abs("did.json");
    let envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        Some(&nonce),
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
    assert!(world.last().status < 300, "first presentation is accepted");
    world.fire(&pending).await;
}

#[when(expr = "the same mandated GET is presented again")]
async fn same_mandated_again(world: &mut StoreWorld) {
    let pending = world.last_mandated.take().expect("a refused mandated GET");
    world.fire(&pending).await;
    world.last_mandated = Some(pending);
}

#[when(expr = "a GET arrives signed by a raw key with an empty mandate list")]
async fn raw_key_empty_mandate(world: &mut StoreWorld) {
    let f = fixtures();
    let path = world.abs("e/circle/blobs/01000000000000000000000000.enc");
    let key = aithos_core::wire::ed25519_pub_to_multibase(&f.agent_sk.verifying_key().to_bytes());
    let envelope = world.envelope(
        &key,
        &f.agent_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed GET arrives with its signature corrupted")]
async fn owner_get_corrupted(world: &mut StoreWorld) {
    let path = world.abs("did.json");
    let mut envelope = world.envelope(
        "#root",
        &fixtures().root_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![],
        None,
    );
    corrupt(&mut envelope.signature.value);
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a mandated GET arrives with its signature corrupted")]
async fn mandated_get_corrupted(world: &mut StoreWorld) {
    let f = fixtures();
    let path = world.abs("e/circle/blobs/01000000000000000000000000.enc");
    let key = aithos_core::wire::ed25519_pub_to_multibase(&f.agent_sk.verifying_key().to_bytes());
    let mut envelope = world.envelope(
        &key,
        &f.agent_sk.clone(),
        "GET",
        &path,
        b"",
        None,
        None,
        vec![f.mandate_id.clone()],
        None,
    );
    corrupt(&mut envelope.signature.value);
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a correctly signed mandated GET arrives for relative path {string}")]
async fn mandated_get(world: &mut StoreWorld, relative: String) {
    let pending = world.mandated_pending(&relative, None);
    world.fire(&pending).await;
}

// -------------------------------------------------------------- thens

#[then(expr = "the response is {int} {string}")]
async fn response_is(world: &mut StoreWorld, status: u16, code: String) {
    let answer = world.last.take().expect("a request was fired");
    world.assert_error(&answer, status, &code);
    world.last = Some(answer);
}

#[then(expr = "the second response is {int} {string}")]
async fn second_response_is(world: &mut StoreWorld, status: u16, code: String) {
    // `last` IS the second response; `previous` was the first.
    assert!(
        world.previous.as_ref().expect("two requests").status < 300,
        "the first presentation was accepted"
    );
    let answer = world.last.take().expect("two requests");
    world.assert_error(&answer, status, &code);
    world.last = Some(answer);
}

#[then(expr = "the request is accepted")]
async fn request_accepted(world: &mut StoreWorld) {
    let answer = world.last();
    assert!(
        answer.status < 300,
        "expected acceptance, got {} ({})",
        answer.status,
        String::from_utf8_lossy(&answer.body)
    );
}

#[then(expr = "the request is accepted with body {string}")]
async fn accepted_with_body(world: &mut StoreWorld, body: String) {
    let answer = world.last();
    assert!(
        answer.status < 300,
        "expected acceptance, got {}",
        answer.status
    );
    assert_eq!(answer.body, body.as_bytes());
}

#[then(expr = "the stored object at {string} equals {string}")]
async fn stored_object_equals(world: &mut StoreWorld, relative: String, content: String) {
    let f = fixtures();
    let stored = world
        .state
        .objects
        .get(&f.tenant, &f.did, &relative)
        .await
        .expect("stored");
    assert_eq!(stored, content.as_bytes());
}

#[then(expr = "the response carries header {string} equal to {string}")]
async fn response_header_equals(world: &mut StoreWorld, name: String, value: String) {
    let answer = world.last();
    let got = answer
        .headers
        .get(name.to_ascii_lowercase())
        .and_then(|v| v.to_str().ok());
    assert_eq!(got, Some(value.as_str()));
}

#[then(
    expr = "the request log for class {string} contains no {string} and no {string} and no envelope material"
)]
async fn log_is_redacted(world: &mut StoreWorld, class: String, a: String, b: String) {
    let buffer = LOG_BUFFER.get().expect("log capture installed");
    let slice = {
        let locked = buffer.lock().expect("log buffer");
        String::from_utf8_lossy(&locked[world.log_mark.min(locked.len())..]).into_owned()
    };
    assert!(
        slice.contains(&format!("class={class}")),
        "one request line with the closed class, got: {slice}"
    );
    for forbidden in [a.as_str(), b.as_str(), "eyJ", "x-aithos-auth"] {
        assert!(
            !slice.contains(forbidden),
            "the log leaked `{forbidden}`: {slice}"
        );
    }
}

// ==================================================================
// Mandated authorization — annexe A.2 #7–#10 (store-publication.feature,
// the p1-deferred block turned green by gate 3)
// ==================================================================

impl StoreWorld {
    /// Seed one stored object directly (fixture state, not the wire).
    async fn seed_object(&self, relative: &str, bytes: Vec<u8>) {
        let f = fixtures();
        self.state
            .objects
            .put(&f.tenant, &f.did, relative, bytes)
            .await;
    }

    /// The mandate cert + the given gamma lines, at the did.json's own
    /// `revocations` pointer — the exact state the #9 checks read.
    async fn seed_chain_state(&self, gamma_lines: &[String]) {
        let f = fixtures();
        self.seed_object(
            &format!("certs/{}.json", f.mandate_id),
            f.mandate_jcs.clone().into_bytes(),
        )
        .await;
        self.seed_object(
            &f.revocations_key.clone(),
            (gamma_lines.join("\n") + "\n").into_bytes(),
        )
        .await;
    }
}

#[given(expr = "the gamma log carries the mandate grant and its bound action")]
async fn gamma_carries_grant(world: &mut StoreWorld) {
    let lines = fixtures().gamma_post_grant.clone();
    world.seed_chain_state(&lines).await;
}

#[given(
    expr = "the gamma log carries the mandate grant, its bound action and an owner revoke at {string}"
)]
async fn gamma_carries_revoke(world: &mut StoreWorld, at: String) {
    let f = fixtures();
    let revoke: serde_json::Value =
        serde_json::from_str(f.gamma_post_revoke.last().expect("revoke line")).unwrap();
    assert_eq!(
        revoke["at"].as_str(),
        Some(at.as_str()),
        "the committed p1 revoke instant"
    );
    let lines = f.gamma_post_revoke.clone();
    world.seed_chain_state(&lines).await;
}

#[given(expr = "the covered circle blob of the p1 vector is stored")]
async fn covered_blob_stored(world: &mut StoreWorld) {
    // Opaque ciphertext by doctrine: the store never inspects it, the
    // scenario only asserts the covered read serves.
    world
        .seed_object(
            "e/circle/blobs/01000000000000000000000000.enc",
            b"opaque-p1-circle-blob".to_vec(),
        )
        .await;
}

#[when(
    expr = "a mandated GET signed by a key that is not the chain leaf arrives for relative path {string}"
)]
async fn mandated_get_wrong_leaf(world: &mut StoreWorld, relative: String) {
    let f = fixtures();
    let path = world.abs(&relative);
    // The committed p6 gateway key: a perfectly valid signer that is NOT
    // the chain leaf grantee.pubkey (A.2 #7).
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let envelope = world.envelope(
        &key,
        &signer,
        "GET",
        &path,
        b"",
        None,
        None,
        vec![f.mandate_id.clone()],
        None,
    );
    let pending = Pending {
        method: "GET".into(),
        path,
        body: vec![],
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

// ==================================================================
// P2 publication — annexe A.4/A.5 (store-publication.feature, étape 4)
// Fixtures are the FROZEN p7 cases (vectors/p7-store-publication.json):
// every loaded body is a committed byte string, every seeded state the
// case's own A.5 tuple. Nothing is re-derived here.
// ==================================================================

fn p7() -> &'static serde_json::Value {
    static P7: OnceLock<serde_json::Value> = OnceLock::new();
    P7.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p7-store-publication.json"
            ))
            .unwrap(),
        )
        .unwrap()
    })
}

fn p7_case(kind: &str, name: &str) -> &'static serde_json::Value {
    p7()[kind]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("p7 case {name} missing from {kind}"))
}

impl StoreWorld {
    fn load_case(&mut self, kind: &str, name: &str, body_field: &str) {
        let case = p7_case(kind, name);
        self.loaded_body = Some(case[body_field].as_str().unwrap().as_bytes().to_vec());
        self.loaded_expect = Some(case["expect"].clone());
    }

    fn seed_manifest_heads(&mut self, height: u64, head: &str) {
        let f = fixtures();
        let bare = head.strip_prefix("sha256:").unwrap_or(head).to_owned();
        self.heads.seed(
            &f.tenant,
            &f.did,
            aithos_provider::heads::HeadsRecord {
                height,
                manifest_chain_hash: bare,
                ..Default::default()
            },
        );
        self.stored_manifest_head = Some(head.to_owned());
    }

    fn seed_gamma_head(&mut self, head: &str) {
        let f = fixtures();
        self.heads.seed(
            &f.tenant,
            &f.did,
            aithos_provider::heads::HeadsRecord {
                gamma_head: head.to_owned(),
                ..Default::default()
            },
        );
        self.stored_gamma_head = Some(head.to_owned());
    }

    /// The owner publish/append/deposit over the loaded frozen body.
    fn deposit_pending(
        &mut self,
        method: &str,
        relative: &str,
        if_head: Option<String>,
        mandated: bool,
    ) -> Pending {
        let f = fixtures();
        let path = self.abs(relative);
        let body = self.loaded_body.clone().expect("a body was loaded");
        let envelope = if mandated {
            let key = {
                let mandate: serde_json::Value = serde_json::from_str(&f.mandate_jcs).unwrap();
                mandate["grantee"]["pubkey"].as_str().unwrap().to_owned()
            };
            let signer = f.agent_sk.clone();
            self.envelope(
                &key,
                &signer,
                method,
                &path,
                &body,
                None,
                None,
                vec![f.mandate_id.clone()],
                None,
            )
        } else {
            let signer = f.root_sk.clone();
            self.envelope(
                "#root",
                &signer,
                method,
                &path,
                &body,
                None,
                None,
                vec![],
                None,
            )
        };
        Pending {
            method: method.to_owned(),
            path,
            body,
            header: Some(header_value(&envelope).unwrap()),
            version_header: None,
            if_head,
        }
    }

    async fn read_heads(&self) -> aithos_provider::heads::HeadsRecord {
        let f = fixtures();
        use aithos_provider::heads::HeadsTable as _;
        self.heads
            .read(&f.tenant, &f.did)
            .await
            .expect("a heads record exists")
    }

    fn answer_json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.last.as_ref().expect("an answer").body)
            .unwrap_or(serde_json::Value::Null)
    }
}

// ------------------------------------------------- givens (state + load)

#[given(expr = "the store holds no manifest head")]
async fn no_manifest_head(world: &mut StoreWorld) {
    let f = fixtures();
    use aithos_provider::heads::HeadsTable as _;
    let record = world.heads.read(&f.tenant, &f.did).await;
    assert!(
        record
            .map(|r| r.manifest_chain_hash.is_empty())
            .unwrap_or(true),
        "the world starts with no manifest head"
    );
}

#[given(expr = "the gamma log is empty")]
async fn gamma_log_empty(world: &mut StoreWorld) {
    let f = fixtures();
    use aithos_provider::heads::HeadsTable as _;
    let record = world.heads.read(&f.tenant, &f.did).await;
    assert!(
        record.map(|r| r.gamma_head.is_empty()).unwrap_or(true),
        "the world starts with an empty gamma log"
    );
}

#[given(expr = "the bundle-exported genesis publication package is loaded from p7")]
async fn load_genesis_package(world: &mut StoreWorld) {
    world.load_case("manifest_cases", "genesis_publish", "body_jcs");
}

#[given(expr = "the bundle-exported height 2 publication package is loaded from p7")]
async fn load_h2_package(world: &mut StoreWorld) {
    world.load_case("manifest_cases", "publish_ok", "body_jcs");
}

#[given(expr = "the bundle-exported stale height 2 publication package is loaded from p7")]
async fn load_stale_package(world: &mut StoreWorld) {
    world.load_case("manifest_cases", "publish_cas_stale", "body_jcs");
}

#[given(expr = "the bundle-exported merge publication package for height 3 is loaded from p7")]
async fn load_merge_package(world: &mut StoreWorld) {
    world.load_case("manifest_cases", "publish_merge_no_arbitration", "body_jcs");
}

#[given(
    expr = "a draft.2 manifest whose prev_hash does not name the stored head is loaded from p7"
)]
async fn load_prev_mismatch_package(world: &mut StoreWorld) {
    // The frozen merge package: its prev_hash pins the cad2 twin — which
    // is NOT the genesis head the scenario seeded (A.4 refuses before any
    // write, whatever the artifact's own coherence).
    world.load_case("manifest_cases", "publish_merge_no_arbitration", "body_jcs");
}

#[given(expr = "a draft.2 genesis manifest with a corrupted signature is loaded from p7")]
async fn load_bad_signature_package(world: &mut StoreWorld) {
    world.load_case("manifest_cases", "genesis_bad_signature", "body_jcs");
}

#[given(expr = "the store holds the p7 genesis edition at height 1")]
async fn holds_genesis(world: &mut StoreWorld) {
    let head = p7_case("manifest_cases", "publish_ok")["state_heads"]["manifest"]
        .as_str()
        .unwrap()
        .to_owned();
    world.seed_manifest_heads(1, &head);
}

#[given(expr = "the store holds the p7 height 2 edition at height 2")]
async fn holds_h2(world: &mut StoreWorld) {
    let head = p7_case("manifest_cases", "publish_cas_stale")["state_heads"]["manifest"]
        .as_str()
        .unwrap()
        .to_owned();
    world.seed_manifest_heads(2, &head);
}

#[given(expr = "the store holds two competing editions at height 2")]
async fn holds_competing_editions(world: &mut StoreWorld) {
    // The A.5 table holds ONE head — the twin the CAS serialized first
    // (the merge case's own frozen state); the other twin lives client-side.
    let head = p7_case("manifest_cases", "publish_merge_no_arbitration")["state_heads"]["manifest"]
        .as_str()
        .unwrap()
        .to_owned();
    world.seed_manifest_heads(2, &head);
}

#[given(expr = "the committed p7 genesis gamma entry is loaded")]
async fn load_gamma_genesis(world: &mut StoreWorld) {
    world.load_case("gamma_cases", "append_genesis", "entry_jcs");
}

#[given(expr = "the committed p7 bound-action gamma entry is loaded")]
async fn load_gamma_action(world: &mut StoreWorld) {
    world.load_case("gamma_cases", "append_ok", "entry_jcs");
}

#[given(expr = "the committed p7 concurrent gamma entry is loaded")]
async fn load_gamma_concurrent(world: &mut StoreWorld) {
    world.load_case("gamma_cases", "append_cas_stale", "entry_jcs");
}

#[given(expr = "a bound-action gamma entry with a corrupted signature is loaded from p7")]
async fn load_gamma_bad_signature(world: &mut StoreWorld) {
    world.load_case("gamma_cases", "append_bad_entry_signature", "entry_jcs");
}

#[given(expr = "the store holds the p7 gamma head after the grant entry")]
async fn holds_gamma_after_grant(world: &mut StoreWorld) {
    let head = p7_case("gamma_cases", "append_ok")["state_heads"]["gamma"]
        .as_str()
        .unwrap()
        .to_owned();
    world.seed_gamma_head(&head);
}

#[given(expr = "the store holds the p7 gamma head after the bound action")]
async fn holds_gamma_after_action(world: &mut StoreWorld) {
    let head = p7_case("gamma_cases", "append_cas_stale")["state_heads"]["gamma"]
        .as_str()
        .unwrap()
        .to_owned();
    world.seed_gamma_head(&head);
}

#[given(expr = "the bundle-exported mandate certificate is loaded from p7")]
async fn load_cert(world: &mut StoreWorld) {
    world.load_case("cert_cases", "deposit_cert_ok", "body_jcs");
}

#[given(expr = "a mandate certificate whose subject is a foreign DID is loaded from p7")]
async fn load_foreign_cert(world: &mut StoreWorld) {
    world.load_case("cert_cases", "deposit_cert_foreign_subject", "body_jcs");
}

// -------------------------------------------------------- whens (wire)

#[when(expr = "the owner publishes the loaded manifest with If-Head {string}")]
async fn owner_publishes_if_head(world: &mut StoreWorld, if_head: String) {
    let pending = world.deposit_pending("PUT", "manifest.json", Some(if_head), false);
    world.fire(&pending).await;
}

#[when(expr = "the owner publishes the loaded manifest with If-Head the stored manifest head")]
async fn owner_publishes_stored_head(world: &mut StoreWorld) {
    let head = world.stored_manifest_head.clone().expect("a seeded head");
    let pending = world.deposit_pending("PUT", "manifest.json", Some(head), false);
    world.fire(&pending).await;
}

#[when(expr = "the owner publishes the loaded manifest with If-Head the p7 genesis head")]
async fn owner_publishes_genesis_head(world: &mut StoreWorld) {
    let head = p7_case("manifest_cases", "publish_ok")["state_heads"]["manifest"]
        .as_str()
        .unwrap()
        .to_owned();
    let pending = world.deposit_pending("PUT", "manifest.json", Some(head), false);
    world.fire(&pending).await;
}

#[when(expr = "the owner publishes the loaded manifest with no If-Head")]
async fn owner_publishes_no_if_head(world: &mut StoreWorld) {
    let pending = world.deposit_pending("PUT", "manifest.json", None, false);
    world.fire(&pending).await;
}

#[when(expr = "a grantee appends the loaded gamma entry with If-Head {string}")]
async fn grantee_appends_if_head(world: &mut StoreWorld, if_head: String) {
    let f = fixtures();
    world
        .seed_object(
            &format!("certs/{}.json", f.mandate_id),
            f.mandate_jcs.clone().into_bytes(),
        )
        .await;
    let pending = world.deposit_pending("POST", "gamma", Some(if_head), true);
    world.fire(&pending).await;
}

#[when(expr = "a grantee appends the loaded gamma entry with If-Head the stored gamma head")]
async fn grantee_appends_stored_head(world: &mut StoreWorld) {
    let head = world
        .stored_gamma_head
        .clone()
        .expect("a seeded gamma head");
    grantee_appends_if_head(world, head).await;
}

#[when(expr = "a grantee appends the loaded gamma entry with If-Head the p7 grant head")]
async fn grantee_appends_grant_head(world: &mut StoreWorld) {
    let head = p7_case("gamma_cases", "append_ok")["state_heads"]["gamma"]
        .as_str()
        .unwrap()
        .to_owned();
    grantee_appends_if_head(world, head).await;
}

#[when(expr = "a grantee appends the loaded gamma entry with no If-Head")]
async fn grantee_appends_no_if_head(world: &mut StoreWorld) {
    let f = fixtures();
    world
        .seed_object(
            &format!("certs/{}.json", f.mandate_id),
            f.mandate_jcs.clone().into_bytes(),
        )
        .await;
    let pending = world.deposit_pending("POST", "gamma", None, true);
    world.fire(&pending).await;
}

#[when(expr = "a delegated author deposits the loaded certificate")]
async fn delegated_deposits_cert(world: &mut StoreWorld) {
    // The depositing principal is the p1 grantee under its own chain
    // (#10 covers certs/** for any valid chain; A.4 judges the artifact).
    let f = fixtures();
    world
        .seed_object(
            &format!("certs/{}.json", f.mandate_id),
            f.mandate_jcs.clone().into_bytes(),
        )
        .await;
    let relative = format!("certs/{}.json", f.mandate_id);
    let pending = world.deposit_pending("PUT", &relative, None, true);
    world.fire(&pending).await;
}

#[when(
    expr = "an owner-signed PUT arrives for relative path {string} with an opaque ciphertext body"
)]
async fn owner_put_opaque(world: &mut StoreWorld, relative: String) {
    let pending = world.owner_pending("PUT", &relative, b"\x00opaque-ciphertext-bytes");
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed PUT arrives for relative path {string} with a non-JSON body")]
async fn owner_put_non_json(world: &mut StoreWorld, relative: String) {
    let pending = world.owner_pending("PUT", &relative, b"not-json{{");
    world.fire(&pending).await;
}

// ------------------------------------------------------ thens (heads)

#[then(expr = "the stored manifest head becomes the package new_manifest_head at height {int}")]
async fn manifest_head_advanced(world: &mut StoreWorld, height: u64) {
    let expect = world.loaded_expect.clone().expect("a loaded case");
    let want_head = expect["new_head"].as_str().unwrap();
    assert_eq!(
        expect["new_height"].as_u64().unwrap(),
        height,
        "vector height"
    );
    let record = world.read_heads().await;
    assert_eq!(
        format!("sha256:{}", record.manifest_chain_hash),
        want_head,
        "the table head is the façade's new_manifest_head"
    );
    assert_eq!(record.height, height, "the table height advanced");
    let answer = world.answer_json();
    assert_eq!(answer["head"].as_str(), Some(want_head), "response head");
    assert_eq!(answer["height"].as_u64(), Some(height), "response height");
}

#[then(expr = "the stored gamma head becomes the loaded entry head")]
async fn gamma_head_advanced(world: &mut StoreWorld) {
    let expect = world.loaded_expect.clone().expect("a loaded case");
    let want_head = expect["new_head"].as_str().unwrap();
    let record = world.read_heads().await;
    assert_eq!(record.gamma_head, want_head, "the table gamma head");
    let answer = world.answer_json();
    assert_eq!(answer["head"].as_str(), Some(want_head), "response head");
}

#[then(expr = "the response is {int} {string} carrying the stored manifest head at height {int}")]
async fn cas_mismatch_with_height(world: &mut StoreWorld, status: u16, code: String, height: u64) {
    let answer = world.last.as_ref().expect("an answer");
    assert_eq!(answer.status, status, "status");
    let body = world.answer_json();
    assert_eq!(body["error"].as_str(), Some(code.as_str()), "registry code");
    assert_eq!(
        body["head"].as_str(),
        world.stored_manifest_head.as_deref(),
        "the 409 carries the CURRENT stored head"
    );
    assert_eq!(
        body["height"].as_u64(),
        Some(height),
        "the 409 carries the height"
    );
}

#[then(expr = "the response is {int} {string} carrying the stored gamma head")]
async fn cas_mismatch_gamma(world: &mut StoreWorld, status: u16, code: String) {
    let answer = world.last.as_ref().expect("an answer");
    assert_eq!(answer.status, status, "status");
    let body = world.answer_json();
    assert_eq!(body["error"].as_str(), Some(code.as_str()), "registry code");
    assert_eq!(
        body["head"].as_str(),
        world.stored_gamma_head.as_deref(),
        "the 409 carries the CURRENT stored gamma head"
    );
}

// ==================================================================
// P2 read surface + remaining writes — étape 5 (store-reads.feature +
// the did.json/replica sections of store-publication.feature).
// Fixtures are the FROZEN p9 vector (vectors/p9-store-reads.json) and
// the p8_cold package of p7-bundle-packages.json — every seeded byte is
// a committed byte, nothing re-derived here.
// ==================================================================

fn p9() -> &'static serde_json::Value {
    static P9: OnceLock<serde_json::Value> = OnceLock::new();
    P9.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p9-store-reads.json"
            ))
            .unwrap(),
        )
        .unwrap()
    })
}

/// The frozen p8_cold objects (path → utf8) of p7-bundle-packages.json.
fn p8_objects() -> &'static std::collections::BTreeMap<String, String> {
    static OBJECTS: OnceLock<std::collections::BTreeMap<String, String>> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        let bundle: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p7-bundle-packages.json"
            ))
            .unwrap(),
        )
        .unwrap();
        bundle["packages"]["p8_cold"]["objects"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v["utf8"].as_str().unwrap().to_owned()))
            .collect()
    })
}

fn p8_key_with_prefix(prefix: &str) -> String {
    p8_objects()
        .keys()
        .find(|k| k.starts_with(prefix))
        .unwrap_or_else(|| panic!("p8_cold has a {prefix} object"))
        .clone()
}

fn p9_gamma(field: &str) -> String {
    p9()["fixtures"]["gamma"][field]
        .as_str()
        .unwrap()
        .to_owned()
}

impl StoreWorld {
    /// Seed the p8_cold A.5 head tuple (height 2, the package's own
    /// manifest head), MERGED over whatever gamma state a previous Given
    /// seeded — étape-5 scenarios combine both heads.
    async fn seed_p8_heads(&mut self) {
        let f = fixtures();
        let head = p9()["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "heads_ok")
            .unwrap()["steps"][0]["expect"]["json"]["manifest"]
            .as_str()
            .unwrap()
            .to_owned();
        use aithos_provider::heads::HeadsTable as _;
        let current = self.heads.read(&f.tenant, &f.did).await.unwrap_or_default();
        self.heads.seed(
            &f.tenant,
            &f.did,
            aithos_provider::heads::HeadsRecord {
                height: 2,
                manifest_chain_hash: head.strip_prefix("sha256:").unwrap().to_owned(),
                ..current
            },
        );
        self.stored_manifest_head = Some(head);
    }

    async fn seed_p8_objects(&mut self) {
        for (key, utf8) in p8_objects() {
            self.seed_object(key, utf8.clone().into_bytes()).await;
        }
        // The p9 fixture blobs — the canonical-zone paths the perimeter
        // filtering scenarios need on both sides of the fence.
        for (key, utf8) in p9()["fixtures"]["blobs"].as_object().unwrap() {
            self.seed_object(key, utf8.as_str().unwrap().as_bytes().to_vec())
                .await;
        }
    }

    /// Seed a gamma segment state: the exact bytes at `gamma/2026-07.jsonl`
    /// and the A.5 gamma head fields, MERGED over the manifest fields.
    async fn seed_gamma_segment(&mut self, segment: &str, head: &str) {
        let f = fixtures();
        // The enrollment fixture: the delegated entries in the segment
        // display `authorized_via` — A.4 resolves it from the STORED cert
        // (the replay bootstrap preloads it the same way).
        self.seed_object(
            &format!("certs/{}.json", f.mandate_id),
            f.mandate_jcs.clone().into_bytes(),
        )
        .await;
        self.seed_object("gamma/2026-07.jsonl", segment.as_bytes().to_vec())
            .await;
        use aithos_provider::heads::HeadsTable as _;
        let current = self.heads.read(&f.tenant, &f.did).await.unwrap_or_default();
        self.heads.seed(
            &f.tenant,
            &f.did,
            aithos_provider::heads::HeadsRecord {
                gamma_head: head.to_owned(),
                gamma_segment: "2026-07".into(),
                gamma_segments: vec!["2026-07".into()],
                ..current
            },
        );
        self.stored_gamma_head = Some(head.to_owned());
    }

    /// An owner-signed listing request (`?list=` — the query IS the
    /// request-target, byte-exact in the envelope).
    fn list_pending(&mut self, query: &str, mandated: bool) -> Pending {
        let f = fixtures();
        let path = format!("/t/{}/{}{query}", f.tenant, f.did);
        let envelope = if mandated {
            let key =
                aithos_core::wire::ed25519_pub_to_multibase(&f.agent_sk.verifying_key().to_bytes());
            let signer = f.agent_sk.clone();
            self.envelope(
                &key,
                &signer,
                "GET",
                &path,
                b"",
                None,
                None,
                vec![f.mandate_id.clone()],
                None,
            )
        } else {
            let signer = f.root_sk.clone();
            self.envelope(
                "#root",
                &signer,
                "GET",
                &path,
                b"",
                None,
                None,
                vec![],
                None,
            )
        };
        Pending {
            method: "GET".into(),
            path,
            body: vec![],
            header: Some(header_value(&envelope).unwrap()),
            version_header: None,
            if_head: None,
        }
    }

    /// A signed POST on a collection route (`/batch`, `/sync`).
    fn post_pending(&mut self, route: &str, body: Vec<u8>, mandated: bool) -> Pending {
        let f = fixtures();
        let path = self.abs(route);
        let envelope = if mandated {
            let key =
                aithos_core::wire::ed25519_pub_to_multibase(&f.agent_sk.verifying_key().to_bytes());
            let signer = f.agent_sk.clone();
            self.envelope(
                &key,
                &signer,
                "POST",
                &path,
                &body,
                None,
                None,
                vec![f.mandate_id.clone()],
                None,
            )
        } else {
            let signer = f.root_sk.clone();
            self.envelope(
                "#root",
                &signer,
                "POST",
                &path,
                &body,
                None,
                None,
                vec![],
                None,
            )
        };
        Pending {
            method: "POST".into(),
            path,
            body,
            header: Some(header_value(&envelope).unwrap()),
            version_header: None,
            if_head: None,
        }
    }

    /// The multipart/mixed parts of the last answer: `(location, status,
    /// body)` — the same byte-exact split as the p9 replay driver.
    fn multipart_parts(&self) -> Vec<(String, u16, Vec<u8>)> {
        let answer = self.last.as_ref().expect("an answer");
        let ctype = answer
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        let boundary = ctype
            .split_once("boundary=")
            .map(|(_, b)| b.trim().trim_matches('"'))
            .expect("a multipart answer");
        let delim = format!("--{boundary}");
        let body = &answer.body;
        let text_start = delim.len() + 2;
        assert!(
            body.starts_with(format!("{delim}\r\n").as_bytes()),
            "the pack opens with the boundary"
        );
        let mut parts = Vec::new();
        let sep = format!("\r\n{delim}");
        let mut rest = &body[text_start..];
        loop {
            let next = rest
                .windows(sep.len())
                .position(|w| w == sep.as_bytes())
                .expect("a closing boundary");
            let chunk = &rest[..next];
            let split = chunk
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .expect("part headers");
            let headers = std::str::from_utf8(&chunk[..split]).expect("part headers are ASCII");
            let part_body = chunk[split + 4..].to_vec();
            let mut location = String::new();
            let mut status = 0u16;
            for line in headers.lines() {
                if let Some((name, value)) = line.split_once(':') {
                    match name.trim().to_ascii_lowercase().as_str() {
                        "content-location" => location = value.trim().to_owned(),
                        "x-aithos-status" => status = value.trim().parse().expect("part status"),
                        _ => {}
                    }
                }
            }
            parts.push((location, status, part_body));
            rest = &rest[next + sep.len()..];
            if rest.starts_with(b"--") {
                break;
            }
            assert!(rest.starts_with(b"\r\n"), "boundary line ends CRLF");
            rest = &rest[2..];
        }
        parts
    }
}

// ------------------------------------------------------------- givens

#[given(expr = "the store holds the p8_cold edition at height 2")]
async fn holds_p8_heads(world: &mut StoreWorld) {
    world.seed_p8_heads().await;
}

#[given(expr = "the store holds the p8_cold edition at height 2 with its reachable objects")]
async fn holds_p8_full(world: &mut StoreWorld) {
    world.seed_p8_heads().await;
    world.seed_p8_objects().await;
}

#[given(expr = "the store holds the p8_cold edition at height 2 without the edition 1 slot")]
async fn holds_p8_purged(world: &mut StoreWorld) {
    world.seed_p8_heads().await;
    for (key, utf8) in p8_objects() {
        if key != "manifests/1.json" {
            world.seed_object(key, utf8.clone().into_bytes()).await;
        }
    }
}

#[given(expr = "the store holds the p8_cold edition at height 1 before its draft.2 publication")]
async fn holds_p8_pre_publish(world: &mut StoreWorld) {
    let genesis = p8_objects()["manifests/1.json"].clone();
    world
        .seed_object("manifest.json", genesis.clone().into_bytes())
        .await;
    world
        .seed_object("manifests/1.json", genesis.into_bytes())
        .await;
    // The predecessor head of the frozen package (its expected_predecessors).
    let head = p7_case("manifest_cases", "publish_ok")["state_heads"]["manifest"]
        .as_str()
        .unwrap()
        .to_owned();
    world.seed_manifest_heads(1, &head);
}

#[given(expr = "the store holds the p7 gamma segment after the grant entry")]
async fn holds_segment_after_grant(world: &mut StoreWorld) {
    let segment = p9_gamma("grant_jcs") + "\n";
    let head = p9_gamma("grant_head");
    world.seed_gamma_segment(&segment, &head).await;
}

#[given(expr = "the store holds the p7 gamma segment after the bound action")]
async fn holds_segment_after_action(world: &mut StoreWorld) {
    let segment = p9_gamma("grant_jcs") + "\n" + &p9_gamma("action_jcs") + "\n";
    let head = p9_gamma("action_head");
    world.seed_gamma_segment(&segment, &head).await;
}

#[given(expr = "a replica appending the committed bound-action entry is loaded")]
async fn load_replica_action(world: &mut StoreWorld) {
    world.loaded_body =
        Some((p9_gamma("grant_jcs") + "\n" + &p9_gamma("action_jcs") + "\n").into_bytes());
}

#[given(expr = "a replica appending the committed concurrent entry over the grant head is loaded")]
async fn load_replica_concurrent(world: &mut StoreWorld) {
    world.loaded_body =
        Some((p9_gamma("grant_jcs") + "\n" + &p9_gamma("concurrent_jcs") + "\n").into_bytes());
}

#[given(expr = "a replica rewriting the stored first entry is loaded")]
async fn load_replica_rewrite(world: &mut StoreWorld) {
    // The action entry alone: NOT an extension of the stored grant line.
    world.loaded_body = Some((p9_gamma("action_jcs") + "\n").into_bytes());
}

#[given(expr = "a replica appending a bound-action entry with a corrupted signature is loaded")]
async fn load_replica_corrupted(world: &mut StoreWorld) {
    world.loaded_body =
        Some((p9_gamma("grant_jcs") + "\n" + &p9_gamma("corrupted_jcs") + "\n").into_bytes());
}

#[given(expr = "the tenant binds the p9 genesis DID with no stored document")]
async fn binds_genesis_did(world: &mut StoreWorld) {
    // Rebuild the control plane with the extra bind-only DID (the
    // pre-genesis state: enrollment precedes, no document stored).
    let f = fixtures();
    let genesis_did = p9()["fixtures"]["genesis"]["did"].as_str().unwrap();
    let bootstrap = serde_json::json!({
        "tenants": [{
            "tenant": f.tenant,
            "dids": [
                {"did": f.did, "did_json": f.did_json},
                {"did": genesis_did},
            ],
        }],
    });
    let (control, preloads, _) =
        ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("genesis bootstrap");
    let objects = Arc::new(MemObjects::new());
    for (tenant, did, key, bytes) in preloads {
        futures::executor::block_on(objects.put(&tenant, &did, &key, bytes));
    }
    let heads = Arc::new(MemHeads::new());
    let dns = Arc::new(MemDnsTxt::new());
    world.heads = heads.clone();
    world.dns = dns.clone();
    world.state = Arc::new(AppState {
        control,
        objects,
        heads,
        deposit_locks: Default::default(),
        nonces: Arc::new(MemNonces::new(600)),
        dns,
        acme: AcmeState::new(),
        authority: world.authority.clone(),
        test_now_enabled: true,
    });
}

#[given(expr = "the p9 genesis did.json is loaded")]
async fn load_genesis_did_json(world: &mut StoreWorld) {
    world.loaded_body = Some(
        p9()["fixtures"]["genesis"]["did_json_jcs"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec(),
    );
}

#[given(expr = "a genesis did.json whose id names a foreign DID is loaded")]
async fn load_foreign_did_json(world: &mut StoreWorld) {
    // The committed p1 document: perfectly self-consistent, wrong path.
    world.loaded_body = Some(fixtures().did_json.clone().into_bytes());
}

#[given(expr = "the store holds the vector did.json for the vector DID")]
async fn holds_vector_did_json(world: &mut StoreWorld) {
    let f = fixtures();
    let stored = world
        .state
        .objects
        .get(&f.tenant, &f.did, "did.json")
        .await
        .expect("the Background stored it");
    assert_eq!(stored, f.did_json.as_bytes());
}

#[given(expr = "a successor did.json signed under the stored succession key is loaded")]
async fn load_successor_succession(world: &mut StoreWorld) {
    world.loaded_body = Some(
        p9()["fixtures"]["rotation"]["succession_signed_jcs"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec(),
    );
}

#[given(expr = "a successor did.json signed under the stored root key is loaded")]
async fn load_successor_root(world: &mut StoreWorld) {
    world.loaded_body = Some(
        p9()["fixtures"]["rotation"]["root_signed_jcs"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec(),
    );
}

// -------------------------------------------------------------- whens

#[when(expr = "an owner-signed GET arrives for {string}")]
async fn owner_get_route(world: &mut StoreWorld, route: String) {
    let relative = route.strip_prefix('/').unwrap_or(&route).to_owned();
    let pending = world.owner_pending("GET", &relative, b"");
    world.fire(&pending).await;
}

#[when(expr = "a correctly signed mandated GET arrives for {string}")]
async fn mandated_get_route(world: &mut StoreWorld, route: String) {
    let relative = route.strip_prefix('/').unwrap_or(&route).to_owned();
    let pending = world.mandated_pending(&relative, None);
    world.fire(&pending).await;
}

#[when(expr = "an anonymous GET arrives for {string}")]
async fn anonymous_get_route(world: &mut StoreWorld, route: String) {
    let relative = route.strip_prefix('/').unwrap_or(&route).to_owned();
    let pending = Pending {
        method: "GET".into(),
        path: world.abs(&relative),
        body: vec![],
        header: None,
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an anonymous GET arrives for the p8_cold public section alias path")]
async fn anonymous_get_public_alias(world: &mut StoreWorld) {
    let pending = Pending {
        method: "GET".into(),
        path: world.abs(&p8_key_with_prefix("public/sections/")),
        body: vec![],
        header: None,
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a correctly signed mandated GET arrives for the p8_cold circle blob alias path")]
async fn mandated_get_circle_alias(world: &mut StoreWorld) {
    let key = p8_key_with_prefix("circle/blobs/");
    let pending = world.mandated_pending(&key, None);
    world.fire(&pending).await;
}

#[when(expr = "a correctly signed mandated GET arrives for the p8_cold changeset sidecar path")]
async fn mandated_get_changeset(world: &mut StoreWorld) {
    let key = p8_key_with_prefix("changesets/");
    let pending = world.mandated_pending(&key, None);
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed list arrives for prefix {string}")]
async fn owner_list(world: &mut StoreWorld, prefix: String) {
    let pending = world.list_pending(&format!("?list={prefix}"), false);
    world.fire(&pending).await;
}

#[when(expr = "a correctly signed mandated list arrives for prefix {string}")]
async fn mandated_list(world: &mut StoreWorld, prefix: String) {
    let pending = world.list_pending(&format!("?list={prefix}"), true);
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed list arrives for prefix {string} with limit {int}")]
async fn owner_list_limit(world: &mut StoreWorld, prefix: String, limit: u64) {
    world.prev_list_paths = world.last.as_ref().and_then(|answer| {
        serde_json::from_slice::<serde_json::Value>(&answer.body)
            .ok()?
            .get("paths")?
            .as_array()
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(|p| p.as_str().map(str::to_owned))
                    .collect()
            })
    });
    let pending = world.list_pending(&format!("?list={prefix}&limit={limit}"), false);
    world.fire(&pending).await;
}

#[when(
    expr = "an owner-signed list arrives for prefix {string} with limit {int} after the last returned path"
)]
async fn owner_list_after(world: &mut StoreWorld, prefix: String, limit: u64) {
    let previous: Vec<String> = serde_json::from_slice::<serde_json::Value>(&world.last().body)
        .expect("a listing answer")["paths"]
        .as_array()
        .expect("paths")
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    let after = previous.last().expect("a non-empty page").clone();
    world.prev_list_paths = Some(previous);
    let pending = world.list_pending(
        &format!("?list={prefix}&after={after}&limit={limit}"),
        false,
    );
    world.fire(&pending).await;
}

#[when(
    expr = "a correctly signed mandated batch arrives for a covered path, a missing covered path and an uncovered path"
)]
async fn mandated_batch_mixed(world: &mut StoreWorld) {
    let body = serde_json::json!({"paths": [
        p8_key_with_prefix("circle/blobs/"),
        "e/circle/blobs/01000000000000000000000001.enc",
        "e/self/blobs/01000000000000000000000000.enc",
    ]})
    .to_string()
    .into_bytes();
    let pending = world.post_pending("batch", body, true);
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed batch arrives with {int} paths")]
async fn owner_batch_overflow(world: &mut StoreWorld, count: usize) {
    let paths = vec!["e/circle/blobs/01000000000000000000000001.enc"; count];
    let body = serde_json::json!({ "paths": paths })
        .to_string()
        .into_bytes();
    let pending = world.post_pending("batch", body, false);
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed batch arrives with a non-JSON body")]
async fn owner_batch_bad_body(world: &mut StoreWorld) {
    let pending = world.post_pending("batch", b"not-json".to_vec(), false);
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed sync arrives with have_edition {int}")]
async fn owner_sync(world: &mut StoreWorld, have: u64) {
    let body = serde_json::json!({ "have_edition": have })
        .to_string()
        .into_bytes();
    let pending = world.post_pending("sync", body, false);
    world.fire(&pending).await;
}

#[when(expr = "an owner-signed PUT arrives for relative path {string} with a JSON body")]
async fn owner_put_json_body(world: &mut StoreWorld, relative: String) {
    let pending = world.owner_pending("PUT", &relative, b"{}");
    world.fire(&pending).await;
}

#[when(expr = "the owner deposits the p8_cold changeset sidecar at its digest path")]
async fn owner_deposit_changeset(world: &mut StoreWorld) {
    let key = p8_key_with_prefix("changesets/");
    let body = p8_objects()[&key].clone().into_bytes();
    let pending = world.owner_pending("PUT", &key, &body);
    world.fire(&pending).await;
}

#[when(expr = "the owner deposits the p8_cold changeset sidecar at a wrong digest path")]
async fn owner_deposit_changeset_wrong(world: &mut StoreWorld) {
    let key = p8_key_with_prefix("changesets/");
    let body = p8_objects()[&key].clone().into_bytes();
    let wrong = format!("changesets/{}.json", "0".repeat(64));
    let pending = world.owner_pending("PUT", &wrong, &body);
    world.fire(&pending).await;
}

#[when(expr = "the genesis owner deposits the loaded did.json")]
async fn genesis_owner_deposits(world: &mut StoreWorld) {
    // « The genesis owner » is the owner OF THE DEPOSITED DOCUMENT: the
    // envelope signs #root under the loaded document's own root key (the
    // A.2 #7 genesis exception resolves against it).
    let body = world.loaded_body.clone().expect("a loaded did.json");
    let doc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let genesis = &p9()["fixtures"]["genesis"];
    let signer = if doc["keys"]["root"]
        == genesis["did_json_jcs"]
            .as_str()
            .map(|jcs| serde_json::from_str::<serde_json::Value>(jcs).unwrap())
            .unwrap()["keys"]["root"]
    {
        sk_from_hex(genesis["root_seed_hex"].as_str().unwrap())
    } else {
        fixtures().root_sk.clone()
    };
    let genesis_did = genesis["did"].as_str().unwrap();
    let path = format!("/t/{}/{genesis_did}/did.json", fixtures().tenant);
    let envelope = world.envelope(
        "#root",
        &signer,
        "PUT",
        &path,
        &body,
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "PUT".into(),
        path,
        body,
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "a foreign key deposits the loaded did.json")]
async fn foreign_key_deposits(world: &mut StoreWorld) {
    let f = fixtures();
    let body = world.loaded_body.clone().expect("a loaded did.json");
    let genesis_did = p9()["fixtures"]["genesis"]["did"].as_str().unwrap();
    let path = format!("/t/{}/{genesis_did}/did.json", f.tenant);
    // A perfectly valid key that is NOT the deposited document's root.
    let signer = f.agent_sk.clone();
    let envelope = world.envelope(
        "#root",
        &signer,
        "PUT",
        &path,
        &body,
        None,
        None,
        vec![],
        None,
    );
    let pending = Pending {
        method: "PUT".into(),
        path,
        body,
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "the owner deposits the loaded did.json")]
async fn owner_deposits_did_json(world: &mut StoreWorld) {
    let pending = world.deposit_pending("PUT", "did.json", None, false);
    world.fire(&pending).await;
}

#[when(expr = "the owner replicates the loaded segment with If-Head the stored gamma head")]
async fn owner_replicates_stored_head(world: &mut StoreWorld) {
    let head = world.stored_gamma_head.clone().expect("a seeded head");
    let pending = world.deposit_pending("PUT", "gamma/2026-07.jsonl", Some(head), false);
    world.fire(&pending).await;
}

#[when(expr = "the owner replicates the loaded segment with no If-Head")]
async fn owner_replicates_no_head(world: &mut StoreWorld) {
    let pending = world.deposit_pending("PUT", "gamma/2026-07.jsonl", None, false);
    world.fire(&pending).await;
}

#[when(expr = "the owner replicates the loaded segment with If-Head the p7 grant head")]
async fn owner_replicates_grant_head(world: &mut StoreWorld) {
    let pending = world.deposit_pending(
        "PUT",
        "gamma/2026-07.jsonl",
        Some(p9_gamma("grant_head")),
        false,
    );
    world.fire(&pending).await;
}

// -------------------------------------------------------------- thens

#[then(expr = "the heads body carries height 2, the p8_cold manifest head, and a null gamma head")]
async fn heads_body_matches(world: &mut StoreWorld) {
    assert_eq!(world.last().status, 200, "accepted");
    let body = world.answer_json();
    assert_eq!(body["height"], 2);
    assert_eq!(
        body["manifest"].as_str(),
        world.stored_manifest_head.as_deref()
    );
    assert!(body["gamma"].is_null(), "no gamma head in this state");
    assert!(body["segment"].is_null(), "no segment in this state");
}

#[then(expr = "the listing carries every stored path in lexicographic order, not truncated")]
async fn listing_carries_all(world: &mut StoreWorld) {
    let f = fixtures();
    let stored = world.state.objects.list(&f.tenant, &f.did).await;
    let body = world.answer_json();
    let paths: Vec<&str> = body["paths"]
        .as_array()
        .expect("paths")
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert_eq!(paths, stored, "every stored path, already sorted");
    assert_eq!(body["truncated"], false);
}

#[then(expr = "the listing carries no path outside the covered perimeter")]
async fn listing_filtered(world: &mut StoreWorld) {
    let body = world.answer_json();
    let paths = body["paths"].as_array().expect("paths");
    assert!(!paths.is_empty(), "the covered circle path is listed");
    for path in paths {
        assert!(
            path.as_str().unwrap().starts_with("e/circle/"),
            "read.circle only reaches its own zone under e/: {path}"
        );
    }
}

#[then(expr = "the listing carries {int} paths and is truncated")]
async fn listing_page(world: &mut StoreWorld, count: usize) {
    let body = world.answer_json();
    assert_eq!(body["paths"].as_array().expect("paths").len(), count);
    assert_eq!(body["truncated"], true);
}

#[then(expr = "the listing continues exactly after the previous page")]
async fn listing_continues(world: &mut StoreWorld) {
    let f = fixtures();
    let stored = world.state.objects.list(&f.tenant, &f.did).await;
    let previous = world.prev_list_paths.clone().expect("a previous page");
    let last = previous.last().expect("non-empty previous page");
    let position = stored.iter().position(|p| p == last).expect("known path");
    let body = world.answer_json();
    let paths: Vec<String> = body["paths"]
        .as_array()
        .expect("paths")
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        paths,
        stored[position + 1..position + 1 + paths.len()],
        "the page starts exactly after `after`"
    );
}

#[then(expr = "the batch parts answer 200, 404 and 403 in request order")]
async fn batch_parts_order(world: &mut StoreWorld) {
    assert_eq!(world.last().status, 200, "the batch itself is a 200");
    let parts = world.multipart_parts();
    let statuses: Vec<u16> = parts.iter().map(|(_, s, _)| *s).collect();
    assert_eq!(statuses, vec![200, 404, 403], "per-part statuses in order");
}

#[then(expr = "only the {int} part carries a body")]
async fn only_status_part_has_body(world: &mut StoreWorld, status: u16) {
    for (location, part_status, body) in world.multipart_parts() {
        if part_status == status {
            assert!(!body.is_empty(), "the {status} part has its bytes");
        } else {
            assert!(
                body.is_empty(),
                "no body on the {part_status} part {location}"
            );
        }
    }
}

#[then(
    expr = "the pack opens with manifest.json and carries exactly the paths changed since edition 1"
)]
async fn sync_pack_delta(world: &mut StoreWorld) {
    let f = fixtures();
    assert_eq!(world.last().status, 200);
    // Recompute the frozen rule from the STORED manifests: manifest.json
    // first, then the lexicographic diff of the pinned files maps.
    let held: serde_json::Value = serde_json::from_str(&p8_objects()["manifests/1.json"]).unwrap();
    let tip: serde_json::Value = serde_json::from_str(&p8_objects()["manifests/2.json"]).unwrap();
    let mut expected = vec!["manifest.json".to_owned()];
    let held_files = held["files"].as_object().unwrap();
    for (key, hash) in tip["files"].as_object().unwrap() {
        if held_files.get(key) != Some(hash) {
            expected.push(key.clone());
        }
    }
    let parts = world.multipart_parts();
    let locations: Vec<String> = parts
        .iter()
        .map(|(location, _, _)| {
            location
                .strip_prefix(&format!("/t/{}/{}/", f.tenant, f.did))
                .expect("part location under the DID")
                .to_owned()
        })
        .collect();
    assert_eq!(locations, expected, "manifest.json first, then the diff");
    for (location, status, body) in &parts {
        assert_eq!(*status, 200, "every delta part serves: {location}");
        assert!(!body.is_empty() || location.ends_with("gamma/2026-07.jsonl"));
    }
}

#[then(expr = "the pack carries manifest.json alone")]
async fn sync_pack_tip_only(world: &mut StoreWorld) {
    assert_eq!(world.last().status, 200);
    let parts = world.multipart_parts();
    assert_eq!(parts.len(), 1, "one part");
    assert!(parts[0].0.ends_with("/manifest.json"));
    assert_eq!(parts[0].1, 200);
    assert_eq!(parts[0].2, p8_objects()["manifest.json"].as_bytes());
}

#[then(expr = "the response is {int} {string} with reason {string}")]
async fn response_with_reason(world: &mut StoreWorld, status: u16, code: String, reason: String) {
    let answer = world.last.as_ref().expect("an answer");
    assert_eq!(
        answer.status,
        status,
        "status (body: {})",
        String::from_utf8_lossy(&answer.body)
    );
    let body = world.answer_json();
    assert_eq!(body["error"].as_str(), Some(code.as_str()), "registry code");
    assert_eq!(
        body["reason"].as_str(),
        Some(reason.as_str()),
        "the closed short reason"
    );
}

#[then(expr = "the stored gamma head becomes the appended entry head")]
async fn gamma_head_after_replica(world: &mut StoreWorld) {
    assert_eq!(world.last().status, 200, "accepted");
    let record = world.read_heads().await;
    assert_eq!(record.gamma_head, p9_gamma("action_head"));
    assert_eq!(
        world.answer_json()["head"].as_str(),
        Some(p9_gamma("action_head").as_str()),
        "the accept body carries the new head"
    );
}

// ==================================================================
// /acme/txt — annexe B.5 (store-acme.feature)
// ==================================================================

const ACME_PATH: &str = "/acme/txt";

fn acme_body(hostname: &str, value: &str) -> Vec<u8> {
    serde_jcs::to_string(&serde_json::json!({"hostname": hostname, "value": value}))
        .unwrap()
        .into_bytes()
}

impl StoreWorld {
    /// A signed B.5 request: envelope A.2 over `/acme/txt` with
    /// `key = gateway_pub`, `mandate: []` (unless a scenario derails it).
    #[allow(clippy::too_many_arguments)]
    fn acme_pending(
        &mut self,
        method: &str,
        body: Vec<u8>,
        signer: &SigningKey,
        key: &str,
        mandate: Vec<String>,
        at: Option<&str>,
        nonce: Option<&str>,
        host: Option<&str>,
    ) -> Pending {
        let envelope = self.envelope(
            key, signer, method, ACME_PATH, &body, at, nonce, mandate, host,
        );
        Pending {
            method: method.to_owned(),
            path: ACME_PATH.to_owned(),
            body,
            header: Some(header_value(&envelope).unwrap()),
            version_header: None,
            if_head: None,
        }
    }

    fn gateway_pending(&mut self, method: &str, hostname: &str, value: &str) -> Pending {
        let f = fixtures();
        let key = f.gateway_pub.clone();
        let signer = f.gateway_sk.clone();
        self.acme_pending(
            method,
            acme_body(hostname, value),
            &signer,
            &key,
            vec![],
            None,
            None,
            None,
        )
    }

    fn now_ms(&self) -> i64 {
        parse_rfc3339z_ms(&self.now).expect("world clock is RFC 3339 Z")
    }
}

// ------------------------------------------------------- acme background

#[given(
    expr = "the control plane binds gateway key {string} to tenant {string} and hostname {string}"
)]
async fn acme_binding_exists(
    world: &mut StoreWorld,
    gateway_pub: String,
    tenant: String,
    hostname: String,
) {
    // The fixture bootstrap (built from the committed p6 vector) must
    // already carry exactly this binding — the Background asserts it.
    let f = fixtures();
    assert_eq!(gateway_pub, f.gateway_pub, "fixture gateway key");
    let binding = world
        .state
        .control
        .resolve_tunnel(&gateway_pub)
        .expect("the demo binding is bootstrapped");
    assert_eq!(binding.tenant, tenant);
    assert_eq!(binding.hostname, hostname);
}

// ----------------------------------------------------------- acme givens

#[given(expr = "the bound gateway posed TXT value {string} for hostname {string}")]
async fn gateway_posed(world: &mut StoreWorld, value: String, hostname: String) {
    let pending = world.gateway_pending("PUT", &hostname, &value);
    world.fire(&pending).await;
    assert_eq!(world.last().status, 204, "the challenge PUT is accepted");
}

#[given(expr = "the binding of the gateway key is suspended")]
async fn binding_suspended(world: &mut StoreWorld) {
    let (state, dns, heads) = StoreWorld::state(&world.authority, true, Suspension::Binding);
    world.state = state;
    world.dns = dns;
    world.heads = heads;
}

#[given(expr = "the tenant {string} is suspended")]
async fn tenant_suspended(world: &mut StoreWorld, tenant: String) {
    assert_eq!(tenant, fixtures().tenant, "fixture tenant");
    let (state, dns, heads) = StoreWorld::state(&world.authority, true, Suspension::Tenant);
    world.state = state;
    world.dns = dns;
    world.heads = heads;
}

#[given(expr = "the bound gateway posed 10 challenge values within the hour")]
async fn gateway_posed_ten(world: &mut StoreWorld) {
    let hostname = fixtures().demo_hostname.clone();
    for i in 0..10 {
        let pending = world.gateway_pending("PUT", &hostname, &format!("bdd-rl-{i:02}"));
        world.fire(&pending).await;
        assert_eq!(world.last().status, 204, "warmup PUT {i} is accepted");
    }
}

#[given(expr = "the server clock advances by {int} seconds")]
async fn clock_advances(world: &mut StoreWorld, secs: i64) {
    world.now = render_rfc3339z(world.now_ms() + secs * 1000);
}

#[given(
    expr = "a gateway PUT for a foreign hostname with nonce {string} was refused with {string}"
)]
async fn gateway_put_foreign_refused(world: &mut StoreWorld, nonce: String, code: String) {
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let pending = world.acme_pending(
        "PUT",
        acme_body("other.mcp.aithos.fr", "bdd-nonce-burn"),
        &signer,
        &key,
        vec![],
        None,
        Some(&nonce),
        None,
    );
    world.fire(&pending).await;
    let answer = world.last.take().expect("fired");
    world.assert_error(&answer, 403, &code);
    world.last = Some(answer);
    world.last_acme = Some(pending);
}

// ------------------------------------------------------------ acme whens

#[when(expr = "the bound gateway PUTs {string} for hostname {string} with value {string}")]
async fn gateway_puts(world: &mut StoreWorld, path: String, hostname: String, value: String) {
    assert_eq!(path, ACME_PATH);
    let pending = world.gateway_pending("PUT", &hostname, &value);
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway DELETEs {string} for hostname {string} with value {string}")]
async fn gateway_deletes(world: &mut StoreWorld, path: String, hostname: String, value: String) {
    assert_eq!(path, ACME_PATH);
    let pending = world.gateway_pending("DELETE", &hostname, &value);
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway GETs {string} with a valid envelope")]
async fn gateway_gets(world: &mut StoreWorld, path: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let pending = world.acme_pending("GET", vec![], &signer, &key, vec![], None, None, None);
    world.fire(&pending).await;
}

#[when(expr = "an unsigned PUT arrives at {string} with a well-formed body")]
async fn unsigned_acme_put(world: &mut StoreWorld, path: String) {
    let pending = Pending {
        method: "PUT".into(),
        path,
        body: acme_body(&fixtures().demo_hostname, "bdd-unsigned"),
        header: None,
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "an unsigned PUT arrives at path {string}")]
async fn unsigned_put_raw_path(world: &mut StoreWorld, path: String) {
    let pending = Pending {
        method: "PUT".into(),
        path,
        body: b"{}".to_vec(),
        header: None,
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} claiming wire version {string}")]
async fn gateway_puts_version(world: &mut StoreWorld, path: String, version: String) {
    assert_eq!(path, ACME_PATH);
    let hostname = fixtures().demo_hostname.clone();
    let mut pending = world.gateway_pending("PUT", &hostname, "bdd-version");
    pending.version_header = Some(version);
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} carrying mandate list {string}")]
async fn gateway_puts_mandated(world: &mut StoreWorld, path: String, mandate_id: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let hostname = f.demo_hostname.clone();
    let pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-mandated"),
        &signer,
        &key,
        vec![mandate_id],
        None,
        None,
        None,
    );
    world.fire(&pending).await;
}

#[when(expr = "an owner-root-signed PUT arrives at {string} with a well-formed body")]
async fn owner_root_acme_put(world: &mut StoreWorld, path: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let signer = f.root_sk.clone();
    let hostname = f.demo_hostname.clone();
    let pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-owner-key"),
        &signer,
        "#root",
        vec![],
        None,
        None,
        None,
    );
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} with a body carrying an extra field {string}")]
async fn gateway_puts_extra_field(world: &mut StoreWorld, path: String, field: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let mut body: serde_json::Value = serde_json::json!({
        "hostname": f.demo_hostname, "value": "bdd-extra-field",
    });
    body[field] = serde_json::json!(300);
    let body = serde_jcs::to_string(&body).unwrap().into_bytes();
    let pending = world.acme_pending("PUT", body, &signer, &key, vec![], None, None, None);
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} for hostname {string} with a 256-character value")]
async fn gateway_puts_long_value(world: &mut StoreWorld, path: String, hostname: String) {
    assert_eq!(path, ACME_PATH);
    let pending = world.gateway_pending("PUT", &hostname, &"A".repeat(256));
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway signs a PUT {string} body but sends different bytes")]
async fn gateway_puts_tampered(world: &mut StoreWorld, path: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let hostname = f.demo_hostname.clone();
    let mut pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-signed-body"),
        &signer,
        &key,
        vec![],
        None,
        None,
        None,
    );
    pending.body = acme_body(&hostname, "bdd-tampered-body");
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} whose envelope names host {string}")]
async fn gateway_puts_wrong_host(world: &mut StoreWorld, path: String, host: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let hostname = f.demo_hostname.clone();
    let pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-cross-host"),
        &signer,
        &key,
        vec![],
        None,
        None,
        Some(&host),
    );
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} signed at {string}")]
async fn gateway_puts_signed_at(world: &mut StoreWorld, path: String, at: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let hostname = f.demo_hostname.clone();
    let pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-skew-case"),
        &signer,
        &key,
        vec![],
        Some(&at),
        None,
        None,
    );
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} twice with nonce {string}")]
async fn gateway_puts_twice(world: &mut StoreWorld, path: String, nonce: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let hostname = f.demo_hostname.clone();
    let pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-nonce-twice"),
        &signer,
        &key,
        vec![],
        None,
        Some(&nonce),
        None,
    );
    world.fire(&pending).await;
    assert_eq!(world.last().status, 204, "first presentation is accepted");
    world.fire(&pending).await;
}

#[when(expr = "the bound gateway PUTs {string} with its signature corrupted")]
async fn gateway_puts_corrupted(world: &mut StoreWorld, path: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let key = f.gateway_pub.clone();
    let signer = f.gateway_sk.clone();
    let hostname = f.demo_hostname.clone();
    let body = acme_body(&hostname, "bdd-badsig");
    let mut envelope = world.envelope(
        &key,
        &signer,
        "PUT",
        ACME_PATH,
        &body,
        None,
        None,
        vec![],
        None,
    );
    corrupt(&mut envelope.signature.value);
    let pending = Pending {
        method: "PUT".into(),
        path: ACME_PATH.to_owned(),
        body,
        header: Some(header_value(&envelope).unwrap()),
        version_header: None,
        if_head: None,
    };
    world.fire(&pending).await;
}

#[when(expr = "the same gateway PUT is presented again")]
async fn same_gateway_put_again(world: &mut StoreWorld) {
    let pending = world.last_acme.take().expect("a refused gateway PUT");
    world.fire(&pending).await;
    world.last_acme = Some(pending);
}

#[when(expr = "an unenrolled gateway key PUTs {string} for hostname {string}")]
async fn stranger_puts(world: &mut StoreWorld, path: String, hostname: String) {
    assert_eq!(path, ACME_PATH);
    let f = fixtures();
    let signer = f.stranger_sk.clone();
    let key = aithos_core::wire::ed25519_pub_to_multibase(&signer.verifying_key().to_bytes());
    let pending = world.acme_pending(
        "PUT",
        acme_body(&hostname, "bdd-unenrolled"),
        &signer,
        &key,
        vec![],
        None,
        None,
        None,
    );
    world.fire(&pending).await;
}

#[when(expr = "the acme purge runs {int} seconds later")]
async fn acme_purge_runs(world: &mut StoreWorld, secs: i64) {
    let now_ms = world.now_ms() + secs * 1000;
    world
        .state
        .acme
        .purge_stale(world.state.dns.as_ref(), now_ms)
        .await;
}

// ------------------------------------------------------------ acme thens

#[then(expr = "the request is accepted with status {int}")]
async fn accepted_with_status(world: &mut StoreWorld, status: u16) {
    let answer = world.last();
    assert_eq!(
        answer.status,
        status,
        "expected {status}, got {} ({})",
        answer.status,
        String::from_utf8_lossy(&answer.body)
    );
}

#[then(expr = "the DNS backend holds TXT {string} with value {string} and TTL {int}")]
async fn dns_holds(world: &mut StoreWorld, name: String, value: String, ttl: i64) {
    let record = world.dns.record_of(&name);
    assert_eq!(record, Some((value, ttl)), "TXT {name}");
}

#[then(expr = "the DNS backend holds no TXT for {string}")]
async fn dns_holds_none(world: &mut StoreWorld, name: String) {
    assert_eq!(world.dns.record_of(&name), None, "TXT {name} must be gone");
}

// ------------------------------------------------------------- helpers

fn base64_url(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn corrupt(hex_sig: &mut String) {
    let tail = if hex_sig.ends_with("00") { "ff" } else { "00" };
    hex_sig.truncate(hex_sig.len() - 2);
    hex_sig.push_str(tail);
}

#[tokio::main]
async fn main() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let _ = LOG_BUFFER.set(buffer.clone());
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(BufMake(buffer))
        .with_ansi(false)
        .init();
    // The house @wip discipline (the bundle harness pattern): contract
    // scenarios land tagged @wip and are excluded until their gate wires
    // the steps; everything that runs may not skip.
    StoreWorld::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit("tests/features/store", |_, _, scenario| {
            !scenario.tags.iter().any(|tag| tag == "wip")
        })
        .await;
}
