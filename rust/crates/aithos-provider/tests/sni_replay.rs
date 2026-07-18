//! Byte-exact replay of `vectors/p5-tunnel-sni.json` against the REAL SNI
//! peek (`sni::peek_client_hello`), in process.
//!
//! Each case feeds the committed `hello_hex` bytes and asserts the exact
//! committed `decision` (and, when peeked, the exact lowercased `sni` and
//! ordered `alpn`). The ClientHellos were built by an independent Python
//! generator (RFC 8446 §4.1.2 wire layout, deterministic filler); this
//! proves the Rust peek reproduces every routing decision the vector pins
//! — the security-critical half of M2 (route without terminating, A3).
//!
//! The TLS termination on the tunnel door and the yamux passthrough are
//! deploy-gated plumbing proven by the cucumber relay harness over real
//! duplex sockets; the SNI authority is proven right here.

use aithos_provider::sni::{peek_client_hello, PeekDecision, PEEK_BOUND_BYTES};
use serde_json::Value;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors");

fn load(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{VECTORS}/{name}")).unwrap())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn p5_sni_peek_cases_replay_byte_exact() {
    let p5 = load("p5-tunnel-sni.json");
    assert_eq!(
        p5["peek_bound_bytes"].as_u64().unwrap() as usize,
        PEEK_BOUND_BYTES,
        "the vector's peek bound must equal the spec constant (B.4)"
    );

    let mut seen = Vec::new();
    for case in p5["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        seen.push(name.to_owned());
        let hello = hex::decode(case["hello_hex"].as_str().unwrap()).unwrap();
        let want = &case["expect"];
        let got = peek_client_hello(&hello);

        match want["decision"].as_str().unwrap() {
            "peeked" => {
                let PeekDecision::Peeked { sni, alpn } = got else {
                    panic!("{name}: expected peeked, got {got:?}");
                };
                assert_eq!(sni, want["sni"].as_str().unwrap(), "{name}: sni");
                let want_alpn: Vec<String> = want["alpn"]
                    .as_array()
                    .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
                    .unwrap_or_default();
                assert_eq!(alpn, want_alpn, "{name}: alpn");
            }
            "no_sni" => assert_eq!(got, PeekDecision::NoSni, "{name}"),
            "not_tls" => assert_eq!(got, PeekDecision::NotTls, "{name}"),
            "incomplete" => assert_eq!(got, PeekDecision::Incomplete, "{name}"),
            "too_large" => assert_eq!(got, PeekDecision::TooLarge, "{name}"),
            other => panic!("{name}: unknown decision {other}"),
        }
    }

    // Every routing decision of B.4 is exercised by the committed vector.
    for expected in [
        "peek_demo_hostname",
        "peek_mixed_case_is_lowercased",
        "peek_tunnel_door",
        "peek_fragmented_two_records",
        "no_sni_closes",
        "not_tls_closes",
        "truncated_is_incomplete",
        "hello_over_16kib_closes",
    ] {
        assert!(seen.iter().any(|n| n == expected), "p5 missing {expected}");
    }
    eprintln!("p5 SNI peek: {} cases replayed byte-exact", seen.len());
}
