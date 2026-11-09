//! SubtypeA3 writes only radius+0x7c. Its signed square then 0.01 multiply
//! differs from squaring a value already scaled by 0.1. Host f32 arithmetic
//! for nonzero inputs remains unverified against original PPC execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_squared_bits: u32,
}

impl State {
    pub fn from_serialized(raw: u32) -> Self {
        let converted = (raw as i32) as f32;
        let square = converted * converted;
        let scaled_square = square * f32::from_bits(0x3c23_d70a);
        Self {
            radius_squared_bits: if scaled_square == 0.0 {
                f32::MAX.to_bits()
            } else {
                scaled_square.to_bits()
            },
        }
    }

    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
}
