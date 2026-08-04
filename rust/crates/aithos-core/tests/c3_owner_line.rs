//! Conformance vector C3 (I3 owner line, spec §03.1) — header tier.
//!
//! The owner line of a key version is the line whose recipient key is the
//! subject's `owner_kex`, as published in the DID document. The routing
//! label `to` establishes nothing in either direction (spec §00.2 I3,
//! §03.1, §03.2). Expected values generated independently by
//! `vectors/gen-c.py` (Python blake3 + PyNaCl + manual RFC 5869 HKDF +
//! base58); the positive owner line is byte-identical to
//! `vectors/c1-header-seal.json`'s `owner_line`.
//!
//! The edition tier of the same vector lives in
//! `aithos-bundle/tests/c3_owner_line_edition.rs` (I3's second half).

use aithos_core::header::{owner_kid, Header, Recipient};
use aithos_core::seal::line_aad;
use aithos_core::wire;
use serde::Deserialize;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

#[derive(Deserialize)]
struct Case {
    name: String,
    verdict: String,
    tier: String,
    header: Header,
}

#[derive(Deserialize)]
struct C3 {
    subject_did: String,
    node: String,
    key_version: u64,
    dk_hex: String,
    owner_kex_sk_hex: String,
    owner_kex_pub_hex: String,
    owner_kex_pub_multibase: String,
    stranger_multibase: String,
    line_aad_hex: String,
    cases: Vec<Case>,
}

fn vector() -> C3 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/c3-owner-line.json"
    ));
    serde_json::from_str(raw).expect("vector c3-owner-line.json parses")
}

fn b32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}
fn b24(s: &str) -> [u8; 24] {
    hex::decode(s).unwrap().try_into().unwrap()
}

fn case<'a>(v: &'a C3, name: &str) -> &'a Case {
    v.cases
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("vector C3 has no case {name}"))
}

/// The vector's own AAD, cross-checked against the Rust codec: everything
/// below is bound to this exact `subject_did ‖ node ‖ key_version`.
#[test]
fn c3_line_aad_cross_check() {
    let v = vector();
    assert_eq!(
        hex::encode(line_aad(&v.subject_did, &v.node, v.key_version)),
        v.line_aad_hex,
        "C3 line AAD cross-check vs Python"
    );
    assert_eq!(
        wire::x25519_pub_to_multibase(&b32(&v.owner_kex_pub_hex)),
        v.owner_kex_pub_multibase,
        "the vector's owner_kex multibase is the Rust codec's"
    );
}

/// CHDR-012, variant A on the wire: the owner line names its key. Its `kid`
/// carries `owner_kex` in multibase, exactly as a grantee's line carries the
/// grantee's key (spec §03.1: "`kid` names the line's recipient key … or —
/// for the owner line — the subject's `owner_kex` in multibase").
#[test]
fn c3_owner_recipient_names_its_key_on_the_wire() {
    let v = vector();
    let owner_pub = XPublicKey::from(b32(&v.owner_kex_pub_hex));
    assert_eq!(
        Recipient::owner(owner_pub).kid,
        v.owner_kex_pub_multibase,
        "Recipient::owner must name owner_kex, not a fixed label"
    );
}

/// The same fact proven on the sealed bytes: building the positive case's
/// owner line from C1's ephemeral and nonce must reproduce the vector's line
/// field for field. Only `kid` can differ — the seal itself is unchanged by
/// variant A, `kid` being absent from the line AAD.
#[test]
fn c3_positive_owner_line_is_byte_exact() {
    let v = vector();
    let expected = &case(&v, "owner_line_present").header.key_versions["1"].lines[0];
    let owner_pub = XPublicKey::from(b32(&v.owner_kex_pub_hex));
    // C1's owner-line ephemeral; the nonce is the vector's own.
    let esk = b32("78797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f9091929394959697");
    let built = Header::build(
        &v.subject_did,
        &v.node,
        &b32(&v.dk_hex),
        &owner_pub,
        &[Recipient::owner(owner_pub)],
        &[esk],
        &[b24(&expected.n)],
    )
    .expect("build the positive owner line");
    assert_eq!(
        &built.key_versions["1"].lines[0], expected,
        "the built owner line must equal C3's owner_line_present line 0"
    );
}

/// CHDR-012, keyless tier: `Header::validate` must decide I3 on the recipient
/// key declared by `kid`, never on `to`. Every C3 case is consumed here.
/// A case whose tier is `owner_kex` is one a keyless verifier accepts by
/// design — the documented residual boundary of spec §03.1 — so its keyless
/// expectation is "accepted" even though its overall verdict is invalid.
#[test]
fn c3_keyless_i3_verdicts() {
    let v = vector();
    let mut mismatches: Vec<String> = Vec::new();
    for c in &v.cases {
        let keyless_accepts = c.verdict == "valid" || c.tier == "owner_kex";
        match (
            keyless_accepts,
            c.header.validate(&v.owner_kex_pub_multibase),
        ) {
            (true, Err(e)) => mismatches.push(format!(
                "{}: keyless I3 must accept (verdict={}, tier={}), rejected: {e}",
                c.name, c.verdict, c.tier
            )),
            (false, Ok(())) => mismatches.push(format!(
                "{}: keyless I3 must reject (verdict={}, tier={}), accepted",
                c.name, c.verdict, c.tier
            )),
            _ => {}
        }
    }
    assert_eq!(mismatches, Vec::<String>::new(), "C3 keyless I3 verdicts");
}

/// The `owner_kex`-BEARING tier of spec §03.1 — the half no keyless verifier
/// can reach. Each of the five cases gets its full verdict here: a verifier
/// holding `owner_kex` accepts exactly the two positives and rejects all
/// three negatives, including `owner_label_foreign_seal`, whose line declares
/// `owner_kex` but is sealed to a stranger. That case is precisely why I3
/// cannot be settled by any string comparison alone.
#[test]
fn c3_owner_kex_tier_verdicts() {
    let v = vector();
    let sk = StaticSecret::from(b32(&v.owner_kex_sk_hex));
    assert_eq!(
        owner_kid(&XPublicKey::from(&sk)),
        v.owner_kex_pub_multibase,
        "the tier derives the kid from the key it holds"
    );
    let mut mismatches: Vec<String> = Vec::new();
    for c in &v.cases {
        match (
            c.verdict == "valid",
            c.header.validate_as_owner(&v.subject_did, &sk),
        ) {
            (true, Err(e)) => mismatches.push(format!(
                "{}: the owner_kex tier must accept (tier={}), rejected: {e}",
                c.name, c.tier
            )),
            (false, Ok(())) => mismatches.push(format!(
                "{}: the owner_kex tier must reject (tier={}), accepted",
                c.name, c.tier
            )),
            _ => {}
        }
    }
    assert_eq!(mismatches, Vec::<String>::new(), "C3 owner_kex I3 verdicts");
}

/// The seal side of the same five cases: which line actually opens under
/// `owner_kex`. This is what makes the label's irrelevance a cryptographic
/// fact and not a naming convention — and what the `owner_kex` tier of
/// spec §03.1 must additionally check.
#[test]
fn c3_owner_kex_opens_exactly_the_owner_lines() {
    let v = vector();
    let sk = StaticSecret::from(b32(&v.owner_kex_sk_hex));
    let dk = b32(&v.dk_hex);
    let kid = &v.owner_kex_pub_multibase;

    // Positive 1: the labelled owner line opens under owner_kex.
    assert_eq!(
        case(&v, "owner_line_present")
            .header
            .open(&v.subject_did, v.key_version, kid, &sk)
            .expect("owner_line_present opens under owner_kex"),
        dk
    );
    // Positive 2: so does the line whose `to` names the stranger — the label
    // decides nothing in either direction.
    assert_eq!(
        case(&v, "unlabelled_owner_line")
            .header
            .open(&v.subject_did, v.key_version, kid, &sk)
            .expect("unlabelled_owner_line opens under owner_kex"),
        dk
    );
    // Negative 1: no line at all for owner_kex.
    assert!(case(&v, "no_owner_line_at_all")
        .header
        .open(&v.subject_did, v.key_version, kid, &sk)
        .is_err());
    // Negative 2: the line labelled "owner" declares the stranger's key.
    assert!(case(&v, "owner_label_foreign_key")
        .header
        .open(&v.subject_did, v.key_version, kid, &sk)
        .is_err());
    // Negative 3: it declares owner_kex but is sealed to the stranger — the
    // case only an owner_kex-bearing verifier can catch.
    assert!(case(&v, "owner_label_foreign_seal")
        .header
        .open(&v.subject_did, v.key_version, kid, &sk)
        .is_err());
    // And the stranger's own kid is present exactly where the vector says.
    assert!(case(&v, "no_owner_line_at_all").header.key_versions["1"]
        .lines
        .iter()
        .any(|l| l.kid == v.stranger_multibase));
}
