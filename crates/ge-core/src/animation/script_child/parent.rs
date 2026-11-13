use super::*;
impl Store {
    pub(crate) fn copy_parent_phase(&mut self, id: NodeId, bits: u32) -> Result<()> {
        let h = self
            .headers
            .get_mut(&id)
            .context("copied script child absent")?;
        ensure!(
            h.initializer_returned(),
            "phase copy precedes actual factory return"
        );
        h.w(0x80, bits);
        h.w(0x84, bits);
        Ok(())
    }
    pub(crate) fn initialize_parent_scalars(&mut self, id: NodeId, divisor: u32) -> Result<()> {
        let h = self
            .headers
            .get_mut(&id)
            .context("parent script child absent")?;
        ensure!(
            h.initializer_returned(),
            "scalar writes precede script initializer return"
        );
        h.w(0x34, 0);
        h.w(
            0x3c,
            if divisor == 0 {
                1f32
            } else {
                1f32 / (divisor as f32)
            }
            .to_bits(),
        );
        Ok(())
    }
    pub(crate) fn store_parent_word(&mut self, id: NodeId, word: u32) -> Result<()> {
        let h = self
            .headers
            .get_mut(&id)
            .context("parent script word owner absent")?;
        ensure!(
            h.initializer_returned(),
            "parent word precedes actual factory return"
        );
        h.w(0x24, word);
        Ok(())
    }
}
impl Header {
    pub fn initializer_returned(&self) -> bool {
        matches!(
            self.stage,
            Stage::ReturnedWithoutBody | Stage::ReturnedWithoutPlay | Stage::ReturnedInitialized
        )
    }
}
