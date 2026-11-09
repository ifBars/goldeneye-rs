use super::*;

pub(crate) struct Profile<'a> {
    pub game_id: &'a [u8; 6],
    pub revision: u8,
    pub dol_bytes: u64,
    pub dol_sha256: &'a str,
    pub data_index: usize,
    pub data_offset: u32,
    pub data_address: u32,
    pub data_bytes: u32,
    pub data_sha256: &'a str,
    pub registry_root: u32,
    pub order_root: u32,
}

pub(crate) fn load_profile<R: Read + Seek + ?Sized>(
    partition: &mut R,
    identity: &DiscIdentity,
    budget: LoadBudget,
    profile: &Profile<'_>,
) -> Result<WeaponTables> {
    let shared = supplied_data::loader::Profile {
        game_id: profile.game_id,
        revision: profile.revision,
        dol_bytes: profile.dol_bytes,
        dol_sha256: profile.dol_sha256,
        data_index: profile.data_index,
        data_offset: profile.data_offset,
        data_address: profile.data_address,
        data_bytes: profile.data_bytes,
        data_sha256: profile.data_sha256,
    };
    supplied_data::loader::load_profile(
        partition,
        identity,
        supplied_data::LoadBudget {
            max_dol_bytes: budget.max_dol_bytes,
            retained_data_bytes: budget.retained_data_bytes,
        },
        &shared,
    )?
    .weapon_tables_at(
        profile.registry_root,
        profile.order_root,
        budget.registry_keys,
    )
}
