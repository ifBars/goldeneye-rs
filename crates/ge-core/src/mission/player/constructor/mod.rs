//! Shared PlayerNew constructor allocations/count, with actual uninitialized
//! owned storage. Reset and final cache cleanup remain real caller operations.
mod allocation;
mod prelude;
pub use allocation::{Allocation, Kind};
pub use prelude::PendingPrelude;

use super::identity::PlayerId;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub native_bytes: usize,
    pub leases: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lease {
    player: PlayerId,
    sequence: u64,
}
impl Lease {
    pub fn player(self) -> PlayerId {
        self.player
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetStage {
    Counted,
    Complete,
    Failed,
    ReleaseFailed,
}
#[derive(Debug)]
struct Entry {
    lease: Lease,
    stage: ResetStage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseReceipt {
    pub count_before: u32,
    pub count_after: u32,
    pub last: bool,
    pub native_bytes_released: usize,
    pub host_bytes_released: usize,
}

#[derive(Debug)]
pub struct Shared {
    count: u32,
    cache_enabled: u8,
    allocations: [Option<Allocation>; 3],
    entries: Vec<Entry>,
    next_sequence: u64,
    allocation_failed: bool,
    cache_stores: u64,
    entry_noops: u64,
    reset_calls: u64,
    cleanup_calls: u64,
}
impl Shared {
    /// Source boot clears the count/pointers through its BSS table; the separate
    /// supplied DATA cache flag starts one. Host ownership persists thereafter.
    pub fn from_boot() -> Self {
        Self {
            count: 0,
            cache_enabled: 1,
            allocations: [None, None, None],
            entries: Vec::new(),
            next_sequence: 1,
            allocation_failed: false,
            cache_stores: 0,
            entry_noops: 0,
            reset_calls: 0,
            cleanup_calls: 0,
        }
    }
    pub fn count(&self) -> u32 {
        self.count
    }
    pub fn cache_enabled(&self) -> u8 {
        self.cache_enabled
    }
    pub fn allocation(&self, kind: Kind) -> Option<&Allocation> {
        self.allocations[kind.index()].as_ref()
    }
    pub fn live_leases(&self) -> usize {
        self.entries.len()
    }
    pub fn entry_noops(&self) -> u64 {
        self.entry_noops
    }
    pub fn cache_stores(&self) -> u64 {
        self.cache_stores
    }
    pub fn reset_calls(&self) -> u64 {
        self.reset_calls
    }
    pub fn cleanup_calls(&self) -> u64 {
        self.cleanup_calls
    }
    pub fn allocation_failed(&self) -> bool {
        self.allocation_failed
    }
    pub fn retained_native_bytes(&self) -> usize {
        self.allocations
            .iter()
            .flatten()
            .map(Allocation::native_bytes)
            .sum()
    }
    pub fn retained_host_capacity_bytes(&self) -> usize {
        self.allocations
            .iter()
            .flatten()
            .map(Allocation::host_capacity_bytes)
            .sum()
    }
    pub fn lease(&self, player: PlayerId) -> Option<Lease> {
        self.entries
            .iter()
            .find(|entry| entry.lease.player == player)
            .map(|entry| entry.lease)
    }
    pub fn reset_stage(&self, lease: Lease) -> Result<ResetStage> {
        Ok(self
            .entries
            .iter()
            .find(|entry| entry.lease == lease)
            .context("constructor lease is stale or foreign")?
            .stage)
    }

    /// The reset is called only after source allocations and count increment.
    /// Failure retains the lease, count and reached allocations. Recover the
    /// retained lease with lease(player); it does not represent reset success.
    pub fn acquire(
        &mut self,
        player: PlayerId,
        budget: Budget,
        reset: impl FnOnce() -> Result<()>,
    ) -> Result<Lease> {
        ensure!(
            !self.allocation_failed,
            "constructor retains a failed allocation prefix"
        );
        ensure!(
            !self
                .entries
                .iter()
                .any(|entry| entry.stage == ResetStage::ReleaseFailed),
            "constructor retains incomplete final cache cleanup"
        );
        ensure!(
            self.lease(player).is_none(),
            "player already holds a constructor lease"
        );
        ensure!(
            self.entries.len() < budget.leases,
            "constructor lease budget exhausted"
        );
        let next = self
            .count
            .checked_add(1)
            .context("constructor count overflow")?;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .context("constructor lease sequence overflow")?;
        self.entries.try_reserve(1)?;
        if self.count == 0 {
            ensure!(
                self.allocations.iter().all(Option::is_none),
                "zero-count constructor retains unexpected allocations"
            );
            for kind in [Kind::Raw, Kind::Array34, Kind::Array64] {
                let remaining = budget
                    .native_bytes
                    .saturating_sub(self.retained_native_bytes());
                match Allocation::allocate(kind, remaining) {
                    Ok(allocation) => {
                        self.entry_noops += u64::from(allocation.entry_noops());
                        self.allocations[kind.index()] = Some(allocation);
                    }
                    Err(error) => {
                        self.allocation_failed = true;
                        return Err(error);
                    }
                }
            }
            self.cache_enabled = 1;
            self.cache_stores += 1;
        } else {
            ensure!(
                self.allocations.iter().all(Option::is_some),
                "live constructor allocation owner is incomplete"
            );
            ensure!(
                self.retained_native_bytes() <= budget.native_bytes,
                "constructor retained storage exceeds budget"
            );
        }
        let lease = Lease {
            player,
            sequence: self.next_sequence,
        };
        self.next_sequence = next_sequence;
        self.count = next;
        self.entries.push(Entry {
            lease,
            stage: ResetStage::Counted,
        });
        self.reset_calls += 1;
        if let Err(error) = reset() {
            self.entries.last_mut().expect("just inserted lease").stage = ResetStage::Failed;
            return Err(error);
        }
        self.entries.last_mut().expect("just inserted lease").stage = ResetStage::Complete;
        Ok(lease)
    }

    /// Caller has performed required per-player cleanup. On the last lease the
    /// actual cache cleanup must return before array64/array34/raw are released.
    /// Failed cleanup retains its decremented count and allocation ownership;
    /// retry is unsupported rather than silently replaying native callbacks.
    pub fn release(
        &mut self,
        lease: Lease,
        cleanup: impl FnOnce() -> Result<()>,
    ) -> Result<ReleaseReceipt> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.lease == lease)
            .context("constructor lease is stale or foreign")?;
        ensure!(
            self.entries[index].stage != ResetStage::ReleaseFailed,
            "constructor cleanup already failed"
        );
        let before = self.count;
        let after = before
            .checked_sub(1)
            .context("constructor lease count underflow")?;
        self.count = after;
        let mut receipt = ReleaseReceipt {
            count_before: before,
            count_after: after,
            last: after == 0,
            native_bytes_released: 0,
            host_bytes_released: 0,
        };
        if after == 0 {
            self.cleanup_calls += 1;
            if let Err(error) = cleanup() {
                self.entries[index].stage = ResetStage::ReleaseFailed;
                return Err(error);
            }
            for kind in [Kind::Array64, Kind::Array34, Kind::Raw] {
                if let Some(allocation) = self.allocations[kind.index()].take() {
                    receipt.native_bytes_released += allocation.native_bytes();
                    receipt.host_bytes_released += allocation.host_capacity_bytes();
                    drop(allocation);
                }
            }
        }
        self.entries.remove(index);
        Ok(receipt)
    }
    /// Versioned owned-state words only. Uninitialized payload/header bytes are
    /// represented by metadata/unknown spans, never read or serialized.
    pub fn visit_state_words(&self, mut visit: impl FnMut(u32)) {
        visit(1);
        visit(self.count);
        visit(u32::from(self.cache_enabled));
        visit(u32::from(self.allocation_failed));
        for allocation in &self.allocations {
            visit(u32::from(allocation.is_some()));
            if let Some(allocation) = allocation {
                allocation.visit(&mut visit);
            }
        }
        visit(self.entries.len() as u32);
        for entry in &self.entries {
            for value in [entry.lease.player.get(), entry.lease.sequence] {
                visit((value >> 32) as u32);
                visit(value as u32);
            }
            visit(match entry.stage {
                ResetStage::Counted => 0,
                ResetStage::Complete => 1,
                ResetStage::Failed => 2,
                ResetStage::ReleaseFailed => 3,
            });
        }
        for value in [
            self.next_sequence,
            self.cache_stores,
            self.entry_noops,
            self.reset_calls,
            self.cleanup_calls,
        ] {
            visit((value >> 32) as u32);
            visit(value as u32);
        }
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
