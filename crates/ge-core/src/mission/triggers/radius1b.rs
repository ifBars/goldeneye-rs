//! Subtype1b owns radius7c, retaining raw numeric0. The separate dispatcher
//! writes runtime numeric14 zero. No token provider or later callback runs.
use super::numeric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_squared_bits: u32,
}

impl State {
    pub fn from_serialized(raw: u32) -> Self {
        let radius = numeric::signed_tenths(raw);
        let radius_squared_bits = if radius == 0.0 {
            f32::MAX.to_bits()
        } else {
            let first = numeric::signed_tenths(raw);
            let second = numeric::signed_tenths(raw);
            (second * first).to_bits()
        };
        Self {
            radius_squared_bits,
        }
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
}
