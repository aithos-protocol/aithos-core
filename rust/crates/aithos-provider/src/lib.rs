#![forbid(unsafe_code)]
//! # aithos-provider
//!
//! Server side of the Aithos provider (piste P — `INFRA-PROVIDER.md`,
//! annexes normatives A/B/C). One crate, three binaries at term
//! (`aithos-store-api`, `aithos-relay`, `aithos-witness`); lot P1 ships the
//! store skeleton only.
//!
//! **Doctrine (opposable):** this crate moves bytes and verifies signatures.
//! It never holds a client secret, never unseals anything, never signs on a
//! client's behalf, and never decides — its `covers()` is anti-abuse, the
//! authority always lives in the client-side chain. Fail-closed everywhere:
//! the first failing check answers, nothing else is evaluated (annexe A.2).
//!
//! Module map → annexe sections:
//! - [`envelope`] — `X-Aithos-Auth`, normative order A.2 #2–#10
//! - [`pathmap`]  — route/object grammar A.1 (#0) and `covers()` A.3 (#10)
//! - [`nonces`]   — anti-rejeu reservation A.2 #6 (DynamoDB TTL / memory)
//! - [`control`]  — tenant read-model (#1) — static bootstrap until P7
//! - [`objects`]  — object store seam — memory until the S3 layout (P2)
//! - [`acme`]     — the delegated DNS-01 surface `/acme/txt` (B.5, M2)
//! - [`dns`]      — TXT record seam (Route 53 / memory) consumed by B.5
//! - [`keepalive`]— TCP keepalive on the tunnel socket (redline B.3, M2)
//! - [`redact`]   — log discipline A.8 (registre fermé, façon `credentials.rs`)
//! - [`service`]  — the axum surface consumed by `bin/store_api` and the tests
//! - [`time`]     — strict RFC 3339 Zulu parsing (the annexes' instants)
//!
//! P2 (étape 4) adds [`artifacts`] — the A.4 deposit verification,
//! composing core/bundle — and [`heads`] — the A.5 heads table, whose
//! opaque CAS is the only serialization point (DynamoDB behind the seam
//! at étape 6). P5 `witness` (annexe C); P6 `tunnel` (annexe B). The
//! arborescence cible is in `docs/HANDOFF-PROVIDER-AWS.md`.

pub mod acme;
pub mod artifacts;
pub mod control;
pub mod dns;
pub mod envelope;
pub mod heads;
pub mod keepalive;
pub mod nonces;
pub mod objects;
pub mod passthrough;
pub mod pathmap;
pub mod redact;
pub mod relay;
pub mod service;
pub mod sni;
pub mod time;
pub mod tls;
pub mod tunnel;
pub mod witness;

/// The wire version every response carries (annexe A.1). Any breaking
/// change bumps the draft and opens a double-service period (§8).
pub const STORE_WIRE_VERSION: &str = "1.0.0-draft.1";
