use anyhow::{Context, Result};
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BindingId(NonZeroU64);
impl BindingId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(
            NonZeroU64::new(value).context("binding identity must be nonzero")?,
        ))
    }
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// A real independently owned item. Unrelated original item fields are omitted.
#[derive(Debug)]
pub struct Binding {
    id: BindingId,
    byte_40: u8,
    byte_41: u8,
}
impl Binding {
    pub fn from_owned(id: BindingId, byte_40: u8, byte_41: u8) -> Self {
        Self {
            id,
            byte_40,
            byte_41,
        }
    }
    pub fn id(&self) -> BindingId {
        self.id
    }
    pub fn byte_40(&self) -> u8 {
        self.byte_40
    }
    pub fn byte_41(&self) -> u8 {
        self.byte_41
    }

    pub(super) fn release_flags(&mut self) -> BindingRelease {
        let first = BindingWrite {
            offset: 0x41,
            before: self.byte_41,
            after: 0,
        };
        self.byte_41 = 0;
        let second = BindingWrite {
            offset: 0x40,
            before: self.byte_40,
            after: 1,
        };
        self.byte_40 = 1;
        BindingRelease {
            id: self.id,
            writes: [first, second],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingSlot {
    Unknown,
    Absent,
    Bound(BindingId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingWrite {
    pub offset: u8,
    pub before: u8,
    pub after: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingRelease {
    pub id: BindingId,
    pub writes: [BindingWrite; 2],
}
