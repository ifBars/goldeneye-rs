//! Delta encoding between two snapshots.
//!
//! ```text
//! u16 destroyed count
//!     u16 index, u32 generation               strictly ascending by index
//! u16 upsert count
//!     u8 operation (0 create, 1 update)
//!     u16 index, u32 generation
//!     u16 state length, state bytes           strictly ascending by index
//! ```
//!
//! A full snapshot is the delta against an empty baseline: no destroyed
//! entries and every upsert a create.

use std::fmt;

use super::{EntityRecord, ReplicationLimits, Snapshot, SnapshotError};
use crate::identity::EntityId;
use crate::protocol::DecodeError;
use crate::protocol::wire::{Reader, put_u8, put_u16, put_u32};

const OP_CREATE: u8 = 0;
const OP_UPDATE: u8 = 1;
/// The two counts.
pub(super) const FULL_OVERHEAD: usize = 4;
/// Operation, index, generation and state length.
pub(super) const UPSERT_OVERHEAD: usize = 9;
const DESTROY_BYTES: usize = 6;

/// A delta that cannot be applied to the baseline it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaError {
    Decode(DecodeError),
    /// Destroys an entity the baseline does not hold.
    DestroyMissing(EntityId),
    /// Creates an entity whose index is still occupied.
    CreateOccupied(EntityId),
    /// Replaces an entity with a generation that is not newer.
    CreateNotNewer(EntityId),
    /// Updates an entity the baseline does not hold.
    UpdateMissing(EntityId),
    /// The reconstructed snapshot breaks a size limit.
    Snapshot(SnapshotError),
}

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "malformed delta: {error}"),
            Self::DestroyMissing(id) => write!(f, "delta destroys absent {id}"),
            Self::CreateOccupied(id) => write!(f, "delta creates {id} over a live entity"),
            Self::CreateNotNewer(id) => write!(f, "delta replaces with non-newer {id}"),
            Self::UpdateMissing(id) => write!(f, "delta updates absent {id}"),
            Self::Snapshot(error) => write!(f, "delta result invalid: {error}"),
        }
    }
}

impl std::error::Error for DeltaError {}

impl From<DecodeError> for DeltaError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

/// Append the delta from `base` (empty if `None`) to `target`.
///
/// Both snapshots must have passed [`Snapshot::check`], so counts and state
/// lengths fit their `u16` fields.
pub fn encode_delta(base: Option<&Snapshot>, target: &Snapshot, out: &mut Vec<u8>) {
    let base = base.map_or(&[][..], Snapshot::records);
    let target = target.records();

    let mut destroyed: Vec<EntityId> = Vec::new();
    let mut upserts: Vec<(u8, &EntityRecord)> = Vec::new();
    let (mut b, mut t) = (0, 0);
    while b < base.len() || t < target.len() {
        let old = base.get(b);
        let new = target.get(t);
        match (old, new) {
            (Some(old), Some(new)) if old.id.index == new.id.index => {
                if old.id != new.id {
                    destroyed.push(old.id);
                    upserts.push((OP_CREATE, new));
                } else if old.state != new.state {
                    upserts.push((OP_UPDATE, new));
                }
                b += 1;
                t += 1;
            }
            (Some(old), Some(new)) if old.id.index < new.id.index => {
                destroyed.push(old.id);
                b += 1;
            }
            (Some(old), None) => {
                destroyed.push(old.id);
                b += 1;
            }
            (_, Some(new)) => {
                upserts.push((OP_CREATE, new));
                t += 1;
            }
            (None, None) => unreachable!("loop condition"),
        }
    }

    put_u16(out, destroyed.len() as u16);
    for id in destroyed {
        put_u16(out, id.index);
        put_u32(out, id.generation);
    }
    put_u16(out, upserts.len() as u16);
    for (operation, record) in upserts {
        put_u8(out, operation);
        put_u16(out, record.id.index);
        put_u32(out, record.id.generation);
        put_u16(out, record.state.len() as u16);
        out.extend_from_slice(&record.state);
    }
}

struct Upsert<'a> {
    operation: u8,
    id: EntityId,
    state: &'a [u8],
}

/// Reconstruct the snapshot that `delta` describes relative to `base`.
pub fn apply_delta(
    base: Option<&Snapshot>,
    delta: &[u8],
    limits: &ReplicationLimits,
) -> Result<Snapshot, DeltaError> {
    let mut reader = Reader::new(delta);

    let destroyed_count = usize::from(reader.u16()?);
    if destroyed_count > limits.max_entities {
        return Err(DecodeError::LimitExceeded("destroyed entities").into());
    }
    if destroyed_count * DESTROY_BYTES > reader.remaining() {
        return Err(DecodeError::Truncated.into());
    }
    let mut destroyed: Vec<EntityId> = Vec::with_capacity(destroyed_count);
    for _ in 0..destroyed_count {
        let id = EntityId::new(reader.u16()?, reader.u32()?);
        if destroyed.last().is_some_and(|last| last.index >= id.index) {
            return Err(DecodeError::InvalidField("destroyed order").into());
        }
        destroyed.push(id);
    }

    let upsert_count = usize::from(reader.u16()?);
    if upsert_count > limits.max_entities {
        return Err(DecodeError::LimitExceeded("upserted entities").into());
    }
    if upsert_count * UPSERT_OVERHEAD > reader.remaining() {
        return Err(DecodeError::Truncated.into());
    }
    let mut upserts: Vec<Upsert<'_>> = Vec::with_capacity(upsert_count);
    for _ in 0..upsert_count {
        let operation = reader.u8()?;
        if operation != OP_CREATE && operation != OP_UPDATE {
            return Err(DecodeError::InvalidField("upsert operation").into());
        }
        let id = EntityId::new(reader.u16()?, reader.u32()?);
        if upserts.last().is_some_and(|last| last.id.index >= id.index) {
            return Err(DecodeError::InvalidField("upsert order").into());
        }
        let len = usize::from(reader.u16()?);
        if len > limits.max_entity_bytes {
            return Err(DecodeError::LimitExceeded("entity bytes").into());
        }
        upserts.push(Upsert {
            operation,
            id,
            state: reader.bytes(len)?,
        });
    }
    reader.finish()?;

    let base = base.map_or(&[][..], Snapshot::records);
    let mut records: Vec<EntityRecord> =
        Vec::with_capacity((base.len() + upserts.len()).min(limits.max_entities));
    let mut destroyed = destroyed.into_iter().peekable();
    let mut upserts = upserts.into_iter().peekable();
    let create = |upsert: &Upsert<'_>| EntityRecord {
        id: upsert.id,
        state: upsert.state.to_vec(),
    };

    for old in base {
        // Upserts for indices the baseline does not occupy come first.
        while let Some(upsert) = upserts.next_if(|upsert| upsert.id.index < old.id.index) {
            if upsert.operation != OP_CREATE {
                return Err(DeltaError::UpdateMissing(upsert.id));
            }
            records.push(create(&upsert));
        }
        if let Some(stray) = destroyed.next_if(|id| id.index < old.id.index) {
            return Err(DeltaError::DestroyMissing(stray));
        }
        let removed = match destroyed.next_if(|id| id.index == old.id.index) {
            Some(id) if id != old.id => return Err(DeltaError::DestroyMissing(id)),
            Some(_) => true,
            None => false,
        };
        match upserts.next_if(|upsert| upsert.id.index == old.id.index) {
            Some(upsert) if upsert.operation == OP_CREATE => {
                if !removed {
                    return Err(DeltaError::CreateOccupied(upsert.id));
                }
                if upsert.id.generation <= old.id.generation {
                    return Err(DeltaError::CreateNotNewer(upsert.id));
                }
                records.push(create(&upsert));
            }
            Some(upsert) => {
                if removed || upsert.id != old.id {
                    return Err(DeltaError::UpdateMissing(upsert.id));
                }
                records.push(create(&upsert));
            }
            None if removed => {}
            None => records.push(old.clone()),
        }
    }
    if let Some(stray) = destroyed.next() {
        return Err(DeltaError::DestroyMissing(stray));
    }
    for upsert in upserts {
        if upsert.operation != OP_CREATE {
            return Err(DeltaError::UpdateMissing(upsert.id));
        }
        records.push(create(&upsert));
    }

    let snapshot = Snapshot::from_sorted(records);
    snapshot.check(limits).map_err(DeltaError::Snapshot)?;
    Ok(snapshot)
}
