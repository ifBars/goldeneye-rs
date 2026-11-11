//! Reached stores to the persistent animation fallback global. Unknown prior
//! history stays unknown until the caller performs its unconditional store.
use anyhow::{Context, Result};
#[derive(Debug, Default)]
pub struct Fallback {
    value: Option<u32>,
    writes: u64,
}
impl Fallback {
    pub fn store(&mut self, value: u32) -> Result<()> {
        self.writes = self
            .writes
            .checked_add(1)
            .context("animation fallback write counter overflow")?;
        self.value = Some(value);
        Ok(())
    }
    pub fn value(&self) -> Result<u32> {
        self.value
            .context("animation fallback global has not been written by the reached caller")
    }
    pub fn writes(&self) -> u64 {
        self.writes
    }
}
