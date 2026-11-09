use std::fmt;

use crate::mission::actor_definitions::{DefaultReason, RecordId, SpecialKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Predicate,
    NoSelectedLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedDefinition {
    pub record: RecordId,
    pub key: u32,
    pub source_index: usize,
    pub group: Option<u16>,
    pub member_index: Option<usize>,
}

/// One source800e3d20 invocation, including invocations that return NULL.
/// Ordinals distinguish the two calls even when their raw tokens are identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub ordinal: u8,
    pub file: u32,
    pub target: u32,
}

impl Request {
    /// The outer source helper checks the top BYTE only. Prefix-only01000000
    /// reaches its manager; topbyte81 does not, despite the inner helper's mask.
    /// An invalid file still belongs to the ordered plan and can return NULL.
    pub fn is_file_request(self) -> bool {
        self.file >> 24 == 1
    }
}

/// Only fields written by the final source tail, not a complete manager image.
/// No prior value for these or any unwritten field is fabricated at preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TailReset {
    pub words_00_0c: [u32; 4],
    pub word_338: u32,
    pub scalar_340_bits: u32,
    pub byte_344: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationError {
    RequiresDefault { reason: DefaultReason },
    RequiresSpecial { kind: SpecialKind },
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresDefault { reason } => write!(
                formatter,
                "post-trigger selection requires an unbound default definition: {reason:?}"
            ),
            Self::RequiresSpecial { kind } => write!(
                formatter,
                "post-trigger selection requires an unbound special definition: {kind:?}"
            ),
        }
    }
}

impl std::error::Error for PreparationError {}
