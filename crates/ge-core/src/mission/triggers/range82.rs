//! Subtype82 shares the range/token calculations with subtype0c, but preserves
//! runtime bit0x2000 and has additional post-provider scalar/token writes.
use super::{numeric, range0c};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    base: range0c::State,
    numeric5_bits: Option<u32>,
    numeric7_bits: Option<u32>,
    numeric8_reference: Option<u32>,
    word_11c: Option<u32>,
    diagnostic_reached: Option<bool>,
}

impl State {
    pub fn from_serialized(numeric: &[u32; 16]) -> Self {
        let base = range0c::State::from_serialized(numeric[0], numeric[2], numeric[4], numeric[13]);
        let mut state = Self {
            base,
            numeric5_bits: None,
            numeric7_bits: None,
            numeric8_reference: None,
            word_11c: None,
            diagnostic_reached: None,
        };
        // Source returns from the required provider before these writes. A
        // missing accepted resource must not expose invented completed values.
        if !base.requires_resource_provider() {
            let scalar5 = numeric::signed_tenths(numeric[5]);
            state.numeric5_bits = Some(scalar5.to_bits());
            state.numeric7_bits = Some(numeric::signed_tenths(numeric[7]).to_bits());
            state.numeric8_reference = Some(numeric::reference(numeric[8], 0x0b00));
            state.diagnostic_reached = Some(f32::from_bits(base.secondary_radius_bits()) > scalar5);
            state.word_11c = Some(0);
        }
        state
    }

    pub fn radius_bits(self) -> u32 {
        self.base.radius_bits()
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.base.radius_squared_bits()
    }
    pub fn secondary_radius_bits(self) -> u32 {
        self.base.secondary_radius_bits()
    }
    pub fn secondary_radius_squared_bits(self) -> u32 {
        self.base.secondary_radius_squared_bits()
    }
    pub fn animation_reference(self) -> u32 {
        self.base.animation_reference()
    }
    pub fn resource_reference(self) -> u32 {
        self.base.resource_reference()
    }
    pub fn requested_resource(self) -> Option<u32> {
        self.base.requested_resource()
    }
    pub fn mode_low_byte(self) -> Option<u8> {
        self.base.mode_low_byte()
    }
    pub fn mode_high_byte(self) -> u8 {
        self.base.mode_high_byte()
    }
    pub fn numeric5_bits(self) -> Option<u32> {
        self.numeric5_bits
    }
    pub fn numeric7_bits(self) -> Option<u32> {
        self.numeric7_bits
    }
    pub fn numeric8_reference(self) -> Option<u32> {
        self.numeric8_reference
    }
    pub fn word_11c(self) -> Option<u32> {
        self.word_11c
    }
    /// Source diagnostic callbacks are literal no-ops in this release.
    pub fn diagnostic_reached(self) -> Option<bool> {
        self.diagnostic_reached
    }
}

pub fn initialize_runtime_flags(runtime_flags_raw: u32) -> u32 {
    runtime_flags_raw
}
