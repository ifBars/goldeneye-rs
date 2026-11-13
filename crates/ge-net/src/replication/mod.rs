//! Snapshots, acknowledged baselines, deltas and ticked commands.
//!
//! Replication is snapshot based. The authority publishes a complete
//! [`Snapshot`] per tick: an ordered set of entities, each with opaque state
//! bytes that only the application's adapter understands. The library
//! computes a per-peer delta against the newest snapshot that peer has
//! acknowledged holding, and the peer reconstructs the complete snapshot.
//!
//! Create, update and destroy are explicit operations inside a delta and are
//! validated against the baseline, so entity lifetime changes apply
//! atomically with the state they belong to. A delta whose baseline is not
//! held is never applied; the client asks for a bounded full-state recovery.

mod commands;
mod delta;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

pub use commands::{CommandAck, ReceivedCommand, SubmitError};
pub(crate) use commands::{CommandInbox, CommandOutbox};
pub use delta::{DeltaError, apply_delta, encode_delta};

use crate::identity::{EntityId, SnapshotId, Tick};

/// One entity and its opaque, adapter-encoded state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityRecord {
    pub id: EntityId,
    pub state: Vec<u8>,
}

/// Why a snapshot cannot be published or accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// Two records share an entity index.
    DuplicateIndex(u16),
    /// An entity index is not below the entity limit.
    IndexOutOfRange(u16),
    TooManyEntities {
        count: usize,
        max: usize,
    },
    EntityTooLarge {
        id: EntityId,
        len: usize,
        max: usize,
    },
    SnapshotTooLarge {
        bytes: usize,
        max: usize,
    },
    /// An index reappeared with a generation at or below a retired one.
    GenerationRegression(EntityId),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIndex(index) => write!(f, "duplicate entity index {index}"),
            Self::IndexOutOfRange(index) => write!(f, "entity index {index} out of range"),
            Self::TooManyEntities { count, max } => {
                write!(f, "{count} entities exceed limit {max}")
            }
            Self::EntityTooLarge { id, len, max } => {
                write!(f, "{id} state of {len} bytes exceeds limit {max}")
            }
            Self::SnapshotTooLarge { bytes, max } => {
                write!(f, "snapshot of {bytes} bytes exceeds limit {max}")
            }
            Self::GenerationRegression(id) => write!(f, "{id} regresses a retired generation"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// Size limits applied to snapshots and deltas.
#[derive(Clone, Copy, Debug)]
pub struct ReplicationLimits {
    pub max_entities: usize,
    pub max_entity_bytes: usize,
    pub max_snapshot_bytes: usize,
}

/// A complete authoritative state: entities in strictly ascending index order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    records: Vec<EntityRecord>,
}

impl Snapshot {
    pub const fn empty() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Build a snapshot from records in any order. Indices must be unique.
    pub fn from_records(mut records: Vec<EntityRecord>) -> Result<Self, SnapshotError> {
        records.sort_by_key(|record| record.id.index);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].id.index == pair[1].id.index)
        {
            return Err(SnapshotError::DuplicateIndex(pair[0].id.index));
        }
        Ok(Self { records })
    }

    /// Records are already strictly ascending by index.
    pub(crate) fn from_sorted(records: Vec<EntityRecord>) -> Self {
        Self { records }
    }

    pub fn records(&self) -> &[EntityRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The live occupant of `index`, whatever its generation.
    pub fn at_index(&self, index: u16) -> Option<&EntityRecord> {
        self.records
            .binary_search_by_key(&index, |record| record.id.index)
            .ok()
            .map(|position| &self.records[position])
    }

    /// The state of exactly this entity generation.
    pub fn get(&self, id: EntityId) -> Option<&[u8]> {
        self.at_index(id.index)
            .filter(|record| record.id == id)
            .map(|record| record.state.as_slice())
    }

    /// Bytes of this snapshot encoded in full.
    pub fn encoded_len(&self) -> usize {
        delta::FULL_OVERHEAD
            + self
                .records
                .iter()
                .map(|record| delta::UPSERT_OVERHEAD + record.state.len())
                .sum::<usize>()
    }

    /// Check every size limit; returns the full encoded length.
    ///
    /// The record count and each state length must also fit their 16-bit
    /// wire fields, whatever larger limits a caller passes.
    pub fn check(&self, limits: &ReplicationLimits) -> Result<usize, SnapshotError> {
        let max_entities = limits.max_entities.min(usize::from(u16::MAX));
        let max_entity_bytes = limits.max_entity_bytes.min(usize::from(u16::MAX));
        if self.records.len() > max_entities {
            return Err(SnapshotError::TooManyEntities {
                count: self.records.len(),
                max: max_entities,
            });
        }
        for record in &self.records {
            if usize::from(record.id.index) >= limits.max_entities {
                return Err(SnapshotError::IndexOutOfRange(record.id.index));
            }
            if record.state.len() > max_entity_bytes {
                return Err(SnapshotError::EntityTooLarge {
                    id: record.id,
                    len: record.state.len(),
                    max: max_entity_bytes,
                });
            }
        }
        let bytes = self.encoded_len();
        if bytes > limits.max_snapshot_bytes {
            return Err(SnapshotError::SnapshotTooLarge {
                bytes,
                max: limits.max_snapshot_bytes,
            });
        }
        Ok(bytes)
    }
}

/// A snapshot with its identity, shared between history and peers.
#[derive(Clone, Debug)]
pub(crate) struct StoredSnapshot {
    pub id: SnapshotId,
    pub tick: Tick,
    pub state: Arc<Snapshot>,
}

/// A bounded ring of recent snapshots, newest last.
#[derive(Debug)]
pub(crate) struct SnapshotHistory {
    ring: VecDeque<StoredSnapshot>,
    capacity: usize,
}

impl SnapshotHistory {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            ring: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(crate) fn push(&mut self, snapshot: StoredSnapshot) {
        if self.ring.len() == self.capacity {
            self.ring.pop_front();
        }
        self.ring.push_back(snapshot);
    }

    pub(crate) fn get(&self, id: SnapshotId) -> Option<&StoredSnapshot> {
        self.ring.iter().rev().find(|stored| stored.id == id)
    }

    pub(crate) fn latest(&self) -> Option<&StoredSnapshot> {
        self.ring.back()
    }
}

/// The lowest generation each entity index may still carry.
///
/// A live entity pins its index at its generation. When it is destroyed or
/// replaced the floor moves past it, so a stale message cannot resurrect it.
/// Storage is bounded by the entity limit.
#[derive(Debug, Default)]
pub(crate) struct GenerationFloors {
    floors: Vec<u64>,
}

impl GenerationFloors {
    /// Validate `next` against the floors, then advance them. `previous` is
    /// the snapshot that `next` supersedes.
    pub(crate) fn advance(
        &mut self,
        previous: Option<&Snapshot>,
        next: &Snapshot,
    ) -> Result<(), SnapshotError> {
        let previous = previous.map_or(&[][..], Snapshot::records);
        let highest = [previous.last(), next.records().last()]
            .into_iter()
            .flatten()
            .map(|record| usize::from(record.id.index) + 1)
            .max()
            .unwrap_or(0);
        if self.floors.len() < highest {
            self.floors.resize(highest, 0);
        }
        for record in next.records() {
            if u64::from(record.id.generation) < self.floors[usize::from(record.id.index)] {
                return Err(SnapshotError::GenerationRegression(record.id));
            }
        }
        // Validation passed; retire everything `next` no longer carries.
        for record in previous {
            let survives = next
                .at_index(record.id.index)
                .is_some_and(|current| current.id == record.id);
            if !survives {
                let floor = &mut self.floors[usize::from(record.id.index)];
                *floor = (*floor).max(u64::from(record.id.generation) + 1);
            }
        }
        for record in next.records() {
            let floor = &mut self.floors[usize::from(record.id.index)];
            *floor = (*floor).max(u64::from(record.id.generation));
        }
        Ok(())
    }
}
