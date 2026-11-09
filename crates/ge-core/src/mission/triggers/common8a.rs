//! Ordered initial hook admission only. A reached hook has real external
//! effects and remains unsupported; the actual skipped subset is common-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    HeaderDisabled,
    NumericBitClear,
    RequiresHook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    gate: Gate,
}

impl State {
    /// Source does not read numeric0 when headerbit0 already disables the hook.
    pub fn from_serialized(header_flags_raw: u32, numeric0: impl FnOnce() -> u32) -> Self {
        Self {
            gate: if header_flags_raw & 1 != 0 {
                Gate::HeaderDisabled
            } else if numeric0() & 1 == 0 {
                Gate::NumericBitClear
            } else {
                Gate::RequiresHook
            },
        }
    }
    pub fn gate(self) -> Gate {
        self.gate
    }
}
