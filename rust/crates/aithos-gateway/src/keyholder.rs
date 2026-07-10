//! The keyholder: owns the agent's signing seed inside the gateway process.
//!
//! Two rules, both load-bearing for the whole product:
//! - nothing in here is ever serialised towards the agent — the LLM
//!   produces intentions, never signatures;
//! - only `core_bridge` may borrow the seed (pub(crate)), to sign gamma
//!   entries with the kind the *operation* imposes.
//!
//! Entropy is injected (the core's purity rule extends here): the binary
//! passes OS randomness, tests pass fixed bytes.

use zeroize::Zeroize;

/// Holds the agent seed; zeroised on drop.
pub struct Keyholder {
    agent_seed: [u8; 32],
}

impl Keyholder {
    /// Build from injected entropy (32 bytes of caller-provided randomness).
    pub fn from_entropy(entropy: [u8; 32]) -> Self {
        Self { agent_seed: entropy }
    }

    /// The agent seed — crate-private: only the core bridge signs.
    #[allow(dead_code)] // consumed by core_bridge when the audit MVP lands
    pub(crate) fn agent_seed(&self) -> &[u8; 32] {
        &self.agent_seed
    }
}

impl Drop for Keyholder {
    fn drop(&mut self) {
        self.agent_seed.zeroize();
    }
}

impl std::fmt::Debug for Keyholder {
    /// Never print key material, even in debug output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keyholder(<sealed>)")
    }
}
