//! Fresh subtype08 byte state, separate from unchanged common numeric words.
//! Later counter events and resource-state provider operations are unsupported.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    requested_resource: Option<u32>,
    mode_low_byte: Option<u8>,
    mode_high_byte: u8,
    base_radius_squared_bits: u32,
}

impl State {
    pub fn from_serialized(resource_raw: u32) -> Self {
        let accepted = super::numeric::reference(resource_raw, 0x4200) != 0;
        Self {
            requested_resource: accepted.then_some(resource_raw),
            // The accepted branch requires a real provider return; no dummy
            // byte is substituted for its low8-bit result.
            mode_low_byte: if accepted { None } else { Some(0) },
            mode_high_byte: 0,
            base_radius_squared_bits: 0,
        }
    }

    pub fn mode_low_byte(self) -> Option<u8> {
        self.mode_low_byte
    }
    /// Common constructor explicitly supplies this byte; subtype08 does not
    /// perform Generic's category/unit split or overwrite the high byte.
    pub fn mode_high_byte(self) -> u8 {
        self.mode_high_byte
    }
    pub fn requested_resource(self) -> Option<u32> {
        self.requested_resource
    }
    pub fn base_radius_squared_bits(self) -> u32 {
        self.base_radius_squared_bits
    }
    pub fn requires_resource_provider(self) -> bool {
        self.requested_resource.is_some()
    }
}
