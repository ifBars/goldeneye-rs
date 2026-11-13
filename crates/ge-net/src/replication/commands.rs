//! Ticked commands: client outbox and host inbox.
//!
//! A client numbers the commands of each slot consecutively and resends
//! every unacknowledged one in each input datagram. The host accepts each
//! sequence once, in order, and releases commands per tick in a stable order
//! that does not depend on packet arrival. Transporting commands does not
//! validate them: the application checks game rules before executing one.

use std::collections::VecDeque;
use std::fmt;

use crate::identity::{AttemptId, CommandSeq, PeerId, PlayerSlot, Tick};

/// What the host has done with one slot's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandAck {
    pub slot: PlayerSlot,
    /// Every earlier sequence was received. The client stops resending them.
    pub received_next: CommandSeq,
    /// Every earlier sequence was released to the simulation or discarded
    /// before the acknowledged snapshot. A prediction layer reconciles here.
    pub processed_next: CommandSeq,
}

/// A command released to the authority for execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedCommand {
    pub attempt: AttemptId,
    pub peer: PeerId,
    pub slot: PlayerSlot,
    pub sequence: CommandSeq,
    pub tick: Tick,
    pub payload: Vec<u8>,
}

/// Why a client could not queue a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// Commands are accepted only while the session is active.
    NotActive,
    /// The slot is not assigned to this client.
    UnknownSlot(PlayerSlot),
    /// Too many unacknowledged commands; retry after acknowledgements arrive.
    OutboxFull,
    CommandTooLarge {
        len: usize,
        max: usize,
    },
    /// The tick is earlier than this slot's previous command.
    TickRegression,
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotActive => write!(f, "session is not active"),
            Self::UnknownSlot(slot) => write!(f, "{slot} is not assigned to this client"),
            Self::OutboxFull => write!(f, "command outbox is full"),
            Self::CommandTooLarge { len, max } => {
                write!(f, "command of {len} bytes exceeds limit {max}")
            }
            Self::TickRegression => write!(f, "command tick precedes the previous command"),
        }
    }
}

impl std::error::Error for SubmitError {}

#[derive(Debug)]
pub(crate) struct PendingCommand {
    pub sequence: CommandSeq,
    pub tick: Tick,
    pub payload: Vec<u8>,
}

/// Unacknowledged commands of one local slot.
#[derive(Debug)]
pub(crate) struct CommandOutbox {
    pub slot: PlayerSlot,
    next: CommandSeq,
    pending: VecDeque<PendingCommand>,
    last_tick: Option<Tick>,
    capacity: usize,
}

impl CommandOutbox {
    pub(crate) fn new(slot: PlayerSlot, origin: CommandSeq, capacity: usize) -> Self {
        Self {
            slot,
            next: origin,
            pending: VecDeque::new(),
            last_tick: None,
            capacity,
        }
    }

    pub(crate) fn submit(&mut self, tick: Tick, payload: &[u8]) -> Result<CommandSeq, SubmitError> {
        if self.pending.len() >= self.capacity {
            return Err(SubmitError::OutboxFull);
        }
        if self.last_tick.is_some_and(|last| tick < last) {
            return Err(SubmitError::TickRegression);
        }
        let sequence = self.next;
        self.next = sequence.next();
        self.last_tick = Some(tick);
        self.pending.push_back(PendingCommand {
            sequence,
            tick,
            payload: payload.to_vec(),
        });
        Ok(sequence)
    }

    pub(crate) fn pending(&self) -> &VecDeque<PendingCommand> {
        &self.pending
    }

    /// Drop commands the host has received. `Err` means the host
    /// acknowledged a sequence this client never sent.
    pub(crate) fn acknowledge(&mut self, received_next: CommandSeq) -> Result<(), ()> {
        let oldest = self.next.offset((self.pending.len() as u32).wrapping_neg());
        let acknowledged = received_next.distance_from(oldest) as usize;
        if acknowledged <= self.pending.len() {
            self.pending.drain(..acknowledged);
            Ok(())
        } else if oldest.is_newer_than(received_next) {
            // A reordered, older acknowledgement.
            Ok(())
        } else {
            Err(())
        }
    }
}

/// What one offered command group did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InboxReport {
    pub accepted: u64,
    pub duplicates: u64,
    pub late: u64,
    pub future: u64,
    pub full: u64,
    pub gap: u64,
    /// The stream's ticks went backwards; the peer broke the protocol.
    pub tick_regression: bool,
}

/// Commands one slot's owner has sent and the host has not yet released.
#[derive(Debug)]
pub(crate) struct CommandInbox {
    received_next: CommandSeq,
    processed_next: CommandSeq,
    pending: VecDeque<PendingCommand>,
    last_tick: Option<Tick>,
    capacity: usize,
}

impl CommandInbox {
    pub(crate) fn new(origin: CommandSeq, capacity: usize) -> Self {
        Self {
            received_next: origin,
            processed_next: origin,
            pending: VecDeque::new(),
            last_tick: None,
            capacity,
        }
    }

    pub(crate) fn ack(&self, slot: PlayerSlot) -> CommandAck {
        CommandAck {
            slot,
            received_next: self.received_next,
            processed_next: self.processed_next,
        }
    }

    /// Offer consecutive commands starting at `first`.
    ///
    /// `consumed` is the last tick the application released. A command at or
    /// before it is late. A command more than `future_ticks` after it waits
    /// for a later resend.
    pub(crate) fn offer<'a>(
        &mut self,
        first: CommandSeq,
        commands: impl Iterator<Item = (Tick, &'a [u8])>,
        consumed: Option<Tick>,
        future_ticks: u64,
    ) -> InboxReport {
        let mut report = InboxReport::default();
        let horizon = match consumed {
            Some(tick) => tick.checked_add(future_ticks),
            None => Some(Tick::new(future_ticks)),
        };
        for (offset, (tick, payload)) in commands.enumerate() {
            let sequence = first.offset(offset as u32);
            if sequence != self.received_next {
                if self.received_next.is_newer_than(sequence) {
                    report.duplicates += 1;
                    continue;
                }
                report.gap += 1;
                break;
            }
            if self.last_tick.is_some_and(|last| tick < last) {
                report.tick_regression = true;
                break;
            }
            if horizon.is_some_and(|horizon| tick > horizon) {
                report.future += 1;
                break;
            }
            if consumed.is_some_and(|consumed| tick <= consumed) {
                // Nothing earlier can still be pending: ticks never decrease.
                self.last_tick = Some(tick);
                self.received_next = sequence.next();
                self.processed_next = self.received_next;
                report.late += 1;
                continue;
            }
            if self.pending.len() >= self.capacity {
                report.full += 1;
                break;
            }
            self.last_tick = Some(tick);
            self.received_next = sequence.next();
            self.pending.push_back(PendingCommand {
                sequence,
                tick,
                payload: payload.to_vec(),
            });
            report.accepted += 1;
        }
        report
    }

    /// Release the commands for `tick`. Returns how many earlier commands
    /// were discarded because the application skipped their tick.
    pub(crate) fn take(&mut self, tick: Tick, mut release: impl FnMut(PendingCommand)) -> u64 {
        let mut missed = 0;
        while let Some(command) = self.pending.pop_front_if(|command| command.tick <= tick) {
            self.processed_next = command.sequence.next();
            if command.tick == tick {
                release(command);
            } else {
                missed += 1;
            }
        }
        missed
    }
}
