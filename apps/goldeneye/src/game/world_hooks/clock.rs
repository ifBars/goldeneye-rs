//! Actual native-window frame time, retained through startup, menu and loading.
//! This host clock has its own epoch; original engine timing is not inferred.
use anyhow::{Result, ensure};
use ge_core::mission::world_hooks::clock_reset;
use std::time::Duration;

pub(super) struct Clock {
    seconds: f32,
    updates: usize,
}

impl Clock {
    pub fn from_constructor() -> Self {
        Self {
            seconds: 0.0,
            updates: 0,
        }
    }

    pub fn advance(&mut self, elapsed: Duration) -> Result<()> {
        let step = elapsed.as_secs_f32() * f32::from_bits(0x3f80_0000);
        let next = self.seconds + step;
        let updates = self
            .updates
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("engine frame-clock update count overflow"))?;
        ensure!(next.is_finite(), "host engine frame-clock overflow");
        self.seconds = next;
        self.updates = updates;
        Ok(())
    }

    pub fn bits(&self) -> u32 {
        self.seconds.to_bits()
    }
    pub fn updates(&self) -> usize {
        self.updates
    }
}

impl clock_reset::Clock for Clock {
    fn read(&mut self) -> Result<u32> {
        Ok(self.bits())
    }
}

/// A lease of the actual cached frame word. Every source getter is still called;
/// holding the manager lock prevents a concurrent frame update during the hook.
pub(super) struct SnapshotClock(pub u32);
impl clock_reset::Clock for SnapshotClock {
    fn read(&mut self) -> Result<u32> {
        Ok(self.0)
    }
}
