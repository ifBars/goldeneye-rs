use super::*;
use anyhow::Context;

impl Owner {
    pub(crate) fn write_callback_state(&mut self, state: u32) -> Result<Option<u32>> {
        self.require(Stage::VirtualBasicComplete)?;
        ensure!(matches!(state, 2 | 3), "unsupported callback state");
        // Diagnostic only: the source overwrites this field without reading it.
        let initial = self.word(0x704);
        self.w(0x704, state);
        Ok(initial)
    }

    pub(crate) fn finish_callback_flags(&mut self) -> Result<(u32, u32)> {
        self.require(Stage::VirtualBasicComplete)?;
        let before = self.word(0x220).context("callback handler flags unknown")?;
        self.w(0x708, 1);
        let after = before | 1;
        self.w(0x220, after);
        Ok((before, after))
    }
}

#[cfg(test)]
#[path = "tests/callback_tail_fixture.rs"]
mod fixture;
