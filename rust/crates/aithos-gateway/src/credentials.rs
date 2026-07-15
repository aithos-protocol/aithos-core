//! Credential brokering: upstream secrets resolved from an enterprise
//! vault at the last possible moment, never persisted, never shown.
//!
//! Three objects, deliberately small (HANDOFF-GATEWAY-VAULT §3/§5):
//! - [`CredentialRef`] — the non-secret address of one secret (broker
//!   name, vault path, field). This is all the config, the router and
//!   the policy ever see or store.
//! - [`SecretValue`] — one resolved secret. No `Debug`, no `Display`,
//!   no `Serialize`, no `Clone`: the only read is [`SecretValue::expose`],
//!   at the upstream wire; the buffer zeroizes on drop. Any struct that
//!   embeds one loses those derives too — that is the point.
//! - [`CredentialBroker`] — the object-safe async seam to one vault.
//!   HashiCorp Vault KV v2 is the reference adapter; anything else
//!   (Infisical, cloud secret managers) plugs in behind the same trait
//!   without touching the router.
//!
//! Custody note (honest scope): `reqwest` may copy the header into its
//! own buffers, so "the gateway process holds zero secret bytes" is NOT
//! the claim. The claims are: the secret never crosses the gateway→agent
//! boundary, is never persisted anywhere, and is resolved per call —
//! after the mandate said yes and the act is already in the gamma.
//!
//! Redaction discipline: broker errors are BUILT from status codes and
//! fixed reasons — never from response bodies, header values or secret
//! material — so `Debug`/`Display` of any [`GatewayError`] stays safe.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use zeroize::{Zeroize, Zeroizing};

use crate::config::{BrokerAuthConfig, BrokerConfig, GatewayConfig};
use crate::{GatewayError, Result};

/// The non-secret reference to one brokered credential. Deserializes
/// straight from the config (`servers[].credential`), fail-closed on
/// unknown fields; deeper validation lives with the config validators.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialRef {
    /// Which configured broker resolves this reference.
    pub broker: String,
    /// Path under the broker's mount (KV v2 read: `<mount>/data/<path>`).
    pub path: String,
    /// The field of the secret payload carrying the value.
    pub field: String,
}

/// One resolved secret, alive for one relay. Exposed exactly once,
/// wire-side; zeroized on drop. Deliberately implements neither `Debug`
/// nor `Display` nor `Serialize` nor `Clone`.
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The only read. Call at the last possible moment, apply on the
    /// upstream wire, drop the value.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Object-safe async resolver: non-secret reference in, secret out,
/// fail-closed. Implementations must uphold the redaction discipline:
/// no error they return ever carries a secret, a header value or a raw
/// vault response.
pub trait CredentialBroker: Send + Sync {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>>;
}

// ------------------------------------------------- HashiCorp Vault KV v2

/// Bounded resolution time: a hanging vault refuses the relay, it never
/// holds it open.
const VAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// The reference adapter: HashiCorp Vault KV v2 over HTTP(S).
///
/// One `GET <address>/v1/<mount>/data/<path>` per resolution — per call,
/// no secret cache, so a KV rotation is honoured on the very next relay
/// without any config change. The vault access token is read from the
/// configured environment variable at resolution time and travels only
/// as the `X-Vault-Token` header of that one request.
pub struct VaultKv2Broker {
    client: reqwest::Client,
    address: String,
    mount: String,
    token_env: String,
}

impl VaultKv2Broker {
    pub fn new(address: &str, mount: &str, token_env: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(VAULT_TIMEOUT)
            .build()
            .map_err(|e| GatewayError::ConfigRejected(format!("vault http client: {e}")))?;
        Ok(Self {
            client,
            address: address.trim_end_matches('/').to_owned(),
            mount: mount.to_owned(),
            token_env: token_env.to_owned(),
        })
    }

    /// One strict KV v2 read. Every failure is summarised fail-closed —
    /// status class or fixed cause — and NEVER echoes the vault answer,
    /// a header or any secret material.
    async fn fetch(&self, reference: &CredentialRef) -> Result<SecretValue> {
        let token = Zeroizing::new(
            std::env::var(&self.token_env)
                .ok()
                .filter(|token| !token.trim().is_empty())
                .ok_or_else(|| {
                    GatewayError::CredentialUnavailable(format!(
                        "vault token environment variable `{}` is unset or empty",
                        self.token_env
                    ))
                })?,
        );
        let url = format!("{}/v1/{}/data/{}", self.address, self.mount, reference.path);
        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token.as_str())
            .send()
            .await
            .map_err(|e| {
                // Fixed transport summaries only: reqwest messages can
                // carry URLs and bodies, never relay them.
                GatewayError::CredentialUnavailable(
                    if e.is_timeout() {
                        "vault request timed out"
                    } else if e.is_connect() {
                        "vault is unreachable"
                    } else {
                        "vault transport failed"
                    }
                    .to_owned(),
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(GatewayError::CredentialUnavailable(format!(
                "vault answered status {}",
                status.as_u16()
            )));
        }
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|_| GatewayError::CredentialUnavailable("vault answer is not JSON".into()))?;
        // KV v2 wraps the secret as data.data.<field>; anything else —
        // wrong shape, absent field, non-string or empty value — refuses
        // without describing what WAS there.
        match body
            .pointer("/data/data")
            .and_then(serde_json::Value::as_object)
            .and_then(|fields| fields.get(&reference.field))
        {
            Some(serde_json::Value::String(value)) if !value.is_empty() => {
                Ok(SecretValue::new(value.clone()))
            }
            _ => Err(GatewayError::CredentialUnavailable(format!(
                "vault secret `{}` has no usable field `{}`",
                reference.path, reference.field
            ))),
        }
    }
}

impl CredentialBroker for VaultKv2Broker {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
        Box::pin(self.fetch(reference))
    }
}

/// Build every configured broker once, at startup. The map is what the
/// upstream construction consumes; an unresolvable kind fails closed
/// before anything listens.
pub fn build_brokers(cfg: &GatewayConfig) -> Result<BTreeMap<String, Arc<dyn CredentialBroker>>> {
    let mut brokers: BTreeMap<String, Arc<dyn CredentialBroker>> = BTreeMap::new();
    if let Some(declared) = &cfg.credential_brokers {
        for (name, broker) in declared {
            let BrokerConfig::VaultKv2 {
                address,
                mount,
                auth,
            } = broker;
            let BrokerAuthConfig::TokenEnv { env } = auth;
            brokers.insert(
                name.clone(),
                Arc::new(VaultKv2Broker::new(address, mount, env)?),
            );
        }
    }
    Ok(brokers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct CannedBroker;

    impl CredentialBroker for CannedBroker {
        fn resolve<'a>(
            &'a self,
            reference: &'a CredentialRef,
        ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
            Box::pin(async move { Ok(SecretValue::new(format!("secret-for-{}", reference.path))) })
        }
    }

    #[test]
    fn the_broker_seam_is_object_safe_and_the_secret_reads_back_once() {
        let broker: Arc<dyn CredentialBroker> = Arc::new(CannedBroker);
        let reference = CredentialRef {
            broker: "enterprise".into(),
            path: "aithos/mcp/github".into(),
            field: "token".into(),
        };
        let secret = futures::executor::block_on(broker.resolve(&reference)).unwrap();
        assert_eq!(secret.expose(), "secret-for-aithos/mcp/github");
    }

    // ------------------------------------------ redaction (fail-closed)
    //
    // The handoff's non-leak demand: `Debug` and every broker error must
    // contain neither the MCP token nor the vault token — even when the
    // vault answers with bodies stuffed with secrets. The fake vault
    // below returns adversarial payloads; the assertions run on the
    // REAL KV v2 client.

    const MCP_SENTINEL: &str = "github-mcp-sentinel-3f9c";
    const VAULT_TOKEN_SENTINEL: &str = "vault-access-sentinel-77aa";

    async fn serve_fake_vault(
        answer: (u16, serde_json::Value),
    ) -> (u16, Arc<std::sync::Mutex<Vec<Option<String>>>>) {
        use axum::{extract::State, http::HeaderMap, routing::get, Router};

        type Seen = Arc<std::sync::Mutex<Vec<Option<String>>>>;
        let seen: Seen = Arc::default();
        let state = (answer.0, answer.1.clone(), Arc::clone(&seen));
        let app = Router::new()
            .route(
                "/v1/{*path}",
                get(
                    |State((status, body, seen)): State<(u16, serde_json::Value, Seen)>,
                     headers: HeaderMap| async move {
                        seen.lock().unwrap().push(
                            headers
                                .get("x-vault-token")
                                .and_then(|value| value.to_str().ok())
                                .map(str::to_owned),
                        );
                        (
                            axum::http::StatusCode::from_u16(status).unwrap(),
                            axum::Json(body),
                        )
                    },
                ),
            )
            .with_state(state);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (port, seen)
    }

    fn reference() -> CredentialRef {
        CredentialRef {
            broker: "enterprise".into(),
            path: "aithos/mcp/github".into(),
            field: "token".into(),
        }
    }

    /// `Result::unwrap_err` needs `T: Debug` — and `SecretValue` has
    /// none, on purpose. The property that blocks the stdlib here is
    /// exactly the guarantee under test.
    fn expect_refusal(result: Result<SecretValue>) -> crate::GatewayError {
        match result {
            Err(err) => err,
            Ok(_) => panic!("the broker must refuse"),
        }
    }

    fn assert_redacted(err: &crate::GatewayError) {
        for shown in [format!("{err}"), format!("{err:?}")] {
            assert!(
                !shown.contains(MCP_SENTINEL) && !shown.contains(VAULT_TOKEN_SENTINEL),
                "a broker error leaked secret material: {shown}"
            );
        }
        assert!(
            matches!(err, crate::GatewayError::CredentialUnavailable(_)),
            "broker failures keep the stable credential_unavailable code"
        );
        assert_eq!(err.refusal_code(), "credential_unavailable");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_happy_path_resolves_and_sends_the_vault_token_wire_side_only() {
        let (port, seen) = serve_fake_vault((
            200,
            serde_json::json!({ "data": { "data": { "token": MCP_SENTINEL } } }),
        ))
        .await;
        std::env::set_var("AITHOS_TEST_VAULT_OK", VAULT_TOKEN_SENTINEL);
        let broker = VaultKv2Broker::new(
            &format!("http://127.0.0.1:{port}"),
            "secret",
            "AITHOS_TEST_VAULT_OK",
        )
        .unwrap();
        let secret = broker.resolve(&reference()).await.unwrap();
        assert_eq!(secret.expose(), MCP_SENTINEL);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            [Some(VAULT_TOKEN_SENTINEL.to_owned())],
            "the vault token rides the X-Vault-Token header of the one read"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failing_vault_status_is_summarised_without_the_body() {
        let (port, _seen) = serve_fake_vault((
            500,
            serde_json::json!({
                "errors": [format!("boom {MCP_SENTINEL} {VAULT_TOKEN_SENTINEL}")]
            }),
        ))
        .await;
        std::env::set_var("AITHOS_TEST_VAULT_500", VAULT_TOKEN_SENTINEL);
        let broker = VaultKv2Broker::new(
            &format!("http://127.0.0.1:{port}"),
            "secret",
            "AITHOS_TEST_VAULT_500",
        )
        .unwrap();
        let err = expect_refusal(broker.resolve(&reference()).await);
        assert!(format!("{err}").contains("status 500"));
        assert_redacted(&err);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_missing_field_never_lists_what_the_secret_does_hold() {
        let (port, _seen) = serve_fake_vault((
            200,
            serde_json::json!({
                "data": { "data": {
                    "token": MCP_SENTINEL,
                    "second": format!("also-{VAULT_TOKEN_SENTINEL}")
                } }
            }),
        ))
        .await;
        std::env::set_var("AITHOS_TEST_VAULT_FIELD", VAULT_TOKEN_SENTINEL);
        let broker = VaultKv2Broker::new(
            &format!("http://127.0.0.1:{port}"),
            "secret",
            "AITHOS_TEST_VAULT_FIELD",
        )
        .unwrap();
        let miss = CredentialRef {
            field: "missing".into(),
            ..reference()
        };
        let err = expect_refusal(broker.resolve(&miss).await);
        assert!(format!("{err}").contains("no usable field `missing`"));
        assert_redacted(&err);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_malformed_payload_fails_closed_and_redacted() {
        let (port, _seen) =
            serve_fake_vault((200, serde_json::json!({ "data": MCP_SENTINEL }))).await;
        std::env::set_var("AITHOS_TEST_VAULT_SHAPE", VAULT_TOKEN_SENTINEL);
        let broker = VaultKv2Broker::new(
            &format!("http://127.0.0.1:{port}"),
            "secret",
            "AITHOS_TEST_VAULT_SHAPE",
        )
        .unwrap();
        let err = expect_refusal(broker.resolve(&reference()).await);
        assert_redacted(&err);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unreachable_vault_and_an_unset_token_env_refuse_redacted() {
        // A port that nothing serves: connection refused.
        let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = dead.local_addr().unwrap().port();
        drop(dead);
        std::env::set_var("AITHOS_TEST_VAULT_DOWN", VAULT_TOKEN_SENTINEL);
        let broker = VaultKv2Broker::new(
            &format!("http://127.0.0.1:{port}"),
            "secret",
            "AITHOS_TEST_VAULT_DOWN",
        )
        .unwrap();
        let err = expect_refusal(broker.resolve(&reference()).await);
        assert!(format!("{err}").contains("unreachable"));
        assert_redacted(&err);

        let broker = VaultKv2Broker::new(
            "http://127.0.0.1:1",
            "secret",
            "AITHOS_TEST_VAULT_NEVER_SET",
        )
        .unwrap();
        let err = expect_refusal(broker.resolve(&reference()).await);
        assert!(format!("{err}").contains("AITHOS_TEST_VAULT_NEVER_SET"));
        assert_redacted(&err);
    }

    #[test]
    fn the_reference_is_the_only_debuggable_half() {
        // The reference is non-secret by construction: printing it is fine.
        let reference = CredentialRef {
            broker: "enterprise".into(),
            path: "aithos/mcp/github".into(),
            field: "token".into(),
        };
        let shown = format!("{reference:?}");
        assert!(shown.contains("aithos/mcp/github"));
        // `SecretValue` has no Debug/Display/Serialize/Clone impls — this
        // is enforced at compile time (uncommenting the line below must
        // not build):
        // let _ = format!("{:?}", SecretValue::new("x".into()));
    }
}
