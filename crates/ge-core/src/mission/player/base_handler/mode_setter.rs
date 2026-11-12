use super::*;
impl Owner {
    pub(crate) fn begin_primary_mode_update(&mut self) -> Result<()> {
        self.require(Stage::VirtualBasicComplete)?;
        self.w(0x1c0, 0);
        self.w(0x6e4, 0);
        ensure!(
            self.word(0x4f8) == Some(0),
            "primary mode setter requires the actual interface dirty-channel provider"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/mode_setter.rs"]
mod tests;
