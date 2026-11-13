//! The authority's side of a session.
//!
//! A listen host and a dedicated host use this same type. The host admits
//! transport connections, negotiates compatibility, assigns peers, epochs and
//! player slots, transfers a full snapshot to each joiner, publishes per-peer
//! deltas against acknowledged baselines, and releases client commands in a
//! stable per-tick order.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::Arc;

use super::link::{DrainState, Link, QueueOverflow, Received};
use super::{
    CloseReason, DrainLimit, DrainReport, IngestError, Ingested, Outbound, Phase, SplitMix64,
    StateError, TimeRegression, Violation,
};
use crate::budget::{ConfigError, Limits, SessionBudget};
use crate::channel::{ChannelId, Lane};
use crate::diagnostics::{Diagnostics, DropReason, PeerDiagnostics, SyncProgress};
use crate::identity::{
    AttemptId, CommandSeq, EntropySource, PeerId, PlayerSlot, Sequence, SessionEpoch, SnapshotId,
    Tick,
};
use crate::protocol::compat::CompatibilityDescriptor;
use crate::protocol::envelope::BASE_HEADER_BYTES;
use crate::protocol::message::{
    Accept, Hello, InputBatch, Message, RejectReason, SnapshotRequest, StateUpdate, SyncBegin,
    SyncChunk, decode_payload,
};
use crate::replication::{
    CommandInbox, GenerationFloors, ReceivedCommand, Snapshot, SnapshotError, SnapshotHistory,
    StoredSnapshot, encode_delta,
};
use crate::time::NetInstant;
use crate::transport::{SendOutcome, TransportCapabilities, TransportFailure};

/// What a host needs to exist.
#[derive(Clone, Debug)]
pub struct HostConfig {
    pub budget: SessionBudget,
    pub descriptor: CompatibilityDescriptor,
    /// First envelope sequence of every channel. Both ends must agree. Zero
    /// is the normal value; others exist to exercise wraparound.
    pub sequence_origin: u32,
}

/// Something the host application should know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostEvent {
    /// Negotiation succeeded. The peer is a member but not yet active.
    PeerAccepted {
        attempt: AttemptId,
        peer: PeerId,
        slots: Vec<PlayerSlot>,
        extensions: Vec<u32>,
    },
    /// The peer applied a snapshot at or after the accepted tick. Its
    /// commands are accepted from now on.
    PeerActive {
        attempt: AttemptId,
        peer: PeerId,
        tick: Tick,
    },
    /// The attempt ended. Its slots are free and its later events are stale.
    PeerClosed {
        attempt: AttemptId,
        peer: Option<PeerId>,
        reason: CloseReason,
    },
}

/// Why a snapshot was not published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishError {
    /// Snapshot ticks must strictly increase.
    TickNotIncreasing {
        last: Tick,
    },
    Snapshot(SnapshotError),
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TickNotIncreasing { last } => {
                write!(f, "snapshot tick must exceed {}", last.get())
            }
            Self::Snapshot(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PublishError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Negotiating,
    /// Accepted; waiting for the client to ask for its first snapshot.
    AwaitingRequest,
    /// Asked, but nothing has been published yet.
    AwaitingSnapshot,
    Transferring,
    /// Every chunk is queued; waiting for the application's acknowledgement.
    AwaitingApplied,
    /// Sending deltas until the client reports the accepted tick.
    CatchingUp,
    Active,
    Closing,
}

impl Stage {
    const fn phase(self) -> Phase {
        match self {
            Self::Negotiating => Phase::Negotiating,
            Self::AwaitingRequest
            | Self::AwaitingSnapshot
            | Self::Transferring
            | Self::AwaitingApplied
            | Self::CatchingUp => Phase::Synchronizing,
            Self::Active => Phase::Active,
            Self::Closing => Phase::Closing,
        }
    }

    const fn replicating(self) -> bool {
        matches!(self, Self::CatchingUp | Self::Active)
    }
}

#[derive(Debug)]
struct Transfer {
    snapshot: StoredSnapshot,
    bytes: Vec<u8>,
    next_chunk: usize,
    chunk_count: usize,
}

#[derive(Debug)]
struct HostPeer {
    link: Link,
    stage: Stage,
    peer: Option<PeerId>,
    slots: Vec<PlayerSlot>,
    inboxes: Vec<(PlayerSlot, CommandInbox)>,
    deadline: NetInstant,
    /// The newest snapshot this peer acknowledged holding.
    baseline: Option<StoredSnapshot>,
    transfer: Option<Transfer>,
    /// A fully queued transfer that the peer has not acknowledged yet.
    transferred: Option<StoredSnapshot>,
    /// When a full-state recovery for a replicating peer must have been
    /// acknowledged.
    recovery_deadline: Option<NetInstant>,
    /// Bulk allowance in byte-microseconds.
    bulk_credit: u64,
    bulk_refilled: NetInstant,
    close_reason: Option<CloseReason>,
}

/// Borrowed session state that per-peer helpers need.
struct Shared<'a> {
    budget: &'a SessionBudget,
    limits: &'a Limits,
    now: NetInstant,
    diagnostics: &'a mut Diagnostics,
}

const MICROS_PER_SECOND: u64 = 1_000_000;

impl HostPeer {
    fn begin_close(&mut self, reason: CloseReason, send_close: bool, shared: &mut Shared<'_>) {
        if self.stage == Stage::Closing {
            return;
        }
        self.stage = Stage::Closing;
        self.deadline = shared.now.after(shared.budget.close_timeout);
        self.transfer = None;
        self.transferred = None;
        self.baseline = None;
        for channel in [
            ChannelId::Bulk,
            ChannelId::Input,
            ChannelId::State,
            ChannelId::Probe,
        ] {
            self.link.out.clear_channel(channel);
        }
        if send_close {
            let close = Message::Close(reason.wire_code());
            if self
                .link
                .send(&close, None, shared.limits, shared.diagnostics)
                .is_err()
            {
                // No room even for the goodbye: drop the backlog and leave.
                self.link.out.clear();
            }
        }
        self.close_reason = Some(reason);
    }

    fn refill_bulk(&mut self, shared: &Shared<'_>) {
        let elapsed = shared.now.micros_since(self.bulk_refilled);
        self.bulk_refilled = shared.now;
        let cap = u64::from(shared.budget.bulk_burst_bytes).saturating_mul(MICROS_PER_SECOND);
        self.bulk_credit = self
            .bulk_credit
            .saturating_add(elapsed.saturating_mul(u64::from(shared.budget.bulk_bytes_per_second)))
            .min(cap);
    }

    fn start_transfer(
        &mut self,
        snapshot: &StoredSnapshot,
        shared: &mut Shared<'_>,
    ) -> Result<(), QueueOverflow> {
        let mut bytes = Vec::with_capacity(snapshot.state.encoded_len());
        encode_delta(None, &snapshot.state, &mut bytes);
        let chunk = usize::from(shared.budget.snapshot_chunk_bytes);
        let chunk_count = bytes.len().div_ceil(chunk).max(1);
        let begin = Message::SyncBegin(SyncBegin {
            snapshot: snapshot.id,
            // Publishing bounded both values by the budget.
            total_bytes: bytes.len() as u32,
            chunk_count: chunk_count as u16,
        });
        self.link.send(
            &begin,
            Some(snapshot.tick),
            shared.limits,
            shared.diagnostics,
        )?;
        shared.diagnostics.full_transfers_started += 1;
        self.transferred = None;
        self.transfer = Some(Transfer {
            snapshot: snapshot.clone(),
            bytes,
            next_chunk: 0,
            chunk_count,
        });
        if self.stage == Stage::AwaitingSnapshot {
            self.stage = Stage::Transferring;
        } else if self.recovery_deadline.is_none() {
            // Recovery is bounded like the initial synchronization.
            self.recovery_deadline = Some(shared.now.after(shared.budget.sync_timeout));
        }
        self.pump_transfer(shared)
    }

    /// Queue as many chunks as the bulk allowance and a shallow queue permit.
    fn pump_transfer(&mut self, shared: &mut Shared<'_>) -> Result<(), QueueOverflow> {
        let chunk = usize::from(shared.budget.snapshot_chunk_bytes);
        while let Some(transfer) = self.transfer.as_mut() {
            let start = transfer.next_chunk * chunk;
            let end = (start + chunk).min(transfer.bytes.len());
            let cost = ((BASE_HEADER_BYTES + 6 + end - start) as u64) * MICROS_PER_SECOND;
            if self.bulk_credit < cost || self.link.out.len(ChannelId::Bulk) >= 2 {
                return Ok(());
            }
            let message = Message::SyncChunk(SyncChunk {
                snapshot: transfer.snapshot.id,
                index: transfer.next_chunk as u16,
                data: &transfer.bytes[start..end],
            });
            self.link
                .send(&message, None, shared.limits, shared.diagnostics)?;
            self.bulk_credit -= cost;
            shared.diagnostics.full_transfer_bytes += (end - start) as u64;
            transfer.next_chunk += 1;
            if transfer.next_chunk == transfer.chunk_count {
                let finished = self.transfer.take().expect("transfer present");
                self.transferred = Some(finished.snapshot);
                shared.diagnostics.full_transfers_completed += 1;
                if self.stage == Stage::Transferring {
                    self.stage = Stage::AwaitingApplied;
                }
            }
        }
        Ok(())
    }

    /// Bring this peer towards `latest`: a delta against its acknowledged
    /// baseline when one fits, otherwise a full transfer.
    fn replicate(
        &mut self,
        latest: &StoredSnapshot,
        shared: &mut Shared<'_>,
    ) -> Result<(), QueueOverflow> {
        if self.stage == Stage::AwaitingSnapshot {
            return self.start_transfer(latest, shared);
        }
        if !self.stage.replicating() {
            return Ok(());
        }
        let awaiting_full = self.transfer.is_some() || self.transferred.is_some();
        let baseline = match &self.baseline {
            Some(baseline) if baseline.id == latest.id => return Ok(()),
            Some(baseline) => Some(baseline),
            // A full state is already on its way; wait for its acknowledgement.
            None if awaiting_full => return Ok(()),
            None => None,
        };
        let mut delta = Vec::new();
        encode_delta(
            baseline.map(|stored| &*stored.state),
            &latest.state,
            &mut delta,
        );
        let acks: Vec<_> = self
            .inboxes
            .iter()
            .map(|(slot, inbox)| inbox.ack(*slot))
            .collect();
        // Snapshot id, optional baseline, ack count and nine bytes per ack.
        let body = 4 + 5 + 1 + acks.len() * 9 + delta.len();
        if body <= shared.limits.max_update_bytes {
            let update = Message::StateUpdate(StateUpdate {
                snapshot: latest.id,
                baseline: baseline.map(|stored| stored.id),
                acks,
                delta: &delta,
            });
            shared.diagnostics.state_updates_sent += 1;
            return self.link.send(
                &update,
                Some(latest.tick),
                shared.limits,
                shared.diagnostics,
            );
        }
        if awaiting_full {
            return Ok(());
        }
        self.start_transfer(latest, shared)
    }

    fn note_ack(&mut self, id: SnapshotId, history: &SnapshotHistory, shared: &mut Shared<'_>) {
        if self.baseline.as_ref().is_some_and(|stored| stored.id == id) {
            return;
        }
        let candidate = self
            .transferred
            .as_ref()
            .filter(|stored| stored.id == id)
            .or_else(|| history.get(id))
            .cloned();
        let Some(candidate) = candidate else {
            shared.diagnostics.drop(DropReason::AckUnknownSnapshot);
            return;
        };
        let newer = self
            .baseline
            .as_ref()
            .is_none_or(|stored| candidate.id.is_newer_than(stored.id));
        if !newer {
            shared.diagnostics.drop(DropReason::Stale);
            return;
        }
        if self
            .transferred
            .as_ref()
            .is_some_and(|stored| !stored.id.is_newer_than(candidate.id))
        {
            self.transferred = None;
        }
        self.baseline = Some(candidate);
        self.recovery_deadline = None;
    }
}

/// The authority's session state machine.
#[derive(Debug)]
pub struct HostSession {
    budget: SessionBudget,
    descriptor: CompatibilityDescriptor,
    limits: Limits,
    sequence_origin: Sequence,
    now: NetInstant,
    peers: BTreeMap<AttemptId, HostPeer>,
    last_admitted: Option<AttemptId>,
    rng: SplitMix64,
    next_epoch: SessionEpoch,
    next_peer: u16,
    next_snapshot: SnapshotId,
    history: SnapshotHistory,
    floors: GenerationFloors,
    consumed_tick: Option<Tick>,
    events: VecDeque<HostEvent>,
    disconnects: VecDeque<AttemptId>,
    inbound_used: u32,
    drain_cursor: usize,
    shutting_down: bool,
    diagnostics: Diagnostics,
}

impl HostSession {
    /// Create a host. `entropy` is drawn once to derive session epochs and
    /// sequence origins; it must not be the gameplay RNG.
    pub fn new(
        config: HostConfig,
        capabilities: TransportCapabilities,
        entropy: &mut dyn EntropySource,
        now: NetInstant,
    ) -> Result<Self, ConfigError> {
        let limits = Limits::derive(&config.budget, &capabilities)?;
        config
            .descriptor
            .validate()
            .map_err(|_| ConfigError::InvalidDescriptor)?;
        let mut rng = SplitMix64::new(entropy.next_u64());
        let next_epoch = SessionEpoch::new(rng.next_u32()).successor();
        let next_snapshot = SnapshotId::new(rng.next_u32());
        Ok(Self {
            history: SnapshotHistory::new(usize::from(config.budget.baseline_history)),
            budget: config.budget,
            descriptor: config.descriptor,
            limits,
            sequence_origin: Sequence::new(config.sequence_origin),
            now,
            peers: BTreeMap::new(),
            last_admitted: None,
            rng,
            next_epoch,
            next_peer: 1,
            next_snapshot,
            floors: GenerationFloors::default(),
            consumed_tick: None,
            events: VecDeque::new(),
            disconnects: VecDeque::new(),
            inbound_used: 0,
            drain_cursor: 0,
            shutting_down: false,
            diagnostics: Diagnostics::default(),
        })
    }

    /// Admit an established, authenticated transport connection and wait
    /// for its `Hello`. Attempt identifiers must strictly increase.
    pub fn admit(&mut self, attempt: AttemptId) -> Result<(), StateError> {
        if self.shutting_down {
            return Err(StateError::WrongPhase(Phase::Closing));
        }
        if self.last_admitted.is_some_and(|last| attempt <= last) {
            self.diagnostics.stale_attempt_events += 1;
            return Err(StateError::AttemptNotIncreasing);
        }
        self.last_admitted = Some(attempt);
        let negotiating = self
            .peers
            .values()
            .filter(|peer| peer.stage == Stage::Negotiating)
            .count();
        let capacity = usize::from(self.budget.max_peers) + usize::from(self.budget.max_handshakes);
        if negotiating >= usize::from(self.budget.max_handshakes) || self.peers.len() >= capacity {
            return Err(StateError::TooManyAttempts);
        }
        let link = Link::new(
            attempt,
            self.now,
            &self.budget,
            &self.limits,
            self.sequence_origin,
        );
        self.peers.insert(
            attempt,
            HostPeer {
                link,
                stage: Stage::Negotiating,
                peer: None,
                slots: Vec::new(),
                inboxes: Vec::new(),
                deadline: self.now.after(self.budget.handshake_timeout),
                baseline: None,
                transfer: None,
                transferred: None,
                recovery_deadline: None,
                bulk_credit: u64::from(self.budget.bulk_burst_bytes)
                    .saturating_mul(MICROS_PER_SECOND),
                bulk_refilled: self.now,
                close_reason: None,
            },
        );
        Ok(())
    }

    /// Publish the authoritative state for `tick`.
    ///
    /// Each replicating peer gets a delta against its acknowledged baseline,
    /// or a rate-limited full transfer when it has none or the delta is too
    /// large for the datagram budget.
    pub fn publish_snapshot(
        &mut self,
        tick: Tick,
        snapshot: Snapshot,
    ) -> Result<SnapshotId, PublishError> {
        let previous = self.history.latest();
        if let Some(last) = previous.map(|stored| stored.tick)
            && tick <= last
        {
            return Err(PublishError::TickNotIncreasing { last });
        }
        snapshot
            .check(&self.limits.replication)
            .map_err(PublishError::Snapshot)?;
        self.floors
            .advance(previous.map(|stored| &*stored.state), &snapshot)
            .map_err(PublishError::Snapshot)?;

        let id = self.next_snapshot;
        self.next_snapshot = id.next();
        let stored = StoredSnapshot {
            id,
            tick,
            state: Arc::new(snapshot),
        };
        self.history.push(stored.clone());
        self.diagnostics.snapshots_published += 1;

        let mut shared = Shared {
            budget: &self.budget,
            limits: &self.limits,
            now: self.now,
            diagnostics: &mut self.diagnostics,
        };
        for peer in self.peers.values_mut() {
            if peer.replicate(&stored, &mut shared).is_err() {
                peer.begin_close(CloseReason::QueueOverflow, true, &mut shared);
            }
        }
        Ok(id)
    }

    /// Release the commands for `tick`, ordered by slot and then sequence.
    ///
    /// The order does not depend on packet arrival. Ticks must strictly
    /// increase; commands for a skipped tick are discarded and counted. The
    /// application still validates each command before executing it.
    pub fn take_commands(&mut self, tick: Tick) -> Result<Vec<ReceivedCommand>, PublishError> {
        if let Some(last) = self.consumed_tick
            && tick <= last
        {
            return Err(PublishError::TickNotIncreasing { last });
        }
        self.consumed_tick = Some(tick);
        let mut commands = Vec::new();
        for (attempt, peer) in &mut self.peers {
            let Some(peer_id) = peer.peer else {
                continue;
            };
            for (slot, inbox) in &mut peer.inboxes {
                let missed = inbox.take(tick, |command| {
                    commands.push(ReceivedCommand {
                        attempt: *attempt,
                        peer: peer_id,
                        slot: *slot,
                        sequence: command.sequence,
                        tick: command.tick,
                        payload: command.payload,
                    });
                });
                self.diagnostics
                    .drop_many(DropReason::CommandMissed, missed);
            }
        }
        // Each slot has one owner whose commands are already in sequence order.
        commands.sort_by_key(|command| command.slot);
        self.diagnostics.commands_delivered += commands.len() as u64;
        Ok(commands)
    }

    /// How many more messages `ingest` will accept before the next
    /// `advance`. Zero also when undrained events fill their budget.
    pub fn inbound_credit(&self) -> u32 {
        if self.events.len() >= self.budget.max_pending_events as usize {
            return 0;
        }
        self.budget
            .max_inbound_messages_per_update
            .saturating_sub(self.inbound_used)
    }

    /// Ingest one message from one lane of an admitted connection.
    pub fn ingest(
        &mut self,
        attempt: AttemptId,
        lane: Lane,
        bytes: &[u8],
    ) -> Result<Ingested, IngestError> {
        if !self.peers.contains_key(&attempt) {
            self.diagnostics.stale_attempt_events += 1;
            return Err(IngestError::StaleAttempt);
        }
        if self.inbound_used >= self.budget.max_inbound_messages_per_update {
            return Err(IngestError::InboundBudgetExhausted);
        }
        if self.events.len() >= self.budget.max_pending_events as usize {
            return Err(IngestError::EventQueueFull);
        }
        self.inbound_used += 1;

        let peer = self.peers.get_mut(&attempt).expect("peer checked above");
        let received = peer.link.receive(
            lane,
            bytes,
            self.now,
            &self.budget,
            &self.limits,
            &mut self.diagnostics,
        );
        let closing = peer.stage == Stage::Closing;
        let (kind, tick, payload) = match received {
            Received::Message {
                kind,
                tick,
                payload,
            } => (kind, tick, payload),
            Received::Pending => return Ok(Ingested::Accepted),
            Received::Dropped(reason) => return Ok(Ingested::Dropped(reason)),
            Received::Violation(_) if closing => {
                return Ok(Ingested::Dropped(DropReason::UnexpectedPhase));
            }
            Received::Violation(violation) => return Ok(self.violation(attempt, violation)),
        };
        let message = match decode_payload(kind, &payload, &self.limits.codec) {
            Ok(message) => message,
            Err(_) if closing => return Ok(Ingested::Dropped(DropReason::UnexpectedPhase)),
            Err(error) => return Ok(self.violation(attempt, Violation::Decode(error))),
        };
        if closing && !matches!(message, Message::Close(_)) {
            self.diagnostics.drop(DropReason::UnexpectedPhase);
            return Ok(Ingested::Dropped(DropReason::UnexpectedPhase));
        }
        match self.handle(attempt, message, tick) {
            Ok(ingested) => Ok(ingested),
            Err(violation) => Ok(self.violation(attempt, violation)),
        }
    }

    /// The transport reported that a connection ended.
    pub fn transport_closed(&mut self, attempt: AttemptId, failure: TransportFailure) {
        if !self.peers.contains_key(&attempt) {
            self.diagnostics.stale_attempt_events += 1;
            return;
        }
        let reason = self.peers[&attempt]
            .close_reason
            .unwrap_or(CloseReason::Transport(failure));
        self.finalize(attempt, reason, false);
    }

    /// Advance network timers. Time must not go backwards.
    pub fn advance(&mut self, now: NetInstant) -> Result<(), TimeRegression> {
        if now < self.now {
            return Err(TimeRegression);
        }
        self.now = now;
        self.inbound_used = 0;
        let mut shared = Shared {
            budget: &self.budget,
            limits: &self.limits,
            now,
            diagnostics: &mut self.diagnostics,
        };
        for peer in self.peers.values_mut() {
            let expired = now >= peer.deadline;
            let timeout = match peer.stage.phase() {
                Phase::Negotiating if expired => Some(CloseReason::HandshakeTimeout),
                Phase::Synchronizing if expired => Some(CloseReason::SyncTimeout),
                Phase::Closing => continue,
                _ if peer.link.is_idle(now, shared.budget.idle_timeout) => {
                    Some(CloseReason::IdleTimeout)
                }
                _ if peer
                    .recovery_deadline
                    .is_some_and(|deadline| now >= deadline) =>
                {
                    Some(CloseReason::SyncTimeout)
                }
                _ => None,
            };
            if let Some(reason) = timeout {
                peer.begin_close(reason, true, &mut shared);
                continue;
            }
            peer.link.probe(
                now,
                shared.budget.ping_interval,
                shared.limits,
                shared.diagnostics,
            );
            peer.refill_bulk(&shared);
            if peer.pump_transfer(&mut shared).is_err() {
                peer.begin_close(CloseReason::QueueOverflow, true, &mut shared);
            }
        }
        self.reap();
        Ok(())
    }

    /// Hand queued messages to `sink` within `limit`.
    ///
    /// Control traffic of every peer goes first, then datagrams, then bulk
    /// chunks, so a joining peer's transfer cannot starve active peers. The
    /// starting peer rotates between calls.
    pub fn drain_outbound(
        &mut self,
        limit: DrainLimit,
        mut sink: impl FnMut(Outbound<'_>) -> SendOutcome,
    ) -> DrainReport {
        let mut state = DrainState::new(limit);
        let mut order: Vec<AttemptId> = self.peers.keys().copied().collect();
        if !order.is_empty() {
            let start = self.drain_cursor % order.len();
            order.rotate_left(start);
            self.drain_cursor = self.drain_cursor.wrapping_add(1);
        }
        let mut blocked = vec![[false; 3]; order.len()];
        let passes: [&[ChannelId]; 3] = [
            &[ChannelId::Control],
            &[ChannelId::Probe, ChannelId::State, ChannelId::Input],
            &[ChannelId::Bulk],
        ];
        for pass in passes {
            for (index, attempt) in order.iter().enumerate() {
                let peer = self.peers.get_mut(attempt).expect("peer in drain order");
                for channel in pass {
                    peer.link.drain_channel(
                        *channel,
                        &mut blocked[index],
                        &mut state,
                        &mut sink,
                        &mut self.diagnostics,
                    );
                }
            }
        }
        self.reap();
        while let Some(attempt) = self.disconnects.pop_front() {
            sink(Outbound::Disconnect { attempt });
            state.report.disconnects += 1;
        }
        state.report
    }

    /// The next undrained event.
    pub fn poll_event(&mut self) -> Option<HostEvent> {
        self.events.pop_front()
    }

    /// Close one connection gracefully: its final `Close` is handed to the
    /// transport before the disconnect instruction.
    pub fn close_peer(&mut self, attempt: AttemptId) -> Result<(), StateError> {
        self.begin_close(attempt, CloseReason::Requested, true)
    }

    /// Invalidate an attempt immediately. Its queues are cleared, nothing
    /// more is sent, and its later events are rejected as stale.
    pub fn cancel(&mut self, attempt: AttemptId) -> Result<(), StateError> {
        if !self.peers.contains_key(&attempt) {
            return Err(StateError::StaleAttempt);
        }
        self.finalize(attempt, CloseReason::Cancelled, true);
        Ok(())
    }

    /// Close every connection gracefully and refuse new attempts.
    pub fn shutdown(&mut self) {
        self.shutting_down = true;
        let attempts: Vec<AttemptId> = self.peers.keys().copied().collect();
        for attempt in attempts {
            let _ = self.begin_close(attempt, CloseReason::Shutdown, true);
        }
    }

    /// Whether a shutdown has finished: no connection or instruction remains.
    pub fn is_shut_down(&self) -> bool {
        self.shutting_down && self.peers.is_empty() && self.disconnects.is_empty()
    }

    pub fn phase(&self, attempt: AttemptId) -> Option<Phase> {
        self.peers.get(&attempt).map(|peer| peer.stage.phase())
    }

    /// Live attempts in ascending order.
    pub fn attempts(&self) -> Vec<AttemptId> {
        self.peers.keys().copied().collect()
    }

    /// The newest published snapshot and its tick.
    pub fn latest_snapshot(&self) -> Option<(SnapshotId, Tick)> {
        self.history.latest().map(|stored| (stored.id, stored.tick))
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    pub fn peer_diagnostics(&self, attempt: AttemptId) -> Option<PeerDiagnostics> {
        let peer = self.peers.get(&attempt)?;
        Some(PeerDiagnostics {
            attempt,
            peer: peer.peer,
            phase: peer.stage.phase(),
            queued_bytes: ChannelId::ALL.map(|channel| peer.link.out.bytes(channel)),
            queue_high_water_bytes: peer.link.out.high_water(),
            rtt_micros: peer.link.rtt.smoothed(),
            jitter_micros: peer.link.rtt.deviation(),
            rtt_samples_micros: peer.link.rtt.samples(),
            sync_progress: peer.transfer.as_ref().map(|transfer| SyncProgress {
                transferred_bytes: (transfer.next_chunk
                    * usize::from(self.budget.snapshot_chunk_bytes))
                .min(transfer.bytes.len()),
                total_bytes: transfer.bytes.len(),
            }),
            baseline: peer.baseline.as_ref().map(|stored| stored.id),
        })
    }

    fn shared(&mut self) -> (Shared<'_>, &mut BTreeMap<AttemptId, HostPeer>) {
        (
            Shared {
                budget: &self.budget,
                limits: &self.limits,
                now: self.now,
                diagnostics: &mut self.diagnostics,
            },
            &mut self.peers,
        )
    }

    fn begin_close(
        &mut self,
        attempt: AttemptId,
        reason: CloseReason,
        send_close: bool,
    ) -> Result<(), StateError> {
        let (mut shared, peers) = self.shared();
        let peer = peers.get_mut(&attempt).ok_or(StateError::StaleAttempt)?;
        peer.begin_close(reason, send_close, &mut shared);
        Ok(())
    }

    fn violation(&mut self, attempt: AttemptId, violation: Violation) -> Ingested {
        let reason = CloseReason::Violation(violation);
        let _ = self.begin_close(attempt, reason, true);
        Ingested::Closed(reason)
    }

    /// Remove an attempt. Everything it owned is released here.
    fn finalize(&mut self, attempt: AttemptId, reason: CloseReason, disconnect: bool) {
        let Some(peer) = self.peers.remove(&attempt) else {
            return;
        };
        self.diagnostics.disconnected(&reason);
        self.events.push_back(HostEvent::PeerClosed {
            attempt,
            peer: peer.peer,
            reason,
        });
        if disconnect {
            self.disconnects.push_back(attempt);
        }
    }

    /// Finish closing connections whose goodbye reached the transport, or
    /// whose transport or deadline gave out first.
    fn reap(&mut self) {
        let now = self.now;
        let finished: Vec<(AttemptId, CloseReason)> = self
            .peers
            .iter()
            .filter(|(_, peer)| {
                peer.stage == Stage::Closing
                    && (peer.link.out.len(ChannelId::Control) == 0
                        || peer.link.transport_gone
                        || now >= peer.deadline)
            })
            .map(|(attempt, peer)| {
                (
                    *attempt,
                    peer.close_reason.unwrap_or(CloseReason::Requested),
                )
            })
            .collect();
        for (attempt, reason) in finished {
            self.finalize(attempt, reason, true);
        }
    }

    fn handle(
        &mut self,
        attempt: AttemptId,
        message: Message<'_>,
        tick: Option<Tick>,
    ) -> Result<Ingested, Violation> {
        let stage = self.peers[&attempt].stage;
        match message {
            Message::Ping(stamp) => {
                let (shared, peers) = self.shared();
                let peer = peers.get_mut(&attempt).expect("live peer");
                peer.link.on_ping(stamp, shared.limits, shared.diagnostics);
                Ok(Ingested::Accepted)
            }
            Message::Pong(stamp) => {
                let (shared, peers) = self.shared();
                let peer = peers.get_mut(&attempt).expect("live peer");
                peer.link.on_pong(stamp, shared.now, shared.diagnostics);
                Ok(Ingested::Accepted)
            }
            Message::Close(code) => {
                let reason = CloseReason::RemoteClosed(code);
                self.finalize(attempt, reason, true);
                Ok(Ingested::Closed(reason))
            }
            Message::Hello(hello) if stage == Stage::Negotiating => {
                Ok(self.on_hello(attempt, hello))
            }
            Message::SnapshotRequest(request) => self.on_snapshot_request(attempt, stage, request),
            Message::SyncApplied(id) if stage == Stage::AwaitingApplied => {
                self.on_sync_applied(attempt, id)
            }
            Message::Ready if stage == Stage::CatchingUp => {
                let peer = self.peers.get_mut(&attempt).expect("live peer");
                peer.stage = Stage::Active;
                let peer_id = peer.peer.expect("accepted peer");
                self.events.push_back(HostEvent::PeerActive {
                    attempt,
                    peer: peer_id,
                    tick: tick.unwrap_or_default(),
                });
                Ok(Ingested::Accepted)
            }
            Message::StateAck(id) if stage.replicating() => {
                let history = &self.history;
                let mut shared = Shared {
                    budget: &self.budget,
                    limits: &self.limits,
                    now: self.now,
                    diagnostics: &mut self.diagnostics,
                };
                let peer = self.peers.get_mut(&attempt).expect("live peer");
                peer.note_ack(id, history, &mut shared);
                Ok(Ingested::Accepted)
            }
            Message::StateAck(_) => {
                self.diagnostics.drop(DropReason::UnexpectedPhase);
                Ok(Ingested::Dropped(DropReason::UnexpectedPhase))
            }
            Message::InputBatch(batch) => {
                self.on_input(attempt, stage, batch, tick.unwrap_or_default())
            }
            other => Err(Violation::UnexpectedMessage(other.kind())),
        }
    }

    fn on_hello(&mut self, attempt: AttemptId, hello: Hello) -> Ingested {
        let extensions = match self.descriptor.negotiate(&hello.descriptor) {
            Ok(extensions) => extensions,
            Err(mismatch) => return self.reject(attempt, RejectReason::Incompatible(mismatch)),
        };
        let members = self
            .peers
            .values()
            .filter(|peer| peer.peer.is_some() && peer.stage != Stage::Closing)
            .count();
        if members >= usize::from(self.budget.max_peers) {
            return self.reject(attempt, RejectReason::SessionFull);
        }
        let requested = usize::from(hello.requested_slots);
        if requested == 0 || requested > usize::from(self.budget.max_slots_per_peer) {
            return self.reject(attempt, RejectReason::InvalidRequest);
        }
        let slots: Vec<PlayerSlot> = (0..self.budget.max_player_slots)
            .map(PlayerSlot::new)
            .filter(|slot| !self.peers.values().any(|peer| peer.slots.contains(slot)))
            .take(requested)
            .collect();
        if slots.len() < requested {
            return self.reject(attempt, RejectReason::SlotsUnavailable);
        }

        let peer_id = self.allocate_peer();
        let epoch = self.next_epoch;
        self.next_epoch = epoch.successor();
        let command_origin = CommandSeq::new(self.rng.next_u32());
        let capacity = usize::from(self.budget.max_pending_commands);
        let deadline = self.now.after(self.budget.sync_timeout);

        let (shared, peers) = self.shared();
        let peer = peers.get_mut(&attempt).expect("live peer");
        peer.link.epoch = epoch;
        peer.peer = Some(peer_id);
        peer.slots = slots.clone();
        peer.inboxes = slots
            .iter()
            .map(|slot| (*slot, CommandInbox::new(command_origin, capacity)))
            .collect();
        peer.stage = Stage::AwaitingRequest;
        peer.deadline = deadline;
        let accept = Message::Accept(Accept {
            peer: peer_id,
            slots: slots.clone(),
            extensions: extensions.clone(),
            command_origin,
        });
        if peer
            .link
            .send(&accept, None, shared.limits, shared.diagnostics)
            .is_err()
        {
            let _ = self.begin_close(attempt, CloseReason::QueueOverflow, true);
            return Ingested::Closed(CloseReason::QueueOverflow);
        }
        self.events.push_back(HostEvent::PeerAccepted {
            attempt,
            peer: peer_id,
            slots,
            extensions,
        });
        Ingested::Accepted
    }

    fn allocate_peer(&mut self) -> PeerId {
        loop {
            let candidate = PeerId::new(self.next_peer);
            self.next_peer = self.next_peer.checked_add(1).unwrap_or(1);
            if candidate != PeerId::HOST
                && !self.peers.values().any(|peer| peer.peer == Some(candidate))
            {
                return candidate;
            }
        }
    }

    fn reject(&mut self, attempt: AttemptId, reason: RejectReason) -> Ingested {
        let (mut shared, peers) = self.shared();
        let peer = peers.get_mut(&attempt).expect("live peer");
        let sent = peer.link.send(
            &Message::Reject(reason),
            None,
            shared.limits,
            shared.diagnostics,
        );
        let close = CloseReason::Rejected(reason);
        peer.begin_close(close, false, &mut shared);
        if sent.is_err() {
            peer.link.out.clear();
        }
        Ingested::Closed(close)
    }

    fn on_snapshot_request(
        &mut self,
        attempt: AttemptId,
        stage: Stage,
        request: SnapshotRequest,
    ) -> Result<Ingested, Violation> {
        let latest = self.history.latest().cloned();
        let held = request.held.and_then(|id| self.history.get(id).cloned());
        let (mut shared, peers) = self.shared();
        let peer = peers.get_mut(&attempt).expect("live peer");
        let result = match stage {
            Stage::AwaitingRequest => {
                peer.stage = Stage::AwaitingSnapshot;
                match &latest {
                    Some(latest) => peer.replicate(latest, &mut shared),
                    None => Ok(()),
                }
            }
            Stage::CatchingUp | Stage::Active => {
                shared.diagnostics.recovery_requests += 1;
                if peer.transfer.is_some() {
                    shared.diagnostics.drop(DropReason::RecoveryIgnored);
                    return Ok(Ingested::Dropped(DropReason::RecoveryIgnored));
                }
                // Fall back to a baseline the client says it still holds.
                peer.baseline = held;
                peer.transferred = None;
                match &latest {
                    Some(latest) => peer.replicate(latest, &mut shared),
                    None => Ok(()),
                }
            }
            Stage::AwaitingSnapshot | Stage::Transferring | Stage::AwaitingApplied => {
                shared.diagnostics.drop(DropReason::RecoveryIgnored);
                return Ok(Ingested::Dropped(DropReason::RecoveryIgnored));
            }
            Stage::Negotiating | Stage::Closing => {
                return Err(Violation::UnexpectedMessage(
                    Message::SnapshotRequest(request).kind(),
                ));
            }
        };
        Ok(self.after_send(attempt, result))
    }

    fn on_sync_applied(
        &mut self,
        attempt: AttemptId,
        id: SnapshotId,
    ) -> Result<Ingested, Violation> {
        let latest = self.history.latest().cloned();
        let (mut shared, peers) = self.shared();
        let peer = peers.get_mut(&attempt).expect("live peer");
        if peer.transferred.as_ref().map(|stored| stored.id) != Some(id) {
            return Err(Violation::Transfer("applied acknowledgement"));
        }
        peer.baseline = peer.transferred.take();
        peer.recovery_deadline = None;
        peer.stage = Stage::CatchingUp;
        let latest = latest.expect("a transfer implies a published snapshot");
        let result = peer
            .link
            .send(
                &Message::Activate,
                Some(latest.tick),
                shared.limits,
                shared.diagnostics,
            )
            .and_then(|()| peer.replicate(&latest, &mut shared));
        Ok(self.after_send(attempt, result))
    }

    fn after_send(&mut self, attempt: AttemptId, result: Result<(), QueueOverflow>) -> Ingested {
        match result {
            Ok(()) => Ingested::Accepted,
            Err(QueueOverflow) => {
                let _ = self.begin_close(attempt, CloseReason::QueueOverflow, true);
                Ingested::Closed(CloseReason::QueueOverflow)
            }
        }
    }

    fn on_input(
        &mut self,
        attempt: AttemptId,
        stage: Stage,
        batch: InputBatch<'_>,
        base: Tick,
    ) -> Result<Ingested, Violation> {
        if !stage.replicating() {
            self.diagnostics.drop(DropReason::UnexpectedPhase);
            return Ok(Ingested::Dropped(DropReason::UnexpectedPhase));
        }
        let consumed = self.consumed_tick;
        let future_ticks = u64::from(self.budget.command_future_ticks);
        let history = &self.history;
        let mut shared = Shared {
            budget: &self.budget,
            limits: &self.limits,
            now: self.now,
            diagnostics: &mut self.diagnostics,
        };
        let peer = self.peers.get_mut(&attempt).expect("live peer");
        if let Some(ack) = batch.ack {
            peer.note_ack(ack, history, &mut shared);
        }
        if base.checked_add(u64::from(u16::MAX)).is_none() {
            return Err(Violation::Decode(
                crate::protocol::DecodeError::InvalidField("command base tick"),
            ));
        }
        for group in &batch.groups {
            let Some((_, inbox)) = peer
                .inboxes
                .iter_mut()
                .find(|(slot, _)| *slot == group.slot)
            else {
                return Err(Violation::UnownedSlot(group.slot));
            };
            if stage != Stage::Active {
                // The readiness report travels on another lane and has not
                // arrived yet. The client resends these.
                shared
                    .diagnostics
                    .drop_many(DropReason::CommandBeforeActive, group.commands.len() as u64);
                continue;
            }
            let commands = group.commands.iter().map(|command| {
                (
                    Tick::new(base.get() + u64::from(command.tick_offset)),
                    command.payload,
                )
            });
            let report = inbox.offer(group.first, commands, consumed, future_ticks);
            let diagnostics = &mut *shared.diagnostics;
            diagnostics.commands_accepted += report.accepted;
            diagnostics.drop_many(DropReason::CommandDuplicate, report.duplicates);
            diagnostics.drop_many(DropReason::CommandLate, report.late);
            diagnostics.drop_many(DropReason::CommandFuture, report.future);
            diagnostics.drop_many(DropReason::CommandInboxFull, report.full);
            diagnostics.drop_many(DropReason::CommandGap, report.gap);
            if report.tick_regression {
                return Err(Violation::CommandTickRegression);
            }
        }
        Ok(Ingested::Accepted)
    }
}
