//! Stable identifiers and display names (spec §02.2).

use crate::error::{Error, Result};
use core::fmt;
use core::str::FromStr;

/// Stable identifier of a folder or section: a ULID, assigned at creation,
/// **never changed**. Sids are the derivation labels and blob filenames;
/// names are metadata (§02.2), so renaming never re-keys anything (§02.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sid(pub ulid::Ulid);

impl Sid {
    pub fn parse(s: &str) -> Result<Self> {
        ulid::Ulid::from_string(s)
            .map(Sid)
            .map_err(|_| Error::InvalidSid(s.to_owned()))
    }
}

impl fmt::Display for Sid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Sid {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Sid::parse(s)
    }
}

/// Validate a human display name or tag: `[a-z0-9_-]{1,64}`, unique among
/// siblings (uniqueness is checked at the index layer, not here).
pub fn validate_name(s: &str) -> Result<()> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidName(s.to_owned()))
    }
}

/// Tags share the name alphabet.
pub fn validate_tag(s: &str) -> Result<()> {
    validate_name(s).map_err(|_| Error::InvalidTag(s.to_owned()))
}
