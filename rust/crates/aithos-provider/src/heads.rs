//! The A.5 heads table — the two hot heads of one `(tenant, did)`.
//!
//! **The CAS is the seam.** One record per `(tenant, did)`:
//! `{height, manifest_chain_hash, gamma_head, gamma_segment}` (annexe A.5),
//! swapped as an OPAQUE atomic compare-and-swap: the caller presents the
//! exact record it read and the record it wants; the table either swaps or
//! returns the current truth. The store never arbitrates a fork — this
//! serialization point is the whole story, the witness observes (annexe C).
//!
//! **P2 shape:** an in-process map. Étape 6 replaces the backend with the
//! DynamoDB conditional write (transaction with the S3 deposit) behind this
//! same seam. The [`crate::objects::ObjectStore`] seam deliberately has no
//! conditional write — nothing else may pretend to be the CAS.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// One heads record — the A.5 tuple.
///
/// Wire conventions (the committed p7 vector's): `manifest_chain_hash` is
/// the BARE 64-hex SHA-256 the successor pins as `edition.prev_hash`
/// (`""` = no manifest yet); `gamma_head` is the `sha256:<hex>`-prefixed
/// value the next entry pins as `prev` (`""` = empty log); `gamma_segment`
/// is the `YYYY-MM` of the segment holding the head entry (`""` = none).
///
/// `gamma_segments` lists every month segment this DID ever appended to —
/// the merge set the #9 revocation scan reads (pointer log + appended
/// segments). It is an implementation detail of the store, not an A.5
/// field; the pointer-vs-segments tension is arbitrage ③ of the gate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeadsRecord {
    pub height: u64,
    pub manifest_chain_hash: String,
    pub gamma_head: String,
    pub gamma_segment: String,
    pub gamma_segments: Vec<String>,
}

/// Object-safe async seam, house style (`objects.rs`).
pub trait HeadsTable: Send + Sync {
    /// Read the record for `(tenant, did)`; `None` = never written.
    fn read<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
    ) -> Pin<Box<dyn Future<Output = Option<HeadsRecord>> + Send + 'a>>;

    /// Atomic compare-and-swap: install `next` iff the current record is
    /// exactly `expected` (`None` = the record must not exist). On
    /// conflict, returns the CURRENT record (`None` = still absent) and
    /// installs nothing — the caller answers `cas_mismatch` with it.
    fn cas<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        expected: Option<&'a HeadsRecord>,
        next: HeadsRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), Option<HeadsRecord>>> + Send + 'a>>;
}

/// The P2 in-memory backend. Per-instance and ephemeral by design;
/// DynamoDB lands at étape 6 behind the same trait.
#[derive(Default)]
pub struct MemHeads {
    map: Mutex<HashMap<(String, String), HeadsRecord>>,
}

impl MemHeads {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed one record at startup (bootstrap/replay fixtures) — a plain
    /// write, deliberately NOT the CAS: enrollment precedes, it does not
    /// race (the P7 control plane owns this in production).
    pub fn seed(&self, tenant: &str, did: &str, record: HeadsRecord) {
        self.map
            .lock()
            .expect("heads map poisoned")
            .insert((tenant.to_owned(), did.to_owned()), record);
    }
}

impl HeadsTable for MemHeads {
    fn read<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
    ) -> Pin<Box<dyn Future<Output = Option<HeadsRecord>> + Send + 'a>> {
        Box::pin(async move {
            self.map
                .lock()
                .expect("heads map poisoned")
                .get(&(tenant.to_owned(), did.to_owned()))
                .cloned()
        })
    }

    fn cas<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        expected: Option<&'a HeadsRecord>,
        next: HeadsRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), Option<HeadsRecord>>> + Send + 'a>> {
        Box::pin(async move {
            let mut map = self.map.lock().expect("heads map poisoned");
            let key = (tenant.to_owned(), did.to_owned());
            let current = map.get(&key);
            if current != expected {
                return Err(current.cloned());
            }
            map.insert(key, next);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(height: u64, manifest: &str, gamma: &str) -> HeadsRecord {
        HeadsRecord {
            height,
            manifest_chain_hash: manifest.into(),
            gamma_head: gamma.into(),
            gamma_segment: if gamma.is_empty() {
                String::new()
            } else {
                "2026-07".into()
            },
            gamma_segments: if gamma.is_empty() {
                vec![]
            } else {
                vec!["2026-07".into()]
            },
        }
    }

    #[test]
    fn cas_from_absent_installs_once() {
        let table = MemHeads::new();
        let first = rec(1, "aa", "");
        assert_eq!(
            futures::executor::block_on(table.cas("acme", "did:aithos:zX", None, first.clone())),
            Ok(())
        );
        // A second genesis on the same key loses and learns the truth.
        assert_eq!(
            futures::executor::block_on(table.cas("acme", "did:aithos:zX", None, rec(1, "bb", ""))),
            Err(Some(first.clone()))
        );
        assert_eq!(
            futures::executor::block_on(table.read("acme", "did:aithos:zX")),
            Some(first)
        );
    }

    #[test]
    fn cas_swaps_only_on_the_exact_expected_record() {
        let table = MemHeads::new();
        let v1 = rec(1, "aa", "");
        table.seed("acme", "did:aithos:zX", v1.clone());
        let v2 = rec(2, "bb", "");
        // Stale expectation (twin race): refused, current returned.
        assert_eq!(
            futures::executor::block_on(table.cas(
                "acme",
                "did:aithos:zX",
                Some(&rec(1, "other", "")),
                v2.clone()
            )),
            Err(Some(v1.clone()))
        );
        // Exact expectation: swapped.
        assert_eq!(
            futures::executor::block_on(table.cas("acme", "did:aithos:zX", Some(&v1), v2.clone())),
            Ok(())
        );
        assert_eq!(
            futures::executor::block_on(table.read("acme", "did:aithos:zX")),
            Some(v2)
        );
    }

    #[test]
    fn reads_are_per_key_and_fail_closed() {
        let table = MemHeads::new();
        table.seed("acme", "did:aithos:zX", rec(1, "aa", "sha256:gg"));
        assert_eq!(
            futures::executor::block_on(table.read("acme", "did:aithos:zY")),
            None
        );
        assert_eq!(
            futures::executor::block_on(table.read("ghost", "did:aithos:zX")),
            None
        );
    }
}
