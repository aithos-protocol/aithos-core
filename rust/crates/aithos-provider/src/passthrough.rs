//! Relay passthrough — annexe B.1/B.3/B.4, contrat C2 (lot P6, jalon M2).
//!
//! One public door (`:443`) behind the NLB. The [`sni`](crate::sni) peek
//! decides, without terminating TLS, what each inbound connection is:
//!
//! - the relay's own tunnel name + ALPN `aithos-tunnel/1` → the pod door.
//!   This is the ONLY TLS the relay terminates (its own certificate, never
//!   a client's). After the B.2 registration line
//!   ([`crate::tunnel::verify_registration`]) the pod becomes the **yamux
//!   server** and the relay the **yamux client** (B.3), and the live
//!   session is pinned in the [`SessionRegistry`] by hostname.
//! - a hostname with an active tunnel → one yamux stream, the raw TCP
//!   bytes piped **from the first byte, ClientHello included** (B.3); the
//!   pod re-reads the SNI and terminates the public TLS itself — the
//!   private key stays client-side (A3).
//! - everything else → silent close (handled by the caller on a
//!   non-routable [`sni::PeekDecision`]).
//!
//! Doctrine held here: the relay moves bytes and never reads one
//! application byte of a passthrough stream; it holds no client key (it
//! authenticates the pod by its signed line); a replaced tunnel gets a
//! prompt GoAway (B.2); anti-flap and the pod-liveness cleanup are
//! bounded, fail-closed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures::future::poll_fn;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

use crate::control::{authorize_gateway, ControlStore, GatewayAuthzRefusal};
use crate::nonces::NonceStore;
use crate::relay::log_registration;
use crate::sni::{
    is_tunnel_door, peek_client_hello, PeekDecision, HELLO_DEADLINE_SECS, PEEK_BOUND_BYTES,
};
use crate::tunnel::{answer, verify_registration, Accepted, TunnelRefusal, MAX_REGISTRATION_BYTES};

/// Anti-flap of annexe B.2: at most 5 accepted registrations per rolling
/// minute on one hostname; the 6th answers `rate_limited`.
pub const MAX_REGISTRATIONS_PER_MIN: usize = 5;
const MINUTE_MS: i64 = 60_000;

/// yamux initial stream receive window (annexe B.3: 256 KiB).
pub const YAMUX_WINDOW_BYTES: usize = 256 * 1024;

/// A handle to a pod's live tunnel: open outbound yamux streams to it, or
/// GoAway it. The registry holds one of these per active hostname (B.2: a
/// hostname = one active tunnel).
pub struct TunnelSession {
    pub facts: Accepted,
    /// Monotonic id — distinguishes a session from a later one that
    /// replaced it on the same hostname (liveness cleanup never evicts a
    /// fresher tunnel).
    pub id: u64,
    open_tx: mpsc::UnboundedSender<oneshot::Sender<std::io::Result<yamux::Stream>>>,
    shutdown: Arc<Notify>,
}

impl TunnelSession {
    /// Open a fresh outbound yamux stream to the pod (one per inbound
    /// public connection). Fails closed if the tunnel driver is gone.
    pub async fn open_stream(&self) -> std::io::Result<yamux::Stream> {
        let (tx, rx) = oneshot::channel();
        self.open_tx
            .send(tx)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "tunnel closed"))?;
        rx.await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "tunnel closed"))?
    }

    /// Send this tunnel a GoAway and close its mux (B.2: the replaced pod
    /// does not wait for a timeout).
    pub fn goaway(&self) {
        self.shutdown.notify_waiters();
    }
}

/// The active-tunnel registry (annexe B.2). Holds live sessions by
/// hostname and the anti-flap counters. Replacing a hostname returns the
/// evicted session so the caller can GoAway it.
#[derive(Default)]
pub struct SessionRegistry {
    by_hostname: Mutex<HashMap<String, Arc<TunnelSession>>>,
    /// hostname → accept instants (ms) within the rolling minute.
    flap: Mutex<HashMap<String, Vec<i64>>>,
    next_id: AtomicU64,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Record a registration attempt for anti-flap accounting and report
    /// whether it is within budget (annexe B.2: ≥ 6 per minute → refuse).
    /// `now_ms` is injected. Called only after B.2 verification passes, so
    /// a bad signer cannot burn a hostname's budget.
    pub fn admit_registration(&self, hostname: &str, now_ms: i64) -> bool {
        let mut flap = self.flap.lock().expect("flap poisoned");
        let times = flap.entry(hostname.to_owned()).or_default();
        times.retain(|t| now_ms - *t < MINUTE_MS);
        if times.len() >= MAX_REGISTRATIONS_PER_MIN {
            return false;
        }
        times.push(now_ms);
        true
    }

    /// Pin a live session; returns the session it replaced, if any (the
    /// caller GoAways it). B.2: one hostname = one active tunnel.
    pub fn register(&self, session: Arc<TunnelSession>) -> Option<Arc<TunnelSession>> {
        self.by_hostname
            .lock()
            .expect("registry poisoned")
            .insert(session.facts.hostname.clone(), session)
    }

    /// Resolve a public-side SNI (already lowercased by the peek) to an
    /// active tunnel.
    pub fn resolve(&self, hostname: &str) -> Option<Arc<TunnelSession>> {
        self.by_hostname
            .lock()
            .expect("registry poisoned")
            .get(hostname)
            .cloned()
    }

    /// Remove a hostname only if the currently pinned session is the one
    /// with `id` (a fresher replacement is never evicted by an older
    /// driver's cleanup). Returns true if it removed the mapping.
    pub fn remove_if_current(&self, hostname: &str, id: u64) -> bool {
        let mut map = self.by_hostname.lock().expect("registry poisoned");
        if map.get(hostname).is_some_and(|s| s.id == id) {
            map.remove(hostname);
            true
        } else {
            false
        }
    }

    pub fn active_count(&self) -> usize {
        self.by_hostname.lock().expect("registry poisoned").len()
    }

    /// Snapshot the live sessions (P7b reconciliation sweep input).
    pub fn sessions(&self) -> Vec<Arc<TunnelSession>> {
        self.by_hostname
            .lock()
            .expect("registry poisoned")
            .values()
            .cloned()
            .collect()
    }
}

/// One reconciliation sweep (P7b — the B.4 « fermeture des tunnels
/// < 60 s »): re-resolve every ACTIVE tunnel's gateway against the control
/// seam and close (GoAway + unpin) what is no longer enrolled — suspended
/// binding or tenant, purged enrollment, or a mapping moved to another
/// (tenant, hostname). Arbitrage ② P7b: an UNANSWERABLE backend closes
/// NOTHING — a control outage never decapitates live traffic; new
/// registrations already refuse `unavailable`, and the < 60 s bound counts
/// from the moment the backend answers again. Returns the number of
/// tunnels closed.
///
/// `now_ms` is injected (wall clock in the binary, a test instant in the
/// harness) — with the [`crate::control::CachedControl`] seam the bound is
/// freshness TTL + sweep period, provable at the injected clock, never at
/// a sleep.
pub async fn reconcile_registry(
    registry: &SessionRegistry,
    control: &dyn ControlStore,
    now_ms: i64,
) -> usize {
    let mut closed = 0usize;
    for session in registry.sessions() {
        let facts = &session.facts;
        let verdict = authorize_gateway(control, &facts.gateway_pub, now_ms).await;
        let reason = match verdict {
            // Outage: keep serving (arbitrage ② — never close on an error).
            Err(GatewayAuthzRefusal::Unavailable) => continue,
            Err(GatewayAuthzRefusal::Suspended) => "suspended",
            Err(GatewayAuthzRefusal::MappingMismatch) => "unenrolled",
            Ok(binding)
                if binding.tenant != facts.tenant || binding.hostname != facts.hostname =>
            {
                "remapped"
            }
            Ok(_) => continue, // still enrolled exactly as pinned
        };
        session.goaway();
        if registry.remove_if_current(&facts.hostname, session.id) {
            closed += 1;
            // Discipline A.8/B.4: the hostname here is the VERIFIED
            // routing fact of an accepted registration — the allowed
            // register (event, hostname, reason class), never a byte more.
            tracing::info!(
                target: "aithos_relay::reconcile",
                "event=tunnel outcome=closed reason={reason} hostname={}",
                facts.hostname,
            );
        }
    }
    closed
}

/// Build the yamux client over a pod tunnel transport and pin it. `T` is
/// the post-registration byte stream (a TLS stream in the binary; a plain
/// duplex in tests). The relay is the yamux **client** (B.3); the pod
/// accepts streams. Returns the pinned session (already in the registry).
///
/// On the driver's exit (pod disconnect, GoAway) the hostname is unpinned
/// if this session is still the current one.
pub fn spawn_pod_tunnel<T>(
    registry: Arc<SessionRegistry>,
    facts: Accepted,
    transport: T,
) -> Arc<TunnelSession>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let id = registry.next_id();
    let shutdown = Arc::new(Notify::new());
    let (open_tx, open_rx) = mpsc::unbounded_channel();

    // Default config: the per-stream window is yamux's 256 KiB (annexe
    // B.3), and the connection window default already satisfies the
    // crate's `window ≥ max_num_streams × 256 KiB` invariant.
    let cfg = yamux::Config::default();
    // The relay is the yamux client; the pod (yamux server) accepts.
    let conn = yamux::Connection::new(transport.compat(), cfg, yamux::Mode::Client);

    let session = Arc::new(TunnelSession {
        facts: facts.clone(),
        id,
        open_tx,
        shutdown: shutdown.clone(),
    });
    // Pin BEFORE spawning the driver so a public connection racing in
    // finds it; register() returns the evicted session for GoAway.
    let replaced = registry.register(session.clone());
    if let Some(old) = replaced {
        old.goaway();
    }

    tokio::spawn(drive_tunnel(
        conn,
        open_rx,
        shutdown,
        registry,
        facts.hostname,
        id,
    ));
    session
}

enum Ev {
    Opened(Result<yamux::Stream, yamux::ConnectionError>),
    InboundClosedOrErr,
    InboundStream, // unexpected on the client side — drop it
}

/// Drive one pod's yamux connection: service outbound-stream opens, drain
/// (and drop) any inbound stream, and close promptly on GoAway or pod
/// disconnect. On exit, unpin the hostname if still current.
async fn drive_tunnel<T>(
    mut conn: yamux::Connection<tokio_util::compat::Compat<T>>,
    mut open_rx: mpsc::UnboundedReceiver<oneshot::Sender<std::io::Result<yamux::Stream>>>,
    shutdown: Arc<Notify>,
    registry: Arc<SessionRegistry>,
    hostname: String,
    id: u64,
) where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let mut pending: Option<oneshot::Sender<std::io::Result<yamux::Stream>>> = None;
    loop {
        tokio::select! {
            biased;
            // GoAway: close the mux and stop.
            _ = shutdown.notified() => break,
            // A new open request (only when not already servicing one).
            req = open_rx.recv(), if pending.is_none() => {
                match req {
                    Some(tx) => pending = Some(tx),
                    // All session handles dropped: nobody routes here anymore.
                    None => break,
                }
            }
            // Drive the connection.
            ev = poll_fn(|cx| {
                if pending.is_some() {
                    if let std::task::Poll::Ready(r) = conn.poll_new_outbound(cx) {
                        return std::task::Poll::Ready(Ev::Opened(r));
                    }
                }
                match conn.poll_next_inbound(cx) {
                    std::task::Poll::Ready(Some(Ok(_stream))) => std::task::Poll::Ready(Ev::InboundStream),
                    std::task::Poll::Ready(Some(Err(_))) => std::task::Poll::Ready(Ev::InboundClosedOrErr),
                    std::task::Poll::Ready(None) => std::task::Poll::Ready(Ev::InboundClosedOrErr),
                    std::task::Poll::Pending => std::task::Poll::Pending,
                }
            }) => {
                match ev {
                    Ev::Opened(r) => {
                        if let Some(tx) = pending.take() {
                            let _ = tx.send(r.map_err(yamux_to_io));
                        }
                    }
                    Ev::InboundStream => { /* client side: ignore, stream dropped */ }
                    Ev::InboundClosedOrErr => break,
                }
            }
        }
    }
    // Best-effort graceful close, then unpin if we are still the current
    // tunnel for this hostname (a replacement must not be evicted).
    let _ = poll_fn(|cx| conn.poll_close(cx)).await;
    registry.remove_if_current(&hostname, id);
}

fn yamux_to_io(e: yamux::ConnectionError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string())
}

/// A redacted flow-close log (discipline A.8/B.4): a closed reason class
/// and the peer IP only. NEVER the claimed SNI (unverified attacker input,
/// nothing to enumerate), never an application byte.
fn log_flow_close(reason: &'static str, peer: &str) {
    tracing::info!(
        target: "aithos_relay::flow",
        "event=flow outcome=closed reason={reason} peer={peer}"
    );
}

/// A redacted served-flow log: the VERIFIED (routable) hostname, the peer
/// IP and byte counts — the exhaustive allowed register of B.4. Never an
/// application byte (the bytes are piped, never inspected).
fn log_flow_served(hostname: &str, peer: &str, in_bytes: u64, out_bytes: u64) {
    tracing::info!(
        target: "aithos_relay::flow",
        "event=flow outcome=served hostname={hostname} peer={peer} bytes_in={in_bytes} bytes_out={out_bytes}"
    );
}

/// Read exactly one framed registration line (`≤ 4 KiB` + LF, annexe B.2)
/// from the post-TLS stream, byte by byte so no following yamux frame is
/// swallowed. Returns the raw bytes (LF included) or an I/O error.
pub async fn read_registration_line<S>(stream: &mut S) -> std::io::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut line = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            break; // EOF before LF
        }
        line.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
        if line.len() > MAX_REGISTRATION_BYTES {
            break; // oversized: verify_registration will refuse
        }
    }
    Ok(line)
}

/// Splice one public inbound connection into the pod's tunnel: open a
/// yamux stream, write the peeked ClientHello prefix (B.3: the pipe starts
/// at the first byte), then copy bidirectionally until either side closes.
/// The relay reads no application byte — [`tokio::io::copy_bidirectional`]
/// moves them without inspection. Returns bytes copied (in, out).
pub async fn splice_public<C>(
    mut client: C,
    prefix: &[u8],
    session: &TunnelSession,
) -> std::io::Result<(u64, u64)>
where
    C: AsyncRead + AsyncWrite + Unpin,
{
    let stream = session.open_stream().await?;
    let mut pod = stream.compat();
    if !prefix.is_empty() {
        pod.write_all(prefix).await?;
        pod.flush().await?;
    }
    tokio::io::copy_bidirectional(&mut client, &mut pod).await
}

/// The shared configuration of the public door (annexe B.1/B.4). One per
/// relay process; cloned cheaply per connection.
#[derive(Clone)]
pub struct RelayDoor {
    pub control: Arc<dyn ControlStore>,
    pub nonces: Arc<dyn NonceStore>,
    pub registry: Arc<SessionRegistry>,
    pub acceptor: tokio_rustls::TlsAcceptor,
    /// The relay's own SNI (the tunnel door), lowercased.
    pub tunnel_name: String,
    /// How long a connection has to present a complete ClientHello before
    /// it is closed dry (annexe B.4: ≤ 10 s). A field, not a const, only so
    /// tests can shorten it — production uses [`Self::hello_deadline`].
    pub hello_deadline: std::time::Duration,
}

impl RelayDoor {
    /// A door with the spec hello deadline ([`HELLO_DEADLINE_SECS`]).
    pub fn new(
        control: Arc<dyn ControlStore>,
        nonces: Arc<dyn NonceStore>,
        registry: Arc<SessionRegistry>,
        acceptor: tokio_rustls::TlsAcceptor,
        tunnel_name: String,
    ) -> Self {
        Self {
            control,
            nonces,
            registry,
            acceptor,
            tunnel_name,
            hello_deadline: std::time::Duration::from_secs(HELLO_DEADLINE_SECS),
        }
    }

    /// Serve one inbound connection end to end (annexe B.1/B.4): peek the
    /// ClientHello WITHOUT terminating, then route. `now_ms` is injected
    /// (the wall clock in the binary, a fixed instant in tests) — the same
    /// value B.2 verification consumes. Silent close on any non-routable
    /// decision: not one byte is emitted.
    pub async fn serve<S>(&self, mut stream: S, peer_ip: String, now_ms: i64) -> std::io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let mut buf = Vec::with_capacity(1024);
        let mut chunk = [0u8; 4096];
        let decision = loop {
            match peek_client_hello(&buf) {
                PeekDecision::Incomplete => {
                    if buf.len() > PEEK_BOUND_BYTES {
                        log_flow_close("too_large", &peer_ip);
                        return Ok(()); // over the peek bound: silent close
                    }
                    let read =
                        tokio::time::timeout(self.hello_deadline, stream.read(&mut chunk)).await;
                    match read {
                        Ok(Ok(0)) => {
                            log_flow_close("eof", &peer_ip);
                            return Ok(());
                        }
                        Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
                        Ok(Err(e)) => return Err(e),
                        Err(_) => {
                            log_flow_close("hello_timeout", &peer_ip);
                            return Ok(()); // hello deadline: silent close
                        }
                    }
                }
                other => break other,
            }
        };

        match decision {
            PeekDecision::Peeked { sni, alpn } => {
                if is_tunnel_door(&sni, &alpn, &self.tunnel_name) {
                    self.serve_tunnel_door(stream, buf, peer_ip, now_ms).await
                } else if let Some(session) = self.registry.resolve(&sni) {
                    // A public flow: the redacted register carries the
                    // VERIFIED (routable) hostname, IP and byte counts — never
                    // an application byte (B.4). copy inside splice_public
                    // moves bytes without inspection.
                    match splice_public(stream, &buf, &session).await {
                        Ok((in_bytes, out_bytes)) => {
                            log_flow_served(&sni, &peer_ip, in_bytes, out_bytes)
                        }
                        Err(_) => log_flow_close("pipe_error", &peer_ip),
                    }
                    Ok(())
                } else {
                    // Unknown hostname → silent close. The claimed SNI is
                    // unverified attacker input and is NEVER echoed (no
                    // enumeration oracle, B.4): only the reason class.
                    log_flow_close("no_tunnel", &peer_ip);
                    Ok(())
                }
            }
            PeekDecision::NoSni => {
                log_flow_close("no_sni", &peer_ip);
                Ok(())
            }
            PeekDecision::NotTls => {
                log_flow_close("not_tls", &peer_ip);
                Ok(())
            }
            PeekDecision::TooLarge => {
                log_flow_close("too_large", &peer_ip);
                Ok(())
            }
            PeekDecision::Incomplete => Ok(()),
        }
    }

    /// The tunnel door: terminate the relay's OWN TLS (ALPN
    /// `aithos-tunnel/1` enforced by the config), verify the B.2
    /// registration line, answer, and on accept pin the live yamux tunnel.
    async fn serve_tunnel_door<S>(
        &self,
        stream: S,
        peeked: Vec<u8>,
        peer_ip: String,
        now_ms: i64,
    ) -> std::io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let started = Instant::now();
        let rewound = Rewound::new(peeked, stream);
        let mut tls = match self.acceptor.accept(rewound).await {
            Ok(s) => s,
            // A handshake that fails (e.g. no matching ALPN) never reaches
            // a registration line.
            Err(_) => return Ok(()),
        };

        let line = read_registration_line(&mut tls).await?;
        let verdict =
            verify_registration(&line, self.control.as_ref(), self.nonces.as_ref(), now_ms).await;

        // Anti-flap (B.2): only an already-verified registration spends a
        // hostname's per-minute budget — a bad signer cannot burn it.
        let verdict = match verdict {
            Ok(accepted) if !self.registry.admit_registration(&accepted.hostname, now_ms) => {
                Err(TunnelRefusal::RateLimited)
            }
            other => other,
        };

        let mut answer_line = answer(&verdict);
        answer_line.push('\n');
        tls.write_all(answer_line.as_bytes()).await?;
        tls.flush().await?;

        log_registration(&verdict, &peer_ip, started.elapsed().as_millis());

        if let Ok(accepted) = verdict {
            spawn_pod_tunnel(self.registry.clone(), accepted, tls);
        }
        Ok(())
    }
}

/// A read adapter that replays already-consumed prefix bytes (the peeked
/// ClientHello) before yielding the live stream — so the TLS acceptor sees
/// the whole handshake even though the relay peeked it first. Writes pass
/// straight through.
pub struct Rewound<S> {
    prefix: Vec<u8>,
    pos: usize,
    inner: S,
}

impl<S> Rewound<S> {
    pub fn new(prefix: Vec<u8>, inner: S) -> Self {
        Self {
            prefix,
            pos: 0,
            inner,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Rewound<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.pos < self.prefix.len() {
            let remaining = self.prefix.len() - self.pos;
            let n = remaining.min(buf.remaining());
            let start = self.pos;
            buf.put_slice(&self.prefix[start..start + n]);
            self.pos += n;
            return std::task::Poll::Ready(Ok(()));
        }
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Rewound<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(host: &str) -> Accepted {
        Accepted {
            tenant: "acme".into(),
            hostname: host.into(),
            gateway_pub: "z6MkTest".into(),
        }
    }

    #[test]
    fn anti_flap_admits_five_then_refuses_the_sixth() {
        let reg = SessionRegistry::new();
        let h = "demo.mcp.aithos.fr";
        for i in 0..MAX_REGISTRATIONS_PER_MIN {
            assert!(
                reg.admit_registration(h, 1_000 + i as i64),
                "reg {i} within budget"
            );
        }
        // The 6th inside the minute is refused.
        assert!(!reg.admit_registration(h, 1_500));
        // A different hostname has its own budget.
        assert!(reg.admit_registration("beta.mcp.aithos.fr", 1_500));
        // After the minute rolls, the budget frees.
        assert!(reg.admit_registration(h, 1_000 + MINUTE_MS + 1));
    }

    #[test]
    fn remove_if_current_never_evicts_a_replacement() {
        let reg = SessionRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let s1 = Arc::new(TunnelSession {
            facts: facts("demo.mcp.aithos.fr"),
            id: 1,
            open_tx: tx.clone(),
            shutdown: Arc::new(Notify::new()),
        });
        assert!(reg.register(s1).is_none());
        // A newer session replaces it.
        let s2 = Arc::new(TunnelSession {
            facts: facts("demo.mcp.aithos.fr"),
            id: 2,
            open_tx: tx,
            shutdown: Arc::new(Notify::new()),
        });
        let replaced = reg.register(s2).expect("replaced s1");
        assert_eq!(replaced.id, 1);
        // The OLD driver's cleanup (id=1) must not evict the fresher s2.
        assert!(!reg.remove_if_current("demo.mcp.aithos.fr", 1));
        assert_eq!(reg.active_count(), 1);
        // The current driver's cleanup does.
        assert!(reg.remove_if_current("demo.mcp.aithos.fr", 2));
        assert_eq!(reg.active_count(), 0);
    }
}
