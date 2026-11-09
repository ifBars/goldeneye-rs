//! Fresh shapeless subtype06 initialization. Resource-provider activation and
//! later event/update behavior are unsupported; general host f32 parity is open.
use super::numeric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_bits: u32,
    radius_squared_bits: u32,
    resource_reference: u32,
    provider_activation: Option<bool>,
}
impl State {
    pub fn from_serialized(radius_raw: u32, resource_raw: u32) -> Self {
        let radius = numeric::signed_tenths(radius_raw);
        let squared = radius * radius;
        let resource_reference = numeric::reference(resource_raw, 0x4200);
        Self {
            radius_bits: radius.to_bits(),
            radius_squared_bits: if squared == 0.0 {
                f32::MAX.to_bits()
            } else {
                squared.to_bits()
            },
            resource_reference,
            provider_activation: if resource_reference == 0 {
                Some(false)
            } else {
                None
            },
        }
    }
    pub fn radius_bits(self) -> u32 {
        self.radius_bits
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
    pub fn resource_reference(self) -> u32 {
        self.resource_reference
    }
    pub fn requested_resource(self) -> Option<u32> {
        (self.resource_reference != 0).then_some(self.resource_reference)
    }
    pub fn provider_activation(self) -> Option<bool> {
        self.provider_activation
    }
    pub fn requires_resource_provider(self) -> bool {
        self.resource_reference != 0
    }
}
