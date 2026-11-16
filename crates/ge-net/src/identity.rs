//! Distinct identities for attempts, peers, epochs, slots, entities, command
//! sequences and snapshots.
//!
//! None of these is a game-owner handle. A game adapter maps them to its own
//! objects; component-local IDs never cross the wire.

use std::collections::VecDeque;
use std::fmt;

/// Largest forward distance at which a wrapping `u32` counts as newer.
///
/// The window is one less than half the numeric range, so "newer" and "older"
/// are never both true for a pair of distinct values.
pub const SERIAL_WINDOW: u32 = (1 << 31) - 1;

/// Serial-number comparison: is `candidate` strictly newer than `reference`?
pub const fn serial_newer(candidate: u32, reference: u32) -> bool {
    let distance = candidate.wrapping_sub(reference);
    distance != 0 && distance <= SERIAL_WINDOW
}

macro_rules! serial_id {
    ($(#[$meta:meta])* $name:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u32 {
                self.0
            }

            /// The following value; wraps at `u32::MAX`.
            pub const fn next(self) -> Self {
                Self(self.0.wrapping_add(1))
            }

            /// Wrapping offset from this value.
            pub const fn offset(self, by: u32) -> Self {
                Self(self.0.wrapping_add(by))
            }

            /// Serial comparison within [`SERIAL_WINDOW`].
            pub const fn is_newer_than(self, other: Self) -> bool {
                serial_newer(self.0, other.0)
            }

            /// Wrapping distance from `earlier` to `self`.
            pub const fn distance_from(self, earlier: Self) -> u32 {
                self.0.wrapping_sub(earlier.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($label, "#{}"), self.0)
            }
        }
    };
}

serial_id!(
    /// Per-channel, per-direction logical message sequence.
    Sequence,
    "seq"
);
serial_id!(
    /// Per-slot command sequence assigned by the submitting client.
    CommandSeq,
    "cmd"
);
serial_id!(
    /// Authority-assigned identifier of one published snapshot.
    SnapshotId,
    "snapshot"
);

/// One transport connection attempt, as identified by the transport adapter.
///
/// Values must be strictly increasing for the lifetime of a session object.
/// A closed or cancelled attempt is never reused, so its delayed transport
/// events are recognisably stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptId(u64);

impl AttemptId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for AttemptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attempt#{}", self.0)
    }
}

/// A session member, assigned by the host after negotiation succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(u16);

impl PeerId {
    /// The authority itself.
    pub const HOST: Self = Self(0);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "peer#{}", self.0)
    }
}

/// The epoch of one accepted membership.
///
/// The host assigns a fresh nonzero epoch to every accepted attempt. A
/// reconnect therefore never shares an epoch with the connection it replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionEpoch(u32);

impl SessionEpoch {
    /// Carried before the host has assigned an epoch.
    pub const UNASSIGNED: Self = Self(0);

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub const fn is_assigned(self) -> bool {
        self.0 != 0
    }

    /// The next assignable epoch; skips the unassigned value on wraparound.
    pub const fn successor(self) -> Self {
        let next = self.0.wrapping_add(1);
        Self(if next == 0 { 1 } else { next })
    }
}

/// A player slot. One peer can own several; slot count is not peer count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerSlot(u8);

impl PlayerSlot {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for PlayerSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slot#{}", self.0)
    }
}

/// A simulation tick. Arithmetic is checked; ticks never wrap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);

impl Tick {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn checked_add(self, ticks: u64) -> Option<Self> {
        match self.0.checked_add(ticks) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn checked_sub(self, earlier: Self) -> Option<u64> {
        self.0.checked_sub(earlier.0)
    }
}

/// A replicated entity in the authority's namespace.
///
/// Only the authority allocates these. `generation` distinguishes successive
/// occupants of one `index`, so a delayed update cannot reach a replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId {
    pub index: u16,
    pub generation: u32,
}

impl EntityId {
    pub const fn new(index: u16, generation: u32) -> Self {
        Self { index, generation }
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity#{}.{}", self.index, self.generation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityAllocError {
    /// Every index is live or retired.
    Exhausted,
    /// The entity is not the live occupant of its index.
    NotLive(EntityId),
}

impl fmt::Display for EntityAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => write!(f, "entity namespace exhausted"),
            Self::NotLive(id) => write!(f, "{id} is not live"),
        }
    }
}

impl std::error::Error for EntityAllocError {}

/// Authority-side allocator for [`EntityId`] values.
///
/// Freed indices are reused oldest-first with the generation advanced. An
/// index whose generation would overflow is retired instead of wrapping.
#[derive(Clone, Debug)]
pub struct EntityAllocator {
    capacity: u16,
    generations: Vec<u32>,
    live: Vec<bool>,
    free: VecDeque<u16>,
}

impl EntityAllocator {
    pub fn new(capacity: u16) -> Self {
        Self {
            capacity,
            generations: Vec::new(),
            live: Vec::new(),
            free: VecDeque::new(),
        }
    }

    pub fn capacity(&self) -> u16 {
        self.capacity
    }

    pub fn live_count(&self) -> usize {
        self.live.iter().filter(|live| **live).count()
    }

    pub fn allocate(&mut self) -> Result<EntityId, EntityAllocError> {
        if let Some(index) = self.free.pop_front() {
            self.live[usize::from(index)] = true;
            return Ok(EntityId::new(index, self.generations[usize::from(index)]));
        }
        let next = self.generations.len();
        if next >= usize::from(self.capacity) {
            return Err(EntityAllocError::Exhausted);
        }
        self.generations.push(0);
        self.live.push(true);
        // `next` is below a `u16` capacity.
        Ok(EntityId::new(next as u16, 0))
    }

    pub fn free(&mut self, id: EntityId) -> Result<(), EntityAllocError> {
        let slot = usize::from(id.index);
        if self.live.get(slot) != Some(&true) || self.generations[slot] != id.generation {
            return Err(EntityAllocError::NotLive(id));
        }
        self.live[slot] = false;
        if let Some(next) = id.generation.checked_add(1) {
            self.generations[slot] = next;
            self.free.push_back(id.index);
        }
        Ok(())
    }

    pub fn is_live(&self, id: EntityId) -> bool {
        let slot = usize::from(id.index);
        self.live.get(slot) == Some(&true) && self.generations[slot] == id.generation
    }
}

/// Session randomness, separate from any gameplay RNG.
///
/// Tests supply a seeded source. A real adapter supplies an operating-system
/// or cryptographic source. The core draws from it only for session epochs.
pub trait EntropySource {
    fn next_u64(&mut self) -> u64;
}

impl<F: FnMut() -> u64> EntropySource for F {
    fn next_u64(&mut self) -> u64 {
        self()
    }
}
