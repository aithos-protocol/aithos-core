#![forbid(unsafe_code)]
//! # aithos-core
//!
//! Pure protocol logic for Aithos Core (`aithos-core: 1.0.0-draft.1`).
//!
//! **Layering rule (normative for this workspace):** this crate performs no I/O,
//! reads no clock, opens no socket. Time `T`, randomness, and storage are always
//! injected by the caller — which is what makes every operation deterministic,
//! replayable against the `vectors/` contract, and compilable to WASM unchanged.
//!
//! Module map → spec chapters:
//! - [`derive`], [`keys`] → 01 (identity & keys)
//! - [`ids`], [`path`]    → 02 (content tree: sids, canonical sid-paths)
//! - `header`  (todo)     → 03 (headers, lines, rotation, up-link wrap)
//! - `mandate` (todo)     → 04/05 (certificates, perimeter algebra, verifier)
//! - `revoke`  (todo)     → 06 (revocation entries, ladder)
//! - `gamma`   (todo)     → 07 (log entries, counting)
//! - `merkle`  (todo)     → 02.10 (state roots, inclusion proofs)

pub mod catalog;
pub mod carriers;
pub mod constraints;
pub mod delegated_counts;
pub mod derive;
pub mod did;
pub mod error;
pub mod gamma;
pub mod gamma_replay;
pub mod gamma_v2;
pub mod header;
pub mod ids;
pub mod jcs;
pub mod keys;
pub mod mandate;
pub mod merkle;
pub mod operation;
pub mod path;
pub mod receipts;
pub mod revocation;
pub mod seal;
pub mod wire;

pub use error::{Error, Result};
