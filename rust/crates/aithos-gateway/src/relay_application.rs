//! G1c bridge from client-terminated public TLS streams to the one axum
//! application router also served by the direct listener.
//!
//! The bounded channel carries opaque IO objects, not requests. Axum owns
//! HTTP parsing and routing once TLS has terminated in [`PublicTlsAcceptor`].

use std::future::pending;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::public_tls::PublicTlsAcceptor;
use crate::{GatewayError, Result};

pub type RelayedTlsStream =
    tokio_rustls::server::TlsStream<tokio_util::compat::Compat<yamux::Stream>>;

const ROUTER_QUEUE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct RelayApplicationIngress {
    sender: mpsc::Sender<RelayedTlsStream>,
}

pub struct RelayApplicationListener {
    receiver: mpsc::Receiver<RelayedTlsStream>,
}

/// Create one bounded listener/ingress pair. `capacity` is explicit so tests
/// and deployments can prove that public connection pressure never becomes
/// an unbounded in-memory queue.
pub fn relay_application_channel(
    capacity: usize,
) -> Result<(RelayApplicationIngress, RelayApplicationListener)> {
    if capacity == 0 || capacity > 4_096 {
        return Err(GatewayError::RelayUnavailable(
            "relay_application_capacity_invalid".into(),
        ));
    }
    let (sender, receiver) = mpsc::channel(capacity);
    Ok((
        RelayApplicationIngress { sender },
        RelayApplicationListener { receiver },
    ))
}

impl RelayApplicationIngress {
    /// Complete public TLS inside the gateway and hand the resulting byte
    /// stream to axum. A full/closed router queue fails closed and bounded.
    pub async fn accept(&self, tls: &PublicTlsAcceptor, stream: yamux::Stream) -> Result<()> {
        let stream = tls.accept(stream).await?;
        tokio::time::timeout(ROUTER_QUEUE_TIMEOUT, self.sender.send(stream))
            .await
            .map_err(|_| GatewayError::RelayUnavailable("relay_application_busy".into()))?
            .map_err(|_| GatewayError::RelayUnavailable("relay_application_closed".into()))
    }
}

impl axum::serve::Listener for RelayApplicationListener {
    type Io = RelayedTlsStream;
    type Addr = ();

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.receiver.recv().await {
            Some(stream) => (stream, ()),
            None => pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_capacity_is_bounded_and_nonzero() {
        assert!(relay_application_channel(0).is_err());
        assert!(relay_application_channel(4_097).is_err());
        assert!(relay_application_channel(64).is_ok());
    }
}
