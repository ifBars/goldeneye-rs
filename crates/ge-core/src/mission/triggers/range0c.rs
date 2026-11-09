//! Fresh subtype0c range/token state. Host f32 arithmetic for general inputs
//! remains unverified against PPC. No animation lookup or later event runs here.
use super::numeric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_bits: u32,
    radius_squared_bits: u32,
    secondary_radius_bits: u32,
    secondary_radius_squared_bits: u32,
    animation_reference: u32,
    resource_reference: u32,
    mode_low_byte: Option<u8>,
    mode_high_byte: u8,
}

impl State {
    pub fn from_serialized(
        radius_raw: u32,
        secondary_raw: u32,
        animation_raw: u32,
        resource_raw: u32,
    ) -> Self {
        let radius = if radius_raw == 0 {
            f32::from_bits(0x4348_0000)
        } else {
            numeric::signed_tenths(radius_raw)
        };
        let secondary = if secondary_raw == 0 {
            1.0_f32
        } else {
            numeric::signed_tenths(secondary_raw)
        };
        let secondary_squared = secondary * secondary;
        let radius_squared_bits = if radius == 0.0 {
            f32::MAX.to_bits()
        } else {
            (radius * radius).to_bits()
        };
        let resource_reference = numeric::reference(resource_raw, 0x4200);
        Self {
            radius_bits: radius.to_bits(),
            radius_squared_bits,
            secondary_radius_bits: secondary.to_bits(),
            secondary_radius_squared_bits: secondary_squared.to_bits(),
            animation_reference: numeric::reference(animation_raw, 0x0900),
            resource_reference,
            mode_low_byte: if resource_reference == 0 {
                Some(0)
            } else {
                None
            },
            mode_high_byte: 0,
        }
    }
    pub fn radius_bits(self) -> u32 {
        self.radius_bits
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
    pub fn secondary_radius_bits(self) -> u32 {
        self.secondary_radius_bits
    }
    pub fn secondary_radius_squared_bits(self) -> u32 {
        self.secondary_radius_squared_bits
    }
    pub fn animation_reference(self) -> u32 {
        self.animation_reference
    }
    pub fn resource_reference(self) -> u32 {
        self.resource_reference
    }
    pub fn requested_resource(self) -> Option<u32> {
        (self.resource_reference != 0).then_some(self.resource_reference)
    }
    pub fn mode_low_byte(self) -> Option<u8> {
        self.mode_low_byte
    }
    pub fn mode_high_byte(self) -> u8 {
        self.mode_high_byte
    }
    pub fn requires_resource_provider(self) -> bool {
        self.resource_reference != 0
    }
}

pub fn initialize_runtime_flags(runtime_flags_raw: u32) -> u32 {
    runtime_flags_raw & !0x2000
}
