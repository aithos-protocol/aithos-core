//! Conformance vectors C1 (header line seal/open) and C2 (wrap) — spec 03.8.
//! Expected ciphertexts generated independently (Python PyNaCl + manual
//! RFC 5869 HKDF); all randomness fixed as inputs.

use aithos_core::seal::{line_aad, open_line, seal_line, wrap_aad, wrap_open, wrap_seal};
use serde::Deserialize;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

#[derive(Deserialize)]
struct LineVec {
    esk_hex: String,
    epk_hex: String,
    n_hex: String,
    c_hex: String,
}

#[derive(Deserialize)]
struct WrapVec {
    via_key_hex: String,
    wrapped_node: String,
    key_version: u64,
    dk_hex: String,
    n_hex: String,
    c_hex: String,
}

#[derive(Deserialize)]
struct C1 {
    subject_did: String,
    node: String,
    key_version: u64,
    dk_hex: String,
    owner_kex_sk_hex: String,
    grantee_sk_hex: String,
    owner_line: LineVec,
    grantee_line: LineVec,
    wrap: WrapVec,
}

fn vector() -> C1 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/c1-header-seal.json"
    ));
    serde_json::from_str(raw).expect("vector c1-header-seal.json parses")
}

fn b32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
fn b24(s: &str) -> [u8; 24] {
    hex::decode(s).unwrap().try_into().unwrap()
}

fn check_line(v: &C1, line: &LineVec, recipient_sk_hex: &str) {
    let aad = line_aad(&v.subject_did, &v.node, v.key_version);
    let sk = StaticSecret::from(b32(recipient_sk_hex));
    let (epk, c) = seal_line(
        &StaticSecret::from(b32(&line.esk_hex)),
        &XPublicKey::from(&sk),
        &b32(&v.dk_hex),
        &b24(&line.n_hex),
        &aad,
    );
    assert_eq!(hex::encode(epk), line.epk_hex, "epk cross-check vs Python");
    assert_eq!(
        hex::encode(&c),
        line.c_hex,
        "ciphertext cross-check vs Python"
    );

    let opened = open_line(&sk, &epk, &c, &b24(&line.n_hex), &aad).unwrap();
    assert_eq!(hex::encode(opened), v.dk_hex);
}

#[test]
fn c1_owner_and_grantee_lines() {
    let v = vector();
    check_line(&v, &v.owner_line, &v.owner_kex_sk_hex);
    check_line(&v, &v.grantee_line, &v.grantee_sk_hex);
}

#[test]
fn c1_fail_closed() {
    let v = vector();
    let aad = line_aad(&v.subject_did, &v.node, v.key_version);
    let sk = StaticSecret::from(b32(&v.owner_kex_sk_hex));
    let epk = b32(&v.owner_line.epk_hex);
    let n = b24(&v.owner_line.n_hex);
    let mut c = hex::decode(&v.owner_line.c_hex).unwrap();

    // corrupted byte
    c[5] ^= 1;
    assert!(open_line(&sk, &epk, &c, &n, &aad).is_err());
    c[5] ^= 1;

    // wrong recipient
    let stranger = StaticSecret::from([0x99u8; 32]);
    assert!(open_line(&stranger, &epk, &c, &n, &aad).is_err());

    // replayed on another node (AAD mismatch)
    let other_aad = line_aad(&v.subject_did, "/e/self", v.key_version);
    assert!(open_line(&sk, &epk, &c, &n, &other_aad).is_err());

    // wrong key version
    let other_ver = line_aad(&v.subject_did, &v.node, v.key_version + 1);
    assert!(open_line(&sk, &epk, &c, &n, &other_ver).is_err());
}

#[test]
fn c2_wrap_roundtrip_and_cross_check() {
    let v = vector();
    let w = &v.wrap;
    let aad = wrap_aad(&v.subject_did, &w.wrapped_node, w.key_version);
    let c = wrap_seal(&b32(&w.via_key_hex), &b32(&w.dk_hex), &b24(&w.n_hex), &aad);
    assert_eq!(hex::encode(&c), w.c_hex, "wrap cross-check vs Python");
    assert_eq!(
        hex::encode(wrap_open(&b32(&w.via_key_hex), &c, &b24(&w.n_hex), &aad).unwrap()),
        w.dk_hex
    );
    // wrong via key fails closed
    assert!(wrap_open(&[0u8; 32], &c, &b24(&w.n_hex), &aad).is_err());
}
