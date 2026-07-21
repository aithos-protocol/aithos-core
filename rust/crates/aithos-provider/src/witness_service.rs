//! The witness SERVICE (lot A / P5) — annexe C made operational.
//!
//! The witness **observes, never invents**: an event from the A.5 heads
//! table (the store's single serialization point) names a `(tenant, did,
//! height, manifest_chain_hash)`; the service fetches the manifest the
//! layout actually stores, recomputes its chain hash, and signs a
//! checkpoint ONLY when the observation coheres. A missing or mismatched
//! manifest (the documented crash window of A.5 — heads ahead of the
//! object) emits nothing and stays pending for the next sweep; the store
//! is never "repaired" from here (doctrine: the witness signs
//! observations, never authority, §4).
//!
//! Arbitrages Mathieu 2026-07-20 (session lot A) : déclencheur = DynamoDB
//! Streams sur la table heads (① — un append gamma seul n'avance pas
//! l'édition et n'émet rien) ; publication = S3 + CloudFront sur
//! `witness.aithos.fr` (②) ; un seul écrivain, `desired_count = 1` (③) ;
//! clé KMS native Ed25519 sign-only per annexe C.1 (④ — le seam
//! [`crate::witness::WitnessSigner`] reçoit l'impl KMS au binaire).
//!
//! **Idempotence C.2 is derived from the feed itself** — re-readable at
//! boot, never a process memory: re-seeing the same `(did,
//! edition_height, manifest_hash)` in the same UTC day emits nothing
//! outside the heartbeat; several lines of the same head across days (the
//! heartbeat) are freshness, never a fault. Two valid lines of the same
//! `did` and height with DIFFERENT manifest hashes are deliberately BOTH
//! emitted: the pair in the public feed is the C.4 portable proof.

use std::sync::Mutex;

use crate::objects::ObjectStore;
use crate::witness::{build_checkpoint, build_daily_root, feed_line, Checkpoint, WitnessSigner};

// ------------------------------------------------------------ feed seam

/// The feed backend cannot answer. Fixed cause only (discipline A.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedUnavailable;

/// A conditional put lost its race (the ETag moved) — the caller re-reads
/// and retries; with the single deployed writer (arbitrage ③) this only
/// guards against torn restarts, never a twin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedConflict;

pub type FeedFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, FeedUnavailable>> + Send + 'a>>;

/// Object-safe feed seam (house style, `objects.rs`): the public
/// publication surface of annexe C.3 — `<did>.jsonl` append-only feeds,
/// `roots/<YYYY-MM-DD>.json`, `keys.json`. Backends: in-process map
/// (tests) and the S3 feed bucket (module Terraform `witness`).
pub trait FeedStore: Send + Sync {
    /// Read `key`; `Ok(None)` = absent. The tag is an opaque
    /// generation marker (ETag) for [`FeedStore::put_if`].
    fn get<'a>(&'a self, key: &'a str) -> FeedFuture<'a, Option<(Vec<u8>, String)>>;

    /// Write `key` iff the stored generation is exactly `expected`
    /// (`None` = the key must not exist yet). `Ok(Err(FeedConflict))` =
    /// the generation moved — re-read and retry.
    #[allow(clippy::type_complexity)]
    fn put_if<'a>(
        &'a self,
        key: &'a str,
        bytes: Vec<u8>,
        expected: Option<&'a str>,
    ) -> FeedFuture<'a, Result<(), FeedConflict>>;

    /// List the keys under `prefix`, sorted.
    fn list<'a>(&'a self, prefix: &'a str) -> FeedFuture<'a, Vec<String>>;
}

/// The in-memory feed (tests, dev). Tags are write counters.
#[derive(Default)]
pub struct MemFeed {
    map: Mutex<std::collections::HashMap<String, (Vec<u8>, u64)>>,
}

impl MemFeed {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl FeedStore for MemFeed {
    fn get<'a>(&'a self, key: &'a str) -> FeedFuture<'a, Option<(Vec<u8>, String)>> {
        Box::pin(async move {
            Ok(self
                .map
                .lock()
                .expect("feed map poisoned")
                .get(key)
                .map(|(bytes, generation)| (bytes.clone(), generation.to_string())))
        })
    }

    fn put_if<'a>(
        &'a self,
        key: &'a str,
        bytes: Vec<u8>,
        expected: Option<&'a str>,
    ) -> FeedFuture<'a, Result<(), FeedConflict>> {
        Box::pin(async move {
            let mut map = self.map.lock().expect("feed map poisoned");
            let current = map.get(key).map(|(_, generation)| generation.to_string());
            if current.as_deref() != expected {
                return Ok(Err(FeedConflict));
            }
            let next = current
                .as_deref()
                .and_then(|g| g.parse::<u64>().ok())
                .map_or(1, |g| g + 1);
            map.insert(key.to_owned(), (bytes, next));
            Ok(Ok(()))
        })
    }

    fn list<'a>(&'a self, prefix: &'a str) -> FeedFuture<'a, Vec<String>> {
        Box::pin(async move {
            let mut keys: Vec<String> = self
                .map
                .lock()
                .expect("feed map poisoned")
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            keys.sort();
            Ok(keys)
        })
    }
}

// -------------------------------------------------------- observations

/// One heads-table event as the stream delivers it: the OLD height (if
/// the row existed) and the NEW record fields the observation needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadsEvent {
    pub tenant: String,
    pub did: String,
    /// The row's height before the write (`None` = the row is new).
    pub old_height: Option<u64>,
    /// The row's height after the write.
    pub height: u64,
    /// The row's `m` — the BARE 64-hex manifest chain hash (A.5).
    pub manifest_chain_hash: String,
}

/// A pending observation: the layout could not corroborate the announced
/// head (manifest absent or mismatched — the A.5 crash window). Retried
/// by [`WitnessService::sweep_pending`]; never dropped, never invented.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingObservation {
    tenant: String,
    did: String,
    height: u64,
    manifest_chain_hash: String,
}

/// The verdict of one observation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// A checkpoint was appended to the DID feed.
    Emitted,
    /// The same `(did, height, manifest_hash)` was already observed this
    /// UTC day (C.2) — nothing appended.
    AlreadyObserved,
    /// The event does not advance the edition (gamma-only) — no trigger.
    NotAnEdition,
    /// The layout does not (yet) corroborate the announced head; the
    /// observation is pending for the next sweep.
    Pending,
}

/// The witness service: the observation logic between the heads events,
/// the object layout and the public feed. All instants are the CALLER's
/// (`now` in RFC 3339 Z) — the binary passes the wall clock, the harness
/// injects; nothing here ever reads time.
pub struct WitnessService {
    signer: Box<dyn WitnessSigner>,
    objects: std::sync::Arc<dyn ObjectStore>,
    feed: std::sync::Arc<dyn FeedStore>,
    pending: Mutex<Vec<PendingObservation>>,
}

impl WitnessService {
    pub fn new(
        signer: Box<dyn WitnessSigner>,
        objects: std::sync::Arc<dyn ObjectStore>,
        feed: std::sync::Arc<dyn FeedStore>,
    ) -> Self {
        Self {
            signer,
            objects,
            feed,
            pending: Mutex::new(Vec::new()),
        }
    }

    /// The published witness key (multibase).
    #[must_use]
    pub fn witness_key(&self) -> String {
        self.signer.witness_key()
    }

    /// One heads event (C.2 trigger ①): emits iff the edition advanced
    /// AND the layout corroborates. A gamma-only write (height unchanged)
    /// is not an edition and never triggers.
    pub async fn on_event(
        &self,
        event: &HeadsEvent,
        now: &str,
    ) -> Result<Observed, FeedUnavailable> {
        if event.old_height == Some(event.height) {
            return Ok(Observed::NotAnEdition);
        }
        self.observe(
            &event.tenant,
            &event.did,
            event.height,
            &event.manifest_chain_hash,
            false,
            now,
        )
        .await
    }

    /// The daily heartbeat (C.2): re-sign the CURRENT head of every known
    /// `(tenant, did)` with a fresh `observed_at` — freshness, never a
    /// fault. The rows come from the heads table sweep (the same source
    /// the boot reconciliation uses).
    pub async fn heartbeat(
        &self,
        rows: &[(String, String, u64, String)],
        now: &str,
    ) -> Result<(), FeedUnavailable> {
        for (tenant, did, height, manifest_chain_hash) in rows {
            // Height 0 = no manifest yet: nothing to attest.
            if *height == 0 {
                continue;
            }
            self.observe(tenant, did, *height, manifest_chain_hash, true, now)
                .await?;
        }
        Ok(())
    }

    /// The boot reconciliation sweep: observe every known row with the
    /// C.2 dedup semantics — anything already in the feed for this UTC
    /// day emits nothing, anything missed while the service was down is
    /// emitted now. Same rows source as the heartbeat.
    pub async fn reconcile(
        &self,
        rows: &[(String, String, u64, String)],
        now: &str,
    ) -> Result<(), FeedUnavailable> {
        for (tenant, did, height, manifest_chain_hash) in rows {
            if *height == 0 {
                continue;
            }
            self.observe(tenant, did, *height, manifest_chain_hash, false, now)
                .await?;
        }
        Ok(())
    }

    /// Retry every pending observation (the layout may have healed).
    pub async fn sweep_pending(&self, now: &str) -> Result<(), FeedUnavailable> {
        let retries: Vec<PendingObservation> = {
            let mut pending = self.pending.lock().expect("pending poisoned");
            std::mem::take(&mut *pending)
        };
        for p in retries {
            self.observe(
                &p.tenant,
                &p.did,
                p.height,
                &p.manifest_chain_hash,
                false,
                now,
            )
            .await?;
        }
        Ok(())
    }

    /// How many observations wait for the layout to heal.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().expect("pending poisoned").len()
    }

    /// The single observation path (events, heartbeat, sweep alike):
    /// fetch the manifest the layout stores, recompute its chain hash,
    /// sign ONLY what coheres with the announced head.
    async fn observe(
        &self,
        tenant: &str,
        did: &str,
        height: u64,
        manifest_chain_hash: &str,
        heartbeat: bool,
        now: &str,
    ) -> Result<Observed, FeedUnavailable> {
        // 1. The observed manifest — fetched, parsed, re-hashed. Any
        //    incoherence leaves the observation pending (never a
        //    checkpoint invented, never a repair).
        let fetched = self
            .objects
            .get(tenant, did, "manifest.json")
            .await
            .ok()
            .flatten();
        let manifest = fetched
            .as_deref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|text| serde_json::from_str::<aithos_bundle::manifest::Manifest>(text).ok());
        let coherent = manifest.as_ref().is_some_and(|m| {
            m.edition.height == height && m.chain_hash().is_ok_and(|h| h == manifest_chain_hash)
        });
        let Some(manifest) = manifest.filter(|_| coherent) else {
            if !heartbeat {
                let mut pending = self.pending.lock().expect("pending poisoned");
                let p = PendingObservation {
                    tenant: tenant.to_owned(),
                    did: did.to_owned(),
                    height,
                    manifest_chain_hash: manifest_chain_hash.to_owned(),
                };
                if !pending.contains(&p) {
                    pending.push(p);
                }
            }
            return Ok(Observed::Pending);
        };

        // 2. The checkpoint fields per annexe C.1: `did` is the row's key
        //    (the store's serialization identity), `manifest_hash` is the
        //    recomputed chain hash, `gamma_head` is COPIED from the
        //    observed manifest.
        let manifest_hash = format!("sha256:{manifest_chain_hash}");
        let checkpoint = build_checkpoint(
            self.signer.as_ref(),
            did,
            height,
            &manifest_hash,
            &manifest.gamma_head,
            now,
        );

        // 3. Idempotence C.2, derived from the feed itself: the same
        //    triple already observed this UTC day emits nothing outside
        //    the heartbeat.
        let key = format!("{did}.jsonl");
        let stored = self.feed.get(&key).await?;
        if !heartbeat {
            let day = utc_day(now);
            let already = stored
                .as_ref()
                .map(|(bytes, _)| {
                    parse_lines(bytes).any(|ck| {
                        ck.edition_height == height
                            && ck.manifest_hash == manifest_hash
                            && utc_day(&ck.observed_at) == day
                    })
                })
                .unwrap_or(false);
            if already {
                return Ok(Observed::AlreadyObserved);
            }
        }

        // 4. Append the EXACT signed JCS bytes as one line (C.3).
        let line = feed_line(&checkpoint);
        loop {
            let stored = self.feed.get(&key).await?;
            let (mut bytes, tag) = stored.map_or((Vec::new(), None), |(b, t)| (b, Some(t)));
            bytes.extend_from_slice(line.as_bytes());
            bytes.push(b'\n');
            match self.feed.put_if(&key, bytes, tag.as_deref()).await? {
                Ok(()) => return Ok(Observed::Emitted),
                Err(FeedConflict) => continue,
            }
        }
    }

    /// Publish the daily root of `date` (C.3): the mroot over ALL lines
    /// of that UTC day across every DID feed, sorted by JCS byte order,
    /// deduplicated. Idempotent: an already-published root is left as is
    /// (date-addressed, never rewritten).
    pub async fn publish_daily_root(&self, date: &str) -> Result<(), FeedUnavailable> {
        let root_key = format!("roots/{date}.json");
        if self.feed.get(&root_key).await?.is_some() {
            return Ok(());
        }
        let lines = self.day_lines(date).await?;
        if lines.is_empty() {
            return Ok(());
        }
        let root = build_daily_root(self.signer.as_ref(), date, &lines);
        let bytes = serde_jcs::to_string(&root)
            .expect("root serializable")
            .into_bytes();
        match self.feed.put_if(&root_key, bytes, None).await? {
            Ok(()) | Err(FeedConflict) => Ok(()), // a concurrent identical publisher lost benignly
        }
    }

    /// The root sweep (verdict témoin D1, gate P5) : publish the daily
    /// root of EVERY finished UTC day that has feed lines and no root
    /// yet. Idempotent and feed-derived — a rollover missed across a
    /// restart, or a feed error at the rollover tick, heals at the next
    /// sweep ; a published root is never recomputed (date-addressed,
    /// C.3). `today` bounds the sweep : the running day is never sealed.
    pub async fn publish_missing_roots(&self, today: &str) -> Result<(), FeedUnavailable> {
        let mut days: Vec<String> = Vec::new();
        for key in self.feed.list("").await? {
            if !key.ends_with(".jsonl") {
                continue;
            }
            if let Some((bytes, _)) = self.feed.get(&key).await? {
                for ck in parse_lines(&bytes) {
                    let day = utc_day(&ck.observed_at);
                    if !day.is_empty() && day < today && !days.iter().any(|d| d == day) {
                        days.push(day.to_owned());
                    }
                }
            }
        }
        for day in days {
            self.publish_daily_root(&day).await?;
        }
        Ok(())
    }

    /// Every feed line of the UTC day `date`, across all DIDs.
    pub async fn day_lines(&self, date: &str) -> Result<Vec<String>, FeedUnavailable> {
        let mut lines = Vec::new();
        for key in self.feed.list("").await? {
            if !key.ends_with(".jsonl") {
                continue;
            }
            if let Some((bytes, _)) = self.feed.get(&key).await? {
                for line in String::from_utf8_lossy(&bytes).lines() {
                    if let Ok(ck) = serde_json::from_str::<Checkpoint>(line) {
                        if utc_day(&ck.observed_at) == date {
                            lines.push(line.to_owned());
                        }
                    }
                }
            }
        }
        Ok(lines)
    }

    /// Publish `keys.json` — the registry of accepted witness keys
    /// (annexe C.1: « le registre publié des clés témoin, signé par la
    /// clé sortante »). Written unconditionally at boot: the content is a
    /// pure function of the key set.
    pub async fn publish_keys(&self) -> Result<(), FeedUnavailable> {
        let doc =
            crate::witness::build_keys_doc(self.signer.as_ref(), &[self.signer.witness_key()]);
        let bytes = serde_jcs::to_string(&doc)
            .expect("keys doc serializable")
            .into_bytes();
        loop {
            let stored = self.feed.get("keys.json").await?;
            let tag = stored.map(|(_, t)| t);
            match self
                .feed
                .put_if("keys.json", bytes.clone(), tag.as_deref())
                .await?
            {
                Ok(()) => return Ok(()),
                Err(FeedConflict) => continue,
            }
        }
    }
}

/// The deployed feed backend (arbitrage ②): the S3 feed bucket fronted
/// by CloudFront on `witness.aithos.fr` (module Terraform `witness`).
/// Conditional writes ride S3's `If-Match`/`If-None-Match` (the same
/// primitive as the store's write-once ⑧b); cache classes per C.3 —
/// feeds `max-age=60`, date-addressed roots immutable, keys.json short.
pub struct S3Feed {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3Feed {
    pub fn new(client: aws_sdk_s3::Client, bucket: String) -> Self {
        Self { client, bucket }
    }

    fn cache_control(key: &str) -> &'static str {
        if key.starts_with("roots/") {
            // A finished day's root is date-addressed and never rewritten.
            "public, max-age=31536000, immutable"
        } else {
            // Feeds advance (C.3: max-age=60); keys.json follows suit.
            "public, max-age=60"
        }
    }

    fn content_type(key: &str) -> &'static str {
        if key.ends_with(".jsonl") {
            "application/x-ndjson"
        } else {
            "application/json"
        }
    }
}

impl FeedStore for S3Feed {
    fn get<'a>(&'a self, key: &'a str) -> FeedFuture<'a, Option<(Vec<u8>, String)>> {
        Box::pin(async move {
            use aws_sdk_s3::error::SdkError;
            use aws_sdk_s3::operation::get_object::GetObjectError;
            match self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
            {
                Ok(out) => {
                    let tag = out.e_tag().unwrap_or_default().to_owned();
                    let bytes = out
                        .body
                        .collect()
                        .await
                        .map_err(|_| FeedUnavailable)?
                        .into_bytes()
                        .to_vec();
                    Ok(Some((bytes, tag)))
                }
                Err(SdkError::ServiceError(e))
                    if matches!(e.err(), GetObjectError::NoSuchKey(_)) =>
                {
                    Ok(None)
                }
                Err(_) => Err(FeedUnavailable),
            }
        })
    }

    fn put_if<'a>(
        &'a self,
        key: &'a str,
        bytes: Vec<u8>,
        expected: Option<&'a str>,
    ) -> FeedFuture<'a, Result<(), FeedConflict>> {
        Box::pin(async move {
            let mut put = self
                .client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .cache_control(Self::cache_control(key))
                .content_type(Self::content_type(key))
                .body(aws_sdk_s3::primitives::ByteStream::from(bytes));
            put = match expected {
                None => put.if_none_match("*"),
                Some(tag) => put.if_match(tag),
            };
            match put.send().await {
                Ok(_) => Ok(Ok(())),
                Err(e) => {
                    // 412 Precondition Failed / 409 conditional conflict =
                    // the generation moved — a retryable race, never an
                    // outage.
                    let status = e.raw_response().map(|r| r.status().as_u16());
                    if matches!(status, Some(412) | Some(409)) {
                        Ok(Err(FeedConflict))
                    } else {
                        Err(FeedUnavailable)
                    }
                }
            }
        })
    }

    fn list<'a>(&'a self, prefix: &'a str) -> FeedFuture<'a, Vec<String>> {
        Box::pin(async move {
            let mut keys = Vec::new();
            let mut token: Option<String> = None;
            loop {
                let mut req = self
                    .client
                    .list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(prefix);
                if let Some(t) = &token {
                    req = req.continuation_token(t);
                }
                let out = req.send().await.map_err(|_| FeedUnavailable)?;
                keys.extend(
                    out.contents()
                        .iter()
                        .filter_map(|o| o.key().map(str::to_owned)),
                );
                match out.next_continuation_token() {
                    Some(t) => token = Some(t.to_owned()),
                    None => break,
                }
            }
            keys.sort();
            Ok(keys)
        })
    }
}

/// The `YYYY-MM-DD` prefix of an RFC 3339 Z instant.
fn utc_day(at: &str) -> &str {
    at.get(..10).unwrap_or("")
}

fn parse_lines(bytes: &[u8]) -> impl Iterator<Item = Checkpoint> + '_ {
    std::str::from_utf8(bytes)
        .into_iter()
        .flat_map(|text| text.lines())
        .filter_map(|line| serde_json::from_str::<Checkpoint>(line).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_day_is_the_date_prefix() {
        assert_eq!(utc_day("2026-07-16T11:05:00Z"), "2026-07-16");
        assert_eq!(utc_day("x"), "");
    }
}
