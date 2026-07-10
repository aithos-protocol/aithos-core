//! Conformance vector G2 — the mechanical rotation rule and up-link wrap
//! (spec 03.4). Independent Python generator.

use aithos_core::error::Error;
use aithos_core::header::{Header, KeyVersion, Line, Wrap};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct G2 {
    old_kids: Vec<String>,
    revoked_kid: String,
    expected_survivor_kids: Vec<String>,
    smuggled_new_kid: String,
    uplink: Value,
}

fn vector() -> G2 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/g2-rotation.json"
    )))
    .expect("valid vector json")
}

fn line(kid: &str) -> Line {
    Line {
        to: if kid == "owner-kex" {
            "owner".into()
        } else {
            kid.into()
        },
        kid: kid.into(),
        epk: "00".into(),
        n: "00".into(),
        c: "00".into(),
    }
}

fn header_with(kids: &[String]) -> Header {
    let mut kv = std::collections::BTreeMap::new();
    kv.insert(
        "1".to_owned(),
        KeyVersion {
            lines: kids.iter().map(|k| line(k)).collect(),
        },
    );
    Header {
        object: "header".into(),
        v: 1,
        node: "/e/circle/d/00000000000000000000000001".into(),
        key_versions: kv,
    }
}

#[test]
fn survivor_set_is_old_minus_revoked() {
    let v = vector();
    let survivors: Vec<String> = v
        .old_kids
        .iter()
        .filter(|k| **k != v.revoked_kid)
        .cloned()
        .collect();
    assert_eq!(survivors, v.expected_survivor_kids);
}

#[test]
fn a_smuggled_recipient_is_rejected() {
    let v = vector();
    // v2 lines = survivors + an intruder never present in v1.
    let mut header = header_with(&v.old_kids);
    let mut v2: Vec<Line> = v.expected_survivor_kids.iter().map(|k| line(k)).collect();
    v2.push(line(&v.smuggled_new_kid));
    header
        .key_versions
        .insert("2".to_owned(), KeyVersion { lines: v2 });
    assert!(matches!(
        header.check_rotation(2),
        Err(Error::GammaRevocationRejected(_))
    ));
}

#[test]
fn a_clean_rotation_is_accepted() {
    let v = vector();
    let mut header = header_with(&v.old_kids);
    let v2: Vec<Line> = v.expected_survivor_kids.iter().map(|k| line(k)).collect();
    header
        .key_versions
        .insert("2".to_owned(), KeyVersion { lines: v2 });
    header.check_rotation(2).unwrap();
}

#[test]
fn uplink_wrap_bytes_match_python() {
    let v = vector();
    let u = &v.uplink;
    let via_key: [u8; 32] = hex::decode(u["via_key_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let new_dk: [u8; 32] = hex::decode(u["new_dk_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let nonce: [u8; 24] = hex::decode(u["nonce_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let wrap = Wrap::seal(
        u["subject_did"].as_str().unwrap(),
        "/e/circle",
        &via_key,
        u["node"].as_str().unwrap(),
        u["key_version"].as_u64().unwrap(),
        &new_dk,
        nonce,
    );
    assert_eq!(wrap.c, u["cipher_hex"].as_str().unwrap());
    // And it round-trips back to DK' under the via key.
    let opened = wrap
        .open(u["subject_did"].as_str().unwrap(), &via_key)
        .unwrap();
    assert_eq!(opened, new_dk);
}
