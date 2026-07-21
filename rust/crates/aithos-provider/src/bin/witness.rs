//! aithos-witness: the notary emitter (INFRA-PROVIDER annexe C, lot A/P5).
//!
//! The binary supplies what the library never touches: the AWS session,
//! the KMS signer, the heads stream, the wall clock. Configuration is
//! environment-only (twelve-factor, matches the Terraform task
//! definition) and **fail-closed at startup**: a missing or inconsistent
//! variable refuses to boot rather than booting permissive.
//!
//! | Variable | Rôle |
//! |---|---|
//! | `AITHOS_WITNESS_SIGNER`        | `kms` (annexe C.1 — the key is born in KMS and never leaves) or `file` (dev/tests only, warns) |
//! | `AITHOS_WITNESS_KMS_KEY_ID`    | REQUIRED when the signer is kms |
//! | `AITHOS_WITNESS_SEED_HEX`      | REQUIRED when the signer is file (32-byte hex seed — never a deployment) |
//! | `AITHOS_WITNESS_FEED_BACKEND`  | `s3` (the public feed bucket, module witness) or `memory` (dev only, warns — nothing is published) |
//! | `AITHOS_WITNESS_FEED_BUCKET`   | REQUIRED when the feed backend is s3 |
//! | `AITHOS_WITNESS_STORE_BACKEND` | `s3` (the store-data layout, read-only) or `memory` (dev only) |
//! | `AITHOS_WITNESS_STORE_BUCKET`  | REQUIRED when the store backend is s3 |
//! | `AITHOS_WITNESS_HEADS_TABLE`   | REQUIRED — the A.5 heads table (boot reconcile + heartbeat sweep) |
//! | `AITHOS_WITNESS_STREAM_ARN`    | optional — the heads table stream (C.2 trigger ①); absent = sweep-only mode (warns) |
//! | `AITHOS_WITNESS_TICK_SECS`     | pending/rollover tick period (default 60) |
//!
//! The KMS signer uses `ED25519_SHA_512` with `MessageType: RAW` — never
//! the prehashed `_PH_` mode (annexe C.1, documented trap). The witness
//! signs observations, never authority: it holds no client key, reads
//! only the heads table, the store layout and its own feed bucket.

use std::sync::Arc;

use aithos_provider::objects::{MemObjects, ObjectStore, S3Objects};
use aithos_provider::witness::WitnessSigner;
use aithos_provider::witness_service::{FeedStore, HeadsEvent, MemFeed, S3Feed, WitnessService};

fn required(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("fatal: {name} is required (fail-closed startup)");
            std::process::exit(2);
        }
    }
}

fn now_rfc3339z() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    aithos_provider::time::render_rfc3339z(ms)
}

/// The KMS witness signer — annexe C.1: key spec `ECC_NIST_EDWARDS25519`,
/// algorithm `ED25519_SHA_512`, `MessageType: RAW` (the prehashed `_PH_`
/// mode is NOT interoperable with pure Ed25519 verification and is never
/// used). The seam is synchronous; KMS is async — a dedicated signing
/// thread with its own runtime bridges the two (signing volume is one
/// call per checkpoint, never a hot path).
struct KmsWitnessSigner {
    key_multibase: String,
    tx: std::sync::mpsc::Sender<(Vec<u8>, std::sync::mpsc::Sender<Option<String>>)>,
}

impl KmsWitnessSigner {
    fn spawn(key_id: String) -> Self {
        let (tx, rx) =
            std::sync::mpsc::channel::<(Vec<u8>, std::sync::mpsc::Sender<Option<String>>)>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("signer runtime");
            let client = rt.block_on(async {
                let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                aws_sdk_kms::Client::new(&config)
            });
            // GetPublicKey → DER SPKI; a pure Ed25519 SPKI is 44 bytes,
            // the raw 32-byte key after the fixed 12-byte prefix.
            const SPKI_ED25519_PREFIX: [u8; 12] = [
                0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
            ];
            let public = rt.block_on(async {
                client
                    .get_public_key()
                    .key_id(&key_id)
                    .send()
                    .await
                    .map_err(|e| format!("kms GetPublicKey failed: {e}"))
            });
            let multibase = public.and_then(|out| {
                let der = out
                    .public_key()
                    .map(|b| b.as_ref().to_vec())
                    .unwrap_or_default();
                if der.len() == 44 && der[..12] == SPKI_ED25519_PREFIX {
                    let raw: [u8; 32] = der[12..].try_into().expect("32 bytes");
                    Ok(aithos_core::wire::ed25519_pub_to_multibase(&raw))
                } else {
                    Err("kms public key is not a pure Ed25519 SPKI (wrong key spec?)".into())
                }
            });
            match multibase {
                Err(e) => {
                    let _ = init_tx.send(Err(e));
                }
                Ok(mb) => {
                    let _ = init_tx.send(Ok(mb));
                    while let Ok((bytes, reply)) = rx.recv() {
                        let signed = rt.block_on(async {
                            client
                                .sign()
                                .key_id(&key_id)
                                .message(aws_sdk_kms::primitives::Blob::new(bytes))
                                .message_type(aws_sdk_kms::types::MessageType::Raw)
                                .signing_algorithm(aws_sdk_kms::types::SigningAlgorithmSpec::from(
                                    "ED25519_SHA_512",
                                ))
                                .send()
                                .await
                                .ok()
                                .and_then(|out| out.signature().map(|s| hex::encode(s.as_ref())))
                        });
                        let _ = reply.send(signed);
                    }
                }
            }
        });
        match init_rx.recv() {
            Ok(Ok(key_multibase)) => {
                eprintln!("witness key (kms): {key_multibase}");
                Self { key_multibase, tx }
            }
            Ok(Err(e)) => {
                eprintln!("fatal: {e} (fail-closed startup)");
                std::process::exit(2);
            }
            Err(_) => {
                eprintln!("fatal: kms signer thread died (fail-closed startup)");
                std::process::exit(2);
            }
        }
    }
}

impl WitnessSigner for KmsWitnessSigner {
    fn witness_key(&self) -> String {
        self.key_multibase.clone()
    }

    fn sign(&self, unsigned_jcs: &[u8]) -> String {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.tx
            .send((unsigned_jcs.to_vec(), reply_tx))
            .expect("signer thread alive");
        // A signature that cannot be produced must never yield a bogus
        // checkpoint: crash the emitter (the service restarts, the
        // observation replays from the reconcile sweep).
        reply_rx
            .recv()
            .expect("signer thread alive")
            .expect("kms signature produced")
    }
}

/// Scan the heads table into heartbeat/reconcile rows.
async fn scan_heads(
    client: &aws_sdk_dynamodb::Client,
    table: &str,
) -> Result<Vec<(String, String, u64, String)>, String> {
    let mut rows = Vec::new();
    let mut start_key = None;
    loop {
        let mut req = client.scan().table_name(table).consistent_read(true);
        if let Some(k) = &start_key {
            req = req.set_exclusive_start_key(Some(std::collections::HashMap::clone(k)));
        }
        let out = req
            .send()
            .await
            .map_err(|e| format!("heads scan failed: {e}"))?;
        for item in out.items() {
            let s = |name: &str| {
                item.get(name)
                    .and_then(|v| v.as_s().ok())
                    .map(std::string::ToString::to_string)
            };
            let h = item
                .get("h")
                .and_then(|v| v.as_n().ok())
                .and_then(|v| v.parse::<u64>().ok());
            if let (Some(t), Some(d), Some(h), Some(m)) = (s("t"), s("d"), h, s("m")) {
                rows.push((t, d, h, m));
            }
        }
        match out.last_evaluated_key() {
            Some(k) if !k.is_empty() => start_key = Some(k.clone()),
            _ => break,
        }
    }
    Ok(rows)
}

/// One stream record → the observation event (NEW_AND_OLD_IMAGES).
fn event_of(record: &aws_sdk_dynamodbstreams::types::Record) -> Option<HeadsEvent> {
    let dyn_rec = record.dynamodb()?;
    let new = dyn_rec.new_image()?;
    let s = |img: &std::collections::HashMap<
        String,
        aws_sdk_dynamodbstreams::types::AttributeValue,
    >,
             name: &str| {
        img.get(name)
            .and_then(|v| v.as_s().ok())
            .map(std::string::ToString::to_string)
    };
    let n = |img: &std::collections::HashMap<
        String,
        aws_sdk_dynamodbstreams::types::AttributeValue,
    >,
             name: &str| {
        img.get(name)
            .and_then(|v| v.as_n().ok())
            .and_then(|v| v.parse::<u64>().ok())
    };
    Some(HeadsEvent {
        tenant: s(new, "t")?,
        did: s(new, "d")?,
        old_height: dyn_rec.old_image().and_then(|old| n(old, "h")),
        height: n(new, "h")?,
        manifest_chain_hash: s(new, "m")?,
    })
}

/// Poll the heads stream forever (trigger ① — C.2): LATEST iterators,
/// the boot reconcile sweep covers anything before them. Errors re-list
/// the shards; the sweep makes losses benign (idempotence C.2).
async fn poll_stream(
    client: aws_sdk_dynamodbstreams::Client,
    stream_arn: String,
    service: Arc<WitnessService>,
    tick_secs: u64,
) {
    loop {
        let described = client
            .describe_stream()
            .stream_arn(&stream_arn)
            .send()
            .await;
        let shards: Vec<String> = match &described {
            Ok(out) => out
                .stream_description()
                .map(|d| {
                    d.shards()
                        .iter()
                        .filter_map(|s| s.shard_id().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            Err(e) => {
                tracing::warn!("witness stream describe failed: {e}");
                Vec::new()
            }
        };
        let mut iterators = Vec::new();
        for shard in shards {
            if let Ok(out) = client
                .get_shard_iterator()
                .stream_arn(&stream_arn)
                .shard_id(&shard)
                .shard_iterator_type(aws_sdk_dynamodbstreams::types::ShardIteratorType::Latest)
                .send()
                .await
            {
                if let Some(it) = out.shard_iterator() {
                    iterators.push(it.to_owned());
                }
            }
        }
        if iterators.is_empty() {
            tokio::time::sleep(std::time::Duration::from_secs(tick_secs)).await;
            continue;
        }
        // Poll the open shards until one closes or errors; then re-list.
        'poll: loop {
            let mut next = Vec::new();
            for it in &iterators {
                match client.get_records().shard_iterator(it).send().await {
                    Ok(out) => {
                        for record in out.records() {
                            if let Some(event) = event_of(record) {
                                let now = now_rfc3339z();
                                if let Err(e) = service.on_event(&event, &now).await {
                                    tracing::warn!("witness emit failed (feed): {e:?}");
                                }
                            }
                        }
                        match out.next_shard_iterator() {
                            Some(n) => next.push(n.to_owned()),
                            None => break 'poll, // shard closed — re-describe
                        }
                    }
                    Err(e) => {
                        tracing::warn!("witness stream read failed: {e}");
                        break 'poll;
                    }
                }
            }
            iterators = next;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    // --- signer (annexe C.1) ---
    let signer_backend = std::env::var("AITHOS_WITNESS_SIGNER").unwrap_or_else(|_| "kms".into());
    let signer: Box<dyn WitnessSigner> = match signer_backend.as_str() {
        "kms" => Box::new(KmsWitnessSigner::spawn(required(
            "AITHOS_WITNESS_KMS_KEY_ID",
        ))),
        "file" => {
            tracing::warn!("signer = file: dev/tests only, NEVER a deployment (annexe C.1 = KMS)");
            let seed_hex = required("AITHOS_WITNESS_SEED_HEX");
            let seed: [u8; 32] = match hex::decode(seed_hex.trim())
                .ok()
                .and_then(|b| b.try_into().ok())
            {
                Some(seed) => seed,
                None => {
                    eprintln!("fatal: AITHOS_WITNESS_SEED_HEX is not a 32-byte hex seed (fail-closed startup)");
                    std::process::exit(2);
                }
            };
            Box::new(aithos_provider::witness::LocalWitnessSigner::new(
                ed25519_dalek::SigningKey::from_bytes(&seed),
            ))
        }
        other => {
            eprintln!("fatal: unknown signer backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    // --- feed (C.3 publication) ---
    let feed_backend = std::env::var("AITHOS_WITNESS_FEED_BACKEND").unwrap_or_else(|_| "s3".into());
    let feed: Arc<dyn FeedStore> = match feed_backend.as_str() {
        "s3" => {
            let bucket = required("AITHOS_WITNESS_FEED_BUCKET");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(S3Feed::new(aws_sdk_s3::Client::new(&config), bucket))
        }
        "memory" => {
            tracing::warn!("feed backend = memory: nothing is published (dev/tests only)");
            Arc::new(MemFeed::new())
        }
        other => {
            eprintln!("fatal: unknown feed backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    // --- store layout (read-only observation source) ---
    let store_backend =
        std::env::var("AITHOS_WITNESS_STORE_BACKEND").unwrap_or_else(|_| "s3".into());
    let objects: Arc<dyn ObjectStore> = match store_backend.as_str() {
        "s3" => {
            let bucket = required("AITHOS_WITNESS_STORE_BUCKET");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(S3Objects::new(aws_sdk_s3::Client::new(&config), bucket))
        }
        "memory" => {
            tracing::warn!("store backend = memory: an empty layout (dev/tests only)");
            Arc::new(MemObjects::new())
        }
        other => {
            eprintln!("fatal: unknown store backend `{other}` (fail-closed startup)");
            std::process::exit(2);
        }
    };

    // --- heads table (reconcile + heartbeat) ---
    let heads_table = required("AITHOS_WITNESS_HEADS_TABLE");
    let ddb = {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        aws_sdk_dynamodb::Client::new(&config)
    };

    let tick_secs = std::env::var("AITHOS_WITNESS_TICK_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(1);

    let service = Arc::new(WitnessService::new(signer, objects, feed));

    // Boot gestures: publish the key registry, then reconcile everything
    // the heads table knows (idempotence C.2 dedups what the feed holds).
    if let Err(e) = service.publish_keys().await {
        eprintln!("fatal: cannot publish keys.json: {e:?}");
        std::process::exit(2);
    }
    match scan_heads(&ddb, &heads_table).await {
        Ok(rows) => {
            let now = now_rfc3339z();
            tracing::info!("boot reconcile over {} heads row(s)", rows.len());
            if let Err(e) = service.reconcile(&rows, &now).await {
                tracing::warn!("boot reconcile: feed unavailable ({e:?}); the tick retries");
            }
            // Verdict témoin D1 : seal any day a restart left unrooted.
            if let Err(e) = service.publish_missing_roots(&now[..10]).await {
                tracing::warn!("boot root sweep: feed unavailable ({e:?}); the tick retries");
            }
        }
        Err(e) => {
            // Fail-closed but alive: the witness degrades to freshness
            // loss, never to invented state. The tick retries.
            tracing::warn!("boot reconcile skipped: {e}");
        }
    }

    // The C.2 trigger: the heads stream (arbitrage ①). Absent = sweep-only.
    match std::env::var("AITHOS_WITNESS_STREAM_ARN")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(stream_arn) => {
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let streams = aws_sdk_dynamodbstreams::Client::new(&config);
            let svc = service.clone();
            tokio::spawn(poll_stream(streams, stream_arn, svc, tick_secs));
        }
        None => {
            tracing::warn!(
                "AITHOS_WITNESS_STREAM_ARN absent: sweep-only mode (observation latency = tick)"
            );
        }
    }

    eprintln!(
        "aithos-witness {} observing heads table {heads_table}, witness key {}",
        aithos_provider::witness::WITNESS_WIRE_VERSION,
        service.witness_key()
    );

    // The tick: pending sweep + day rollover (heartbeat + daily root) +
    // periodic reconcile in sweep-only mode.
    let mut last_day = now_rfc3339z()[..10].to_owned();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(tick_secs));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let now = now_rfc3339z();
        if let Err(e) = service.sweep_pending(&now).await {
            tracing::warn!("pending sweep: feed unavailable ({e:?})");
        }
        // Reconcile every tick: in stream mode this is the safety net
        // (C.2 idempotence makes it free); in sweep-only mode it IS the
        // trigger.
        match scan_heads(&ddb, &heads_table).await {
            Ok(rows) => {
                if let Err(e) = service.reconcile(&rows, &now).await {
                    tracing::warn!("reconcile: feed unavailable ({e:?})");
                }
                let day = now[..10].to_owned();
                if day != last_day {
                    // Day rollover: heartbeat every DID with a fresh
                    // observed_at (C.2).
                    if let Err(e) = service.heartbeat(&rows, &now).await {
                        tracing::warn!("heartbeat: feed unavailable ({e:?})");
                    }
                    last_day = day.clone();
                }
                // Verdict témoin D1 : the root sweep runs EVERY tick —
                // idempotent, feed-derived ; a rollover missed across a
                // restart or a feed error is healed at the next pass,
                // never lost (C.3 : la racine couvre TOUTES les lignes
                // émises du jour).
                if let Err(e) = service.publish_missing_roots(&day).await {
                    tracing::warn!("root sweep: feed unavailable ({e:?})");
                }
            }
            Err(e) => tracing::warn!("heads scan failed: {e}"),
        }
    }
}
