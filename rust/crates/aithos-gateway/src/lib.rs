//! aithos-gateway — the containerised runner's gateway.
//!
//! Interposes between an existing agent and its external dependencies
//! (MCP tools first; LLM API and web egress in later iterations), applies
//! the agent's mandate on every call, writes one gamma entry per act, and
//! holds the keys the agent never sees.
//!
//! Layering rule: **only [`core_bridge`] (and its [`store_adapter`] seam)
//! imports from `aithos-core` / `aithos-bundle`.** Everything else is
//! core-agnostic plumbing, so protocol API drift is absorbed in one place.
//!
//! Posture: fail-closed everywhere. Any policy ambiguity is a refusal,
//! and every refusal is logged.

pub mod config;
pub mod control;
pub mod core_bridge;
pub mod credentials;
pub mod demo_lea;
pub mod hub;
pub mod keyholder;
pub mod oauth;
pub mod policy;
pub mod proxy_llm;
pub mod proxy_mcp;
pub mod proxy_web;
pub mod public_tls;
pub mod relay;
pub mod relay_application;
pub mod store_adapter;
pub mod upstream_oauth;

mod error;

pub use error::GatewayError;

/// Gateway-wide result type.
pub type Result<T> = std::result::Result<T, GatewayError>;
