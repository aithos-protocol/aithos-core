//! The one thin layer between the gateway and the trust engine.
//!
//! **Only this module (and `store_adapter`) imports `aithos-core` /
//! `aithos-bundle`.** The rest of the gateway speaks in tool names, op
//! strings and outcomes; this bridge translates those into mandate
//! verification (`verify_op`), gamma appends (kind imposed by the
//! operation, never by the caller) and scoped audit reads (`read.gamma`).
//! When the core's API moves, this file absorbs the change.
//!
//! Skeleton for the scaffold commit — lands with the audit MVP.

use crate::config::GatewayConfig;
use crate::keyholder::Keyholder;
use crate::store_adapter::GatewayStore;
use crate::Result;

/// What onboarding hands back to the operator (never key material).
#[derive(Debug)]
pub struct OnboardOutcome {
    /// The owner's DID (the enterprise identity anchoring the ethos).
    pub owner_did: String,
    /// The agent-facing endpoint to configure in the agent runtime.
    pub agent_endpoint: String,
    /// Where the ethos lives (for the operator's records).
    pub store_summary: String,
}

/// Live bridge: the mandate chain, the ethos store and the keyholder,
/// assembled and ready to authorise, log and export.
pub struct Bridge {
    _store: GatewayStore,
    _keyholder: Keyholder,
}

impl Bridge {
    /// Onboard: initialise the ethos, mint the agent identity, grant the
    /// read-only mandate derived from the tool map, and grant the scoped
    /// auditor mandate. One command, minutes not days.
    pub fn onboard(
        _cfg: &GatewayConfig,
        _keyholder: Keyholder,
        _entropy: [u8; 32],
        _now: &str,
    ) -> Result<(Self, OnboardOutcome)> {
        todo!("audit MVP: init bundle + agent identity + read-only mandate + audit grant")
    }

    /// Is `op` covered by the agent's mandate chain at `now`?
    /// Fail-closed: any core rejection surfaces as a denial.
    pub fn authorize(&self, _op: &str, _now: &str) -> Result<()> {
        todo!("audit MVP: verify_op against the mandate chain")
    }

    /// Append one act entry to the gamma for an authorised call, via the
    /// agent's mandate chain. The kind is imposed by the operation
    /// mapping, never by the caller. Returns the entry id.
    pub fn record_act(&mut self, _tool: &str, _now: &str) -> Result<String> {
        todo!("audit MVP: gamma append signed by the gateway-held agent key")
    }

    /// Append one refusal entry. A refusal is not an act of the agent —
    /// the agent did not act — but a governance act of the gateway's own
    /// identity, under the gateway's own mandate.
    pub fn record_refusal(&mut self, _tool: &str, _reason: &str, _now: &str) -> Result<String> {
        todo!("audit MVP: governance act via the gateway's own mandate")
    }

    /// Export the audit slice the auditor's `read.gamma` mandate covers.
    pub fn export_audit(&self, _now: &str) -> Result<String> {
        todo!("audit MVP: LogFilter-scoped read for the auditor mandate")
    }
}
