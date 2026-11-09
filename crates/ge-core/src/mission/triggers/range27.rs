//! Subtype27's supported map-common continuation shares Range0c's scalar and
//! token calculations. Common has overwritten every numeric slot before this
//! input snapshot, so pre-common constructor defaults are not fallback inputs.
//! Later file/entity/component binding is outside this initial interface.
use super::range0c;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    base: range0c::State,
}

impl State {
    pub fn from_serialized(
        radius_raw: u32,
        secondary_raw: u32,
        animation_raw: u32,
        resource_raw: u32,
    ) -> Self {
        Self {
            base: range0c::State::from_serialized(
                radius_raw,
                secondary_raw,
                animation_raw,
                resource_raw,
            ),
        }
    }

    pub fn range0c_state(&self) -> &range0c::State {
        &self.base
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
    pub fn requires_resource_provider(self) -> bool {
        self.base.requires_resource_provider()
    }
}

/// The source clear is conditional on subtype0c, and is not reached for27.
pub fn initialize_runtime_flags(runtime_flags_raw: u32) -> u32 {
    runtime_flags_raw
}
