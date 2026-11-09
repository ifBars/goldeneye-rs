use anyhow::{Context, Result};

use super::super::buffer::Buffer;
use super::BufferClear;

#[derive(Debug)]
pub struct TwoBuffers {
    buffers: [Buffer; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwoBuffersReset {
    pub cleared: [BufferClear; 2],
}

impl TwoBuffers {
    pub fn fresh() -> Self {
        Self {
            buffers: std::array::from_fn(|_| Buffer::empty(4)),
        }
    }
    pub fn buffers(&self) -> &[Buffer; 2] {
        &self.buffers
    }
    pub fn buffer_mut(&mut self, index: usize) -> Result<&mut Buffer> {
        self.buffers
            .get_mut(index)
            .context("two-buffer index out of bounds")
    }
    pub fn reset(&mut self) -> TwoBuffersReset {
        TwoBuffersReset {
            cleared: [
                BufferClear {
                    source_offset: 0,
                    stats: self.buffers[0].clear_backing(),
                },
                BufferClear {
                    source_offset: 0x14,
                    stats: self.buffers[1].clear_backing(),
                },
            ],
        }
    }
}
