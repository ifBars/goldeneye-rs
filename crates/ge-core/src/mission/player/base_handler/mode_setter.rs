use super::*;
use anyhow::Context;
impl Owner {
    pub(crate) fn begin_secondary_mode_update(&mut self) -> Result<()> {
        self.require(Stage::VirtualBasicComplete)?;
        self.w(0x1c0, 0);
        Ok(())
    }

    pub(crate) fn begin_primary_mode_update(&mut self) -> Result<()> {
        self.require(Stage::VirtualBasicComplete)?;
        self.w(0x1c0, 0);
        let interface = self
            .word(0x4f8)
            .context("primary mode dirty interface is unknown")?;
        self.w(0x6e4, 0);
        ensure!(
            interface == 0,
            "primary mode setter requires the actual interface dirty-channel provider"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/mode_setter.rs"]
mod tests;
