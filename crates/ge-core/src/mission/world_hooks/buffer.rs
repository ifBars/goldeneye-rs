//! Owned backing storage for the observed buffer headers. Logical header words
//! never authorize dereferencing a native pointer or deleting pointed objects.
use anyhow::{Result, ensure};
use std::fmt;

pub struct Buffer {
    capacity: u32,
    growth: u32,
    count: u32,
    backing: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearStats {
    pub had_backing: bool,
    /// Released host Vec allocation capacity, not an original heap-byte claim.
    pub bytes_released: usize,
    pub previous_capacity: u32,
    pub previous_count: u32,
}

impl Buffer {
    pub fn empty(growth: u32) -> Self {
        Self {
            capacity: 0,
            growth,
            count: 0,
            backing: None,
        }
    }

    /// Already-owned opaque bytes with an explicit logical header. No element
    /// stride is inferred from capacity/count and bytes are never reinterpreted.
    pub fn from_owned(
        capacity: u32,
        growth: u32,
        count: u32,
        backing: Option<Vec<u8>>,
    ) -> Result<Self> {
        ensure!(
            count <= capacity,
            "owned buffer count exceeds logical capacity"
        );
        Ok(Self {
            capacity,
            growth,
            count,
            backing,
        })
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }
    pub fn growth(&self) -> u32 {
        self.growth
    }

    /// An explicit caller write, separate from clear_backing's growth retention.
    pub fn set_growth(&mut self, growth: u32) {
        self.growth = growth;
    }
    pub fn count(&self) -> u32 {
        self.count
    }
    pub fn has_backing(&self) -> bool {
        self.backing.is_some()
    }
    pub fn backing_len(&self) -> usize {
        self.backing.as_ref().map_or(0, Vec::len)
    }
    /// Actual live host Vec allocation capacity, not the native logical header.
    pub fn backing_capacity(&self) -> usize {
        self.backing.as_ref().map_or(0, Vec::capacity)
    }
    pub fn backing(&self) -> Option<&[u8]> {
        self.backing.as_deref()
    }

    /// Release the actual owned allocation, then clear pointer/capacity/count.
    /// The source's growth word is retained even when no backing was present.
    pub fn clear_backing(&mut self) -> ClearStats {
        let old = self.backing.take();
        let stats = ClearStats {
            had_backing: old.is_some(),
            bytes_released: old.as_ref().map_or(0, Vec::capacity),
            previous_capacity: self.capacity,
            previous_count: self.count,
        };
        drop(old);
        self.capacity = 0;
        self.count = 0;
        stats
    }
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffer")
            .field("capacity", &self.capacity)
            .field("growth", &self.growth)
            .field("count", &self.count)
            .field("has_backing", &self.has_backing())
            .field("backing_len", &self.backing_len())
            .finish()
    }
}

#[cfg(test)]
#[path = "final_reset/tests/buffer.rs"]
mod tests;
