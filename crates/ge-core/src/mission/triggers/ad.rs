//! SubtypeAD's initial numeric0 conversion and two token filters. Accepted
//! tokens are retained identifiers, not initial provider/script requests.
//! General host f32 arithmetic remains unverified against PPC execution.
use super::numeric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_bits: u32,
    radius_squared_bits: u32,
    numeric2_reference: u32,
    numeric15_reference: u32,
}

impl State {
    pub fn from_serialized(radius_raw: u32, numeric2_raw: u32, numeric15_raw: u32) -> Self {
        let radius = numeric::signed_tenths(radius_raw);
        Self {
            radius_bits: radius.to_bits(),
            radius_squared_bits: if radius == 0.0 {
                f32::MAX.to_bits()
            } else {
                (radius * radius).to_bits()
            },
            numeric2_reference: numeric::reference(numeric2_raw, 0x4200),
            numeric15_reference: numeric::reference(numeric15_raw, 0x4108),
        }
    }

    pub fn radius_bits(self) -> u32 {
        self.radius_bits
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
    pub fn numeric2_reference(self) -> u32 {
        self.numeric2_reference
    }
    pub fn numeric15_reference(self) -> u32 {
        self.numeric15_reference
    }
}
