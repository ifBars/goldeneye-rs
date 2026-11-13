use super::*;
use anyhow::Context;

impl PendingReset {
    pub(crate) fn clear_callback_timer(&mut self, player: PlayerId) -> Result<u8> {
        ensure!(
            self.stage == Stage::Complete && self.player == player,
            "callback timer reset owner or completion differs"
        );
        let e59 = self.byte(0xe59).context("callback e59 branch unknown")?;
        self.word_store(0xe60, 0);
        Ok(e59)
    }
}

#[cfg(test)]
#[path = "tests/callback_tail_fixture.rs"]
mod fixture;
