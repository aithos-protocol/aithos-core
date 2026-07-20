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

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use aithos_core::did::DidDocument;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantState {
    Unknown,
    Suspended,
    Active,
}

/// The control backend cannot answer. Fixed cause only (discipline A.8) —
/// the caller refuses `503 unavailable`, it NEVER invents an
/// `unknown_tenant` or a `did_not_bound` (P7 gate contrat, pattern of the
/// étape-6 seams).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlUnavailable;

/// Boxed seam future, house style (`objects.rs`, `heads.rs`).
pub type ControlFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ControlUnavailable>> + Send + 'a>>;

/// The store-side control-plane seam (P7): the SAME three lookups
/// `control.rs` has served since P1, behind a backend. `now_ms` is the
/// request clock (the injected test instant under `AITHOS_STORE_TEST_NOW`)
/// — the freshness bound of the cached backend counts against it, so the
/// < 60 s propagation promise is provable with the test clock, never with
/// a sleep.
///
/// Backends: [`ControlPlane`] (the bootstrap file — dev/tests, and the
/// relay's P6 shape), [`DynamoDbControl`] (the P7 table) and
/// [`CachedControl`] (the freshness wrapper the deployed composition
/// uses). The relay keeps the sync [`ControlPlane`] read-model — its
/// bascule is a separate lot (arbitrage gate contrat P7, 2026-07-20).
pub trait ControlStore: Send + Sync {
    /// Annexe A.2 #1 — the tenant gate.
    fn tenant_state<'a>(&'a self, tenant: &'a str, now_ms: i64) -> ControlFuture<'a, TenantState>;

    /// Annexe A.2 #7 — the DID binding (only ever named under a valid
    /// envelope; the anti-enumeration note of A.7 holds at every backend).
    fn did_bound<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        now_ms: i64,
    ) -> ControlFuture<'a, bool>;

    /// Annexe B.2 — the gateway mapping (the store's `/acme/txt`
    /// authority). `None` = enrolled for no tunnel (`mapping_mismatch`,
    /// never an enumeration oracle).
    fn resolve_tunnel<'a>(
        &'a self,
        gateway_pub: &'a str,
        now_ms: i64,
    ) -> ControlFuture<'a, Option<TunnelBinding>>;
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
    /// The signed `did.json` (JCS string), byte-preserved into the object
    /// store. OPTIONAL since étape 5: a binding without a document is the
    /// pre-genesis state — the control plane lists the DID for the tenant
    /// (« l'enrôlement P7 précède toujours », A.4) and the first
    /// `did.json` arrives through the wire genesis deposit.
    #[serde(default)]
    did_json: Option<String>,
    /// Optional PUBLIC artifacts preloaded beside `did.json` (replay and
    /// dev fixtures: mandate certs, gamma segments). Opaque at load —
    /// every use-site verifies them (the #9 chain check parses certs and
    /// verifies signatures; a bogus preload fails closed there). P7's
    /// enrollment plane replaces this file entirely.
    #[serde(default)]
    objects: Vec<BootstrapObject>,
    /// Optional A.5 heads-table seed (replay fixtures: the p7 cases name
    /// the table state each case starts from). Wire forms of the vector:
    /// `manifest`/`gamma` carry `sha256:<hex>` (or are absent/empty),
    /// `segment` the `YYYY-MM` month. Étape-4 additive; the P7 enrollment
    /// plane never seeds heads (genesis walks through the CAS).
    #[serde(default)]
    heads: Option<BootstrapHeads>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapHeads {
    #[serde(default)]
    height: u64,
    #[serde(default)]
    manifest: String,
    #[serde(default)]
    gamma: String,
    #[serde(default)]
    segment: String,
    #[serde(default)]
    segments: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapObject {
    key: String,
    utf8: String,
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

/// A preload destined for the object store: `(tenant, did, chemin,
/// bytes)` — `did.json` (verified here) plus the optional public fixture
/// objects (verified at use).
pub type PreloadedDoc = (String, String, String, Vec<u8>);

/// A heads-table seed: `(tenant, did, record)` — replay fixtures only.
pub type PreloadedHeads = (String, String, crate::heads::HeadsRecord);

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
    /// Load and verify a bootstrap file. Returns the read-model, the
    /// verified `did.json` documents (plus fixture objects) to preload
    /// into the object store, and the optional heads-table seeds.
    #[allow(clippy::type_complexity)]
    pub fn load_bootstrap(
        path: &str,
    ) -> Result<(Self, Vec<PreloadedDoc>, Vec<PreloadedHeads>), BootstrapError> {
        let raw =
            std::fs::read_to_string(path).map_err(|e| BootstrapError::Unreadable(e.to_string()))?;
        Self::from_bootstrap_json(&raw)
    }

    #[allow(clippy::type_complexity)]
    pub fn from_bootstrap_json(
        raw: &str,
    ) -> Result<(Self, Vec<PreloadedDoc>, Vec<PreloadedHeads>), BootstrapError> {
        let file: BootstrapFile =
            serde_json::from_str(raw).map_err(|e| BootstrapError::Malformed(e.to_string()))?;
        let mut plane = ControlPlane::default();
        let mut preloads = Vec::new();
        let mut heads = Vec::new();
        for tenant in file.tenants {
            let entry = plane.tenants.entry(tenant.tenant.clone()).or_default();
            entry.suspended = tenant.suspended;
            for bound in tenant.dids {
                if let Some(did_json) = &bound.did_json {
                    let doc: DidDocument = serde_json::from_str(did_json).map_err(|e| {
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
                    preloads.push((
                        tenant.tenant.clone(),
                        bound.did.clone(),
                        "did.json".to_owned(),
                        did_json.clone().into_bytes(),
                    ));
                }
                entry.dids.insert(bound.did.clone());
                for object in bound.objects {
                    preloads.push((
                        tenant.tenant.clone(),
                        bound.did.clone(),
                        object.key,
                        object.utf8.into_bytes(),
                    ));
                }
                if let Some(seed) = bound.heads {
                    let manifest_chain_hash = seed
                        .manifest
                        .strip_prefix("sha256:")
                        .unwrap_or(&seed.manifest)
                        .to_owned();
                    let gamma_segments = seed.segments.unwrap_or_else(|| {
                        if seed.segment.is_empty() {
                            Vec::new()
                        } else {
                            vec![seed.segment.clone()]
                        }
                    });
                    heads.push((
                        tenant.tenant.clone(),
                        bound.did.clone(),
                        crate::heads::HeadsRecord {
                            height: seed.height,
                            manifest_chain_hash,
                            gamma_head: seed.gamma,
                            gamma_segment: seed.segment,
                            gamma_segments,
                        },
                    ));
                }
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
        Ok((plane, preloads, heads))
    }

    /// True when the plane carries neither a tenant nor a tunnel mapping
    /// — the P7 boot guard's question (`store_api.rs`: a dynamodb control
    /// backend refuses any bootstrap that carries either).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty() && self.tunnels.is_empty()
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

    /// Seed a tenant row (test/tooling surface, the `bind_tunnel` twin —
    /// P7b: the B.2 step 4 joins the TENANT state, so every fixture plane
    /// that binds a tunnel names its tenant too, exactly as the admin CLI
    /// demands `create` before `bind-gateway`).
    pub fn seed_tenant(&mut self, tenant: &str, suspended: bool) {
        self.tenants.entry(tenant.to_owned()).or_default().suspended = suspended;
    }
}

// ----------------------------------------------------- gateway authority

/// A refusal of the graved gateway authority (B.5, adopted by B.2 step 4 —
/// arbitrage bascule relay P7b, 2026-07-20). The caller maps these onto its
/// own wire registry (`Refusal` on /acme, `TunnelRefusal` on the tunnel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayAuthzRefusal {
    /// The backend cannot answer — refuse, never guess (fail-closed).
    Unavailable,
    /// No binding, or a binding onto an unknown tenant (an orphan binding
    /// never resolves) — the same answer as a foreign hostname, no
    /// enumeration oracle.
    MappingMismatch,
    /// The binding or its tenant is suspended.
    Suspended,
}

/// The graved authority order shared by the store's `/acme` surface (B.5)
/// and the relay's registration step 4 (B.2, P7b): resolve the binding by
/// `gateway_pub`, then the binding's own suspension, then the TENANT's
/// state. The caller still performs its exact-match checks (hostname,
/// claimed tenant) — this helper decides AUTHORITY, not routing.
pub async fn authorize_gateway(
    control: &dyn ControlStore,
    gateway_pub: &str,
    now_ms: i64,
) -> Result<TunnelBinding, GatewayAuthzRefusal> {
    let Some(binding) = control
        .resolve_tunnel(gateway_pub, now_ms)
        .await
        .map_err(|_| GatewayAuthzRefusal::Unavailable)?
    else {
        return Err(GatewayAuthzRefusal::MappingMismatch);
    };
    if binding.suspended {
        return Err(GatewayAuthzRefusal::Suspended);
    }
    match control
        .tenant_state(&binding.tenant, now_ms)
        .await
        .map_err(|_| GatewayAuthzRefusal::Unavailable)?
    {
        TenantState::Unknown => Err(GatewayAuthzRefusal::MappingMismatch),
        TenantState::Suspended => Err(GatewayAuthzRefusal::Suspended),
        TenantState::Active => Ok(binding),
    }
}

/// The bootstrap read-model IS a control backend — infallible (it lives
/// in process memory) and clock-blind. This keeps dev/tests and the
/// committed vectors on the exact P1 semantics behind the P7 seam.
impl ControlStore for ControlPlane {
    fn tenant_state<'a>(&'a self, tenant: &'a str, _now_ms: i64) -> ControlFuture<'a, TenantState> {
        Box::pin(async move { Ok(ControlPlane::tenant_state(self, tenant)) })
    }

    fn did_bound<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        _now_ms: i64,
    ) -> ControlFuture<'a, bool> {
        Box::pin(async move { Ok(ControlPlane::did_bound(self, tenant, did)) })
    }

    fn resolve_tunnel<'a>(
        &'a self,
        gateway_pub: &'a str,
        _now_ms: i64,
    ) -> ControlFuture<'a, Option<TunnelBinding>> {
        Box::pin(async move { Ok(ControlPlane::resolve_tunnel(self, gateway_pub).cloned()) })
    }
}

// ------------------------------------------------------------------ cache

/// The freshness wrapper (P7 arbitrage 2026-07-20: short TTL cache over
/// the table, bound ≤ 30 s by default — the < 60 s suspension promise
/// holds with margin). Semantics are strict:
///
/// - a FRESH entry (age < TTL against the REQUEST clock) answers without
///   touching the backend — positive AND negative results cache alike
///   (an unknown tenant probed in a loop must not hammer the table, and
///   a CREATION propagates within the same bound as a suspension);
/// - a stale or absent entry reads through; a backend that cannot answer
///   is `Err` — the cache NEVER serves a stale value past its TTL
///   (fail-closed, no stale-while-error).
pub struct CachedControl<S> {
    inner: S,
    ttl_ms: i64,
    tenants: Mutex<HashMap<String, (i64, TenantState)>>,
    bindings: Mutex<HashMap<(String, String), (i64, bool)>>,
    tunnels: Mutex<HashMap<String, (i64, Option<TunnelBinding>)>>,
}

impl<S: ControlStore> CachedControl<S> {
    pub fn new(inner: S, ttl_secs: u64) -> Self {
        Self {
            inner,
            ttl_ms: i64::try_from(ttl_secs)
                .unwrap_or(i64::MAX)
                .saturating_mul(1000),
            tenants: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
            tunnels: Mutex::new(HashMap::new()),
        }
    }

    fn fresh<K: std::hash::Hash + Eq, V: Clone>(
        map: &Mutex<HashMap<K, (i64, V)>>,
        key: &K,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Option<V> {
        let map = map.lock().expect("control cache poisoned");
        map.get(key).and_then(|(at, value)| {
            // The request clock may be re-injected backwards in tests; a
            // "future" entry is stale too — freshness is a WINDOW.
            let age = now_ms.saturating_sub(*at);
            ((0..ttl_ms).contains(&age)).then(|| value.clone())
        })
    }

    fn store<K: std::hash::Hash + Eq, V>(
        map: &Mutex<HashMap<K, (i64, V)>>,
        key: K,
        now_ms: i64,
        value: V,
    ) {
        map.lock()
            .expect("control cache poisoned")
            .insert(key, (now_ms, value));
    }
}

impl<S: ControlStore> ControlStore for CachedControl<S> {
    fn tenant_state<'a>(&'a self, tenant: &'a str, now_ms: i64) -> ControlFuture<'a, TenantState> {
        Box::pin(async move {
            if let Some(state) = Self::fresh(&self.tenants, &tenant.to_owned(), now_ms, self.ttl_ms)
            {
                return Ok(state);
            }
            let state = self.inner.tenant_state(tenant, now_ms).await?;
            Self::store(&self.tenants, tenant.to_owned(), now_ms, state);
            Ok(state)
        })
    }

    fn did_bound<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        now_ms: i64,
    ) -> ControlFuture<'a, bool> {
        Box::pin(async move {
            let key = (tenant.to_owned(), did.to_owned());
            if let Some(bound) = Self::fresh(&self.bindings, &key, now_ms, self.ttl_ms) {
                return Ok(bound);
            }
            let bound = self.inner.did_bound(tenant, did, now_ms).await?;
            Self::store(&self.bindings, key, now_ms, bound);
            Ok(bound)
        })
    }

    fn resolve_tunnel<'a>(
        &'a self,
        gateway_pub: &'a str,
        now_ms: i64,
    ) -> ControlFuture<'a, Option<TunnelBinding>> {
        Box::pin(async move {
            if let Some(binding) =
                Self::fresh(&self.tunnels, &gateway_pub.to_owned(), now_ms, self.ttl_ms)
            {
                return Ok(binding);
            }
            let binding = self.inner.resolve_tunnel(gateway_pub, now_ms).await?;
            Self::store(
                &self.tunnels,
                gateway_pub.to_owned(),
                now_ms,
                binding.clone(),
            );
            Ok(binding)
        })
    }
}

// --------------------------------------------------------------- dynamodb

/// The P7 table backend (module Terraform `control-plane-min`, single
/// table, composite key `pk`/`sk`):
///
/// | `pk`                  | `sk`          | attributes                          |
/// |-----------------------|---------------|-------------------------------------|
/// | `tenant#<tenant>`     | `meta`        | `s` (BOOL, suspended)               |
/// | `tenant#<tenant>`     | `did#<did>`   | — (presence is the binding)         |
/// | `gateway#<gw_pub>`    | `meta`        | `t`, `h` (S), `s` (BOOL, suspended) |
///
/// Reads are plain `GetItem` (eventually consistent — well inside the
/// freshness bound); a malformed item is unanswerable, never a phantom
/// absence (fail-closed, `heads.rs` precedent). The admin CLI
/// (`aithos-store-admin`) is the only writer; the task role carries the
/// reader policy alone.
pub struct DynamoDbControl {
    client: aws_sdk_dynamodb::Client,
    table: String,
}

impl DynamoDbControl {
    pub fn new(client: aws_sdk_dynamodb::Client, table: String) -> Self {
        Self { client, table }
    }

    async fn get(
        &self,
        pk: String,
        sk: &str,
    ) -> Result<Option<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>>, ControlUnavailable>
    {
        use aws_sdk_dynamodb::types::AttributeValue;
        let got = self
            .client
            .get_item()
            .table_name(&self.table)
            .key("pk", AttributeValue::S(pk))
            .key("sk", AttributeValue::S(sk.to_owned()))
            .send()
            .await
            .map_err(|_| ControlUnavailable)?;
        Ok(got.item.map(|item| item.into_iter().collect()))
    }

    fn suspended_of(
        item: &HashMap<String, aws_sdk_dynamodb::types::AttributeValue>,
    ) -> Result<bool, ControlUnavailable> {
        match item.get("s") {
            None => Ok(false),
            Some(v) => v.as_bool().copied().map_err(|_| ControlUnavailable),
        }
    }
}

impl ControlStore for DynamoDbControl {
    fn tenant_state<'a>(&'a self, tenant: &'a str, _now_ms: i64) -> ControlFuture<'a, TenantState> {
        Box::pin(async move {
            match self.get(format!("tenant#{tenant}"), "meta").await? {
                None => Ok(TenantState::Unknown),
                Some(item) if Self::suspended_of(&item)? => Ok(TenantState::Suspended),
                Some(_) => Ok(TenantState::Active),
            }
        })
    }

    fn did_bound<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        _now_ms: i64,
    ) -> ControlFuture<'a, bool> {
        Box::pin(async move {
            Ok(self
                .get(format!("tenant#{tenant}"), &format!("did#{did}"))
                .await?
                .is_some())
        })
    }

    fn resolve_tunnel<'a>(
        &'a self,
        gateway_pub: &'a str,
        _now_ms: i64,
    ) -> ControlFuture<'a, Option<TunnelBinding>> {
        Box::pin(async move {
            match self.get(format!("gateway#{gateway_pub}"), "meta").await? {
                None => Ok(None),
                Some(item) => {
                    let field = |name: &str| -> Result<String, ControlUnavailable> {
                        item.get(name)
                            .and_then(|v| v.as_s().ok())
                            .map(|v| v.to_owned())
                            .ok_or(ControlUnavailable)
                    };
                    Ok(Some(TunnelBinding {
                        tenant: field("t")?,
                        hostname: field("h")?,
                        suspended: Self::suspended_of(&item)?,
                    }))
                }
            }
        })
    }
}

/// `Arc` delegation — the composition root (and the test harness, which
/// keeps an admin handle on its double) hands one seam whatever the
/// backend shape.
impl<S: ControlStore + ?Sized> ControlStore for Arc<S> {
    fn tenant_state<'a>(&'a self, tenant: &'a str, now_ms: i64) -> ControlFuture<'a, TenantState> {
        self.as_ref().tenant_state(tenant, now_ms)
    }

    fn did_bound<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        now_ms: i64,
    ) -> ControlFuture<'a, bool> {
        self.as_ref().did_bound(tenant, did, now_ms)
    }

    fn resolve_tunnel<'a>(
        &'a self,
        gateway_pub: &'a str,
        now_ms: i64,
    ) -> ControlFuture<'a, Option<TunnelBinding>> {
        self.as_ref().resolve_tunnel(gateway_pub, now_ms)
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
        let (plane, preloads, heads) =
            ControlPlane::from_bootstrap_json(&raw).expect("bootstrap loads");
        // The static replay bootstrap seeds no heads: genesis walks
        // through the CAS (the per-case replay harness seeds its own).
        assert!(heads.is_empty());

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
                "did.json".to_owned(),
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
        let (plane, _, _) = ControlPlane::from_bootstrap_json(r#"{"tenants":[]}"#).unwrap();
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
        let (plane, _, _) = ControlPlane::from_bootstrap_json(raw).unwrap();
        let bound = plane.resolve_tunnel("z6MkGw1").expect("gw1 bound");
        assert_eq!(bound.tenant, "acme");
        assert_eq!(bound.hostname, "demo.mcp.aithos.fr");
        assert!(!bound.suspended);
        assert!(plane.resolve_tunnel("z6MkGw2").unwrap().suspended);
        assert!(plane.resolve_tunnel("z6MkNope").is_none());
    }

    // ------------------------------------------------ P7 freshness cache

    /// A scripted backend: answers from a mutable map, counts reads, and
    /// refuses when told to — the CachedControl contract under test.
    struct Scripted {
        state: Mutex<(TenantState, bool)>, // (state, down)
        reads: std::sync::atomic::AtomicUsize,
    }

    impl Default for Scripted {
        fn default() -> Self {
            Self {
                state: Mutex::new((TenantState::Unknown, false)),
                reads: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl Scripted {
        fn set(&self, state: TenantState) {
            self.state.lock().unwrap().0 = state;
        }
        fn down(&self, down: bool) {
            self.state.lock().unwrap().1 = down;
        }
        fn reads(&self) -> usize {
            self.reads.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl ControlStore for Scripted {
        fn tenant_state<'a>(
            &'a self,
            _tenant: &'a str,
            _now_ms: i64,
        ) -> ControlFuture<'a, TenantState> {
            Box::pin(async move {
                self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let (state, down) = *self.state.lock().unwrap();
                if down {
                    return Err(ControlUnavailable);
                }
                Ok(state)
            })
        }

        fn did_bound<'a>(
            &'a self,
            _tenant: &'a str,
            _did: &'a str,
            _now_ms: i64,
        ) -> ControlFuture<'a, bool> {
            Box::pin(async move { Ok(false) })
        }

        fn resolve_tunnel<'a>(
            &'a self,
            _gateway_pub: &'a str,
            _now_ms: i64,
        ) -> ControlFuture<'a, Option<TunnelBinding>> {
            Box::pin(async move { Ok(None) })
        }
    }

    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        futures::executor::block_on(f)
    }

    #[test]
    fn a_fresh_entry_answers_without_the_backend() {
        let inner = Arc::new(Scripted::default());
        inner.set(TenantState::Active);
        let cached = CachedControl::new(inner.clone(), 30);
        assert_eq!(
            block(cached.tenant_state("acme", 1_000)),
            Ok(TenantState::Active)
        );
        // 29.999 s later: still fresh — no second read.
        assert_eq!(
            block(cached.tenant_state("acme", 30_999)),
            Ok(TenantState::Active)
        );
        assert_eq!(inner.reads(), 1);
        // Exactly the bound: stale — the write below is now visible.
        inner.set(TenantState::Suspended);
        assert_eq!(
            block(cached.tenant_state("acme", 31_000)),
            Ok(TenantState::Suspended)
        );
        assert_eq!(inner.reads(), 2);
    }

    #[test]
    fn negative_results_cache_and_propagate_within_the_same_bound() {
        let inner = Arc::new(Scripted::default());
        inner.set(TenantState::Unknown);
        let cached = CachedControl::new(inner.clone(), 30);
        assert_eq!(
            block(cached.tenant_state("acme", 0)),
            Ok(TenantState::Unknown)
        );
        // The creation lands; within the bound the negative entry serves.
        inner.set(TenantState::Active);
        assert_eq!(
            block(cached.tenant_state("acme", 10_000)),
            Ok(TenantState::Unknown)
        );
        // Past the bound: the creation is live — no redeploy anywhere.
        assert_eq!(
            block(cached.tenant_state("acme", 30_000)),
            Ok(TenantState::Active)
        );
    }

    #[test]
    fn the_cache_never_serves_a_stale_value_past_its_ttl() {
        let inner = Arc::new(Scripted::default());
        inner.set(TenantState::Active);
        let cached = CachedControl::new(inner.clone(), 30);
        assert_eq!(
            block(cached.tenant_state("acme", 0)),
            Ok(TenantState::Active)
        );
        // Backend mute + entry expired: refuse — NEVER the stale Active.
        inner.down(true);
        assert_eq!(
            block(cached.tenant_state("acme", 60_000)),
            Err(ControlUnavailable)
        );
        // Within the bound the fresh entry still serves (an outage does
        // not amputate the freshness window it already paid for).
        assert_eq!(
            block(cached.tenant_state("acme", 29_999)),
            Ok(TenantState::Active)
        );
    }

    // ------------------------------------------- P7b gateway authority

    /// The B.5 order adopted by B.2 step 4 (P7b): binding → binding
    /// suspension → tenant state. An orphan binding (tenant unknown) is a
    /// mapping_mismatch, a suspended tenant gates its tunnels in ONE
    /// tenant-level write.
    #[test]
    fn gateway_authority_joins_the_tenant_state() {
        let mut plane = ControlPlane::default();
        plane.bind_tunnel(
            "z6MkGw".into(),
            TunnelBinding {
                tenant: "acme".into(),
                hostname: "demo.mcp.aithos.fr".into(),
                suspended: false,
            },
        );
        // Orphan binding: the tenant row does not exist.
        assert_eq!(
            block(authorize_gateway(&plane, "z6MkGw", 0)),
            Err(GatewayAuthzRefusal::MappingMismatch)
        );
        // Active tenant: the binding resolves.
        plane.seed_tenant("acme", false);
        let bound = block(authorize_gateway(&plane, "z6MkGw", 0)).expect("authorized");
        assert_eq!(bound.hostname, "demo.mcp.aithos.fr");
        // Tenant-level suspension gates the tunnel (one write).
        plane.seed_tenant("acme", true);
        assert_eq!(
            block(authorize_gateway(&plane, "z6MkGw", 0)),
            Err(GatewayAuthzRefusal::Suspended)
        );
        // No binding at all: the same mapping_mismatch as the orphan.
        assert_eq!(
            block(authorize_gateway(&plane, "z6MkGhost", 0)),
            Err(GatewayAuthzRefusal::MappingMismatch)
        );
    }

    /// Binding-level suspension still precedes the tenant join (B.2 order).
    #[test]
    fn a_suspended_binding_refuses_before_the_tenant_join() {
        let mut plane = ControlPlane::default();
        plane.seed_tenant("acme", false);
        plane.bind_tunnel(
            "z6MkGw".into(),
            TunnelBinding {
                tenant: "acme".into(),
                hostname: "demo.mcp.aithos.fr".into(),
                suspended: true,
            },
        );
        assert_eq!(
            block(authorize_gateway(&plane, "z6MkGw", 0)),
            Err(GatewayAuthzRefusal::Suspended)
        );
    }

    /// An unanswerable backend refuses `Unavailable` — never a phantom
    /// mismatch (fail-closed, the étape-6 seam pattern).
    #[test]
    fn an_unanswerable_backend_refuses_unavailable() {
        struct DownResolver;
        impl ControlStore for DownResolver {
            fn tenant_state<'a>(
                &'a self,
                _tenant: &'a str,
                _now_ms: i64,
            ) -> ControlFuture<'a, TenantState> {
                Box::pin(async move { Err(ControlUnavailable) })
            }
            fn did_bound<'a>(
                &'a self,
                _tenant: &'a str,
                _did: &'a str,
                _now_ms: i64,
            ) -> ControlFuture<'a, bool> {
                Box::pin(async move { Err(ControlUnavailable) })
            }
            fn resolve_tunnel<'a>(
                &'a self,
                _gateway_pub: &'a str,
                _now_ms: i64,
            ) -> ControlFuture<'a, Option<TunnelBinding>> {
                Box::pin(async move { Err(ControlUnavailable) })
            }
        }
        assert_eq!(
            block(authorize_gateway(&DownResolver, "z6MkGw", 0)),
            Err(GatewayAuthzRefusal::Unavailable)
        );
    }

    #[test]
    fn a_backwards_clock_is_stale_too() {
        let inner = Arc::new(Scripted::default());
        inner.set(TenantState::Active);
        let cached = CachedControl::new(inner.clone(), 30);
        assert_eq!(
            block(cached.tenant_state("acme", 100_000)),
            Ok(TenantState::Active)
        );
        // A request clock BEFORE the entry's birth re-reads (freshness is
        // a window, not a signed distance).
        assert_eq!(
            block(cached.tenant_state("acme", 50_000)),
            Ok(TenantState::Active)
        );
        assert_eq!(inner.reads(), 2);
    }
}
