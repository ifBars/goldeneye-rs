//! Typed tables sharing one verified supplied DATA5 owner. The executable is
//! streamed for identity checks; only its checked data section is retained.
//! Original instructions are never exposed, retained or executed by this adapter.
mod data;
pub(crate) mod loader;
use anyhow::Result;
pub(crate) use data::{Data, field};
pub(crate) use loader::load;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadBudget {
    pub max_dol_bytes: u64,
    pub retained_data_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Identity {
    pub game_id: String,
    pub revision: u8,
    pub main_dol_partition_offset: u64,
    pub dol_bytes: u64,
    pub dol_sha256: String,
    pub data_section_index: usize,
    pub data_virtual_address: u32,
    pub data_bytes: usize,
    pub data_sha256: String,
}

pub struct SuppliedData {
    data: Arc<Data>,
    identity: Identity,
}
impl SuppliedData {
    pub(crate) fn initialize(data: Data, identity: Identity) -> Self {
        Self {
            data: Arc::new(data),
            identity,
        }
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn player_reset_defaults(&self) -> Result<crate::player_static::ResetDefaults> {
        crate::player_static::ResetDefaults::read(&self.data)
    }
    pub fn weapon_tables(
        &self,
        registry_keys: usize,
    ) -> Result<crate::weapon_static::WeaponTables> {
        self.weapon_tables_at(0x8057_8ab0, 0x8057_4d70, registry_keys)
    }
    pub fn trigger_classes(&self) -> crate::trigger_classes::View<'_> {
        crate::trigger_classes::View::from_data(&self.data)
    }
    pub(crate) fn weapon_tables_at(
        &self,
        registry: u32,
        order: u32,
        keys: usize,
    ) -> Result<crate::weapon_static::WeaponTables> {
        crate::weapon_static::WeaponTables::initialize(
            Arc::clone(&self.data),
            self.identity.clone(),
            registry,
            order,
            keys,
        )
    }
}
