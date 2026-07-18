//! DNS TXT seam for the delegated ACME DNS-01 — annexe B.5 (lot P6,
//! jalon M2).
//!
//! The store's ONLY write into DNS: pose/retire
//! `TXT _acme-challenge.<hostname>` (TTL 60 s) in the delegated mcp zone.
//! The IAM policy (module `dns`, `acme_txt_policy_arn`) scopes the task
//! role to exactly that: TXT records, `_acme-challenge.*` names, one
//! zone. This module never sees a key, a cert or a chain — the pod runs
//! the ACME conversation and keeps its private key (A3); the store just
//! moves one record.
//!
//! Semantics, shared by both backends and graved by the p6 vector:
//! - `upsert(name, value)` — the record set becomes exactly `[value]`
//!   (one live challenge per hostname; a fresh PUT replaces the old);
//! - `delete(name, value)` — retire the `(name, value)` pair. Absence,
//!   or a different live value (a newer challenge), is SUCCESS: the pair
//!   named by the caller is gone either way (idempotent cleanup).
//!
//! Fail-closed: a backend failure is an error the caller turns into a
//! `503` refusal — never into a silent acceptance. The service-side purge
//! (annexe B.5: « de toute façon purgé après 10 min ») lives in
//! [`crate::acme`]; this seam only moves records.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// TTL of the challenge TXT record (annexe B.5: 60 s).
pub const ACME_TXT_TTL_SECS: i64 = 60;

/// Fixed-cause failure (redaction discipline A.8: no backend message, no
/// record material ever travels in an error).
#[derive(Debug, thiserror::Error)]
#[error("dns backend unavailable")]
pub struct DnsUnavailable;

/// Object-safe async seam, house style (`NonceStore`, `ObjectStore`).
pub trait DnsTxt: Send + Sync {
    /// Make `name`'s TXT record set exactly `[value]`, TTL 60.
    fn upsert<'a>(
        &'a self,
        name: &'a str,
        value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>>;

    /// Retire the `(name, value)` pair; absence or a newer value is
    /// success (the named pair is gone either way).
    fn delete<'a>(
        &'a self,
        name: &'a str,
        value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>>;
}

// ----------------------------------------------------------- in memory

/// Test/dev backend: a process-local record table, inspectable by the
/// BDD harness (`record_of`) so the DNS effect is asserted, not assumed.
#[derive(Default)]
pub struct MemDnsTxt {
    records: Mutex<HashMap<String, (String, i64)>>,
}

impl MemDnsTxt {
    pub fn new() -> Self {
        Self::default()
    }

    /// The live `(value, ttl)` of `name`, if any — test inspection only.
    pub fn record_of(&self, name: &str) -> Option<(String, i64)> {
        self.records
            .lock()
            .expect("dns table poisoned")
            .get(name)
            .cloned()
    }
}

impl DnsTxt for MemDnsTxt {
    fn upsert<'a>(
        &'a self,
        name: &'a str,
        value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>> {
        Box::pin(async move {
            self.records
                .lock()
                .expect("dns table poisoned")
                .insert(name.to_owned(), (value.to_owned(), ACME_TXT_TTL_SECS));
            Ok(())
        })
    }

    fn delete<'a>(
        &'a self,
        name: &'a str,
        value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>> {
        Box::pin(async move {
            let mut records = self.records.lock().expect("dns table poisoned");
            if records.get(name).is_some_and(|(v, _)| v == value) {
                records.remove(name);
            }
            Ok(())
        })
    }
}

// ------------------------------------------------------------- disabled

/// The fail-closed default when no DNS backend is configured: every
/// effect refuses (`503 unavailable`). An old task definition can still
/// boot the new binary — the data plane serves, the acme surface refuses.
pub struct NoDnsTxt;

impl DnsTxt for NoDnsTxt {
    fn upsert<'a>(
        &'a self,
        _name: &'a str,
        _value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>> {
        Box::pin(async { Err(DnsUnavailable) })
    }

    fn delete<'a>(
        &'a self,
        _name: &'a str,
        _value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>> {
        Box::pin(async { Err(DnsUnavailable) })
    }
}

// -------------------------------------------------------------- Route 53

/// The deployed backend: `ChangeResourceRecordSets` on the delegated mcp
/// zone. UPSERT replaces the record set; DELETE names the exact pair and
/// maps "already gone / already replaced" to success (the deployed twin
/// of the memory semantics). The request path does NOT wait for INSYNC —
/// the pod owns its propagation wait (it is the one talking to the CA).
pub struct Route53DnsTxt {
    client: aws_sdk_route53::Client,
    zone_id: String,
}

impl Route53DnsTxt {
    pub fn new(client: aws_sdk_route53::Client, zone_id: String) -> Self {
        Self { client, zone_id }
    }

    async fn change(
        &self,
        action: aws_sdk_route53::types::ChangeAction,
        name: &str,
        value: &str,
    ) -> ChangeOutcome {
        use aws_sdk_route53::types::{
            Change, ChangeBatch, ResourceRecord, ResourceRecordSet, RrType,
        };
        let record = ResourceRecord::builder()
            // TXT wire form: the value travels quoted. The caller already
            // constrained it to the base64url alphabet — nothing to escape.
            .value(format!("\"{value}\""))
            .build();
        let record_set = record.and_then(|record| {
            ResourceRecordSet::builder()
                .name(name)
                .r#type(RrType::Txt)
                .ttl(ACME_TXT_TTL_SECS)
                .resource_records(record)
                .build()
        });
        let change = record_set.and_then(|record_set| {
            Change::builder()
                .action(action)
                .resource_record_set(record_set)
                .build()
        });
        let batch = change.and_then(|change| ChangeBatch::builder().changes(change).build());
        let Ok(batch) = batch else {
            return ChangeOutcome::Failed;
        };
        match self
            .client
            .change_resource_record_sets()
            .hosted_zone_id(&self.zone_id)
            .change_batch(batch)
            .send()
            .await
        {
            Ok(_) => ChangeOutcome::Applied,
            Err(e) => match aws_sdk_route53::Error::from(e) {
                aws_sdk_route53::Error::InvalidChangeBatch(_) => ChangeOutcome::InvalidChangeBatch,
                _ => ChangeOutcome::Failed,
            },
        }
    }
}

enum ChangeOutcome {
    Applied,
    /// Route 53 refused the batch as inconsistent with the live records —
    /// on DELETE this is "the named pair is not the live set".
    InvalidChangeBatch,
    Failed,
}

impl DnsTxt for Route53DnsTxt {
    fn upsert<'a>(
        &'a self,
        name: &'a str,
        value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>> {
        Box::pin(async move {
            // Fixed cause only: SDK messages never cross into our error
            // type (discipline A.8).
            match self
                .change(aws_sdk_route53::types::ChangeAction::Upsert, name, value)
                .await
            {
                ChangeOutcome::Applied => Ok(()),
                _ => Err(DnsUnavailable),
            }
        })
    }

    fn delete<'a>(
        &'a self,
        name: &'a str,
        value: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DnsUnavailable>> + Send + 'a>> {
        Box::pin(async move {
            match self
                .change(aws_sdk_route53::types::ChangeAction::Delete, name, value)
                .await
            {
                ChangeOutcome::Applied => Ok(()),
                // The pair is not the live record set (absent, or already
                // replaced by a newer challenge): the caller's pair is
                // gone — success, exactly the memory semantics.
                ChangeOutcome::InvalidChangeBatch => Ok(()),
                ChangeOutcome::Failed => Err(DnsUnavailable),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block<T>(f: impl Future<Output = T>) -> T {
        futures::executor::block_on(f)
    }

    #[test]
    fn upsert_replaces_and_delete_retires_the_named_pair_only() {
        let dns = MemDnsTxt::new();
        let name = "_acme-challenge.demo.mcp.aithos.fr";
        block(dns.upsert(name, "old")).unwrap();
        assert_eq!(dns.record_of(name), Some(("old".into(), 60)));
        // UPSERT replaces: one live challenge per hostname.
        block(dns.upsert(name, "new")).unwrap();
        assert_eq!(dns.record_of(name), Some(("new".into(), 60)));
        // Deleting the OLD pair is a no-op success (already replaced).
        block(dns.delete(name, "old")).unwrap();
        assert_eq!(dns.record_of(name), Some(("new".into(), 60)));
        // Deleting the live pair retires it; doing it again stays Ok.
        block(dns.delete(name, "new")).unwrap();
        assert_eq!(dns.record_of(name), None);
        block(dns.delete(name, "new")).unwrap();
    }

    #[test]
    fn the_disabled_backend_refuses_every_effect() {
        let dns = NoDnsTxt;
        assert!(block(dns.upsert("n", "v")).is_err());
        assert!(block(dns.delete("n", "v")).is_err());
    }
}
