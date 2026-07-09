//! Canonical sid-paths of nodes (spec §02.1).
//!
//! `/e/<zone>` — zone root folder; then `d/<sid>` folder segments, terminated
//! optionally by `s/<sid>` (section) or `t/<tag>` (tag view). Marker segments
//! `d`/`t`/`s` give domain separation; human names never appear here.

use crate::error::{Error, Result};
use crate::ids::{validate_tag, Sid};
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Public,
    Circle,
    /// The `self` zone (`Self` is a Rust keyword).
    Self_,
}

impl Zone {
    pub fn as_str(&self) -> &'static str {
        match self {
            Zone::Public => "public",
            Zone::Circle => "circle",
            Zone::Self_ => "self",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "public" => Ok(Zone::Public),
            "circle" => Ok(Zone::Circle),
            "self" => Ok(Zone::Self_),
            other => Err(Error::InvalidPath(format!("unknown zone: {other}"))),
        }
    }
}

/// Terminal segment of a node path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Leaf {
    /// The node is the folder itself.
    Folder,
    /// A section under the folder: `s/<sid>`.
    Section(Sid),
    /// A tag view anchored at the folder: `t/<tag>` (§02.9).
    TagView(String),
}

/// A canonical node path: zone root, folder spine, optional terminal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodePath {
    pub zone: Zone,
    /// Folder sids from the zone root downward (may be empty = the root).
    pub folders: Vec<Sid>,
    pub leaf: Leaf,
}

impl NodePath {
    pub fn zone_root(zone: Zone) -> Self {
        NodePath { zone, folders: vec![], leaf: Leaf::Folder }
    }

    pub fn folder(zone: Zone, folders: Vec<Sid>) -> Self {
        NodePath { zone, folders, leaf: Leaf::Folder }
    }

    pub fn section(zone: Zone, folders: Vec<Sid>, sid: Sid) -> Self {
        NodePath { zone, folders, leaf: Leaf::Section(sid) }
    }

    pub fn tag_view(zone: Zone, folders: Vec<Sid>, tag: &str) -> Result<Self> {
        validate_tag(tag)?;
        Ok(NodePath { zone, folders, leaf: Leaf::TagView(tag.to_owned()) })
    }

    /// Parse a canonical path like `/e/circle/d/<sid>/d/<sid>/s/<sid>`.
    pub fn parse(s: &str) -> Result<Self> {
        let err = |m: &str| Error::InvalidPath(format!("{m}: {s}"));
        let mut it = s.split('/');
        if it.next() != Some("") || it.next() != Some("e") {
            return Err(err("must start with /e/<zone>"));
        }
        let zone = Zone::parse(it.next().ok_or_else(|| err("missing zone"))?)?;
        let mut folders = Vec::new();
        let mut leaf = Leaf::Folder;
        while let Some(marker) = it.next() {
            if !matches!(leaf, Leaf::Folder) {
                return Err(err("segments after terminal"));
            }
            let value = it.next().ok_or_else(|| err("dangling marker"))?;
            match marker {
                "d" => folders.push(Sid::parse(value)?),
                "s" => leaf = Leaf::Section(Sid::parse(value)?),
                "t" => {
                    validate_tag(value)?;
                    leaf = Leaf::TagView(value.to_owned());
                }
                other => return Err(err(&format!("unknown marker '{other}'"))),
            }
        }
        Ok(NodePath { zone, folders, leaf })
    }

    /// True iff `self` is `other` or an ancestor of it (segment-list
    /// containment, §04.2 — `a/b` covers `a/b/c`, never `a/bc`).
    pub fn covers(&self, other: &NodePath) -> bool {
        if self.zone != other.zone || !matches!(self.leaf, Leaf::Folder) {
            return self == other;
        }
        other.folders.len() >= self.folders.len()
            && other.folders[..self.folders.len()] == self.folders[..]
    }
}

impl fmt::Display for NodePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/e/{}", self.zone.as_str())?;
        for sid in &self.folders {
            write!(f, "/d/{sid}")?;
        }
        match &self.leaf {
            Leaf::Folder => Ok(()),
            Leaf::Section(sid) => write!(f, "/s/{sid}"),
            Leaf::TagView(tag) => write!(f, "/t/{tag}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(n: u128) -> Sid {
        Sid(ulid::Ulid::from(n))
    }

    #[test]
    fn parse_display_roundtrip() {
        for raw in [
            "/e/circle",
            &format!("/e/circle/d/{}", sid(1)),
            &format!("/e/self/d/{}/d/{}/s/{}", sid(1), sid(2), sid(3)),
            &format!("/e/public/d/{}/t/toto", sid(9)),
            "/e/circle/t/toto",
        ] {
            let p = NodePath::parse(raw).unwrap();
            assert_eq!(p.to_string(), *raw);
        }
    }

    #[test]
    fn rejects_malformed_paths() {
        for raw in [
            "/e/nowhere",                          // unknown zone
            "/e/circle/x/abc",                     // unknown marker
            "/e/circle/d",                         // dangling marker
            &format!("/e/circle/s/{}/d/{}", sid(1), sid(2)), // segment after terminal
            "/e/circle/t/Bad Tag",                 // invalid tag
            "e/circle",                            // missing leading slash
        ] {
            assert!(NodePath::parse(raw).is_err(), "should reject: {raw}");
        }
    }

    #[test]
    fn covers_is_segment_list_containment() {
        let a = NodePath::folder(Zone::Circle, vec![sid(1)]);
        let ab = NodePath::folder(Zone::Circle, vec![sid(1), sid(2)]);
        let ab_sec = NodePath::section(Zone::Circle, vec![sid(1), sid(2)], sid(7));
        let other_zone = NodePath::folder(Zone::Self_, vec![sid(1)]);

        assert!(a.covers(&a));
        assert!(a.covers(&ab));
        assert!(a.covers(&ab_sec));
        assert!(!ab.covers(&a)); // child never covers parent
        assert!(!a.covers(&other_zone)); // zones never cross

        // §04.2: containment by segment list, never by string prefix —
        // distinct sids at the same level do not cover each other.
        let ac = NodePath::folder(Zone::Circle, vec![sid(1), sid(3)]);
        assert!(!ab.covers(&ac));

        // A non-folder node covers only itself.
        assert!(ab_sec.covers(&ab_sec));
        assert!(!ab_sec.covers(&ab));
    }
}
