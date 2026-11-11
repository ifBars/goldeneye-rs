use super::{
    Identity, QueryBudget,
    data::{Data, field},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct RecordIndex {
    key: u32,
    address: u32,
    ordinal: usize,
}

pub struct WeaponTables {
    data: Arc<Data>,
    identity: Identity,
    records: Vec<RecordIndex>,
    order_root: u32,
}

/// An opaque pointer obtained only from a typed field, validated when reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapRef(u32);

pub struct WeaponRecord<'a> {
    data: &'a Data,
    index: RecordIndex,
}

impl WeaponRecord<'_> {
    pub fn key(&self) -> u32 {
        self.index.key
    }
    pub fn source_index(&self) -> usize {
        self.index.ordinal
    }
    pub fn source_address(&self) -> u32 {
        self.index.address
    }
    pub fn fp_parent_key(&self) -> Result<u32> {
        self.word(8)
    }
    pub fn tp_parent_key(&self) -> Result<u32> {
        self.word(0x10)
    }
    pub fn normal_map(&self) -> Result<Option<MapRef>> {
        self.map_field(0xc)
    }
    pub fn tp_map(&self) -> Result<Option<MapRef>> {
        self.map_field(0x14)
    }
    pub fn secondary_map(&self) -> Result<Option<MapRef>> {
        self.map_field(0x18)
    }
    fn word(&self, offset: u32) -> Result<u32> {
        self.data.u32(field(self.index.address, offset)?)
    }
    fn map_field(&self, offset: u32) -> Result<Option<MapRef>> {
        let raw = self.word(offset)?;
        Ok((raw != 0).then_some(MapRef(raw)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapLookup {
    pub matched_source_ordinal: Option<usize>,
    pub mapped_raw: u32,
}

/// Runtime-owned source order, deliberately without a Serialize implementation.
pub struct WeaponOrder {
    keys: Vec<u32>,
}
impl WeaponOrder {
    pub fn len(&self) -> usize {
        self.keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    pub fn key_at(&self, index: usize) -> Option<u32> {
        self.keys.get(index).copied()
    }
    pub fn first_index(&self, key: u32) -> Option<usize> {
        self.keys.iter().position(|&value| value == key)
    }
    pub fn keys(&self) -> &[u32] {
        &self.keys
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemapStep {
    pub weapon: u32,
    pub record_source_index: Option<usize>,
    pub matched_source_ordinal: Option<usize>,
    pub candidate_raw: u32,
    pub available: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remap {
    pub source_weapon: u32,
    pub generic: u32,
    pub variant: u32,
    pub output: u32,
    pub mapped: bool,
    pub steps: Vec<RemapStep>,
    pub map_rows_read: usize,
}

impl WeaponTables {
    pub(crate) fn initialize(
        data: impl Into<Arc<Data>>,
        identity: Identity,
        root: u32,
        order_root: u32,
        budget: usize,
    ) -> Result<Self> {
        let data = data.into();
        let mut records = Vec::new();
        let mut keys = BTreeSet::new();
        let mut address = root;
        for ordinal in 0..budget {
            let key = data.u32(address)?;
            if key == 0 {
                return Ok(Self {
                    data,
                    identity,
                    records,
                    order_root,
                });
            }
            ensure!(
                keys.insert(key),
                "unsupported duplicate static weapon registry key {key:#x}"
            );
            records.push(RecordIndex {
                key,
                address,
                ordinal,
            });
            address = field(address, 0x2c)?;
        }
        anyhow::bail!("static weapon registry key budget exhausted")
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn registry_count(&self) -> usize {
        self.records.len()
    }
    pub fn record(&self, key: u32) -> Option<WeaponRecord<'_>> {
        self.records
            .iter()
            .find(|record| record.key == key)
            .copied()
            .map(|index| WeaponRecord {
                data: &self.data,
                index,
            })
    }

    pub fn weapon_order(&self, maximum_words: usize) -> Result<WeaponOrder> {
        self.order_at(self.order_root, maximum_words)
    }

    /// Source801690e0 uses a separate supplied DATA order, not the primary
    /// order's contents. Its fixed profile-relative location is +0x78.
    pub fn secondary_order(&self, maximum_words: usize) -> Result<WeaponOrder> {
        self.order_at(field(self.order_root, 0x78)?, maximum_words)
    }

    fn order_at(&self, root: u32, maximum_words: usize) -> Result<WeaponOrder> {
        let mut keys = Vec::new();
        let mut address = root;
        for _ in 0..maximum_words {
            let key = self.data.u32(address)?;
            if key == 0 {
                return Ok(WeaponOrder { keys });
            }
            keys.push(key);
            address = field(address, 4)?;
        }
        anyhow::bail!("static weapon order word budget exhausted")
    }

    pub fn map_lookup(
        &self,
        map: Option<MapRef>,
        generic: u32,
        variant: u32,
        maximum_rows: usize,
    ) -> Result<MapLookup> {
        self.lookup(map, generic, variant, maximum_rows, &mut 0)
    }

    pub(super) fn lookup(
        &self,
        map: Option<MapRef>,
        generic: u32,
        variant: u32,
        maximum_rows: usize,
        rows: &mut usize,
    ) -> Result<MapLookup> {
        let Some(MapRef(mut address)) = map else {
            return Ok(MapLookup {
                matched_source_ordinal: None,
                mapped_raw: 0,
            });
        };
        let mut ordinal = 0;
        loop {
            ensure!(
                *rows < maximum_rows,
                "static weapon map-row budget exhausted"
            );
            *rows = rows
                .checked_add(1)
                .context("static map-row count overflow")?;
            let key = self.data.u32(address)?;
            if key == 0 {
                return Ok(MapLookup {
                    matched_source_ordinal: None,
                    mapped_raw: 0,
                });
            }
            if key == generic {
                let mapped_raw = if variant <= 3 {
                    self.data.u32(field(address, 4 + variant * 4)?)?
                } else {
                    0
                };
                return Ok(MapLookup {
                    matched_source_ordinal: Some(ordinal),
                    mapped_raw,
                });
            }
            ordinal = ordinal
                .checked_add(1)
                .context("static map ordinal overflow")?;
            address = field(address, 0x14)?;
        }
    }

    /// Narrow source FP normal branch. Mode availability is an explicit lazy
    /// callback; TP/secondary maps and live caller history are not inferred.
    pub fn map_fp_normal(
        &self,
        weapon: u32,
        generic: u32,
        variant: u32,
        budget: QueryBudget,
        available: impl FnMut(u32) -> Result<bool>,
    ) -> Result<Remap> {
        self.map_action(
            weapon,
            generic,
            variant,
            super::MappingKind::FirstPerson,
            budget,
            available,
        )
    }
}
