//! BDD acceptance harness for `tests/features/tunnel/tunnel-register.feature`
//! — the annexe B.2 verification order, case by case, against the REAL
//! `tunnel::verify_registration`. Fixtures are the committed `p3` vector:
//! the gateway key re-derives from `gateway_sk_hex`, the mapping is the
//! vector's `control_plane_mapping`. Instants are injected (`server_now`
//! is an input — the B.2 replayability property), never wall-clock.

use std::sync::{Arc, OnceLock};

use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::{MemNonces, NonceStore};
use aithos_provider::tunnel::{
    answer, registration_line, sign_registration, verify_registration, Accepted, Registration,
    RegistrationSignature, TunnelRefusal, TUNNEL_WIRE_VERSION,
};
use cucumber::{given, then, when, World as _};
use ed25519_dalek::SigningKey;

// ------------------------------------------------------------- fixtures

struct Fixtures {
    gateway_sk: SigningKey,
    gateway_pub: String,
    tenant: String,
    hostname: String,
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let p3: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p3-tunnel-register.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let sk = SigningKey::from_bytes(
            &hex::decode(p3["gateway_sk_hex"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let m = &p3["control_plane_mapping"];
        Fixtures {
            gateway_pub: aithos_core::wire::ed25519_pub_to_multibase(
                &sk.verifying_key().to_bytes(),
            ),
            gateway_sk: sk,
            tenant: m["tenant"].as_str().unwrap().to_owned(),
            hostname: m["hostname"].as_str().unwrap().to_owned(),
        }
    })
}

// --------------------------------------------------------------- world

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct TunnelWorld {
    control: ControlPlane,
    now: String,
    nonce_counter: u64,
    last: Option<Result<Accepted, TunnelRefusal>>,
    previous: Option<Result<Accepted, TunnelRefusal>>,
    answer_line: Option<String>,
}

impl std::fmt::Debug for TunnelWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TunnelWorld")
    }
}

impl TunnelWorld {
    fn new() -> Self {
        Self {
            control: ControlPlane::default(),
            now: "2026-07-16T12:00:00Z".into(),
            nonce_counter: 0,
            last: None,
            previous: None,
            answer_line: None,
        }
    }

    fn fresh_nonce(&mut self) -> String {
        self.nonce_counter += 1;
        format!("bdd-t-{:010}", self.nonce_counter)
    }

    fn now_ms(&self) -> i64 {
        aithos_provider::time::parse_rfc3339z_ms(&self.now).expect("test instant")
    }

    /// A registration for the fixture gateway, valid unless a step mutates
    /// it. `at`/`nonce` default to the world clock and a fresh nonce.
    fn reg(&mut self, hostname: &str, at: Option<&str>, nonce: Option<&str>) -> Registration {
        let f = fixtures();
        let reg = Registration {
            version: TUNNEL_WIRE_VERSION.into(),
            tenant: f.tenant.clone(),
            hostname: hostname.to_owned(),
            gateway_pub: f.gateway_pub.clone(),
            at: at.unwrap_or(&self.now).to_owned(),
            nonce: nonce
                .map(str::to_owned)
                .unwrap_or_else(|| self.fresh_nonce()),
            signature: RegistrationSignature {
                alg: "ed25519".into(),
                value: String::new(),
            },
        };
        sign_registration(reg, &f.gateway_sk.clone())
    }

    async fn fire(&mut self, line: &[u8]) {
        let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
        self.fire_with(line, nonces.as_ref()).await;
    }

    async fn fire_with(&mut self, line: &[u8], nonces: &dyn NonceStore) {
        let result = verify_registration(line, &self.control, nonces, self.now_ms()).await;
        self.answer_line = Some(answer(&result));
        self.previous = self.last.take();
        self.last = Some(result);
    }

    fn refused(&self, code: &str) {
        match self.last.as_ref().expect("a registration was fired") {
            Err(refusal) => assert_eq!(refusal.code(), code, "wrong refusal code"),
            Ok(_) => panic!("expected refusal {code}, got acceptance"),
        }
    }
}

// ---------------------------------------------------------- background

#[given(expr = "the control plane binds gateway {string} to tenant {string} and hostname {string}")]
async fn bind(world: &mut TunnelWorld, gateway_pub: String, tenant: String, hostname: String) {
    // The scenario names the vector's gateway; the fixture key must match.
    assert_eq!(gateway_pub, fixtures().gateway_pub, "fixture gateway key");
    // P7b: the B.2 step 4 joins the tenant state — a bound tunnel names
    // its enrolled tenant (the CLI demands `create` before `bind-gateway`).
    world.control.seed_tenant(&tenant, false);
    world.control.bind_tunnel(
        gateway_pub,
        TunnelBinding {
            tenant,
            hostname,
            suspended: false,
        },
    );
}

#[given("the control-plane binding is suspended")]
async fn suspend(world: &mut TunnelWorld) {
    let f = fixtures();
    // Binding-level suspension precedes the tenant join (B.2 order): the
    // tenant stays active, the binding's own flag refuses.
    world.control.seed_tenant(&f.tenant, false);
    world.control.bind_tunnel(
        f.gateway_pub.clone(),
        TunnelBinding {
            tenant: f.tenant.clone(),
            hostname: f.hostname.clone(),
            suspended: true,
        },
    );
}

#[given(expr = "the relay clock reads {string}")]
async fn clock(world: &mut TunnelWorld, now: String) {
    world.now = now;
}

// -------------------------------------------------------------- whens

#[when(expr = "a registration arrives carrying an extra field {string}")]
async fn extra_field(world: &mut TunnelWorld, field: String) {
    let reg = world.reg(&fixtures().hostname.clone(), None, None);
    let mut value: serde_json::Value =
        serde_json::from_str(&serde_jcs::to_string(&reg).unwrap()).unwrap();
    value[field] = serde_json::json!("x");
    let line = format!("{}\n", serde_jcs::to_string(&value).unwrap());
    world.fire(line.as_bytes()).await;
}

#[when("a registration arrives re-encoded with spaces between JSON tokens")]
async fn non_canonical(world: &mut TunnelWorld) {
    let reg = world.reg(&fixtures().hostname.clone(), None, None);
    let value: serde_json::Value =
        serde_json::from_str(&serde_jcs::to_string(&reg).unwrap()).unwrap();
    let line = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    world.fire(line.as_bytes()).await;
}

#[when(expr = "a registration arrives claiming version {string}")]
async fn wrong_version(world: &mut TunnelWorld, version: String) {
    let mut reg = world.reg(&fixtures().hostname.clone(), None, None);
    reg.version = version;
    let reg = sign_registration(reg, &fixtures().gateway_sk.clone());
    world.fire(registration_line(&reg).as_bytes()).await;
}

#[when(expr = "a well-formed registration signed at {string} arrives")]
async fn signed_at(world: &mut TunnelWorld, at: String) {
    let reg = world.reg(&fixtures().hostname.clone(), Some(&at), None);
    world.fire(registration_line(&reg).as_bytes()).await;
}

#[when("a valid registration is presented twice")]
async fn presented_twice(world: &mut TunnelWorld) {
    let reg = world.reg(&fixtures().hostname.clone(), None, Some("bdd-t-replay-01"));
    let line = registration_line(&reg);
    // One shared nonce store across both presentations.
    let nonces: Arc<dyn NonceStore> = Arc::new(MemNonces::new(600));
    world.fire_with(line.as_bytes(), nonces.as_ref()).await;
    assert!(world.last.as_ref().unwrap().is_ok(), "first is accepted");
    world.fire_with(line.as_bytes(), nonces.as_ref()).await;
}

#[when("a registration arrives with its signature corrupted")]
async fn corrupted(world: &mut TunnelWorld) {
    let mut reg = world.reg(&fixtures().hostname.clone(), None, None);
    let v = &mut reg.signature.value;
    let tail = if v.ends_with("00") { "ff" } else { "00" };
    v.truncate(v.len() - 2);
    v.push_str(tail);
    world.fire(registration_line(&reg).as_bytes()).await;
}

#[when(expr = "a valid registration claims hostname {string}")]
async fn claims_hostname(world: &mut TunnelWorld, hostname: String) {
    let reg = world.reg(&hostname, None, None);
    world.fire(registration_line(&reg).as_bytes()).await;
}

#[when("a valid registration is signed by an unmapped gateway key")]
async fn unmapped_gateway(world: &mut TunnelWorld) {
    let stranger = SigningKey::from_bytes(&[0x33; 32]);
    let pub_mb = aithos_core::wire::ed25519_pub_to_multibase(&stranger.verifying_key().to_bytes());
    let f = fixtures();
    let reg = Registration {
        version: TUNNEL_WIRE_VERSION.into(),
        tenant: f.tenant.clone(),
        hostname: f.hostname.clone(),
        gateway_pub: pub_mb,
        at: world.now.clone(),
        nonce: world.fresh_nonce(),
        signature: RegistrationSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let reg = sign_registration(reg, &stranger);
    world.fire(registration_line(&reg).as_bytes()).await;
}

#[when("a valid registration arrives")]
async fn valid(world: &mut TunnelWorld) {
    let reg = world.reg(&fixtures().hostname.clone(), None, None);
    world.fire(registration_line(&reg).as_bytes()).await;
}

#[when(expr = "a {int}-byte registration line arrives")]
async fn oversized(world: &mut TunnelWorld, size: usize) {
    world.fire(vec![b'{'; size].as_slice()).await;
}

// -------------------------------------------------------------- thens

#[then("the registration is accepted")]
async fn accepted(world: &mut TunnelWorld) {
    assert!(
        world.last.as_ref().expect("fired").is_ok(),
        "expected acceptance, got {:?}",
        world
            .last
            .as_ref()
            .unwrap()
            .as_ref()
            .err()
            .map(|e| e.code())
    );
}

#[then(expr = "the registration is refused with {string}")]
async fn refused(world: &mut TunnelWorld, code: String) {
    world.refused(&code);
}

#[then(expr = "the second registration is refused with {string}")]
async fn second_refused(world: &mut TunnelWorld, code: String) {
    assert!(
        world.previous.as_ref().expect("two fires").is_ok(),
        "first presentation was accepted"
    );
    world.refused(&code);
}

#[then(expr = "the relay answer is the single line {word}")]
async fn answer_line(world: &mut TunnelWorld, _word: String) {
    // The step's remaining text is the literal JSON; compare against the
    // canonical happy-path answer regardless of how cucumber tokenized it.
    let got = world.answer_line.as_ref().expect("an answer");
    assert_eq!(got, r#"{"aithos-tunnel":"1.0.0-draft.1","ok":true}"#);
}

#[tokio::main]
async fn main() {
    TunnelWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit("tests/features/tunnel")
        .await;
}
