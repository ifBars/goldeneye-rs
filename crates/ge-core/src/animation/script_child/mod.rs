//! Retained class-four headers and phase-zero initialization. Source pointer
//! roles are typed bindings; unwritten bytes and unperformed effects stay unknown.
pub mod initialization;
mod state;
use super::{
    layers::NodeId,
    nodes::{Ids, Kind, OwnerId},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
pub const HEADER_BYTES: usize = 0x90;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub directory_index: usize,
    pub hash: u32,
}
pub trait Provider {
    fn file(&self) -> u32;
    fn generation(&mut self) -> Result<u32>;
    fn resolve_script(&mut self, reference: u32) -> Result<Option<Binding>>;
    fn bind_body(&mut self, binding: Binding) -> Result<bool>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Constructed,
    AwaitingScript,
    ResolvingScript,
    AwaitingInitializer,
    ReturnedWithoutBody,
    Failed,
    Initializing,
    AwaitingClipFile,
    AwaitingClipRange,
    AwaitingFrameCount,
    AwaitingAnimation,
    ReturnedWithoutPlay,
}
#[derive(Debug)]
pub struct Header {
    owner: OwnerId,
    file: u32,
    reference: u32,
    slot: Option<u8>,
    bytes: [Option<u8>; HEADER_BYTES],
    stage: Stage,
    observed_generation: Option<u32>,
    cached: Option<Binding>,
    body_bound: Option<bool>,
    initialization: Option<initialization::Trace>,
    clips: Option<initialization::Storage>,
}
#[derive(Debug)]
pub struct Store {
    owner: OwnerId,
    maximum: usize,
    headers: BTreeMap<NodeId, Header>,
}
impl Store {
    pub fn from_constructor(owner: OwnerId, maximum: usize) -> Self {
        Self {
            owner,
            maximum,
            headers: BTreeMap::new(),
        }
    }
    pub fn allocate(&mut self, ids: &mut Ids, file: u32, reference: u32) -> Result<NodeId> {
        ensure!(
            ids.owner() == self.owner && self.headers.len() < self.maximum,
            "script header owner or allocation budget differs"
        );
        ensure!(
            file >> 24 == 1 && ((reference >> 24) & 0x7f) == 4,
            "script factory file/class differs"
        );
        let id = ids.allocate(Kind::Script)?;
        self.headers
            .insert(id, Header::from_constructor(self.owner, file, reference));
        Ok(id)
    }
    pub fn get(&self, id: NodeId) -> Option<&Header> {
        self.headers.get(&id)
    }
    pub fn len(&self) -> usize {
        self.headers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }
    pub fn bind_factory_context(&mut self, id: NodeId, slot: u8, flags: u16) -> Result<()> {
        let header = self
            .headers
            .get_mut(&id)
            .context("script header allocation absent")?;
        ensure!(
            header.stage == Stage::Constructed && slot < 4,
            "script factory context differs or replays"
        );
        header.w(0x20, u32::from(flags));
        header.slot = Some(slot);
        header.bytes[0x1c..0x20].fill(None);
        header.stage = Stage::AwaitingScript;
        Ok(())
    }
    pub fn initialize_prefix(&mut self, id: NodeId, source: &mut impl Provider) -> Result<()> {
        let header = self
            .headers
            .get_mut(&id)
            .context("script initializer owner absent")?;
        ensure!(
            header.stage == Stage::AwaitingScript,
            "script initializer is out of source order or replays"
        );
        header.stage = Stage::ResolvingScript;
        let result = header.resolve(source);
        if result.is_err() {
            header.stage = Stage::Failed;
        }
        result
    }
}
impl Header {
    fn from_constructor(owner: OwnerId, file: u32, reference: u32) -> Self {
        let mut header = Self {
            owner,
            file,
            reference,
            slot: None,
            bytes: [None; HEADER_BYTES],
            stage: Stage::Constructed,
            observed_generation: None,
            cached: None,
            body_bound: None,
            initialization: None,
            clips: None,
        };
        // Common control, skeletal instance and class-four constructors, in
        // source order. Self-relative pointers have typed roles, not addresses.
        for (at, word) in [
            (0x18, 0),
            (0x10, reference),
            (0x14, u32::MAX),
            (0x20, 0),
            (0x1c, 0),
            (0x24, 0),
            (0x34, 1f32.to_bits()),
            (0x28, 0),
            (0x3c, 0),
            (0x2c, 1f32.to_bits()),
            (0x4c, 0),
            (0x78, 0),
            (0x7c, 0),
            (0x40, 0),
            (0x44, reference),
            (0x48, u32::MAX),
            (0x50, u32::MAX),
            (0x54, 0),
            (0x64, 0),
            (0x68, (-1f32).to_bits()),
        ] {
            header.w(at, word);
        }
        for at in [0x58, 0x5a, 0x6c, 0x6e, 0x70, 0x72] {
            header.h(at, 0);
        }
        for (at, byte) in [(0x76, 0), (0x75, 0), (0x74, 1), (0x77, 1)] {
            header.bytes[at] = Some(byte);
        }
        for (at, word) in [
            (0x28, 0),
            (0x80, 0),
            (0x84, (-1f32).to_bits()),
            (0x88, u32::MAX),
            (0x40, 0),
            (0x44, u32::MAX),
            (0x48, u32::MAX),
            (0x50, u32::MAX),
            (0x54, 0),
            (0x8c, 0),
        ] {
            header.w(at, word);
        }
        header
    }
    fn resolve(&mut self, source: &mut impl Provider) -> Result<()> {
        ensure!(
            source.file() == self.file,
            "script initializer resource owner differs"
        );
        let generation = source.generation()?;
        self.observed_generation = Some(generation);
        if self.word(0x14) != Some(generation) {
            self.cached = source.resolve_script(self.reference)?;
            if self.cached.is_some() {
                self.bytes[0x18..0x1c].fill(None);
                self.w(0x14, generation);
            } else {
                self.w(0x18, 0);
            }
        }
        let body = if let Some(binding) = self.cached {
            source.bind_body(binding)?
        } else {
            false
        };
        self.body_bound = Some(body);
        self.stage = if body {
            Stage::AwaitingInitializer
        } else {
            Stage::ReturnedWithoutBody
        };
        Ok(())
    }
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn file(&self) -> u32 {
        self.file
    }
    pub fn reference(&self) -> u32 {
        self.reference
    }
    pub fn slot(&self) -> Option<u8> {
        self.slot
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn cached(&self) -> Option<Binding> {
        self.cached
    }
    pub fn observed_generation(&self) -> Option<u32> {
        self.observed_generation
    }
    pub fn body_bound(&self) -> Option<bool> {
        self.body_bound
    }
    pub fn byte(&self, at: usize) -> Option<u8> {
        self.bytes.get(at).copied().flatten()
    }
    pub fn word(&self, at: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.byte(at)?,
            self.byte(at.checked_add(1)?)?,
            self.byte(at.checked_add(2)?)?,
            self.byte(at.checked_add(3)?)?,
        ]))
    }
    fn w(&mut self, at: usize, word: u32) {
        for (i, byte) in word.to_be_bytes().into_iter().enumerate() {
            self.bytes[at + i] = Some(byte);
        }
    }
    fn h(&mut self, at: usize, half: u16) {
        for (i, byte) in half.to_be_bytes().into_iter().enumerate() {
            self.bytes[at + i] = Some(byte);
        }
    }
}

#[cfg(test)]
mod tests;
