//! The listening end.

use std::fmt;
use std::net::SocketAddr;

use ge_net::{AttemptId, Lane, SendOutcome, Transport, TransportCapabilities, TransportEvent};

use crate::identity::{Fingerprint, HostIdentity};
use crate::worker::{Backend, Role};
use crate::{QuicConfig, TransportError};

/// A QUIC endpoint that accepts connections under one pinned identity.
///
/// Every accepted connection completed the TLS 1.3 handshake with this
/// host's private key and opened both reliable lanes before it is reported
/// as [`TransportEvent::Connected`]. Attempt identifiers increase in the
/// order connections are reported.
pub struct QuicHost {
    backend: Backend,
    fingerprint: Fingerprint,
}

impl QuicHost {
    /// Bind a UDP socket and start accepting. Port zero asks the operating
    /// system for a free port; read it back with [`QuicHost::local_addr`].
    pub fn bind(
        address: SocketAddr,
        identity: HostIdentity,
        config: QuicConfig,
    ) -> Result<Self, TransportError> {
        let fingerprint = identity.fingerprint();
        let backend = Backend::start(Role::Host { address, identity }, config)?;
        Ok(Self {
            backend,
            fingerprint,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.backend.local_addr()
    }

    /// What clients must pin to reach this host.
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// Datagrams this endpoint discarded because its event queue was full.
    pub fn dropped_datagrams(&self) -> u64 {
        self.backend.dropped_datagrams()
    }

    /// Flush and close every connection, then join the worker thread.
    /// Dropping the host does the same.
    pub fn shutdown(mut self) {
        self.backend.stop();
    }
}

impl fmt::Debug for QuicHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuicHost")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl Transport for QuicHost {
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
