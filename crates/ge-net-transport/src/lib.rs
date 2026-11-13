//! QUIC transport backend for the `ge-net` session core.
//!
//! This crate is the one production backend. It implements
//! [`ge_net::Transport`] over QUIC (the `quinn` crate) and owns everything
//! the core deliberately does not: the UDP socket, the async runtime, the
//! worker thread and the host's TLS identity. The application still drives
//! it by polling; nothing here calls back into application code.
//!
//! How the backend meets the transport contract:
//!
//! | Contract | QUIC mechanism |
//! | --- | --- |
//! | Control lane, reliable ordered | One bidirectional stream, length-prefixed messages |
//! | Bulk lane, reliable ordered | A second, independent bidirectional stream |
//! | Datagram lane, unreliable | QUIC datagrams (RFC 9221), never a stream |
//! | Encryption and integrity | TLS 1.3 inside QUIC |
//! | Congestion and flow control | QUIC's own; a full send queue reports `Blocked` |
//! | Host identity | A pinned certificate fingerprint, see [`Fingerprint`] |
//!
//! Reliability, ordering, encryption and congestion control are QUIC's. This
//! crate adds none of its own.
//!
//! # Identity
//!
//! A host owns a self-signed certificate ([`HostIdentity`]). A client
//! connects with the SHA-256 [`Fingerprint`] of that certificate, received
//! out of band, and the handshake fails unless the host proves possession of
//! the matching private key. There is no certificate authority and no
//! "accept anything" mode. Clients are not authenticated individually: the
//! host knows each one only as its connection.
//!
//! This is direct connection only. NAT traversal, relays and matchmaking are
//! not implemented.

mod client;
mod host;
mod identity;
mod worker;

use std::fmt;
use std::time::Duration;

pub use client::QuicClient;
pub use host::QuicHost;
pub use identity::{Fingerprint, FingerprintParseError, HostIdentity};

/// A seed for session randomness from the TLS stack's secure random source.
///
/// Pass it to the session core, which never draws from a gameplay RNG:
///
/// ```no_run
/// let seed = ge_net_transport::secure_seed().expect("secure random source");
/// let mut entropy = move || seed;
/// # let _ = &mut entropy;
/// ```
pub fn secure_seed() -> Result<u64, TransportError> {
    let mut bytes = [0; 8];
    rustls::crypto::ring::default_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| TransportError::Worker("secure random source failed".into()))?;
    Ok(u64::from_be_bytes(bytes))
}

/// The ALPN protocol both ends must offer.
pub const ALPN: &[u8] = b"ge-net/1";

/// Tuning for one endpoint. Both ends should agree on the size limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuicConfig {
    /// Usable datagram payload this backend declares to the session core.
    ///
    /// It must fit QUIC's smallest guaranteed datagram, a little over a
    /// kilobyte once packet framing and the AEAD tag are subtracted. A
    /// connection whose path cannot carry it is refused at setup.
    pub max_datagram_bytes: u16,
    /// Largest message accepted on a reliable lane.
    pub max_reliable_message_bytes: u32,
    /// Silence after which QUIC declares the connection dead.
    pub idle_timeout: Duration,
    /// Interval of QUIC keep-alive packets.
    pub keep_alive: Duration,
    /// Time allowed for the handshake and lane setup.
    pub connect_timeout: Duration,
    /// Time a closing connection has, from the moment it is closed, to hand
    /// over and have acknowledged its queued reliable data. When it runs out
    /// the connection is closed anyway and the peer reports
    /// `ReliableDeliveryFailed`.
    pub close_flush_timeout: Duration,
    /// Messages queued per reliable lane before `send` reports `Blocked`.
    pub send_queue_messages: usize,
    /// Events queued for the application. Reliable lanes stop reading when
    /// it is full; datagrams are dropped. A host turns away a new connection
    /// that cannot be reported within `connect_timeout`.
    pub event_queue_messages: usize,
    /// Connections a host admits at once, counting handshakes in progress.
    /// Further connections are refused before any work is spent on them.
    /// Clients ignore it.
    pub max_connections: usize,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            max_datagram_bytes: 1_100,
            max_reliable_message_bytes: 64 * 1024,
            idle_timeout: Duration::from_secs(10),
            keep_alive: Duration::from_secs(2),
            connect_timeout: Duration::from_secs(5),
            close_flush_timeout: Duration::from_secs(2),
            send_queue_messages: 64,
            event_queue_messages: 4_096,
            max_connections: 64,
        }
    }
}

/// Why an endpoint could not be created.
#[derive(Debug)]
pub enum TransportError {
    /// The socket could not be bound.
    Io(std::io::Error),
    /// The TLS configuration or identity was rejected.
    Tls(String),
    /// The worker thread or its runtime could not start.
    Worker(String),
    /// A configuration value is out of range.
    InvalidConfig(&'static str),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "socket error: {error}"),
            Self::Tls(message) => write!(f, "TLS configuration rejected: {message}"),
            Self::Worker(message) => write!(f, "transport worker failed: {message}"),
            Self::InvalidConfig(name) => write!(f, "invalid transport configuration: {name}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Compiles the example in the crate README.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
