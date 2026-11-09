//! Explicit player/tuning inputs for the source secondary head-blend clamp.
//! These values are a supplied snapshot, not defaults or an elapsed clock.
pub(super) struct SlotInputs {
    pub player_9a0_bits: u32,
    pub player_ec4_bits: u32,
    pub upper_tuning_bits: u32,
    pub lower_tuning_bits: u32,
}

impl SlotInputs {
    pub fn secondary_blend_bits(self) -> u32 {
        // Keep the native single-precision operations separate; no mul_add.
        let offset = f32::from_bits(self.player_ec4_bits) - 0.0;
        let range = f32::from_bits(self.upper_tuning_bits) - 1.0;
        let fraction = offset / 1.0;
        let scaled = fraction * range;
        let upper = 1.0 + scaled;
        let value = f32::from_bits(self.player_9a0_bits);
        if value >= upper {
            return upper.to_bits();
        }
        let lower = f32::from_bits(self.lower_tuning_bits);
        if value <= lower {
            return self.lower_tuning_bits;
        }
        self.player_9a0_bits
    }
}
