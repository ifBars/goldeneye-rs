//! Owned bounded Tutorial loader phases, separate scalar owner and phase8 byte.
//! Phase changes do not emit event26 or construct a world/player/camera.
use std::num::NonZeroU64;

use anyhow::{Context, Result, ensure};

use super::work::{WorkOwner, WorkPoll};

pub const WAIT_PHASE: u8 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttemptId(NonZeroU64);
impl AttemptId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(
            NonZeroU64::new(value).context("loader attempt identity must be nonzero")?,
        ))
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePoll {
    Busy,
    Complete,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitPoll {
    WorkBusy,
    ResourceBusy,
    Promoted { phase: u8 },
}

fn supported_phase(phase: u8) -> bool {
    matches!(phase, 6..=8 | 10..=19 | 21)
}
fn transition(current: u8, next: u8) -> bool {
    matches!((current, next), (6, 6 | 7) | (7, 8) | (8, 10) | (19, 21))
        || ((10..=18).contains(&current) && next == current + 1)
}

#[derive(Debug)]
pub struct PhaseState {
    attempt: AttemptId,
    current: u8,
    pending: Option<u8>,
    cancelled: bool,
}
impl PhaseState {
    /// An explicit supported phase snapshot. Actual fresh host orchestration
    /// enters at its genuinely reached phase; this constructor is not a proof
    /// that earlier work or native loader history completed.
    pub fn new(attempt: AttemptId, initial: u8) -> Result<Self> {
        ensure!(supported_phase(initial), "unsupported initial loader phase");
        Ok(Self {
            attempt,
            current: initial,
            pending: None,
            cancelled: false,
        })
    }
    fn ensure_current(&self, attempt: AttemptId) -> Result<()> {
        ensure!(self.attempt == attempt, "loader attempt identity differs");
        ensure!(!self.cancelled, "loader attempt was cancelled");
        Ok(())
    }
    fn ensure_queue(&self, attempt: AttemptId, next: u8) -> Result<()> {
        self.ensure_current(attempt)?;
        ensure!(
            self.pending.is_none() && self.current != WAIT_PHASE,
            "loader is already waiting"
        );
        ensure!(
            supported_phase(next) && transition(self.current, next),
            "unsupported loader phase transition"
        );
        Ok(())
    }
    pub fn queue(&mut self, attempt: AttemptId, next: u8) -> Result<()> {
        self.ensure_queue(attempt, next)?;
        self.current = WAIT_PHASE;
        self.pending = Some(next);
        Ok(())
    }
    /// The resource closure must poll root's actual owned resource/provider
    /// adapter. It is never a UI/evidence readiness boolean. Work failures and
    /// either busy result preserve pending; promotion returns without dispatch.
    pub fn poll(
        &mut self,
        attempt: AttemptId,
        work: &WorkOwner,
        resource: impl FnOnce() -> Result<ResourcePoll>,
    ) -> Result<WaitPoll> {
        self.ensure_current(attempt)?;
        ensure!(self.current == WAIT_PHASE, "loader is not waiting");
        let pending = self.pending.context("loader wait has no pending phase")?;
        if work.poll(attempt)? == WorkPoll::Busy {
            return Ok(WaitPoll::WorkBusy);
        }
        if resource()? == ResourcePoll::Busy {
            return Ok(WaitPoll::ResourceBusy);
        }
        self.current = pending;
        self.pending = None;
        Ok(WaitPoll::Promoted { phase: pending })
    }
    pub fn cancel(&mut self, attempt: AttemptId) -> Result<()> {
        self.ensure_current(attempt)?;
        self.cancelled = true;
        Ok(())
    }
    pub const fn attempt(&self) -> AttemptId {
        self.attempt
    }
    pub const fn current(&self) -> u8 {
        self.current
    }
    pub const fn pending(&self) -> Option<u8> {
        self.pending
    }
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn apply_case8(&mut self, attempt: AttemptId, byte: &mut Phase8Byte) -> Result<()> {
        self.ensure_queue(attempt, 10)?;
        ensure!(self.current == 8, "phase8 byte reset is out of sequence");
        let resets = byte
            .resets
            .checked_add(1)
            .context("phase8 reset count overflow")?;
        byte.raw = Some(10);
        byte.resets = resets;
        self.queue(attempt, 10)
    }
    pub fn apply_case16(
        &mut self,
        attempt: AttemptId,
        scalars: &mut LoaderScalars,
    ) -> Result<ScalarWrites> {
        self.ensure_queue(attempt, 17)?;
        ensure!(
            self.current == 16,
            "phase16 scalar reset is out of sequence"
        );
        let writes = ScalarWrites {
            previous_c4: scalars.c4,
            previous_c0: scalars.c0,
        };
        // Separate source owner805c94a8: ordered c4 then c0, without arithmetic.
        scalars.c4 = Some(0x44bb_8000);
        scalars.c0 = Some(0x3a83_126f);
        self.queue(attempt, 17)?;
        Ok(writes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoaderScalars {
    c0: Option<u32>,
    c4: Option<u32>,
}
impl LoaderScalars {
    pub const fn unbound() -> Self {
        Self { c0: None, c4: None }
    }
    pub const fn from_snapshot(c0: Option<u32>, c4: Option<u32>) -> Self {
        Self { c0, c4 }
    }
    pub const fn c0_bits(&self) -> Option<u32> {
        self.c0
    }
    pub const fn c4_bits(&self) -> Option<u32> {
        self.c4
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarWrites {
    pub previous_c4: Option<u32>,
    pub previous_c0: Option<u32>,
}
impl ScalarWrites {
    pub const fn ordered(&self) -> [(u16, u32); 2] {
        [(0xc4, 0x44bb_8000), (0xc0, 0x3a83_126f)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase8Byte {
    raw: Option<u8>,
    resets: usize,
}
impl Phase8Byte {
    pub const fn from_constructor() -> Self {
        Self {
            raw: Some(10),
            resets: 0,
        }
    }
    pub const fn from_snapshot(raw: Option<u8>) -> Self {
        Self { raw, resets: 0 }
    }
    pub const fn value(&self) -> Option<u8> {
        self.raw
    }
    pub const fn resets(&self) -> usize {
        self.resets
    }
    /// The same source helper is also reached by the earlier New Game handoff.
    /// Equal current value does not elide the actual byte store.
    pub fn reset_to_ten(&mut self) -> Result<()> {
        let resets = self
            .resets
            .checked_add(1)
            .context("phase8 byte reset count overflow")?;
        self.raw = Some(10);
        self.resets = resets;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/phase.rs"]
mod tests;
