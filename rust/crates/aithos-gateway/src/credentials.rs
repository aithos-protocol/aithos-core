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

use std::future::Future;
use std::pin::Pin;

use serde::Deserialize;
use zeroize::Zeroize;

use crate::Result;

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
            Box::pin(async move {
                Ok(SecretValue::new(format!("secret-for-{}", reference.path)))
            })
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
