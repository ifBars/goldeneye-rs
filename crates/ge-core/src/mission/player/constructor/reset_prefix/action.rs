use super::*;
impl PendingReset {
    pub(crate) fn begin_fresh_action(&mut self, base: &base_handler::Owner) -> Result<u32> {
        ensure!(
            self.stage == Stage::Complete && self.player == base.player(),
            "initial action requires the same completed reset owner"
        );
        ensure!(
            self.byte(0xe59) == Some(0) && base.word(0x704) == Some(0),
            "initial action requires the captured fresh default handler branch"
        );
        ensure!(
            self.byte(0xc54) == Some(0) && self.word(0x9a4) == Some(0),
            "initial action needs its actual fresh selector inputs"
        );
        // With c54 zero, d00/d01 are not read. Positive-zero heading selects
        // direction -1 and stores it before the caller's component searches.
        self.bytes.insert(0xc7c, 0xff);
        Ok(0x0900_071e)
    }
}
