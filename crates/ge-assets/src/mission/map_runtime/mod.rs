//! Narrow fresh-map constructor and lazy publication inputs. Retained file bytes
//! do not establish logical section publication.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
    pub zones: usize,
    pub placements: usize,
    pub component_slots: usize,
    pub auxiliary_records: usize,
    pub bitset_bits: usize,
    pub section_keys: usize,
    pub placement_links: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            maps: 128,
            zones: 256,
            placements: 1_000_000,
            component_slots: 1_000_000,
            auxiliary_records: 1_000_000,
            bitset_bits: 1_000_000,
            section_keys: 256,
            placement_links: 1_000_000,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneInput {
    pub initial_values_bits: [u32; 4],
    pub flag_word10_raw: u32,
    pub word2c_raw: u32,
}
#[derive(Debug)]
pub struct Initializer {
    pub declared_bounds_bits: [[u32; 3]; 2],
    pub zones: Vec<ZoneInput>,
    pub placement_count: usize,
    pub component_slots: usize,
    pub auxiliary_records: usize,
    pub bitset_bits: usize,
    pub section_count: usize,
}
#[derive(Debug, Clone)]
pub struct SectionKeys {
    pub count_raw: i16,
    pub keys: Vec<u32>,
}
#[derive(Debug, Clone, Copy)]
pub struct ZonePublication {
    pub section_index: usize,
    pub root_reference: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct Root {
    pub binding_count: u8,
    pub dynamic: u8,
}
pub struct View<'a> {
    data: &'a [u8],
    tables: &'a [Table],
    descriptor: super::world::MapDescriptor,
    budget: Budget,
    zones: usize,
    placements: usize,
}
fn plus(base: usize, offset: usize) -> Result<usize> {
    base.checked_add(offset)
        .context("map runtime address overflow")
}
fn row(base: usize, index: usize, stride: usize) -> Result<usize> {
    plus(
        base,
        index
            .checked_mul(stride)
            .context("map runtime stride overflow")?,
    )
}
pub(crate) fn read<'a>(
    data: &'a [u8],
    tables: &'a [Table],
    reference: u32,
    budget: Budget,
) -> Result<Option<View<'a>>> {
    let Some(descriptor) = super::world::read(
        data,
        tables.get(6).context("missing map directory")?,
        reference,
        super::world::Budget { maps: budget.maps },
    )?
    else {
        return Ok(None);
    };
    let b = Bytes(data);
    let zones = b.u32(plus(descriptor.body_address, 0x84)?)? as usize;
    let placements = b.u32(plus(descriptor.body_address, 0x48)?)? as usize;
    ensure!(
        zones <= budget.zones && placements <= budget.placements,
        "map runtime count budget exhausted"
    );
    Ok(Some(View {
        data,
        tables,
        descriptor,
        budget,
        zones,
        placements,
    }))
}
impl View<'_> {
    pub fn zone_count(&self) -> usize {
        self.zones
    }
    fn zone_address(&self, zone: usize) -> Result<usize> {
        ensure!(zone < self.zones, "map runtime zone outside directory");
        row(plus(self.descriptor.body_address, 0x88)?, zone, 0x8c)
    }
    pub fn initializer(&self) -> Result<Initializer> {
        let b = Bytes(self.data);
        let map = self.descriptor.body_address;
        let bitset_bits = b.u32(plus(map, 0x28)?)? as usize;
        let component_slots = b.u32(plus(map, 0x40)?)? as usize;
        let auxiliary_records = b.u32(plus(map, 0x50)?)? as usize;
        ensure!(
            bitset_bits <= self.budget.bitset_bits
                && component_slots <= self.budget.component_slots
                && auxiliary_records <= self.budget.auxiliary_records,
            "map runtime constructor budget exhausted"
        );
        let mut zones = Vec::new();
        zones.try_reserve_exact(self.zones)?;
        for zone in 0..self.zones {
            let pointer = plus(self.zone_address(zone)?, 4)?;
            ensure!(b.u32(pointer)? != 0, "map zone identifier is null");
            let id = b.relative(pointer)?;
            let mut values = [0; 4];
            for (i, value) in values.iter_mut().enumerate() {
                *value = b.u32(plus(id, i * 4)?)?;
            }
            zones.push(ZoneInput {
                initial_values_bits: values,
                flag_word10_raw: b.u32(plus(id, 0x10)?)?,
                word2c_raw: b.u32(plus(id, 0x2c)?)?,
            });
        }
        let sections = self.tables.first().context("missing section directory")?;
        let signed_count = sections.count as i16;
        ensure!(
            signed_count > 0,
            "fresh map requires positive section directory count"
        );
        Ok(Initializer {
            declared_bounds_bits: self.descriptor.declared_bounds_bits,
            zones,
            placement_count: self.placements,
            component_slots,
            auxiliary_records,
            bitset_bits,
            section_count: signed_count as usize,
        })
    }
    pub fn section_keys(&self) -> Result<SectionKeys> {
        let table = self.tables.first().context("missing section directory")?;
        let count_raw = table.count as i16;
        let count = usize::try_from(count_raw.max(0))?.min(256);
        ensure!(
            count <= self.budget.section_keys,
            "section context key budget exhausted"
        );
        let mut keys = Vec::new();
        keys.try_reserve_exact(count)?;
        for index in 0..count {
            keys.push(Bytes(self.data).u32(row(table.offset, index, 16)?)?);
        }
        Ok(SectionKeys { count_raw, keys })
    }
    /// Reads references only; no root body or placement-link payload is reached.
    pub fn zone_publication(&self, zone: usize) -> Result<ZonePublication> {
        let b = Bytes(self.data);
        let address = self.zone_address(zone)?;
        let section_ref = b.u32(plus(address, 0x34)?)?;
        let section_index = crate::reference::resolve(
            self.data,
            self.tables.first().context("missing section directory")?,
            16,
            section_ref,
        )?
        .context("map zone section reference missing")?;
        Ok(ZonePublication {
            section_index,
            root_reference: b.u32(address)?,
        })
    }
    pub fn zone_placement_indices(&self, zone: usize) -> Result<Vec<usize>> {
        let b = Bytes(self.data);
        let field = plus(self.zone_address(zone)?, 0x28)?;
        ensure!(b.u32(field)? != 0, "map zone placement-link block is null");
        let block = b.relative(field)?;
        let count = b.u32(block)? as usize;
        ensure!(
            count <= self.budget.placement_links,
            "map zone placement-link budget exhausted"
        );
        let mut indices = Vec::new();
        indices.try_reserve_exact(count)?;
        if count == 0 {
            return Ok(indices);
        }
        let list_field = plus(block, 4)?;
        ensure!(b.u32(list_field)? != 0, "map zone placement list is null");
        let list = b.relative(list_field)?;
        for index in 0..count {
            let placement = usize::from(b.u16(row(list, index, 2)?)?);
            ensure!(
                placement < self.placements,
                "map zone placement index outside map"
            );
            indices.push(placement);
        }
        Ok(indices)
    }
    /// The reached fresh publication path only publishes section zero. Validate
    /// descriptor section BEFORE touching body address, tag or root +54/+55.
    pub fn zone_root(&self, zone: usize) -> Result<Root> {
        let reference = Bytes(self.data).u32(self.zone_address(zone)?)?;
        let table = self
            .tables
            .get(1)
            .context("missing geometry reference directory")?;
        let index = crate::reference::resolve(self.data, table, 16, reference)?
            .context("map zone root reference missing")?;
        let entry = row(table.offset, index, 16)?;
        let b = Bytes(self.data);
        ensure!(b.u32(entry)? == 0, "external map zone root unsupported");
        ensure!(
            b.u16(plus(entry, 4)?)? & 0x7fff == 0,
            "map zone root body section is unrequested"
        );
        let body = b.u32(plus(entry, 8)?)? as usize;
        ensure!(
            b.u32(body)? == 0x608,
            "fresh map zone root type unsupported"
        );
        Ok(Root {
            binding_count: b.range(plus(body, 0x54)?, 1)?[0],
            dynamic: b.range(plus(body, 0x55)?, 1)?[0],
        })
    }
}
pub(crate) fn entity_section(data: &[u8], table: &Table, index: usize) -> Result<usize> {
    ensure!(
        (table.count as i16) > 0 && index < table.count as usize,
        "entity section index outside directory"
    );
    Ok(usize::from(
        Bytes(data).u16(plus(row(table.offset, index, 20)?, 4)?)? & 0x7fff,
    ))
}
#[cfg(test)]
mod tests;
