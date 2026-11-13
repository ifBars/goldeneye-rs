use super::*;
use anyhow::Context;

impl Item {
    pub fn byte_8c(&self) -> u8 {
        self.byte_8c
    }

    pub(crate) fn store_constructor_scale(&mut self, bits: [u32; 4]) {
        self.scale = bits;
    }
    pub(crate) fn invalidate_constructor_pose(&mut self) -> Result<()> {
        self.factory
            .as_mut()
            .context("pose scale item factory owner absent")?
            .byte_8e = 0;
        Ok(())
    }

    pub(crate) fn clear_constructor_scale_dirty(&mut self) -> (u8, u8) {
        let before = self.byte_8c;
        self.byte_8c &= 0xdf;
        (before, self.byte_8c)
    }
    pub(crate) fn constructor_rotation_pair(&self, second: bool) -> [u32; 2] {
        let at = usize::from(second) * 2;
        [self.rotation[at], self.rotation[at + 1]]
    }
    pub(crate) fn store_constructor_rotation_pair(&mut self, second: bool, bits: [u32; 2]) {
        let at = usize::from(second) * 2;
        self.rotation[at..at + 2].copy_from_slice(&bits);
    }
    pub(crate) fn store_constructor_scale_pair(&mut self, second: bool, bits: [u32; 2]) {
        let at = usize::from(second) * 2;
        self.scale[at..at + 2].copy_from_slice(&bits);
    }
    pub(crate) fn mark_constructor_component_pose(&mut self) -> (u8, u8) {
        let before = self.byte_8c;
        self.byte_8c |= 3;
        (before, self.byte_8c)
    }
}

#[cfg(test)]
mod tests;
