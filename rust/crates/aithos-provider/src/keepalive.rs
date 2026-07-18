//! TCP keepalive on the pod tunnel socket — the M2 backstop of the B.3
//! redline (gravée 2026-07-18) : « M2 = détection de déconnexion (EOF) +
//! TCP keepalive ; PING actif applicatif (pod FIGÉ, TCP vivant mais
//! muet) = draft.2 via le canal de contrôle riche que B.6 réserve. »
//!
//! What this buys, and what it does not: a pod that dies without a FIN
//! (crash, NAT expiry, cable pull) is detected by the kernel probes in
//! ~idle + interval × retries (≈ 60 s here) and the socket errors → the
//! relay's EOF path unpins the hostname. It also KEEPS the NAT mapping
//! of the pod's outbound tunnel alive (a silent tunnel would otherwise
//! expire in middleboxes). A FROZEN pod — TCP alive but the process
//! mute — is NOT detected here: that is the applicative PING of draft.2
//! (the `@wip @draft2` scenario of `relay-passthrough.feature`).
//!
//! Both sides set it: the relay on the accepted socket (the tunnel-door
//! socket is the target; on public flows it is inert pipe hygiene), the
//! pod-stub on its outbound socket. yamux stays `Config::default()` — no
//! mux-level knob is touched (G1 must not couple to a legacy mux).

use std::time::Duration;

/// Idle time before the first probe (the redline's « idle court »).
pub const TUNNEL_KEEPALIVE_IDLE_SECS: u64 = 30;
/// Interval between unanswered probes.
pub const TUNNEL_KEEPALIVE_INTERVAL_SECS: u64 = 10;
/// Unanswered probes before the kernel declares the peer dead.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub const TUNNEL_KEEPALIVE_RETRIES: u32 = 3;

/// Enable SO_KEEPALIVE (short idle) on a tokio `TcpStream`. Failure is
/// reported, never fatal: keepalive is liveness hygiene, not a security
/// gate — the B.2/B.4 verification path does not depend on it.
pub fn enable_tunnel_keepalive(stream: &tokio::net::TcpStream) -> std::io::Result<()> {
    let sock = socket2::SockRef::from(stream);
    #[allow(unused_mut)]
    let mut ka = socket2::TcpKeepalive::new()
        .with_time(Duration::from_secs(TUNNEL_KEEPALIVE_IDLE_SECS))
        .with_interval(Duration::from_secs(TUNNEL_KEEPALIVE_INTERVAL_SECS));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        ka = ka.with_retries(TUNNEL_KEEPALIVE_RETRIES);
    }
    sock.set_tcp_keepalive(&ka)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The helper really flips SO_KEEPALIVE on a live socket — read back
    /// through the kernel, not assumed.
    #[test]
    fn keepalive_is_set_and_readable_on_a_real_socket() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        rt.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (client, _server) = tokio::join!(
                async { tokio::net::TcpStream::connect(addr).await.unwrap() },
                async { listener.accept().await.unwrap().0 },
            );
            let sock = socket2::SockRef::from(&client);
            assert!(!sock.keepalive().unwrap(), "off by default");
            enable_tunnel_keepalive(&client).unwrap();
            assert!(sock.keepalive().unwrap(), "SO_KEEPALIVE is on");
            #[cfg(target_os = "linux")]
            {
                assert_eq!(
                    sock.keepalive_time().unwrap(),
                    Duration::from_secs(TUNNEL_KEEPALIVE_IDLE_SECS)
                );
                assert_eq!(
                    sock.keepalive_interval().unwrap(),
                    Duration::from_secs(TUNNEL_KEEPALIVE_INTERVAL_SECS)
                );
                assert_eq!(sock.keepalive_retries().unwrap(), TUNNEL_KEEPALIVE_RETRIES);
            }
        });
    }
}
