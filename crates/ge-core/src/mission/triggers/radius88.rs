//! Subtype88 shares the audited A3 scalar expression, then initializes its
//! extra full word118. Constructor unknown storage is not an allocator zero.
use super::a3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Constructor,
    PostCommon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    stage: Stage,
    radius_squared_bits: u32,
    word_118: Option<u32>,
}

impl State {
    pub fn constructor() -> Self {
        Self {
            stage: Stage::Constructor,
            radius_squared_bits: 0,
            word_118: None,
        }
    }
    pub fn from_serialized(raw: u32) -> Self {
        let mut state = Self::constructor();
        state.initialize(raw);
        state
    }
    pub fn initialize(&mut self, raw: u32) {
        self.radius_squared_bits = a3::State::from_serialized(raw).radius_squared_bits();
        self.word_118 = Some(0);
        self.stage = Stage::PostCommon;
    }
    pub fn stage(self) -> Stage {
        self.stage
    }
    pub fn radius_squared_bits(self) -> u32 {
        self.radius_squared_bits
    }
    pub fn word_118(self) -> Option<u32> {
        self.word_118
    }
}
