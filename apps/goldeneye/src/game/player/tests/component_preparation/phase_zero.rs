use super::*;
use ge_core::animation::script_child::initialization::{Entry, FileLookup, Source};
impl Source for ModeSource {
    fn file(&self) -> u32 {
        0x0102_0017
    }
    fn duration(&mut self) -> Result<u32> {
        self.calls.push("duration");
        Ok(60)
    }
    fn entry(&mut self, _: u32) -> Result<Entry> {
        self.calls.push("entry");
        Ok(Entry {
            start: 0,
            opcode: 0x12,
            next: 0,
        })
    }
    fn counts(&mut self, _: u32) -> Result<[u8; 2]> {
        anyhow::bail!("no-play branch must not read counts")
    }
    fn file_token(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<u32> {
        anyhow::bail!("no-play branch must not read clips")
    }
    fn lookup_file(&mut self, _: u32) -> Result<FileLookup> {
        anyhow::bail!("no-play branch must not look up files")
    }
    fn reference(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<u32> {
        anyhow::bail!("no-play branch must not read reference")
    }
    fn blend(&mut self, _: u32, _: usize) -> Result<u32> {
        anyhow::bail!("no-play branch must not read blend")
    }
    fn start(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<i16> {
        anyhow::bail!("no-play branch must not read range")
    }
    fn end(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<i16> {
        anyhow::bail!("no-play branch must not read range")
    }
    fn initial_index(&mut self, _: u32) -> Result<u8> {
        anyhow::bail!("no-play branch must not read index")
    }
    fn flags(&mut self, _: u32) -> Result<u8> {
        anyhow::bail!("no-play branch must not read flags")
    }
}
