//! Weapon-domain adapter over the shared verified supplied DATA owner.
use super::{LoadBudget, WeaponTables};
use crate::{disc::DiscIdentity, supplied_data};
use anyhow::Result;
use std::io::{Read, Seek};

pub(crate) fn load<R: Read + Seek + ?Sized>(
    partition: &mut R,
    identity: &DiscIdentity,
    budget: LoadBudget,
) -> Result<WeaponTables> {
    supplied_data::load(
        partition,
        identity,
        supplied_data::LoadBudget {
            max_dol_bytes: budget.max_dol_bytes,
            retained_data_bytes: budget.retained_data_bytes,
        },
    )?
    .weapon_tables(budget.registry_keys)
}

#[cfg(test)]
#[path = "tests/loader.rs"]
mod authored;
#[cfg(test)]
pub(super) use authored::{Profile, load_profile};
