//! Individual command reads for live execution. Later pointer words stay unread
//! until the caller reaches their ordinal.
use super::{Budget, Bytes, Command, CommandLocation, View, add};
use anyhow::{Context, Result, ensure};
impl View<'_> {
    pub fn command_count(&self, index: usize, row: usize, maximum: usize) -> Result<usize> {
        Ok(self.command_list(index, row, maximum)?.1)
    }
    pub fn command_location(
        &self,
        index: usize,
        row: usize,
        ordinal: usize,
        maximum: usize,
    ) -> Result<CommandLocation> {
        let (list, count) = self.command_list(index, row, maximum)?;
        ensure!(ordinal < count, "animation command ordinal is absent");
        self.location_at(list, ordinal)
    }
    pub fn command(
        &self,
        index: usize,
        row: usize,
        ordinal: usize,
        maximum: usize,
    ) -> Result<Command> {
        let (list, count) = self.command_list(index, row, maximum)?;
        ensure!(ordinal < count, "animation command ordinal is absent");
        self.command_at(list, ordinal)
    }
    /// Inspection convenience: live execution uses one reached ordinal at a time.
    pub fn commands(&self, index: usize, row: usize, budget: Budget) -> Result<Vec<Command>> {
        let (list, count) = self.command_list(index, row, budget.commands)?;
        let mut commands = Vec::new();
        commands.try_reserve_exact(count)?;
        for ordinal in 0..count {
            commands.push(self.command_at(list, ordinal)?);
        }
        Ok(commands)
    }
    fn command_list(&self, index: usize, row: usize, maximum: usize) -> Result<(usize, usize)> {
        let at = self.transition_address(index, row)?;
        let list = self.required_relative(add(at, 4)?, "transition command list")?;
        let count = usize::try_from(Bytes(self.data).u32(list)?)?;
        ensure!(count <= maximum, "animation command budget exhausted");
        Ok((list, count))
    }
    fn command_at(&self, list: usize, ordinal: usize) -> Result<Command> {
        let location = self.location_at(list, ordinal)?;
        self.decode_command(location)
    }
    fn location_at(&self, list: usize, ordinal: usize) -> Result<CommandLocation> {
        let b = Bytes(self.data);
        let field = add(
            add(list, 4)?,
            ordinal
                .checked_mul(4)
                .context("animation command offset overflow")?,
        )?;
        let source_address = self.required_relative(field, "animation command")?;
        let opcode = b.u32(source_address)?;
        Ok(CommandLocation {
            address: source_address,
            opcode,
        })
    }
    fn decode_command(&self, location: CommandLocation) -> Result<Command> {
        let b = Bytes(self.data);
        let source_address = location.address;
        let opcode = location.opcode;
        let command = match opcode {
            0x0c00_0002 => {
                let set_reference = b.u32(add(source_address, 4)?)?;
                let flags_override = b.u16(add(source_address, 8)?)?;
                let options_raw = b.u16(add(source_address, 10)?)? as i16;
                let divisor_override_raw = if options_raw & 1 != 0 {
                    b.u32(add(source_address, 12)?)?
                } else {
                    0
                };
                Command::StartSet {
                    source_address,
                    set_reference,
                    flags_override,
                    options_raw,
                    divisor_override_raw,
                }
            }
            0x0c00_0005 => Command::MaskOperation {
                source_address,
                mask_raw: b.u16(add(source_address, 8)?)?,
            },
            _ => anyhow::bail!(
                "unsupported reached animation opcode {opcode:#010x} at {source_address:#x}"
            ),
        };
        Ok(command)
    }
}
