use super::*;

impl Item {
    pub fn transform_flags_90(&self) -> u16 {
        self.transform_flags_90
    }
    pub fn rotation_mode_93(&self) -> Option<u8> {
        self.rotation_mode_93
    }

    pub(crate) fn enable_initializer_transform_flags(&mut self) -> (u16, u16) {
        let before = self.transform_flags_90;
        self.transform_flags_90 |= 0x000c;
        (before, self.transform_flags_90)
    }
    pub(crate) fn store_initializer_rotation_mode(&mut self) {
        self.rotation_mode_93 = Some(3);
    }
}
