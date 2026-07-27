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

    /// Run one closed `aithos-client` operation under the Gateway capability
    /// named by delegated OAuth session leaves.
    ///
    /// The temporary client keyholder cannot escape this call: callers only
    /// receive `T`, never the handle or seed. `aithos-client` itself exposes
    /// purpose-bound session/content/publication/provider operations rather
    /// than an arbitrary `sign(bytes)` primitive.
    pub(crate) fn with_ethos_client_grantee<T>(
        &self,
        operation: impl FnOnce(&aithos_client::MemoryGranteeKeyholder) -> T,
    ) -> T {
        let keyholder = aithos_client::MemoryGranteeKeyholder::from_seed(self.gateway_seed);
        operation(&keyholder)
    }

    /// Open one authenticated client session and keep both the temporary
    /// keyholder and session proof inside this custody boundary.
    pub(crate) fn with_ethos_client_session<T>(
        &self,
        snapshot: aithos_client::VerifiedSnapshot,
        chain: Vec<aithos_core::mandate::Mandate>,
        at: &str,
        nonce: [u8; 32],
        operation: impl FnOnce(
            &aithos_client::GranteeSession<aithos_client::MemoryGranteeKeyholder>,
        ) -> std::result::Result<T, aithos_client::ClientError>,
    ) -> std::result::Result<T, aithos_client::ClientError> {
        let keyholder = aithos_client::MemoryGranteeKeyholder::from_seed(self.gateway_seed);
        let session = aithos_client::GranteeSession::open(
            snapshot,
            keyholder,
            chain,
            aithos_client::SessionContext::new(at, nonce),
        )?;
        operation(&session)
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

#[cfg(test)]
mod tests {
    use super::*;
    use aithos_client::{
        ArtifactSnapshot, GenesisEntropy, GenesisIntent, GenesisPlan, Keyholder as _,
        MemoryGenesisKeyholder, MutationGrantIntent, MutationGrantTarget, MutationIntent,
        ProviderEnvelopePlan, ProviderUploadIntent, PublicationEntropy, PublicationPlan,
    };
    use aithos_core::mandate::Verb;
    use aithos_core::path::Zone;
    use ed25519_dalek::SigningKey;

    #[test]
    fn ethos_client_grantee_is_operation_scoped_and_matches_the_gateway_leaf() {
        let gateway = Keyholder::from_entropy([0x31; 32], [0x32; 32]);
        let expected = aithos_core::wire::ed25519_pub_to_multibase(
            &SigningKey::from_bytes(gateway.gateway_seed())
                .verifying_key()
                .to_bytes(),
        );

        let public = gateway
            .with_ethos_client_grantee(|client| client.public_keys().expect("client public keys"));

        assert_eq!(public.signing, expected);
        assert_eq!(format!("{gateway:?}"), "Keyholder(<sealed>)");
    }

    #[test]
    fn gateway_custody_builds_a_verified_circle_publication_and_envelopes_offline() {
        let gateway = Keyholder::from_entropy([0xb2; 32], [0xb4; 32]);
        gateway.with_ethos_client_grantee(|grantee| {
            let owner = MemoryGenesisKeyholder::from_entropy([0xb1; 32], [0xb3; 32])
                .expect("owner keyholder");
            let genesis = GenesisPlan::build(
                &owner,
                GenesisIntent::new(
                    "2026-07-25T06:00:00Z",
                    "guide/welcome",
                    "Welcome",
                    "# Welcome\n",
                ),
                GenesisEntropy::new([0x11; 16], [0x12; 16]),
            )
            .expect("genesis");
            let snapshot = ArtifactSnapshot::try_from_iter(
                genesis
                    .artifacts()
                    .iter()
                    .map(|(path, bytes)| (path.clone(), bytes.clone())),
            )
            .expect("snapshot")
            .cold_verify()
            .expect("verified snapshot");
            let grant = PublicationPlan::build_mutation_grant_owner(
                &owner,
                grantee,
                snapshot,
                MutationGrantIntent::new(
                    Zone::Circle,
                    Verb::Append,
                    MutationGrantTarget::Zone,
                    "gateway-writer",
                    "2026-07-25T05:59:00Z",
                    "2026-07-26T00:00:00Z",
                    "2026-07-25T06:01:00Z",
                ),
                PublicationEntropy::new([0x13; 16], [0x14; 16]),
            )
            .expect("published grant");
            let plan = PublicationPlan::build_grantee(
                grantee,
                grant.chain(),
                grant.publication().cold_verify().expect("grant snapshot"),
                MutationIntent::create(
                    Zone::Circle,
                    "dry-run",
                    "opaque delegated body",
                    "2026-07-25T06:02:00Z",
                ),
                PublicationEntropy::new([0x15; 16], [0x16; 16]),
            )
            .expect("delegated plan");
            plan.cold_verify().expect("publication cold verifies");

            for (index, path) in plan.upload_order().iter().enumerate() {
                let envelope = ProviderEnvelopePlan::for_grantee_publication(
                    grantee,
                    &plan,
                    ProviderUploadIntent::new(
                        "store.aithos.fr",
                        "demo",
                        path,
                        "2026-07-25T06:03:00Z",
                        [(index as u8).saturating_add(1); 16],
                    ),
                )
                .expect("closed provider envelope");
                assert_eq!(envelope.method(), "PUT");
                assert_eq!(envelope.body(), &plan.artifacts()[path][..]);
                assert!(!envelope.header_value().is_empty());
            }
        });
    }
}
