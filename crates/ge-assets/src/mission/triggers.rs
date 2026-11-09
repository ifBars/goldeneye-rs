//! Shared lazy map-trigger traversal. Consumers classify records before reading
//! their type-specific flags/properties; unconsumed record fields stay unread.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
    pub rows: usize,
    pub types: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            maps: 32_768,
            rows: 65_536,
            types: 65_536,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Locator {
    pub map_reference: u32,
    pub map_directory_index: usize,
    pub map_hash: u32,
    pub map_address: usize,
    pub trigger_block: usize,
    pub trigger_ordinal: usize,
    pub record_address: usize,
    pub schema_index: u16,
    pub schema_address: usize,
    pub native_type_index: u32,
}

pub(super) struct Records<'a> {
    bytes: Bytes<'a>,
    locator: Locator,
    count: usize,
    count_raw: i32,
    next: usize,
    budget: Budget,
}

pub(super) struct Record<'a> {
    bytes: Bytes<'a>,
    pub locator: Locator,
}

impl Record<'_> {
    pub fn word(&self, offset: usize) -> Result<u32> {
        self.bytes.u32(
            self.locator
                .record_address
                .checked_add(offset)
                .context("trigger field offset overflow")?,
        )
    }
}

pub(super) fn records<'a>(
    data: &'a [u8],
    maps: &Table,
    reference: u32,
    budget: Budget,
) -> Result<Option<Records<'a>>> {
    ensure!(maps.name == "maps", "trigger lookup requires map table six");
    ensure!(
        usize::from(maps.count) <= budget.maps,
        "trigger map directory budget exhausted"
    );
    let Some(index) = crate::reference::resolve(data, maps, 16, reference)? else {
        return Ok(None);
    };
    let b = Bytes(data);
    let row = maps
        .offset
        .checked_add(index.checked_mul(16).context("map row overflow")?)
        .context("map directory offset overflow")?;
    let map_address =
        usize::try_from(b.u32(row.checked_add(8).context("map body field overflow")?)?)?;
    ensure!(
        b.u32(map_address)? == 0x500,
        "trigger lookup requires a map body"
    );
    let trigger_block = required_relative(
        b,
        map_address
            .checked_add(0x58)
            .context("map trigger field overflow")?,
        "map trigger block absent",
    )?;
    let count_raw = b.u32(trigger_block)? as i32;
    let count = if count_raw <= 0 {
        0
    } else {
        usize::try_from(count_raw)?
    };
    ensure!(count <= budget.rows, "trigger row budget exhausted");
    Ok(Some(Records {
        bytes: b,
        locator: Locator {
            map_reference: reference,
            map_directory_index: index,
            map_hash: b.u32(row)?,
            map_address,
            trigger_block,
            trigger_ordinal: 0,
            record_address: 0,
            schema_index: 0,
            schema_address: 0,
            native_type_index: 0,
        },
        count,
        count_raw,
        next: 0,
        budget,
    }))
}

impl<'a> Records<'a> {
    pub fn row_count(&self) -> usize {
        self.count
    }

    pub fn row_count_raw(&self) -> i32 {
        self.count_raw
    }

    pub fn map_locator(&self) -> Locator {
        self.locator
    }

    pub fn next_record(&mut self) -> Result<Option<Record<'a>>> {
        if self.next == self.count {
            return Ok(None);
        }
        let record = self.record_at(self.next)?;
        self.next += 1;
        Ok(Some(record))
    }

    pub fn record_at(&self, ordinal: usize) -> Result<Record<'a>> {
        ensure!(ordinal < self.count, "trigger ordinal is out of range");
        let b = self.bytes;
        let block = self.locator.trigger_block;
        let directory = required_relative(
            b,
            block
                .checked_add(4)
                .context("trigger directory field overflow")?,
            "map trigger row directory absent",
        )?;
        let field = directory
            .checked_add(
                ordinal
                    .checked_mul(8)
                    .context("trigger row stride overflow")?,
            )
            .context("trigger row offset overflow")?;
        let record = required_relative(b, field, "map trigger record absent")?;
        let schema_index = b.u16(record)?;
        ensure!(
            usize::from(schema_index) < self.budget.types,
            "trigger schema budget exhausted"
        );
        let schemas = required_relative(
            b,
            block
                .checked_add(12)
                .context("trigger schema field overflow")?,
            "map trigger schema directory absent",
        )?;
        let schema = schemas
            .checked_add(
                usize::from(schema_index)
                    .checked_mul(16)
                    .context("trigger schema stride overflow")?,
            )
            .context("trigger schema offset overflow")?;
        let native_type_index = b.u32(schema)?;
        let locator = Locator {
            trigger_ordinal: ordinal,
            record_address: record,
            schema_index,
            schema_address: schema,
            native_type_index,
            ..self.locator
        };
        Ok(Record { bytes: b, locator })
    }
}

fn required_relative(bytes: Bytes<'_>, field: usize, message: &'static str) -> Result<usize> {
    ensure!(bytes.u32(field)? != 0, "{message}");
    bytes.relative(field)
}
