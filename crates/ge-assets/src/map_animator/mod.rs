//! Lazy views of map-embedded type-0x38 base animators.
//!
//! These are neither frontend 0x300 scripts nor skeletal animations. Reading a
//! header does not address its states/instructions or execute its create event.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
    /// Exclusive bound on an addressed directory index, not a native count.
    pub animator_indices: usize,
    pub states: usize,
    pub instructions: usize,
    /// Maximum addressed prefix from the selected body, checked lazily.
    pub bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locator {
    pub map_reference: u32,
    pub map_directory_index: usize,
    pub map_hash: u32,
    pub map_address: usize,
    pub trigger_block: usize,
    pub animator_directory: usize,
    pub animator_index: usize,
    pub row_address: usize,
    pub body_address: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Copied by the source constructor to +0xc4. The checked instruction view
    /// uses this as a host bound; the native dispatch does not enforce it.
    pub statement_count_raw: u16,
    pub variables: u8,
    pub shared_variables: u8,
    pub states: u8,
    pub format: u8,
}

#[derive(Clone, Copy)]
pub struct Animator<'a> {
    bytes: Bytes<'a>,
    locator: Locator,
    header: Header,
    budget: Budget,
}

/// A reached 20-byte state; uninterpreted fields remain privately borrowed.
pub struct State<'a> {
    pub index: usize,
    pub address: usize,
    pub program: u16,
    pub behavior_raw: u8,
    pub execution_flag_raw: u8,
    raw: &'a [u8],
}

impl State<'_> {
    pub fn raw(&self) -> &[u8] {
        self.raw
    }
}

/// A reached 12-byte statement. Opcode-specific operands require an independent
/// interpreter; this reader does not turn unknown statements into no-ops.
pub struct Instruction<'a> {
    pub index: usize,
    pub address: usize,
    pub opcode: u8,
    raw: &'a [u8],
}

impl Instruction<'_> {
    pub fn raw(&self) -> &[u8] {
        self.raw
    }
}

pub fn read<'a>(
    data: &'a [u8],
    maps: &Table,
    map_reference: u32,
    animator_index: usize,
    budget: Budget,
) -> Result<Option<Animator<'a>>> {
    ensure!(maps.name == "maps", "map animator requires table six");
    ensure!(
        usize::from(maps.count) <= budget.maps,
        "map animator map budget exhausted"
    );
    let Some(index) = crate::reference::resolve(data, maps, 16, map_reference)? else {
        return Ok(None);
    };
    ensure!(
        animator_index < budget.animator_indices,
        "map animator directory index budget exhausted"
    );
    let b = Bytes(data);
    let map_row = add(maps.offset, mul(index, 16)?)?;
    let map_address = usize::try_from(b.u32(add(map_row, 8)?)?)?;
    ensure!(
        b.u32(map_address)? == 0x500,
        "map animator requires map body"
    );
    let trigger_block = required_relative(b, add(map_address, 0x58)?)?;
    let animator_directory = required_relative(b, add(trigger_block, 8)?)?;
    let row_address = add(animator_directory, mul(animator_index, 8)?)?;
    if b.u32(row_address)? == 0 {
        return Ok(None);
    }
    let body_address = b.relative(row_address)?;
    ensure!(
        budget.bytes >= 0x34,
        "map animator header byte budget exhausted"
    );
    let raw = b.range(body_address, 0x34)?;
    let header = Header {
        statement_count_raw: b.u16(add(body_address, 0x24)?)?,
        variables: raw[0x2d],
        shared_variables: raw[0x2e],
        states: raw[0x2f],
        format: raw[0x30],
    };
    ensure!(header.format == 0x38, "unsupported map animator format");
    Ok(Some(Animator {
        bytes: b,
        locator: Locator {
            map_reference,
            map_directory_index: index,
            map_hash: b.u32(map_row)?,
            map_address,
            trigger_block,
            animator_directory,
            animator_index,
            row_address,
            body_address,
        },
        header,
        budget,
    }))
}

impl<'a> Animator<'a> {
    pub fn locator(&self) -> Locator {
        self.locator
    }

    pub fn header(&self) -> Header {
        self.header
    }

    /// Mapped events are distinct from direct state requests below 0xff.
    /// The returned 0xff is an unmapped state, never a state to address.
    pub fn event_mapping(&self, event: u16) -> Result<u8> {
        ensure!(
            (0xff..=0x11e).contains(&event),
            "map animator mapped event outside header"
        );
        Ok(self.body_range(4 + usize::from(event - 0xff), 1)?[0])
    }

    pub fn state(&self, index: usize) -> Result<State<'a>> {
        ensure!(
            index < usize::from(self.header.states),
            "map animator state absent"
        );
        ensure!(
            index < self.budget.states,
            "map animator state budget exhausted"
        );
        let offset = add(0x34, mul(index, 20)?)?;
        let raw = self.body_range(offset, 20)?;
        Ok(State {
            index,
            address: add(self.locator.body_address, offset)?,
            program: u16::from_be_bytes([raw[0], raw[1]]),
            behavior_raw: raw[15],
            execution_flag_raw: raw[16],
            raw,
        })
    }

    pub fn instruction(&self, index: usize) -> Result<Instruction<'a>> {
        ensure!(
            index < usize::from(self.header.statement_count_raw),
            "map animator instruction outside raw count"
        );
        ensure!(
            index < self.budget.instructions,
            "map animator instruction budget exhausted"
        );
        let stream = add(0x34, mul(usize::from(self.header.states), 20)?)?;
        let offset = add(stream, mul(index, 12)?)?;
        let raw = self.body_range(offset, 12)?;
        Ok(Instruction {
            index,
            address: add(self.locator.body_address, offset)?,
            opcode: raw[0],
            raw,
        })
    }

    fn body_range(&self, offset: usize, length: usize) -> Result<&'a [u8]> {
        ensure!(
            add(offset, length)? <= self.budget.bytes,
            "map animator body byte budget exhausted"
        );
        self.bytes
            .range(add(self.locator.body_address, offset)?, length)
    }
}

fn required_relative(bytes: Bytes<'_>, field: usize) -> Result<usize> {
    ensure!(
        bytes.u32(field)? != 0,
        "map animator directory pointer absent"
    );
    bytes.relative(field)
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).context("map animator offset overflow")
}

fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).context("map animator stride overflow")
}

#[cfg(test)]
mod tests;
