//! Conformance vector G2 — the mechanical rotation rule and up-link wrap
//! (spec 03.4). Independent Python generator.

use aithos_core::error::Error;
use aithos_core::header::{owner_kid, Header, KeyVersion, Line, Recipient, Wrap};
use serde::Deserialize;
use serde_json::Value;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

/// G2 is a fixture of SHAPE: its kids are synthetic routing identities,
/// `zAGENT1` and `zAGENT2` included — none of the three is a real key. This is
/// that fixture's owner kid, not an assertion about the wire: §03.1 requires
/// `check_rotation` to be given the owner key it must find, and this test
/// gives it. The real wire form of the owner line — its `kid` carrying
/// `owner_kex` in multibase — is proven by C3 and by
/// `c3_owner_recipient_names_its_key_on_the_wire`, which is where it belongs.
///
/// Rewriting G2 to carry a real `owner_kex` was considered and deliberately
/// NOT done: the vector is pinned four levels deep (see the run report of
/// 2026-08-04-r1), and the cascade lies entirely outside `CHDR-007` /
/// `CHDR-012`. Do not reopen it without pricing that first.
const G2_OWNER_KID: &str = "owner-kex";

#[derive(Deserialize)]
struct G2 {
    old_kids: Vec<String>,
    revoked_kid: String,
    expected_survivor_kids: Vec<String>,
    smuggled_new_kid: String,
    /// CHDR-009: `vectors/g2-rotation.json:17` declares this normative case and
    /// the struct did not even deserialize it — the field had no consumer
    /// anywhere in the repository, while its sibling `smuggled_must_fail` was
    /// honoured by `a_smuggled_recipient_is_rejected`. A case specified by a
    /// vector and implemented nowhere is worse than an untested gate.
    missing_owner_must_fail: String,
    uplink: Value,
}

fn vector() -> G2 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/g2-rotation.json"
    )))
    .expect("valid vector json")
}

fn line(kid: &str, owner_kid: &str) -> Line {
    Line {
        to: if kid == owner_kid {
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

fn header_with(kids: &[String], owner_kid: &str) -> Header {
    let mut kv = std::collections::BTreeMap::new();
    kv.insert(
        "1".to_owned(),
        KeyVersion {
            lines: kids.iter().map(|k| line(k, owner_kid)).collect(),
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
    let mut header = header_with(&v.old_kids, G2_OWNER_KID);
    let mut v2: Vec<Line> = v
        .expected_survivor_kids
        .iter()
        .map(|k| line(k, G2_OWNER_KID))
        .collect();
    v2.push(line(&v.smuggled_new_kid, G2_OWNER_KID));
    header
        .key_versions
        .insert("2".to_owned(), KeyVersion { lines: v2 });
    assert!(matches!(
        header.check_rotation(2, G2_OWNER_KID),
        Err(Error::GammaRevocationRejected(_))
    ));
}

#[test]
fn a_clean_rotation_is_accepted() {
    let v = vector();
    let mut header = header_with(&v.old_kids, G2_OWNER_KID);
    let v2: Vec<Line> = v
        .expected_survivor_kids
        .iter()
        .map(|k| line(k, G2_OWNER_KID))
        .collect();
    header
        .key_versions
        .insert("2".to_owned(), KeyVersion { lines: v2 });
    header.check_rotation(2, G2_OWNER_KID).unwrap();
}

// --- CHDR-009: the fail-closed side of the three I3 gates no test reached ---
//
// Before this block, `Error::MissingOwnerLine` was asserted by NO test in the
// repository — not as a typed variant, not anywhere. The build gate was
// exercised only through a string match on "I3" in the Cucumber harness. The
// three gates below (`check_rotation`, `rotate`, `validate`) were executed by
// several call sites and never observed failing.

const DID: &str = "did:aithos:test-i3";
const NODE: &str = "/e/circle/d/00000000000000000000000001";
const DK1: [u8; 32] = [0x11; 32];
const DK2: [u8; 32] = [0x22; 32];

fn real_owner() -> (StaticSecret, XPublicKey) {
    let sk = StaticSecret::from([0x0au8; 32]);
    let pk = XPublicKey::from(&sk);
    (sk, pk)
}

fn real_grantee() -> Recipient {
    let sk = StaticSecret::from([0x21u8; 32]);
    Recipient {
        to: "g1".into(),
        kid: "g1".into(),
        pubkey: XPublicKey::from(&sk),
    }
}

/// The vector's `missing_owner_must_fail` case, finally consumed: a v2 whose
/// recipient set is a strict subset of v1 — so the smuggling gate is silent —
/// but which drops the owner. §03.4 requires the owner to be kept always.
#[test]
fn check_rotation_refuses_a_new_version_without_the_owner_line() {
    let v = vector();
    assert_eq!(
        v.missing_owner_must_fail, "MissingOwnerLine",
        "the vector names the variant this test must observe"
    );

    let mut header = header_with(&v.old_kids, G2_OWNER_KID);
    // Survivors minus the owner: every kid still comes from v1.
    let v2: Vec<Line> = v
        .expected_survivor_kids
        .iter()
        .filter(|k| *k != G2_OWNER_KID)
        .map(|k| line(k, G2_OWNER_KID))
        .collect();
    assert!(
        !v2.is_empty(),
        "the case must not degenerate into an empty version"
    );
    header
        .key_versions
        .insert("2".to_owned(), KeyVersion { lines: v2 });

    let outcome = header.check_rotation(2, G2_OWNER_KID);
    assert!(
        matches!(outcome, Err(Error::MissingOwnerLine(_))),
        "expected {}, got {outcome:?}",
        v.missing_owner_must_fail
    );
}

/// The same obligation at the WRITE gate: `rotate` must refuse to emit a
/// version whose survivor set omits the owner, so a writer can never produce a
/// header an edition verifier would reject (§00.2, §09.4).
#[test]
fn rotate_refuses_a_survivor_set_without_the_owner() {
    let (_owner_sk, owner_pub) = real_owner();
    let g1 = real_grantee();
    let mut header = Header::build(
        DID,
        NODE,
        &DK1,
        &owner_pub,
        &[Recipient::owner(owner_pub), g1.clone()],
        &[[0x41; 32], [0x42; 32]],
        &[[0x61; 24], [0x62; 24]],
    )
    .expect("the fixture header is valid");

    let outcome = header.rotate(
        DID,
        2,
        &DK2,
        &owner_pub,
        &[g1],
        &[[0x43; 32]],
        &[[0x63; 24]],
    );
    assert!(
        matches!(outcome, Err(Error::MissingOwnerLine(_))),
        "rotate must fail closed on a missing owner line, got {outcome:?}"
    );
    assert!(
        !header.key_versions.contains_key("2"),
        "a refused rotation must leave no partial version behind"
    );
}

/// And at the KEYLESS parse gate: `validate` must reject a header any one of
/// whose key versions lacks the owner line, naming the offending version.
#[test]
fn validate_refuses_a_key_version_without_the_owner_line() {
    let (_owner_sk, owner_pub) = real_owner();
    let g1 = real_grantee();
    let kid = owner_kid(&owner_pub);
    let mut header = Header::build(
        DID,
        NODE,
        &DK1,
        &owner_pub,
        &[Recipient::owner(owner_pub), g1.clone()],
        &[[0x41; 32], [0x42; 32]],
        &[[0x61; 24], [0x62; 24]],
    )
    .expect("the fixture header is valid");

    header
        .validate(&kid)
        .expect("the untouched header is valid");

    header.key_versions.insert(
        "2".to_owned(),
        KeyVersion {
            lines: vec![line("g1", &kid)],
        },
    );
    let outcome = header.validate(&kid);
    assert!(
        matches!(outcome, Err(Error::MissingOwnerLine(_))),
        "validate must reject a key version with no owner line, got {outcome:?}"
    );
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
