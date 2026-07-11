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

    #[error("invalid multibase key encoding: {0}")]
    InvalidMultibase(String),

    #[error("canonicalization failed: {0}")]
    Canonicalization(String),

    #[error("invalid DID document: {0}")]
    InvalidDidDocument(String),

    #[error("invalid epoch transition: {0}")]
    InvalidEpochTransition(String),

    #[error("seal rejected: {0}")]
    SealRejected(String),

    #[error("invalid mandate: {0}")]
    InvalidMandate(String),

    #[error("I3 violated — header without an owner line: {0}")]
    MissingOwnerLine(String),

    #[error("invalid gamma entry: {0}")]
    InvalidGammaEntry(String),

    #[error("invalid gamma chain: {0}")]
    InvalidGammaChain(String),

    #[error("gamma budget exhausted: {0}")]
    GammaBudgetExhausted(String),

    #[error("I5 violated — grant never logged: {0}")]
    GammaGrantNotLogged(String),

    #[error("heartbeat stale — owner silent beyond every+grace: {0}")]
    GammaHeartbeatStale(String),

    #[error("obligation unsatisfied — no valid receipt discharges the gate: {0}")]
    GammaObligationUnsatisfied(String),

    #[error("merkle proof invalid: {0}")]
    MerkleProofInvalid(String),

    #[error("merkle root mismatch: {0}")]
    MerkleRootMismatch(String),

    #[error("stale freshness anchor: {0}")]
    GammaStaleAnchor(String),

    #[error("mandate revoked: {0}")]
    MandateRevoked(String),

    #[error("revocation rejected — signer lacks authority: {0}")]
    GammaRevocationRejected(String),
}

pub type Result<T> = core::result::Result<T, Error>;
