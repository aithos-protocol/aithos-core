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

use crate::{GatewayError, Result};

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

/// On-disk identity file — RUNNER custody only, never inside an ethos
/// store (a store may one day be cloud-replicated; this file must not).
#[derive(serde::Serialize, serde::Deserialize)]
struct IdentityFile {
    agent_seed_hex: String,
    gateway_seed_hex: String,
}

impl Keyholder {
    /// Persist to the runner's identity file (0600 on unix).
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let body = serde_json::to_vec_pretty(&IdentityFile {
            agent_seed_hex: hex::encode(self.agent_seed),
            gateway_seed_hex: hex::encode(self.gateway_seed),
        })
        .map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        }
        std::fs::write(path, body).map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        }
        Ok(())
    }

    /// Load the runner's identity file. Absent or malformed = fail closed.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| GatewayError::IdentityUnavailable(format!("{}: {e}", path.display())))?;
        let f: IdentityFile = serde_json::from_slice(&bytes)
            .map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        Ok(Self {
            agent_seed: decode_seed(&f.agent_seed_hex)?,
            gateway_seed: decode_seed(&f.gateway_seed_hex)?,
        })
    }
}

fn decode_seed(hex_str: &str) -> Result<[u8; 32]> {
    hex::decode(hex_str)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| GatewayError::IdentityUnavailable("seed is not 32 hex bytes".into()))
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
