//! SNI peek — annexe B.1/B.4, contrat C2 (lot P6, jalon M2).
//!
//! The relay reads the **ClientHello of every inbound connection without
//! terminating TLS** (A3): it extracts `(sni, alpn)` and routes. Its own
//! tunnel name with ALPN `aithos-tunnel/1` is the only TLS it terminates
//! (the pod door, B.2); an active-tunnel hostname is piped from the first
//! byte (the public passthrough, B.3); everything else closes silently
//! (B.4 — no banner, nothing to enumerate).
//!
//! This module is **pure**: [`peek_client_hello`] decides from a byte
//! buffer only, so the committed `p5` vector replays it byte for byte. The
//! I/O bounds it enforces — ≤ 16 KiB read, ≤ 10 s to a complete hello
//! ([`PEEK_BOUND_BYTES`], [`HELLO_DEADLINE_SECS`]) — are **spec constants,
//! not runtime knobs**: no configuration can loosen the peek.
//!
//! Parsing is deliberately minimal and fail-closed. We read exactly the
//! record framing (RFC 8446 §5.1) and the ClientHello extension vector
//! (§4.1.2) needed to reach `server_name` (RFC 6066) and
//! `application_layer_protocol_negotiation` (RFC 7301). Any structural lie
//! is [`PeekDecision::NotTls`]; a hello that cannot fit the bound is
//! [`PeekDecision::TooLarge`]; missing bytes are
//! [`PeekDecision::Incomplete`] (read more, until the bound or the
//! deadline). We never decrypt, never terminate, never allocate on
//! attacker-chosen lengths beyond the bound.

/// The peek reads at most this many bytes before giving up (annexe B.4:
/// the ClientHello is bounded to 16 KiB). A hello that needs more is
/// [`PeekDecision::TooLarge`] — closed before routing.
pub const PEEK_BOUND_BYTES: usize = 16 * 1024;

/// The peek has at most this long to see a complete ClientHello (annexe
/// B.4: ≤ 10 s), otherwise the connection is closed dry. Enforced by the
/// I/O caller (a read deadline); this module stays pure.
pub const HELLO_DEADLINE_SECS: u64 = 10;

/// The ALPN protocol id of the pod tunnel door (annexe B.1/B.2). The relay
/// terminates TLS only for a hello that offers this and targets the
/// relay's own name.
pub const TUNNEL_ALPN: &[u8] = b"aithos-tunnel/1";

/// The routing decision from a (possibly partial) ClientHello. These map
/// one-to-one to the `p5` vector's `decision` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeekDecision {
    /// A ClientHello with a server_name: route by `sni` (lowercased) and
    /// the offered `alpn` protocols (order preserved, may be empty).
    Peeked { sni: String, alpn: Vec<String> },
    /// A valid-looking ClientHello with no server_name — silent close.
    NoSni,
    /// Not a TLS handshake record, or a structurally invalid one — silent
    /// close.
    NotTls,
    /// Not enough bytes yet: read more (up to the bound / deadline).
    Incomplete,
    /// A hello whose bytes cross the 16 KiB peek bound — closed before
    /// routing.
    TooLarge,
}

/// A tiny cursor that fails closed on any over-read.
struct Cur<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.p.checked_add(n)?;
        let s = self.b.get(self.p..end)?;
        self.p = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<usize> {
        self.take(1).map(|s| s[0] as usize)
    }
    fn u16(&mut self) -> Option<usize> {
        self.take(2).map(|s| ((s[0] as usize) << 8) | s[1] as usize)
    }
    fn done(&self) -> bool {
        self.p >= self.b.len()
    }
}

/// Reassemble the handshake bytes out of TLS records (type `0x16`,
/// `0x03 xx` version), bounded. Returns the full ClientHello handshake
/// message (`0x01` + 3-byte length + body) or a decision.
fn reassemble(data: &[u8]) -> Result<Vec<u8>, PeekDecision> {
    let mut hs: Vec<u8> = Vec::new();
    let mut off = 0usize;
    let mut hs_total: Option<usize> = None; // 4 + body length, once known

    loop {
        if let Some(total) = hs_total {
            if hs.len() >= total {
                hs.truncate(total);
                return Ok(hs);
            }
        }
        if off >= data.len() {
            return Err(PeekDecision::Incomplete);
        }
        // record: type(1) version(2) length(2) fragment
        if data[off] != 0x16 {
            return Err(PeekDecision::NotTls);
        }
        if off + 5 > data.len() {
            return Err(PeekDecision::Incomplete);
        }
        if data[off + 1] != 0x03 {
            return Err(PeekDecision::NotTls);
        }
        let rlen = ((data[off + 3] as usize) << 8) | data[off + 4] as usize;
        let fstart = off + 5;
        let fend = match fstart.checked_add(rlen) {
            Some(e) => e,
            None => return Err(PeekDecision::NotTls),
        };
        if fend > data.len() {
            return Err(PeekDecision::Incomplete);
        }
        hs.extend_from_slice(&data[fstart..fend]);
        off = fend;

        if hs_total.is_none() && hs.len() >= 4 {
            if hs[0] != 0x01 {
                return Err(PeekDecision::NotTls); // not a ClientHello
            }
            hs_total =
                Some(4 + (((hs[1] as usize) << 16) | ((hs[2] as usize) << 8) | hs[3] as usize));
        }
        // The peek bound covers the bytes we must read to complete the
        // hello: what we have consumed plus what the header says is still
        // outstanding. A complete valid hello past 16 KiB is TooLarge.
        let outstanding = hs_total.map(|t| t.saturating_sub(hs.len())).unwrap_or(0);
        if off.saturating_add(outstanding) > PEEK_BOUND_BYTES {
            return Err(PeekDecision::TooLarge);
        }
    }
}

/// Decide the route from a buffered ClientHello (annexe B.1/B.4). Pure:
/// same bytes in → same decision out, which is exactly what the `p5`
/// vector pins. `NotTls`/`NoSni`/`TooLarge` all mean "close silently"
/// upstream; `Incomplete` means "read more".
pub fn peek_client_hello(data: &[u8]) -> PeekDecision {
    // A first byte that is not a handshake record is not TLS — decide
    // immediately (a plain-HTTP `GET …` closes without waiting for a
    // deadline).
    if data.first().is_some_and(|&b| b != 0x16) {
        return PeekDecision::NotTls;
    }
    let hs = match reassemble(data) {
        Ok(hs) => hs,
        Err(decision) => return decision,
    };

    // Walk the ClientHello body (skip the 4-byte handshake header).
    let mut c = Cur { b: &hs[4..], p: 0 };
    let parsed = (|| -> Option<(Option<String>, Vec<String>)> {
        c.take(2)?; // legacy_version
        c.take(32)?; // random
        let sid = c.u8()?;
        c.take(sid)?; // legacy_session_id
        let cs = c.u16()?;
        c.take(cs)?; // cipher_suites
        let comp = c.u8()?;
        c.take(comp)?; // legacy_compression_methods
        let mut sni: Option<String> = None;
        let mut alpn: Vec<String> = Vec::new();
        if c.done() {
            return Some((None, alpn)); // no extensions → no SNI
        }
        let ext_len = c.u16()?;
        let ext_bytes = c.take(ext_len)?;
        let mut e = Cur { b: ext_bytes, p: 0 };
        while !e.done() {
            let etype = e.u16()?;
            let elen = e.u16()?;
            let ebody = e.take(elen)?;
            match etype {
                0x0000 => sni = parse_server_name(ebody)?,
                0x0010 => alpn = parse_alpn(ebody)?,
                _ => {}
            }
        }
        Some((sni, alpn))
    })();

    match parsed {
        Some((Some(sni), alpn)) => PeekDecision::Peeked { sni, alpn },
        Some((None, _)) => PeekDecision::NoSni,
        None => PeekDecision::NotTls,
    }
}

/// server_name extension (RFC 6066 §3): a `ServerNameList` of one entry,
/// type `host_name` (0). We read the first host_name and lowercase it
/// (B.4: matching is case-insensitive). A malformed body fails closed.
fn parse_server_name(body: &[u8]) -> Option<Option<String>> {
    let mut c = Cur { b: body, p: 0 };
    let list_len = c.u16()?;
    let list = c.take(list_len)?;
    let mut l = Cur { b: list, p: 0 };
    while !l.done() {
        let name_type = l.u8()?;
        let name_len = l.u16()?;
        let name = l.take(name_len)?;
        if name_type == 0 {
            let host = core::str::from_utf8(name).ok()?;
            // ASCII only on the wire (RFC 6066: US-ASCII, no trailing dot).
            if !host.is_ascii() || host.is_empty() {
                return None;
            }
            return Some(Some(host.to_ascii_lowercase()));
        }
    }
    Some(None)
}

/// ALPN extension (RFC 7301 §3.1): a `ProtocolNameList` of length-prefixed
/// names. Order is preserved. A malformed body fails closed.
fn parse_alpn(body: &[u8]) -> Option<Vec<String>> {
    let mut c = Cur { b: body, p: 0 };
    let list_len = c.u16()?;
    let list = c.take(list_len)?;
    let mut l = Cur { b: list, p: 0 };
    let mut out = Vec::new();
    while !l.done() {
        let plen = l.u8()?;
        let proto = l.take(plen)?;
        out.push(core::str::from_utf8(proto).ok()?.to_owned());
    }
    Some(out)
}

/// Does this hello target the pod tunnel door (annexe B.1/B.2)? The relay
/// terminates TLS only when the SNI is its own tunnel name and the ALPN
/// offers `aithos-tunnel/1`.
pub fn is_tunnel_door(sni: &str, alpn: &[String], relay_tunnel_name: &str) -> bool {
    sni.eq_ignore_ascii_case(relay_tunnel_name) && alpn.iter().any(|p| p.as_bytes() == TUNNEL_ALPN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_http_request_is_not_tls() {
        assert_eq!(
            peek_client_hello(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"),
            PeekDecision::NotTls
        );
    }

    #[test]
    fn an_empty_buffer_wants_more() {
        assert_eq!(peek_client_hello(b""), PeekDecision::Incomplete);
    }

    #[test]
    fn the_tunnel_door_needs_both_name_and_alpn() {
        let alpn = vec!["aithos-tunnel/1".to_owned()];
        assert!(is_tunnel_door("relay.aithos.fr", &alpn, "relay.aithos.fr"));
        assert!(is_tunnel_door("Relay.Aithos.FR", &alpn, "relay.aithos.fr"));
        // Right name, wrong ALPN → not the tunnel door (would be routed as
        // a public hostname, and close if unmapped).
        assert!(!is_tunnel_door(
            "relay.aithos.fr",
            &["h2".to_owned()],
            "relay.aithos.fr"
        ));
        // Tunnel ALPN but a public hostname → not the tunnel door.
        assert!(!is_tunnel_door(
            "demo.mcp.aithos.fr",
            &alpn,
            "relay.aithos.fr"
        ));
    }
}
