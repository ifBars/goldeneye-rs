//! Session lifecycle shared by the host and client state machines.
//!
//! | Phase | Entered when | Accepted input | Retained state |
//! | --- | --- | --- | --- |
//! | `Idle` | A client exists with no attempt | `begin` | Configuration |
//! | `Connecting` | A client began an attempt | Transport connected or closed | Attempt identity |
//! | `Negotiating` | The transport connected (client) or was admitted (host) | `Hello`, `Accept`, `Reject`, probes, `Close` | Link sequences |
//! | `Synchronizing` | The host accepted the descriptor | Snapshot request, transfer, applied acknowledgement, catch-up updates, probes, `Close` | Membership, slots, epoch, transfer progress, baseline |
//! | `Active` | The client applied a snapshot at or after the accepted tick | Commands, updates, acknowledgements, recovery, probes, `Close` | Everything above plus command queues |
//! | `Closing` | A local close is waiting to hand its last message to the transport | Nothing except `Close` | The final control message only |
//! | `Closed` | The attempt ended for any reason | Nothing; its events are stale | Diagnostics |
//!
//! A transport connection alone never yields an active member: negotiation,
//! a full snapshot, the application's applied acknowledgement and catch-up
//! all come first.

pub mod client;
pub mod host;
mod link;

use std::fmt;

use crate::channel::Lane;
use crate::identity::{AttemptId, EntityId, PlayerSlot};
use crate::protocol::DecodeError;
use crate::protocol::message::{MessageKind, RejectReason};
use crate::replication::DeltaError;
use crate::transport::TransportFailure;

/// Where a connection is in its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Idle,
    Connecting,
    Negotiating,
    Synchronizing,
    Active,
    Closing,
    Closed,
}

/// A contract the remote peer or the transport broke. Always terminal for
/// the connection, never for the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// The bytes are not a valid version-one message.
    Decode(DecodeError),
    /// A message arrived on a lane its channel does not use.
    LaneMismatch,
    /// A reliable message carried the wrong session epoch.
    EpochMismatch,
    /// A reliable lane skipped, repeated or reordered a message.
    ReliableOrder {
        expected: u32,
        received: u32,
    },
    FragmentOnReliableChannel,
    TooManyFragments(u8),
    /// Fragments of one sequence disagree about the message.
    FragmentMismatch,
    AssembledMessageTooLarge,
    /// A valid kind that this side or phase must not receive.
    UnexpectedMessage(MessageKind),
    /// More unknown optional messages than the budget allows.
    TooManySkippedOptional,
    /// A command for a slot the connection does not own.
    UnownedSlot(PlayerSlot),
    /// A slot's command ticks went backwards.
    CommandTickRegression,
    /// The host acknowledged a command the client never sent.
    CommandAckAhead,
    /// A snapshot transfer's framing is inconsistent.
    Transfer(&'static str),
    /// A snapshot or delta is internally inconsistent.
    Delta(DeltaError),
    /// A snapshot carried an entity generation that was already retired.
    StaleGeneration(EntityId),
    /// An acceptance that assigns nothing usable.
    InvalidAccept,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "malformed message: {error}"),
            Self::LaneMismatch => write!(f, "message arrived on the wrong lane"),
            Self::EpochMismatch => write!(f, "reliable message from another epoch"),
            Self::ReliableOrder { expected, received } => write!(
                f,
                "reliable lane delivered sequence {received}, expected {expected}"
            ),
            Self::FragmentOnReliableChannel => write!(f, "fragment on a reliable channel"),
            Self::TooManyFragments(count) => write!(f, "{count} fragments exceed the limit"),
            Self::FragmentMismatch => write!(f, "fragments disagree about their message"),
            Self::AssembledMessageTooLarge => write!(f, "reassembled message too large"),
            Self::UnexpectedMessage(kind) => write!(f, "unexpected {kind:?} message"),
            Self::TooManySkippedOptional => write!(f, "too many unknown optional messages"),
            Self::UnownedSlot(slot) => write!(f, "command for unowned {slot}"),
            Self::CommandTickRegression => write!(f, "command ticks went backwards"),
            Self::CommandAckAhead => write!(f, "acknowledgement for an unsent command"),
            Self::Transfer(what) => write!(f, "inconsistent snapshot transfer: {what}"),
            Self::Delta(error) => write!(f, "{error}"),
            Self::StaleGeneration(id) => write!(f, "{id} carries a retired generation"),
            Self::InvalidAccept => write!(f, "acceptance assigns nothing usable"),
        }
    }
}

/// Why a connection ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The local application asked for a graceful close.
    Requested,
    /// The remote end sent `Close` with this raw code.
    RemoteClosed(u8),
    /// The host refused the `Hello`.
    Rejected(RejectReason),
    Violation(Violation),
    HandshakeTimeout,
    SyncTimeout,
    IdleTimeout,
    /// A reliable message did not fit the outbound queue budget.
    QueueOverflow,
    Transport(TransportFailure),
    /// The local application cancelled the attempt without a goodbye.
    Cancelled,
    /// The local session is shutting down.
    Shutdown,
}

impl CloseReason {
    /// The code sent in a `Close` message. It names a category only.
    pub const fn wire_code(&self) -> u8 {
        match self {
            Self::Requested => 1,
            Self::Shutdown => 2,
            Self::Violation(_) => 3,
            Self::HandshakeTimeout => 4,
            Self::SyncTimeout => 5,
            Self::IdleTimeout => 6,
            Self::QueueOverflow => 7,
            Self::Rejected(_) => 8,
            Self::RemoteClosed(_) | Self::Transport(_) | Self::Cancelled => 0,
        }
    }
}

impl fmt::Display for CloseReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requested => write!(f, "closed by request"),
            Self::RemoteClosed(code) => write!(f, "remote closed with code {code}"),
            Self::Rejected(reason) => write!(f, "rejected: {reason:?}"),
            Self::Violation(violation) => write!(f, "protocol violation: {violation}"),
            Self::HandshakeTimeout => write!(f, "handshake timed out"),
            Self::SyncTimeout => write!(f, "synchronization timed out"),
            Self::IdleTimeout => write!(f, "connection went silent"),
            Self::QueueOverflow => write!(f, "reliable outbound queue overflowed"),
            Self::Transport(failure) => write!(f, "transport failure: {failure:?}"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Shutdown => write!(f, "session shut down"),
        }
    }
}

/// Why `ingest` refused a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestError {
    /// The attempt is not live: never admitted, closed or cancelled. The
    /// message was discarded.
    StaleAttempt,
    /// The per-update inbound budget is spent. The message was not consumed;
    /// offer it again after `advance`.
    InboundBudgetExhausted,
    /// Too many undrained events. The message was not consumed; offer it
    /// again after `poll_event`.
    EventQueueFull,
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleAttempt => write!(f, "attempt is not live"),
            Self::InboundBudgetExhausted => write!(f, "inbound budget exhausted for this update"),
            Self::EventQueueFull => write!(f, "event queue is full"),
        }
    }
}

impl std::error::Error for IngestError {}

/// What `ingest` did with a message it consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ingested {
    /// Processed, or stored as part of a fragmented message.
    Accepted,
    /// Discarded for a reason the unreliable contract expects.
    Dropped(crate::diagnostics::DropReason),
    /// The connection was closed as a result.
    Closed(CloseReason),
}

/// A caller-supplied time went backwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeRegression;

impl fmt::Display for TimeRegression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "network time went backwards")
    }
}

impl std::error::Error for TimeRegression {}

/// An operation that the current phase or attempt does not permit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    /// The operation is not valid in this phase.
    WrongPhase(Phase),
    /// The attempt is not the live one.
    StaleAttempt,
    /// Attempt identifiers must be strictly increasing.
    AttemptNotIncreasing,
    /// The handshake or peer budget is exhausted.
    TooManyAttempts,
    /// The snapshot is not one awaiting acknowledgement.
    UnknownSnapshot,
    /// A reliable message did not fit the outbound queue.
    QueueOverflow,
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPhase(phase) => write!(f, "operation not valid while {phase:?}"),
            Self::StaleAttempt => write!(f, "attempt is not live"),
            Self::AttemptNotIncreasing => write!(f, "attempt identifiers must increase"),
            Self::TooManyAttempts => write!(f, "attempt budget exhausted"),
            Self::UnknownSnapshot => write!(f, "snapshot is not awaiting acknowledgement"),
            Self::QueueOverflow => write!(f, "reliable outbound queue overflowed"),
        }
    }
}

impl std::error::Error for StateError {}

/// One unit of outbound work for the transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outbound<'a> {
    /// One complete envelope for one lane.
    Message {
        attempt: AttemptId,
        lane: Lane,
        bytes: &'a [u8],
    },
    /// Close this connection. Its final reliable messages precede this.
    Disconnect { attempt: AttemptId },
}

/// Caller limits for one `drain_outbound` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrainLimit {
    pub max_messages: usize,
    pub max_bytes: usize,
}

impl DrainLimit {
    pub const UNLIMITED: Self = Self {
        max_messages: usize::MAX,
        max_bytes: usize::MAX,
    };
}

/// What one `drain_outbound` call handed over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrainReport {
    pub messages: usize,
    pub bytes: usize,
    /// Connections the transport was told to close.
    pub disconnects: usize,
    /// A lane reported congestion and still holds queued work.
    pub blocked: bool,
    /// The caller's limit stopped the drain with work remaining.
    pub limited: bool,
}

/// Deterministic derivation of non-secret session values from one seed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SplitMix64(u64);

impl SplitMix64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub(crate) fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
}
