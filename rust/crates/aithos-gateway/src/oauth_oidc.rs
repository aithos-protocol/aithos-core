//! OLR-3 — OIDC ID Token validation for upstream account binding.
//!
//! Validates issuer, audience, signature (JWKS), nonce and expiry via
//! `openidconnect`. Verified claims never become Aithos authority: they only
//! feed the existing connector account-binding fields.

use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Duration;

use openidconnect::core::{CoreIdToken, CoreIdTokenVerifier, CoreJsonWebKeySet};
use openidconnect::{ClientId, IssuerUrl, Nonce};
use serde::Deserialize;
use zeroize::Zeroize;

use crate::{GatewayError, Result};

const JWKS_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_JWKS_BYTES: usize = 64 * 1024;

fn unavailable(reason: &str) -> GatewayError {
    GatewayError::UpstreamOauthUnavailable(reason.to_owned())
}

#[derive(Debug, Clone)]
pub struct OidcValidationRequest<'a> {
    pub id_token: &'a str,
    pub issuer: &'a str,
    pub audience: &'a str,
    pub jwks_uri: &'a str,
    pub nonce: OidcNoncePolicy<'a>,
}

#[derive(Debug, Clone, Copy)]
pub enum OidcNoncePolicy<'a> {
    /// Authorization responses must carry and match the nonce issued with the
    /// browser request.
    AuthorizationCode(&'a str),
    /// ID Tokens returned by a refresh grant are not tied to a new browser
    /// authorization request, so no nonce is required.
    Refresh,
}

/// Fetch a pinned JWKS document and verify one ID Token. Returns a flat map
/// of standard claims usable by account binding (`sub`, `email`, …).
pub async fn validate_id_token(
    req: OidcValidationRequest<'_>,
) -> Result<BTreeMap<String, serde_json::Value>> {
    if req.id_token.is_empty() {
        return Err(unavailable("OAuth token answer has no ID token"));
    }
    let jwks = fetch_jwks(req.jwks_uri).await?;
    let issuer =
        IssuerUrl::new(req.issuer.to_owned()).map_err(|_| unavailable("OIDC issuer is invalid"))?;
    let verifier = CoreIdTokenVerifier::new_public_client(
        ClientId::new(req.audience.to_owned()),
        issuer,
        jwks,
    );
    let id_token = CoreIdToken::from_str(req.id_token)
        .map_err(|_| unavailable("OIDC ID token is malformed"))?;
    let claims = match req.nonce {
        OidcNoncePolicy::AuthorizationCode(nonce) if !nonce.is_empty() => id_token
            .claims(&verifier, &Nonce::new(nonce.to_owned()))
            .map_err(|_| unavailable("OIDC ID token verification failed"))?,
        OidcNoncePolicy::AuthorizationCode(_) => {
            return Err(unavailable("OIDC nonce is missing"));
        }
        OidcNoncePolicy::Refresh => id_token
            .claims(&verifier, |_: Option<&Nonce>| Ok(()))
            .map_err(|_| unavailable("OIDC ID token verification failed"))?,
    };

    let mut map = BTreeMap::new();
    map.insert(
        "iss".into(),
        serde_json::Value::String(claims.issuer().as_str().to_owned()),
    );
    map.insert(
        "sub".into(),
        serde_json::Value::String(claims.subject().as_str().to_owned()),
    );
    if let Some(email) = claims.email() {
        map.insert(
            "email".into(),
            serde_json::Value::String(email.as_str().to_owned()),
        );
    }
    // Preserve audience as a JSON array for diagnostics (never logged by callers).
    let audiences: Vec<serde_json::Value> = claims
        .audiences()
        .iter()
        .map(|aud| serde_json::Value::String(aud.as_str().to_owned()))
        .collect();
    map.insert("aud".into(), serde_json::Value::Array(audiences));
    Ok(map)
}

async fn fetch_jwks(jwks_uri: &str) -> Result<CoreJsonWebKeySet> {
    let http = reqwest::Client::builder()
        .timeout(JWKS_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| unavailable("cannot build OIDC JWKS client"))?;
    let response = http.get(jwks_uri).send().await.map_err(|e| {
        unavailable(if e.is_timeout() {
            "OIDC JWKS endpoint timed out"
        } else if e.is_connect() {
            "OIDC JWKS endpoint is unreachable"
        } else {
            "OIDC JWKS fetch failed"
        })
    })?;
    if !response.status().is_success() {
        return Err(unavailable("OIDC JWKS endpoint refused the request"));
    }
    let bytes = bounded_bytes(response, MAX_JWKS_BYTES).await?;
    serde_json::from_slice(&bytes).map_err(|_| unavailable("OIDC JWKS document is malformed"))
}

async fn bounded_bytes(response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    use futures::StreamExt;
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| unavailable("OIDC JWKS endpoint is temporarily unavailable"))?;
        if out.len().saturating_add(chunk.len()) > limit {
            return Err(unavailable("OIDC JWKS document is malformed"));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Zeroize helper for ephemeral ID token strings held by callers.
pub fn zeroize_id_token(token: &mut String) {
    token.zeroize();
}

#[derive(Deserialize)]
struct _ForbidAccidentalInsecure;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_id_token_is_rejected() {
        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(validate_id_token(OidcValidationRequest {
                id_token: "",
                issuer: "http://127.0.0.1/issuer",
                audience: "client",
                jwks_uri: "http://127.0.0.1/jwks",
                nonce: OidcNoncePolicy::AuthorizationCode("n"),
            }))
            .expect_err("empty");
        assert!(format!("{err:?}").contains("no ID token"));
    }
}
