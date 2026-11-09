use super::types::*;
use anyhow::{Context, Result, ensure};

const CAPTURE_LIMIT: usize = 500;

#[derive(Debug)]
pub struct Owner {
    rows: Vec<Record>,
    // Semantic source capacity, distinct from the Rust allocator's capacity.
    capacity: u32,
    reset_limit: u32,
    origin: Origin,
}

struct Work {
    remaining: usize,
}
impl Work {
    fn take(&mut self, amount: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .context("resource state work budget exhausted")?;
        Ok(())
    }
}

impl Owner {
    /// The separate current global reset limit is an explicit snapshot input.
    pub fn from_constructor(reset_limit: u32) -> Self {
        Self {
            rows: Vec::new(),
            capacity: 0,
            reset_limit,
            origin: Origin::FreshConstructor,
        }
    }

    pub fn from_snapshot(
        rows: Vec<Record>,
        capacity: u32,
        reset_limit: u32,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            rows.len() as u64 <= u64::from(capacity),
            "resource snapshot exceeds supplied capacity"
        );
        ensure!(
            u64::from(capacity) <= budget.records as u64,
            "resource snapshot capacity budget exceeded"
        );
        ensure!(
            rows.len() <= budget.work,
            "resource snapshot work budget exceeded"
        );
        let mut owned = Vec::new();
        owned.try_reserve_exact(usize::try_from(capacity)?)?;
        owned.extend(rows);
        Ok(Self {
            rows: owned,
            capacity,
            reset_limit,
            origin: Origin::SuppliedSnapshot,
        })
    }

    pub fn rows(&self) -> &[Record] {
        &self.rows
    }
    pub fn origin(&self) -> Origin {
        self.origin
    }
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
    pub fn reset_limit(&self) -> u32 {
        self.reset_limit
    }

    pub fn lookup(&self, token: u32, fallback: u32, budget: Budget) -> Result<Lookup> {
        if token == 0 {
            return Ok(Lookup {
                value: fallback,
                matched: false,
                scanned: 0,
            });
        }
        let mut work = Work {
            remaining: budget.work,
        };
        for (index, row) in self.rows.iter().enumerate() {
            work.take(1)?;
            if row.key == token as u16 {
                return Ok(Lookup {
                    value: u32::from(row.value),
                    matched: true,
                    scanned: index + 1,
                });
            }
        }
        Ok(Lookup {
            value: fallback,
            matched: false,
            scanned: self.rows.len(),
        })
    }

    pub fn set(&mut self, token: u32, value: u32, budget: Budget) -> Result<SetOutcome> {
        if token >> 16 != 0x4200 || token as u16 == 0 {
            return Ok(SetOutcome::Invalid);
        }
        self.set_valid(
            token as u16,
            value as u16,
            budget.records,
            &mut Work {
                remaining: budget.work,
            },
        )
    }

    fn set_valid(
        &mut self,
        key: u16,
        value: u16,
        capacity_budget: usize,
        work: &mut Work,
    ) -> Result<SetOutcome> {
        for index in 0..self.rows.len() {
            work.take(1)?;
            if self.rows[index].key == key {
                work.take(1)?;
                self.rows[index].value = value;
                return Ok(SetOutcome::Updated { index });
            }
        }
        // Validate/allocate everything before the observable append or value write.
        work.take(1)?;
        let capacity = if self.rows.len() as u64 == u64::from(self.capacity) {
            self.capacity
                .checked_add(32)
                .context("resource state capacity overflow")?
        } else {
            self.capacity
        };
        ensure!(
            u64::from(capacity) <= capacity_budget as u64,
            "resource state capacity budget exceeded"
        );
        let required = usize::try_from(capacity)?;
        if required > self.rows.capacity() {
            self.rows.try_reserve_exact(required - self.rows.len())?;
        }
        let index = self.rows.len();
        self.rows.push(Record { key, value });
        self.capacity = capacity;
        Ok(SetOutcome::Inserted { index })
    }

    pub fn reset_values(&mut self, budget: Budget) -> Result<usize> {
        let count = self.rows.len().min(usize::try_from(self.reset_limit)?);
        ensure!(
            count <= budget.work,
            "resource state reset work budget exceeded"
        );
        for row in &mut self.rows[..count] {
            row.value = 0;
        }
        Ok(count)
    }

    pub fn capture_nonzero(&self, budget: Budget) -> Result<Snapshot> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.rows.len().min(CAPTURE_LIMIT).min(budget.records))?;
        let mut work = Work {
            remaining: budget.work,
        };
        for row in &self.rows {
            work.take(1)?;
            if row.value == 0 {
                continue;
            }
            ensure!(
                rows.len() < budget.records,
                "resource capture row budget exceeded"
            );
            rows.push(*row);
            if rows.len() == CAPTURE_LIMIT {
                return Ok(Snapshot {
                    rows,
                    source_cap_reached: true,
                });
            }
        }
        Ok(Snapshot {
            rows,
            source_cap_reached: false,
        })
    }

    /// Host budget/allocation failures are atomic: prepare privately, then
    /// publish. Native admission is checked before inspecting the row payload.
    pub fn restore_admitted(
        &mut self,
        admission: Admission,
        rows: &[Record],
        budget: Budget,
    ) -> Result<RestoreOutcome> {
        if !admission.captured || admission.saved_generation != admission.current_generation {
            return Ok(RestoreOutcome::Rejected);
        }
        ensure!(
            rows.len() <= CAPTURE_LIMIT,
            "resource snapshot exceeds source capture bound"
        );
        ensure!(
            u64::from(self.capacity) <= budget.records as u64,
            "resource restore capacity budget exceeded"
        );
        let mut work = Work {
            remaining: budget.work,
        };
        work.take(self.rows.len())?;
        let mut staged_rows = Vec::new();
        staged_rows.try_reserve_exact(usize::try_from(self.capacity)?)?;
        staged_rows.extend_from_slice(&self.rows);
        let mut staged = Self {
            rows: staged_rows,
            capacity: self.capacity,
            reset_limit: self.reset_limit,
            origin: self.origin,
        };
        let reset_count = staged.rows.len().min(usize::try_from(staged.reset_limit)?);
        work.take(reset_count)?;
        for row in &mut staged.rows[..reset_count] {
            row.value = 0;
        }
        let mut applied = 0;
        for row in rows {
            work.take(1)?;
            // Source7174 skips key0 without find/create or a value write.
            if row.key == 0 {
                continue;
            }
            staged.set_valid(row.key, row.value, budget.records, &mut work)?;
            applied += 1;
        }
        *self = staged;
        Ok(RestoreOutcome::Restored {
            rows_applied: applied,
        })
    }
}

#[cfg(test)]
#[path = "tests/owner.rs"]
mod tests;
