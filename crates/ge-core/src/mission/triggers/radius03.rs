//! Subtype03 derives only radius7c and conditionally stores a real provider's
//! low byte. Both serialized numeric inputs remain unchanged.
use super::{MapContext, TriggerId, numeric};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub trigger: TriggerId,
    pub map: MapContext,
    pub ordinal: u16,
    pub zone: u16,
    pub resource: u32,
    pub fallback: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_squared_bits: u32,
    requested_resource: Option<u32>,
    mode_low_byte: Option<u8>,
}

impl State {
    pub fn from_serialized(radius_raw: u32, resource_raw: u32) -> Self {
        let first = numeric::signed_tenths(radius_raw);
        let second = numeric::signed_tenths(radius_raw);
        let resource = numeric::reference(resource_raw, 0x4200);
        Self {
            radius_squared_bits: (second * first).to_bits(),
            requested_resource: (resource != 0).then_some(resource),
            mode_low_byte: if resource == 0 { Some(0) } else { None },
        }
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
    pub fn requested_resource(self) -> Option<u32> {
        self.requested_resource
    }
    pub fn mode_high_byte(self) -> u8 {
        0
    }
    pub fn mode_low_byte(self) -> Option<u8> {
        self.mode_low_byte
    }
    pub(super) fn complete(&mut self, result: u32) {
        self.mode_low_byte = Some(result as u8);
    }
}
