//! FP secondary blend coordinates. These values do not change child speed,
//! resolve an animation, or advance an update.

/// Raw scalar inputs retained by the controller that owns the action callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Globals {
    pub upper_basis_bits: u32,
    pub dispatch_lower_bits: u32,
    pub suffix_lower_bits: u32,
}

impl Globals {
    /// Verified initialized source values for a new controller lifetime.
    pub const fn from_boot() -> Self {
        Self {
            upper_basis_bits: 0x3f00_0000,
            dispatch_lower_bits: 0,
            suffix_lower_bits: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowerKind {
    Dispatch,
    Suffix,
}

/// Evaluate the two callers' separately rounded arithmetic and ordered bounds.
/// The upper comparison takes priority even when the bounds are inverted.
/// An unordered test falls through; a NaN input retains its original payload.
pub fn coordinate_bits(
    ec4_bits: u32,
    input_bits: u32,
    globals: Globals,
    lower_kind: LowerKind,
) -> u32 {
    let ec4 = f32::from_bits(ec4_bits);
    let input = f32::from_bits(input_bits);
    let basis = f32::from_bits(globals.upper_basis_bits);
    let lower = f32::from_bits(match lower_kind {
        LowerKind::Dispatch => globals.dispatch_lower_bits,
        LowerKind::Suffix => globals.suffix_lower_bits,
    });
    let numerator = ec4 - 0.0f32;
    let ratio = numerator / 1.0f32;
    let difference = basis - 1.0f32;
    let term = ratio * difference;
    let upper = 1.0f32 + term;
    if input >= upper {
        upper.to_bits()
    } else if input <= lower {
        lower.to_bits()
    } else {
        input_bits
    }
}

#[cfg(test)]
mod tests;
