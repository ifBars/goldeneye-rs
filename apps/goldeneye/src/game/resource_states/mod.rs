//! Shared pure resource state retained for the native window's lifetime.
//! This explicitly starts from the source constructor snapshot; it does not
//! establish the original profile's saved or intervening runtime history.
mod evidence;

use std::sync::{Arc, Mutex};

use anyhow::{Result, ensure};
pub use evidence::Evidence;
use ge_core::mission::resource_state::{Budget, Owner};

const BUDGET: Budget = Budget {
    records: 65_536,
    work: 65_536,
};

#[derive(Clone)]
pub(super) struct Shared(Arc<Mutex<Owner>>);

impl Shared {
    pub fn from_constructor() -> Self {
        Self(Arc::new(Mutex::new(Owner::from_constructor(2500))))
    }

    /// Source NewGame's flag1 handoff clears the value prefix, retaining the
    /// store itself. Called on the CPU worker, never from a window callback.
    pub fn new_game_handoff(&self) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource-state owner lock is poisoned"))?
            .reset_values(BUDGET)?;
        Ok(())
    }
}

pub(super) struct Attempt {
    shared: Shared,
    lookups: usize,
    hits: usize,
    misses: usize,
}

impl Attempt {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            lookups: 0,
            hits: 0,
            misses: 0,
        }
    }

    pub fn lookup(&mut self, token: u32, fallback: u32) -> Result<u32> {
        ensure!(
            self.lookups < 65_536,
            "resource-state query budget exhausted"
        );
        let result = self
            .shared
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource-state owner lock is poisoned"))?
            .lookup(token, fallback, BUDGET)?;
        self.lookups += 1;
        if result.matched {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        Ok(result.value)
    }

    pub fn evidence(&self) -> Result<Evidence> {
        let owner = self
            .shared
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource-state owner lock is poisoned"))?;
        Ok(Evidence::snapshot(
            &owner,
            self.lookups,
            self.hits,
            self.misses,
        ))
    }

    /// Actual value setter used by player reset. This shares the retained
    /// owner and leaves trigger-lookup counters independent of writes.
    pub fn set(
        &mut self,
        token: u32,
        value: u32,
        mut current: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        current()?;
        let mut owner = self
            .shared
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource-state owner lock is poisoned"))?;
        current()?;
        let outcome = owner.set(token, value, BUDGET)?;
        ensure!(
            !matches!(
                outcome,
                ge_core::mission::resource_state::SetOutcome::Invalid
            ),
            "player reset resource token is invalid"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
