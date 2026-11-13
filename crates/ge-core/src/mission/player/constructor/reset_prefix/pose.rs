use super::*;

impl PendingReset {
    pub(crate) fn store_pose_input(&mut self, input: u8) -> Result<()> {
        ensure!(
            self.stage == Stage::Complete,
            "pose input reset owner incomplete"
        );
        self.bytes.insert(0xbab, input);
        Ok(())
    }
    pub(crate) fn store_pose_weapon_mode(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::Complete,
            "pose weapon mode reset owner incomplete"
        );
        self.word_store(0xca4, 0x0e00_0070);
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/pose.rs"]
mod tests;
