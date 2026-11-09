//! Persistent sparse owner for the source sixteen-read engine-clock reset.
//!
//! The clock is supplied by the host, never a wall clock or invented zero here.
//! Constructor-compatible headers are initialized once; each reset clears only
//! four of six actual owned buffers. Failed providers leave reached stores and
//! buffer cleanup intact. Host f32 arithmetic is not native FPSCR/timing parity.
use anyhow::{Context, Result};

use super::buffer::{Buffer, ClearStats};

pub trait Clock {
    fn read(&mut self) -> Result<u32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    HeadersInitialized,
    Reading { ordinal: u8 },
    Failed { ordinal: u8 },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearReceipt {
    pub buffer: usize,
    pub stats: ClearStats,
}

/// Scalar words written by the constructor/reset, excluding nested headers.
/// Unknown/untouched owner fields are absent rather than implicitly zero.
pub const WORD_OFFSETS: [usize; 27] = [
    0x00, 0x04, 0x08, 0x0c, 0x10, 0x28, 0x2c, 0x30, 0x48, 0x4c, 0x50, 0x68, 0x6c, 0xac, 0xb0, 0xb4,
    0xb8, 0xbc, 0xc0, 0xc4, 0xc8, 0xd8, 0xdc, 0xe0, 0xe4, 0xe8, 0xec,
];

pub struct Owner {
    words: [Option<u32>; 27],
    // Source offsets14,34,54,70,84,98; all constructor growth words are0.
    buffers: [Buffer; 6],
    stage: Stage,
    reads_in_attempt: usize,
    total_reads: usize,
    completed_resets: usize,
    clears: [Option<ClearReceipt>; 4],
}

impl Owner {
    /// Actual source constructor initializes headers/timers, then invokes reset.
    /// A construction failure returns no owner. Later reset errors retain the
    /// existing owner and its partial mutation instead of replacing it.
    pub fn from_constructor(clock: &mut impl Clock) -> Result<Self> {
        let mut owner = Self {
            words: [None; 27],
            buffers: std::array::from_fn(|_| Buffer::empty(0)),
            stage: Stage::HeadersInitialized,
            reads_in_attempt: 0,
            total_reads: 0,
            completed_resets: 0,
            clears: [None; 4],
        };
        for offset in [
            0x08, 0x0c, 0x28, 0x2c, 0x48, 0x4c, 0x68, 0x6c, 0xac, 0xb0, 0xd8, 0xdc, 0xe0, 0xe4,
            0xe8, 0xec,
        ] {
            owner.store(offset, 0);
        }
        owner.reset(clock)?;
        Ok(owner)
    }

    pub fn reset(&mut self, clock: &mut impl Clock) -> Result<()> {
        // Host accounting must not overflow after mutating actual retained data.
        self.total_reads
            .checked_add(16)
            .context("clock reset read accounting overflow")?;
        let completed = self
            .completed_resets
            .checked_add(1)
            .context("clock reset completion accounting overflow")?;
        self.reads_in_attempt = 0;
        self.clears = [None; 4];
        self.stage = Stage::Reading { ordinal: 1 };

        self.store(0x00, 0);
        self.store(0x04, 0);
        self.pair(clock, 1, 0x08, 0x0c)?;
        self.store(0x10, 0);
        self.clear(0, 0);

        self.pair(clock, 3, 0x28, 0x2c)?;
        self.store(0x30, 0);
        self.clear(1, 1);

        self.pair(clock, 5, 0x48, 0x4c)?;
        self.store(0x50, 0);
        self.clear(2, 2);

        self.pair(clock, 7, 0x68, 0x6c)?;
        self.store(0xc4, 0);
        self.store(0xb4, 0);
        self.store(0xb8, 0);
        self.store(0xbc, 0);
        self.store(0xc0, 0);
        self.store(0xc8, 0);

        self.pair(clock, 9, 0xe0, 0xe4)?;
        self.pair(clock, 11, 0xe8, 0xec)?;
        self.pair(clock, 13, 0xd8, 0xdc)?;
        self.clear(3, 5);
        self.pair(clock, 15, 0xac, 0xb0)?;

        self.completed_resets = completed;
        self.stage = Stage::Complete;
        Ok(())
    }

    fn pair(
        &mut self,
        clock: &mut impl Clock,
        ordinal: u8,
        first_offset: usize,
        second_offset: usize,
    ) -> Result<()> {
        let first = self.read(clock, ordinal)?;
        // Keep an actual addition: +0 + -0 becomes +0 under host RNE. The
        // second sample is a direct bit copy, with no clamp/finite filtering.
        self.store(first_offset, (0.0f32 + f32::from_bits(first)).to_bits());
        let second = self.read(clock, ordinal + 1)?;
        self.store(second_offset, second);
        Ok(())
    }

    fn read(&mut self, clock: &mut impl Clock, ordinal: u8) -> Result<u32> {
        self.stage = Stage::Reading { ordinal };
        self.reads_in_attempt += 1;
        self.total_reads += 1;
        match clock.read() {
            Ok(bits) => Ok(bits),
            Err(error) => {
                self.stage = Stage::Failed { ordinal };
                Err(error).with_context(|| format!("clock reset provider read {ordinal} failed"))
            }
        }
    }

    fn clear(&mut self, receipt: usize, buffer: usize) {
        self.clears[receipt] = Some(ClearReceipt {
            buffer,
            stats: self.buffers[buffer].clear_backing(),
        });
    }

    fn store(&mut self, offset: usize, bits: u32) {
        let index = WORD_OFFSETS
            .binary_search(&offset)
            .expect("internal clock reset word offset");
        self.words[index] = Some(bits);
    }

    pub fn word(&self, offset: usize) -> Option<u32> {
        WORD_OFFSETS
            .binary_search(&offset)
            .ok()
            .and_then(|index| self.words[index])
    }

    pub fn buffers(&self) -> &[Buffer; 6] {
        &self.buffers
    }

    pub fn buffer_mut(&mut self, index: usize) -> Option<&mut Buffer> {
        self.buffers.get_mut(index)
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Provider calls attempted, including a call that returned an error.
    pub fn reads_in_attempt(&self) -> usize {
        self.reads_in_attempt
    }

    pub fn total_reads(&self) -> usize {
        self.total_reads
    }

    /// Includes the reset invoked by a successfully completed constructor.
    pub fn completed_resets(&self) -> usize {
        self.completed_resets
    }

    pub fn clears_in_attempt(&self) -> &[Option<ClearReceipt>; 4] {
        &self.clears
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
