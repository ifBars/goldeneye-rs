//! Typed weapon tables retained from a verified supplied DOL DATA section.
//!
//! The disc adapter hashes bounded original input without executing it. Only
//! the selected DATA section survives loading; no public executable-byte or
//! arbitrary-address reader is provided. These tables establish no inventory,
//! animation allocation, sampled pose or camera readiness.
mod data;
mod loader;
mod tables;

pub use tables::{MapLookup, MapRef, Remap, RemapStep, WeaponOrder, WeaponRecord, WeaponTables};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadBudget {
    pub max_dol_bytes: u64,
    pub retained_data_bytes: usize,
    /// Includes the terminating zero key; only keys are read at bootstrap.
    pub registry_keys: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryBudget {
    /// Aggregate across all reached maps, including their terminating keys.
    pub map_rows: usize,
    pub remap_steps: usize,
}

/// Safe metadata only; supplied bytes stay in the shared private DATA owner.
pub use crate::supplied_data::Identity;

pub(crate) use loader::load;

#[cfg(test)]
mod tests;
