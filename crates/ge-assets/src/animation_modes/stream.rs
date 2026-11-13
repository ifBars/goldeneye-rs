//! Reached scalar reads for a live command. This cursor owns offsets, never a
//! second payload or resource binding. The caller supplies the same resident view.
use super::{Bytes, CommandLocation, View, add};
use anyhow::{Context, Result, ensure};
#[derive(Debug)]
pub struct CommandStream {
    command: CommandLocation,
    set: Option<usize>,
    group_at: Option<usize>,
    group_count: Option<usize>,
    group: usize,
    children: Option<i16>,
}
impl CommandStream {
    pub fn new(command: CommandLocation) -> Self {
        Self {
            command,
            set: None,
            group_at: None,
            group_count: None,
            group: 0,
            children: None,
        }
    }
    pub fn set_reference(&self, view: &View<'_>) -> Result<u32> {
        self.require_set_opcode()?;
        Bytes(view.data).u32(add(self.command.address, 4)?)
    }
    pub fn resolve_set(&mut self, view: &View<'_>, reference: u32) -> Result<Option<usize>> {
        self.require_set_opcode()?;
        self.set = crate::reference::resolve(view.data, view.sets, 20, reference)?;
        Ok(self.set)
    }
    pub fn options(&self, view: &View<'_>) -> Result<i16> {
        self.require_set_opcode()?;
        Ok(Bytes(view.data).u16(add(self.command.address, 10)?)? as i16)
    }
    pub fn divisor_override(&self, view: &View<'_>) -> Result<u32> {
        self.require_set_opcode()?;
        Bytes(view.data).u32(add(self.command.address, 12)?)
    }
    pub fn flags_override(&self, view: &View<'_>) -> Result<u16> {
        ensure!(
            matches!(self.command.opcode, 0x0c00_0002 | 0x0c00_0003 | 0x0c00_0005),
            "unsupported command flags operand"
        );
        Bytes(view.data).u16(add(self.command.address, 8)?)
    }
    pub fn begin_set(&mut self, view: &View<'_>, index: usize) -> Result<usize> {
        ensure!(
            self.set == Some(index) && self.group_at.is_none(),
            "live set binding differs or replays"
        );
        let at = view.directory_address(view.sets, index)?;
        self.group_at = Some(view.body_address_at(at)?);
        let count = usize::try_from(Bytes(view.data).u32(add(at, 16)?)?)?;
        self.group_count = Some(count);
        Ok(count)
    }
    pub fn group_slot(&self, view: &View<'_>) -> Result<u16> {
        Bytes(view.data).u16(self.group_address()?)
    }
    pub fn group_weight(&self, view: &View<'_>) -> Result<u32> {
        Bytes(view.data).u32(add(self.group_address()?, 4)?)
    }
    pub fn child_count(&mut self, view: &View<'_>) -> Result<i16> {
        let count = Bytes(view.data).u16(add(self.group_address()?, 2)?)? as i16;
        self.children = Some(count);
        Ok(count)
    }
    pub fn child_divisor(&self, view: &View<'_>, ordinal: usize) -> Result<u16> {
        Bytes(view.data).u16(add(self.child_address(ordinal)?, 6)?)
    }
    pub fn child_flags(&self, view: &View<'_>, ordinal: usize) -> Result<u16> {
        Bytes(view.data).u16(add(self.child_address(ordinal)?, 4)?)
    }
    pub fn child_reference(&self, view: &View<'_>, ordinal: usize) -> Result<u32> {
        Bytes(view.data).u32(self.child_address(ordinal)?)
    }
    pub fn child_word(&self, view: &View<'_>, ordinal: usize) -> Result<u32> {
        Bytes(view.data).u32(add(self.child_address(ordinal)?, 8)?)
    }
    pub fn next_group(&mut self, children: u16) -> Result<()> {
        ensure!(
            self.children == Some(children as i16) && (children as i16) >= 0,
            "live group stride differs from reached child count"
        );
        self.group_at = Some(add(
            add(self.group_address()?, 8)?,
            usize::from(children)
                .checked_mul(12)
                .context("live group stride overflow")?,
        )?);
        self.group = self
            .group
            .checked_add(1)
            .context("live group ordinal overflow")?;
        self.children = None;
        Ok(())
    }
    fn group_address(&self) -> Result<usize> {
        ensure!(
            self.group < self.group_count.context("live set count is unknown")?,
            "live group ordinal is absent"
        );
        self.group_at.context("live group binding is absent")
    }
    fn child_address(&self, ordinal: usize) -> Result<usize> {
        let count = self.children.context("live child count is unknown")?;
        ensure!(
            count >= 0 && ordinal < count as usize,
            "live child ordinal is absent"
        );
        add(
            add(self.group_address()?, 8)?,
            ordinal
                .checked_mul(12)
                .context("live child stride overflow")?,
        )
    }
    fn require_set_opcode(&self) -> Result<()> {
        ensure!(
            matches!(self.command.opcode, 0x0c00_0002 | 0x0c00_0003),
            "unsupported live set opcode"
        );
        Ok(())
    }
}
