//! Anti-rejeu reservation — annexe A.2 #6.
//!
//! `(key, nonce)` is reserved **insert-if-absent, before any side effect**:
//! a nonce burns on first sight even when a later check refuses the
//! request. The window is a MINIMUM (`≥ 600 s`): remembering a nonce
//! longer only strengthens anti-replay, which is exactly what DynamoDB's
//! lazy TTL deletion gives — the conditional put is the guarantee, the TTL
//! is garbage collection.
//!
//! Fail-closed: a store failure is an error the caller must turn into a
//! refusal, never into an acceptance.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// Minimum reservation window of annexe A.2 #6.
pub const MIN_WINDOW_SECS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reservation {
    /// Never seen inside the window: reserved now.
    Fresh,
    /// Already reserved: a replay.
    Replayed,
}

/// Fixed-cause failure (redaction discipline: no backend message, no
/// key/nonce material ever travels in an error).
#[derive(Debug, thiserror::Error)]
#[error("nonce store unavailable")]
pub struct NonceStoreUnavailable;

/// Object-safe async seam, same style as the gateway's `CredentialBroker`.
pub trait NonceStore: Send + Sync {
    fn reserve<'a>(
        &'a self,
        key: &'a str,
        nonce: &'a str,
        now_ms: i64,
    ) -> Pin<Box<dyn Future<Output = Result<Reservation, NonceStoreUnavailable>> + Send + 'a>>;
}

// ----------------------------------------------------------- in memory

/// Process-local reservation table — the test and single-instance backend.
/// Entries expire after the window; expiry only ever EXTENDS coverage
/// (pruning is lazy, like the DynamoDB TTL).
pub struct MemNonces {
    window_ms: i64,
    seen: Mutex<HashMap<(String, String), i64>>,
}

impl MemNonces {
    pub fn new(window_secs: i64) -> Self {
        Self {
            window_ms: window_secs.max(MIN_WINDOW_SECS) * 1000,
            seen: Mutex::new(HashMap::new()),
        }
    }
}

impl NonceStore for MemNonces {
    fn reserve<'a>(
        &'a self,
        key: &'a str,
        nonce: &'a str,
        now_ms: i64,
    ) -> Pin<Box<dyn Future<Output = Result<Reservation, NonceStoreUnavailable>> + Send + 'a>> {
        Box::pin(async move {
            let mut seen = self.seen.lock().expect("nonce table poisoned");
            // Inclusive endpoint: an entry expiring exactly NOW is still a
            // reservation — the window is `[t, t+window]`, closed on both
            // sides, so 600 ≥ 300+300 covers the skew boundary of A.2 #5
            // with no measure-zero replay point.
            seen.retain(|_, expiry| *expiry >= now_ms);
            match seen.entry((key.to_owned(), nonce.to_owned())) {
                std::collections::hash_map::Entry::Occupied(_) => Ok(Reservation::Replayed),
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(now_ms + self.window_ms);
                    Ok(Reservation::Fresh)
                }
            }
        })
    }
}

// ----------------------------------------------------------- DynamoDB

/// The deployed backend: one conditional `PutItem` per reservation on a
/// TTL table — `attribute_not_exists` is the insert-if-absent, the
/// `ConditionalCheckFailed` is the replay verdict. Table schema (module
/// Terraform `store-api`): partition key `k` (the envelope key), sort key
/// `n` (the nonce), TTL attribute `exp` (epoch seconds).
pub struct DynamoDbNonces {
    client: aws_sdk_dynamodb::Client,
    table: String,
    window_secs: i64,
}

impl DynamoDbNonces {
    pub fn new(client: aws_sdk_dynamodb::Client, table: String, window_secs: i64) -> Self {
        Self {
            client,
            table,
            window_secs: window_secs.max(MIN_WINDOW_SECS),
        }
    }
}

impl NonceStore for DynamoDbNonces {
    fn reserve<'a>(
        &'a self,
        key: &'a str,
        nonce: &'a str,
        now_ms: i64,
    ) -> Pin<Box<dyn Future<Output = Result<Reservation, NonceStoreUnavailable>> + Send + 'a>> {
        Box::pin(async move {
            use aws_sdk_dynamodb::error::SdkError;
            use aws_sdk_dynamodb::operation::put_item::PutItemError;
            use aws_sdk_dynamodb::types::AttributeValue;

            let exp = now_ms / 1000 + self.window_secs;
            let put = self
                .client
                .put_item()
                .table_name(&self.table)
                .item("k", AttributeValue::S(key.to_owned()))
                .item("n", AttributeValue::S(nonce.to_owned()))
                .item("exp", AttributeValue::N(exp.to_string()))
                .condition_expression("attribute_not_exists(k)")
                .send()
                .await;
            match put {
                Ok(_) => Ok(Reservation::Fresh),
                Err(SdkError::ServiceError(e))
                    if matches!(e.err(), PutItemError::ConditionalCheckFailedException(_)) =>
                {
                    Ok(Reservation::Replayed)
                }
                // Fixed cause only: SDK messages can carry request detail,
                // they never cross into our error type (discipline A.8).
                Err(_) => Err(NonceStoreUnavailable),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reserve(store: &MemNonces, key: &str, nonce: &str, now: i64) -> Reservation {
        futures::executor::block_on(store.reserve(key, nonce, now)).unwrap()
    }

    #[test]
    fn a_nonce_burns_on_first_sight() {
        let store = MemNonces::new(600);
        assert_eq!(reserve(&store, "#root", "n-1", 0), Reservation::Fresh);
        assert_eq!(reserve(&store, "#root", "n-1", 1), Reservation::Replayed);
        // Distinct key, same nonce: a different reservation (the pair is
        // the unit, annexe A.2 #6).
        assert_eq!(reserve(&store, "z6Mk", "n-1", 2), Reservation::Fresh);
    }

    #[test]
    fn the_window_is_a_minimum() {
        let store = MemNonces::new(0); // clamped up to 600 s
        assert_eq!(reserve(&store, "#root", "n-1", 0), Reservation::Fresh);
        // Still inside the clamped window — INCLUDING the exact endpoint
        // (a first sight at `at − 300 s` replayed at `at + 300 s` lands
        // precisely here; both instants pass the #5 skew check).
        assert_eq!(
            reserve(&store, "#root", "n-1", 599_999),
            Reservation::Replayed
        );
        assert_eq!(
            reserve(&store, "#root", "n-1", 600_000),
            Reservation::Replayed
        );
        // Past the window the entry may be pruned: a replay is then caught
        // by the ±300 s skew of #5, which is why 600 ≥ 300 + 300 suffices.
        assert_eq!(reserve(&store, "#root", "n-1", 600_001), Reservation::Fresh);
    }
}
