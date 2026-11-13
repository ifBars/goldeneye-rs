//! The connecting end.

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};

use ge_net::{AttemptId, Lane, SendOutcome, Transport, TransportCapabilities, TransportEvent};

use crate::identity::Fingerprint;
use crate::worker::{Backend, Role};
use crate::{QuicConfig, TransportError};

/// A QUIC endpoint that connects to hosts by address and pinned fingerprint.
pub struct QuicClient {
    backend: Backend,
}

impl QuicClient {
    /// Bind an ephemeral IPv4 UDP socket.
    pub fn new(config: QuicConfig) -> Result<Self, TransportError> {
        Self::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)), config)
    }

    /// Bind a specific local address, for example an IPv6 one.
    pub fn bind(address: SocketAddr, config: QuicConfig) -> Result<Self, TransportError> {
        let backend = Backend::start(Role::Client { address }, config)?;
        Ok(Self { backend })
    }

    /// Start connecting to `address`, accepting only the host whose
    /// certificate has the `pinned` fingerprint.
    ///
    /// Returns at once. The attempt later reports
    /// [`TransportEvent::Connected`], or [`TransportEvent::Closed`] with
    /// `AuthenticationFailed` if the host could not prove the pinned
    /// identity. Attempt identifiers increase with every call.
    pub fn connect(&mut self, address: SocketAddr, pinned: Fingerprint) -> AttemptId {
        self.backend.connect(address, pinned)
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.backend.local_addr()
    }

    /// Datagrams this endpoint discarded because its event queue was full.
    pub fn dropped_datagrams(&self) -> u64 {
        self.backend.dropped_datagrams()
    }

    /// Flush and close every connection, then join the worker thread.
    /// Dropping the client does the same.
    pub fn shutdown(mut self) {
        self.backend.stop();
    }
}

impl fmt::Debug for QuicClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuicClient").finish_non_exhaustive()
    }
}

impl Transport for QuicClient {
    fn capabilities(&self) -> TransportCapabilities {
        self.backend.capabilities()
    }

    fn poll(&mut self) -> Option<TransportEvent> {
        self.backend.poll()
    }

    fn send(&mut self, attempt: AttemptId, lane: Lane, message: &[u8]) -> SendOutcome {
        self.backend.send(attempt, lane, message)
    }

    fn close(&mut self, attempt: AttemptId) {
        self.backend.close(attempt);
    }
}
