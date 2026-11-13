//! Host-authoritative networking core.
//!
//! `ge-net` is a sans-I/O library. The application feeds it monotonic time,
//! transport events and commands; it produces bounded outbound messages and
//! typed session events. It starts no threads, never sleeps, reads no clock and
//! opens no files or sockets. Game payloads are opaque byte strings.
//!
//! The six explicit operations are:
//!
//! 1. Admit a transport connection: [`HostSession::admit`] or
//!    [`ClientSession::begin`] followed by [`ClientSession::transport_connected`].
//! 2. Submit local work: [`ClientSession::submit_command`] or
//!    [`HostSession::publish_snapshot`].
//! 3. Ingest one bounded message from an identified connection:
//!    [`HostSession::ingest`] or [`ClientSession::ingest`].
//! 4. Advance timers with caller-supplied monotonic time: `advance`.
//! 5. Drain outbound work and events within caller limits: `drain_outbound`
//!    and `poll_event`.
//! 6. Close or cancel an attempt; later events for it are rejected as stale.
//!
//! The version-one wire format is specified in `docs/networking-wire-format.md`.
//! This crate proves a networking foundation against a synthetic simulation. It
//! does not implement prediction, lag compensation, NAT traversal or a game.

pub mod budget;
pub mod channel;
pub mod diagnostics;
pub mod identity;
pub mod protocol;
pub mod replication;
pub mod session;
pub mod time;
pub mod transport;

pub use budget::{ConfigError, SessionBudget};
pub use channel::{ChannelId, Delivery, Lane};
pub use diagnostics::{Diagnostics, DropReason, PeerDiagnostics};
pub use identity::{
    AttemptId, CommandSeq, EntityAllocator, EntityId, EntropySource, PeerId, PlayerSlot,
    SessionEpoch, SnapshotId, Tick,
};
pub use protocol::compat::{CompatibilityDescriptor, Mismatch, TickRate};
pub use replication::{CommandAck, EntityRecord, ReceivedCommand, Snapshot};
pub use session::client::{ClientConfig, ClientEvent, ClientSession};
pub use session::host::{HostConfig, HostEvent, HostSession};
pub use session::{
    CloseReason, DrainLimit, DrainReport, IngestError, Ingested, Outbound, Phase, Violation,
};
pub use time::NetInstant;
pub use transport::{
    SendOutcome, Transport, TransportCapabilities, TransportEvent, TransportFailure,
};

/// Compiles the example in the crate README.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
