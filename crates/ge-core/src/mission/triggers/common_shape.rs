//! Checked common-initializer selector binding only. These types contain no
//! dimensions, geometry, transforms, collision bounds or query readiness.
use super::types::{MapContext, TriggerKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeRequest {
    pub map: MapContext,
    pub ordinal: u16,
    pub selector: u32,
    pub kind: TriggerKind,
    pub zone: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeBinding {
    /// The source common branch addressed a descriptor and consumed its type
    /// byte. The currently supported type0 adds no geometry-related fields.
    SelectorRetained { selector: u32, type_raw: u8 },
}

impl ShapeBinding {
    pub fn selector(self) -> u32 {
        match self {
            Self::SelectorRetained { selector, .. } => selector,
        }
    }

    pub fn type_raw(self) -> u8 {
        match self {
            Self::SelectorRetained { type_raw, .. } => type_raw,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeOutcome {
    Bound(ShapeBinding),
    /// A valid but unsupported addressing/type scope. Malformed reached input
    /// is a factory Result::Err, never this capability outcome.
    Unsupported,
}
