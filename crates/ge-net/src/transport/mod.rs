//! The contract between this core and a transport backend.
//!
//! A backend provides authenticated connection identity, message boundaries,
//! the delivery contract of each lane, congestion handling and transport
//! failures. This crate owns the application handshake, membership,
//! scheduling budgets and replication state. Neither layer implements a
//! second reliability mechanism over a reliable lane.
//!
//! A backend must declare what it supports in [`TransportCapabilities`]. It
//! must not present one reliable stream as the datagram lane, nor share one
//! ordered stream between the two reliable lanes.

use crate::channel::Lane;
use crate::identity::AttemptId;
use crate::session::client::ClientSession;
use crate::session::host::HostSession;
use crate::session::{DrainLimit, DrainReport, IngestError, Outbound};

/// What a backend can deliver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportCapabilities {
    /// Independent reliable ordered lanes per connection. Two are required.
    pub reliable_lanes: u8,
    /// Largest message on a reliable lane.
    pub max_reliable_message_bytes: u32,
    /// Usable datagram payload after the backend's framing and encryption
    /// overhead, or `None` if it has no unreliable lane.
    pub max_datagram_bytes: Option<u16>,
}

/// Why a transport connection ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransportFailure {
    /// The remote end closed the connection.
    RemoteClosed,
    /// The backend gave up waiting for the remote end.
    TimedOut,
    /// A reliable lane could not deliver; ordering can no longer be promised.
    ReliableDeliveryFailed,
    /// The remote end's identity could not be verified.
    AuthenticationFailed,
    /// The connection could not be established.
    ConnectFailed,
    /// The backend itself was shut down.
    LocalShutdown,
    /// Any other backend failure.
    Other,
}

/// Something a backend reports about an identified connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportEvent {
    /// The connection is established and the remote identity is verified.
    Connected { attempt: AttemptId },
    /// One complete message from one lane.
    Message {
        attempt: AttemptId,
        lane: Lane,
        bytes: Vec<u8>,
    },
    /// The connection ended. No further event follows for this attempt.
    Closed {
        attempt: AttemptId,
        failure: TransportFailure,
    },
}

/// What happened to one message handed to a backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendOutcome {
    /// The backend took ownership of the message.
    Sent,
    /// Congestion or flow control; the message was not taken. The session
    /// keeps it queued and stops draining that lane for now.
    Blocked,
    /// The connection is gone; the message was not taken.
    Closed,
}

/// A transport backend, polled by the application.
///
/// Implementations report attempts with strictly increasing [`AttemptId`]
/// values and never reuse one.
pub trait Transport {
    fn capabilities(&self) -> TransportCapabilities;

    /// The next event, if one is ready. Must not block.
    fn poll(&mut self) -> Option<TransportEvent>;

    /// Offer one message to a lane. Must not block.
    fn send(&mut self, attempt: AttemptId, lane: Lane, message: &[u8]) -> SendOutcome;

    /// Close a connection. Reliable messages already accepted by `send`
    /// should still be delivered, within the backend's own bounded deadline.
    /// Closing an unknown or already closed attempt is a no-op.
    fn close(&mut self, attempt: AttemptId);
}

/// Feed ready transport events into a host and hand its outbound work to
/// the transport. Call [`HostSession::advance`] first.
pub fn pump_host<T: Transport + ?Sized>(
    host: &mut HostSession,
    transport: &mut T,
    limit: DrainLimit,
) -> DrainReport {
    // Every polled event spends one credit, whether or not it reaches the
    // session, so stale messages and refused admissions cannot make one call
    // do unbounded work. Rechecking the session's own credit keeps `ingest`
    // from refusing a message after it has been taken from the transport.
    let mut credit = host.inbound_credit();
    while credit > 0 && host.inbound_credit() > 0 {
        let Some(event) = transport.poll() else {
            break;
        };
        credit -= 1;
        match event {
            TransportEvent::Connected { attempt } => {
                if host.admit(attempt).is_err() {
                    transport.close(attempt);
                }
            }
            TransportEvent::Message {
                attempt,
                lane,
                bytes,
            } => {
                // Credit was checked above, so the only refusal is a stale attempt.
                if let Err(IngestError::StaleAttempt) = host.ingest(attempt, lane, &bytes) {
                    transport.close(attempt);
                }
            }
            TransportEvent::Closed { attempt, failure } => host.transport_closed(attempt, failure),
        }
    }
    host.drain_outbound(limit, |outbound| deliver(transport, outbound))
}

/// Feed ready transport events into a client and hand its outbound work to
/// the transport. Call [`ClientSession::advance`] first.
pub fn pump_client<T: Transport + ?Sized>(
    client: &mut ClientSession,
    transport: &mut T,
    limit: DrainLimit,
) -> DrainReport {
    // Every polled event spends one credit, whether or not it reaches the
    // session, so stale messages and refused admissions cannot make one call
    // do unbounded work. Rechecking the session's own credit keeps `ingest`
    // from refusing a message after it has been taken from the transport.
    let mut credit = client.inbound_credit();
    while credit > 0 && client.inbound_credit() > 0 {
        let Some(event) = transport.poll() else {
            break;
        };
        credit -= 1;
        match event {
            TransportEvent::Connected { attempt } => {
                if client.transport_connected(attempt).is_err() {
                    transport.close(attempt);
                }
            }
            TransportEvent::Message {
                attempt,
                lane,
                bytes,
            } => {
                if let Err(IngestError::StaleAttempt) = client.ingest(attempt, lane, &bytes) {
                    transport.close(attempt);
                }
            }
            TransportEvent::Closed { attempt, failure } => {
                client.transport_closed(attempt, failure);
            }
        }
    }
    client.drain_outbound(limit, |outbound| deliver(transport, outbound))
}

fn deliver<T: Transport + ?Sized>(transport: &mut T, outbound: Outbound<'_>) -> SendOutcome {
    match outbound {
        Outbound::Message {
            attempt,
            lane,
            bytes,
        } => transport.send(attempt, lane, bytes),
        Outbound::Disconnect { attempt } => {
            transport.close(attempt);
            SendOutcome::Sent
        }
    }
}
