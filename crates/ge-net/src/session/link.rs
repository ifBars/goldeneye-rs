//! Per-connection machinery shared by the host and client.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::time::Duration;

use super::{DrainLimit, DrainReport, Outbound, Violation};
use crate::budget::{Limits, SessionBudget};
use crate::channel::{Admission, ChannelId, Delivery, InboundGate, Lane, OutboundQueues};
use crate::diagnostics::{Diagnostics, DropReason};
use crate::identity::{AttemptId, Sequence, SessionEpoch, Tick};
use crate::protocol::envelope::{self, BASE_HEADER_BYTES, Fragment, Header, TICK_BYTES};
use crate::protocol::message::{KindClass, Message, MessageKind, classify};
use crate::time::NetInstant;
use crate::transport::SendOutcome;

/// A reliable message could not be queued. The connection must close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct QueueOverflow;

/// What one inbound envelope turned out to be.
#[derive(Debug)]
pub(crate) enum Received<'a> {
    Message {
        kind: MessageKind,
        tick: Option<Tick>,
        payload: Cow<'a, [u8]>,
    },
    /// A fragment was stored.
    Pending,
    Dropped(DropReason),
    Violation(Violation),
}

/// Smoothed round-trip time from probe echoes, in integer microseconds.
#[derive(Debug)]
pub(crate) struct RttEstimator {
    smoothed: Option<u64>,
    deviation: u64,
    samples: VecDeque<u32>,
    capacity: usize,
}

impl RttEstimator {
    fn new(capacity: usize) -> Self {
        Self {
            smoothed: None,
            deviation: 0,
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn sample(&mut self, micros: u64) {
        match self.smoothed {
            None => {
                self.smoothed = Some(micros);
                self.deviation = micros / 2;
            }
            Some(smoothed) => {
                let error = smoothed.abs_diff(micros);
                self.deviation = (self.deviation * 3 + error) / 4;
                self.smoothed = Some((smoothed * 7 + micros) / 8);
            }
        }
        if self.capacity > 0 {
            if self.samples.len() == self.capacity {
                self.samples.pop_front();
            }
            self.samples
                .push_back(u32::try_from(micros).unwrap_or(u32::MAX));
        }
    }

    pub(crate) fn smoothed(&self) -> Option<u64> {
        self.smoothed
    }

    pub(crate) fn deviation(&self) -> u64 {
        self.deviation
    }

    pub(crate) fn samples(&self) -> Vec<u32> {
        self.samples.iter().copied().collect()
    }
}

/// Shared budget for one `drain_outbound` call.
pub(crate) struct DrainState {
    remaining_messages: usize,
    remaining_bytes: usize,
    pub report: DrainReport,
}

impl DrainState {
    pub(crate) fn new(limit: DrainLimit) -> Self {
        Self {
            remaining_messages: limit.max_messages,
            remaining_bytes: limit.max_bytes,
            report: DrainReport::default(),
        }
    }
}

/// Sequencing, queues and liveness of one connection.
#[derive(Debug)]
pub(crate) struct Link {
    pub attempt: AttemptId,
    pub epoch: SessionEpoch,
    pub out: OutboundQueues,
    gates: [InboundGate; ChannelId::COUNT],
    created: NetInstant,
    pub last_heard: NetInstant,
    next_ping: NetInstant,
    pub rtt: RttEstimator,
    skipped_optional: u32,
    /// The transport reported the connection gone during a drain.
    pub transport_gone: bool,
    scratch: Vec<u8>,
}

impl Link {
    pub(crate) fn new(
        attempt: AttemptId,
        now: NetInstant,
        budget: &SessionBudget,
        limits: &Limits,
        sequence_origin: Sequence,
    ) -> Self {
        let mut out = OutboundQueues::new(budget.max_queued_bytes_per_peer as usize);
        out.set_initial_sequence(sequence_origin);
        let gates = ChannelId::ALL.map(|channel| {
            let mut gate = InboundGate::new(
                channel.delivery(),
                budget.max_update_fragments,
                limits.max_update_bytes,
            );
            gate.set_initial_sequence(sequence_origin);
            gate
        });
        Self {
            attempt,
            epoch: SessionEpoch::UNASSIGNED,
            out,
            gates,
            created: now,
            last_heard: now,
            next_ping: now.after(budget.ping_interval),
            rtt: RttEstimator::new(usize::from(budget.max_rtt_samples)),
            skipped_optional: 0,
            transport_gone: false,
            scratch: Vec::new(),
        }
    }

    /// Queue one message, fragmenting an oversized state update.
    ///
    /// An unreliable message that cannot be queued is dropped and counted.
    /// A reliable one that cannot be queued is an error.
    pub(crate) fn send(
        &mut self,
        message: &Message<'_>,
        tick: Option<Tick>,
        limits: &Limits,
        diagnostics: &mut Diagnostics,
    ) -> Result<(), QueueOverflow> {
        let kind = message.kind();
        let channel = kind.channel();
        debug_assert_eq!(kind.has_tick(), tick.is_some());
        let reliable = channel.delivery() == Delivery::ReliableOrdered;

        let mut payload = std::mem::take(&mut self.scratch);
        payload.clear();
        let encoded = message.encode_payload(&mut payload);
        let result = if encoded.is_err() {
            Err(QueueOverflow)
        } else {
            self.send_payload(kind, tick, &payload, limits, diagnostics)
        };
        self.scratch = payload;
        diagnostics.queue_high_water(self.out.total_bytes());
        match result {
            Err(QueueOverflow) if !reliable => {
                diagnostics.drop(DropReason::QueueFull);
                Ok(())
            }
            other => other,
        }
    }

    fn send_payload(
        &mut self,
        kind: MessageKind,
        tick: Option<Tick>,
        payload: &[u8],
        limits: &Limits,
        diagnostics: &mut Diagnostics,
    ) -> Result<(), QueueOverflow> {
        let channel = kind.channel();
        if channel.replaceable() {
            let replaced = self.out.clear_channel(channel);
            diagnostics.drop_many(DropReason::Replaced, replaced as u64);
        }
        let mut header = Header {
            kind: kind.wire(),
            channel,
            epoch: self.epoch,
            sequence: self.out.allocate_sequence(channel),
            tick,
            fragment: None,
        };
        let max = match channel.lane() {
            Lane::Datagram => limits.datagram_bytes,
            Lane::Control | Lane::Bulk => limits.reliable_bytes,
        };
        let mut envelopes = Vec::new();
        if header.encoded_len() + payload.len() <= max {
            envelopes.push(encode(&header, payload, max)?);
        } else {
            if !kind.fragmentable() {
                return Err(QueueOverflow);
            }
            let count = payload.len().div_ceil(limits.fragment_payload);
            let Ok(count) = u8::try_from(count) else {
                return Err(QueueOverflow);
            };
            for (index, part) in payload.chunks(limits.fragment_payload).enumerate() {
                header.fragment = Some(Fragment {
                    index: index as u8,
                    count,
                });
                envelopes.push(encode(&header, part, max)?);
            }
        }
        // Every fragment, or none: a partial update is useless to the peer.
        let evicted = self
            .out
            .push_all(channel, envelopes)
            .map_err(|_| QueueOverflow)?;
        diagnostics.drop_many(DropReason::QueueEvicted, evicted as u64);
        Ok(())
    }

    /// Decode one envelope and run the lane, epoch and sequence checks.
    pub(crate) fn receive<'a>(
        &mut self,
        lane: Lane,
        bytes: &'a [u8],
        now: NetInstant,
        budget: &SessionBudget,
        limits: &Limits,
        diagnostics: &mut Diagnostics,
    ) -> Received<'a> {
        let max = match lane {
            Lane::Datagram => limits.datagram_bytes,
            Lane::Control | Lane::Bulk => limits.reliable_bytes,
        };
        let envelope = match envelope::decode(bytes, max) {
            Ok(envelope) => envelope,
            Err(error) => return Received::Violation(Violation::Decode(error)),
        };
        let header = envelope.header;
        if header.channel.lane() != lane {
            return Received::Violation(Violation::LaneMismatch);
        }
        let class = match classify(&header) {
            Ok(class) => class,
            Err(error) => return Received::Violation(Violation::Decode(error)),
        };
        // A datagram from another epoch is expected debris from an earlier
        // connection and must not move the sequence gate. A reliable lane
        // belongs to one connection, so its order is checked first and a
        // foreign epoch there is a violation.
        let epoch_accepted = self.epoch_accepted(&header, class);
        if lane == Lane::Datagram && !epoch_accepted {
            diagnostics.drop(DropReason::EpochMismatch);
            return Received::Dropped(DropReason::EpochMismatch);
        }
        self.last_heard = now;

        let gate = &mut self.gates[header.channel.index()];
        let abandoned_before = gate.abandoned();
        let admission = gate.admit(&header, envelope.payload);
        diagnostics.drop_many(
            DropReason::PartialAbandoned,
            gate.abandoned() - abandoned_before,
        );
        let payload = match admission {
            Admission::Deliver(payload) => payload,
            Admission::Pending => {
                diagnostics.received(header.channel, bytes.len());
                return Received::Pending;
            }
            Admission::Drop(reason) => {
                diagnostics.drop(reason);
                return Received::Dropped(reason);
            }
            Admission::Violation(violation) => return Received::Violation(violation),
        };
        if !epoch_accepted {
            return Received::Violation(Violation::EpochMismatch);
        }
        diagnostics.received(header.channel, bytes.len());
        if class == KindClass::Known(MessageKind::Accept) && !self.epoch.is_assigned() {
            // The acceptance announces the epoch of this membership.
            self.epoch = header.epoch;
        }
        match class {
            KindClass::Known(kind) => Received::Message {
                kind,
                tick: header.tick,
                payload,
            },
            KindClass::UnknownOptional => {
                self.skipped_optional += 1;
                if self.skipped_optional > budget.max_skipped_optional_messages {
                    return Received::Violation(Violation::TooManySkippedOptional);
                }
                diagnostics.drop(DropReason::UnknownOptional);
                Received::Dropped(DropReason::UnknownOptional)
            }
        }
    }

    fn epoch_accepted(&self, header: &Header, class: KindClass) -> bool {
        if header.epoch == self.epoch {
            return true;
        }
        match class {
            // The acceptance itself announces the epoch.
            KindClass::Known(MessageKind::Accept) => {
                !self.epoch.is_assigned() && header.epoch.is_assigned()
            }
            // A goodbye is bound to the authenticated connection, and may be
            // sent before its sender learned the epoch.
            KindClass::Known(MessageKind::Close) => true,
            _ => false,
        }
    }

    /// Send a liveness probe when one is due.
    pub(crate) fn probe(
        &mut self,
        now: NetInstant,
        interval: Duration,
        limits: &Limits,
        diagnostics: &mut Diagnostics,
    ) {
        if now >= self.next_ping {
            self.next_ping = now.after(interval);
            // Unreliable: cannot overflow into an error.
            let _ = self.send(&Message::Ping(now.as_micros()), None, limits, diagnostics);
        }
    }

    pub(crate) fn on_ping(&mut self, stamp: u64, limits: &Limits, diagnostics: &mut Diagnostics) {
        let _ = self.send(&Message::Pong(stamp), None, limits, diagnostics);
    }

    pub(crate) fn on_pong(&mut self, stamp: u64, now: NetInstant, diagnostics: &mut Diagnostics) {
        if stamp < self.created.as_micros() || stamp > now.as_micros() {
            diagnostics.drop(DropReason::InvalidPong);
            return;
        }
        self.rtt.sample(now.as_micros() - stamp);
    }

    pub(crate) fn is_idle(&self, now: NetInstant, timeout: Duration) -> bool {
        now >= self.last_heard.after(timeout)
    }

    /// Hand queued envelopes of `channel` to the sink until it is empty,
    /// blocked or the caller's limit is reached.
    pub(crate) fn drain_channel(
        &mut self,
        channel: ChannelId,
        blocked: &mut [bool; 3],
        state: &mut DrainState,
        sink: &mut dyn FnMut(Outbound<'_>) -> SendOutcome,
        diagnostics: &mut Diagnostics,
    ) {
        let lane = channel.lane();
        let lane_index = match lane {
            Lane::Control => 0,
            Lane::Bulk => 1,
            Lane::Datagram => 2,
        };
        while let Some(bytes) = self.out.front(channel) {
            if blocked[lane_index] || self.transport_gone {
                return;
            }
            if state.remaining_messages == 0 || bytes.len() > state.remaining_bytes {
                state.report.limited = true;
                return;
            }
            let outcome = sink(Outbound::Message {
                attempt: self.attempt,
                lane,
                bytes,
            });
            match outcome {
                SendOutcome::Sent => {
                    let len = bytes.len();
                    state.remaining_messages -= 1;
                    state.remaining_bytes -= len;
                    state.report.messages += 1;
                    state.report.bytes += len;
                    diagnostics.sent(channel, len);
                    self.out.pop(channel);
                }
                SendOutcome::Blocked => {
                    blocked[lane_index] = true;
                    state.report.blocked = true;
                }
                SendOutcome::Closed => self.transport_gone = true,
            }
        }
    }
}

fn encode(header: &Header, payload: &[u8], max: usize) -> Result<Vec<u8>, QueueOverflow> {
    let mut bytes = Vec::with_capacity(header.encoded_len() + payload.len());
    envelope::encode(header, payload, max, &mut bytes).map_err(|_| QueueOverflow)?;
    Ok(bytes)
}

/// Maximum command bytes one input datagram can carry after its headers.
pub(crate) fn input_capacity(limits: &Limits) -> usize {
    limits.datagram_bytes - BASE_HEADER_BYTES - TICK_BYTES
}
