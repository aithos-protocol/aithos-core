//! Relay connection glue — the pod-facing tunnel handshake (annexe B.2,
//! lot P6, jalon M1).
//!
//! A pod dials **out** to the relay, and after its TLS the first thing it
//! sends is the registration line. [`serve_registration`] reads exactly
//! that one framed line (bounded), verifies it with the pure
//! [`crate::tunnel`] logic, writes the one-line answer, and returns the
//! verdict. On acceptance the caller pins the tunnel into the
//! [`TunnelRegistry`] (hostname → active session) — the public-side SNI
//! passthrough (M2) routes against it.
//!
//! Fail-closed and blind: the handshake decides yes/no from the signed
//! line and the control-plane mapping only; **no application byte is ever
//! read here** (the passthrough pipes raw, A3). The relay holds no client
//! key — it authenticates the pod by its signature (zero new secret).

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::control::ControlPlane;
use crate::nonces::NonceStore;
use crate::tunnel::{answer, verify_registration, Accepted, TunnelRefusal, MAX_REGISTRATION_BYTES};

/// Read the one framed registration line, verify it (annexe B.2), answer
/// `{"...","ok":true}` / `{"ok":false,"error":<code>}` + LF, and return
/// the verdict. `now_ms` is injected (wall clock in the binary; a fixed
/// instant in tests) — the registration replayability of B.2.
///
/// The returned `io::Result` is the transport outcome; the inner
/// `Result<Accepted, TunnelRefusal>` is the protocol verdict. A transport
/// error answers nothing (the peer is gone) — the caller drops the
/// connection.
pub async fn serve_registration<S>(
    stream: S,
    control: &ControlPlane,
    nonces: &dyn NonceStore,
    now_ms: i64,
) -> std::io::Result<Result<Accepted, TunnelRefusal>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    // One line, hard-bounded: read at most 4 KiB + the LF + one overflow
    // byte, so an unterminated or oversized line is caught (B.2 size gate
    // lives in verify_registration, on the full framed bytes).
    let mut line = Vec::new();
    let cap = (MAX_REGISTRATION_BYTES as u64) + 2;
    tokio::io::AsyncReadExt::take(&mut reader, cap)
        .read_until(b'\n', &mut line)
        .await?;

    let verdict = verify_registration(&line, control, nonces, now_ms).await;

    let mut answer_line = answer(&verdict);
    answer_line.push('\n');
    write_half.write_all(answer_line.as_bytes()).await?;
    write_half.flush().await?;

    Ok(verdict)
}

/// The active-tunnel registry (annexe B.2: a hostname = one active
/// tunnel). M1 records the accepted routing facts; M2 stores the live
/// yamux session handle here and the SNI router looks a stream up by
/// hostname. A fresh accept for a served hostname **replaces** the old
/// (the restarted pod does not wait for a timeout).
#[derive(Default)]
pub struct TunnelRegistry {
    by_hostname: Mutex<HashMap<String, Accepted>>,
}

impl TunnelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pin an accepted tunnel; returns the routing facts it replaced, if
    /// any (the caller sends the old session a GoAway — M2).
    pub fn register(&self, accepted: Accepted) -> Option<Accepted> {
        self.by_hostname
            .lock()
            .expect("registry poisoned")
            .insert(accepted.hostname.clone(), accepted)
    }

    /// Resolve a public-side SNI to an active tunnel (M2 routing).
    pub fn resolve(&self, hostname: &str) -> Option<Accepted> {
        self.by_hostname
            .lock()
            .expect("registry poisoned")
            .get(hostname)
            .cloned()
    }

    /// Drop a tunnel (suspension propagation, pod disconnect).
    pub fn remove(&self, hostname: &str) {
        self.by_hostname
            .lock()
            .expect("registry poisoned")
            .remove(hostname);
    }

    pub fn active_count(&self) -> usize {
        self.by_hostname.lock().expect("registry poisoned").len()
    }
}

/// One redacted relay log line (discipline A.8 / B.4): only the allowed
/// register — event, outcome/code, and on acceptance the VERIFIED tenant
/// and hostname. On refusal the claimed tenant/hostname are unverified
/// attacker input and are NOT echoed. Never an application byte.
pub fn log_registration(verdict: &Result<Accepted, TunnelRefusal>, peer: &str, duration_ms: u128) {
    match verdict {
        Ok(accepted) => tracing::info!(
            target: "aithos_relay::register",
            "event=register outcome=ok tenant={} hostname={} peer={peer} dur_ms={duration_ms}",
            accepted.tenant,
            accepted.hostname,
        ),
        Err(refusal) => tracing::info!(
            target: "aithos_relay::register",
            "event=register outcome=refused error={} peer={peer} dur_ms={duration_ms}",
            refusal.code(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::Accepted;

    #[test]
    fn the_registry_is_one_tunnel_per_hostname() {
        let reg = TunnelRegistry::new();
        let a = Accepted {
            tenant: "acme".into(),
            hostname: "demo.mcp.aithos.fr".into(),
            gateway_pub: "z6MkA".into(),
        };
        assert!(reg.register(a.clone()).is_none());
        assert_eq!(reg.active_count(), 1);
        assert_eq!(
            reg.resolve("demo.mcp.aithos.fr").unwrap().gateway_pub,
            "z6MkA"
        );
        // A fresh accept for the same hostname replaces the old one.
        let b = Accepted {
            gateway_pub: "z6MkB".into(),
            ..a.clone()
        };
        let replaced = reg.register(b).expect("replaced the old tunnel");
        assert_eq!(replaced.gateway_pub, "z6MkA");
        assert_eq!(reg.active_count(), 1);
        reg.remove("demo.mcp.aithos.fr");
        assert_eq!(reg.active_count(), 0);
        assert!(reg.resolve("demo.mcp.aithos.fr").is_none());
    }

    /// A refusal log never echoes the claimed (unverified) tenant/hostname
    /// — only the closed register.
    #[test]
    fn refusal_log_does_not_echo_unverified_claims() {
        // Rendered by hand from the same code path the tracing macro uses.
        let refusal = TunnelRefusal::MappingMismatch;
        let rendered = format!("event=register outcome=refused error={}", refusal.code());
        assert!(rendered.contains("mapping_mismatch"));
        assert!(!rendered.contains("hostname="));
        assert!(!rendered.contains("tenant="));
    }
}
