//! Raw decoder inputs and outputs. None of these types assert live pose timing.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub frame_count: u16,
    pub rotation_count: u8,
    pub translation_mask: [u32; 4],
    pub auxiliary_count: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub scalar_records: usize,
    pub keys: usize,
    pub coefficients: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work {
    pub scalar_records: usize,
    pub keys: usize,
    pub coefficients: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// Source nonpositive-radicand or epsilon/zero branch, relative to the
    /// evaluated XYZ values. This is not a global host/PPC parity assertion.
    ExactBranch,
    /// Ordinary host sqrt approximates the source frsqrte/Newton path. Its
    /// numerical result has NOT been verified against native estimates.
    HostSqrtUnverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rotation {
    pub bone_index: u16,
    pub quaternion_bits: [u32; 4],
    pub precision: Precision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Translation {
    pub bone_index: u16,
    pub translation_bits: [u32; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub frame: u32,
    /// Requested bones only, in ascending bone order.
    pub rotations: Vec<Rotation>,
    /// Requested sparse translation channels only, in ascending bone order.
    pub translations: Vec<Translation>,
    pub valid_mask: [u32; 4],
    pub work: Work,
}

impl DecodedFrame {
    pub fn has_unverified_sqrt(&self) -> bool {
        self.rotations
            .iter()
            .any(|r| r.precision == Precision::HostSqrtUnverified)
    }
}
