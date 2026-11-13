//! Bounded counters and sanitized per-connection state.
//!
//! Diagnostics hold aggregate numbers and library identities only. They never
//! hold packet payloads, addresses, credentials or game content.

use crate::channel::ChannelId;
use crate::identity::{AttemptId, PeerId, SnapshotId};
use crate::session::{CloseReason, Phase};

/// Why a message or command was discarded without failing the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropReason {
    /// Older than the last accepted sequence or snapshot.
    Stale,
    /// The same sequence or fragment was already accepted.
    Duplicate,
    /// A datagram from another epoch, such as a previous connection.
    EpochMismatch,
    /// An unknown optional kind, skipped.
    UnknownOptional,
    /// An unsent unreliable message evicted to make room for a newer one.
    QueueEvicted,
    /// An unsent replaceable message superseded by a newer one.
    Replaced,
    /// An unreliable message that did not fit beside queued reliable data.
    QueueFull,
    /// A partial fragmented message superseded before it completed.
    PartialAbandoned,
    /// A message that this phase does not act on.
    UnexpectedPhase,
    /// A command already received.
    CommandDuplicate,
    /// A command for a tick the host already consumed.
    CommandLate,
    /// A command beyond the future window; the client will resend it.
    CommandFuture,
    /// A command whose tick the application skipped.
    CommandMissed,
    /// A command refused because its slot's buffer is full.
    CommandInboxFull,
    /// A command that does not follow the last received one.
    CommandGap,
    /// A command from a peer that is not active yet.
    CommandBeforeActive,
    /// A delta whose baseline the client no longer holds.
    MissingBaseline,
    /// An acknowledgement for a snapshot the host no longer retains.
    AckUnknownSnapshot,
    /// A round-trip echo that cannot have come from a probe we sent.
    InvalidPong,
    /// A recovery request ignored because a transfer is already running.
    RecoveryIgnored,
}

impl DropReason {
    pub const ALL: [Self; 20] = [
        Self::Stale,
        Self::Duplicate,
        Self::EpochMismatch,
        Self::UnknownOptional,
        Self::QueueEvicted,
        Self::Replaced,
        Self::QueueFull,
        Self::PartialAbandoned,
        Self::UnexpectedPhase,
        Self::CommandDuplicate,
        Self::CommandLate,
        Self::CommandFuture,
        Self::CommandMissed,
        Self::CommandInboxFull,
        Self::CommandGap,
        Self::CommandBeforeActive,
        Self::MissingBaseline,
        Self::AckUnknownSnapshot,
        Self::InvalidPong,
        Self::RecoveryIgnored,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

/// Traffic on one channel, summed over every connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelCounters {
    pub messages_sent: u64,
    pub bytes_sent: u64,
    pub messages_received: u64,
    pub bytes_received: u64,
}

/// How connections ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisconnectCounters {
    pub requested: u64,
    pub remote_closed: u64,
    pub rejected: u64,
    pub violation: u64,
    pub handshake_timeout: u64,
    pub sync_timeout: u64,
    pub idle_timeout: u64,
    pub queue_overflow: u64,
    pub transport: u64,
    pub cancelled: u64,
    pub shutdown: u64,
}

/// Aggregate counters for one session object.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    channels: [ChannelCounters; ChannelId::COUNT],
    drops: [u64; DropReason::ALL.len()],
    pub disconnects: DisconnectCounters,
    /// Attempts refused at admission or whose events arrived after closing.
    pub stale_attempt_events: u64,
    pub snapshots_published: u64,
    pub state_updates_sent: u64,
    pub state_updates_applied: u64,
    /// Full-state recovery requests sent (client) or received (host).
    pub recovery_requests: u64,
    pub full_transfers_started: u64,
    pub full_transfers_completed: u64,
    pub full_transfer_bytes: u64,
    pub commands_submitted: u64,
    pub commands_accepted: u64,
    pub commands_delivered: u64,
    /// Highest outbound queue occupancy seen on any single connection.
    pub queue_high_water_bytes: u64,
}

impl Diagnostics {
    pub fn channel(&self, channel: ChannelId) -> ChannelCounters {
        self.channels[channel.index()]
    }

    pub fn drops(&self, reason: DropReason) -> u64 {
        self.drops[reason.index()]
    }

    pub fn total_drops(&self) -> u64 {
        self.drops.iter().sum()
    }

    pub(crate) fn drop(&mut self, reason: DropReason) {
        self.drop_many(reason, 1);
    }

    pub(crate) fn drop_many(&mut self, reason: DropReason, count: u64) {
        self.drops[reason.index()] += count;
    }

    pub(crate) fn sent(&mut self, channel: ChannelId, bytes: usize) {
        let counters = &mut self.channels[channel.index()];
        counters.messages_sent += 1;
        counters.bytes_sent += bytes as u64;
    }

    pub(crate) fn received(&mut self, channel: ChannelId, bytes: usize) {
        let counters = &mut self.channels[channel.index()];
        counters.messages_received += 1;
        counters.bytes_received += bytes as u64;
    }

    pub(crate) fn queue_high_water(&mut self, bytes: usize) {
        self.queue_high_water_bytes = self.queue_high_water_bytes.max(bytes as u64);
    }

    pub(crate) fn disconnected(&mut self, reason: &CloseReason) {
        let counters = &mut self.disconnects;
        let counter = match reason {
            CloseReason::Requested => &mut counters.requested,
            CloseReason::RemoteClosed(_) => &mut counters.remote_closed,
            CloseReason::Rejected(_) => &mut counters.rejected,
            CloseReason::Violation(_) => &mut counters.violation,
            CloseReason::HandshakeTimeout => &mut counters.handshake_timeout,
            CloseReason::SyncTimeout => &mut counters.sync_timeout,
            CloseReason::IdleTimeout => &mut counters.idle_timeout,
            CloseReason::QueueOverflow => &mut counters.queue_overflow,
            CloseReason::Transport(_) => &mut counters.transport,
            CloseReason::Cancelled => &mut counters.cancelled,
            CloseReason::Shutdown => &mut counters.shutdown,
        };
        *counter += 1;
    }
}

/// Progress of a full snapshot transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncProgress {
    pub transferred_bytes: usize,
    pub total_bytes: usize,
}

/// Sanitized state of one connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerDiagnostics {
    pub attempt: AttemptId,
    pub peer: Option<PeerId>,
    pub phase: Phase,
    /// Outbound bytes queued per channel, indexed by [`ChannelId::index`].
    pub queued_bytes: [usize; ChannelId::COUNT],
    pub queue_high_water_bytes: usize,
    /// Smoothed round-trip time, once a probe has been answered.
    pub rtt_micros: Option<u64>,
    /// Smoothed round-trip deviation.
    pub jitter_micros: u64,
    /// The most recent round-trip samples, oldest first, bounded by budget.
    pub rtt_samples_micros: Vec<u32>,
    pub sync_progress: Option<SyncProgress>,
    /// The acknowledged baseline (host) or newest held snapshot (client).
    pub baseline: Option<SnapshotId>,
}
