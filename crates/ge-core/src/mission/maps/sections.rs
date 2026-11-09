//! Logical source publication is separate from cached full-file availability.
use crate::mission::world::WrapperId;
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    BasePublished,
    Unrequested,
}

impl Status {
    pub fn source_status(self) -> u8 {
        match self {
            Self::BasePublished => 3,
            Self::Unrequested => 0,
        }
    }
}

/// An explicit constructor-compatible base-completion snapshot. The real base
/// payload must be retained by the adapter; eager cached tail bytes do not
/// publish nonbase sections. Future requests need their own owned provider.
#[derive(Debug)]
pub struct Owner {
    wrapper: WrapperId,
    section_count: usize,
    loaded_mask: [u32; 4],
}

impl Owner {
    pub fn from_fresh_base(wrapper: WrapperId, section_count: usize, limit: usize) -> Result<Self> {
        ensure!(
            section_count > 0 && section_count <= limit && section_count <= i16::MAX as usize,
            "fresh base section count outside host budget"
        );
        Ok(Self {
            wrapper,
            section_count,
            loaded_mask: [0; 4],
        })
    }
    pub fn wrapper(&self) -> WrapperId {
        self.wrapper
    }
    pub fn section_count(&self) -> usize {
        self.section_count
    }
    pub fn loaded_mask(&self) -> &[u32; 4] {
        &self.loaded_mask
    }
    pub fn status(&self, index: usize) -> Result<Status> {
        ensure!(
            index < self.section_count,
            "logical section index outside owner"
        );
        Ok(if index == 0 {
            Status::BasePublished
        } else {
            Status::Unrequested
        })
    }
}

#[cfg(test)]
#[path = "publication/tests/sections.rs"]
mod tests;
