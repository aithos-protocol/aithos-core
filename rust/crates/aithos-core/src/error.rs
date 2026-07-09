use thiserror::Error;

/// Fail-closed: every rejection is an explicit, named variant.
/// A verifier that cannot positively validate MUST return one of these —
/// there is no "lenient" mode anywhere in the protocol.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("invalid seed length: expected 32 bytes, got {0}")]
    InvalidSeedLength(usize),

    #[error("invalid sid: {0}")]
    InvalidSid(String),

    #[error("invalid name (want [a-z0-9_-]{{1,64}}): {0}")]
    InvalidName(String),

    #[error("invalid tag (want [a-z0-9_-]{{1,64}}): {0}")]
    InvalidTag(String),

    #[error("invalid node path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = core::result::Result<T, Error>;
