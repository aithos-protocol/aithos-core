//! Route and object grammar of annexe A.1 (check #0) and the `covers()`
//! path-map of annexe A.3 (check #10).
//!
//! **Fail-closed, zero interpretation:** any request-target outside the
//! closed grammar answers `path_invalid` before the envelope is even
//! looked at. The grammar never percent-decodes anything — the DID travels
//! literally, and `%` anywhere is a rejection. `covers()` is an
//! **anti-abuse availability gate, never the authority** (§3.1): in P1 it
//! knows two principals — the anonymous reader of the A2 exceptions and
//! the owner, who covers everything on their own DID. Mandated perimeters
//! arrive with the P2 chain machinery.

use aithos_core::ids::Sid;

/// The `<chemin>` grammar of annexe A.1 — layout §02.3, closed set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectPath {
    Manifest,
    DidJson,
    /// `e/public/**` — plaintext named tree.
    Public(Vec<String>),
    /// `e/<zone>/index.json`, zone ∈ {circle, self}.
    ZoneIndex(String),
    /// `e/<zone>/blobs/<sid>.enc`.
    Blob(String, Sid),
    /// `e/<zone>/hdr/<node>.json` — node naming refined with P2 (the P1
    /// grammar bounds the charset; no vector exercises hdr yet).
    Hdr(String, String),
    /// `x/<id>/…` — vault subtree (§08).
    X(String, Vec<String>),
    /// `certs/<mandate_id>.json`.
    Cert(String),
    /// `gamma/<YYYY-MM>.jsonl`.
    GammaSegment(String),
}

impl ObjectPath {
    /// The relative storage key — exactly the `<chemin>` bytes.
    pub fn key(&self) -> String {
        match self {
            ObjectPath::Manifest => "manifest.json".into(),
            ObjectPath::DidJson => "did.json".into(),
            ObjectPath::Public(segs) => format!("e/public/{}", segs.join("/")),
            ObjectPath::ZoneIndex(z) => format!("e/{z}/index.json"),
            ObjectPath::Blob(z, sid) => format!("e/{z}/blobs/{sid}.enc"),
            ObjectPath::Hdr(z, node) => format!("e/{z}/hdr/{node}.json"),
            ObjectPath::X(id, segs) if segs.is_empty() => format!("x/{id}"),
            ObjectPath::X(id, segs) => format!("x/{id}/{}", segs.join("/")),
            ObjectPath::Cert(id) => format!("certs/{id}.json"),
            ObjectPath::GammaSegment(m) => format!("gamma/{m}.jsonl"),
        }
    }
}

/// A parsed data-plane request target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTarget {
    pub tenant: String,
    pub did: String,
    pub kind: TargetKind,
}

/// What the target addresses. `Heads`/`Batch`/`Gamma`/`Sync`/`List` are
/// grammar-valid routes of annexe A.3 that P1 does not serve yet — the
/// service answers them `501 not_implemented` (P1-transitional, outside
/// the wire registry; P2 wires them for real).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetKind {
    Object(ObjectPath),
    Heads,
    Batch,
    Gamma,
    Sync,
    List,
}

/// Parse a raw origin-form request target (`path[?query]`, byte-exact as
/// received). `None` = `path_invalid`.
pub fn parse_target(raw: &str) -> Option<DataTarget> {
    // Never percent-decoded, never re-encoded (A.1): reject the escape
    // character itself, control bytes, and backslashes outright.
    if raw.contains('%') || raw.contains('\\') || raw.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return None;
    }
    let (path, query) = match raw.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (raw, None),
    };
    let rest = path.strip_prefix("/t/")?;
    let (tenant, rest) = rest.split_once('/')?;
    if !valid_tenant(tenant) {
        return None;
    }
    // `<did>` is literal; everything after it is the chemin or a route.
    // The DID contains no '/', so the next segment boundary is unambiguous.
    let (did, chemin) = match rest.split_once('/') {
        Some((did, chemin)) => (did, Some(chemin)),
        None => (rest, None),
    };
    if !valid_did(did) {
        return None;
    }
    let kind = match (chemin, query) {
        // `GET /t/<t>/<did>?list=<prefix>` — the listing route (P2).
        (None, Some(q)) if q.starts_with("list=") => TargetKind::List,
        (None, _) => return None,
        (Some(_), Some(_)) => return None, // no other route carries a query
        (Some("heads"), None) => TargetKind::Heads,
        (Some("batch"), None) => TargetKind::Batch,
        (Some("gamma"), None) => TargetKind::Gamma,
        (Some("sync"), None) => TargetKind::Sync,
        (Some(chemin), None) => TargetKind::Object(parse_chemin(chemin)?),
    };
    Some(DataTarget {
        tenant: tenant.to_owned(),
        did: did.to_owned(),
        kind,
    })
}

/// `tenant` = `[a-z0-9][a-z0-9-]{2,31}` (annexe A.1).
fn valid_tenant(t: &str) -> bool {
    let b = t.as_bytes();
    (3..=32).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// `did:aithos:<multibase ed25519>` — decoded, never trusted on shape alone.
fn valid_did(did: &str) -> bool {
    did.strip_prefix("did:aithos:")
        .is_some_and(|mb| aithos_core::wire::multibase_to_ed25519_pub(mb).is_ok())
}

fn parse_chemin(chemin: &str) -> Option<ObjectPath> {
    let segs: Vec<&str> = chemin.split('/').collect();
    match segs.as_slice() {
        ["manifest.json"] => Some(ObjectPath::Manifest),
        ["did.json"] => Some(ObjectPath::DidJson),
        ["e", "public", rest @ ..] => {
            valid_file_segments(rest).then(|| ObjectPath::Public(own(rest)))
        }
        ["e", zone, "index.json"] if valid_sealed_zone(zone) => {
            Some(ObjectPath::ZoneIndex((*zone).to_owned()))
        }
        ["e", zone, "blobs", file] if valid_sealed_zone(zone) => {
            let sid = Sid::parse(file.strip_suffix(".enc")?).ok()?;
            Some(ObjectPath::Blob((*zone).to_owned(), sid))
        }
        ["e", zone, "hdr", file] if valid_sealed_zone(zone) => {
            let node = file.strip_suffix(".json")?;
            valid_hdr_node(node).then(|| ObjectPath::Hdr((*zone).to_owned(), node.to_owned()))
        }
        ["x", id, rest @ ..] => (aithos_core::ids::validate_name(id).is_ok()
            && (rest.is_empty() || valid_file_segments(rest)))
        .then(|| ObjectPath::X((*id).to_owned(), own(rest))),
        ["certs", file] => {
            let id = file.strip_suffix(".json")?;
            valid_mandate_id(id).then(|| ObjectPath::Cert(id.to_owned()))
        }
        ["gamma", file] => {
            let month = file.strip_suffix(".jsonl")?;
            valid_month(month).then(|| ObjectPath::GammaSegment(month.to_owned()))
        }
        _ => None,
    }
}

fn own(segs: &[&str]) -> Vec<String> {
    segs.iter().map(|s| (*s).to_owned()).collect()
}

/// Encrypted zones of layout §02.3. `public` has no index/blobs/hdr.
fn valid_sealed_zone(z: &str) -> bool {
    z == "circle" || z == "self"
}

/// Free segments (`e/public/**`, `x/<id>/…`): lowercase names with dots for
/// filenames — never empty, never dot-led (kills `.`/`..` and dotfiles),
/// bounded depth and length. Conservative by design: widening is additive,
/// narrowing would be a wire break.
fn valid_file_segments(segs: &[&str]) -> bool {
    !segs.is_empty()
        && segs.len() <= 8
        && segs.iter().all(|s| {
            let b = s.as_bytes();
            (1..=64).contains(&b.len())
                && b[0] != b'.'
                && b.iter().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
                })
        })
}

/// Header node names are sid-addressed (§02.3); the exact encoding is
/// exercised by P2 vectors. Bounded ULID-alphabet-plus-separators until then.
fn valid_hdr_node(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=200).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// `mandate_` + 26-char ULID field (§04).
fn valid_mandate_id(s: &str) -> bool {
    s.strip_prefix("mandate_").is_some_and(|u| {
        u.len() == 26
            && u.bytes().all(|c| {
                c.is_ascii_digit()
                    || (c.is_ascii_uppercase() && !matches!(c, b'I' | b'L' | b'O' | b'U'))
            })
    })
}

fn valid_month(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && matches!(
            &s[5..7],
            "01" | "02" | "03" | "04" | "05" | "06" | "07" | "08" | "09" | "10" | "11" | "12"
        )
}

/// Anonymous perimeter — the A2 exceptions (annexe A.2): `e/public/**` and
/// `did.json`, GET only. `certs/**` + revoke entries join when the P7
/// `certs_public` toggle exists (default: false, so not before P7).
pub fn anonymous_covers(object: &ObjectPath) -> bool {
    matches!(object, ObjectPath::Public(_) | ObjectPath::DidJson)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DID: &str = "did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr";

    fn t(chemin: &str) -> String {
        format!("/t/acme/{DID}/{chemin}")
    }

    #[test]
    fn accepts_the_committed_vector_paths() {
        // The exact targets of vectors/p1-store-envelope.json.
        for (chemin, want_key) in [
            (
                "e/circle/blobs/01000000000000000000000000.enc",
                "e/circle/blobs/01000000000000000000000000.enc",
            ),
            ("e/public/hello.md", "e/public/hello.md"),
            (
                "e/self/blobs/01000000000000000000000000.enc",
                "e/self/blobs/01000000000000000000000000.enc",
            ),
        ] {
            let target = parse_target(&t(chemin)).expect(chemin);
            assert_eq!(target.tenant, "acme");
            assert_eq!(target.did, DID);
            match target.kind {
                TargetKind::Object(o) => assert_eq!(o.key(), want_key),
                other => panic!("expected object, got {other:?}"),
            }
        }
    }

    #[test]
    fn accepts_the_whole_layout_grammar() {
        for chemin in [
            "manifest.json",
            "did.json",
            "e/public/notes/2026/plan.md",
            "e/circle/index.json",
            "e/self/index.json",
            "e/circle/hdr/01000000000000000000000000.json",
            "x/gmail/config.json",
            "x/gmail",
            "certs/mandate_0000000000000000000000P0M1.json",
            "gamma/2026-07.jsonl",
        ] {
            assert!(
                parse_target(&t(chemin)).is_some(),
                "should accept: {chemin}"
            );
        }
        for route in ["heads", "batch", "gamma", "sync"] {
            assert!(parse_target(&t(route)).is_some(), "route: {route}");
        }
        assert_eq!(
            parse_target(&format!("/t/acme/{DID}?list=e/public/")).map(|d| d.kind),
            Some(TargetKind::List)
        );
    }

    #[test]
    fn rejects_outside_the_grammar_fail_closed() {
        for raw in [
            "/",
            "/healthz-not-here",
            "/t/acme",
            &format!("/t/acme/{DID}"),         // bare DID, no route
            &format!("/t/acme/{DID}/"),        // empty chemin
            &format!("/t/A!/{DID}/did.json"),  // bad tenant
            &format!("/t/ab/{DID}/did.json"),  // tenant too short
            "/t/acme/not-a-did/did.json",      // bad DID
            "/t/acme/did:aithos:zzz/did.json", // undecodable DID
            &t("e/public/../secrets.md"),      // traversal
            &t("e/public/.hidden"),            // dot-led
            &t("e/public/UPPER.md"),           // case outside grammar
            &t("e/nowhere/index.json"),        // unknown zone
            &t("e/public/index.json/"),        // trailing empty segment
            &t("e/circle/blobs/notasid.enc"),  // sid grammar
            &t("e/circle/blobs/01000000000000000000000000"), // missing .enc
            &t("certs/mandate_short.json"),    // mandate id grammar
            &t("certs/other_0000000000000000000000P0M1.json"),
            &t("gamma/2026-13.jsonl"), // month 13
            &t("gamma/202607.jsonl"),
            &t("unknown.json"),
            &t("e/public/a%2e%2e/x.md"),            // percent-encoding
            &format!("/t/acme/{DID}/did.json?x=1"), // stray query
            &format!("/t/acme/{DID}?other=1"),      // non-list query
        ] {
            assert!(parse_target(raw).is_none(), "should reject: {raw}");
        }
    }

    #[test]
    fn anonymous_covers_the_a2_exceptions_only() {
        assert!(anonymous_covers(&ObjectPath::DidJson));
        assert!(anonymous_covers(&ObjectPath::Public(vec![
            "hello.md".into()
        ])));
        for denied in [
            ObjectPath::Manifest,
            ObjectPath::ZoneIndex("circle".into()),
            ObjectPath::Blob(
                "circle".into(),
                Sid::parse("01000000000000000000000000").unwrap(),
            ),
            ObjectPath::Hdr("self".into(), "01000000000000000000000000".into()),
            ObjectPath::X("gmail".into(), vec![]),
            ObjectPath::Cert("mandate_0000000000000000000000P0M1".into()),
            ObjectPath::GammaSegment("2026-07".into()),
        ] {
            assert!(!anonymous_covers(&denied), "must not cover: {denied:?}");
        }
    }
}
