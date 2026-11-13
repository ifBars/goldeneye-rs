use super::*;

impl PendingReset {
    pub(crate) fn clear_constructor_effect_word(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::Complete,
            "constructor cleanup reset incomplete"
        );
        self.word_store(0xb84, 0);
        Ok(())
    }

    pub(crate) fn clear_constructor_marker(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::Complete,
            "constructor cleanup reset incomplete"
        );
        self.bytes.insert(0xbd1, 0);
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/constructor_tail.rs"]
mod tests;
