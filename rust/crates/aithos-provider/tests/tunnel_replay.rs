//! Byte-exact replay of `vectors/p3-tunnel-register.json` against the REAL
//! B.2 verification code (`tunnel::verify_registration`), in process.
//!
//! Each case sends the committed `line` bytes with the committed
//! `server_now` as the injected instant and the committed
//! `nonce_seen_before` as the reservation state; the verdict must match
//! the committed `expect` exactly. The control plane is built from the
//! vector's own `control_plane_mapping` — the fixture IS the contract.
//!
//! The relay's TLS/ALPN wrapper and yamux passthrough are deploy-gated
//! plumbing (P6 gate: a gateway behind NAT reachable, relay proven blind);
//! the security-critical registration authority is proven right here.

use std::sync::Arc;

use aithos_provider::control::{ControlPlane, TunnelBinding};
use aithos_provider::nonces::{MemNonces, NonceStore, Reservation};
use aithos_provider::time::parse_rfc3339z_ms;
use aithos_provider::tunnel::{answer, verify_registration};
use serde_json::Value;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");

fn load(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{VECTORS}/{name}")).unwrap())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A control plane bound exactly as the vector's mapping says, with the
/// per-case suspended flag applied. P7b: the B.2 step 4 joins the TENANT
/// state (B.5 authority), so the fixture plane names the mapping's tenant
/// as active too — a binding implies its enrolled tenant (the admin CLI
/// demands `create` before `bind-gateway`); the committed vector bytes
/// are untouched.
fn control_for(mapping: &Value, suspended: bool) -> ControlPlane {
    let mut plane = ControlPlane::default();
    let tenant = mapping["tenant"].as_str().unwrap().to_owned();
    plane.seed_tenant(&tenant, false);
    plane.bind_tunnel(
        mapping["gateway_pub"].as_str().unwrap().to_owned(),
        TunnelBinding {
            tenant,
            hostname: mapping["hostname"].as_str().unwrap().to_owned(),
            suspended,
        },
    );
    plane
}

#[tokio::test]
async fn p3_registration_cases_replay_byte_exact() {
    let p3 = load("p3-tunnel-register.json");
    let mapping = &p3["control_plane_mapping"];

    for case in p3["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mapping_gateway = mapping["gateway_pub"].as_str().unwrap();
        let case_gateway = case["registration"]["gateway_pub"].as_str().unwrap();

        // The exact wire line: JCS + LF, byte-for-byte from the vector.
        let line = case["line"].as_str().map(str::to_owned).unwrap_or_else(|| {
            // reject cases omit the pre-rendered `line`; render the exact
            // committed registration object as JCS + LF ourselves.
            format!("{}\n", serde_jcs::to_string(&case["registration"]).unwrap())
        });
        let now_ms = parse_rfc3339z_ms(case["server_now"].as_str().unwrap()).unwrap();
        let suspended = case["suspended"].as_bool().unwrap_or(false);
        let control = control_for(mapping, suspended);

        // The reservation state is a committed input: a replayed nonce is
        // pre-seeded so the reservation returns Replayed on this sight.
        let nonces = MemNonces::new(600);
        if case["nonce_seen_before"].as_bool().unwrap_or(false) {
            // Burn the exact (gateway_pub, nonce) once, before the case.
            let nonce = case["registration"]["nonce"].as_str().unwrap();
            assert_eq!(
                nonces
                    .reserve(case_gateway, nonce, now_ms - 1)
                    .await
                    .unwrap(),
                Reservation::Fresh
            );
        }
        let nonces_dyn: Arc<dyn NonceStore> = Arc::new(nonces);

        let got = verify_registration(line.as_bytes(), &control, nonces_dyn.as_ref(), now_ms).await;

        let want_ok = case["expect"]["ok"].as_bool().unwrap();
        if want_ok {
            let accepted =
                got.unwrap_or_else(|e| panic!("{name}: expected ok, got {:?}", e.code()));
            assert_eq!(accepted.gateway_pub, case_gateway);
            // The happy-path answer is the exact single line of B.2.
            assert_eq!(
                answer(&Ok(accepted)),
                r#"{"aithos-tunnel":"1.0.0-draft.1","ok":true}"#
            );
            // Sanity: the accepted gateway is the mapped one.
            assert_eq!(case_gateway, mapping_gateway);
        } else {
            let want = case["expect"]["error"].as_str().unwrap();
            let refusal = got.expect_err(&format!("{name}: expected refusal {want}"));
            assert_eq!(refusal.code(), want, "{name}: wrong code");
            // The refusal answer never leaks anything but the code.
            let line = answer(&Err(refusal));
            let parsed: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(parsed["ok"], Value::Bool(false));
            assert_eq!(parsed["error"], want);
            assert_eq!(parsed.as_object().unwrap().len(), 2);
        }
    }

    // Every case name of the committed vector was exercised.
    let names: Vec<&str> = p3["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for expected in [
        "register_ok",
        "reject_mapping_mismatch",
        "reject_signature_invalid",
        "reject_clock_skew",
        "reject_nonce_replayed",
        "reject_suspended",
    ] {
        assert!(names.contains(&expected), "p3 missing case {expected}");
    }
    eprintln!(
        "p3 tunnel registration: {} cases replayed byte-exact",
        names.len()
    );
}

/// The `reject_signature_invalid` case flips the last signature byte of an
/// otherwise valid line: the registration object round-trips to the exact
/// committed bytes (proves our JCS matches the independent Python generator).
#[test]
fn committed_lines_are_canonical_jcs() {
    let p3 = load("p3-tunnel-register.json");
    for case in p3["cases"].as_array().unwrap() {
        if let Some(line) = case["line"].as_str() {
            let body = line.strip_suffix('\n').unwrap();
            let reparsed: Value = serde_json::from_str(body).unwrap();
            assert_eq!(
                serde_jcs::to_string(&reparsed).unwrap(),
                body,
                "{}: committed line is not canonical JCS",
                case["name"]
            );
        }
    }
}
