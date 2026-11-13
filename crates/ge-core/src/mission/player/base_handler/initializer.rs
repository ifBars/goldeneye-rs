use super::*;

impl Owner {
    /// These six literal stores occur after the outer component helper returns.
    /// No old scalar is read and no surrounding fields are cleared.
    pub(crate) fn initialize_post_component_scalars(&mut self) -> Result<()> {
        self.require(Stage::VirtualBasicComplete)?;
        for (offset, bits) in [
            (0x758, 0),
            (0x75c, 0),
            (0x760, 0x4300_0000),
            (0x768, f32::MAX.to_bits()),
            (0x6b8, 0),
            (0x6dc, 0),
        ] {
            self.w(offset, bits);
        }
        Ok(())
    }
}
