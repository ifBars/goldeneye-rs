//! Actual fresh default inventory ownership and supplied-data FP remapping.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    edb::Edb,
    resource::GameAssets,
    weapon_metadata as metadata,
    weapon_static::{LoadBudget, QueryBudget, WeaponOrder, WeaponTables},
};
use serde::Serialize;

const DEFAULT_ITEM: u32 = 0x0200_0830;

pub(super) struct Record {
    pub key: u32,
    pub count: u32,
    pub limit: u32,
    pub flags: u32,
    pub ordinal: u8,
}
pub(super) struct InitialInventory {
    guns: Edb,
    tables: WeaponTables,
    order: WeaponOrder,
    pub record: Record,
    pub selected: usize,
    pub requests: [[u32; 3]; 2],
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    pub inventory_key: u32,
    pub records: usize,
    pub selected_record: usize,
    pub count: u32,
    pub limit: u32,
    pub flags: u32,
    pub ordinal_raw: u8,
    pub guns_resource: u32,
    pub guns_payload_bytes: u32,
    pub metadata_source_index: usize,
    pub metadata_primary_key: u32,
    pub weapon_registry_keys: usize,
    pub weapon_order_keys: usize,
    pub selected_order_index: usize,
    pub primary_request: u32,
    pub secondary_request: u32,
    pub static_data: ge_assets::weapon_static::Identity,
}

impl InitialInventory {
    pub fn load(assets: &mut GameAssets, component: &Edb) -> Result<Self> {
        let guns =
            super::super::super::resource::load(assets, metadata::RESOURCE, 16 * 1024 * 1024)?;
        let row = metadata::find_inventory(&guns, DEFAULT_ITEM, 4096)?
            .context("default inventory metadata absent")?;
        validate_metadata(row)?;
        let record = Record {
            key: row.inventory_key,
            count: 1,
            limit: 1,
            flags: 0,
            ordinal: u8::MAX,
        };
        let tables = assets.disc.weapon_tables(LoadBudget {
            max_dol_bytes: 16 * 1024 * 1024,
            retained_data_bytes: 512 * 1024,
            registry_keys: 1024,
        })?;
        let order = tables.weapon_order(1024)?;
        // The one source record has signed ordinal -1, so ordinal cycling has
        // no candidates. Native selection next scans this supplied key order.
        let selected_order_index = order
            .first_index(record.key)
            .context("default inventory not in supplied weapon order")?;
        let selected = 0;
        let view = component.animation_modes()?;
        let budget = QueryBudget {
            map_rows: 4096,
            remap_steps: 64,
        };
        let primary = tables.map_fp_normal(record.key, 0x0900_071e, 0, budget, |mode| {
            Ok(view.resolve_mode(mode)?.is_some())
        })?;
        let secondary = tables.map_fp_normal(record.key, 0x0900_0765, 0, budget, |mode| {
            Ok(view.resolve_mode(mode)?.is_some())
        })?;
        let requests = [
            [primary.output, 0, 0x0900_065e],
            [secondary.output, 0, 0x0900_000b],
        ];
        let evidence = Evidence {
            inventory_key: record.key,
            records: 1,
            selected_record: selected,
            count: record.count,
            limit: record.limit,
            flags: record.flags,
            ordinal_raw: record.ordinal,
            guns_resource: guns.header.hash,
            guns_payload_bytes: guns.header.payload_bytes,
            metadata_source_index: row.source_index,
            metadata_primary_key: row.primary_key,
            weapon_registry_keys: tables.registry_count(),
            weapon_order_keys: order.len(),
            selected_order_index,
            primary_request: primary.output,
            secondary_request: secondary.output,
            static_data: tables.identity().clone(),
        };
        Ok(Self {
            guns,
            tables,
            order,
            record,
            selected,
            requests,
            evidence,
        })
    }
    pub fn retained(&self) -> bool {
        self.guns.header.hash == self.evidence.guns_resource
            && self.tables.registry_count() == self.evidence.weapon_registry_keys
            && self.order.key_at(self.evidence.selected_order_index) == Some(self.record.key)
            && self.selected == self.evidence.selected_record
    }
}

fn validate_metadata(row: metadata::InventoryMetadata) -> Result<()> {
    ensure!(
        row.inventory_key == DEFAULT_ITEM
            && row.type_key == 0x4b20_0000
            && row.category_key == 0x4b10_0001
            && row.byte_9_raw == 0
            && row.ammunition_raw == [0, 0],
        "unsupported default inventory admission metadata; full alternate/ammunition paths are not initialized"
    );
    Ok(())
}

#[cfg(test)]
#[path = "tests/inventory.rs"]
mod tests;
