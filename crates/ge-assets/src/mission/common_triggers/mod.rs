//! Typed common trigger initialization inputs. Classification enable and the
//! supplied subtype filter run before any common payload is read. These are
//! serialized inputs, not allocated/registered trigger objects or readiness.
mod payload;
pub use super::triggers::{Budget, Locator};
pub use payload::Common;

use super::triggers;
use crate::{edb::Table, trigger_classes::Descriptor};
use anyhow::{Result, ensure};

pub struct View<'a> {
    records: triggers::Records<'a>,
}

pub struct Row<'a> {
    record: triggers::Record<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    Disabled,
    Filtered,
    Admitted(Common),
}

pub(crate) fn read<'a>(
    data: &'a [u8],
    maps: &Table,
    reference: u32,
    budget: Budget,
) -> Result<Option<View<'a>>> {
    Ok(triggers::records(data, maps, reference, budget)?.map(|records| View { records }))
}

impl<'a> View<'a> {
    pub fn row_count(&self) -> usize {
        self.records.row_count()
    }

    pub fn row_count_raw(&self) -> i32 {
        self.records.row_count_raw()
    }

    /// Map identity fields are valid; row-specific locator fields are zero.
    pub fn map_locator(&self) -> Locator {
        self.records.map_locator()
    }

    /// Read only original ordinal/record/local-schema/native-type metadata.
    /// Disabled/filtered rows can have unreadable common properties safely.
    pub fn row(&self, ordinal: usize) -> Result<Row<'a>> {
        Ok(Row {
            record: self.records.record_at(ordinal)?,
        })
    }
}

impl Row<'_> {
    pub fn locator(&self) -> Locator {
        self.record.locator
    }

    /// The map loop supplies serialized XYZ to its zone query before the
    /// callback checks descriptor enable/filter. Query W is forced1; the
    /// distinct common initialization pose retains positive-zero W instead.
    /// No classification, header flags, rotation or properties are read here.
    pub fn query_position_bits(&self) -> Result<[u32; 4]> {
        Ok([
            self.record.word(0xc)?,
            self.record.word(0x10)?,
            self.record.word(0x14)?,
            1.0f32.to_bits(),
        ])
    }

    pub fn read_common(
        &self,
        descriptor: &Descriptor,
        subtype_filter: Option<&[u32]>,
        maximum_property_words: usize,
    ) -> Result<Admission> {
        ensure!(
            descriptor.native_type_index == self.record.locator.native_type_index,
            "common trigger classification does not match source native index"
        );
        if descriptor.enable_byte_raw == 0 {
            return Ok(Admission::Disabled);
        }
        if subtype_filter.is_some_and(|filter| !filter.contains(&descriptor.subtype)) {
            return Ok(Admission::Filtered);
        }
        Ok(Admission::Admitted(payload::read(
            &self.record,
            maximum_property_words,
        )?))
    }
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
