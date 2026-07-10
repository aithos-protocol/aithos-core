//! The keyholder: owns the signing seeds inside the gateway process.
//!
//! Two seeds live here — the **agent's** (signs acts under the agent
//! mandate) and the **gateway's own** (signs governance acts such as
//! refusals). Two rules, both load-bearing for the whole product:
//! - nothing in here is ever serialised towards the agent — the LLM
//!   produces intentions, never signatures;
//! - only `core_bridge` may borrow the seeds (pub(crate)), to sign gamma
//!   entries with the kind the *operation* imposes.
//!
//! Entropy is injected (the core's purity rule extends here): the binary
//! passes OS randomness, tests pass a deterministic source.

use zeroize::Zeroize;

/// Holds the gateway-side seeds; zeroised on drop.
pub struct Keyholder {
    agent_seed: [u8; 32],
    gateway_seed: [u8; 32],
}

impl Keyholder {
    /// Build from injected entropy.
    pub fn from_entropy(agent_seed: [u8; 32], gateway_seed: [u8; 32]) -> Self {
        Self {
            agent_seed,
            gateway_seed,
        }
    }

    /// The agent seed — crate-private: only the core bridge signs.
    pub(crate) fn agent_seed(&self) -> &[u8; 32] {
        &self.agent_seed
    }

    /// The gateway's own seed — crate-private: only the core bridge signs.
    pub(crate) fn gateway_seed(&self) -> &[u8; 32] {
        &self.gateway_seed
    }
}

impl Drop for Keyholder {
    fn drop(&mut self) {
        self.agent_seed.zeroize();
        self.gateway_seed.zeroize();
    }
}

impl std::fmt::Debug for Keyholder {
    /// Never print key material, even in debug output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keyholder(<sealed>)")
    }
}
