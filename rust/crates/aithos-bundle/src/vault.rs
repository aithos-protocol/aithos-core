//! Connector-local `.config` vault capabilities (CB10).
//!
//! A config access is accepted only when the caller presents both the exact
//! `act.x.<connector>.config` authority and a decryptable line on the exact
//! `/x/<connector>` header.  The config key is random per connector and is
//! deliberately unrelated to the historical audit-argument key.

use crate::bundle::Bundle;
use crate::entropy::EntropySource;
use crate::manifest::sha256_hex;
use crate::Store;
use aithos_core::error::{Error, Result};
use aithos_core::gamma::{
    delegated_entry, owner_entry, verify_delegated_entry, Entry, EntrySpec, Kind,
};
use aithos_core::header::{owner_kid as header_owner_kid, Header, Recipient};
use aithos_core::keys::{ed2x, grantee_kex_secret, OwnerKeys};
use aithos_core::mandate::{Mandate, PerimeterEntry};
use aithos_core::seal::{blob_aad, blob_open, blob_seal};
use aithos_core::wire;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub enum VaultConfigOperation<'a> {
    Read { now: &'a str },
    Create { config: &'a [u8], now: &'a str },
    Edit { config: &'a [u8], now: &'a str },
    Delete { now: &'a str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultConfigOutcome {
    Read(Vec<u8>),
    Mutated,
}

/// Opaque proof of exact config physics. The key is intentionally private,
/// non-cloneable and omitted from every public representation.
pub struct VaultConfigCapability {
    connector: String,
    subject: String,
    version: u64,
    key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultConfigBlob {
    key_version: u64,
    n: String,
    c: String,
}

impl<S: Store> Bundle<S> {
    fn config_header_path(connector: &str) -> String {
        format!("e/x/{connector}/header.json")
    }

    fn config_blob_path(connector: &str) -> String {
        // The established canonical connector object name is
        // `manifest.enc`; its plaintext is the connector-private config.
        format!("e/x/{connector}/manifest.enc")
    }

    fn config_node(connector: &str) -> String {
        format!("/x/{connector}")
    }

    fn config_record_key(connector: &str) -> String {
        let mut preimage = b"aithos-core/v1/vault-config-record\0".to_vec();
        preimage.extend_from_slice(connector.as_bytes());
        format!("sha256:{}", sha256_hex(&preimage))
    }

    fn exact_config_authority(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        connector: &str,
        now: &str,
    ) -> Result<()> {
        let perimeter = self.verify_current_grantee(chain, agent_sk, now)?;
        if !perimeter.iter().any(|entry| {
            matches!(
                entry,
                PerimeterEntry::Act {
                    connector: granted,
                    action: Some(action),
                } if granted == connector && action == "config"
            )
        }) {
            return Err(Error::InvalidMandate(format!(
                "exact act.x.{connector}.config authority is required"
            )));
        }
        Ok(())
    }

    /// Open the exact connector-local physics capability after the current
    /// authority verdict. A wildcard action, audit key, sibling connector
    /// line or root vault line cannot satisfy this function.
    pub fn open_vault_with_capability(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        connector: &str,
        now: &str,
    ) -> Result<VaultConfigCapability> {
        Self::gate_display_name(connector)?;
        self.exact_config_authority(chain, agent_sk, connector, now)?;
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty config chain".into()))?;
        let header: Header = self.get_json(&Self::config_header_path(connector))?;
        if header.node != Self::config_node(connector) {
            return Err(Error::SealRejected(
                "connector config header is bound to another node".into(),
            ));
        }
        let kex = grantee_kex_secret(agent_sk);
        let (version, key) = header.open_latest(&self.did, &leaf.grantee.pubkey, &kex)?;
        Ok(VaultConfigCapability {
            connector: connector.to_owned(),
            subject: leaf.grantee.pubkey.clone(),
            version,
            key,
        })
    }

    fn open_config_blob(&self, capability: &VaultConfigCapability) -> Result<Vec<u8>> {
        let blob: VaultConfigBlob =
            self.get_json(&Self::config_blob_path(&capability.connector))?;
        if blob.key_version != capability.version {
            return Err(Error::SealRejected(
                "config blob/header version mismatch".into(),
            ));
        }
        let nonce: [u8; 24] = hex::decode(&blob.n)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| Error::SealRejected("invalid config nonce".into()))?;
        let ciphertext = hex::decode(&blob.c)
            .map_err(|_| Error::SealRejected("invalid config ciphertext".into()))?;
        let aad = blob_aad(
            &self.did,
            &Self::config_node(&capability.connector),
            capability.version,
        );
        blob_open(&capability.key, &ciphertext, &nonce, &aad)
    }

    fn put_config_blob(
        &mut self,
        capability: &VaultConfigCapability,
        config: &[u8],
        ent: &mut dyn EntropySource,
    ) -> Result<String> {
        let nonce = ent.e24();
        let aad = blob_aad(
            &self.did,
            &Self::config_node(&capability.connector),
            capability.version,
        );
        let ciphertext = blob_seal(&capability.key, config, &nonce, &aad);
        let blob = VaultConfigBlob {
            key_version: capability.version,
            n: hex::encode(nonce),
            c: hex::encode(ciphertext),
        };
        let path = Self::config_blob_path(&capability.connector);
        self.put_json(&path, &blob)?;
        Ok(format!("sha256:{}", sha256_hex(&self.get(&path)?)))
    }

    #[allow(clippy::too_many_arguments)]
    fn log_config_operation(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        connector: &str,
        kind: Kind,
        operation: &str,
        before: Option<&str>,
        after: Option<&str>,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Entry> {
        let entry = delegated_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: self.gamma_head()?,
                prevs: None,
                at: now.to_owned(),
                kind,
                target: Some(Self::config_node(connector)),
                payload: Some(serde_json::json!({
                    "after": after,
                    "before": before,
                    "operation": operation,
                    "record_key": Self::config_record_key(connector),
                })),
                body_enc: None,
            },
            chain.iter().map(|mandate| mandate.id.clone()).collect(),
            agent_sk,
        )?;
        verify_delegated_entry(&entry, chain, &self.did_doc()?)?;
        self.gamma_append(&entry)?;
        Ok(entry)
    }

    /// Read/create/edit/delete one connector's opaque config.
    ///
    /// Reads are journalized. Mutations and their exact-authority Gamma
    /// occurrence share one Store transaction; no network or external secret
    /// manager participates.
    pub fn vault_config_operation(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        connector: &str,
        operation: VaultConfigOperation<'_>,
        ent: &mut dyn EntropySource,
    ) -> Result<VaultConfigOutcome> {
        self.transaction(|bundle| {
            let now = match operation {
                VaultConfigOperation::Read { now }
                | VaultConfigOperation::Create { now, .. }
                | VaultConfigOperation::Edit { now, .. }
                | VaultConfigOperation::Delete { now } => now,
            };
            let capability = bundle.open_vault_with_capability(chain, agent_sk, connector, now)?;
            if capability.subject
                != chain
                    .last()
                    .ok_or_else(|| Error::InvalidMandate("empty config chain".into()))?
                    .grantee
                    .pubkey
            {
                return Err(Error::InvalidMandate(
                    "config capability subject mismatch".into(),
                ));
            }
            let path = Self::config_blob_path(connector);
            let exists = bundle.store.get(&path).map_err(|error| {
                Error::SealRejected(format!("config presence check failed: {error}"))
            })?;
            match operation {
                VaultConfigOperation::Read { .. } => {
                    let value = bundle.open_config_blob(&capability)?;
                    let current = format!("sha256:{}", sha256_hex(&bundle.get(&path)?));
                    bundle.log_config_operation(
                        chain,
                        agent_sk,
                        connector,
                        Kind::EthosRead,
                        "read",
                        Some(&current),
                        Some(&current),
                        now,
                        ent,
                    )?;
                    Ok(VaultConfigOutcome::Read(value))
                }
                VaultConfigOperation::Create { config, .. } => {
                    if exists.is_some() {
                        return Err(Error::InvalidOperation(
                            "connector config already exists".into(),
                        ));
                    }
                    let after = bundle.put_config_blob(&capability, config, ent)?;
                    bundle.log_config_operation(
                        chain,
                        agent_sk,
                        connector,
                        Kind::SectionAdd,
                        "create",
                        None,
                        Some(&after),
                        now,
                        ent,
                    )?;
                    Ok(VaultConfigOutcome::Mutated)
                }
                VaultConfigOperation::Edit { config, .. } => {
                    let Some(bytes) = exists else {
                        return Err(Error::InvalidOperation(
                            "connector config does not exist".into(),
                        ));
                    };
                    let before = format!("sha256:{}", sha256_hex(&bytes));
                    let after = bundle.put_config_blob(&capability, config, ent)?;
                    bundle.log_config_operation(
                        chain,
                        agent_sk,
                        connector,
                        Kind::SectionModify,
                        "edit",
                        Some(&before),
                        Some(&after),
                        now,
                        ent,
                    )?;
                    Ok(VaultConfigOutcome::Mutated)
                }
                VaultConfigOperation::Delete { .. } => {
                    let Some(bytes) = exists else {
                        return Err(Error::InvalidOperation(
                            "connector config does not exist".into(),
                        ));
                    };
                    let before = format!("sha256:{}", sha256_hex(&bytes));
                    bundle.delete_object(&path)?;
                    bundle.log_config_operation(
                        chain,
                        agent_sk,
                        connector,
                        Kind::SectionDelete,
                        "delete",
                        Some(&before),
                        None,
                        now,
                        ent,
                    )?;
                    Ok(VaultConfigOutcome::Mutated)
                }
            }
        })
    }

    /// Owner-local config read. This is a KEX-only path and creates no
    /// delegated authority.
    pub fn read_vault_config_owner(&self, owner: &OwnerKeys, connector: &str) -> Result<Vec<u8>> {
        Self::gate_display_name(connector)?;
        let header: Header = self.get_json(&Self::config_header_path(connector))?;
        let (version, key) = header.open_owner_latest(&self.did, &owner.owner_kex)?;
        self.open_config_blob(&VaultConfigCapability {
            connector: connector.to_owned(),
            subject: "owner".into(),
            version,
            key,
        })
    }

    /// Rotate one connector config independently, excluding one recipient,
    /// re-encrypting the local config and publishing the cut atomically.
    pub fn rotate_vault_connector(
        &mut self,
        owner: &OwnerKeys,
        connector: &str,
        revoked_kid: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        Self::gate_display_name(connector)?;
        self.transaction(|bundle| {
            let path = Self::config_header_path(connector);
            let mut header: Header = bundle.get_json(&path)?;
            let old_version = header.latest_version();
            let old_key = header.open_owner(&bundle.did, old_version, &owner.owner_kex)?;
            let current = header
                .key_versions
                .get(&old_version.to_string())
                .ok_or_else(|| Error::SealRejected("missing config key version".into()))?;
            if !current.lines.iter().any(|line| line.kid == revoked_kid) {
                return Err(Error::InvalidMandate(
                    "revoked config recipient is not present".into(),
                ));
            }
            // The owner line is the one declaring owner_kex (§03.1).
            let owner_kex = bundle.owner_kex_pub()?;
            let owner_kid = header_owner_kid(&owner_kex);
            let mut survivors = Vec::new();
            for line in &current.lines {
                if line.kid == revoked_kid {
                    continue;
                }
                if line.kid == owner_kid {
                    survivors.push(Recipient::owner(owner_kex));
                } else {
                    let bytes = wire::multibase_to_ed25519_pub(&line.to)?;
                    let verifying = VerifyingKey::from_bytes(&bytes)
                        .map_err(|_| Error::SealRejected("bad config survivor key".into()))?;
                    survivors.push(Recipient {
                        to: line.to.clone(),
                        kid: line.kid.clone(),
                        pubkey: ed2x(&verifying),
                    });
                }
            }
            let new_version = old_version + 1;
            let new_key = ent.e32();
            let ephemerals = survivors.iter().map(|_| ent.e32()).collect::<Vec<_>>();
            let nonces = survivors.iter().map(|_| ent.e24()).collect::<Vec<_>>();
            header.rotate(
                &bundle.did,
                new_version,
                &new_key,
                &owner_kex,
                &survivors,
                &ephemerals,
                &nonces,
            )?;
            header.check_rotation(new_version, &owner_kid)?;
            bundle.put_json(&path, &header)?;
            let blob_path = Self::config_blob_path(connector);
            if bundle
                .store
                .get(&blob_path)
                .map_err(|error| {
                    Error::SealRejected(format!("config rotation read failed: {error}"))
                })?
                .is_some()
            {
                let old_capability = VaultConfigCapability {
                    connector: connector.to_owned(),
                    subject: "owner".into(),
                    version: old_version,
                    key: old_key,
                };
                let plaintext = bundle.open_config_blob(&old_capability)?;
                let new_capability = VaultConfigCapability {
                    connector: connector.to_owned(),
                    subject: "owner".into(),
                    version: new_version,
                    key: new_key,
                };
                bundle.put_config_blob(&new_capability, &plaintext, ent)?;
            }
            let entry = owner_entry(
                EntrySpec {
                    id: bundle.next_gamma_id(ent),
                    prev: bundle.gamma_head()?,
                    prevs: None,
                    at: now.to_owned(),
                    kind: Kind::Rotate,
                    target: Some(Self::config_node(connector)),
                    payload: Some(serde_json::json!({
                        "domain": "vault",
                        "new_version": new_version,
                    })),
                    body_enc: None,
                },
                &owner.content_sign,
            )?;
            bundle.gamma_append(&entry)?;
            bundle.publish(owner, now)
        })
    }
}
