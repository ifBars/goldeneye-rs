//! Delivery contracts, channel identities, bounded queues and sequence gates.
//!
//! Version one defines five channels over three transport lanes:
//!
//! | Channel | Lane | Contract | Traffic |
//! | --- | --- | --- | --- |
//! | Control | Control | Reliable ordered | Negotiation, membership, close |
//! | Bulk | Bulk | Reliable ordered | Chunked full snapshots |
//! | Input | Datagram | Unreliable sequenced | Command batches, baseline acks |
//! | State | Datagram | Unreliable sequenced | Authoritative state updates |
//! | Probe | Datagram | Unreliable sequenced | Liveness and round-trip probes |
//!
//! The two reliable lanes are independent, so a snapshot transfer never
//! blocks a control message. This crate adds no retransmission of its own to
//! a reliable lane.

mod gate;
mod queue;
#[cfg(test)]
mod tests;

pub(crate) use gate::{Admission, InboundGate};
pub(crate) use queue::OutboundQueues;

/// What a lane promises about delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Delivery {
    /// Every message arrives once, in order, or the connection fails.
    ReliableOrdered,
    /// Messages may be lost, duplicated or reordered. The receiver discards
    /// any that is not newer than the last one it accepted.
    UnreliableSequenced,
}

/// A transport lane. A backend must provide all three or declare otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lane {
    /// Reliable ordered lane for control traffic.
    Control,
    /// Reliable ordered lane for bulk transfer, independent of `Control`.
    Bulk,
    /// Unreliable, unordered, bounded-size messages.
    Datagram,
}

impl Lane {
    pub const fn delivery(self) -> Delivery {
        match self {
            Self::Control | Self::Bulk => Delivery::ReliableOrdered,
            Self::Datagram => Delivery::UnreliableSequenced,
        }
    }
}

/// A protocol channel. Each has its own sequence space in each direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChannelId {
    Control = 0,
    Bulk = 1,
    Input = 2,
    State = 3,
    Probe = 4,
}

impl ChannelId {
    pub const COUNT: usize = 5;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Control,
        Self::Bulk,
        Self::Input,
        Self::State,
        Self::Probe,
    ];

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Control),
            1 => Some(Self::Bulk),
            2 => Some(Self::Input),
            3 => Some(Self::State),
            4 => Some(Self::Probe),
            _ => None,
        }
    }

    pub const fn wire(self) -> u8 {
        self as u8
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn lane(self) -> Lane {
        match self {
            Self::Control => Lane::Control,
            Self::Bulk => Lane::Bulk,
            Self::Input | Self::State | Self::Probe => Lane::Datagram,
        }
    }

    pub const fn delivery(self) -> Delivery {
        self.lane().delivery()
    }

    /// Whether a newer queued message makes older unsent ones worthless.
    pub(crate) const fn replaceable(self) -> bool {
        matches!(self, Self::Input | Self::State)
    }
}
