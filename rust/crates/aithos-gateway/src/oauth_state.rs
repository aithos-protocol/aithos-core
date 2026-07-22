//! Durable state seam for the embedded authorization server.
//!
//! The protocol chooses the namespace; callers supply only bounded opaque
//! identifiers. Production uses Vault KV v2 with versioned writes, while unit
//! and acceptance tests can inject the byte-identical in-memory semantics.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::{GatewayError, Result};

const MAX_RECORD_BYTES: u64 = 1024 * 1024;
const VAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Closed storage namespaces. No browser or OAuth parameter can choose one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StateNamespace {
    AdapterKey,
    DcrClient,
    Pending,
    Code,
    Session,
    SessionKey,
    RefreshFamily,
    Nonce,
}

impl StateNamespace {
    fn as_str(self) -> &'static str {
        match self {
            Self::AdapterKey => "adapter-key",
            Self::DcrClient => "dcr-client",
            Self::Pending => "pending",
            Self::Code => "code",
            Self::Session => "session",
            Self::SessionKey => "session-key",
            Self::RefreshFamily => "refresh-family",
            Self::Nonce => "nonce",
        }
    }
}

/// A live record and the compare-and-swap version that protects it.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionedState {
    pub version: u64,
    pub value: Value,
}

/// Minimal object-safe state API. `take` is an atomic one-shot reservation:
/// after it succeeds, all future reads and takes return absent.
pub trait AsStateStore: Send + Sync {
    fn read(&self, namespace: StateNamespace, id: &str) -> Result<Option<VersionedState>>;
    fn create(&self, namespace: StateNamespace, id: &str, value: Value) -> Result<u64>;
    fn compare_and_swap(
        &self,
        namespace: StateNamespace,
        id: &str,
        expected_version: u64,
        value: Value,
    ) -> Result<u64>;
    fn take(&self, namespace: StateNamespace, id: &str) -> Result<Option<Value>>;
}

#[derive(Clone)]
struct MemoryRecord {
    version: u64,
    value: Value,
    consumed: bool,
}

/// Test implementation with the same create/CAS/one-shot behavior as Vault.
#[derive(Default)]
pub struct MemoryAsStateStore {
    records: Mutex<BTreeMap<(StateNamespace, String), MemoryRecord>>,
}

impl AsStateStore for MemoryAsStateStore {
    fn read(&self, namespace: StateNamespace, id: &str) -> Result<Option<VersionedState>> {
        validate_id(id)?;
        Ok(self
            .records
            .lock()
            .expect("OAuth state lock")
            .get(&(namespace, id.to_owned()))
            .filter(|record| !record.consumed)
            .map(|record| VersionedState {
                version: record.version,
                value: record.value.clone(),
            }))
    }

    fn create(&self, namespace: StateNamespace, id: &str, value: Value) -> Result<u64> {
        validate_record(id, &value)?;
        let mut records = self.records.lock().expect("OAuth state lock");
        let key = (namespace, id.to_owned());
        if records.contains_key(&key) {
            return Err(state_conflict());
        }
        records.insert(
            key,
            MemoryRecord {
                version: 1,
                value,
                consumed: false,
            },
        );
        Ok(1)
    }

    fn compare_and_swap(
        &self,
        namespace: StateNamespace,
        id: &str,
        expected_version: u64,
        value: Value,
    ) -> Result<u64> {
        validate_record(id, &value)?;
        let mut records = self.records.lock().expect("OAuth state lock");
        let record = records
            .get_mut(&(namespace, id.to_owned()))
            .filter(|record| !record.consumed && record.version == expected_version)
            .ok_or_else(state_conflict)?;
        record.version = record.version.checked_add(1).ok_or_else(state_conflict)?;
        record.value = value;
        Ok(record.version)
    }

    fn take(&self, namespace: StateNamespace, id: &str) -> Result<Option<Value>> {
        validate_id(id)?;
        let mut records = self.records.lock().expect("OAuth state lock");
        let Some(record) = records.get_mut(&(namespace, id.to_owned())) else {
            return Ok(None);
        };
        if record.consumed {
            return Ok(None);
        }
        record.consumed = true;
        record.version = record.version.checked_add(1).ok_or_else(state_conflict)?;
        Ok(Some(record.value.clone()))
    }
}

/// Production Vault KV v2 state store. Its prefix is operator-controlled and
/// every path below it is derived from a closed namespace plus a validated id.
pub struct VaultAsStateStore {
    address: String,
    mount: String,
    prefix: String,
    token_env: String,
    agent: ureq::Agent,
}

impl VaultAsStateStore {
    pub fn new(address: &str, mount: &str, prefix: &str, token_env: &str) -> Result<Self> {
        let address = address.trim_end_matches('/');
        if !(address.starts_with("https://") || is_loopback_http(address)) {
            return Err(GatewayError::ConfigRejected(
                "OAuth state Vault must use HTTPS off loopback".into(),
            ));
        }
        validate_segment(mount, "OAuth state Vault mount")?;
        validate_prefix(prefix)?;
        if token_env.trim().is_empty() {
            return Err(GatewayError::ConfigRejected(
                "OAuth state Vault token_env is empty".into(),
            ));
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(VAULT_TIMEOUT))
            .build()
            .into();
        Ok(Self {
            address: address.to_owned(),
            mount: mount.to_owned(),
            prefix: prefix.trim_matches('/').to_owned(),
            token_env: token_env.to_owned(),
            agent,
        })
    }

    fn url(&self, namespace: StateNamespace, id: &str) -> Result<String> {
        validate_id(id)?;
        Ok(format!(
            "{}/v1/{}/data/{}/{}/{}",
            self.address,
            self.mount,
            self.prefix,
            namespace.as_str(),
            id
        ))
    }

    fn token(&self) -> Result<Zeroizing<String>> {
        std::env::var(&self.token_env)
            .ok()
            .filter(|token| !token.trim().is_empty())
            .map(Zeroizing::new)
            .ok_or_else(|| state_unavailable("Vault authentication is unavailable"))
    }

    fn read_wire(
        &self,
        namespace: StateNamespace,
        id: &str,
    ) -> Result<Option<(u64, VaultEnvelope)>> {
        let token = self.token()?;
        let mut response = self
            .agent
            .get(&self.url(namespace, id)?)
            .header("X-Vault-Token", token.as_str())
            .call()
            .map_err(|_| state_unavailable("Vault transport failed"))?;
        match response.status().as_u16() {
            404 => return Ok(None),
            200 => {}
            status => return Err(state_status(status)),
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_RECORD_BYTES)
            .read_to_vec()
            .map_err(|_| state_unavailable("Vault response is unreadable"))?;
        let body: Value = serde_json::from_slice(&bytes)
            .map_err(|_| state_unavailable("Vault response is malformed"))?;
        let version = body
            .pointer("/data/metadata/version")
            .and_then(Value::as_u64)
            .ok_or_else(|| state_unavailable("Vault response metadata is malformed"))?;
        let envelope: VaultEnvelope = serde_json::from_value(
            body.pointer("/data/data/record")
                .cloned()
                .ok_or_else(|| state_unavailable("Vault state record is malformed"))?,
        )
        .map_err(|_| state_unavailable("Vault state record is malformed"))?;
        Ok(Some((version, envelope)))
    }

    fn write_wire(
        &self,
        namespace: StateNamespace,
        id: &str,
        cas: u64,
        envelope: &VaultEnvelope,
    ) -> Result<u64> {
        let token = self.token()?;
        let body = serde_json::to_vec(&json!({
            "options": { "cas": cas },
            "data": { "record": envelope },
        }))
        .map_err(|_| state_unavailable("OAuth state serialization failed"))?;
        if body.len() as u64 > MAX_RECORD_BYTES {
            return Err(state_unavailable("OAuth state record is too large"));
        }
        let mut response = self
            .agent
            .post(&self.url(namespace, id)?)
            .header("X-Vault-Token", token.as_str())
            .header("Content-Type", "application/json")
            .send(&body)
            .map_err(|_| state_unavailable("Vault transport failed"))?;
        if response.status().as_u16() != 200 && response.status().as_u16() != 204 {
            return Err(if response.status().as_u16() == 400 {
                state_conflict()
            } else {
                state_status(response.status().as_u16())
            });
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(64 * 1024)
            .read_to_vec()
            .map_err(|_| state_unavailable("Vault response is unreadable"))?;
        if bytes.is_empty() {
            return Ok(cas.saturating_add(1));
        }
        let response: Value = serde_json::from_slice(&bytes)
            .map_err(|_| state_unavailable("Vault response is malformed"))?;
        Ok(response
            .pointer("/data/version")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| cas.saturating_add(1)))
    }
}

impl AsStateStore for VaultAsStateStore {
    fn read(&self, namespace: StateNamespace, id: &str) -> Result<Option<VersionedState>> {
        Ok(self
            .read_wire(namespace, id)?
            .filter(|(_, envelope)| !envelope.consumed)
            .map(|(version, envelope)| VersionedState {
                version,
                value: envelope.value,
            }))
    }

    fn create(&self, namespace: StateNamespace, id: &str, value: Value) -> Result<u64> {
        validate_record(id, &value)?;
        self.write_wire(
            namespace,
            id,
            0,
            &VaultEnvelope {
                consumed: false,
                value,
            },
        )
    }

    fn compare_and_swap(
        &self,
        namespace: StateNamespace,
        id: &str,
        expected_version: u64,
        value: Value,
    ) -> Result<u64> {
        validate_record(id, &value)?;
        self.write_wire(
            namespace,
            id,
            expected_version,
            &VaultEnvelope {
                consumed: false,
                value,
            },
        )
    }

    fn take(&self, namespace: StateNamespace, id: &str) -> Result<Option<Value>> {
        let Some((version, envelope)) = self.read_wire(namespace, id)? else {
            return Ok(None);
        };
        if envelope.consumed {
            return Ok(None);
        }
        self.write_wire(
            namespace,
            id,
            version,
            &VaultEnvelope {
                consumed: true,
                value: envelope.value.clone(),
            },
        )?;
        Ok(Some(envelope.value))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultEnvelope {
    consumed: bool,
    value: Value,
}

fn validate_record(id: &str, value: &Value) -> Result<()> {
    validate_id(id)?;
    let bytes = serde_json::to_vec(value)
        .map_err(|_| state_unavailable("OAuth state serialization failed"))?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(state_unavailable("OAuth state record is too large"));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(state_unavailable("OAuth state identifier is invalid"));
    }
    Ok(())
}

fn validate_segment(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(GatewayError::ConfigRejected(format!("{label} is invalid")));
    }
    Ok(())
}

fn validate_prefix(prefix: &str) -> Result<()> {
    let trimmed = prefix.trim_matches('/');
    if trimmed.is_empty()
        || trimmed.len() > 256
        || trimmed
            .split('/')
            .any(|segment| validate_segment(segment, "OAuth state Vault prefix").is_err())
    {
        return Err(GatewayError::ConfigRejected(
            "OAuth state Vault prefix is invalid".into(),
        ));
    }
    Ok(())
}

fn is_loopback_http(address: &str) -> bool {
    address
        .strip_prefix("http://")
        .and_then(|rest| rest.split(['/', ':']).next())
        .is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        })
}

fn state_conflict() -> GatewayError {
    GatewayError::OauthDenied {
        error: "temporarily_unavailable".into(),
        detail: "OAuth state compare-and-swap was refused".into(),
    }
}

fn state_unavailable(detail: &str) -> GatewayError {
    GatewayError::OauthDenied {
        error: "temporarily_unavailable".into(),
        detail: detail.to_owned(),
    }
}

fn state_status(status: u16) -> GatewayError {
    state_unavailable(&format!("Vault answered status {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_create_cas_and_take_are_closed_and_one_shot() {
        let store = MemoryAsStateStore::default();
        assert_eq!(
            store
                .create(StateNamespace::Code, "abc", json!({ "v": 1 }))
                .unwrap(),
            1
        );
        assert!(store
            .create(StateNamespace::Code, "abc", json!({ "v": 2 }))
            .is_err());
        assert_eq!(
            store
                .compare_and_swap(StateNamespace::Code, "abc", 1, json!({ "v": 2 }))
                .unwrap(),
            2
        );
        assert!(store
            .compare_and_swap(StateNamespace::Code, "abc", 1, json!({ "v": 3 }))
            .is_err());
        assert_eq!(
            store.take(StateNamespace::Code, "abc").unwrap(),
            Some(json!({ "v": 2 }))
        );
        assert_eq!(store.take(StateNamespace::Code, "abc").unwrap(), None);
        assert_eq!(store.read(StateNamespace::Code, "abc").unwrap(), None);
    }

    #[test]
    fn namespaces_and_ids_cannot_be_path_injected() {
        let store = MemoryAsStateStore::default();
        assert!(store
            .create(StateNamespace::Session, "../neighbor", json!({}))
            .is_err());
        assert!(VaultAsStateStore::new(
            "https://vault.example",
            "secret/data",
            "aithos/as",
            "AITHOS_AS_VAULT_TOKEN"
        )
        .is_err());
    }
}
