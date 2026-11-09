//! SubtypeAC's in-place auxiliaries and post-common state. Padding has explicit
//! unknown markers; source zero stores never initialize those padding bytes.
use super::numeric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Constructor,
    PostCommon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Auxiliary {
    words: [u32; 6],
    flags: [u8; 2],
    padding: [Option<u8>; 2],
}

impl Auxiliary {
    fn constructor() -> Self {
        Self {
            words: [0; 6],
            flags: [0; 2],
            padding: [None; 2],
        }
    }
    fn reset_mode_one(&mut self) {
        self.flags = [0; 2];
        self.words = [0; 6];
        // +1a/+1b are deliberately untouched by both source initializations.
    }
    pub fn words(&self) -> &[u32; 6] {
        &self.words
    }
    pub fn flag_bytes(&self) -> [u8; 2] {
        self.flags
    }
    pub fn padding(&self) -> [Option<u8>; 2] {
        self.padding
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    stage: Stage,
    radius_squared_bits: u32,
    extra_words: [Option<u32>; 6],
    auxiliary_records: [Auxiliary; 4],
}

impl State {
    pub fn constructor() -> Self {
        Self {
            stage: Stage::Constructor,
            radius_squared_bits: 0,
            extra_words: [None; 6],
            auxiliary_records: std::array::from_fn(|_| Auxiliary::constructor()),
        }
    }
    pub fn from_serialized(numeric1_raw: u32) -> Self {
        let mut state = Self::constructor();
        state.initialize(numeric1_raw);
        state
    }
    pub fn initialize(&mut self, numeric1_raw: u32) {
        let radius = numeric::signed_tenths(numeric1_raw);
        self.radius_squared_bits = if radius == 0.0 {
            f32::MAX.to_bits()
        } else {
            let first = numeric::signed_tenths(numeric1_raw);
            let second = numeric::signed_tenths(numeric1_raw);
            (second * first).to_bits()
        };
        self.extra_words = [Some(0); 6];
        for auxiliary in &mut self.auxiliary_records {
            auxiliary.reset_mode_one();
        }
        self.stage = Stage::PostCommon;
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn radius_squared_bits(&self) -> u32 {
        self.radius_squared_bits
    }
    pub fn extra_words(&self) -> &[Option<u32>; 6] {
        &self.extra_words
    }
    pub fn auxiliary_records(&self) -> &[Auxiliary; 4] {
        &self.auxiliary_records
    }
}

#[cfg(test)]
#[path = "tests/aux_ac_stages.rs"]
mod tests;
