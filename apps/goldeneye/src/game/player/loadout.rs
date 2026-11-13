//! Retained inputs for the reached single-user character and inventory branch.
//! Loading these sources does not run the reset or construct animation layers.
use crate::mission::{MissionContext, resource};
use anyhow::{Context, Result, ensure};
use ge_assets::{
    character::{self, CharacterRow},
    edb::Edb,
    resource::GameAssets,
    supplied_data::{self, SuppliedData},
    weapon_metadata::{self, DefaultMetadata},
    weapon_static::{WeaponOrder, WeaponTables},
};
use ge_core::mission::{
    player::inventory::{Budget, FlagProvider, Inventory, Metadata, PoseInputs, Selection},
    session::{Session, Stage},
};
use sha2::{Digest, Sha256};

const DEFAULT_ITEM: u32 = 0x0200_0830;
const ROW_LIMIT: usize = 4096;
const ORDER_LIMIT: usize = 1024;
const INVENTORY_BUDGET: Budget = Budget {
    records: 32,
    order_words: 2 * ORDER_LIMIT,
};

/// Safe source IDs, counts and a fingerprint of the bound input state only.
/// This contains no source rows, positions, native pointers or readiness flags.
pub(super) struct Provenance {
    pub character_resource: u32,
    pub character_key: u32,
    pub character_source_index: usize,
    pub guns_resource: u32,
    pub guns_payload_bytes: u32,
    pub metadata_source_index: usize,
    pub metadata_sorted_index: usize,
    pub weapon_registry_keys: usize,
    pub primary_order_keys: usize,
    pub secondary_order_keys: usize,
    pub state_sha256: String,
}

pub(super) struct Sources {
    character: CharacterRow,
    guns: Edb,
    supplied: SuppliedData,
    tables: WeaponTables,
    primary: WeaponOrder,
    secondary: WeaponOrder,
    metadata: Metadata,
    provenance: Provenance,
}

impl Sources {
    pub fn metadata_at_selected_slot(&self, slot: u8, inventory: &Inventory) -> Result<&Metadata> {
        selected_metadata(&self.metadata, slot, inventory)
    }
    pub fn map_initial_action(
        &self,
        request: ge_core::mission::player::actions::Request,
        available: impl FnMut(u32) -> Result<bool>,
    ) -> Result<ge_assets::weapon_static::Remap> {
        use ge_assets::weapon_static::{MappingKind, QueryBudget};
        use ge_core::mission::player::actions::Target;
        let kind = match request.target {
            Target::FirstPerson => MappingKind::FirstPerson,
            Target::ThirdPerson => MappingKind::ThirdPerson,
            Target::ThirdPersonSecondary => MappingKind::ThirdPersonSecondary,
        };
        self.tables.map_action(
            request.weapon,
            request.generic,
            request.variant,
            kind,
            QueryBudget {
                map_rows: 4096,
                remap_steps: 64,
            },
            available,
        )
    }
    pub fn load(
        assets: &mut GameAssets,
        context: &MissionContext,
        session: &Session,
    ) -> Result<Self> {
        let level = context
            .registry()
            .selected_primary()
            .context("loadout selected level absent")?;
        validate_session(session, level.hash())?;
        let character = context.selected_character()?.clone();
        let guns = resource::load(assets, weapon_metadata::RESOURCE, 16 * 1024 * 1024)?;
        let metadata = adapt_metadata(
            weapon_metadata::find_default_inventory(&guns, DEFAULT_ITEM, ROW_LIMIT)?
                .context("default inventory metadata absent")?,
        );
        let supplied = assets.disc.supplied_data(supplied_data::LoadBudget {
            max_dol_bytes: 16 * 1024 * 1024,
            retained_data_bytes: 512 * 1024,
        })?;
        let tables = supplied.weapon_tables(1024)?;
        let primary = tables.weapon_order(ORDER_LIMIT)?;
        let secondary = tables.secondary_order(ORDER_LIMIT)?;
        let state_sha256 = fingerprint(&character, &guns, &tables, &primary, &secondary, &metadata);
        let provenance = Provenance {
            character_resource: character::CATALOG_RESOURCE,
            character_key: character.key(),
            character_source_index: character.source_index(),
            guns_resource: guns.header.hash,
            guns_payload_bytes: guns.header.payload_bytes,
            metadata_source_index: metadata.source_index,
            metadata_sorted_index: metadata.native_sorted_index,
            weapon_registry_keys: tables.registry_count(),
            primary_order_keys: primary.len(),
            secondary_order_keys: secondary.len(),
            state_sha256,
        };
        Ok(Self {
            character,
            guns,
            supplied,
            tables,
            primary,
            secondary,
            metadata,
            provenance,
        })
    }

    pub fn selected_character(&self) -> &CharacterRow {
        &self.character
    }
    pub fn supplied_data(&self) -> &SuppliedData {
        &self.supplied
    }
    pub fn provenance(&self) -> &Provenance {
        debug_assert_eq!(self.guns.header.hash, self.provenance.guns_resource);
        debug_assert_eq!(
            self.tables.registry_count(),
            self.provenance.weapon_registry_keys
        );
        &self.provenance
    }

    /// Source reset -> admission -> slot selection on the caller's actual C40
    /// container. Prior scratch/E58/C56 stores remain the reset orchestrator's
    /// responsibility. An error preserves any reached container/slot writes.
    pub fn initialize_inventory(
        &self,
        inventory: &mut Inventory,
        inputs: PoseInputs,
        flags: &mut impl FlagProvider,
    ) -> Result<Selection> {
        initialize(
            inventory,
            &self.metadata,
            self.primary.keys(),
            self.secondary.keys(),
            inputs,
            flags,
        )
    }
}

/// Only the retained selected row is available here. A different positive slot
/// needs its actual row provider, even if its values might match the default.
fn selected_metadata<'a>(
    metadata: &'a Metadata,
    slot: u8,
    inventory: &Inventory,
) -> Result<&'a Metadata> {
    let selection = inventory
        .selection()
        .context("constructor metadata selection is unavailable")?;
    ensure!(
        (slot as i8) >= 0
            && inventory.slots().c98 == Some(slot)
            && usize::from(slot) == selection.metadata_sorted_index
            && selection.metadata_sorted_index == metadata.native_sorted_index,
        "constructor selected metadata row is unavailable or differs from the retained inventory"
    );
    Ok(metadata)
}

fn validate_session(session: &Session, level: u32) -> Result<()> {
    let descriptor = session
        .descriptor()
        .context("loadout session descriptor absent")?;
    ensure!(
        session.stage() == Stage::Active
            && session.active() == Some(session.identity())
            && session.mode() == 3
            && session.backend() == 2
            && descriptor.word_2c_raw == 1
            && descriptor.word_30_raw == 0
            && descriptor.local_user_count == 1
            && !session.multiple_local_users_in_mode_three(),
        "loadout requires the actual admitted single-user branch; MP/history is unsupported"
    );
    ensure!(
        session.request().level_hash() == level && descriptor.level_hashes == [level; 3],
        "loadout level differs from admitted session"
    );
    Ok(())
}

fn adapt_metadata(row: DefaultMetadata) -> Metadata {
    Metadata {
        inventory_key: row.inventory.inventory_key,
        primary_key: row.inventory.primary_key,
        type_key: row.inventory.type_key,
        category_key: row.inventory.category_key,
        byte_8_raw: row.byte_8_raw,
        byte_9_raw: row.inventory.byte_9_raw,
        ammunition_raw: row.inventory.ammunition_raw,
        source_index: row.inventory.source_index,
        native_sorted_index: row.native_sorted_index,
        normal_values_bits: row.normal_values_bits,
        modifier_bits: row.modifier_bits,
    }
}

fn initialize(
    inventory: &mut Inventory,
    metadata: &Metadata,
    primary: &[u32],
    secondary: &[u32],
    inputs: PoseInputs,
    flags: &mut impl FlagProvider,
) -> Result<Selection> {
    inventory.reset();
    inventory.admit_default(*metadata, INVENTORY_BUDGET)?;
    inventory.select_default(
        primary,
        secondary,
        metadata,
        inputs,
        flags,
        INVENTORY_BUDGET,
    )
}

fn fingerprint(
    character: &CharacterRow,
    guns: &Edb,
    tables: &WeaponTables,
    primary: &WeaponOrder,
    secondary: &WeaponOrder,
    metadata: &Metadata,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"ge_player_loadout_inputs_be_v1\0");
    for word in [
        character::CATALOG_RESOURCE,
        guns.header.hash,
        guns.header.version,
        guns.header.payload_bytes,
    ] {
        digest.update(word.to_be_bytes());
    }
    digest.update((character.source_index() as u64).to_be_bytes());
    for word in character.words_raw() {
        digest.update(word.to_be_bytes());
    }
    // Identity fields are length-delimited; only the typed supplied DATA owner
    // is retained by WeaponTables, never executable instructions.
    for text in [
        &tables.identity().dol_sha256,
        &tables.identity().data_sha256,
    ] {
        digest.update((text.len() as u64).to_be_bytes());
        digest.update(text.as_bytes());
    }
    digest.update((tables.registry_count() as u64).to_be_bytes());
    for order in [primary, secondary] {
        digest.update((order.len() as u64).to_be_bytes());
        for word in order.keys() {
            digest.update(word.to_be_bytes());
        }
    }
    for word in [
        metadata.inventory_key,
        metadata.primary_key,
        metadata.type_key,
        metadata.category_key,
        u32::from(metadata.byte_8_raw),
        u32::from(metadata.byte_9_raw),
    ]
    .into_iter()
    .chain(metadata.ammunition_raw)
    .chain(metadata.normal_values_bits)
    .chain(metadata.modifier_bits)
    {
        digest.update(word.to_be_bytes());
    }
    for count in [metadata.source_index, metadata.native_sorted_index] {
        digest.update((count as u64).to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
#[path = "tests/loadout.rs"]
mod tests;
