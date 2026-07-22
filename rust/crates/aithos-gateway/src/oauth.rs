//! The embedded OAuth 2.1 authorization server (lot G3, chantier C1).
//!
//! Served BY the gateway on the same listener as `/mcp` (never by
//! Aithos — INFRA §5: a provider-side AS could fabricate sessions). It
//! is the "projection" side of the mandate: a client trades a consent
//! for a short, audience-bound **access token** that only grants
//! *entry* to the hub — every ACT behind it is still re-verified against
//! the mandate chain (the token is a pointer, never an authority, C1).
//!
//! Deliberately dependency-free (the workspace lockfile belongs to the P
//! track): the token is a hand-rolled EdDSA JWT (compact JWS), signed by
//! the **adapter key** — an ordinary gateway secret, born at first run,
//! NEVER a protocol object (it does not live in the keyholder, it signs
//! nothing in the trust engine). base64url, the JWT envelope, PKCE S256
//! and the token stores are all local, small and fail-closed.
//!
//! Pre-G4 the session binds to the runner's **agent-chain authority**
//! through an INJECTABLE ceiling: `mint`/`refresh` take the bound
//! authority's `not_after` as a parameter (`ceiling`), and a refresh
//! never issues past it (C1: past `not_after`, redo the ceremony). G4/G5
//! swap in the session sub-mandate's `not_after` without touching this
//! module.
//!
//! Redaction discipline (like `credentials.rs`): no token, code, secret
//! or key byte is ever placed in a log, an error message or a panic.

use std::sync::{Arc, Mutex};

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroize;

use crate::core_bridge::EntropySource;
use crate::oauth_state::{AsStateStore, MemoryAsStateStore, StateNamespace};
use crate::{GatewayError, Result};

/// The exact Claude custom-connector callback (verified 2026-07-16):
/// public DCR client, PKCE, consent required. Always accepted.
pub const CLAUDE_CALLBACK: &str = "https://claude.ai/api/mcp/auth_callback";

/// Authorization-code lifetime: one exchange, promptly (RFC 6749 §4.1.2
/// recommends ≤ 10 min; the G4 ceremony contract fixes two minutes).
const CODE_TTL_SECS: i64 = 2 * 60;

/// The JWT `typ` for our access tokens (RFC 9068 style).
const ACCESS_TYP: &str = "at+jwt";

// ----------------------------------------------------------- base64url

/// URL-safe base64 without padding (RFC 4648 §5) — hand-rolled to avoid
/// a new dependency. Encoding is total; decoding is fail-closed.
pub fn b64url_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        }
    }
    out
}

/// Decode URL-safe base64 (no padding). Any stray byte fails closed.
pub fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        })
    };
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

// --------------------------------------------------------- adapter key

/// The token-signing key: an ordinary gateway secret (C1 §7.2), born
/// from injected entropy at the first `run` with `as:` active, persisted
/// 0600 beside the identity, rotated by replacing the file. It is NEVER
/// a protocol object — it lives outside the keyholder and signs nothing
/// in the trust engine.
pub struct AdapterKey {
    signing: SigningKey,
}

impl AdapterKey {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&seed),
        }
    }

    /// Born from injected entropy (the binary passes OS randomness, tests
    /// a deterministic source) — the purity rule extends here.
    pub fn generate(entropy: &mut dyn EntropySource) -> Self {
        Self::from_seed(entropy.e32())
    }

    fn verifying(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// The key id carried in the JWT header — the multibase of the
    /// adapter public key (a public value, safe to publish).
    pub fn kid(&self) -> String {
        aithos_core::wire::ed25519_pub_to_multibase(&self.verifying().to_bytes())
    }

    /// Persist to a 0600 file (unix), hex — the same custody discipline
    /// as the runner identity, but a distinct, standalone secret.
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let body = hex::encode(self.signing.to_bytes());
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
            }
        }
        std::fs::write(path, body).map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| GatewayError::IdentityUnavailable(e.to_string()))?;
        }
        Ok(())
    }

    /// Load an existing adapter key, or mint+persist one on first use.
    /// Fail-closed: a present-but-malformed file is an error, never a
    /// silent re-key (that would invalidate live tokens without a trace).
    pub fn load_or_create(path: &std::path::Path, entropy: &mut dyn EntropySource) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut raw = hex::decode(text.trim())
                    .ok()
                    .and_then(|v| <[u8; 32]>::try_from(v).ok())
                    .ok_or_else(|| {
                        GatewayError::IdentityUnavailable(
                            "adapter key file is not 32 hex bytes".into(),
                        )
                    })?;
                let key = Self::from_seed(raw);
                raw.zeroize();
                Ok(key)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let key = Self::generate(entropy);
                key.save(path)?;
                Ok(key)
            }
            Err(e) => Err(GatewayError::IdentityUnavailable(e.to_string())),
        }
    }

    /// Load the adapter key from the dedicated AS state custody, or create it
    /// with CAS on first use. Production never falls back to a local file.
    pub fn load_or_create_in_state(
        state: &dyn AsStateStore,
        entropy: &mut dyn EntropySource,
    ) -> Result<Self> {
        if let Some(record) = state.read(StateNamespace::AdapterKey, "active")? {
            return adapter_from_state(record.value);
        }
        let mut raw = entropy.e32();
        let key = Self::from_seed(raw);
        let value = json!({ "v": 1, "seed_hex": hex::encode(raw) });
        let created = state.create(StateNamespace::AdapterKey, "active", value);
        raw.zeroize();
        match created {
            Ok(_) => Ok(key),
            Err(_) => {
                // A concurrent first start may have won CAS. Load its key;
                // any other state fault still fails closed.
                let record = state
                    .read(StateNamespace::AdapterKey, "active")?
                    .ok_or_else(|| oauth_state_error("adapter key creation was refused"))?;
                adapter_from_state(record.value)
            }
        }
    }

    /// Sign an access token (compact EdDSA JWS) with the given claims —
    /// the one public signing primitive. Handy for the acceptance
    /// harness to forge "right key, wrong audience" tokens; the AS uses
    /// it internally for every minting. Exposing it leaks no key: signing
    /// still requires custody of this `AdapterKey`.
    pub fn sign_access_token(&self, claims: &Value) -> String {
        self.sign_jwt(ACCESS_TYP, claims)
    }

    /// Sign a compact JWS (EdDSA): `b64url(header).b64url(payload).b64url(sig)`.
    fn sign_jwt(&self, typ: &str, claims: &Value) -> String {
        let header = json!({ "alg": "EdDSA", "typ": typ, "kid": self.kid() });
        let signing_input = format!(
            "{}.{}",
            b64url_encode(header.to_string().as_bytes()),
            b64url_encode(claims.to_string().as_bytes())
        );
        let sig = self.signing.sign(signing_input.as_bytes());
        format!("{signing_input}.{}", b64url_encode(&sig.to_bytes()))
    }

    /// Verify a compact JWS against the adapter key and return its
    /// claims. Fail-closed on shape, algorithm, or signature — the error
    /// is a fixed string, never the offending token.
    fn verify_jwt(&self, token: &str) -> Result<Value> {
        let mut parts = token.split('.');
        let (Some(h), Some(p), Some(s), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid_token());
        };
        let header: Value = b64url_decode(h)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or_else(invalid_token)?;
        if header.get("alg").and_then(Value::as_str) != Some("EdDSA") {
            return Err(invalid_token());
        }
        let sig_bytes: [u8; 64] = b64url_decode(s)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(invalid_token)?;
        let signing_input = format!("{h}.{p}");
        self.verifying()
            .verify(
                signing_input.as_bytes(),
                &ed25519_dalek::Signature::from_bytes(&sig_bytes),
            )
            .map_err(|_| invalid_token())?;
        b64url_decode(p)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or_else(invalid_token)
    }
}

fn adapter_from_state(value: Value) -> Result<AdapterKey> {
    let object = value
        .as_object()
        .filter(|object| object.len() == 2)
        .ok_or_else(|| oauth_state_error("stored adapter key is malformed"))?;
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(oauth_state_error("stored adapter key is malformed"));
    }
    let mut seed = object
        .get("seed_hex")
        .and_then(Value::as_str)
        .and_then(|encoded| hex::decode(encoded).ok())
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or_else(|| oauth_state_error("stored adapter key is malformed"))?;
    let key = AdapterKey::from_seed(seed);
    seed.zeroize();
    Ok(key)
}

/// One `invalid_token` refusal — never carries the token bytes.
fn invalid_token() -> GatewayError {
    GatewayError::OauthDenied {
        error: "invalid_token".into(),
        detail: "the bearer token is missing, malformed, expired or not for this resource".into(),
    }
}

/// One OAuth protocol error with a fixed, leak-free reason.
fn oauth_err(error: &str, detail: &str) -> GatewayError {
    GatewayError::OauthDenied {
        error: error.to_owned(),
        detail: detail.to_owned(),
    }
}

fn oauth_state_error(detail: &str) -> GatewayError {
    oauth_err("temporarily_unavailable", detail)
}

// ---------------------------------------------------------- token stores

/// A registered public client (DCR, RFC 7591): PKCE, no secret ever.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientRecord {
    redirect_uris: Vec<String>,
}

/// A live authorization code (one-shot, PKCE-bound, resource-bound).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeRecord {
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    resource: String,
    expires: i64,
}

/// A refresh rotation family. Only token digests are persisted. Reusing any
/// consumed token cuts the complete family (OAuth 2.1 rotation semantics).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshFamilyRecord {
    client_id: String,
    resource: String,
    expires: i64,
    current_hash: String,
    consumed_hashes: Vec<String>,
    revoked: bool,
}

// --------------------------------------------------------- authorize IO

/// What the `/authorize` endpoint decides — the router turns this into
/// an HTTP response (the transport concern stays out of the pure core).
pub enum AuthorizeOutcome {
    /// Render the DEV consent page (HTML) — the request is well-formed.
    Consent { html: String },
    /// Redirect back to the client's registered URI with query params
    /// (a code on approval, or an OAuth `error` on a recoverable fault).
    Redirect { location: String },
    /// A fault that MUST NOT redirect (unknown client / mismatched URI):
    /// answered as a plain 400, naming the supported path.
    HardError { detail: String },
}

/// Parsed `/authorize` query (whatever the transport extracted).
pub struct AuthorizeRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub resource: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
}

/// The minted token pair returned by `/token`.
pub struct TokenGrant {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_secs: i64,
}

// ------------------------------------------------------------ the AS

/// The authorization server state. Raw codes and refresh tokens are never
/// persisted; the injected state store receives only closed records and
/// one-way token digests.
pub struct AuthServer {
    adapter: AdapterKey,
    issuer: String,
    /// The protected resource = the `/mcp` endpoint (RFC 8707 audience).
    resource: String,
    access_ttl: i64,
    refresh_ttl: i64,
    /// Extra redirect_uris accepted beyond the built-in allowlist.
    extra_redirects: Vec<String>,
    state: Arc<dyn AsStateStore>,
    entropy: Mutex<Box<dyn EntropySource + Send>>,
}

impl AuthServer {
    pub fn new(
        adapter: AdapterKey,
        issuer: &str,
        access_ttl: i64,
        refresh_ttl: i64,
        extra_redirects: Vec<String>,
        entropy: Box<dyn EntropySource + Send>,
    ) -> Self {
        Self::new_with_state(
            adapter,
            issuer,
            access_ttl,
            refresh_ttl,
            extra_redirects,
            entropy,
            Arc::new(MemoryAsStateStore::default()),
        )
    }

    /// Construct the AS over an injected durable store. Reconstructing an AS
    /// with the same store models a real restart without replaying state.
    pub fn new_with_state(
        adapter: AdapterKey,
        issuer: &str,
        access_ttl: i64,
        refresh_ttl: i64,
        extra_redirects: Vec<String>,
        entropy: Box<dyn EntropySource + Send>,
        state: Arc<dyn AsStateStore>,
    ) -> Self {
        let issuer = issuer.trim_end_matches('/').to_owned();
        let resource = format!("{issuer}/mcp");
        Self {
            adapter,
            issuer,
            resource,
            access_ttl,
            refresh_ttl,
            extra_redirects,
            state,
            entropy: Mutex::new(entropy),
        }
    }

    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// The RFC 9728 protected-resource metadata: this `/mcp` resource
    /// points at exactly one authorization server (the issuer).
    pub fn protected_resource_metadata(&self) -> Value {
        json!({
            "resource": self.resource,
            "authorization_servers": [self.issuer],
        })
    }

    /// The RFC 8414 authorization-server metadata: endpoints, S256 only,
    /// public clients only, the two grant types we serve.
    pub fn authorization_server_metadata(&self) -> Value {
        json!({
            "issuer": self.issuer,
            "authorization_endpoint": format!("{}/authorize", self.issuer),
            "token_endpoint": format!("{}/token", self.issuer),
            "registration_endpoint": format!("{}/register", self.issuer),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
        })
    }

    /// The `WWW-Authenticate` value a 401 on `/mcp` carries — it points
    /// the resource metadata so a client discovers the AS (RFC 9728).
    pub fn www_authenticate(&self, invalid: bool) -> String {
        let base = format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource\"",
            self.issuer
        );
        if invalid {
            format!("{base}, error=\"invalid_token\"")
        } else {
            base
        }
    }

    fn mint_opaque(&self, prefix: &str) -> String {
        let mut ent = self.entropy.lock().expect("entropy lock");
        format!("{prefix}_{}", hex::encode(ent.e32()))
    }

    fn load_record<T: DeserializeOwned>(
        &self,
        namespace: StateNamespace,
        id: &str,
    ) -> Result<Option<(T, u64)>> {
        self.state
            .read(namespace, id)?
            .map(|record| {
                serde_json::from_value(record.value)
                    .map(|value| (value, record.version))
                    .map_err(|_| oauth_state_error("stored OAuth state is malformed"))
            })
            .transpose()
    }

    fn create_record<T: Serialize>(
        &self,
        namespace: StateNamespace,
        id: &str,
        record: &T,
    ) -> Result<u64> {
        let value = serde_json::to_value(record)
            .map_err(|_| oauth_state_error("OAuth state serialization failed"))?;
        self.state.create(namespace, id, value)
    }

    fn replace_record<T: Serialize>(
        &self,
        namespace: StateNamespace,
        id: &str,
        expected_version: u64,
        record: &T,
    ) -> Result<u64> {
        let value = serde_json::to_value(record)
            .map_err(|_| oauth_state_error("OAuth state serialization failed"))?;
        self.state
            .compare_and_swap(namespace, id, expected_version, value)
    }

    /// Is this redirect_uri acceptable? The built-in allowlist is the
    /// exact Claude callback plus loopback on any port (RFC 8252); the
    /// stanza may extend it with exact entries.
    fn redirect_allowed(&self, uri: &str) -> bool {
        uri == CLAUDE_CALLBACK
            || is_loopback_redirect(uri)
            || self.extra_redirects.iter().any(|u| u == uri)
    }

    // ------------------------------------------------ dynamic registration

    /// RFC 7591 registration, public PKCE clients only. Returns the
    /// client info document (no secret is ever issued).
    pub fn register(&self, request: &Value) -> Result<Value> {
        // Public clients only: the method must be `none` (or absent).
        if let Some(method) = request
            .get("token_endpoint_auth_method")
            .and_then(Value::as_str)
        {
            if method != "none" {
                return Err(oauth_err(
                    "invalid_client_metadata",
                    "this authorization server registers public PKCE clients only \
                     (token_endpoint_auth_method must be \"none\")",
                ));
            }
        }
        let uris: Vec<String> = request
            .get("redirect_uris")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if uris.is_empty() {
            return Err(oauth_err(
                "invalid_redirect_uri",
                "at least one redirect_uri is required",
            ));
        }
        for uri in &uris {
            if !self.redirect_allowed(uri) {
                return Err(oauth_err(
                    "invalid_redirect_uri",
                    "redirect_uri is off the built-in allowlist (the Claude callback \
                     and http://localhost:*|127.0.0.1:* on any port) and off the \
                     configured redirect_allowlist",
                ));
            }
        }
        let client_id = self.mint_opaque("client");
        self.create_record(
            StateNamespace::DcrClient,
            &client_id,
            &ClientRecord {
                redirect_uris: uris.clone(),
            },
        )?;
        Ok(json!({
            "client_id": client_id,
            "token_endpoint_auth_method": "none",
            "redirect_uris": uris,
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        }))
    }

    // ---------------------------------------------------------- authorize

    /// Validate an `/authorize` request. A well-formed one renders the
    /// DEV consent; a recoverable fault redirects an OAuth `error`; an
    /// unknown client or a mismatched redirect never redirects.
    pub fn authorize(&self, req: &AuthorizeRequest) -> AuthorizeOutcome {
        let client =
            match self.load_record::<ClientRecord>(StateNamespace::DcrClient, &req.client_id) {
                Ok(Some((client, _))) => client,
                Ok(None) => {
                    return AuthorizeOutcome::HardError {
                        detail: "unknown client_id — register dynamically first (RFC 7591); \
                             self-asserted client_id URLs (CIMD) are not served yet"
                            .into(),
                    }
                }
                Err(_) => {
                    return AuthorizeOutcome::HardError {
                        detail: "authorization state is temporarily unavailable".into(),
                    }
                }
            };
        if !client.redirect_uris.contains(&req.redirect_uri) {
            return AuthorizeOutcome::HardError {
                detail: "redirect_uri does not match the registration".into(),
            };
        }

        // From here every fault is a redirect (the URI is trusted).
        let err_redirect = |error: &str, desc: &str| AuthorizeOutcome::Redirect {
            location: redirect_with(
                &req.redirect_uri,
                &[("error", error), ("error_description", desc)],
                req.state.as_deref(),
            ),
        };
        if req.response_type != "code" {
            return err_redirect(
                "unsupported_response_type",
                "only response_type=code is served",
            );
        }
        match req.code_challenge_method.as_deref() {
            Some("S256") => {}
            Some(_) => {
                return err_redirect("invalid_request", "PKCE code_challenge_method must be S256");
            }
            None if req.code_challenge.is_some() => {
                return err_redirect("invalid_request", "PKCE code_challenge_method must be S256");
            }
            None => {}
        }
        let Some(challenge) = req.code_challenge.clone() else {
            return err_redirect(
                "invalid_request",
                "PKCE is required: send a code_challenge (S256)",
            );
        };
        let Some(resource) = req.resource.clone() else {
            return err_redirect(
                "invalid_target",
                "the resource parameter is required (RFC 8707)",
            );
        };
        if resource != self.resource {
            return err_redirect("invalid_target", "the resource does not match this hub");
        }

        // Well-formed: the DEV consent page. It carries the request in a
        // signed-free hidden form; approval POSTs it back to /authorize.
        let html = consent_page(
            &req.client_id,
            &req.redirect_uri,
            &resource,
            &challenge,
            req.scope.as_deref(),
            req.state.as_deref(),
        );
        AuthorizeOutcome::Consent { html }
    }

    /// The consent POST: the user approved, so mint a one-shot code and
    /// redirect back. Re-validates the client and redirect (never trust
    /// the form blindly).
    pub fn approve(&self, req: &AuthorizeRequest, now: &str) -> Result<String> {
        let (client, _) = self
            .load_record::<ClientRecord>(StateNamespace::DcrClient, &req.client_id)?
            .ok_or_else(|| oauth_err("invalid_request", "unknown client_id"))?;
        if !client.redirect_uris.contains(&req.redirect_uri) {
            return Err(oauth_err("invalid_request", "redirect_uri mismatch"));
        }
        let challenge = req
            .code_challenge
            .clone()
            .ok_or_else(|| oauth_err("invalid_request", "missing code_challenge"))?;
        let resource = req
            .resource
            .clone()
            .ok_or_else(|| oauth_err("invalid_target", "missing resource"))?;
        if resource != self.resource {
            return Err(oauth_err("invalid_target", "resource mismatch"));
        }
        let code = self.mint_opaque("code");
        let expires = epoch(now)? + CODE_TTL_SECS;
        self.create_record(
            StateNamespace::Code,
            &token_digest(&code),
            &CodeRecord {
                client_id: req.client_id.clone(),
                redirect_uri: req.redirect_uri.clone(),
                code_challenge: challenge,
                resource,
                expires,
            },
        )?;
        Ok(redirect_with(
            &req.redirect_uri,
            &[("code", &code)],
            req.state.as_deref(),
        ))
    }

    // ------------------------------------------------------------- token

    /// The authorization-code grant: verify the one-shot code, the PKCE
    /// verifier and the resource, then mint the audience-bound pair. The
    /// `ceiling` is the bound authority's `not_after` (injectable): the
    /// token never outlives it. Returns the grant AND the client_id (the
    /// caller journalizes the issuance — never silent, I5).
    pub fn exchange_code(
        &self,
        code: &str,
        verifier: &str,
        resource: &str,
        redirect_uri: &str,
        ceiling: Option<&str>,
        now: &str,
    ) -> Result<(TokenGrant, String)> {
        let now_epoch = epoch(now)?;
        // One-shot: reserve on sight, valid or not (a replay finds nothing).
        let record: CodeRecord = self
            .state
            .take(StateNamespace::Code, &token_digest(code))?
            .ok_or_else(|| oauth_err("invalid_grant", "unknown, used or expired code"))
            .and_then(|value| {
                serde_json::from_value(value)
                    .map_err(|_| oauth_state_error("stored authorization code is malformed"))
            })?;
        if now_epoch > record.expires {
            return Err(oauth_err("invalid_grant", "the authorization code expired"));
        }
        if record.redirect_uri != redirect_uri {
            return Err(oauth_err("invalid_grant", "redirect_uri mismatch"));
        }
        if record.resource != resource {
            return Err(oauth_err("invalid_target", "resource mismatch"));
        }
        if !pkce_matches(verifier, &record.code_challenge) {
            return Err(oauth_err("invalid_grant", "PKCE verifier does not match"));
        }
        let grant = self.mint_pair(&record.client_id, resource, ceiling, now_epoch)?;
        Ok((grant, record.client_id))
    }

    /// The refresh grant: rotate one-shot. A reuse of a consumed token
    /// cuts the whole family. The `ceiling` re-bounds the fresh tokens —
    /// past the authority's `not_after`, the refresh is refused.
    pub fn refresh(
        &self,
        refresh_token: &str,
        ceiling: Option<&str>,
        now: &str,
    ) -> Result<(TokenGrant, String)> {
        let now_epoch = epoch(now)?;
        let family = refresh_family_id(refresh_token)
            .ok_or_else(|| oauth_err("invalid_grant", "unknown refresh token"))?;
        let (mut record, version) = self
            .load_record::<RefreshFamilyRecord>(StateNamespace::RefreshFamily, family)?
            .ok_or_else(|| oauth_err("invalid_grant", "unknown refresh token"))?;
        if record.revoked {
            return Err(oauth_err(
                "invalid_grant",
                "the refresh session family has been revoked",
            ));
        }
        let presented_hash = token_digest(refresh_token);
        if record
            .consumed_hashes
            .iter()
            .any(|digest| digest == &presented_hash)
        {
            record.revoked = true;
            self.replace_record(StateNamespace::RefreshFamily, family, version, &record)?;
            return Err(oauth_err(
                "invalid_grant",
                "refresh token already used — the session family has been revoked",
            ));
        }
        if record.current_hash != presented_hash {
            return Err(oauth_err("invalid_grant", "unknown refresh token"));
        }
        if now_epoch > record.expires {
            record.revoked = true;
            self.replace_record(StateNamespace::RefreshFamily, family, version, &record)?;
            return Err(oauth_err("invalid_grant", "the refresh token expired"));
        }
        // The authority ceiling MUST still be live — this is where "past
        // not_after, redo the ceremony" bites.
        let ceiling_epoch = match ceiling {
            Some(c) => epoch(c)?,
            None => {
                return Err(oauth_err(
                    "invalid_grant",
                    "the bound authority is no longer valid — restart the authorization flow",
                ))
            }
        };
        if now_epoch >= ceiling_epoch {
            return Err(oauth_err(
                "invalid_grant",
                "the bound authority has expired — restart the authorization flow",
            ));
        }
        if record.consumed_hashes.len() >= 64 {
            record.revoked = true;
            self.replace_record(StateNamespace::RefreshFamily, family, version, &record)?;
            return Err(oauth_err(
                "invalid_grant",
                "the refresh session family reached its rotation bound",
            ));
        }

        let refresh_token = format!("refresh.{family}.{}", self.mint_opaque("next"));
        record.consumed_hashes.push(presented_hash);
        record.current_hash = token_digest(&refresh_token);
        self.replace_record(StateNamespace::RefreshFamily, family, version, &record)?;
        let (access_token, access_expires_secs) =
            self.mint_access(&record.client_id, &record.resource, ceiling, now_epoch)?;
        let client_id = record.client_id;
        Ok((
            TokenGrant {
                access_token,
                refresh_token,
                access_expires_secs,
            },
            client_id,
        ))
    }

    /// Mint an access+refresh pair, both capped by the authority ceiling.
    /// Only a digest of the refresh token enters durable state.
    fn mint_pair(
        &self,
        client_id: &str,
        resource: &str,
        ceiling: Option<&str>,
        now_epoch: i64,
    ) -> Result<TokenGrant> {
        let ceiling_epoch = ceiling.map(epoch).transpose()?;
        let refresh_exp = ceiling_epoch
            .map(|ceiling| (now_epoch + self.refresh_ttl).min(ceiling))
            .unwrap_or(now_epoch + self.refresh_ttl);
        let family = self.mint_opaque("fam");
        let refresh_token = format!("refresh.{family}.{}", self.mint_opaque("first"));
        self.create_record(
            StateNamespace::RefreshFamily,
            &family,
            &RefreshFamilyRecord {
                client_id: client_id.to_owned(),
                resource: resource.to_owned(),
                expires: refresh_exp,
                current_hash: token_digest(&refresh_token),
                consumed_hashes: Vec::new(),
                revoked: false,
            },
        )?;
        let (access_token, access_expires_secs) =
            self.mint_access(client_id, resource, ceiling, now_epoch)?;
        Ok(TokenGrant {
            access_token,
            refresh_token,
            access_expires_secs,
        })
    }

    fn mint_access(
        &self,
        client_id: &str,
        resource: &str,
        ceiling: Option<&str>,
        now_epoch: i64,
    ) -> Result<(String, i64)> {
        let ceiling_epoch = ceiling.map(epoch).transpose()?;
        let access_exp = ceiling_epoch
            .map(|ceiling| (now_epoch + self.access_ttl).min(ceiling))
            .unwrap_or(now_epoch + self.access_ttl);
        let jti = self.mint_opaque("jti");
        let claims = json!({
            "iss": self.issuer,
            "sub": "aithos-runner",
            "aud": resource,
            "iat": now_epoch,
            "exp": access_exp,
            "jti": jti,
            "client_id": client_id,
        });
        let access_token = self.adapter.sign_jwt(ACCESS_TYP, &claims);
        Ok((access_token, (access_exp - now_epoch).max(0)))
    }

    /// Validate a bearer token presented on `/mcp`: signature (adapter
    /// key), audience (this resource), expiry. Returns nothing useful to
    /// leak — success means "may enter", the mandate chain still decides
    /// every act behind it.
    pub fn validate_bearer(&self, token: &str, now: &str) -> Result<()> {
        let claims = self.adapter.verify_jwt(token)?;
        if claims.get("aud").and_then(Value::as_str) != Some(self.resource.as_str()) {
            return Err(invalid_token());
        }
        let exp = claims
            .get("exp")
            .and_then(Value::as_i64)
            .ok_or_else(invalid_token)?;
        if epoch(now)? >= exp {
            return Err(invalid_token());
        }
        Ok(())
    }

    #[cfg(test)]
    fn sign_for_test(&self, claims: &Value) -> String {
        self.adapter.sign_access_token(claims)
    }
}

// ------------------------------------------------------------ helpers

/// RFC 3339 Z instant → epoch seconds, through the core's strict parser.
fn epoch(now: &str) -> Result<i64> {
    aithos_core::gamma::ts_epoch(now)
        .map_err(|_| GatewayError::BridgeFailed(format!("bad instant `{now}`")))
}

fn token_digest(token: &str) -> String {
    aithos_core::gamma::sha256_hex(token.as_bytes())
}

fn refresh_family_id(token: &str) -> Option<&str> {
    let mut parts = token.split('.');
    let (Some("refresh"), Some(family), Some(secret), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if family.is_empty() || secret.is_empty() {
        return None;
    }
    Some(family)
}

/// The PKCE S256 challenge for a verifier: `base64url(sha256(verifier))`
/// (RFC 7636 §4.2). The raw digest comes from the core's `sha256_hex`
/// (decoded back to bytes) so no new hashing dependency enters the crate.
/// Public so a client harness can derive the challenge the same way.
pub fn s256_challenge(verifier: &str) -> String {
    let hex = aithos_core::gamma::sha256_hex(verifier.as_bytes());
    let bytes = hex::decode(hex).unwrap_or_default();
    b64url_encode(&bytes)
}

/// PKCE S256 match: the derived challenge equals the stored one.
fn pkce_matches(verifier: &str, challenge: &str) -> bool {
    s256_challenge(verifier) == challenge
}

/// Is this a loopback http(s) redirect (RFC 8252 §7.3 — any port)?
fn is_loopback_redirect(uri: &str) -> bool {
    let rest = match uri
        .strip_prefix("http://")
        .or_else(|| uri.strip_prefix("https://"))
    {
        Some(rest) => rest,
        None => return false,
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or_default()
    } else {
        host_port.rsplit_once(':').map_or(host_port, |(h, _)| h)
    };
    host == "localhost"
        || host == "::1"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Build a redirect URI with appended query params (and the echoed
/// state, when present). Values are percent-encoded for the query.
fn redirect_with(base: &str, params: &[(&str, &str)], state: Option<&str>) -> String {
    let sep = if base.contains('?') { '&' } else { '?' };
    let mut query = String::new();
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            query.push('&');
        }
        query.push_str(k);
        query.push('=');
        query.push_str(&percent_encode(v));
    }
    if let Some(state) = state {
        query.push_str("&state=");
        query.push_str(&percent_encode(state));
    }
    format!("{base}{sep}{query}")
}

/// Minimal RFC 3986 query percent-encoding (unreserved kept as-is).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

/// Escape text for safe interpolation into the consent HTML body.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The DEV consent page: a single Approve button that POSTs the request
/// back to `/authorize`. Honest about being a development stand-in for
/// the G4 ceremony; names the client and the resource so the human sees
/// exactly what is being authorized. NO secret is ever placed here (the
/// code challenge is public PKCE material by construction).
fn consent_page(
    client_id: &str,
    redirect_uri: &str,
    resource: &str,
    challenge: &str,
    scope: Option<&str>,
    state: Option<&str>,
) -> String {
    let hidden = |name: &str, value: &str| {
        format!(
            "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
            name,
            html_escape(value)
        )
    };
    let mut fields = String::new();
    fields.push_str(&hidden("client_id", client_id));
    fields.push_str(&hidden("redirect_uri", redirect_uri));
    fields.push_str(&hidden("response_type", "code"));
    fields.push_str(&hidden("code_challenge", challenge));
    fields.push_str(&hidden("code_challenge_method", "S256"));
    fields.push_str(&hidden("resource", resource));
    if let Some(scope) = scope {
        fields.push_str(&hidden("scope", scope));
    }
    if let Some(state) = state {
        fields.push_str(&hidden("state", state));
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>Aithos gateway — authorize (DEV)</title></head><body>\
         <main style=\"font-family:system-ui;max-width:34rem;margin:3rem auto\">\
         <p style=\"background:#fee;border:1px solid #c00;padding:.5rem 1rem;border-radius:.4rem\">\
         <strong>DEV consent</strong> — auto-consent stand-in for the G4 ceremony. \
         Do not ship this screen to production.</p>\
         <h1>Authorize a connection</h1>\
         <p>Client <code>{client}</code> is requesting access to \
         <code>{resource}</code>.</p>\
         <form method=\"post\" action=\"/authorize\">{fields}\
         <button type=\"submit\" style=\"font-size:1rem;padding:.6rem 1.4rem\">Approve</button>\
         </form></main></body></html>",
        client = html_escape(client_id),
        resource = html_escape(resource),
        fields = fields,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_bridge::SeqEntropy;

    const T0: &str = "2026-07-17T12:00:00Z";
    const ISSUER: &str = "http://127.0.0.1:4870";
    const CHAIN_END: &str = "2026-08-09T00:00:00Z";

    fn server() -> AuthServer {
        let mut ent = SeqEntropy::default();
        let adapter = AdapterKey::generate(&mut ent);
        AuthServer::new(
            adapter,
            ISSUER,
            3_600,
            7 * 86_400,
            Vec::new(),
            Box::new(SeqEntropy::default()),
        )
    }

    fn restarted_server(state: Arc<MemoryAsStateStore>) -> AuthServer {
        AuthServer::new_with_state(
            AdapterKey::from_seed([42; 32]),
            ISSUER,
            3_600,
            7 * 86_400,
            Vec::new(),
            Box::new(SeqEntropy::default()),
            state,
        )
    }

    fn resource() -> String {
        format!("{ISSUER}/mcp")
    }

    /// A verifier and its S256 challenge (the ASCII verifier is fine for
    /// tests; production verifiers are 43+ chars of entropy).
    fn pkce() -> (String, String) {
        let verifier = "the-quick-brown-fox-jumps-over-the-lazy-dog-1234".to_owned();
        let hex = aithos_core::gamma::sha256_hex(verifier.as_bytes());
        let challenge = b64url_encode(&hex::decode(hex).unwrap());
        (verifier, challenge)
    }

    fn register(as_: &AuthServer, uri: &str) -> String {
        let doc = as_
            .register(&json!({ "redirect_uris": [uri] }))
            .expect("registers");
        assert!(doc.get("client_secret").is_none(), "public client");
        doc["client_id"].as_str().unwrap().to_owned()
    }

    fn approve_code(as_: &AuthServer, client_id: &str, uri: &str, challenge: &str) -> String {
        approve_code_at(as_, client_id, uri, challenge, T0)
    }

    fn approve_code_at(
        as_: &AuthServer,
        client_id: &str,
        uri: &str,
        challenge: &str,
        now: &str,
    ) -> String {
        let req = AuthorizeRequest {
            client_id: client_id.to_owned(),
            redirect_uri: uri.to_owned(),
            response_type: "code".to_owned(),
            code_challenge: Some(challenge.to_owned()),
            code_challenge_method: Some("S256".to_owned()),
            resource: Some(resource()),
            scope: None,
            state: Some("xyz".to_owned()),
        };
        let location = as_.approve(&req, now).expect("approves");
        // pull the code out of the redirect query
        location
            .split(['?', '&'])
            .find_map(|kv| kv.strip_prefix("code="))
            .expect("a code")
            .to_owned()
    }

    #[test]
    fn base64url_roundtrips_every_length() {
        for len in 0..40 {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 7 + 1) as u8).collect();
            let enc = b64url_encode(&bytes);
            assert!(!enc.contains('='), "no padding");
            assert!(!enc.contains('+') && !enc.contains('/'), "url-safe");
            assert_eq!(b64url_decode(&enc).as_deref(), Some(bytes.as_slice()));
        }
    }

    #[test]
    fn jwt_roundtrips_and_a_tamper_fails() {
        let as_ = server();
        let token = as_.sign_for_test(&json!({ "aud": resource(), "exp": 9_999_999_999i64 }));
        assert!(as_.adapter.verify_jwt(&token).is_ok());
        // Flip one payload byte → signature no longer verifies.
        let mut parts: Vec<&str> = token.split('.').collect();
        let mut payload = b64url_decode(parts[1]).unwrap();
        payload[0] ^= 0x01;
        let bad = b64url_encode(&payload);
        parts[1] = &bad;
        let tampered = parts.join(".");
        assert!(as_.adapter.verify_jwt(&tampered).is_err());
    }

    #[test]
    fn adapter_key_custody_survives_restart_in_the_secret_namespace() {
        let state = MemoryAsStateStore::default();
        let mut first_entropy = SeqEntropy::default();
        let first = AdapterKey::load_or_create_in_state(&state, &mut first_entropy).unwrap();
        let first_kid = first.kid();
        drop(first);

        let mut different_entropy = SeqEntropy::default();
        let _ = different_entropy.e32();
        let second = AdapterKey::load_or_create_in_state(&state, &mut different_entropy).unwrap();
        assert_eq!(second.kid(), first_kid);
        assert!(state
            .read(StateNamespace::DcrClient, "active")
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_forged_token_from_another_key_is_refused() {
        let as_ = server();
        let mut ent = SeqEntropy::default();
        let _ = ent.e32();
        let other = AdapterKey::generate(&mut ent);
        let forged = other.sign_jwt(
            ACCESS_TYP,
            &json!({ "aud": resource(), "exp": 9_999_999_999i64 }),
        );
        assert!(as_.validate_bearer(&forged, T0).is_err());
    }

    #[test]
    fn discovery_metadata_pins_the_contract() {
        let as_ = server();
        let prm = as_.protected_resource_metadata();
        assert_eq!(prm["resource"], resource());
        assert_eq!(prm["authorization_servers"][0], ISSUER);
        let asm = as_.authorization_server_metadata();
        assert_eq!(asm["code_challenge_methods_supported"], json!(["S256"]));
        assert_eq!(
            asm["token_endpoint_auth_methods_supported"],
            json!(["none"])
        );
        assert_eq!(
            asm["grant_types_supported"],
            json!(["authorization_code", "refresh_token"])
        );
    }

    #[test]
    fn registration_bounds_redirects() {
        let as_ = server();
        // Claude callback + loopback register; a random https does not.
        assert!(as_
            .register(&json!({ "redirect_uris": [CLAUDE_CALLBACK] }))
            .is_ok());
        assert!(as_
            .register(&json!({ "redirect_uris": ["http://localhost:9999/cb"] }))
            .is_ok());
        assert!(matches!(
            as_.register(&json!({ "redirect_uris": ["https://evil.example/cb"] })),
            Err(GatewayError::OauthDenied { error, .. }) if error == "invalid_redirect_uri"
        ));
        assert!(matches!(
            as_.register(&json!({
                "redirect_uris": [CLAUDE_CALLBACK],
                "token_endpoint_auth_method": "client_secret_basic"
            })),
            Err(GatewayError::OauthDenied { error, .. }) if error == "invalid_client_metadata"
        ));
    }

    #[test]
    fn the_happy_path_mints_an_audience_bound_token() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (verifier, challenge) = pkce();
        let code = approve_code(&as_, &client, uri, &challenge);
        let (grant, who) = as_
            .exchange_code(&code, &verifier, &resource(), uri, Some(CHAIN_END), T0)
            .expect("mints");
        assert_eq!(who, client, "issuance names the client");
        as_.validate_bearer(&grant.access_token, T0)
            .expect("valid on /mcp");
        // Wrong audience token (right key) is refused at /mcp.
        let wrong_aud = as_.sign_for_test(&json!({
            "aud": "https://elsewhere.example/mcp", "exp": 9_999_999_999i64
        }));
        assert!(as_.validate_bearer(&wrong_aud, T0).is_err());
    }

    #[test]
    fn dcr_codes_and_refresh_rotation_survive_real_as_reconstruction() {
        let state = Arc::new(MemoryAsStateStore::default());
        let uri = "http://127.0.0.1:9410/cb";
        let (verifier, challenge) = pkce();

        let first = restarted_server(state.clone());
        let client = register(&first, uri);
        let code = approve_code(&first, &client, uri, &challenge);
        drop(first);

        let second = restarted_server(state.clone());
        assert!(matches!(
            second.authorize(&AuthorizeRequest {
                client_id: client.clone(),
                redirect_uri: uri.to_owned(),
                response_type: "code".into(),
                code_challenge: Some(challenge),
                code_challenge_method: Some("S256".into()),
                resource: Some(resource()),
                scope: None,
                state: None,
            }),
            AuthorizeOutcome::Consent { .. }
        ));
        let (initial, _) = second
            .exchange_code(&code, &verifier, &resource(), uri, Some(CHAIN_END), T0)
            .expect("persisted code exchanges after restart");
        drop(second);

        let third = restarted_server(state);
        let (rotated, _) = third
            .refresh(&initial.refresh_token, Some(CHAIN_END), T0)
            .expect("persisted refresh rotates after restart");
        assert!(third
            .refresh(&initial.refresh_token, Some(CHAIN_END), T0)
            .is_err());
        assert!(third
            .refresh(&rotated.refresh_token, Some(CHAIN_END), T0)
            .is_err());
    }

    #[test]
    fn a_wrong_verifier_and_a_replay_both_fail() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (verifier, challenge) = pkce();
        let code = approve_code(&as_, &client, uri, &challenge);
        assert!(as_
            .exchange_code(
                &code,
                "wrong-verifier",
                &resource(),
                uri,
                Some(CHAIN_END),
                T0
            )
            .is_err());
        // The code was consumed on sight — the right verifier now finds nothing.
        assert!(as_
            .exchange_code(&code, &verifier, &resource(), uri, Some(CHAIN_END), T0)
            .is_err());
    }

    #[test]
    fn a_resource_mismatch_at_token_is_refused() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (verifier, challenge) = pkce();
        let code = approve_code(&as_, &client, uri, &challenge);
        assert!(matches!(
            as_.exchange_code(&code, &verifier, "https://elsewhere.example/mcp", uri, Some(CHAIN_END), T0),
            Err(GatewayError::OauthDenied { error, .. }) if error == "invalid_target"
        ));
    }

    #[test]
    fn refresh_rotates_and_a_reuse_cuts_the_family() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (verifier, challenge) = pkce();
        let code = approve_code(&as_, &client, uri, &challenge);
        let (grant, _) = as_
            .exchange_code(&code, &verifier, &resource(), uri, Some(CHAIN_END), T0)
            .unwrap();
        let (fresh, _) = as_
            .refresh(&grant.refresh_token, Some(CHAIN_END), T0)
            .expect("rotates");
        assert_ne!(fresh.refresh_token, grant.refresh_token);
        // Reuse of the consumed token → invalid_grant AND the successor dies.
        assert!(as_
            .refresh(&grant.refresh_token, Some(CHAIN_END), T0)
            .is_err());
        assert!(as_
            .refresh(&fresh.refresh_token, Some(CHAIN_END), T0)
            .is_err());
    }

    #[test]
    fn a_refresh_never_survives_its_authority() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (verifier, challenge) = pkce();
        let code = approve_code(&as_, &client, uri, &challenge);
        let (grant, _) = as_
            .exchange_code(&code, &verifier, &resource(), uri, Some(CHAIN_END), T0)
            .unwrap();
        // No live authority (ceiling None) → refused.
        assert!(as_.refresh(&grant.refresh_token, None, T0).is_err());
        // A ceiling already in the past → refused.
        let past = "2026-07-16T00:00:00Z";
        assert!(as_
            .refresh(&grant.refresh_token, Some(past), "2026-07-17T00:00:00Z")
            .is_err());
    }

    #[test]
    fn token_lifetime_is_capped_by_the_ceiling() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (verifier, challenge) = pkce();
        // Now sits 30 min before the ceiling; access ttl is 60 min.
        let now = "2026-08-08T23:30:00Z";
        let code = approve_code_at(&as_, &client, uri, &challenge, now);
        let (grant, _) = as_
            .exchange_code(&code, &verifier, &resource(), uri, Some(CHAIN_END), now)
            .unwrap();
        assert_eq!(
            grant.access_expires_secs,
            30 * 60,
            "capped at the chain end"
        );
    }

    #[test]
    fn an_unknown_client_never_redirects() {
        let as_ = server();
        let req = AuthorizeRequest {
            client_id: "client_never_registered".to_owned(),
            redirect_uri: "http://127.0.0.1:9410/cb".to_owned(),
            response_type: "code".to_owned(),
            code_challenge: Some("x".to_owned()),
            code_challenge_method: Some("S256".to_owned()),
            resource: Some(resource()),
            scope: None,
            state: None,
        };
        assert!(matches!(
            as_.authorize(&req),
            AuthorizeOutcome::HardError { .. }
        ));
    }

    #[test]
    fn a_plain_pkce_method_redirects_an_error() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let req = AuthorizeRequest {
            client_id: client,
            redirect_uri: uri.to_owned(),
            response_type: "code".to_owned(),
            code_challenge: Some("x".to_owned()),
            code_challenge_method: Some("plain".to_owned()),
            resource: Some(resource()),
            scope: None,
            state: Some("s".to_owned()),
        };
        match as_.authorize(&req) {
            AuthorizeOutcome::Redirect { location } => {
                assert!(location.contains("error=invalid_request"));
                assert!(location.contains("state=s"));
            }
            _ => panic!("a plain challenge must redirect an error"),
        }
    }

    #[test]
    fn a_well_formed_request_renders_a_dev_consent() {
        let as_ = server();
        let uri = "http://127.0.0.1:9410/cb";
        let client = register(&as_, uri);
        let (_verifier, challenge) = pkce();
        let req = AuthorizeRequest {
            client_id: client.clone(),
            redirect_uri: uri.to_owned(),
            response_type: "code".to_owned(),
            code_challenge: Some(challenge),
            code_challenge_method: Some("S256".to_owned()),
            resource: Some(resource()),
            scope: None,
            state: None,
        };
        match as_.authorize(&req) {
            AuthorizeOutcome::Consent { html } => {
                assert!(html.contains("DEV consent"));
                assert!(html.contains(&client), "names the client");
                assert!(html.contains(&resource()), "names the resource");
            }
            _ => panic!("a well-formed request renders the consent"),
        }
    }
}
