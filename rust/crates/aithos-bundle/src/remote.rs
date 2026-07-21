//! P3 — the `RemoteStore` client (INFRA-PROVIDER annexe A, consumed from
//! the CLIENT side; HANDOFF-PROVIDER-AWS P3).
//!
//! A sync [`Store`] backend that speaks the wire `aithos-store
//! 1.0.0-draft.1` against `https://store.aithos.fr` (or any authority):
//!
//! - every non-anonymous request carries the signed `X-Aithos-Auth`
//!   envelope (A.2): JCS, `body_b3` = BLAKE3 of the exact bytes sent,
//!   fresh injected nonce, injected `at` — *what is signed IS what is
//!   sent* (the pod_stub motif, arbitrage ① 2026-07-21: ureq + rustls,
//!   no async runtime in this lib);
//! - `put("manifest.json")` is a **publish** under the A.5 CAS
//!   (`If-Head` from the tracked head, genesis = `none`); a `409`
//!   surfaces the served head so the caller rebases (§02.6 stays
//!   client-side, above this seam);
//! - `put("gamma/<YYYY-MM>.jsonl")` appending exactly ONE entry rides
//!   `POST /gamma` — the mode-B hot path; anything else is the segment
//!   replica PUT (mode A), same CAS discipline;
//! - transport faults retry with bounded backoff (injected sleeper);
//!   a wire VERDICT (4xx) is never retried — fail-closed, never
//!   fail-open;
//! - the A.6 cache classes are honoured from the wire's own headers:
//!   `immutable` responses are cached forever, `must-revalidate` +
//!   strong ETag responses revalidate with `If-None-Match` (304 serves
//!   the cache), `no-store` responses are never kept.
//!
//! The signer is INJECTED (arbitrage ② 2026-07-21): a seam, never a key
//! baked into the lib — the owner `#content`, the gateway key, or a
//! mandated leaf, per the caller's mode. Clock and nonce entropy are
//! injected too (the §00 purity rule extends here).

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;

use crate::entropy::EntropySource;
use crate::Store;

/// Wire version this client speaks (A.1).
pub const WIRE_VERSION: &str = "1.0.0-draft.1";

/// The injected signing seam (arbitrage ②): who signs the envelopes.
/// `key()` is the A.2 `key` field (`"#root"`, `"#content"`, or the
/// multibase pubkey of a mandated leaf); `mandate()` the chain ids,
/// root first (empty for the owner).
pub trait EnvelopeSigner: Send + Sync {
    fn key(&self) -> String;
    fn mandate(&self) -> Vec<String>;
    fn sign(&self, message: &[u8]) -> [u8; 64];
}

/// Owner/leaf signer over an Ed25519 signing key — the common impl.
pub struct KeySigner {
    key: String,
    mandate: Vec<String>,
    sk: ed25519_dalek::SigningKey,
}

impl KeySigner {
    /// An owner signer (`#root` / `#content`, mandate `[]`).
    pub fn owner(fragment: &str, sk: ed25519_dalek::SigningKey) -> Self {
        Self {
            key: fragment.to_owned(),
            mandate: vec![],
            sk,
        }
    }

    /// A mandated leaf signer (multibase key + chain root→leaf).
    pub fn mandated(sk: ed25519_dalek::SigningKey, chain: Vec<String>) -> Self {
        let key = aithos_core::wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes());
        Self {
            key,
            mandate: chain,
            sk,
        }
    }
}

impl EnvelopeSigner for KeySigner {
    fn key(&self) -> String {
        self.key.clone()
    }
    fn mandate(&self) -> Vec<String> {
        self.mandate.clone()
    }
    fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer as _;
        self.sk.sign(message).to_bytes()
    }
}

/// A typed wire error — carried inside `io::Error` through the sync
/// [`Store`] trait, and returned directly by the typed API.
#[derive(Debug, Clone)]
pub enum RemoteError {
    /// Connection/transport fault after the bounded retries.
    Transport(String),
    /// A wire verdict (A.7): never retried, never silent.
    Wire {
        status: u16,
        code: String,
        /// The closed `reason` of an `artifact_invalid` (A.7 registry).
        reason: Option<String>,
        /// The served head on `cas_mismatch` — the rebase input.
        head: Option<String>,
        height: Option<u64>,
    },
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteError::Transport(e) => write!(f, "remote store transport: {e}"),
            RemoteError::Wire {
                status,
                code,
                reason,
                head,
                ..
            } => {
                write!(f, "remote store {status} {code}")?;
                if let Some(r) = reason {
                    write!(f, " ({r})")?;
                }
                if let Some(h) = head {
                    write!(f, " (head {h})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RemoteError {}

impl From<RemoteError> for io::Error {
    fn from(e: RemoteError) -> Self {
        io::Error::other(e)
    }
}

/// A raw wire answer: status, lowercased headers, body.
type WireAnswer = (u16, HashMap<String, String>, Vec<u8>);

/// Accepted publish/append acknowledgement (A.5 redline gate 4).
#[derive(Debug, Clone)]
pub struct Ack {
    pub head: String,
    pub height: Option<u64>,
}

/// The two hot heads as `/heads` serves them.
#[derive(Debug, Clone)]
pub struct Heads {
    pub height: u64,
    pub manifest: Option<String>,
    pub gamma: Option<String>,
    pub segment: Option<String>,
}

/// What the cache remembers for one path (A.6, honoured FROM the wire's
/// own `Cache-Control`/`ETag` — the client never re-invents the classes).
enum Cached {
    /// `immutable` — served locally forever, never revalidated.
    Immutable(Vec<u8>),
    /// `must-revalidate` + strong ETag — revalidated via `If-None-Match`.
    Revalidate { etag: String, bytes: Vec<u8> },
}

struct Tracked {
    manifest_head: Option<String>,
    gamma_head: Option<String>,
    /// Last segment bytes this client OBSERVED per gamma segment path —
    /// the diff base that routes an append onto `POST /gamma`.
    segments: HashMap<String, Vec<u8>>,
}

/// One request that reached a wire VERDICT (acceptance-harness tap).
/// One part of a `/batch` or `/sync` multipart pack (A.3): the
/// RELATIVE chemin (the `Content-Location` with the client's own
/// `/t/<tenant>/<did>/` prefix stripped), its per-part verdict
/// (`X-Aithos-Status`: 200 | 403 | 404) and the bytes on 200 only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPart {
    pub path: String,
    pub status: u16,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct SentRequest {
    pub method: String,
    pub path: String,
    pub if_head: Option<String>,
    pub if_none_match: Option<String>,
    pub status: u16,
}

/// Debug tap for the acceptance harness: the last envelope actually
/// sent (the same JCS bytes the header carries) and per-path wire hits.
#[derive(Default)]
struct Taps {
    last_envelope_jcs: Option<String>,
    envelopes: Vec<String>,
    waited: Vec<Duration>,
    requests: Vec<SentRequest>,
}

/// The remote `Store` (P3). One instance = one `(url, tenant, did)`.
pub struct RemoteStore {
    base: String,
    host: String,
    tenant: String,
    did: String,
    signer: Arc<dyn EnvelopeSigner>,
    now: Arc<dyn Fn() -> String + Send + Sync>,
    entropy: Mutex<Box<dyn EntropySource + Send>>,
    sleep: Arc<dyn Fn(Duration) + Send + Sync>,
    agent: ureq::Agent,
    max_retries: u32,
    tracked: Mutex<Tracked>,
    cache: Mutex<HashMap<String, Cached>>,
    taps: Mutex<Taps>,
}

impl RemoteStore {
    /// Build a client. `url` is the authority base (`https://…`, no
    /// trailing slash); the envelope `host` is derived from it (A.2:
    /// lowercase, port kept when explicit).
    pub fn new(
        url: &str,
        tenant: &str,
        did: &str,
        signer: Arc<dyn EnvelopeSigner>,
        now: Arc<dyn Fn() -> String + Send + Sync>,
        entropy: Box<dyn EntropySource + Send>,
    ) -> io::Result<Self> {
        let base = url.trim_end_matches('/').to_owned();
        let host = base
            .strip_prefix("https://")
            .or_else(|| base.strip_prefix("http://"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "remote store url must be http(s)://…",
                )
            })?
            .to_ascii_lowercase();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        Ok(Self {
            base,
            host,
            tenant: tenant.to_owned(),
            did: did.to_owned(),
            signer,
            now,
            entropy: Mutex::new(entropy),
            sleep: Arc::new(std::thread::sleep),
            agent,
            max_retries: 3,
            tracked: Mutex::new(Tracked {
                manifest_head: None,
                gamma_head: None,
                segments: HashMap::new(),
            }),
            cache: Mutex::new(HashMap::new()),
            taps: Mutex::new(Taps::default()),
        })
    }

    /// Inject the backoff sleeper (tests record instead of sleeping).
    pub fn with_sleeper(mut self, sleep: Arc<dyn Fn(Duration) + Send + Sync>) -> Self {
        self.sleep = sleep;
        self
    }

    /// Bound the transport retries (default 3 attempts after the first).
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    // ------------------------------------------------------------ taps

    /// The last envelope sent, parsed — acceptance-harness tap; the
    /// value IS the JCS the header carried (nothing re-serialized).
    pub fn last_envelope(&self) -> Option<serde_json::Value> {
        let taps = self.taps.lock().expect("taps");
        taps.last_envelope_jcs
            .as_ref()
            .and_then(|j| serde_json::from_str(j).ok())
    }

    /// Every envelope sent, in order (JCS strings).
    pub fn sent_envelopes(&self) -> Vec<serde_json::Value> {
        let taps = self.taps.lock().expect("taps");
        taps.envelopes
            .iter()
            .filter_map(|j| serde_json::from_str(j).ok())
            .collect()
    }

    /// The backoff waits the client performed (test sleeper records).
    pub fn backoff_waits(&self) -> Vec<Duration> {
        self.taps.lock().expect("taps").waited.clone()
    }

    /// Every request that reached a wire verdict, in order.
    pub fn sent_requests(&self) -> Vec<SentRequest> {
        self.taps.lock().expect("taps").requests.clone()
    }

    /// The manifest head this client tracks (adopted from accepts AND
    /// from `cas_mismatch` verdicts — the rebase input).
    pub fn tracked_manifest_head(&self) -> Option<String> {
        self.tracked.lock().expect("tracked").manifest_head.clone()
    }

    /// The gamma head this client tracks.
    pub fn tracked_gamma_head(&self) -> Option<String> {
        self.tracked.lock().expect("tracked").gamma_head.clone()
    }

    // ------------------------------------------------------ wire plumbing

    fn abs(&self, relative: &str) -> String {
        format!("/t/{}/{}/{}", self.tenant, self.did, relative)
    }

    fn fresh_nonce(&self) -> String {
        // 16 bytes of injected entropy, hex — 32 chars, ≥ 96 bits (A.2
        // client guidance; the server only enforces ≤ 64).
        let mut entropy = self.entropy.lock().expect("entropy");
        hex::encode(entropy.e16())
    }

    /// Build the signed `X-Aithos-Auth` header for `(method, path, body)`.
    /// The signature covers the JCS with `signature.value = ""` (A.1);
    /// the header carries the JCS with the value filled — byte for byte
    /// what was signed, plus the signature itself.
    fn auth_header(&self, method: &str, path: &str, body: &[u8]) -> io::Result<String> {
        let body_b3 = if body.is_empty() {
            String::new()
        } else {
            blake3::hash(body).to_hex().to_string()
        };
        let mut envelope = serde_json::json!({
            "v": 1,
            "host": self.host,
            "method": method,
            "path": path,
            "body_b3": body_b3,
            "at": (self.now)(),
            "nonce": self.fresh_nonce(),
            "mandate": self.signer.mandate(),
            "key": self.signer.key(),
            "signature": { "alg": "ed25519", "value": "" },
        });
        let unsigned = aithos_core::jcs::canonicalize(&envelope)
            .map_err(|e| io::Error::other(format!("envelope jcs: {e}")))?;
        let signature = self.signer.sign(unsigned.as_bytes());
        envelope["signature"]["value"] = serde_json::Value::String(hex::encode(signature));
        let signed = aithos_core::jcs::canonicalize(&envelope)
            .map_err(|e| io::Error::other(format!("envelope jcs: {e}")))?;
        {
            let mut taps = self.taps.lock().expect("taps");
            taps.last_envelope_jcs = Some(signed.clone());
            taps.envelopes.push(signed.clone());
        }
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signed.as_bytes()))
    }

    /// Fire one signed request with bounded transport retries + backoff.
    /// A wire VERDICT (any HTTP status) is returned as-is and NEVER
    /// retried on 4xx; connect/transport faults and 502/503/504 retry
    /// up to `max_retries` with exponential backoff.
    fn send(
        &self,
        method: &str,
        relative: &str,
        query: Option<&str>,
        body: &[u8],
        if_head: Option<&str>,
        if_none_match: Option<&str>,
    ) -> Result<WireAnswer, RemoteError> {
        let path = match query {
            Some(q) => format!("/t/{}/{}?{q}", self.tenant, self.did),
            None => self.abs(relative),
        };
        let url = format!("{}{}", self.base, path);
        let mut attempt = 0u32;
        loop {
            // A fresh envelope per attempt: a retry is a NEW request
            // (fresh nonce — the burnt nonce of a half-sent try must
            // never be replayed into `nonce_replayed`).
            let header = self
                .auth_header(method, &path, body)
                .map_err(|e| RemoteError::Transport(e.to_string()))?;
            let outcome = if method == "GET" || method == "DELETE" {
                let mut request = if method == "GET" {
                    self.agent.get(&url)
                } else {
                    self.agent.delete(&url)
                };
                request = request
                    .header("x-aithos-auth", &header)
                    .header("x-aithos-store", WIRE_VERSION);
                if let Some(h) = if_head {
                    request = request.header("if-head", h);
                }
                if let Some(e) = if_none_match {
                    request = request.header("if-none-match", e);
                }
                request.call()
            } else {
                let mut request = if method == "PUT" {
                    self.agent.put(&url)
                } else {
                    self.agent.post(&url)
                };
                request = request
                    .header("x-aithos-auth", &header)
                    .header("x-aithos-store", WIRE_VERSION);
                if let Some(h) = if_head {
                    request = request.header("if-head", h);
                }
                if let Some(e) = if_none_match {
                    request = request.header("if-none-match", e);
                }
                request.send(body)
            };
            match outcome {
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    let retryable = matches!(status, 502..=504);
                    if retryable && attempt < self.max_retries {
                        attempt += 1;
                        self.backoff(attempt);
                        continue;
                    }
                    let mut headers = HashMap::new();
                    for (name, value) in response.headers() {
                        if let Ok(v) = value.to_str() {
                            headers.insert(name.as_str().to_ascii_lowercase(), v.to_owned());
                        }
                    }
                    let bytes = response
                        .body_mut()
                        .with_config()
                        .limit(64 * 1024 * 1024)
                        .read_to_vec()
                        .map_err(|e| RemoteError::Transport(e.to_string()))?;
                    if status >= 400 && std::env::var("AITHOS_REMOTE_DEBUG").is_ok() {
                        eprintln!(
                            "remote-debug: {method} {path} -> {status} {}",
                            String::from_utf8_lossy(&bytes)
                        );
                    }
                    self.taps.lock().expect("taps").requests.push(SentRequest {
                        method: method.to_owned(),
                        path: path.clone(),
                        if_head: if_head.map(str::to_owned),
                        if_none_match: if_none_match.map(str::to_owned),
                        status,
                    });
                    return Ok((status, headers, bytes));
                }
                Err(e) => {
                    if attempt < self.max_retries {
                        attempt += 1;
                        self.backoff(attempt);
                        continue;
                    }
                    return Err(RemoteError::Transport(e.to_string()));
                }
            }
        }
    }

    fn backoff(&self, attempt: u32) {
        // 100 ms × 2^(attempt-1), capped at 2 s — bounded, deterministic.
        let ms = 100u64.saturating_mul(1 << (attempt - 1).min(4)).min(2000);
        let wait = Duration::from_millis(ms);
        self.taps.lock().expect("taps").waited.push(wait);
        (self.sleep)(wait);
    }

    fn wire_error(status: u16, body: &[u8]) -> RemoteError {
        let parsed: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
        RemoteError::Wire {
            status,
            code: parsed["error"].as_str().unwrap_or("unknown").to_owned(),
            reason: parsed["reason"].as_str().map(str::to_owned),
            head: parsed["head"].as_str().map(str::to_owned),
            height: parsed["height"].as_u64(),
        }
    }

    // ------------------------------------------------------ typed surface

    /// GET `/heads` — the two hot heads (never cached: `no-store`).
    pub fn heads(&self) -> Result<Heads, RemoteError> {
        let (status, _headers, body) = self.send("GET", "heads", None, b"", None, None)?;
        if status != 200 {
            return Err(Self::wire_error(status, &body));
        }
        let v: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| RemoteError::Transport(e.to_string()))?;
        let heads = Heads {
            height: v["height"].as_u64().unwrap_or(0),
            manifest: v["manifest"].as_str().map(str::to_owned),
            gamma: v["gamma"].as_str().map(str::to_owned),
            segment: v["segment"].as_str().map(str::to_owned),
        };
        // Knowing the heads IS tracking them (A.5): an absent head is
        // KNOWN-empty — the `If-Head: none` of the genesis publish and
        // the first append. A later conflict must fire on THIS
        // knowledge, never on a silent re-read.
        {
            let mut tracked = self.tracked.lock().expect("tracked");
            tracked.manifest_head =
                Some(heads.manifest.clone().unwrap_or_else(|| "none".to_owned()));
            tracked.gamma_head = Some(heads.gamma.clone().unwrap_or_else(|| "none".to_owned()));
        }
        Ok(heads)
    }

    /// Publish the manifest under the A.5 CAS. `If-Head` = the tracked
    /// head (or the served one when unknown; genesis = `none`). An
    /// accept adopts the returned head; a `cas_mismatch` ADOPTS the
    /// served head (the rebase input) and surfaces the verdict.
    pub fn publish_manifest(&self, bytes: &[u8]) -> Result<Ack, RemoteError> {
        let if_head = match self.tracked_manifest_head() {
            Some(h) => h,
            None => self.heads()?.manifest.unwrap_or_else(|| "none".to_owned()),
        };
        let (status, _headers, body) =
            self.send("PUT", "manifest.json", None, bytes, Some(&if_head), None)?;
        match status {
            200 => {
                let v: serde_json::Value = serde_json::from_slice(&body)
                    .map_err(|e| RemoteError::Transport(e.to_string()))?;
                let head = v["head"].as_str().unwrap_or_default().to_owned();
                self.tracked.lock().expect("tracked").manifest_head = Some(head.clone());
                Ok(Ack {
                    head,
                    height: v["height"].as_u64(),
                })
            }
            _ => {
                let err = Self::wire_error(status, &body);
                if let RemoteError::Wire {
                    head: Some(served), ..
                } = &err
                {
                    self.tracked.lock().expect("tracked").manifest_head = Some(served.clone());
                }
                Err(err)
            }
        }
    }

    /// Append ONE gamma entry (the JCS line, no trailing newline) via
    /// `POST /gamma` — the mode-B hot path, CAS on the gamma head.
    pub fn append_gamma(&self, entry_jcs: &[u8]) -> Result<Ack, RemoteError> {
        let if_head = match self.tracked_gamma_head() {
            Some(h) => h,
            None => self.heads()?.gamma.unwrap_or_else(|| "none".to_owned()),
        };
        let (status, _headers, body) =
            self.send("POST", "gamma", None, entry_jcs, Some(&if_head), None)?;
        match status {
            200 => {
                let v: serde_json::Value = serde_json::from_slice(&body)
                    .map_err(|e| RemoteError::Transport(e.to_string()))?;
                let head = v["head"].as_str().unwrap_or_default().to_owned();
                self.tracked.lock().expect("tracked").gamma_head = Some(head.clone());
                Ok(Ack { head, height: None })
            }
            _ => {
                let err = Self::wire_error(status, &body);
                if let RemoteError::Wire {
                    head: Some(served), ..
                } = &err
                {
                    self.tracked.lock().expect("tracked").gamma_head = Some(served.clone());
                }
                Err(err)
            }
        }
    }

    /// `get_many` — POST `/batch` (A.3): N chemins, UNE réponse
    /// multipart, une part par chemin dans l'ordre de la requête. Un
    /// chemin refusé ou absent est un VERDICT PAR PART (403/404), jamais
    /// une erreur du pack ; la réponse est `no-store` (jamais cachée).
    pub fn get_many(&self, paths: &[String]) -> Result<Vec<PackPart>, RemoteError> {
        let body = serde_json::json!({ "paths": paths })
            .to_string()
            .into_bytes();
        let (status, headers, resp) = self.send("POST", "batch", None, &body, None, None)?;
        if status != 200 {
            return Err(Self::wire_error(status, &resp));
        }
        self.parse_pack(&headers, &resp)
    }

    /// `sync` — POST `/sync {have_edition}` (A.3): le pack des chemins
    /// changés depuis l'édition tenue (manifest.json en première part).
    /// Une édition purgée répond `410 edition_gone` — le resync complet
    /// est la décision du CLIENT, jamais un silence.
    pub fn sync(&self, have_edition: u64) -> Result<Vec<PackPart>, RemoteError> {
        let body = serde_json::json!({ "have_edition": have_edition })
            .to_string()
            .into_bytes();
        let (status, headers, resp) = self.send("POST", "sync", None, &body, None, None)?;
        if status != 200 {
            return Err(Self::wire_error(status, &resp));
        }
        self.parse_pack(&headers, &resp)
    }

    /// Parse one `multipart/mixed` pack: boundary from the Content-Type
    /// (never assumed), per part the closed header block
    /// (`Content-Location` + `X-Aithos-Status`) then the bytes. The
    /// prefix `/t/<tenant>/<did>/` is stripped so callers reason in
    /// RELATIVE chemins, like the rest of the trait surface.
    fn parse_pack(
        &self,
        headers: &HashMap<String, String>,
        body: &[u8],
    ) -> Result<Vec<PackPart>, RemoteError> {
        let content_type = headers
            .get("content-type")
            .ok_or_else(|| RemoteError::Transport("pack without a content-type".into()))?;
        let boundary = content_type
            .split("boundary=")
            .nth(1)
            .map(|b| b.trim().trim_matches('"').to_owned())
            .ok_or_else(|| RemoteError::Transport("pack without a boundary".into()))?;
        let open = format!("--{boundary}\r\n");
        let sep = format!("\r\n--{boundary}");
        let text_of = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        // Split on the boundary markers; the first fragment (before the
        // first opener) is empty by construction.
        let raw = body;
        let mut parts = Vec::new();
        let open_b = open.as_bytes();
        let sep_b = sep.as_bytes();
        // position of the first opener
        let Some(first) = raw.windows(open_b.len()).position(|w| w == open_b) else {
            // an empty pack is just the closing marker
            return Ok(parts);
        };
        let mut at = first + open_b.len();
        loop {
            // the part ends at the next `\r\n--boundary`
            let rest = &raw[at..];
            let Some(end) = rest.windows(sep_b.len()).position(|w| w == sep_b) else {
                return Err(RemoteError::Transport("unterminated pack part".into()));
            };
            let part = &rest[..end];
            let Some(header_end) = part.windows(4).position(|w| w == b"\r\n\r\n") else {
                return Err(RemoteError::Transport("pack part without headers".into()));
            };
            let head = text_of(&part[..header_end]);
            let bytes = &part[header_end + 4..];
            let mut location = None;
            let mut status = None;
            for line in head.split("\r\n") {
                if let Some(v) = line.strip_prefix("Content-Location:") {
                    location = Some(v.trim().to_owned());
                }
                if let Some(v) = line.strip_prefix("X-Aithos-Status:") {
                    status = v.trim().parse::<u16>().ok();
                }
            }
            let (Some(location), Some(status)) = (location, status) else {
                return Err(RemoteError::Transport(
                    "pack part headers incomplete".into(),
                ));
            };
            let prefix = format!("/t/{}/{}/", self.tenant, self.did);
            let path = location
                .strip_prefix(&prefix)
                .unwrap_or(location.as_str())
                .to_owned();
            parts.push(PackPart {
                path,
                status,
                bytes: (status == 200).then(|| bytes.to_vec()),
            });
            // move past the separator; a following `--` closes the pack
            at += end + sep_b.len();
            if raw[at..].starts_with(b"--") {
                break;
            }
            // otherwise the separator is followed by `\r\n` (next part)
            if raw[at..].starts_with(b"\r\n") {
                at += 2;
            }
        }
        Ok(parts)
    }

    /// One GET with the A.6 cache honoured from the wire's own headers.
    fn get_wire(&self, relative: &str) -> Result<Option<Vec<u8>>, RemoteError> {
        // Immutable hit: no wire at all.
        if let Some(Cached::Immutable(bytes)) = self.cache.lock().expect("cache").get(relative) {
            return Ok(Some(bytes.clone()));
        }
        let etag = match self.cache.lock().expect("cache").get(relative) {
            Some(Cached::Revalidate { etag, .. }) => Some(etag.clone()),
            _ => None,
        };
        let (status, headers, body) =
            self.send("GET", relative, None, b"", None, etag.as_deref())?;
        match status {
            200 => {
                let cache_control = headers
                    .get("cache-control")
                    .cloned()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let mut cache = self.cache.lock().expect("cache");
                if cache_control.contains("immutable") {
                    cache.insert(relative.to_owned(), Cached::Immutable(body.clone()));
                } else if cache_control.contains("must-revalidate") {
                    if let Some(etag) = headers.get("etag") {
                        cache.insert(
                            relative.to_owned(),
                            Cached::Revalidate {
                                etag: etag.clone(),
                                bytes: body.clone(),
                            },
                        );
                    }
                } else {
                    // `no-store` (or unclassified): never kept.
                    cache.remove(relative);
                }
                drop(cache);
                // A gamma segment read is the diff base of the next append.
                if relative.starts_with("gamma/") {
                    self.tracked
                        .lock()
                        .expect("tracked")
                        .segments
                        .insert(relative.to_owned(), body.clone());
                }
                Ok(Some(body))
            }
            304 => match self.cache.lock().expect("cache").get(relative) {
                Some(Cached::Revalidate { bytes, .. }) => Ok(Some(bytes.clone())),
                _ => Err(RemoteError::Transport(
                    "304 without a cached representation".into(),
                )),
            },
            404 => {
                let err = Self::wire_error(status, &body);
                match &err {
                    RemoteError::Wire { code, .. } if code == "not_found" => {
                        // An absence INSIDE a covered perimeter (A.7) —
                        // the trait's `None`, and a gamma segment that
                        // does not exist yet is an empty diff base.
                        if relative.starts_with("gamma/") {
                            self.tracked
                                .lock()
                                .expect("tracked")
                                .segments
                                .insert(relative.to_owned(), Vec::new());
                        }
                        Ok(None)
                    }
                    _ => Err(err),
                }
            }
            _ => Err(Self::wire_error(status, &body)),
        }
    }

    /// The signed-JSON classes travel as their JCS bytes (A.1: "tout
    /// JSON signé = JCS"). The local bundle stores a pretty form for
    /// humans; the signature covers the JCS — canonicalizing at the
    /// deposit boundary loses nothing and is what the wire verifies.
    fn deposit_bytes(relative: &str, bytes: &[u8]) -> Vec<u8> {
        let signed_class =
            relative == "manifest.json" || relative == "did.json" || relative.starts_with("certs/");
        if signed_class {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
                if let Ok(jcs) = aithos_core::jcs::canonicalize(&value) {
                    return jcs.into_bytes();
                }
            }
        }
        bytes.to_vec()
    }

    /// The segment-aware PUT routing (mode B hot path vs replica).
    fn put_wire(&self, relative: &str, bytes: &[u8]) -> Result<(), RemoteError> {
        let canonical = Self::deposit_bytes(relative, bytes);
        let bytes: &[u8] = &canonical;
        if relative == "manifest.json" {
            return self.publish_manifest(bytes).map(|_| ());
        }
        if relative.starts_with("gamma/") && relative.ends_with(".jsonl") {
            // Diff against the segment content this client last observed:
            // exactly one appended full line → POST /gamma (mode B);
            // anything else → the replica PUT (mode A), same CAS.
            let known = self
                .tracked
                .lock()
                .expect("tracked")
                .segments
                .get(relative)
                .cloned();
            if let Some(known) = known {
                if bytes.len() > known.len() && bytes.starts_with(&known) {
                    let suffix = &bytes[known.len()..];
                    if suffix.ends_with(b"\n")
                        && suffix.iter().filter(|b| **b == b'\n').count() == 1
                    {
                        let line = &suffix[..suffix.len() - 1];
                        self.append_gamma(line)?;
                        self.tracked
                            .lock()
                            .expect("tracked")
                            .segments
                            .insert(relative.to_owned(), bytes.to_vec());
                        return Ok(());
                    }
                }
            }
            // Replica PUT under the segment-head CAS (A.3): the stored
            // content must stay a byte prefix — the server verifies.
            let if_head = match self.tracked_gamma_head() {
                Some(h) => h,
                None => self.heads()?.gamma.unwrap_or_else(|| "none".to_owned()),
            };
            let (status, _headers, body) =
                self.send("PUT", relative, None, bytes, Some(&if_head), None)?;
            if !(200..300).contains(&status) {
                let err = Self::wire_error(status, &body);
                if let RemoteError::Wire {
                    head: Some(served), ..
                } = &err
                {
                    self.tracked.lock().expect("tracked").gamma_head = Some(served.clone());
                }
                return Err(err);
            }
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) {
                if let Some(head) = v["head"].as_str() {
                    self.tracked.lock().expect("tracked").gamma_head = Some(head.to_owned());
                }
            }
            self.tracked
                .lock()
                .expect("tracked")
                .segments
                .insert(relative.to_owned(), bytes.to_vec());
            return Ok(());
        }
        // Plain artifact deposit (A.4 verifies server-side).
        let (status, _headers, body) = self.send("PUT", relative, None, bytes, None, None)?;
        if (200..300).contains(&status) {
            // A re-deposit under an immutable name may have changed what
            // we cached under a revalidate class — drop the entry.
            self.cache.lock().expect("cache").remove(relative);
            Ok(())
        } else {
            Err(Self::wire_error(status, &body))
        }
    }

    fn list_wire(&self, prefix: &str) -> Result<Vec<String>, RemoteError> {
        let mut out = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let query = match &after {
                Some(a) => format!("list={prefix}&after={a}"),
                None => format!("list={prefix}"),
            };
            let (status, _headers, body) = self.send("GET", "", Some(&query), b"", None, None)?;
            if status != 200 {
                return Err(Self::wire_error(status, &body));
            }
            let v: serde_json::Value =
                serde_json::from_slice(&body).map_err(|e| RemoteError::Transport(e.to_string()))?;
            let paths: Vec<String> = v["paths"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let truncated = v["truncated"].as_bool().unwrap_or(false);
            after = paths.last().cloned();
            out.extend(paths);
            if !truncated || after.is_none() {
                return Ok(out);
            }
        }
    }
}

impl Store for RemoteStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        self.get_wire(path).map_err(io::Error::from)
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.put_wire(path, bytes).map_err(io::Error::from)
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        self.list_wire(prefix).map_err(io::Error::from)
    }
}

/// Shared-handle wrapper: the gateway's store seam clones stores; a
/// remote store clone SHARES the tracked heads, cache and taps (one
/// wire identity per configured store, like the Mem arc).
#[derive(Clone)]
pub struct SharedRemoteStore(pub Arc<Mutex<RemoteStore>>);

impl SharedRemoteStore {
    pub fn new(store: RemoteStore) -> Self {
        Self(Arc::new(Mutex::new(store)))
    }
}

impl Store for SharedRemoteStore {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        self.0.lock().expect("remote store lock").get(path)
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.0.lock().expect("remote store lock").put(path, bytes)
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        self.0.lock().expect("remote store lock").list(prefix)
    }
}
