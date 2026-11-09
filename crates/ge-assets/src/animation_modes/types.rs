//! Opaque source fields and ordered requests, never evaluated animation poses.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub commands: usize,
    pub groups: usize,
    /// Aggregate child records across all groups in one reached set.
    pub children: usize,
}

/// Only the source hash and count are consumed here. Section/body fields remain
/// lazy, including for descriptors whose count is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub directory_index: usize,
    pub descriptor_address: usize,
    pub hash: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    StartSet {
        source_address: usize,
        set_reference: u32,
        flags_override: u16,
        options_raw: i16,
        divisor_override_raw: u32,
    },
    /// Retain the operation without inventing layer clearing/allocation success.
    MaskOperation {
        source_address: usize,
        mask_raw: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    pub descriptor: Descriptor,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub source_address: usize,
    pub slot: u16,
    pub weight_bits: u32,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Child {
    pub source_address: usize,
    pub reference: u32,
    pub flags_raw: u16,
    pub divisor_raw: u16,
    pub word8_raw: u32,
}
