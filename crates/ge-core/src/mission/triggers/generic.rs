//! Fresh generic-trigger post-common state. This derives no provider result
//! and runs no later generic update/event/owner methods. Host f32 precision for
//! nonzero inputs remains unverified against the original PPC runtime.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    radius_bits: u32,
    radius_squared_bits: u32,
    resource_reference: u32,
    mode_bytes: Option<[u8; 2]>,
}

impl State {
    /// Source first rounds signed i32 to f32, then multiplies by f32 0.1.
    /// A valid resource reaches an unavailable provider: mode bytes remain
    /// unknown, rather than fabricating provider value zero/no-resource state.
    pub fn from_serialized(radius_raw: u32, resource_raw: u32) -> Self {
        let radius = super::numeric::signed_tenths(radius_raw);
        let radius_squared_bits = if radius == 0.0 {
            f32::MAX.to_bits()
        } else {
            (radius * radius).to_bits()
        };
        let resource_reference = super::numeric::reference(resource_raw, 0x4200);
        let valid_resource = resource_reference != 0;
        Self {
            radius_bits: radius.to_bits(),
            radius_squared_bits,
            resource_reference,
            mode_bytes: if valid_resource { None } else { Some([0, 0]) },
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
    pub fn mode_bytes(self) -> Option<[u8; 2]> {
        self.mode_bytes
    }
    pub fn requires_resource_provider(self) -> bool {
        self.mode_bytes.is_none()
    }
}
