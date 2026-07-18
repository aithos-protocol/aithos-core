//! Mandates: the certificate plane (spec §04, §05).
//!
//! A mandate grants a keypair a perimeter, under constraints, for a window.
//! Everything here is pure: the verifier takes time `T` as a parameter and
//! reads only the documents it is handed.

use crate::did::{DidDocument, SignatureBlock};
use crate::error::{Error, Result};
use crate::ids::{validate_tag, Sid};
use crate::jcs;
use crate::keys::ed2x;
use crate::path::Zone;
use crate::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const MANDATE_VERSION_DRAFT1: &str = "1.0.0-draft.1";
pub const MANDATE_VERSION_DRAFT2: &str = "1.0.0-draft.2";
pub const MANDATE_VERSION: &str = MANDATE_VERSION_DRAFT2;

// ------------------------------------------------------------- perimeter

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verb {
    Read,
    Edit,
    Append,
    Delete,
    Write,
}

impl Verb {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "read" => Verb::Read,
            "edit" => Verb::Edit,
            "append" => Verb::Append,
            "delete" => Verb::Delete,
            "write" => Verb::Write,
            other => return Err(Error::InvalidMandate(format!("unknown verb {other}"))),
        })
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Verb::Read => "read",
            Verb::Edit => "edit",
            Verb::Append => "append",
            Verb::Delete => "delete",
            Verb::Write => "write",
        }
    }

    /// Verb lattice (§04.2): read ⊑ edit ⊑ append ⊑ write and
    /// read ⊑ delete ⊑ write.
    fn covers(self, child: Verb) -> bool {
        use Verb::*;
        match (self, child) {
            (a, b) if a == b => true,
            (Write, _) => true,
            (Append, Read | Edit) => true,
            (Edit | Delete, Read) => true,
            _ => false,
        }
    }
}

/// One perimeter entry (§04.2). Selectors compose by intersection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerimeterEntry {
    Ethos {
        verb: Verb,
        zone: Zone,
        /// Folder sid-path from the zone root; empty = the whole zone.
        dir: Vec<Sid>,
        /// Folder-local (or zone-root) tag restriction.
        tag: Option<String>,
    },
    /// Exact section selector. `id=` composes with no other selector (D1).
    EthosId {
        verb: Verb,
        zone: Zone,
        id: Sid,
    },
    /// Connector action right: `act.x.<connector>.<action|*>`.
    Act {
        connector: String,
        /// `None` = wildcard `*` (read/act class only, §04.2 — binding
        /// actions must be named; enforced with connector manifests, §08.1).
        action: Option<String>,
    },
    /// Log read right: `read.gamma#<gamma-selector>` (§04.2, §07.8).
    /// Absent dimensions cover any value.
    Gamma {
        dir: Vec<Sid>,
        id: Option<Sid>,
        tag: Option<String>,
        kind: Option<String>,
        action: Option<String>,
        since: Option<String>,
        until: Option<String>,
    },
    Issue {
        depth: u32,
    },
    /// Revocation right (§06.7): the authority to publish `revoke` entries,
    /// carrying no key. `None` scope = the issuer's whole revocable reach.
    Revoke {
        scope: Option<Box<PerimeterEntry>>,
    },
}

impl PerimeterEntry {
    pub fn parse(s: &str) -> Result<Self> {
        let err = |m: &str| Error::InvalidMandate(format!("{m}: {s}"));
        if let Some(rest) = s.strip_prefix("issue") {
            let depth = match rest.strip_prefix("#depth=") {
                Some(n) => n.parse().map_err(|_| err("bad depth"))?,
                None if rest.is_empty() => 1,
                _ => return Err(err("bad issue entry")),
            };
            if depth == 0 {
                return Err(err("issue depth must be at least one"));
            }
            return Ok(PerimeterEntry::Issue { depth });
        }
        if let Some(rest) = s.strip_prefix("act.x.") {
            let (connector, pat) = rest
                .rsplit_once('.')
                .ok_or_else(|| err("want act.x.<connector>.<action|*>"))?;
            if connector.is_empty() || pat.is_empty() {
                return Err(err("empty connector or action"));
            }
            return Ok(PerimeterEntry::Act {
                connector: connector.to_owned(),
                action: (pat != "*").then(|| pat.to_owned()),
            });
        }
        if s == "read.gamma" || s.starts_with("read.gamma#") {
            return Self::parse_gamma(s.strip_prefix("read.gamma").expect("prefix checked"));
        }
        if s == "revoke" {
            return Ok(PerimeterEntry::Revoke { scope: None });
        }
        if let Some(rest) = s.strip_prefix("revoke.") {
            // `revoke.<zone>[#selector]` reuses the ethos parser for its scope.
            let inner = PerimeterEntry::parse(&format!("read.{rest}"))?;
            return Ok(PerimeterEntry::Revoke {
                scope: Some(Box::new(inner)),
            });
        }
        let (head, selector) = match s.split_once('#') {
            Some((h, sel)) => (h, Some(sel)),
            None => (s, None),
        };
        let (verb, zone) = head
            .split_once('.')
            .ok_or_else(|| err("want <verb>.<zone>"))?;
        let (verb, zone) = (Verb::parse(verb)?, Zone::parse(zone)?);
        let mut dir = Vec::new();
        let mut id = None;
        let mut tag = None;
        let mut seen_dir = false;
        let mut seen_id = false;
        let mut seen_tag = false;
        if let Some(sel) = selector {
            for part in sel.split('&') {
                match part.split_once('=') {
                    Some(("dir", p)) => {
                        if seen_dir {
                            return Err(err("duplicate dir selector"));
                        }
                        seen_dir = true;
                        for seg in p.split('/').filter(|x| !x.is_empty()) {
                            dir.push(Sid::parse(seg)?);
                        }
                    }
                    Some(("id", value)) => {
                        if seen_id {
                            return Err(err("duplicate id selector"));
                        }
                        seen_id = true;
                        id = Some(Sid::parse(value)?);
                    }
                    Some(("tag", t)) => {
                        if seen_tag {
                            return Err(err("duplicate tag selector"));
                        }
                        seen_tag = true;
                        validate_tag(t)?;
                        tag = Some(t.to_owned());
                    }
                    _ => return Err(err("unknown selector")),
                }
            }
        }
        if let Some(id) = id {
            if seen_dir || seen_tag {
                return Err(err("id selector composes with nothing"));
            }
            Ok(PerimeterEntry::EthosId { verb, zone, id })
        } else {
            Ok(PerimeterEntry::Ethos {
                verb,
                zone,
                dir,
                tag,
            })
        }
    }

    fn parse_gamma(selector: &str) -> Result<Self> {
        let err = |m: &str| Error::InvalidMandate(format!("{m}: read.gamma{selector}"));
        let mut e = PerimeterEntry::Gamma {
            dir: Vec::new(),
            id: None,
            tag: None,
            kind: None,
            action: None,
            since: None,
            until: None,
        };
        let PerimeterEntry::Gamma {
            dir,
            id,
            tag,
            kind,
            action,
            since,
            until,
        } = &mut e
        else {
            unreachable!()
        };
        if let Some(sel) = selector.strip_prefix('#') {
            for part in sel.split('&') {
                match part.split_once('=') {
                    Some(("dir", p)) => {
                        for seg in p.split('/').filter(|x| !x.is_empty()) {
                            dir.push(Sid::parse(seg)?);
                        }
                    }
                    Some(("id", v)) => *id = Some(Sid::parse(v)?),
                    Some(("tag", t)) => {
                        validate_tag(t)?;
                        *tag = Some(t.to_owned());
                    }
                    Some(("kind", k)) => *kind = Some(k.to_owned()),
                    Some(("action", a)) => *action = Some(a.to_owned()),
                    Some(("since", t)) => *since = Some(t.to_owned()),
                    Some(("until", t)) => *until = Some(t.to_owned()),
                    _ => return Err(err("unknown gamma selector")),
                }
            }
        } else if !selector.is_empty() {
            return Err(err("bad gamma entry"));
        }
        Ok(e)
    }

    pub fn to_entry_string(&self) -> String {
        match self {
            PerimeterEntry::Issue { depth } => format!("issue#depth={depth}"),
            PerimeterEntry::Act { connector, action } => {
                format!("act.x.{connector}.{}", action.as_deref().unwrap_or("*"))
            }
            PerimeterEntry::Gamma {
                dir,
                id,
                tag,
                kind,
                action,
                since,
                until,
            } => {
                let mut sels = Vec::new();
                if !dir.is_empty() {
                    let p: Vec<String> = dir.iter().map(ToString::to_string).collect();
                    sels.push(format!("dir={}", p.join("/")));
                }
                if let Some(v) = id {
                    sels.push(format!("id={v}"));
                }
                if let Some(v) = tag {
                    sels.push(format!("tag={v}"));
                }
                if let Some(v) = kind {
                    sels.push(format!("kind={v}"));
                }
                if let Some(v) = action {
                    sels.push(format!("action={v}"));
                }
                if let Some(v) = since {
                    sels.push(format!("since={v}"));
                }
                if let Some(v) = until {
                    sels.push(format!("until={v}"));
                }
                if sels.is_empty() {
                    "read.gamma".to_owned()
                } else {
                    format!("read.gamma#{}", sels.join("&"))
                }
            }
            PerimeterEntry::Ethos {
                verb,
                zone,
                dir,
                tag,
            } => {
                let mut out = format!("{}.{}", verb.as_str(), zone.as_str());
                let mut sels = Vec::new();
                if !dir.is_empty() {
                    let p: Vec<String> = dir.iter().map(ToString::to_string).collect();
                    sels.push(format!("dir={}", p.join("/")));
                }
                if let Some(t) = tag {
                    sels.push(format!("tag={t}"));
                }
                if !sels.is_empty() {
                    out.push('#');
                    out.push_str(&sels.join("&"));
                }
                out
            }
            PerimeterEntry::EthosId { verb, zone, id } => {
                format!("{}.{}#id={id}", verb.as_str(), zone.as_str())
            }
            PerimeterEntry::Revoke { scope: None } => "revoke".to_owned(),
            PerimeterEntry::Revoke { scope: Some(s) } => {
                // Render the inner ethos scope, then swap read→revoke.
                let inner = s.to_entry_string();
                format!("revoke.{}", inner.trim_start_matches("read."))
            }
        }
    }

    /// Containment (§04.2, §05.3): nodal dir containment, tag equality,
    /// verb lattice; an absent dimension covers any value of it.
    pub fn covers(&self, child: &PerimeterEntry) -> bool {
        /// Absent covers anything; present covers only the equal value.
        fn dim<T: PartialEq>(parent: &Option<T>, child: &Option<T>) -> bool {
            match (parent, child) {
                (None, _) => true,
                (Some(a), Some(b)) => a == b,
                (Some(_), None) => false,
            }
        }
        match (self, child) {
            (PerimeterEntry::Issue { depth: n }, PerimeterEntry::Issue { depth: m }) => m < n,
            (
                PerimeterEntry::Act {
                    connector: pc,
                    action: pa,
                },
                PerimeterEntry::Act {
                    connector: cc,
                    action: ca,
                },
            ) => pc == cc && dim(pa, ca),
            (
                PerimeterEntry::Gamma {
                    dir: pd,
                    id: pi,
                    tag: pt,
                    kind: pk,
                    action: pa,
                    since: ps,
                    until: pu,
                },
                PerimeterEntry::Gamma {
                    dir: cd,
                    id: ci,
                    tag: ct,
                    kind: ck,
                    action: ca,
                    since: cs,
                    until: cu,
                },
            ) => {
                cd.len() >= pd.len()
                    && cd[..pd.len()] == pd[..]
                    && dim(pi, ci)
                    && dim(pt, ct)
                    && dim(pk, ck)
                    && dim(pa, ca)
                    // Time bounds tighten: child range inside parent range
                    // (RFC 3339 Zulu strings compare chronologically).
                    && match (ps, cs) {
                        (None, _) => true,
                        (Some(p), Some(c)) => c >= p,
                        (Some(_), None) => false,
                    }
                    && match (pu, cu) {
                        (None, _) => true,
                        (Some(p), Some(c)) => c <= p,
                        (Some(_), None) => false,
                    }
            }
            (
                PerimeterEntry::Ethos {
                    verb: pv,
                    zone: pz,
                    dir: pd,
                    tag: pt,
                },
                PerimeterEntry::Ethos {
                    verb: cv,
                    zone: cz,
                    dir: cd,
                    tag: ct,
                },
            ) => {
                pz == cz
                    && pv.covers(*cv)
                    && dir_covers(pd, cd)
                    && match (pt, ct) {
                        (None, _) => true,
                        (Some(a), Some(b)) => a == b,
                        (Some(_), None) => false,
                    }
            }
            (
                PerimeterEntry::Ethos {
                    verb: pv,
                    zone: pz,
                    dir,
                    tag,
                },
                PerimeterEntry::EthosId {
                    verb: cv, zone: cz, ..
                },
            ) => pz == cz && pv.covers(*cv) && dir.is_empty() && tag.is_none(),
            (
                PerimeterEntry::EthosId {
                    verb: pv,
                    zone: pz,
                    id: pi,
                },
                PerimeterEntry::EthosId {
                    verb: cv,
                    zone: cz,
                    id: ci,
                },
            ) => pz == cz && pv.covers(*cv) && pi == ci,
            (PerimeterEntry::Revoke { scope: ps }, PerimeterEntry::Revoke { scope: cs }) => {
                match (ps, cs) {
                    (None, _) => true, // a bare revoke covers any revoke scope
                    (Some(a), Some(b)) => a.covers(b),
                    (Some(_), None) => false,
                }
            }
            _ => false,
        }
    }
}

/// Free-function containment, for callers that hold two entries (§06.7).
#[must_use]
pub fn covers(parent: &PerimeterEntry, child: &PerimeterEntry) -> bool {
    parent.covers(child)
}

/// Nodal `dir` containment (§04.2): a `dir` names its granted folder by its
/// **terminal sid** — the leading segments are the address at issuance, kept
/// for audit, never a constraint. A target is inside iff its chain passes
/// through that sid; the empty `dir` is the zone root and covers the zone.
/// On a tree that never moved this equals segment-list prefix containment
/// (sids are unique); it diverges only after a move (§02.9): the perimeter
/// follows the node, not its address. `gamma-selector` dirs are NOT nodal —
/// they filter recorded log coordinates (§07.8).
#[must_use]
pub fn dir_covers(dir: &[Sid], chain: &[Sid]) -> bool {
    match dir.last() {
        None => true,
        Some(node) => chain.contains(node),
    }
}

/// The operation a verifier is asked about.
#[derive(Debug, Clone)]
pub struct Op<'a> {
    pub verb: Verb,
    pub zone: Zone,
    /// Folder sid-path of the target section.
    pub folders: &'a [Sid],
    /// Clear tags of the target section (empty for none).
    pub tags: &'a [String],
}

/// A section-precise Ethos operation (§04.2, D1).
///
/// Unlike the historical location-only [`Op`], this input carries the target
/// SID explicitly so an `id=` grant can be decided without a resolver.
#[derive(Debug, Clone)]
pub struct SectionOp<'a> {
    pub verb: Verb,
    pub zone: Zone,
    pub sid: Sid,
    pub folders: &'a [Sid],
    pub tags: &'a [String],
}

/// Decide a section-precise operation against a perimeter.
#[must_use]
pub fn covers_section_op(perimeter: &[PerimeterEntry], op: &SectionOp<'_>) -> bool {
    perimeter.iter().any(|entry| match entry {
        PerimeterEntry::Ethos {
            verb,
            zone,
            dir,
            tag,
        } => {
            *zone == op.zone
                && verb.covers(op.verb)
                && dir_covers(dir, op.folders)
                && tag.as_ref().is_none_or(|value| op.tags.contains(value))
        }
        PerimeterEntry::EthosId { verb, zone, id } => {
            *zone == op.zone && verb.covers(op.verb) && *id == op.sid
        }
        _ => false,
    })
}

/// Does any leaf entry cover the operation? `op.folders` is the target's
/// CURRENT resolved chain — nodal containment (§04.2) makes the perimeter
/// follow the node across moves (§02.9).
pub fn covers_op(perimeter: &[PerimeterEntry], op: &Op<'_>) -> bool {
    perimeter.iter().any(|e| match e {
        PerimeterEntry::Ethos {
            verb,
            zone,
            dir,
            tag,
        } => {
            *zone == op.zone
                && verb.covers(op.verb)
                && dir_covers(dir, op.folders)
                && tag.as_ref().is_none_or(|t| op.tags.contains(t))
        }
        _ => false,
    })
}

/// A connector action a verifier is asked about (§04.2, §07.4).
#[derive(Debug, Clone)]
pub struct ActOp {
    pub connector: String,
    pub action: String,
}

/// Does any leaf entry cover the connector action? Wildcards cover any
/// named action (binding-class refusal arrives with connector manifests,
/// §08.1).
pub fn covers_act(perimeter: &[PerimeterEntry], op: &ActOp) -> bool {
    perimeter.iter().any(|e| match e {
        PerimeterEntry::Act { connector, action } => {
            *connector == op.connector && action.as_ref().is_none_or(|a| *a == op.action)
        }
        _ => false,
    })
}

/// A gamma query a verifier is asked about (§07.8): each present dimension
/// filters; the query is covered iff it stays inside some `read.gamma`
/// entry, dimension by dimension.
#[derive(Debug, Clone, Default)]
pub struct GammaQuery {
    pub dir: Vec<Sid>,
    pub id: Option<Sid>,
    pub tag: Option<String>,
    pub kind: Option<String>,
    pub action: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
}

/// Certificate half of log access (§07.8): an honest verifier refuses a
/// query wider than the granted gamma perimeter on any dimension.
pub fn covers_gamma_query(perimeter: &[PerimeterEntry], q: &GammaQuery) -> bool {
    let child = PerimeterEntry::Gamma {
        dir: q.dir.clone(),
        id: q.id,
        tag: q.tag.clone(),
        kind: q.kind.clone(),
        action: q.action.clone(),
        since: q.since.clone(),
        until: q.until.clone(),
    };
    perimeter
        .iter()
        .any(|e| matches!(e, PerimeterEntry::Gamma { .. }) && e.covers(&child))
}

// --------------------------------------------------------------- mandate

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grantee {
    pub id: String,
    pub label: String,
    pub pubkey: String,
    pub kex_pubkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    #[serde(rename = "aithos-mandate-core")]
    pub version: String,
    pub id: String,
    pub subject: String,
    pub parent: Option<String>,
    pub issued_by: String,
    pub grantee: Grantee,
    pub perimeter: Vec<String>,
    pub constraints: serde_json::Value,
    pub not_before: String,
    pub not_after: String,
    pub issued_at: String,
    pub nonce: String,
    pub signature: SignatureBlock,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TimestampPrefix {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CanonicalTimestamp {
    prefix: TimestampPrefix,
    fractional: Vec<u8>,
}

impl CanonicalTimestamp {
    pub(crate) fn parse(value: &str) -> core::result::Result<Self, ()> {
        let body = value.strip_suffix('Z').ok_or(())?;
        let (date, time) = body.split_once('T').ok_or(())?;
        if time.contains('T') {
            return Err(());
        }

        let mut date_parts = date.split('-');
        let year = parse_fixed_u16(date_parts.next().ok_or(())?, 4)?;
        let month = parse_fixed_u8(date_parts.next().ok_or(())?, 2)?;
        let day = parse_fixed_u8(date_parts.next().ok_or(())?, 2)?;
        if date_parts.next().is_some() || !(1..=12).contains(&month) {
            return Err(());
        }
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let days = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        if !(1..=days).contains(&day) {
            return Err(());
        }

        let mut time_parts = time.split(':');
        let hour = parse_fixed_u8(time_parts.next().ok_or(())?, 2)?;
        let minute = parse_fixed_u8(time_parts.next().ok_or(())?, 2)?;
        let second_and_fraction = time_parts.next().ok_or(())?;
        if time_parts.next().is_some() || hour > 23 || minute > 59 {
            return Err(());
        }
        let (second_raw, fractional_raw) = match second_and_fraction.split_once('.') {
            Some((second, fractional)) if !fractional.is_empty() => (second, fractional),
            Some(_) => return Err(()),
            None => (second_and_fraction, ""),
        };
        let second = parse_fixed_u8(second_raw, 2)?;
        if second > 59 || !fractional_raw.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(());
        }

        Ok(Self {
            prefix: TimestampPrefix {
                year,
                month,
                day,
                hour,
                minute,
                second,
            },
            fractional: fractional_raw.bytes().map(|byte| byte - b'0').collect(),
        })
    }

    pub(crate) fn compare(&self, other: &Self) -> core::cmp::Ordering {
        self.prefix.cmp(&other.prefix).then_with(|| {
            let length = self.fractional.len().max(other.fractional.len());
            (0..length)
                .map(|index| {
                    (
                        self.fractional.get(index).copied().unwrap_or(0),
                        other.fractional.get(index).copied().unwrap_or(0),
                    )
                })
                .find_map(|(left, right)| (left != right).then(|| left.cmp(&right)))
                .unwrap_or(core::cmp::Ordering::Equal)
        })
    }
}

fn parse_fixed_u8(value: &str, width: usize) -> core::result::Result<u8, ()> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    value.parse().map_err(|_| ())
}

fn parse_fixed_u16(value: &str, width: usize) -> core::result::Result<u16, ()> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    value.parse().map_err(|_| ())
}

fn mandate_id_is_valid(value: &str) -> bool {
    value
        .strip_prefix("mandate_")
        .is_some_and(|suffix| Sid::parse(suffix).is_ok())
}

fn grantee_id_is_valid(value: &str) -> bool {
    value
        .strip_prefix("urn:aithos:agent:")
        .is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() && byte != b'#')
        })
}

impl core::str::FromStr for Mandate {
    type Err = Error;

    fn from_str(document: &str) -> Result<Self> {
        let mandate: Self = serde_json::from_str(document)
            .map_err(|error| Error::InvalidMandate(format!("invalid JSON form: {error}")))?;
        mandate.validate_form()?;
        Ok(mandate)
    }
}

pub struct MandateSpec<'a> {
    pub id: String,
    pub subject: String,
    pub grantee_id: String,
    pub grantee_label: String,
    pub grantee_pub: &'a VerifyingKey,
    pub perimeter: Vec<PerimeterEntry>,
    /// Tier V/X/C constraints (§04.4), free-form JSON; counting constraints
    /// are enforced against gamma (§07.4).
    pub constraints: serde_json::Value,
    pub not_before: String,
    pub not_after: String,
    pub issued_at: String,
    pub nonce: String,
}

impl MandateSpec<'_> {
    /// Unconstrained spec default (kept literal at call sites that grant
    /// constraints).
    #[must_use]
    pub fn no_constraints() -> serde_json::Value {
        serde_json::json!({})
    }
}

fn sign_doc(m: &mut Mandate, key: &SigningKey) -> Result<()> {
    m.signature.value = String::new();
    let bytes = jcs::canonical_bytes(m)?;
    m.signature.value = hex::encode(key.sign(&bytes).to_bytes());
    Ok(())
}

pub(crate) fn verify_sig(m: &Mandate, key: &VerifyingKey) -> Result<()> {
    let mut unsigned = m.clone();
    unsigned.signature.value = String::new();
    let sig: [u8; 64] = hex::decode(&m.signature.value)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| Error::InvalidMandate(format!("{}: bad signature encoding", m.id)))?;
    key.verify(
        &jcs::canonical_bytes(&unsigned)?,
        &Signature::from_bytes(&sig),
    )
    .map_err(|_| Error::InvalidMandate(format!("{}: signature does not verify", m.id)))
}

fn grantee_block(spec: &MandateSpec<'_>) -> Grantee {
    Grantee {
        id: spec.grantee_id.clone(),
        label: spec.grantee_label.clone(),
        pubkey: wire::ed25519_pub_to_multibase(&spec.grantee_pub.to_bytes()),
        kex_pubkey: wire::x25519_pub_to_multibase(&ed2x(spec.grantee_pub).to_bytes()),
    }
}

impl Mandate {
    fn validate_form(&self) -> Result<()> {
        let invalid = |reason: &str| Error::InvalidMandate(format!("{}: {reason}", self.id));

        if !matches!(
            self.version.as_str(),
            MANDATE_VERSION_DRAFT1 | MANDATE_VERSION_DRAFT2
        ) {
            return Err(invalid("unsupported mandate profile"));
        }
        if !mandate_id_is_valid(&self.id) {
            return Err(invalid("malformed mandate identifier"));
        }
        if self
            .parent
            .as_deref()
            .is_some_and(|parent| !mandate_id_is_valid(parent))
        {
            return Err(invalid("malformed parent mandate identifier"));
        }
        let subject_key = self
            .subject
            .strip_prefix("did:aithos:")
            .ok_or_else(|| invalid("malformed subject identifier"))?;
        wire::multibase_to_ed25519_pub(subject_key)
            .map_err(|_| invalid("malformed subject identifier"))?;
        if !grantee_id_is_valid(&self.grantee.id) {
            return Err(invalid("malformed grantee identifier"));
        }
        self.grantee_pub()
            .map_err(|_| invalid("malformed grantee signing key"))?;
        let kex = wire::multibase_to_x25519_pub(&self.grantee.kex_pubkey)
            .map_err(|_| invalid("malformed grantee kex key"))?;
        let expected_kex = wire::multibase_to_x25519_pub(&wire::x25519_pub_to_multibase(
            &ed2x(
                &self
                    .grantee_pub()
                    .map_err(|_| invalid("malformed grantee signing key"))?,
            )
            .to_bytes(),
        ))
        .expect("locally encoded X25519 key parses");
        if kex != expected_kex {
            return Err(invalid("grantee kex binding mismatch"));
        }
        if self.nonce.is_empty() {
            return Err(invalid("nonce is empty"));
        }
        if !self.constraints.is_object() {
            return Err(invalid("constraints is not an object"));
        }
        crate::constraints::validate_root_constraints(&self.constraints)?;
        for entry in &self.perimeter {
            PerimeterEntry::parse(entry).map_err(|_| invalid("malformed perimeter entry"))?;
        }

        let not_before = CanonicalTimestamp::parse(&self.not_before)
            .map_err(|()| invalid("not_before is not an RFC 3339 Z instant"))?;
        let not_after = CanonicalTimestamp::parse(&self.not_after)
            .map_err(|()| invalid("not_after is not an RFC 3339 Z instant"))?;
        CanonicalTimestamp::parse(&self.issued_at)
            .map_err(|()| invalid("issued_at is not an RFC 3339 Z instant"))?;
        if not_before.compare(&not_after).is_gt() {
            return Err(invalid("validity window is inverted"));
        }
        if self.signature.alg != "ed25519" {
            return Err(invalid("signature algorithm is not ed25519"));
        }
        let signature_length = hex::decode(&self.signature.value)
            .map_err(|_| invalid("malformed signature value"))?
            .len();
        if signature_length != 64 {
            return Err(invalid("malformed signature value"));
        }

        match &self.parent {
            None => {
                if self.issued_by != format!("{}#root", self.subject) {
                    return Err(invalid("root issuer differs from subject root"));
                }
                if self.signature.key != "#root" {
                    return Err(invalid("root announced signer key is not #root"));
                }
            }
            Some(_) => {
                wire::multibase_to_ed25519_pub(&self.issued_by)
                    .map_err(|_| invalid("child issuer is not an Ed25519 key"))?;
                if self.signature.key != self.issued_by {
                    return Err(invalid("child announced signer key differs from issuer"));
                }
            }
        }
        Ok(())
    }

    /// Root mandate: issued and signed by the owner's root key (§04.1).
    pub fn build_root(root_sign: &SigningKey, spec: &MandateSpec<'_>) -> Result<Self> {
        Self::build_root_with_version(root_sign, MANDATE_VERSION, spec)
    }

    /// Root mandate under an explicitly selected supported profile.
    ///
    /// Normal issuance uses [`Self::build_root`] and the current profile. This
    /// entry point exists for byte-exact historical replay and explicit
    /// reissuance migrations.
    pub fn build_root_with_version(
        root_sign: &SigningKey,
        version: &str,
        spec: &MandateSpec<'_>,
    ) -> Result<Self> {
        if !matches!(version, MANDATE_VERSION_DRAFT1 | MANDATE_VERSION_DRAFT2) {
            return Err(Error::InvalidMandate(format!(
                "unsupported mandate profile {version}"
            )));
        }
        let mut m = Mandate {
            version: version.to_owned(),
            id: spec.id.clone(),
            subject: spec.subject.clone(),
            parent: None,
            issued_by: format!("{}#root", spec.subject),
            grantee: grantee_block(spec),
            perimeter: spec
                .perimeter
                .iter()
                .map(PerimeterEntry::to_entry_string)
                .collect(),
            constraints: spec.constraints.clone(),
            not_before: spec.not_before.clone(),
            not_after: spec.not_after.clone(),
            issued_at: spec.issued_at.clone(),
            nonce: spec.nonce.clone(),
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: "#root".to_owned(),
                value: String::new(),
            },
        };
        sign_doc(&mut m, root_sign)?;
        Ok(m)
    }

    /// Sub-mandate: minted and signed by the parent's grantee key (§05.2).
    pub fn build_sub(
        parent: &Mandate,
        parent_sk: &SigningKey,
        spec: &MandateSpec<'_>,
    ) -> Result<Self> {
        let mut m = Mandate {
            version: parent.version.clone(),
            id: spec.id.clone(),
            subject: spec.subject.clone(),
            parent: Some(parent.id.clone()),
            issued_by: parent.grantee.pubkey.clone(),
            grantee: grantee_block(spec),
            perimeter: spec
                .perimeter
                .iter()
                .map(PerimeterEntry::to_entry_string)
                .collect(),
            constraints: spec.constraints.clone(),
            not_before: spec.not_before.clone(),
            not_after: spec.not_after.clone(),
            issued_at: spec.issued_at.clone(),
            nonce: spec.nonce.clone(),
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: parent.grantee.pubkey.clone(),
                value: String::new(),
            },
        };
        sign_doc(&mut m, parent_sk)?;
        Ok(m)
    }

    /// Re-sign after amendment (re-issuance path, §04.1 widening note).
    pub fn resign(&mut self, key: &SigningKey) -> Result<()> {
        sign_doc(self, key)
    }

    pub fn parsed_perimeter(&self) -> Result<Vec<PerimeterEntry>> {
        self.perimeter
            .iter()
            .map(|s| PerimeterEntry::parse(s))
            .collect()
    }

    pub fn grantee_pub(&self) -> Result<VerifyingKey> {
        let bytes = wire::multibase_to_ed25519_pub(&self.grantee.pubkey)?;
        VerifyingKey::from_bytes(&bytes)
            .map_err(|_| Error::InvalidMandate(format!("{}: malformed grantee key", self.id)))
    }

    /// The kex binding is checked, never trusted (§04.1).
    fn check_kex(&self) -> Result<()> {
        let expected = wire::x25519_pub_to_multibase(&ed2x(&self.grantee_pub()?).to_bytes());
        if self.grantee.kex_pubkey != expected {
            return Err(Error::InvalidMandate(format!(
                "{}: kex_pubkey does not match ed2x(pubkey)",
                self.id
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- chains

/// Offline chain verification (§04.5 + §05.3) at injected time `at`, with no
/// revocation state — structural, window, and attenuation checks only.
pub fn verify_chain(chain: &[Mandate], did_doc: &DidDocument, at: &str) -> Result<()> {
    verify_chain_revocable(chain, did_doc, at, &[])
}

/// Full offline verification (§04.5, all four steps) including revocation
/// (step 4): none of the chain's ids is revoked at `T` (§06.4). Revocation
/// state is injected — the pure core never reads the log itself.
pub fn verify_chain_revocable(
    chain: &[Mandate],
    did_doc: &DidDocument,
    at: &str,
    revocations: &[crate::revocation::Revocation],
) -> Result<()> {
    let err = |m: String| Error::InvalidMandate(m);
    if chain.is_empty() {
        return Err(err("empty chain".into()));
    }
    for mandate in chain {
        mandate.validate_form()?;
    }
    let at_timestamp = CanonicalTimestamp::parse(at).map_err(|()| {
        err(format!(
            "verification time is not an RFC 3339 Z instant: {at}"
        ))
    })?;
    crate::revocation::chain_revoked_at(chain, revocations, at)?;
    did_doc.verify()?;

    // Root link.
    let root = &chain[0];
    if root.parent.is_some() {
        return Err(err(format!(
            "{}: root mandate must have no parent",
            root.id
        )));
    }
    if root.subject != did_doc.id || root.issued_by != format!("{}#root", did_doc.id) {
        return Err(err(format!(
            "{}: not issued by the subject's root",
            root.id
        )));
    }
    let root_key_bytes = wire::multibase_to_ed25519_pub(&did_doc.keys.root)?;
    let root_key =
        VerifyingKey::from_bytes(&root_key_bytes).map_err(|_| err("malformed root key".into()))?;
    verify_sig(root, &root_key)?;

    for (i, m) in chain.iter().enumerate() {
        m.check_kex()?;
        // Window at T, for every mandate in the chain (§04.5 step 3).
        let not_before = CanonicalTimestamp::parse(&m.not_before)
            .expect("form-valid not_before was parsed above");
        let not_after =
            CanonicalTimestamp::parse(&m.not_after).expect("form-valid not_after was parsed above");
        if at_timestamp.compare(&not_before).is_lt() || at_timestamp.compare(&not_after).is_gt() {
            return Err(err(format!("{}: outside validity window at {at}", m.id)));
        }
        if i == 0 {
            continue;
        }
        let parent = &chain[i - 1];
        if m.version != parent.version {
            return Err(err(format!(
                "{}: mandate profile differs from its parent",
                m.id
            )));
        }
        // Link identity (§05.3 rule 5).
        if m.parent.as_deref() != Some(parent.id.as_str()) {
            return Err(err(format!("{}: parent id mismatch", m.id)));
        }
        if m.subject != parent.subject {
            return Err(err(format!("{}: subject changes along the chain", m.id)));
        }
        if m.issued_by != parent.grantee.pubkey {
            return Err(err(format!("{}: not issued by its parent's grantee", m.id)));
        }
        if m.grantee.pubkey == m.issued_by {
            return Err(err(format!("{}: self-issued sub-mandate", m.id)));
        }
        verify_sig(m, &parent.grantee_pub()?)?;
        // Window containment (§05.3 rule 2).
        if m.not_before < parent.not_before || m.not_after > parent.not_after {
            return Err(err(format!("{}: window exceeds its parent's", m.id)));
        }
        // Constraint monotonicity, the WHOLE §04.4 vocabulary (§05.3 rule 3,
        // E+ vector): typed validation of both sides, per-family fail-closed
        // containment, unknown keys rejected both ways (M0.c). Subsumes the
        // historic windows (§04.10) and obligations (§04.12) gates.
        crate::constraints::constraints_attenuate_for_profile(
            &parent.version,
            &parent.constraints,
            &m.constraints,
            &m.not_before,
            &m.not_after,
        )
        .map_err(|e| err(format!("{}: {e}", m.id)))?;
        // Issuing right and depth (§05.1, §05.3 rule 4).
        let parent_perimeter = parent.parsed_perimeter()?;
        let parent_depth = parent_perimeter
            .iter()
            .find_map(|e| match e {
                PerimeterEntry::Issue { depth } => Some(*depth),
                _ => None,
            })
            .ok_or_else(|| err(format!("{}: parent grants no issue right", m.id)))?;
        // Perimeter containment (§05.3 rule 1).
        for child_entry in m.parsed_perimeter()? {
            let ok = match &child_entry {
                PerimeterEntry::Issue { depth } => *depth < parent_depth,
                other => parent_perimeter.iter().any(|p| p.covers(other)),
            };
            if !ok {
                return Err(err(format!(
                    "{}: entry '{}' exceeds the parent perimeter",
                    m.id,
                    child_entry.to_entry_string()
                )));
            }
        }
    }
    Ok(())
}

/// Full verifier front door: chain valid at `at` AND the leaf covers `op`.
pub fn verify_op(chain: &[Mandate], did_doc: &DidDocument, at: &str, op: &Op<'_>) -> Result<()> {
    verify_chain(chain, did_doc, at)?;
    let leaf = chain.last().expect("non-empty");
    if !covers_op(&leaf.parsed_perimeter()?, op) {
        return Err(Error::InvalidMandate(format!(
            "{}: operation not covered by the leaf perimeter",
            leaf.id
        )));
    }
    Ok(())
}
