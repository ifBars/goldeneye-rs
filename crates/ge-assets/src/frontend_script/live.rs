//! Reached scalar reads for live script initialization. Selection never copies
//! a command payload or consumes an unselected instance/clip record.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionEntry {
    pub start: i16,
    pub opcode: u8,
    pub next: u8,
}
impl Script<'_> {
    /// Native timing selection reads only +4 flags. Header and option words,
    /// descriptors and curves are not consumed when both low bits are clear.
    pub fn play_timing_flags(self, offset: u32) -> Result<u32> {
        let b = Bytes(self.data);
        let field = self.address + 0x34;
        ensure!(b.u32(field)? != 0, "live timing table is NULL");
        let table = b.relative(field)?;
        let selector = b.u16(self.command_address(offset)? + 8)?;
        let entry = table
            .checked_add(usize::from(selector) * 4)
            .context("live timing selector overflow")?;
        let bundle = b.relative(entry)?;
        b.u32(
            bundle
                .checked_add(4)
                .context("live timing flags overflow")?,
        )
    }
    pub fn frames_per_second_raw(self) -> Result<u32> {
        Bytes(self.data).u32(self.address + 0xc)
    }
    pub fn duration_raw(self) -> Result<u32> {
        Bytes(self.data).u32(self.address + 4)
    }
    pub fn selection_entry(self, offset: u32) -> Result<SelectionEntry> {
        let at = self.command_address(offset)?;
        let b = Bytes(self.data);
        let start = b.u16(at + 4)? as i16;
        let opcode = b.u8(at)?;
        let next = b.u8(at + 1)?;
        Ok(SelectionEntry {
            start,
            opcode,
            next,
        })
    }
    pub fn clip_counts(self, offset: u32) -> Result<[u8; 2]> {
        let at = self.play_address(offset)?;
        let b = Bytes(self.data);
        Ok([b.u8(at)?, b.u8(at + 3)?])
    }
    pub fn play_initial_index(self, offset: u32) -> Result<u8> {
        Bytes(self.data).u8(self.play_address(offset)? + 2)
    }
    pub fn play_flags(self, offset: u32) -> Result<u8> {
        Bytes(self.data).u8(self.play_address(offset)? + 1)
    }
    pub fn clip_file(
        self,
        offset: u32,
        secondary: bool,
        ordinal: usize,
        normal: u8,
    ) -> Result<u32> {
        Bytes(self.data).u32(self.clip_address(offset, secondary, ordinal, normal)?)
    }
    pub fn clip_reference(
        self,
        offset: u32,
        secondary: bool,
        ordinal: usize,
        normal: u8,
    ) -> Result<u32> {
        Bytes(self.data).u32(self.clip_address(offset, secondary, ordinal, normal)? + 4)
    }
    pub fn clip_blend(self, offset: u32, ordinal: usize) -> Result<u32> {
        Bytes(self.data).u32(self.clip_address(offset, false, ordinal, 0)? + 8)
    }
    pub fn clip_start(
        self,
        offset: u32,
        secondary: bool,
        ordinal: usize,
        normal: u8,
    ) -> Result<i16> {
        Ok(Bytes(self.data).u16(
            self.clip_address(offset, secondary, ordinal, normal)? + if secondary { 8 } else { 12 },
        )? as i16)
    }
    pub fn clip_end(self, offset: u32, secondary: bool, ordinal: usize, normal: u8) -> Result<i16> {
        Ok(Bytes(self.data).u16(
            self.clip_address(offset, secondary, ordinal, normal)?
                + if secondary { 10 } else { 14 },
        )? as i16)
    }
    fn command_address(self, offset: u32) -> Result<usize> {
        self.address
            .checked_add(0x44)
            .and_then(|at| at.checked_add(offset as usize))
            .context("live command offset overflow")
    }
    fn play_address(self, offset: u32) -> Result<usize> {
        self.command_address(offset)?
            .checked_add(0x10)
            .context("live play address overflow")
    }
    fn clip_address(
        self,
        offset: u32,
        secondary: bool,
        ordinal: usize,
        normal: u8,
    ) -> Result<usize> {
        let stride = if secondary { 12 } else { 16 };
        self.play_address(offset)?
            .checked_add(12)
            .and_then(|at| {
                at.checked_add(if secondary {
                    usize::from(normal) * 16
                } else {
                    0
                })
            })
            .and_then(|at| {
                ordinal
                    .checked_mul(stride)
                    .and_then(|step| at.checked_add(step))
            })
            .context("live clip address overflow")
    }
}
