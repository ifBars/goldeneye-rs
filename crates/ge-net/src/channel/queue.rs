//! Bounded per-connection outbound queues.

use std::collections::VecDeque;

use super::{ChannelId, Delivery};
use crate::identity::Sequence;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PushError {
    /// The byte budget cannot hold the message.
    Overflow,
}

/// Encoded envelopes awaiting the transport, with one byte budget shared by
/// every channel of a connection.
///
/// Any message first evicts older unreliable messages to make room; reliable
/// messages are never evicted. A reliable message that still does not fit is
/// an error for the caller to turn into an explicit connection failure.
#[derive(Debug)]
pub(crate) struct OutboundQueues {
    queues: [VecDeque<Vec<u8>>; ChannelId::COUNT],
    bytes: [usize; ChannelId::COUNT],
    next_sequence: [Sequence; ChannelId::COUNT],
    max_bytes: usize,
    high_water: usize,
}

impl OutboundQueues {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            queues: Default::default(),
            bytes: [0; ChannelId::COUNT],
            next_sequence: [Sequence::default(); ChannelId::COUNT],
            max_bytes,
            high_water: 0,
        }
    }

    /// Start every channel's sequence at `start`. Used to exercise wraparound.
    pub(crate) fn set_initial_sequence(&mut self, start: Sequence) {
        self.next_sequence = [start; ChannelId::COUNT];
    }

    pub(crate) fn allocate_sequence(&mut self, channel: ChannelId) -> Sequence {
        let sequence = self.next_sequence[channel.index()];
        self.next_sequence[channel.index()] = sequence.next();
        sequence
    }

    pub(crate) fn total_bytes(&self) -> usize {
        self.bytes.iter().sum()
    }

    pub(crate) fn bytes(&self, channel: ChannelId) -> usize {
        self.bytes[channel.index()]
    }

    pub(crate) fn len(&self, channel: ChannelId) -> usize {
        self.queues[channel.index()].len()
    }

    pub(crate) fn high_water(&self) -> usize {
        self.high_water
    }

    pub(crate) fn has_room(&self, len: usize) -> bool {
        self.total_bytes().saturating_add(len) <= self.max_bytes
    }

    /// Queue one encoded envelope. Returns how many older unreliable
    /// messages were evicted to make room.
    pub(crate) fn push(
        &mut self,
        channel: ChannelId,
        message: Vec<u8>,
    ) -> Result<usize, PushError> {
        let len = message.len();
        let mut evicted = 0;
        if !self.has_room(len) {
            // Oldest first. An unreliable message starts with its own
            // channel; a reliable one evicts only unreliable messages.
            let own = (channel.delivery() == Delivery::UnreliableSequenced).then_some(channel);
            let others = [ChannelId::Probe, ChannelId::State, ChannelId::Input];
            for victim in own.into_iter().chain(others) {
                while !self.has_room(len) && self.pop(victim).is_some() {
                    evicted += 1;
                }
            }
            if !self.has_room(len) {
                return Err(PushError::Overflow);
            }
        }
        self.bytes[channel.index()] += len;
        self.queues[channel.index()].push_back(message);
        self.high_water = self.high_water.max(self.total_bytes());
        Ok(evicted)
    }

    pub(crate) fn front(&self, channel: ChannelId) -> Option<&[u8]> {
        self.queues[channel.index()].front().map(Vec::as_slice)
    }

    pub(crate) fn pop(&mut self, channel: ChannelId) -> Option<Vec<u8>> {
        let message = self.queues[channel.index()].pop_front()?;
        self.bytes[channel.index()] -= message.len();
        Some(message)
    }

    /// Drop every unsent message of `channel`; returns how many were dropped.
    pub(crate) fn clear_channel(&mut self, channel: ChannelId) -> usize {
        let dropped = self.queues[channel.index()].len();
        self.queues[channel.index()].clear();
        self.bytes[channel.index()] = 0;
        dropped
    }

    pub(crate) fn clear(&mut self) {
        for channel in ChannelId::ALL {
            self.clear_channel(channel);
        }
    }
}
