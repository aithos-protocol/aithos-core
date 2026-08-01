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
    /// `e/<zone>/header.json` — the zone ROOT's sealed key lines
    /// (micro-redline A.1, P3 2026-07-21: a protocol object readers
    /// need; zone ∈ {circle, self, x}).
    ZoneHeader(String),
    /// `e/<zone>/root.enc` — the zone root blob (same micro-redline).
    ZoneRoot(String),
    /// `e/x/<id>/header.json` — the vault-pinned connector header
    /// (micro-redline A.1 EXTENDED at the DEMO-LEA gate, 2026-07-21:
    /// the exact remaining subset of the bundle's closed grammar).
    ConnectorHeader(String),
    /// `e/x/<id>/manifest.enc` — the sealed connector config blob
    /// (same extension; opaque by design).
    ConnectorConfig(String),
    /// `e/<zone>/hdr/<node>.json` — node naming refined with P2 (the P1
    /// grammar bounds the charset; no vector exercises hdr yet).
    Hdr(String, String),
    /// `x/<id>/…` — vault subtree (§08).
    X(String, Vec<String>),
    /// `certs/<mandate_id>.json`.
    Cert(String),
    /// `gamma/<YYYY-MM>.jsonl`.
    GammaSegment(String),
    /// `manifests/<h>.json` — the edition-history slot (redline gate 5).
    /// Server-written on an accepted publish; NO write line covers it.
    ManifestSlot(u64),
    /// `changesets/<64hex>.json` — K1-C sidecar (redline gate 5).
    Changeset(String),
    /// `evidence/<64hex>.json` — K1-C sidecar (redline gate 5).
    Evidence(String),
    /// `public/sections/<sid>.md` — K1-C alias of the public zone.
    PublicSectionAlias(Sid),
    /// `circle/blobs/<sid>.json` — K1-C alias of the circle blob.
    CircleBlobAlias(Sid),
    /// `indices/public.json` (K1-C literal).
    IndicesPublic,
    /// `roots/public.json` (K1-C literal).
    RootsPublic,
    /// `vault/catalog-pins.json` (K1-C literal).
    VaultCatalogPins,
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
            ObjectPath::ZoneHeader(z) => format!("e/{z}/header.json"),
            ObjectPath::ZoneRoot(z) => format!("e/{z}/root.enc"),
            ObjectPath::ConnectorHeader(id) => format!("e/x/{id}/header.json"),
            ObjectPath::ConnectorConfig(id) => format!("e/x/{id}/manifest.enc"),
            ObjectPath::Hdr(z, node) => format!("e/{z}/hdr/{node}.json"),
            ObjectPath::X(id, segs) if segs.is_empty() => format!("x/{id}"),
            ObjectPath::X(id, segs) => format!("x/{id}/{}", segs.join("/")),
            ObjectPath::Cert(id) => format!("certs/{id}.json"),
            ObjectPath::GammaSegment(m) => format!("gamma/{m}.jsonl"),
            ObjectPath::ManifestSlot(h) => format!("manifests/{h}.json"),
            ObjectPath::Changeset(hash) => format!("changesets/{hash}.json"),
            ObjectPath::Evidence(hash) => format!("evidence/{hash}.json"),
            ObjectPath::PublicSectionAlias(sid) => format!("public/sections/{sid}.md"),
            ObjectPath::CircleBlobAlias(sid) => format!("circle/blobs/{sid}.json"),
            ObjectPath::IndicesPublic => "indices/public.json".into(),
            ObjectPath::RootsPublic => "roots/public.json".into(),
            ObjectPath::VaultCatalogPins => "vault/catalog-pins.json".into(),
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

/// A bare `<chemin>` through the same closed grammar — the collection
/// routes (`?list=`, `/batch`, `/sync`) only ever name what a direct GET
/// could address (étape 5).
pub(crate) fn parse_chemin_public(chemin: &str) -> Option<ObjectPath> {
    if chemin.contains('%')
        || chemin.contains('\\')
        || chemin.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return None;
    }
    parse_chemin(chemin)
}

fn parse_chemin(chemin: &str) -> Option<ObjectPath> {
    let segs: Vec<&str> = chemin.split('/').collect();
    match segs.as_slice() {
        ["manifest.json"] => Some(ObjectPath::Manifest),
        ["did.json"] => Some(ObjectPath::DidJson),
        ["e", "public", rest @ ..] => {
            valid_file_segments(rest).then(|| ObjectPath::Public(own(rest)))
        }
        // Micro-redline A.1 (P3, 2026-07-21, arbitrage Mathieu): the zone
        // ROOT's header and blob are servable — the bundle's native
        // carriers of the root sealed lines. ADDITIVE and closed; the
        // runner/derived keys (gateway/**, manifests/tree-*, index-*,
        // -alt) stay OUTSIDE — path_invalid, pinned by BDD.
        ["e", zone, "header.json"] if valid_sealed_zone(zone) || *zone == "x" => {
            Some(ObjectPath::ZoneHeader((*zone).to_owned()))
        }
        // Micro-redline A.1, EXTENDED at the DEMO-LEA gate (2026-07-21):
        // the vault-pinned connector carriers — `e/x/<id>/header.json`
        // and `e/x/<id>/manifest.enc` — are the last servable subset of
        // the bundle's own closed grammar (`validate_store_key`). Still
        // ADDITIVE and closed; everything else under `e/x/<id>/` stays
        // outside the wire.
        ["e", "x", id, "header.json"] if aithos_core::ids::validate_name(id).is_ok() => {
            Some(ObjectPath::ConnectorHeader((*id).to_owned()))
        }
        ["e", "x", id, "manifest.enc"] if aithos_core::ids::validate_name(id).is_ok() => {
            Some(ObjectPath::ConnectorConfig((*id).to_owned()))
        }
        ["e", zone, "root.enc"] if valid_sealed_zone(zone) => {
            Some(ObjectPath::ZoneRoot((*zone).to_owned()))
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
        // ---- draft.2 servable layout (redline gate 5, 2026-07-20) ----
        // ADDITIVE and closed: the exact K1-B/K1-C subset of the bundle's
        // own `validate_store_key` grammar. The bundle-internal keys
        // (`tree-`, `index-`, `-alt`, `gateway/**`, `gamma/gamma.jsonl`)
        // stay OUTSIDE the wire — path_invalid.
        ["manifests", file] => {
            let stem = file.strip_suffix(".json")?;
            if !valid_height_stem(stem) {
                return None;
            }
            Some(ObjectPath::ManifestSlot(stem.parse().ok()?))
        }
        ["changesets", file] => {
            let hash = file.strip_suffix(".json")?;
            valid_hash64(hash).then(|| ObjectPath::Changeset(hash.to_owned()))
        }
        ["evidence", file] => {
            let hash = file.strip_suffix(".json")?;
            valid_hash64(hash).then(|| ObjectPath::Evidence(hash.to_owned()))
        }
        ["public", "sections", file] => {
            let sid = Sid::parse(file.strip_suffix(".md")?).ok()?;
            Some(ObjectPath::PublicSectionAlias(sid))
        }
        ["circle", "blobs", file] => {
            let sid = Sid::parse(file.strip_suffix(".json")?).ok()?;
            Some(ObjectPath::CircleBlobAlias(sid))
        }
        ["indices", "public.json"] => Some(ObjectPath::IndicesPublic),
        ["roots", "public.json"] => Some(ObjectPath::RootsPublic),
        ["vault", "catalog-pins.json"] => Some(ObjectPath::VaultCatalogPins),
        _ => None,
    }
}

/// `<h>` of `manifests/<h>.json`: a decimal integer ≥ 1, no leading zero
/// (redline gate 5). `tree-…`/`index-…`/`…-alt` stems are bundle-internal
/// and fail this on their first non-digit byte.
fn valid_height_stem(stem: &str) -> bool {
    !stem.is_empty()
        && stem.len() <= 19
        && stem.bytes().all(|b| b.is_ascii_digit())
        && !stem.starts_with('0')
}

/// 64 lowercase hex — the K1-C digest suffix (§02.6.3).
fn valid_hash64(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
/// `did.json`, GET only, plus the K1-C aliases of the public zone
/// (`public/sections/**`, `indices/public.json`, `roots/public.json` —
/// redline gate 5, 2026-07-20: the alias of a public zone IS public).
/// `certs/**` + revoke entries join when the P7 `certs_public` toggle
/// exists (default: false, so not before P7).
pub fn anonymous_covers(object: &ObjectPath) -> bool {
    matches!(
        object,
        ObjectPath::Public(_)
            | ObjectPath::DidJson
            | ObjectPath::PublicSectionAlias(_)
            | ObjectPath::IndicesPublic
            | ObjectPath::RootsPublic
    )
}

/// The MANDATED path-map of annexe A.3 (check #10) — anti-abuse
/// availability gate over the LEAF perimeter, never the authority (§3.1).
///
/// Étape 3 lands the READ rows literally:
/// - any valid chain: GET `/heads`, `manifest.json`, `did.json`,
///   `certs/**` — and `e/public/**` (public never narrows under a chain);
/// - `read.gamma[#…]`: GET `gamma/**`, filtered by the COARSE
///   `since`/`until` window only (a segment is refused only when the
///   window can exclude the whole month — finer kind filtering is
///   client/export-side, per the annexe);
/// - `read.<zone>[#sel]`: GET the zone index, `e/<zone>/hdr/**` and
///   `e/<zone>/blobs/**`. `dir`/`tag` selectors cannot exclude a path
///   server-side without the tree (graved note in A.3): the server serves,
///   the client enforces. The exact `id=` selector CAN exclude and does.
/// - `act.x.<id>.*`: GET `x/<id>/**`.
///
/// Étape 4 lands the WRITE rows (pass L + the publish row), literally:
/// - `verbe d'écriture (edit|append|write|delete)` on the zone: PUT
///   `e/<zone>/blobs/**`, `e/<zone>/hdr/**`, the zone index — and POST
///   `/gamma` (the entry itself is verified at A.4). Same graved selector
///   rule as reads: a selector that cannot exclude server-side SERVES
///   (`dir`/`tag` are nodal; only `id=` excludes, by sid);
/// - `act.x.<id>.*`: PUT `x/<id>/**` and POST `/gamma` (its
///   `action`/`inference` entries — the entry-level check is A.4's);
/// - `owner, ou délégué avec authorized_by (§02.6)`: PUT `manifest.json`
///   (CAS), `did.json`, `certs/**`, `gamma/<YYYY-MM>.jsonl` (réplique) —
///   any valid chain reaches the deposit, where A.4 verifies the
///   artifact's OWN displayed authority (coverage is anti-abuse, the
///   authority never lives here);
/// - `e/public/**` writes have no A.3 row (the draft.2 `public/sections/`
///   alias is the pending A.1 redline ④): default deny.
///
/// The collection routes (batch, sync, list) land with gate 5; until then
/// this map denies them — the default deny of A.3, always a clean 403.
pub fn mandated_covers(
    perimeter: &[aithos_core::mandate::PerimeterEntry],
    kind: &TargetKind,
    method: &str,
) -> bool {
    use aithos_core::mandate::{PerimeterEntry, Verb};
    let zone_covered = |zone: &str, sid: Option<&Sid>, want_write: bool| {
        // Writes want any of `edit|append|write|delete` (the A.3
        // write-verb set is « everything but read »); reads are served
        // by ANY verb on the zone — the §04.2 lattice ("append creates
        // and reads"): a pen that may write a node may re-read what it
        // needs to write it (P3, aligned at the mode-B gate; anti-abuse,
        // never authority — the core still enforces the real perimeter).
        let verb_fits = |verb: &Verb| {
            if want_write {
                !matches!(verb, Verb::Read)
            } else {
                true
            }
        };
        perimeter.iter().any(|entry| match entry {
            PerimeterEntry::Ethos { verb, zone: z, .. } => verb_fits(verb) && z.as_str() == zone,
            PerimeterEntry::EthosId { verb, zone: z, id } => {
                verb_fits(verb) && z.as_str() == zone && sid.is_some_and(|s| s == id)
            }
            _ => false,
        })
    };
    // An index has no target SID in its storage path. Therefore an exact-id
    // mandate on the same zone must reach the index needed to resolve and
    // commit that one node; Core still validates that the index delta affects
    // only the authorized SID before the manifest can be accepted.
    let zone_index_covered = |zone: &str, want_write: bool| {
        perimeter.iter().any(|entry| match entry {
            PerimeterEntry::Ethos { verb, zone: z, .. }
            | PerimeterEntry::EthosId { verb, zone: z, .. } => {
                z.as_str() == zone && (!want_write || !matches!(verb, Verb::Read))
            }
            _ => false,
        })
    };
    let any_write_verb = || {
        perimeter.iter().any(|entry| {
            matches!(entry,
                PerimeterEntry::Ethos { verb, .. } | PerimeterEntry::EthosId { verb, .. }
                    if !matches!(verb, Verb::Read))
        })
    };
    let any_act = || {
        perimeter
            .iter()
            .any(|entry| matches!(entry, PerimeterEntry::Act { .. }))
    };
    let act_connector = |id: &str| {
        perimeter
            .iter()
            .any(|entry| matches!(entry, PerimeterEntry::Act { connector, .. } if connector == id))
    };
    // Header nodes are sid-or-hash stems; only a parseable sid can be
    // matched by an `id=` selector — hash stems serve under the zone-wide
    // right (the selector cannot exclude them).
    let hdr_covered = |zone: &str, node: &str, want_write: bool| match Sid::parse(node) {
        Ok(sid) => zone_covered(zone, Some(&sid), want_write),
        Err(_) => zone_covered(zone, None, want_write),
    };
    match (kind, method) {
        (TargetKind::Heads, "GET") => true,
        // The collection routes (gate 5): any valid chain may call; the
        // RESULTS are filtered per-path through this same map (coarse
        // perimeter filtering — a shorter 200, never an error).
        (TargetKind::List, "GET") => true,
        (TargetKind::Batch | TargetKind::Sync, "POST") => true,
        (TargetKind::Gamma, "POST") => any_write_verb() || any_act(),
        (TargetKind::Object(object), "GET") => match object {
            ObjectPath::Manifest | ObjectPath::DidJson | ObjectPath::Cert(_) => true,
            // Redline gate 5: proof material for the cold verify —
            // « toute chaîne valide du DID ».
            ObjectPath::ManifestSlot(_)
            | ObjectPath::Changeset(_)
            | ObjectPath::Evidence(_)
            | ObjectPath::VaultCatalogPins => true,
            // Public never narrows under a chain — canonical or alias.
            ObjectPath::Public(_)
            | ObjectPath::PublicSectionAlias(_)
            | ObjectPath::IndicesPublic
            | ObjectPath::RootsPublic => true,
            // `read.gamma` windows (third-party log readers) — and the
            // POST-/gamma set: who may APPEND may re-read the log it
            // chains onto (P3 mode B, aligned at the gate; the sealed
            // bodies stay sealed, the clear skeleton is what the store
            // itself already sees — anti-abuse, never authority).
            ObjectPath::GammaSegment(month) => {
                perimeter.iter().any(|entry| {
                    matches!(entry, PerimeterEntry::Gamma { since, until, .. }
                        if gamma_window_may_reach(month, since.as_deref(), until.as_deref()))
                }) || any_write_verb()
                    || any_act()
            }
            ObjectPath::ZoneIndex(zone) => zone_index_covered(zone, false),
            ObjectPath::Blob(zone, sid) => zone_covered(zone, Some(sid), false),
            // The K1-C blob alias follows its zone's row (the frozen p8
            // read plan: read.circle covers circle/blobs/<sid>.json).
            ObjectPath::CircleBlobAlias(sid) => zone_covered("circle", Some(sid), false),
            ObjectPath::ZoneHeader(zone) | ObjectPath::ZoneRoot(zone) => {
                zone_covered(zone, None, false)
            }
            ObjectPath::Hdr(zone, node) => hdr_covered(zone, node, false),
            // The connector carriers follow the vault subtree's row
            // (micro-redline A.1 extension, DEMO-LEA gate).
            ObjectPath::ConnectorHeader(id) | ObjectPath::ConnectorConfig(id) => {
                // Connector carriers are sealed to the gateway recipient.
                // Its stable governance mandate may fetch a newly
                // owner-published binding before the agent has any business
                // capability for that connector. Core still verifies the
                // owner header, recipient wrapping, AEAD and manifest.
                act_connector(id) || act_connector("gateway")
            }
            ObjectPath::X(id, _) => act_connector(id),
        },
        (TargetKind::Object(object), "PUT") => match object {
            // The publish row: coverage for any valid chain; the deposit
            // (A.4) verifies the artifact's own displayed authority. The
            // draft.2 sidecars and publication derivatives join with the
            // redline gate 5 (deposited BEFORE the publish pinning them).
            ObjectPath::Manifest
            | ObjectPath::DidJson
            | ObjectPath::Cert(_)
            | ObjectPath::GammaSegment(_)
            | ObjectPath::Changeset(_)
            | ObjectPath::Evidence(_)
            | ObjectPath::IndicesPublic
            | ObjectPath::RootsPublic
            | ObjectPath::VaultCatalogPins => true,
            // NO write line, owner included: the slot is written by the
            // server on an accepted publish only (redline gate 5).
            ObjectPath::ManifestSlot(_) => false,
            // Pass L — write verbs on the zone (canonical and alias).
            ObjectPath::ZoneIndex(zone) => zone_index_covered(zone, true),
            ObjectPath::Blob(zone, sid) => zone_covered(zone, Some(sid), true),
            ObjectPath::CircleBlobAlias(sid) => zone_covered("circle", Some(sid), true),
            // The public write line arrives via its K1-C alias (redline
            // gate 5): a write verb on `public` covers the alias section.
            ObjectPath::PublicSectionAlias(sid) => zone_covered("public", Some(sid), true),
            ObjectPath::ZoneHeader(zone) | ObjectPath::ZoneRoot(zone) => {
                zone_covered(zone, None, true)
            }
            ObjectPath::Hdr(zone, node) => hdr_covered(zone, node, true),
            // Same row in write: the vault subtree's act chain deposits
            // its own carriers (owner covers all, as everywhere).
            ObjectPath::ConnectorHeader(id) | ObjectPath::ConnectorConfig(id) => act_connector(id),
            ObjectPath::X(id, _) => act_connector(id),
            // Canonical `e/public/**` writes keep no A.3 row (the redline
            // opened the ALIAS only — §5.4 of the acted redline).
            ObjectPath::Public(_) => false,
        },
        // Everything else: default deny.
        _ => false,
    }
}

/// Can a `read.gamma` window still reach the `YYYY-MM` segment? Refuse
/// only when `until` ends strictly before the month begins or `since`
/// starts at/after the next month — RFC 3339 Z strings compare
/// lexicographically, so no clock math is needed.
fn gamma_window_may_reach(month: &str, since: Option<&str>, until: Option<&str>) -> bool {
    if !valid_month(month) {
        return false;
    }
    let month_start = format!("{month}-01T00:00:00Z");
    let (year, number): (i64, i64) = (month[..4].parse().unwrap(), month[5..7].parse().unwrap());
    let (next_year, next_number) = if number == 12 {
        (year + 1, 1)
    } else {
        (year, number + 1)
    };
    let next_start = format!("{next_year:04}-{next_number:02}-01T00:00:00Z");
    let excluded = until.is_some_and(|u| u < month_start.as_str())
        || since.is_some_and(|s| s >= next_start.as_str());
    !excluded
}

/// The `certs/<id>.json` id grammar, exposed for the #7 chain loader — a
/// malformed id never becomes a storage key.
pub(crate) fn mandate_id_is_valid(id: &str) -> bool {
    valid_mandate_id(id)
}

/// The parsed `?list=` query (A.3): `list=<prefix>[&after=<chemin>]
/// [&limit=<n>]`, in that order, each at most once — a closed grammar,
/// anything else is `path_invalid`. The `limit ≤ 1000` bound is A.8's and
/// answers `413`, not a grammar refusal — the caller enforces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub prefix: String,
    pub after: Option<String>,
    pub limit: Option<u64>,
}

pub fn parse_list_query(query: &str) -> Option<ListQuery> {
    let mut params = query.split('&');
    let prefix = params.next()?.strip_prefix("list=")?.to_owned();
    let mut after = None;
    let mut limit = None;
    for param in params {
        if let Some(value) = param.strip_prefix("after=") {
            if after.is_some() || limit.is_some() || value.is_empty() {
                return None;
            }
            after = Some(value.to_owned());
        } else {
            let value = param.strip_prefix("limit=")?;
            if limit.is_some() || value.is_empty() || value.len() > 6 {
                return None;
            }
            limit = Some(value.parse().ok()?);
        }
    }
    Some(ListQuery {
        prefix,
        after,
        limit,
    })
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
    fn redline_gate5_paths_parse_and_bundle_grammar_agrees() {
        // The redline grammar is a SUBSET of the bundle's own closed
        // grammar (`validate_store_key`) — composition as verification:
        // every wire-accepted redline key must also be a canonical bundle
        // object key. Never the reverse (bundle-internal keys stay out).
        for chemin in [
            "manifests/1.json",
            "manifests/42.json",
            &format!("changesets/{}.json", "a".repeat(64)),
            &format!("evidence/{}.json", "0".repeat(64)),
            "public/sections/01000000000000000000000P81.md",
            "circle/blobs/01000000000000000000000P82.json",
            "indices/public.json",
            "roots/public.json",
            "vault/catalog-pins.json",
        ] {
            let target = parse_target(&t(chemin)).unwrap_or_else(|| panic!("accept: {chemin}"));
            let TargetKind::Object(object) = target.kind else {
                panic!("expected object: {chemin}");
            };
            assert_eq!(object.key(), *chemin);
            aithos_bundle::validate_store_key(chemin)
                .unwrap_or_else(|e| panic!("bundle grammar disagrees on {chemin}: {e}"));
        }
    }

    #[test]
    fn bundle_internal_keys_stay_outside_the_wire() {
        for chemin in [
            "manifests/tree-2.json",
            "manifests/index-public-2.json",
            "manifests/2-alt.json",
            "manifests/0.json",
            "manifests/01.json",
            &format!("changesets/{}.json", "A".repeat(64)),
            &format!("changesets/{}.json", "a".repeat(63)),
            "gateway/state.json",
            "gateway/keys.json",
            "gamma/gamma.jsonl",
            "public/sections/notasid.md",
            "circle/blobs/01000000000000000000000P82.enc",
            "indices/circle.json",
            "roots/circle.json",
            "vault/other.json",
        ] {
            assert!(parse_target(&t(chemin)).is_none(), "reject: {chemin}");
        }
    }

    #[test]
    fn list_query_grammar_is_closed() {
        assert_eq!(
            parse_list_query("list="),
            Some(ListQuery {
                prefix: String::new(),
                after: None,
                limit: None
            })
        );
        assert_eq!(
            parse_list_query("list=e/&after=e/circle/index.json&limit=2"),
            Some(ListQuery {
                prefix: "e/".into(),
                after: Some("e/circle/index.json".into()),
                limit: Some(2)
            })
        );
        for bad in [
            "other=1",
            "list=&bogus=1",
            "list=&limit=",
            "list=&limit=abc",
            "list=&limit=2&limit=3",
            "list=&limit=2&after=x", // wrong order
            "list=&after=",
        ] {
            assert!(parse_list_query(bad).is_none(), "reject: {bad}");
        }
    }

    #[test]
    fn anonymous_covers_the_a2_exceptions_only() {
        assert!(anonymous_covers(&ObjectPath::DidJson));
        assert!(anonymous_covers(&ObjectPath::Public(vec![
            "hello.md".into()
        ])));
        // Redline gate 5: the public-zone aliases are public too.
        assert!(anonymous_covers(&ObjectPath::PublicSectionAlias(
            Sid::parse("01000000000000000000000P81").unwrap()
        )));
        assert!(anonymous_covers(&ObjectPath::IndicesPublic));
        assert!(anonymous_covers(&ObjectPath::RootsPublic));
        for denied in [
            ObjectPath::Manifest,
            ObjectPath::ManifestSlot(1),
            ObjectPath::Changeset("a".repeat(64)),
            ObjectPath::Evidence("a".repeat(64)),
            ObjectPath::VaultCatalogPins,
            ObjectPath::CircleBlobAlias(Sid::parse("01000000000000000000000P82").unwrap()),
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

    #[test]
    fn an_exact_id_write_reaches_its_zone_index_but_not_another_zone() {
        use aithos_core::mandate::{PerimeterEntry, Verb};
        use aithos_core::path::Zone;

        let perimeter = vec![PerimeterEntry::EthosId {
            verb: Verb::Edit,
            zone: Zone::Circle,
            id: Sid::parse("01000000000000000000000000").unwrap(),
        }];
        assert!(mandated_covers(
            &perimeter,
            &TargetKind::Object(ObjectPath::ZoneIndex("circle".into())),
            "PUT",
        ));
        assert!(!mandated_covers(
            &perimeter,
            &TargetKind::Object(ObjectPath::ZoneIndex("self".into())),
            "PUT",
        ));
    }

    #[test]
    fn migrated_gateway_state_rides_the_vault_subtree_row() {
        use aithos_core::mandate::PerimeterEntry;

        // SPL-2 : `x/gateway/state.json` est un objet `x/<id>/**` ordinaire.
        // Le mandat de la gateway (`act.x.gateway.*`) le couvre en lecture ET
        // en écriture — là où l'ancienne clé `gateway/state.json` n'est même
        // pas une route (`bundle_internal_keys_stay_outside_the_wire`).
        let target = parse_target(&t("x/gateway/state.json")).expect("parses");
        let TargetKind::Object(object) = target.kind else {
            panic!("expected object");
        };
        assert_eq!(object.key(), "x/gateway/state.json");
        aithos_bundle::validate_store_key("x/gateway/state.json")
            .expect("bundle grammar accepts the migrated key");

        let perimeter = vec![PerimeterEntry::parse("act.x.gateway.*").unwrap()];
        for method in ["GET", "PUT"] {
            assert!(
                mandated_covers(&perimeter, &TargetKind::Object(object.clone()), method),
                "act.x.gateway.* must cover {method} x/gateway/state.json"
            );
        }
        // Le mandat d'un autre connecteur ne couvre pas l'état de la gateway.
        let other = vec![PerimeterEntry::parse("act.x.gmail.*").unwrap()];
        assert!(!mandated_covers(&other, &TargetKind::Object(object), "GET"));
    }

    #[test]
    fn gateway_governance_can_fetch_but_not_publish_new_connector_carriers() {
        use aithos_core::mandate::PerimeterEntry;

        let perimeter = vec![PerimeterEntry::parse("act.x.gateway.*").unwrap()];
        for object in [
            ObjectPath::ConnectorHeader("notes-live".into()),
            ObjectPath::ConnectorConfig("notes-live".into()),
        ] {
            assert!(mandated_covers(
                &perimeter,
                &TargetKind::Object(object.clone()),
                "GET",
            ));
            assert!(!mandated_covers(
                &perimeter,
                &TargetKind::Object(object),
                "PUT",
            ));
        }
        assert!(!mandated_covers(
            &perimeter,
            &TargetKind::Object(ObjectPath::X("notes-live".into(), vec![])),
            "GET",
        ));
    }
}
