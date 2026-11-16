//! Lazy access to the six supplied bone-tag words that the preload output
//! helper consumes. The words stay in the verified retained DATA owner; none is
//! embedded here and none is read until requested.
use super::{Data, SuppliedData};
use anyhow::{Result, ensure};
use std::{fmt, sync::Arc};

/// Words the source helper requests, strictly by index.
pub const BONE_TAG_COUNT: u8 = 6;

/// Source location of the table inside the supported image's DATA section.
const BONE_TAG_TABLE_ADDRESS: u32 = 0x8057_3f60;

/// An owned, cheap handle to the table; it does not borrow `SuppliedData`.
#[derive(Clone)]
pub struct BoneTagTable {
    data: Arc<Data>,
    address: u32,
}

impl fmt::Debug for BoneTagTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoneTagTable")
            .field("count", &BONE_TAG_COUNT)
            .finish_non_exhaustive()
    }
}

impl BoneTagTable {
    pub const fn count(&self) -> u8 {
        BONE_TAG_COUNT
    }

    /// One raw big-endian word, read when called. The word may be zero; the
    /// caller owns that source behavior (the helper does not advance its hash
    /// cursor past a zero word). An index outside the six, or a word outside
    /// the retained DATA window, is an error.
    pub fn hash(&self, index: u8) -> Result<u32> {
        ensure!(
            index < BONE_TAG_COUNT,
            "preload bone tag index is outside the six supplied words"
        );
        let address = self
            .address
            .checked_add(u32::from(index) * 4)
            .ok_or_else(|| anyhow::anyhow!("preload bone tag address overflow"))?;
        self.data.u32(address)
    }
}

impl SuppliedData {
    /// The six-word table at its actual location. Reads nothing yet.
    pub fn preload_bone_tags(&self) -> BoneTagTable {
        self.preload_bone_tags_at(BONE_TAG_TABLE_ADDRESS)
    }

    pub(crate) fn preload_bone_tags_at(&self, address: u32) -> BoneTagTable {
        BoneTagTable {
            data: Arc::clone(&self.data),
            address,
        }
    }
}

#[cfg(test)]
#[path = "tests/bone_tags.rs"]
mod tests;
