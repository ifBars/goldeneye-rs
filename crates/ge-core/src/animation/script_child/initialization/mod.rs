//! Fresh class-four phase-zero selection and clip-reference setup. Animation
//! bodies and position preparation remain separate, fallible dependencies.
mod cache;
mod clips;
mod selection;
mod state;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub start: i16,
    pub opcode: u8,
    pub next: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    pub component_speed_bits: u32,
    pub scaled_step_bits: u32,
    pub maximum_entries: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLookup {
    Bound(u32),
    Missing,
    Pending,
}
pub trait Source {
    fn file(&self) -> u32;
    fn duration(&mut self) -> Result<u32>;
    fn entry(&mut self, offset: u32) -> Result<Entry>;
    fn counts(&mut self, offset: u32) -> Result<[u8; 2]>;
    fn file_token(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32>;
    fn lookup_file(&mut self, token: u32) -> Result<FileLookup>;
    fn reference(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32>;
    fn blend(&mut self, offset: u32, row: usize) -> Result<u32>;
    fn start(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16>;
    fn end(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16>;
    fn initial_index(&mut self, offset: u32) -> Result<u8>;
    fn flags(&mut self, offset: u32) -> Result<u8>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub inputs: Inputs,
    pub delta_bits: Option<u32>,
    pub duration_raw: Option<u32>,
    pub frame_bits: Option<u32>,
    pub entries_visited: usize,
    pub selected_offset: Option<u32>,
    pub selected_start: Option<i16>,
    pub compared_reference: Option<u32>,
    pub requested: Option<[u8; 2]>,
    pub rows_visited: [usize; 2],
    pub bound: [u8; 2],
    pub pending_file: Option<u32>,
    pub initial_index_raw: Option<u8>,
    pub initial_index_clamped: Option<u8>,
    pub frame_range: Option<i32>,
    pub setup_returned: bool,
    pub flags_raw: Option<u8>,
}
#[derive(Debug)]
pub struct Record {
    file: Option<u32>,
    bytes: [Option<u8>; 0x1c],
    cached: Option<super::position::Binding>,
}
#[derive(Debug)]
pub enum Storage {
    /// Aliases the actual header's +40..+5c constructor storage.
    Inline {
        file: Option<u32>,
        cached: Option<super::position::Binding>,
    },
    Heap(Vec<Record>),
}
impl Store {
    pub fn initialize_phase_zero(
        &mut self,
        id: NodeId,
        source: &mut impl Source,
        inputs: Inputs,
    ) -> Result<()> {
        let header = self
            .headers
            .get_mut(&id)
            .context("phase-zero script child absent")?;
        ensure!(
            header.stage == Stage::AwaitingInitializer && header.initialization.is_none(),
            "phase-zero initialization differs or replays"
        );
        header.stage = Stage::Initializing;
        let mut trace = Trace {
            inputs,
            delta_bits: None,
            duration_raw: None,
            frame_bits: None,
            entries_visited: 0,
            selected_offset: None,
            selected_start: None,
            compared_reference: None,
            requested: None,
            rows_visited: [0; 2],
            bound: [0; 2],
            pending_file: None,
            initial_index_raw: None,
            initial_index_clamped: None,
            frame_range: None,
            setup_returned: false,
            flags_raw: None,
        };
        let result = header.select(source, &mut trace);
        header.initialization = Some(trace);
        if result.is_err() {
            header.stage = Stage::Failed;
        }
        result
    }
}
impl Header {
    pub fn initialization(&self) -> Option<&Trace> {
        self.initialization.as_ref()
    }
    pub fn clip_storage(&self) -> Option<&Storage> {
        self.clips.as_ref()
    }
    /// Physical storage, including records the source has not initialized.
    pub fn clip_word(&self, index: usize, at: usize) -> Option<u32> {
        if at.checked_add(4)? > 0x1c {
            return None;
        }
        let bytes = match self.clips.as_ref()? {
            Storage::Inline { .. } if index == 0 => self
                .bytes
                .get(0x40usize.checked_add(at)?..0x44usize.checked_add(at)?)?,
            Storage::Inline { .. } => return None,
            Storage::Heap(records) => records.get(index)?.bytes.get(at..at.checked_add(4)?)?,
        };
        Some(u32::from_be_bytes([
            bytes[0]?, bytes[1]?, bytes[2]?, bytes[3]?,
        ]))
    }
}

#[cfg(test)]
pub(super) mod tests;
