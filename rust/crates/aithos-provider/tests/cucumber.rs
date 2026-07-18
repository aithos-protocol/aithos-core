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
        Fixtures {
            tenant: p1["tenant"].as_str().unwrap().to_owned(),
            did: p1["did"].as_str().unwrap().to_owned(),
            did_json: p1["did_json_jcs"].as_str().unwrap().to_owned(),
            mandate_id: mandate["id"].as_str().unwrap().to_owned(),
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
    authority: String,
    now: String,
    nonce_counter: u64,
    last: Option<Answer>,
    previous: Option<Answer>,
    last_mandated: Option<Pending>,
    last_acme: Option<Pending>,
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
        let (state, dns) = Self::state(&authority, true, Suspension::None);
        Self {
            state,
            dns,
            authority,
            now: "2026-07-16T12:00:00Z".into(),
            nonce_counter: 0,
            last: None,
            previous: None,
            last_mandated: None,
            last_acme: None,
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

    fn state(
        authority: &str,
        with_did_json: bool,
        suspension: Suspension,
    ) -> (Arc<AppState>, Arc<MemDnsTxt>) {
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
        let (control, preloads) =
            ControlPlane::from_bootstrap_json(&bootstrap.to_string()).expect("fixture bootstrap");
        let objects = Arc::new(MemObjects::new());
        if with_did_json {
            for (tenant, did, bytes) in preloads {
                futures::executor::block_on(objects.put(&tenant, &did, "did.json", bytes));
            }
        }
        let dns = Arc::new(MemDnsTxt::new());
        let state = Arc::new(AppState {
            control,
            objects,
            nonces: Arc::new(MemNonces::new(600)),
            dns: dns.clone(),
            acme: AcmeState::new(),
            authority: authority.to_owned(),
            test_now_enabled: true,
        });
        (state, dns)
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
    let (state, dns) = StoreWorld::state(&world.authority, false, Suspension::None);
    world.state = state;
    world.dns = dns;
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
    let (state, dns) = StoreWorld::state(&world.authority, true, Suspension::Binding);
    world.state = state;
    world.dns = dns;
}

#[given(expr = "the tenant {string} is suspended")]
async fn tenant_suspended(world: &mut StoreWorld, tenant: String) {
    assert_eq!(tenant, fixtures().tenant, "fixture tenant");
    let (state, dns) = StoreWorld::state(&world.authority, true, Suspension::Tenant);
    world.state = state;
    world.dns = dns;
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
    StoreWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit("tests/features/store")
        .await;
}
