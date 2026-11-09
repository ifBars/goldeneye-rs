//! Subtype74 initial scalar/file-token processing. The source compares the
//! rounded square with zero, accepts file tokens by TOP BYTE, and clears only
//! bit31 of numeric2. No file load or later component callback runs here.
use super::numeric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_bits: u32,
    radius_squared_bits: u32,
    file_reference: u32,
    numeric2_raw: u32,
}

impl State {
    pub fn from_serialized(radius_raw: u32, file_raw: u32, numeric2_raw: u32) -> Self {
        let radius = numeric::signed_tenths(radius_raw);
        let square = radius * radius;
        Self {
            radius_bits: radius.to_bits(),
            radius_squared_bits: if square == 0.0 {
                f32::MAX.to_bits()
            } else {
                square.to_bits()
            },
            file_reference: if file_raw >> 24 == 1 && file_raw as u16 != 0 {
                file_raw
            } else {
                0
            },
            numeric2_raw: numeric2_raw & 0x7fff_ffff,
        }
    }
    pub fn radius_bits(self) -> u32 {
        self.radius_bits
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
    pub fn file_reference(self) -> u32 {
        self.file_reference
    }
    pub fn numeric2_raw(self) -> u32 {
        self.numeric2_raw
    }
}
