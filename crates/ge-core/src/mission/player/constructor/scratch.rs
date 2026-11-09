//! Persistent scratch header constructed by 8013d250. The later reset's
//! deficit comparison is intentionally different from ensure-capacity(32).
//! Record storage remains uninitialized; preparation does not create records.
use anyhow::{Context, Result, ensure};
use std::{fmt, mem::MaybeUninit};

const RECORD_BYTES: i32 = 0x94;

#[derive(Debug, Clone, Copy)]
pub struct ScratchBudget {
    /// Maximum requested native backing bytes for this reallocation.
    pub native_bytes: usize,
    /// Checked additions, including the initial capacity + growth operation.
    pub growth_steps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScratchReceipt {
    pub capacity_before: u32,
    pub capacity_after: u32,
    pub count: u32,
    /// Zero when source capacity >= 32 skips the allocator entirely.
    pub requested_bytes: usize,
    pub reallocated: bool,
    pub growth_steps: usize,
}

pub struct Scratch {
    capacity: u32,
    growth: u32,
    count: u32,
    backing: Option<Vec<MaybeUninit<u8>>>,
    realloc_calls: u64,
}

impl fmt::Debug for Scratch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlayerConstructorScratch")
            .field("capacity", &self.capacity)
            .field("growth", &self.growth)
            .field("count", &self.count)
            .field("backing_bytes", &self.backing_len())
            .field("realloc_calls", &self.realloc_calls)
            .finish()
    }
}

impl Scratch {
    /// Actual startup constructor state, not a claim of original later history.
    pub fn from_constructor() -> Self {
        Self {
            capacity: 0,
            growth: 16,
            count: 0,
            backing: None,
            realloc_calls: 0,
        }
    }

    /// 8012f0e0..12c. Host checks reject arithmetic overflow, negative capacity
    /// and nonterminating growth before mutating the owner. Allocation failure
    /// likewise retains the prior header/backing; no fake empty records result.
    pub fn prepare_reset(&mut self, budget: ScratchBudget) -> Result<ScratchReceipt> {
        let capacity = i32::try_from(self.capacity)
            .context("scratch capacity outside supported nonnegative signed range")?;
        let mut receipt = ScratchReceipt {
            capacity_before: self.capacity,
            capacity_after: self.capacity,
            count: self.count,
            requested_bytes: 0,
            reallocated: false,
            growth_steps: 0,
        };
        if capacity >= 32 {
            return Ok(receipt);
        }
        let deficit = 32_i32
            .checked_sub(capacity)
            .context("scratch deficit overflow")?;
        let growth = self.growth as i32;
        ensure!(
            budget.growth_steps >= 1,
            "scratch growth work budget exhausted"
        );
        let mut steps = 1_usize;
        let mut candidate = capacity
            .checked_add(growth)
            .context("scratch signed growth overflow")?;
        if deficit > 0 && candidate < deficit {
            ensure!(growth > 0, "scratch growth cannot reach source deficit");
        }
        while deficit > 0 && candidate < deficit {
            ensure!(
                steps < budget.growth_steps,
                "scratch growth work budget exhausted"
            );
            candidate = candidate
                .checked_add(growth)
                .context("scratch signed growth overflow")?;
            steps = steps
                .checked_add(1)
                .context("scratch growth step count overflow")?;
        }
        ensure!(candidate >= 0, "scratch candidate capacity is negative");
        let next_capacity = u32::try_from(candidate)?;
        ensure!(
            self.count <= next_capacity,
            "scratch resizing would truncate live records"
        );
        let bytes = usize::try_from(
            candidate
                .checked_mul(RECORD_BYTES)
                .context("scratch signed allocation size overflow")?,
        )?;
        ensure!(
            bytes <= budget.native_bytes,
            "scratch native byte budget exhausted"
        );
        let calls = self
            .realloc_calls
            .checked_add(1)
            .context("scratch reallocation count overflow")?;

        if let Some(backing) = &mut self.backing {
            if bytes > backing.len() {
                backing
                    .try_reserve_exact(bytes - backing.len())
                    .context("scratch backing reallocation failed")?;
            }
            // Moving MaybeUninit preserves existing initialization state and
            // values without reading record bytes. Added bytes stay unknown.
            backing.resize_with(bytes, MaybeUninit::uninit);
        } else {
            let mut backing = Vec::new();
            backing
                .try_reserve_exact(bytes)
                .context("scratch backing allocation failed")?;
            backing.resize_with(bytes, MaybeUninit::uninit);
            self.backing = Some(backing);
        }
        // Source publishes returned backing at 8012f128 before capacity at12c.
        self.capacity = next_capacity;
        self.realloc_calls = calls;
        receipt.capacity_after = next_capacity;
        receipt.requested_bytes = bytes;
        receipt.reallocated = true;
        receipt.growth_steps = steps;
        Ok(receipt)
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }
    pub const fn growth(&self) -> u32 {
        self.growth
    }
    pub const fn count(&self) -> u32 {
        self.count
    }
    pub const fn has_backing(&self) -> bool {
        self.backing.is_some()
    }
    pub fn backing_len(&self) -> usize {
        self.backing.as_ref().map_or(0, Vec::len)
    }
    pub fn host_capacity_bytes(&self) -> usize {
        self.backing.as_ref().map_or(0, Vec::capacity)
    }
    /// Completed source reallocation operations backed by host reserve/resize;
    /// not a count of physical host allocator invocations or failed attempts.
    pub const fn realloc_calls(&self) -> u64 {
        self.realloc_calls
    }

    /// Versioned known metadata only. No pointer, host over-allocation, record
    /// payload or invented initialization bitmap contributes to this stream.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.capacity);
        emit(self.growth);
        emit(self.count);
        emit(u32::from(self.has_backing()));
        let bytes = self.backing_len() as u64;
        emit((bytes >> 32) as u32);
        emit(bytes as u32);
        emit((self.realloc_calls >> 32) as u32);
        emit(self.realloc_calls as u32);
    }
}

#[cfg(test)]
#[path = "tests/scratch.rs"]
mod tests;
