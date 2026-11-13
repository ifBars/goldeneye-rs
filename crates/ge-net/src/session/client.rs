//! A client's side of a session.
//!
//! A client owns one attempt at a time. It negotiates, receives a full
//! snapshot, waits for the application to report that it applied that
//! snapshot, catches up to the host's accepted tick and only then becomes
//! active. Received bytes never stand in for application work.

use std::collections::VecDeque;
use std::sync::Arc;

use super::link::{DrainState, Link, QueueOverflow, Received, input_capacity};
use super::{
    CloseReason, DrainLimit, DrainReport, IngestError, Ingested, Outbound, Phase, StateError,
    TimeRegression, Violation,
};
use crate::budget::{ConfigError, Limits, SessionBudget};
use crate::channel::{ChannelId, Lane};
use crate::diagnostics::{Diagnostics, DropReason, PeerDiagnostics, SyncProgress};
use crate::identity::{AttemptId, CommandSeq, PeerId, PlayerSlot, Sequence, SnapshotId, Tick};
use crate::protocol::compat::CompatibilityDescriptor;
use crate::protocol::message::{
    Accept, CommandGroup, Hello, InputBatch, Message, SnapshotRequest, StateUpdate, SyncBegin,
    SyncChunk, WireCommand, decode_payload,
};
use crate::replication::{
    CommandAck, CommandOutbox, GenerationFloors, Snapshot, SnapshotError, SnapshotHistory,
    StoredSnapshot, SubmitError, apply_delta,
};
use crate::time::NetInstant;
use crate::transport::{SendOutcome, TransportCapabilities, TransportFailure};

/// What a client needs to exist.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub budget: SessionBudget,
    pub descriptor: CompatibilityDescriptor,
    /// Local player slots to ask the host for.
    pub requested_slots: u8,
    /// First envelope sequence of every channel. Must match the host.
    pub sequence_origin: u32,
}

/// Something the client application should know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    /// The host accepted the descriptor and assigned an identity.
    Accepted {
        peer: PeerId,
        slots: Vec<PlayerSlot>,
        extensions: Vec<u32>,
    },
    /// The first full snapshot. Apply it, then call
    /// [`ClientSession::acknowledge_applied`] with its identifier.
    SyncSnapshot {
        id: SnapshotId,
        tick: Tick,
        state: Arc<Snapshot>,
    },
    /// A newer complete authoritative state. While synchronizing, apply it
    /// and acknowledge it so the session can become active.
    Snapshot {
        id: SnapshotId,
        tick: Tick,
        state: Arc<Snapshot>,
        /// What the host had done with this client's commands at `tick`.
        acks: Vec<CommandAck>,
    },
    /// The application applied a snapshot at or after the host's accepted
    /// tick. Commands may be submitted from now on.
    Activated { tick: Tick },
    /// The attempt ended. A reconnect begins a new attempt and epoch.
    Closed { reason: CloseReason },
}

#[derive(Debug)]
struct IncomingTransfer {
    snapshot: SnapshotId,
    tick: Tick,
    total_bytes: usize,
    chunk_count: usize,
    next_chunk: usize,
    bytes: Vec<u8>,
}

/// State that exists only while an attempt is live.
#[derive(Debug)]
struct Attempt {
    link: Link,
    deadline: NetInstant,
    peer: Option<PeerId>,
    outboxes: Vec<CommandOutbox>,
    history: SnapshotHistory,
    /// The baseline the host last referenced, kept past ring eviction.
    pinned: Option<StoredSnapshot>,
    latest: Option<StoredSnapshot>,
    floors: GenerationFloors,
    incoming: Option<IncomingTransfer>,
    /// The first full snapshot, until the application acknowledges it.
    pending_apply: Option<SnapshotId>,
    /// The application acknowledged the first full snapshot.
    synced: bool,
    /// Tick of the newest snapshot the application acknowledged applying.
    applied_tick: Option<Tick>,
    /// The host's accepted tick, once announced.
    activation_tick: Option<Tick>,
    next_input: NetInstant,
    recovery_requested: Option<NetInstant>,
    close_reason: Option<CloseReason>,
}

/// A client's session state machine.
#[derive(Debug)]
pub struct ClientSession {
    budget: SessionBudget,
    descriptor: CompatibilityDescriptor,
    requested_slots: u8,
    limits: Limits,
    sequence_origin: Sequence,
    now: NetInstant,
    phase: Phase,
    attempt: Option<Attempt>,
    last_attempt: Option<AttemptId>,
    events: VecDeque<ClientEvent>,
    disconnect: Option<AttemptId>,
    inbound_used: u32,
    diagnostics: Diagnostics,
}

impl ClientSession {
    pub fn new(
        config: ClientConfig,
        capabilities: TransportCapabilities,
        now: NetInstant,
    ) -> Result<Self, ConfigError> {
        let limits = Limits::derive(&config.budget, &capabilities)?;
        config
            .descriptor
            .validate()
            .map_err(|_| ConfigError::InvalidDescriptor)?;
        if config.requested_slots == 0 || config.requested_slots > config.budget.max_slots_per_peer
        {
            return Err(ConfigError::InvalidBudget("requested_slots"));
        }
        Ok(Self {
            budget: config.budget,
            descriptor: config.descriptor,
            requested_slots: config.requested_slots,
            limits,
            sequence_origin: Sequence::new(config.sequence_origin),
            now,
            phase: Phase::Idle,
            attempt: None,
            last_attempt: None,
            events: VecDeque::new(),
            disconnect: None,
            inbound_used: 0,
            diagnostics: Diagnostics::default(),
        })
    }

    /// Begin a new attempt while the transport connects. Valid when idle or
    /// closed. Attempt identifiers must strictly increase.
    pub fn begin(&mut self, attempt: AttemptId) -> Result<(), StateError> {
        if !matches!(self.phase, Phase::Idle | Phase::Closed) {
            return Err(StateError::WrongPhase(self.phase));
        }
        if self.last_attempt.is_some_and(|last| attempt <= last) {
            return Err(StateError::AttemptNotIncreasing);
        }
        self.last_attempt = Some(attempt);
        self.phase = Phase::Connecting;
        self.attempt = Some(Attempt {
            link: Link::new(
                attempt,
                self.now,
                &self.budget,
                &self.limits,
                self.sequence_origin,
            ),
            deadline: self.now.after(self.budget.handshake_timeout),
            peer: None,
            outboxes: Vec::new(),
            history: SnapshotHistory::new(usize::from(self.budget.baseline_history)),
            pinned: None,
            latest: None,
            floors: GenerationFloors::default(),
            incoming: None,
            pending_apply: None,
            synced: false,
            applied_tick: None,
            activation_tick: None,
            next_input: self.now,
            recovery_requested: None,
            close_reason: None,
        });
        Ok(())
    }

    /// The transport connected and verified the host. Sends `Hello`.
    pub fn transport_connected(&mut self, attempt: AttemptId) -> Result<(), StateError> {
        if !self.is_live(attempt) {
            return Err(StateError::StaleAttempt);
        }
        if self.phase != Phase::Connecting {
            return Err(StateError::WrongPhase(self.phase));
        }
        let state = self.attempt.as_mut().expect("live attempt");
        let hello = Message::Hello(Hello {
            requested_slots: self.requested_slots,
            descriptor: self.descriptor.clone(),
        });
        // Liveness is measured from the moment a connection exists.
        state.link.last_heard = self.now;
        if state
            .link
            .send(&hello, None, &self.limits, &mut self.diagnostics)
            .is_err()
        {
            self.finalize(CloseReason::QueueOverflow, true);
            return Err(StateError::QueueOverflow);
        }
        self.phase = Phase::Negotiating;
        Ok(())
    }

    /// Queue a command for one of this client's slots at `tick`.
    ///
    /// Every unacknowledged command is resent until the host reports it
    /// received. A full outbox is an error, never a silent drop.
    pub fn submit_command(
        &mut self,
        slot: PlayerSlot,
        tick: Tick,
        payload: &[u8],
    ) -> Result<CommandSeq, SubmitError> {
        if self.phase != Phase::Active {
            return Err(SubmitError::NotActive);
        }
        let max = usize::from(self.budget.max_command_bytes);
        if payload.len() > max {
            return Err(SubmitError::CommandTooLarge {
                len: payload.len(),
                max,
            });
        }
        let state = self.attempt.as_mut().ok_or(SubmitError::NotActive)?;
        let outbox = state
            .outboxes
            .iter_mut()
            .find(|outbox| outbox.slot == slot)
            .ok_or(SubmitError::UnknownSlot(slot))?;
        let sequence = outbox.submit(tick, payload)?;
        self.diagnostics.commands_submitted += 1;
        Ok(sequence)
    }

    /// Report that the application applied snapshot `id`.
    ///
    /// The first acknowledgement must name the `SyncSnapshot`. Later ones
    /// name any snapshot still held; once one reaches the host's accepted
    /// tick the session becomes active.
    pub fn acknowledge_applied(&mut self, id: SnapshotId) -> Result<(), StateError> {
        if !matches!(self.phase, Phase::Synchronizing | Phase::Active) {
            return Err(StateError::WrongPhase(self.phase));
        }
        let state = self.attempt.as_mut().ok_or(StateError::StaleAttempt)?;
        let tick = state
            .history
            .get(id)
            .or(state.latest.as_ref().filter(|stored| stored.id == id))
            .map(|stored| stored.tick)
            .ok_or(StateError::UnknownSnapshot)?;
        if !state.synced {
            if state.pending_apply != Some(id) {
                return Err(StateError::UnknownSnapshot);
            }
            if state
                .link
                .send(
                    &Message::SyncApplied(id),
                    None,
                    &self.limits,
                    &mut self.diagnostics,
                )
                .is_err()
            {
                self.fail(CloseReason::QueueOverflow);
                return Err(StateError::QueueOverflow);
            }
            state.pending_apply = None;
            state.synced = true;
        }
        if state.applied_tick.is_none_or(|applied| tick > applied) {
            state.applied_tick = Some(tick);
        }
        self.try_activate();
        Ok(())
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

    /// Ingest one message from one lane of the live attempt.
    pub fn ingest(
        &mut self,
        attempt: AttemptId,
        lane: Lane,
        bytes: &[u8],
    ) -> Result<Ingested, IngestError> {
        if !self.is_live(attempt) {
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

        let state = self.attempt.as_mut().expect("live attempt");
        let received = state.link.receive(
            lane,
            bytes,
            self.now,
            &self.budget,
            &self.limits,
            &mut self.diagnostics,
        );
        let closing = self.phase == Phase::Closing;
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
            Received::Violation(violation) => return Ok(self.violation(violation)),
        };
        let message = match decode_payload(kind, &payload, &self.limits.codec) {
            Ok(message) => message,
            Err(_) if closing => return Ok(Ingested::Dropped(DropReason::UnexpectedPhase)),
            Err(error) => return Ok(self.violation(Violation::Decode(error))),
        };
        if closing && !matches!(message, Message::Close(_)) {
            self.diagnostics.drop(DropReason::UnexpectedPhase);
            return Ok(Ingested::Dropped(DropReason::UnexpectedPhase));
        }
        match self.handle(message, tick) {
            Ok(ingested) => Ok(ingested),
            Err(violation) => Ok(self.violation(violation)),
        }
    }

    /// The transport reported that the connection ended or never formed.
    pub fn transport_closed(&mut self, attempt: AttemptId, failure: TransportFailure) {
        if !self.is_live(attempt) {
            self.diagnostics.stale_attempt_events += 1;
            return;
        }
        let reason = self
            .attempt
            .as_ref()
            .and_then(|state| state.close_reason)
            .unwrap_or(CloseReason::Transport(failure));
        self.finalize(reason, false);
    }

    /// Advance network timers. Time must not go backwards.
    pub fn advance(&mut self, now: NetInstant) -> Result<(), TimeRegression> {
        if now < self.now {
            return Err(TimeRegression);
        }
        self.now = now;
        self.inbound_used = 0;
        let Some(state) = self.attempt.as_mut() else {
            return Ok(());
        };
        let expired = now >= state.deadline;
        let connected = !matches!(self.phase, Phase::Connecting);
        let timeout = match self.phase {
            Phase::Connecting | Phase::Negotiating if expired => {
                Some(CloseReason::HandshakeTimeout)
            }
            Phase::Synchronizing if expired => Some(CloseReason::SyncTimeout),
            Phase::Closing => {
                self.reap();
                return Ok(());
            }
            _ if connected && state.link.is_idle(now, self.budget.idle_timeout) => {
                Some(CloseReason::IdleTimeout)
            }
            _ => None,
        };
        if let Some(reason) = timeout {
            self.fail(reason);
            return Ok(());
        }
        if connected {
            state.link.probe(
                now,
                self.budget.ping_interval,
                &self.limits,
                &mut self.diagnostics,
            );
        }
        self.flush_input();
        Ok(())
    }

    /// Hand queued messages to `sink` within `limit`: control first, then
    /// datagrams.
    pub fn drain_outbound(
        &mut self,
        limit: DrainLimit,
        mut sink: impl FnMut(Outbound<'_>) -> SendOutcome,
    ) -> DrainReport {
        let mut state = DrainState::new(limit);
        if let Some(attempt) = self.attempt.as_mut()
            && self.phase != Phase::Connecting
        {
            let mut blocked = [false; 3];
            for channel in [
                ChannelId::Control,
                ChannelId::Probe,
                ChannelId::Input,
                ChannelId::Bulk,
            ] {
                attempt.link.drain_channel(
                    channel,
                    &mut blocked,
                    &mut state,
                    &mut sink,
                    &mut self.diagnostics,
                );
            }
        }
        self.reap();
        if let Some(attempt) = self.disconnect.take() {
            sink(Outbound::Disconnect { attempt });
            state.report.disconnects += 1;
        }
        state.report
    }

    /// The next undrained event.
    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }

    /// Close the live attempt gracefully.
    pub fn close(&mut self) -> Result<(), StateError> {
        if self.attempt.is_none() {
            return Err(StateError::WrongPhase(self.phase));
        }
        if self.phase == Phase::Connecting {
            // Nothing can be said to a host that is not connected yet.
            self.finalize(CloseReason::Requested, true);
        } else {
            self.fail(CloseReason::Requested);
        }
        Ok(())
    }

    /// Invalidate the live attempt immediately. Nothing more is sent and its
    /// later events are rejected as stale.
    pub fn cancel(&mut self) -> Result<(), StateError> {
        if self.attempt.is_none() {
            return Err(StateError::WrongPhase(self.phase));
        }
        self.finalize(CloseReason::Cancelled, true);
        Ok(())
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The live attempt, if any.
    pub fn attempt(&self) -> Option<AttemptId> {
        self.attempt.as_ref().map(|state| state.link.attempt)
    }

    /// The identity the host assigned, once accepted.
    pub fn peer(&self) -> Option<PeerId> {
        self.attempt.as_ref().and_then(|state| state.peer)
    }

    /// The slots the host assigned, once accepted.
    pub fn slots(&self) -> Vec<PlayerSlot> {
        self.attempt.as_ref().map_or_else(Vec::new, |state| {
            state.outboxes.iter().map(|outbox| outbox.slot).collect()
        })
    }

    /// The newest snapshot held, with its tick.
    pub fn latest_snapshot(&self) -> Option<(SnapshotId, Tick)> {
        let latest = self.attempt.as_ref()?.latest.as_ref()?;
        Some((latest.id, latest.tick))
    }

    /// Commands of `slot` that the host has not yet reported receiving.
    pub fn pending_commands(&self, slot: PlayerSlot) -> usize {
        self.attempt
            .as_ref()
            .and_then(|state| state.outboxes.iter().find(|outbox| outbox.slot == slot))
            .map_or(0, |outbox| outbox.pending().len())
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    pub fn peer_diagnostics(&self) -> Option<PeerDiagnostics> {
        let state = self.attempt.as_ref()?;
        Some(PeerDiagnostics {
            attempt: state.link.attempt,
            peer: state.peer,
            phase: self.phase,
            queued_bytes: ChannelId::ALL.map(|channel| state.link.out.bytes(channel)),
            queue_high_water_bytes: state.link.out.high_water(),
            rtt_micros: state.link.rtt.smoothed(),
            jitter_micros: state.link.rtt.deviation(),
            rtt_samples_micros: state.link.rtt.samples(),
            sync_progress: state.incoming.as_ref().map(|incoming| SyncProgress {
                transferred_bytes: incoming.bytes.len(),
                total_bytes: incoming.total_bytes,
            }),
            baseline: state.latest.as_ref().map(|stored| stored.id),
        })
    }

    fn is_live(&self, attempt: AttemptId) -> bool {
        self.attempt
            .as_ref()
            .is_some_and(|state| state.link.attempt == attempt)
    }

    fn violation(&mut self, violation: Violation) -> Ingested {
        let reason = CloseReason::Violation(violation);
        self.fail(reason);
        Ingested::Closed(reason)
    }

    /// Close with a goodbye: the attempt ends once it reaches the transport.
    fn fail(&mut self, reason: CloseReason) {
        let Some(state) = self.attempt.as_mut() else {
            return;
        };
        if self.phase == Phase::Closing {
            return;
        }
        if self.phase == Phase::Connecting {
            self.finalize(reason, true);
            return;
        }
        self.phase = Phase::Closing;
        state.deadline = self.now.after(self.budget.close_timeout);
        state.close_reason = Some(reason);
        state.incoming = None;
        for channel in [ChannelId::Input, ChannelId::Probe] {
            state.link.out.clear_channel(channel);
        }
        let close = Message::Close(reason.wire_code());
        if state
            .link
            .send(&close, None, &self.limits, &mut self.diagnostics)
            .is_err()
        {
            state.link.out.clear();
        }
    }

    /// End the attempt. Everything it owned is released here.
    fn finalize(&mut self, reason: CloseReason, disconnect: bool) {
        let Some(state) = self.attempt.take() else {
            return;
        };
        self.phase = Phase::Closed;
        self.diagnostics.disconnected(&reason);
        self.events.push_back(ClientEvent::Closed { reason });
        if disconnect {
            self.disconnect = Some(state.link.attempt);
        }
    }

    fn reap(&mut self) {
        if self.phase != Phase::Closing {
            return;
        }
        let Some(state) = self.attempt.as_ref() else {
            return;
        };
        if state.link.out.len(ChannelId::Control) == 0
            || state.link.transport_gone
            || self.now >= state.deadline
        {
            let reason = state.close_reason.unwrap_or(CloseReason::Requested);
            self.finalize(reason, true);
        }
    }

    fn send_reliable(&mut self, message: &Message<'_>, tick: Option<Tick>) -> bool {
        let Some(state) = self.attempt.as_mut() else {
            return false;
        };
        if state
            .link
            .send(message, tick, &self.limits, &mut self.diagnostics)
            .is_err()
        {
            self.fail(CloseReason::QueueOverflow);
            return false;
        }
        true
    }

    fn handle(&mut self, message: Message<'_>, tick: Option<Tick>) -> Result<Ingested, Violation> {
        let phase = self.phase;
        let state = self.attempt.as_mut().expect("live attempt");
        match message {
            Message::Ping(stamp) => {
                state
                    .link
                    .on_ping(stamp, &self.limits, &mut self.diagnostics);
                Ok(Ingested::Accepted)
            }
            Message::Pong(stamp) => {
                state.link.on_pong(stamp, self.now, &mut self.diagnostics);
                Ok(Ingested::Accepted)
            }
            Message::Close(code) => {
                let reason = CloseReason::RemoteClosed(code);
                self.finalize(reason, true);
                Ok(Ingested::Closed(reason))
            }
            Message::Accept(accept) if phase == Phase::Negotiating => self.on_accept(accept),
            Message::Reject(reason) if phase == Phase::Negotiating => {
                let reason = CloseReason::Rejected(reason);
                self.finalize(reason, true);
                Ok(Ingested::Closed(reason))
            }
            Message::SyncBegin(begin) if matches!(phase, Phase::Synchronizing | Phase::Active) => {
                self.on_sync_begin(begin, tick.unwrap_or_default())
            }
            Message::SyncChunk(chunk) if matches!(phase, Phase::Synchronizing | Phase::Active) => {
                self.on_sync_chunk(chunk)
            }
            Message::Activate if phase == Phase::Synchronizing && state.synced => {
                state.activation_tick = tick;
                self.try_activate();
                Ok(Ingested::Accepted)
            }
            Message::StateUpdate(update) => {
                if !state.synced {
                    self.diagnostics.drop(DropReason::UnexpectedPhase);
                    return Ok(Ingested::Dropped(DropReason::UnexpectedPhase));
                }
                self.on_state_update(update, tick.unwrap_or_default())
            }
            other => Err(Violation::UnexpectedMessage(other.kind())),
        }
    }

    fn on_accept(&mut self, accept: Accept) -> Result<Ingested, Violation> {
        if accept.peer == PeerId::HOST || accept.slots.len() != usize::from(self.requested_slots) {
            return Err(Violation::InvalidAccept);
        }
        let capacity = usize::from(self.budget.max_pending_commands);
        let state = self.attempt.as_mut().expect("live attempt");
        state.peer = Some(accept.peer);
        state.outboxes = accept
            .slots
            .iter()
            .map(|slot| CommandOutbox::new(*slot, accept.command_origin, capacity))
            .collect();
        state.deadline = self.now.after(self.budget.sync_timeout);
        self.phase = Phase::Synchronizing;
        self.events.push_back(ClientEvent::Accepted {
            peer: accept.peer,
            slots: accept.slots,
            extensions: accept.extensions,
        });
        let request = Message::SnapshotRequest(SnapshotRequest { held: None });
        if !self.send_reliable(&request, None) {
            return Ok(Ingested::Closed(CloseReason::QueueOverflow));
        }
        Ok(Ingested::Accepted)
    }

    fn on_sync_begin(&mut self, begin: SyncBegin, tick: Tick) -> Result<Ingested, Violation> {
        let state = self.attempt.as_mut().expect("live attempt");
        if state.incoming.is_some() {
            return Err(Violation::Transfer("transfer already in progress"));
        }
        let total_bytes = begin.total_bytes as usize;
        let chunk_count = usize::from(begin.chunk_count);
        if total_bytes > self.limits.replication.max_snapshot_bytes {
            return Err(Violation::Delta(crate::replication::DeltaError::Snapshot(
                SnapshotError::SnapshotTooLarge {
                    bytes: total_bytes,
                    max: self.limits.replication.max_snapshot_bytes,
                },
            )));
        }
        if chunk_count == 0 || chunk_count > self.limits.max_chunks.max(1) {
            return Err(Violation::Transfer("chunk count"));
        }
        state.incoming = Some(IncomingTransfer {
            snapshot: begin.snapshot,
            tick,
            total_bytes,
            chunk_count,
            next_chunk: 0,
            // Bounded by the snapshot budget checked above.
            bytes: Vec::with_capacity(total_bytes),
        });
        Ok(Ingested::Accepted)
    }

    fn on_sync_chunk(&mut self, chunk: SyncChunk<'_>) -> Result<Ingested, Violation> {
        let state = self.attempt.as_mut().expect("live attempt");
        let Some(incoming) = state.incoming.as_mut() else {
            return Err(Violation::Transfer("chunk without a transfer"));
        };
        if chunk.snapshot != incoming.snapshot || usize::from(chunk.index) != incoming.next_chunk {
            return Err(Violation::Transfer("chunk order"));
        }
        if incoming.bytes.len() + chunk.data.len() > incoming.total_bytes {
            return Err(Violation::Transfer("chunk overruns the declared size"));
        }
        incoming.bytes.extend_from_slice(chunk.data);
        incoming.next_chunk += 1;
        if incoming.next_chunk < incoming.chunk_count {
            return Ok(Ingested::Accepted);
        }
        let incoming = state.incoming.take().expect("transfer present");
        if incoming.bytes.len() != incoming.total_bytes {
            return Err(Violation::Transfer("size mismatch"));
        }
        self.diagnostics.full_transfers_completed += 1;
        self.diagnostics.full_transfer_bytes += incoming.total_bytes as u64;
        let snapshot = apply_delta(None, &incoming.bytes, &self.limits.replication)
            .map_err(Violation::Delta)?;
        self.accept_snapshot(incoming.snapshot, incoming.tick, snapshot, None, Vec::new())
    }

    fn on_state_update(
        &mut self,
        update: StateUpdate<'_>,
        tick: Tick,
    ) -> Result<Ingested, Violation> {
        let state = self.attempt.as_mut().expect("live attempt");
        if state
            .latest
            .as_ref()
            .is_some_and(|latest| !update.snapshot.is_newer_than(latest.id))
        {
            self.diagnostics.drop(DropReason::Stale);
            return Ok(Ingested::Dropped(DropReason::Stale));
        }
        let baseline = match update.baseline {
            None => None,
            Some(id) => {
                let held = state
                    .history
                    .get(id)
                    .or(state.pinned.as_ref().filter(|stored| stored.id == id))
                    .cloned();
                let Some(held) = held else {
                    self.diagnostics.drop(DropReason::MissingBaseline);
                    self.request_recovery();
                    return Ok(Ingested::Dropped(DropReason::MissingBaseline));
                };
                Some(held)
            }
        };
        let snapshot = apply_delta(
            baseline.as_ref().map(|stored| &*stored.state),
            update.delta,
            &self.limits.replication,
        )
        .map_err(Violation::Delta)?;
        self.diagnostics.state_updates_applied += 1;
        self.accept_snapshot(update.snapshot, tick, snapshot, baseline, update.acks)
    }

    /// Store a reconstructed snapshot and report it to the application.
    fn accept_snapshot(
        &mut self,
        id: SnapshotId,
        tick: Tick,
        snapshot: Snapshot,
        baseline: Option<StoredSnapshot>,
        acks: Vec<CommandAck>,
    ) -> Result<Ingested, Violation> {
        let state = self.attempt.as_mut().expect("live attempt");
        if let Some(latest) = &state.latest
            && !id.is_newer_than(latest.id)
        {
            self.diagnostics.drop(DropReason::Stale);
            return Ok(Ingested::Dropped(DropReason::Stale));
        }
        state
            .floors
            .advance(
                state.latest.as_ref().map(|stored| &*stored.state),
                &snapshot,
            )
            .map_err(|error| match error {
                SnapshotError::GenerationRegression(entity) => Violation::StaleGeneration(entity),
                other => Violation::Delta(crate::replication::DeltaError::Snapshot(other)),
            })?;
        for ack in &acks {
            let Some(outbox) = state
                .outboxes
                .iter_mut()
                .find(|outbox| outbox.slot == ack.slot)
            else {
                return Err(Violation::UnownedSlot(ack.slot));
            };
            outbox
                .acknowledge(ack.received_next)
                .map_err(|()| Violation::CommandAckAhead)?;
        }
        let stored = StoredSnapshot {
            id,
            tick,
            state: Arc::new(snapshot),
        };
        state.history.push(stored.clone());
        if baseline.is_some() {
            state.pinned = baseline;
        }
        state.latest = Some(stored.clone());
        state.recovery_requested = None;
        let event = if state.synced || state.pending_apply.is_some() {
            ClientEvent::Snapshot {
                id,
                tick,
                state: stored.state,
                acks,
            }
        } else {
            state.pending_apply = Some(id);
            ClientEvent::SyncSnapshot {
                id,
                tick,
                state: stored.state,
            }
        };
        self.events.push_back(event);
        Ok(Ingested::Accepted)
    }

    /// Ask for a full state, at most once per retry interval.
    fn request_recovery(&mut self) {
        let state = self.attempt.as_mut().expect("live attempt");
        if state.incoming.is_some()
            || state
                .recovery_requested
                .is_some_and(|at| self.now < at.after(self.budget.recovery_retry))
        {
            return;
        }
        state.recovery_requested = Some(self.now);
        self.diagnostics.recovery_requests += 1;
        let held = state.latest.as_ref().map(|stored| stored.id);
        self.send_reliable(&Message::SnapshotRequest(SnapshotRequest { held }), None);
    }

    /// Become active once the application has applied a snapshot at or
    /// after the host's accepted tick.
    fn try_activate(&mut self) {
        if self.phase != Phase::Synchronizing {
            return;
        }
        let Some(state) = self.attempt.as_ref() else {
            return;
        };
        let (Some(target), Some(applied)) = (state.activation_tick, state.applied_tick) else {
            return;
        };
        if !state.synced || applied < target {
            return;
        }
        if self.send_reliable(&Message::Ready, Some(applied)) {
            self.phase = Phase::Active;
            self.events
                .push_back(ClientEvent::Activated { tick: applied });
        }
    }

    /// Send the periodic input datagram: the baseline acknowledgement and
    /// every unacknowledged command that fits.
    fn flush_input(&mut self) {
        let Some(state) = self.attempt.as_mut() else {
            return;
        };
        let sending = matches!(self.phase, Phase::Synchronizing | Phase::Active) && state.synced;
        if !sending || self.now < state.next_input {
            return;
        }
        state.next_input = self.now.after(self.budget.input_interval);
        let ack = state.latest.as_ref().map(|stored| stored.id);

        // The earliest pending tick is the base every offset is relative to.
        let base = state
            .outboxes
            .iter()
            .filter_map(|outbox| outbox.pending().front())
            .map(|command| command.tick)
            .min();
        let Some(base) = base else {
            if let Some(ack) = ack {
                let _ = state.link.send(
                    &Message::StateAck(ack),
                    None,
                    &self.limits,
                    &mut self.diagnostics,
                );
            }
            return;
        };

        // Take one command per slot in turn so no slot starves another.
        let mut remaining = input_capacity(&self.limits) - 6;
        let mut counts = vec![0usize; state.outboxes.len()];
        loop {
            let mut progressed = false;
            for (index, outbox) in state.outboxes.iter().enumerate() {
                let Some(command) = outbox.pending().get(counts[index]) else {
                    continue;
                };
                let group_header = if counts[index] == 0 { 6 } else { 0 };
                let cost = group_header + 4 + command.payload.len();
                let offset = command.tick.checked_sub(base).unwrap_or(u64::MAX);
                if cost > remaining || offset > u64::from(u16::MAX) || counts[index] == 255 {
                    continue;
                }
                remaining -= cost;
                counts[index] += 1;
                progressed = true;
            }
            if !progressed {
                break;
            }
        }
        let groups: Vec<CommandGroup<'_>> = state
            .outboxes
            .iter()
            .zip(&counts)
            .filter(|(_, count)| **count > 0)
            .map(|(outbox, count)| CommandGroup {
                slot: outbox.slot,
                first: outbox.pending()[0].sequence,
                commands: outbox
                    .pending()
                    .iter()
                    .take(*count)
                    .map(|command| WireCommand {
                        // Bounded to `u16` by the packing loop.
                        tick_offset: (command.tick.get() - base.get()) as u16,
                        payload: &command.payload,
                    })
                    .collect(),
            })
            .collect();
        let batch = Message::InputBatch(InputBatch { ack, groups });
        let result: Result<(), QueueOverflow> =
            state
                .link
                .send(&batch, Some(base), &self.limits, &mut self.diagnostics);
        // Unreliable: a full queue was already counted as a drop.
        let _ = result;
    }
}
