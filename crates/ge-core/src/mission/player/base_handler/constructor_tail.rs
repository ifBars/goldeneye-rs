use super::*;
use anyhow::Context;

impl Owner {
    pub(crate) fn restore_constructor_state_five(&mut self) -> Result<()> {
        self.require(Stage::VirtualBasicComplete)?;
        self.w(0x704, 5);
        Ok(())
    }

    /// The removal helper reads only this signed count before its empty exit.
    pub(crate) fn constructor_override_count(&self) -> Result<u32> {
        self.require(Stage::VirtualBasicComplete)?;
        self.word(0x480)
            .context("constructor override count unknown")
    }
}

#[cfg(test)]
#[path = "tests/constructor_tail.rs"]
mod tests;
