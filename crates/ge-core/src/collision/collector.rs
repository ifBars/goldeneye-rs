//! Pure geometry-filter selection and fixed-capacity collected-record preparation.
//! Live candidate membership, recursive geometry dispatch and contact solving are separate.
use super::frame::FrameKey;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct CollectionContext {
    pub matrix: [f32; 16],
    pub owner_key: u32,
    /// Full context word before the append helper truncates it to a signed short.
    pub slot_raw: u32,
}
impl CollectionContext {
    pub fn map_zone(matrix: [f32; 16]) -> Self {
        Self {
            matrix,
            owner_key: 0,
            slot_raw: u32::MAX,
        }
    }
    /// Candidate ordinal is local to the current zone's returned list. It is
    /// not the addressed global placement index, hash, or a unique frame ID.
    pub fn map_placement(matrix: [f32; 16], candidate_ordinal: u32) -> Self {
        Self {
            matrix,
            owner_key: 0,
            slot_raw: candidate_ordinal,
        }
    }
}

pub fn map_placement_flags(inherited: u32, engine_flags: u16) -> u32 {
    if engine_flags & 4 != 0 {
        inherited | 2
    } else {
        inherited
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollectedRecord {
    pub matrix: [f32; 16],
    /// Opaque native identity; this pure component never dereferences it.
    pub geometry_key: u32,
    pub optional_context_key: u32,
    pub owner_key: u32,
    pub slot: i16,
    pub flags_raw: u16,
}
impl CollectedRecord {
    /// Append copies all sixteen matrix lanes without evaluating them. Validation
    /// belongs to whichever downstream operation actually uses those lanes.
    pub fn from_dispatch(
        context: &CollectionContext,
        geometry_key: u32,
        optional_context_key: u32,
        flags: u32,
    ) -> Self {
        Self {
            matrix: context.matrix,
            geometry_key,
            optional_context_key,
            owner_key: context.owner_key,
            slot: context.slot_raw as u16 as i16,
            flags_raw: flags as u16,
        }
    }
    pub fn frame_key(&self) -> FrameKey {
        FrameKey {
            owner_key: self.owner_key,
            slot: self.slot,
        }
    }
}

pub struct RecordBuffer {
    capacity: usize,
    records: Vec<CollectedRecord>,
}
impl RecordBuffer {
    pub fn new(capacity: usize) -> Result<Self> {
        ensure!(
            capacity <= 1_000_000,
            "collected-record host capacity budget exceeded"
        );
        Ok(Self {
            capacity,
            records: Vec::new(),
        })
    }
    pub fn records(&self) -> &[CollectedRecord] {
        &self.records
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Native full-buffer behavior is a failed append with count unchanged;
    /// matrix, identity and flag sources stay unread, and capacity never grows.
    /// A failed host adapter preparation also retains the previous valid list.
    /// This host error guard is not a native exception-recovery claim.
    pub fn append(&mut self, prepare: impl FnOnce() -> Result<CollectedRecord>) -> Result<bool> {
        if self.records.len() >= self.capacity {
            return Ok(false);
        }
        let record = prepare()?;
        self.records.push(record);
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GeometryFilter {
    pub current_slot: u32,
    pub flags_raw: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum GeometrySelection<T> {
    Keep,
    Replace(T),
    Reject,
}
// PowerPC slw uses the low six shift bits; shifts 32..63 produce zero.
fn native_shift(value: u32, amount: u32) -> u32 {
    let shift = amount & 63;
    if shift < 32 { value << shift } else { 0 }
}
impl GeometryFilter {
    /// Source filter 802cd850 can replace the addressed entity. The caller's
    /// separate entity-flag0x100 gate occurs before this helper. No component
    /// header is addressed on bypass0x400, or after a selected component.
    pub fn select<T>(
        self,
        component_mask: impl FnOnce() -> Result<u8>,
        mut component: impl FnMut(u8) -> Result<T>,
    ) -> Result<GeometrySelection<T>> {
        if self.flags_raw & 0x400 != 0 {
            return Ok(GeometrySelection::Keep);
        }
        let mask = u32::from(component_mask()?);
        let direct_slot = (self.current_slot & 0xff) as u8;
        if mask & native_shift(1, u32::from(direct_slot)) != 0 {
            return Ok(GeometrySelection::Replace(component(direct_slot)?));
        }
        ensure!(
            self.current_slot <= 1_000_000,
            "geometry-filter host slot-test budget exceeded"
        );
        for lower in (0..self.current_slot).rev() {
            if self.flags_raw & native_shift(2, lower) == 0 {
                continue;
            }
            let slot = (lower & 0xff) as u8;
            if mask & native_shift(1, u32::from(slot)) != 0 {
                return Ok(GeometrySelection::Replace(component(slot)?));
            }
        }
        Ok(if self.current_slot == 0 || self.flags_raw & 1 != 0 {
            GeometrySelection::Keep
        } else {
            GeometrySelection::Reject
        })
    }
}

#[cfg(test)]
#[path = "../tests/collision_collector.rs"]
mod tests;
