//! Tenant read-model — annexe A.2 #1 / A.7 (`unknown_tenant`, `suspended`,
//! `did_not_bound`) for the store, and the tunnel mapping of annexe B.2
//! (`gateway_pub ↔ tenant ↔ hostname ↔ suspended`) for the relay (P6).
//!
//! **P1/P6 shape:** a static bootstrap file loaded at startup — the P7
//! control plane (DynamoDB table, CLI d'enrôlement, suspension < 60 s)
//! replaces this behind the same lookups. The bootstrap carries **public
//! material only**: tenant names, DIDs, their public `did.json` documents,
//! and public gateway keys — never a secret, never a private key
//! (doctrine §1: the provider holds none).
//!
//! Every embedded `did.json` is verified (`DidDocument::verify`, §01.4)
//! before it may resolve an owner key: a bootstrap that does not verify
//! refuses to load — fail-closed at startup, not at request time.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::did::DidDocument;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantState {
    Unknown,
    Suspended,
    Active,
}

/// The control-plane binding of one gateway key (annexe B.2): the tenant
/// and hostname it is enrolled for, and whether that enrollment is
/// suspended. Resolved by `gateway_pub` — one gateway key is one identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelBinding {
    pub tenant: String,
    pub hostname: String,
    pub suspended: bool,
}

/// Bootstrap file schema (P1/P6): `{"tenants": [{"tenant", "suspended"?,
/// "dids": [{"did", "did_json"}]}], "tunnels"?: [{"gateway_pub", "tenant",
/// "hostname", "suspended"?}]}`. `did_json` is the JCS string of the signed
/// document, byte-preserved into the object store. `tunnels` is optional —
/// a store-only bootstrap omits it, the relay's needs the mappings.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapFile {
    tenants: Vec<BootstrapTenant>,
    #[serde(default)]
    tunnels: Vec<BootstrapTunnel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapTenant {
    tenant: String,
    #[serde(default)]
    suspended: bool,
    dids: Vec<BootstrapDid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapDid {
    did: String,
    did_json: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapTunnel {
    gateway_pub: String,
    tenant: String,
    hostname: String,
    #[serde(default)]
    suspended: bool,
}

#[derive(Debug, Default)]
pub struct ControlPlane {
    tenants: BTreeMap<String, TenantEntry>,
    /// gateway_pub (multibase) → binding (annexe B.2).
    tunnels: BTreeMap<String, TunnelBinding>,
}

#[derive(Debug, Default)]
struct TenantEntry {
    suspended: bool,
    dids: BTreeSet<String>,
}

/// A verified preload destined for the object store: `(tenant, did,
/// did.json bytes)`.
pub type PreloadedDoc = (String, String, Vec<u8>);

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("bootstrap file unreadable: {0}")]
    Unreadable(String),
    #[error("bootstrap file malformed: {0}")]
    Malformed(String),
    #[error("bootstrap did.json rejected for {did}: {why}")]
    DidRejected { did: String, why: String },
}

impl ControlPlane {
    /// Load and verify a bootstrap file. Returns the read-model and the
    /// verified `did.json` documents to preload into the object store.
    pub fn load_bootstrap(path: &str) -> Result<(Self, Vec<PreloadedDoc>), BootstrapError> {
        let raw =
            std::fs::read_to_string(path).map_err(|e| BootstrapError::Unreadable(e.to_string()))?;
        Self::from_bootstrap_json(&raw)
    }

    pub fn from_bootstrap_json(raw: &str) -> Result<(Self, Vec<PreloadedDoc>), BootstrapError> {
        let file: BootstrapFile =
            serde_json::from_str(raw).map_err(|e| BootstrapError::Malformed(e.to_string()))?;
        let mut plane = ControlPlane::default();
        let mut preloads = Vec::new();
        for tenant in file.tenants {
            let entry = plane.tenants.entry(tenant.tenant.clone()).or_default();
            entry.suspended = tenant.suspended;
            for bound in tenant.dids {
                let doc: DidDocument = serde_json::from_str(&bound.did_json).map_err(|e| {
                    BootstrapError::DidRejected {
                        did: bound.did.clone(),
                        why: format!("parse: {e}"),
                    }
                })?;
                doc.verify().map_err(|e| BootstrapError::DidRejected {
                    did: bound.did.clone(),
                    why: e.to_string(),
                })?;
                if doc.id != bound.did {
                    return Err(BootstrapError::DidRejected {
                        did: bound.did,
                        why: "document id differs from the bound did".into(),
                    });
                }
                entry.dids.insert(bound.did.clone());
                preloads.push((
                    tenant.tenant.clone(),
                    bound.did,
                    bound.did_json.into_bytes(),
                ));
            }
        }
        for tunnel in file.tunnels {
            plane.tunnels.insert(
                tunnel.gateway_pub,
                TunnelBinding {
                    tenant: tunnel.tenant,
                    hostname: tunnel.hostname,
                    suspended: tunnel.suspended,
                },
            );
        }
        Ok((plane, preloads))
    }

    pub fn tenant_state(&self, tenant: &str) -> TenantState {
        match self.tenants.get(tenant) {
            None => TenantState::Unknown,
            Some(e) if e.suspended => TenantState::Suspended,
            Some(_) => TenantState::Active,
        }
    }

    pub fn did_bound(&self, tenant: &str, did: &str) -> bool {
        self.tenants
            .get(tenant)
            .is_some_and(|e| e.dids.contains(did))
    }

    /// Resolve a gateway key to its tunnel binding (annexe B.2). `None` =
    /// the key is enrolled for no tunnel — the relay answers
    /// `mapping_mismatch` (never an enumeration oracle: same answer as a
    /// wrong hostname).
    pub fn resolve_tunnel(&self, gateway_pub: &str) -> Option<&TunnelBinding> {
        self.tunnels.get(gateway_pub)
    }

    /// Insert a tunnel binding (test/tooling surface; the P7 admin CLI
    /// writes the real ones to DynamoDB).
    pub fn bind_tunnel(&mut self, gateway_pub: String, binding: TunnelBinding) {
        self.tunnels.insert(gateway_pub, binding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed p1 vector is the fixture of record: the replay bootstrap
    /// (`bootstrap/replay.json`, baked into the image) must carry exactly its
    /// tenant, DID and did.json — a drift here would deploy a store
    /// the vectors cannot replay against.
    #[test]
    fn replay_bootstrap_matches_the_committed_p1_vector() {
        let vector: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p1-store-envelope.json"
            ))
            .expect("vectors/p1-store-envelope.json"),
        )
        .unwrap();
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/bootstrap/replay.json"
        ))
        .expect("bootstrap/dev.json");
        let (plane, preloads) = ControlPlane::from_bootstrap_json(&raw).expect("bootstrap loads");

        let tenant = vector["tenant"].as_str().unwrap();
        let did = vector["did"].as_str().unwrap();
        let did_json = vector["did_json_jcs"].as_str().unwrap();
        assert_eq!(plane.tenant_state(tenant), TenantState::Active);
        assert!(plane.did_bound(tenant, did));
        assert_eq!(
            preloads,
            vec![(
                tenant.to_owned(),
                did.to_owned(),
                did_json.as_bytes().to_vec()
            )]
        );

        // The B.5 surface needs the demo tunnel mapping in the STORE
        // bootstrap too (the /acme authority) — pinned against the p6
        // vector's first mapping, so image and vectors cannot drift.
        let p6: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../vectors/p6-acme-txt.json"
            ))
            .expect("vectors/p6-acme-txt.json"),
        )
        .unwrap();
        let demo = &p6["control_plane_mappings"][0];
        let bound = plane
            .resolve_tunnel(demo["gateway_pub"].as_str().unwrap())
            .expect("the demo gateway is bound in the replay bootstrap");
        assert_eq!(bound.tenant, demo["tenant"].as_str().unwrap());
        assert_eq!(bound.hostname, demo["hostname"].as_str().unwrap());
        assert!(!bound.suspended);
    }

    #[test]
    fn a_bootstrap_with_a_broken_did_document_refuses_to_load() {
        let raw = r#"{"tenants":[{"tenant":"acme","dids":[{"did":"did:aithos:zBogus","did_json":"{}"}]}]}"#;
        assert!(ControlPlane::from_bootstrap_json(raw).is_err());
    }

    #[test]
    fn lookups_are_fail_closed() {
        let (plane, _) = ControlPlane::from_bootstrap_json(r#"{"tenants":[]}"#).unwrap();
        assert_eq!(plane.tenant_state("ghost"), TenantState::Unknown);
        assert!(!plane.did_bound("ghost", "did:aithos:zX"));
        // A gateway key nobody enrolled resolves to no tunnel.
        assert!(plane.resolve_tunnel("z6MkGhost").is_none());
    }

    #[test]
    fn bootstrap_loads_the_optional_tunnel_mappings() {
        // The P7-static slice consumed by the relay (annexe B.2): a
        // store-only bootstrap omits `tunnels`; the relay's carries them.
        let raw = r#"{
            "tenants": [],
            "tunnels": [
                {"gateway_pub": "z6MkGw1", "tenant": "acme", "hostname": "demo.mcp.aithos.fr"},
                {"gateway_pub": "z6MkGw2", "tenant": "beta", "hostname": "beta.mcp.aithos.fr", "suspended": true}
            ]
        }"#;
        let (plane, _) = ControlPlane::from_bootstrap_json(raw).unwrap();
        let bound = plane.resolve_tunnel("z6MkGw1").expect("gw1 bound");
        assert_eq!(bound.tenant, "acme");
        assert_eq!(bound.hostname, "demo.mcp.aithos.fr");
        assert!(!bound.suspended);
        assert!(plane.resolve_tunnel("z6MkGw2").unwrap().suspended);
        assert!(plane.resolve_tunnel("z6MkNope").is_none());
    }
}
