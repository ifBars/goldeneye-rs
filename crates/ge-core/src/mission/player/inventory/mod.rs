//! Actual fresh inventory container/admission and bounded normal slot selection.
//! Remaining PlayerNew reset operations, components and publication are separate.
mod components;
mod pose;
use anyhow::{Context, Result, ensure};
pub use pose::PoseInputs;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub records: usize,
    pub order_words: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub inventory_key: u32,
    pub primary_key: u32,
    pub type_key: u32,
    pub category_key: u32,
    pub byte_8_raw: u8,
    pub byte_9_raw: u8,
    pub ammunition_raw: [u32; 2],
    pub source_index: usize,
    pub native_sorted_index: usize,
    pub normal_values_bits: [u32; 10],
    pub modifier_bits: [u32; 4],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub key: u32,
    pub count: i32,
    pub limit: i32,
    pub flags: u16,
    pub ordinal: u8,
    pub padding: Option<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearReceipt {
    pub previous_capacity: usize,
    pub previous_count: usize,
    pub had_backing: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Slots {
    pub c96: Option<u8>,
    pub c97: Option<u8>,
    pub c98: Option<u8>,
    pub c99: Option<u8>,
    pub c55: Option<u8>,
    pub ca0: Option<u8>,
    pub pose_bits: Option<[u32; 5]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub primary_record: usize,
    pub primary_order_index: usize,
    pub secondary_record: Option<usize>,
    pub metadata_sorted_index: usize,
    pub pose_bits: [u32; 5],
    pub interface_callbacks: usize,
}
pub trait FlagProvider {
    /// Actual owned player+4f8 link; constructor-null is distinct from unavailable.
    fn interface_bound(&mut self) -> Result<bool>;
    fn clear_interface_flags(&mut self, mask: u32) -> Result<()>;
}
#[derive(Debug)]
pub struct Inventory {
    records: Vec<Record>,
    capacity: usize,
    growth: usize,
    slots: Slots,
    selection: Option<Selection>,
}
impl Inventory {
    pub fn from_constructor() -> Self {
        Self {
            records: Vec::new(),
            capacity: 0,
            growth: 16,
            slots: Slots::default(),
            selection: None,
        }
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn growth(&self) -> usize {
        self.growth
    }
    pub fn slots(&self) -> &Slots {
        &self.slots
    }
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }
    pub fn reset(&mut self) -> ClearReceipt {
        let receipt = ClearReceipt {
            previous_capacity: self.capacity,
            previous_count: self.records.len(),
            had_backing: self.capacity != 0,
        };
        self.records = Vec::new();
        self.capacity = 0;
        self.selection = None;
        // 8010b650 does not write player slot bytes or the growth field.
        receipt
    }
    pub fn admit_default(&mut self, metadata: Metadata, budget: Budget) -> Result<usize> {
        ensure!(
            self.records.is_empty() && self.capacity == 0,
            "fresh default admission requires the actual reset empty container"
        );
        ensure!(
            metadata.inventory_key == 0x0200_0830
                && metadata.type_key == 0x4b20_0000
                && metadata.category_key == 0x4b10_0001
                && metadata.byte_8_raw == 0
                && metadata.byte_9_raw == 0
                && metadata.ammunition_raw == [0; 2],
            "default admission requires unsupported ammunition/variant metadata"
        );
        ensure!(
            self.growth <= budget.records,
            "inventory backing record budget exhausted"
        );
        let mut records = Vec::new();
        records.try_reserve_exact(self.growth)?;
        records.push(Record {
            key: metadata.inventory_key,
            count: 1,
            limit: 1,
            flags: 0,
            ordinal: 0xff,
            padding: None,
        });
        self.records = records;
        self.capacity = self.growth;
        Ok(0)
    }
    pub fn select_default<P: FlagProvider>(
        &mut self,
        primary_order: &[u32],
        secondary_order: &[u32],
        metadata: &Metadata,
        inputs: PoseInputs,
        provider: &mut P,
        budget: Budget,
    ) -> Result<Selection> {
        ensure!(
            self.records.len() == 1
                && self.records[0].key == metadata.inventory_key
                && self.records[0].count == 1
                && self.records[0].ordinal == 0xff
                && self.records[0].flags == 0,
            "unsupported default inventory selection state"
        );
        ensure!(
            primary_order
                .len()
                .checked_add(secondary_order.len())
                .context("inventory order size overflow")?
                <= budget.order_words,
            "inventory selection order budget exhausted"
        );
        self.selection = None;
        self.slots.c96 = Some(0xff);
        self.slots.c97 = Some(0xff);
        let primary_order_index = primary_order
            .iter()
            .position(|&key| key == self.records[0].key)
            .context("default inventory absent from actual primary order")?;
        self.slots.c96 = Some(0);
        self.slots.c97 = Some(0);
        let secondary_record = secondary_order.iter().find_map(|key| {
            self.records
                .iter()
                .position(|record| record.key == *key && record.count != 0)
        });
        self.slots.c99 = Some(secondary_record.map_or(0xff, |index| index as u8));
        ensure!(
            metadata.native_sorted_index <= i8::MAX as usize,
            "default metadata index exceeds supported signed slot width"
        );
        self.slots.c98 = Some(metadata.native_sorted_index as u8);
        self.slots.c55 = Some(0xff);
        self.slots.ca0 = Some(0);
        let pose_bits = pose::normal(metadata, inputs, &mut self.slots.pose_bits)?;
        self.slots.pose_bits = Some(pose_bits);
        let mut interface_callbacks = 0;
        for mask in [0x40, 0x80, 0x100] {
            // The source reloads the interface link at each conditional call.
            if provider.interface_bound()? {
                provider.clear_interface_flags(mask)?;
                interface_callbacks += 1;
            }
        }
        let selected = Selection {
            primary_record: 0,
            primary_order_index,
            secondary_record,
            metadata_sorted_index: metadata.native_sorted_index,
            pose_bits,
            interface_callbacks,
        };
        self.selection = Some(selected);
        Ok(selected)
    }
    /// Canonical sparse fields; no source arrays or native padding image.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(emit: &mut impl FnMut(u32), value: usize) {
            let value = value as u64;
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        wide(&mut emit, self.capacity);
        wide(&mut emit, self.growth);
        wide(&mut emit, self.records.len());
        for record in &self.records {
            emit(record.key);
            emit(record.count as u32);
            emit(record.limit as u32);
            emit(u32::from(record.flags));
            emit(u32::from(record.ordinal));
            emit(u32::from(record.padding.is_some()));
            if let Some(value) = record.padding {
                emit(u32::from(value));
            }
        }
        for field in [
            self.slots.c96,
            self.slots.c97,
            self.slots.c98,
            self.slots.c99,
            self.slots.c55,
            self.slots.ca0,
        ] {
            emit(u32::from(field.is_some()));
            if let Some(value) = field {
                emit(u32::from(value));
            }
        }
        emit(u32::from(self.slots.pose_bits.is_some()));
        if let Some(words) = self.slots.pose_bits {
            for word in words {
                emit(word);
            }
        }
        emit(u32::from(self.selection.is_some()));
    }
}
#[cfg(test)]
mod tests;
