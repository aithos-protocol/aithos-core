//! The A.5 heads table — the two hot heads of one `(tenant, did)`.
//!
//! **The CAS is the seam.** One record per `(tenant, did)`:
//! `{height, manifest_chain_hash, gamma_head, gamma_segment}` (annexe A.5),
//! swapped as an OPAQUE atomic compare-and-swap: the caller presents the
//! exact record it read and the record it wants; the table either swaps or
//! returns the current truth. The store never arbitrates a fork — this
//! serialization point is the whole story, the witness observes (annexe C).
//!
//! **Étape 6 shape:** the seam speaks `Result` — a table that cannot
//! answer refuses (`503 unavailable`, the nonce precedent), never a
//! silent accept or an invented absence. Two backends: the in-process map
//! (dev/tests, replay) and the DynamoDB conditional write (module
//! Terraform `store-api`). The [`crate::objects::ObjectStore`] seam
//! deliberately has no head CAS — nothing else may pretend to be the CAS.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// The backend cannot answer. Fixed cause only (discipline A.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadsUnavailable;

/// Boxed seam future, house style (`objects.rs`).
pub type HeadsFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, HeadsUnavailable>> + Send + 'a>>;

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
/// field, never exposed on the wire (redline gate 4 in A.5).
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
    /// Read the record for `(tenant, did)`; `Ok(None)` = never written,
    /// `Err` = the table could not answer (the caller refuses 503).
    fn read<'a>(&'a self, tenant: &'a str, did: &'a str) -> HeadsFuture<'a, Option<HeadsRecord>>;

    /// Atomic compare-and-swap: install `next` iff the current record is
    /// exactly `expected` (`None` = the record must not exist). On
    /// conflict, returns the CURRENT record (`None` = still absent) and
    /// installs nothing — the caller answers `cas_mismatch` with it. An
    /// unanswerable table is `Err` — never a swap, never a conflict.
    #[allow(clippy::type_complexity)]
    fn cas<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        expected: Option<&'a HeadsRecord>,
        next: HeadsRecord,
    ) -> HeadsFuture<'a, Result<(), Option<HeadsRecord>>>;
}

/// The in-memory backend. Per-instance and ephemeral by design (dev,
/// tests, replay); DynamoDB is the deployed backend behind the same trait.
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
    fn read<'a>(&'a self, tenant: &'a str, did: &'a str) -> HeadsFuture<'a, Option<HeadsRecord>> {
        Box::pin(async move {
            Ok(self
                .map
                .lock()
                .expect("heads map poisoned")
                .get(&(tenant.to_owned(), did.to_owned()))
                .cloned())
        })
    }

    fn cas<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        expected: Option<&'a HeadsRecord>,
        next: HeadsRecord,
    ) -> HeadsFuture<'a, Result<(), Option<HeadsRecord>>> {
        Box::pin(async move {
            let mut map = self.map.lock().expect("heads map poisoned");
            let key = (tenant.to_owned(), did.to_owned());
            let current = map.get(&key);
            if current != expected {
                return Ok(Err(current.cloned()));
            }
            map.insert(key, next);
            Ok(Ok(()))
        })
    }
}

/// The deployed backend (étape 6): one item per `(tenant, did)` on a
/// DynamoDB table — partition key `t` (tenant), sort key `d` (DID);
/// attributes `h` (height, N), `m` (manifest_chain_hash, S), `g`
/// (gamma_head, S), `gs` (gamma_segment, S), `months` (gamma_segments,
/// S — the `,`-joined month list; an implementation attribute beside the
/// A.5 tuple, never on the wire). The CAS is one conditional `PutItem`;
/// reads are strongly consistent — a stale read would fabricate CAS
/// conflicts with phantom heads.
pub struct DynamoDbHeads {
    client: aws_sdk_dynamodb::Client,
    table: String,
}

impl DynamoDbHeads {
    pub fn new(client: aws_sdk_dynamodb::Client, table: String) -> Self {
        Self { client, table }
    }

    fn record_of(
        item: &HashMap<String, aws_sdk_dynamodb::types::AttributeValue>,
    ) -> Option<HeadsRecord> {
        let s = |name: &str| {
            item.get(name)
                .and_then(|v| v.as_s().ok())
                .map(|v| v.to_owned())
        };
        Some(HeadsRecord {
            height: item.get("h")?.as_n().ok()?.parse().ok()?,
            manifest_chain_hash: s("m")?,
            gamma_head: s("g")?,
            gamma_segment: s("gs")?,
            gamma_segments: match s("months")? {
                joined if joined.is_empty() => vec![],
                joined => joined.split(',').map(str::to_owned).collect(),
            },
        })
    }
}

impl HeadsTable for DynamoDbHeads {
    fn read<'a>(&'a self, tenant: &'a str, did: &'a str) -> HeadsFuture<'a, Option<HeadsRecord>> {
        Box::pin(async move {
            use aws_sdk_dynamodb::types::AttributeValue;
            let got = self
                .client
                .get_item()
                .table_name(&self.table)
                .key("t", AttributeValue::S(tenant.to_owned()))
                .key("d", AttributeValue::S(did.to_owned()))
                .consistent_read(true)
                .send()
                .await
                .map_err(|_| HeadsUnavailable)?;
            match got.item() {
                None => Ok(None),
                // A malformed item is unanswerable, never a phantom
                // absence (fail-closed).
                Some(item) => Self::record_of(item).map(Some).ok_or(HeadsUnavailable),
            }
        })
    }

    fn cas<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        expected: Option<&'a HeadsRecord>,
        next: HeadsRecord,
    ) -> HeadsFuture<'a, Result<(), Option<HeadsRecord>>> {
        Box::pin(async move {
            use aws_sdk_dynamodb::error::SdkError;
            use aws_sdk_dynamodb::operation::put_item::PutItemError;
            use aws_sdk_dynamodb::types::AttributeValue;

            let mut put = self
                .client
                .put_item()
                .table_name(&self.table)
                .item("t", AttributeValue::S(tenant.to_owned()))
                .item("d", AttributeValue::S(did.to_owned()))
                .item("h", AttributeValue::N(next.height.to_string()))
                .item("m", AttributeValue::S(next.manifest_chain_hash))
                .item("g", AttributeValue::S(next.gamma_head))
                .item("gs", AttributeValue::S(next.gamma_segment))
                .item("months", AttributeValue::S(next.gamma_segments.join(",")));
            // Expression attribute NAMES throughout: short attribute
            // names never collide with DynamoDB's reserved-word list.
            put = match expected {
                None => put
                    .condition_expression("attribute_not_exists(#t)")
                    .expression_attribute_names("#t", "t"),
                Some(record) => put
                    .condition_expression(
                        "#h = :h AND #m = :m AND #g = :g AND #gs = :gs AND #months = :months",
                    )
                    .expression_attribute_names("#h", "h")
                    .expression_attribute_names("#m", "m")
                    .expression_attribute_names("#g", "g")
                    .expression_attribute_names("#gs", "gs")
                    .expression_attribute_names("#months", "months")
                    .expression_attribute_values(":h", AttributeValue::N(record.height.to_string()))
                    .expression_attribute_values(
                        ":m",
                        AttributeValue::S(record.manifest_chain_hash.clone()),
                    )
                    .expression_attribute_values(":g", AttributeValue::S(record.gamma_head.clone()))
                    .expression_attribute_values(
                        ":gs",
                        AttributeValue::S(record.gamma_segment.clone()),
                    )
                    .expression_attribute_values(
                        ":months",
                        AttributeValue::S(record.gamma_segments.join(",")),
                    ),
            };
            match put.send().await {
                Ok(_) => Ok(Ok(())),
                Err(SdkError::ServiceError(e))
                    if matches!(e.err(), PutItemError::ConditionalCheckFailedException(_)) =>
                {
                    // The loser learns the CURRENT truth — a consistent
                    // read, never the stale expectation.
                    Ok(Err(self.read(tenant, did).await?))
                }
                Err(_) => Err(HeadsUnavailable),
            }
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
            Ok(Ok(()))
        );
        // A second genesis on the same key loses and learns the truth.
        assert_eq!(
            futures::executor::block_on(table.cas("acme", "did:aithos:zX", None, rec(1, "bb", ""))),
            Ok(Err(Some(first.clone())))
        );
        assert_eq!(
            futures::executor::block_on(table.read("acme", "did:aithos:zX")),
            Ok(Some(first))
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
            Ok(Err(Some(v1.clone())))
        );
        // Exact expectation: swapped.
        assert_eq!(
            futures::executor::block_on(table.cas("acme", "did:aithos:zX", Some(&v1), v2.clone())),
            Ok(Ok(()))
        );
        assert_eq!(
            futures::executor::block_on(table.read("acme", "did:aithos:zX")),
            Ok(Some(v2))
        );
    }

    #[test]
    fn reads_are_per_key_and_fail_closed() {
        let table = MemHeads::new();
        table.seed("acme", "did:aithos:zX", rec(1, "aa", "sha256:gg"));
        assert_eq!(
            futures::executor::block_on(table.read("acme", "did:aithos:zY")),
            Ok(None)
        );
        assert_eq!(
            futures::executor::block_on(table.read("ghost", "did:aithos:zX")),
            Ok(None)
        );
    }
}
