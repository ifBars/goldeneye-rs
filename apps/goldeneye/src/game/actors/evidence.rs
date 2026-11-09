use anyhow::Result;
use ge_assets::{
    actor_definitions::{self as source, Definitions},
    edb::Edb,
};
use ge_core::mission::{actor_definitions::Registry, source_rng::SourceRngOwner};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub resource_hash: u32,
    pub spreadsheet_hash: u32,
    pub payload_bytes: u32,
    pub resource_shared: bool,
    pub primary_present: bool,
    pub primary_rows: usize,
    pub secondary_present: bool,
    pub secondary_rows: usize,
    pub registered_primary: usize,
    pub duplicate_primary: usize,
    pub runtime_groups: usize,
    pub grouped_members: usize,
    pub group8_members: usize,
    pub fresh_cursors_zero: bool,
    pub selection_calls: usize,
    pub default_record_bound: bool,
    pub special_records_bound: bool,
    pub original_cursor_history_verified: bool,
    pub rng_bound: bool,
    pub rng_algorithm: &'static str,
    pub rng_origin: &'static str,
    pub rng_draws: u64,
    pub rng_state_sha256: String,
    pub original_rng_history_verified: bool,
    pub native9_constructor_ready: bool,
    pub state_sha256: String,
    pub scope: &'static str,
}

impl Evidence {
    pub(super) fn from_source(
        edb: &Edb,
        view: &Definitions<'_>,
        registry: &Registry,
        rng: &SourceRngOwner,
        selection_calls: usize,
        actor_ready: bool,
    ) -> Result<Self> {
        Ok(Self {
            resource_hash: edb.header.hash,
            spreadsheet_hash: source::SHEET,
            payload_bytes: edb.header.payload_bytes,
            resource_shared: true,
            primary_present: view.primary_count().is_some(),
            primary_rows: view.primary_count().unwrap_or(0),
            secondary_present: view.secondary_count().is_some(),
            secondary_rows: view.secondary_count().unwrap_or(0),
            registered_primary: registry.record_count(),
            duplicate_primary: registry.duplicates().len(),
            runtime_groups: registry.group_count(),
            grouped_members: registry.member_count(),
            group8_members: registry
                .groups()
                .iter()
                .find(|group| group.key() == 8)
                .map_or(0, |group| group.count()),
            fresh_cursors_zero: registry.groups().iter().all(|group| group.cursor() == 0),
            selection_calls,
            default_record_bound: false,
            special_records_bound: false,
            original_cursor_history_verified: false,
            rng_bound: true,
            rng_algorithm: "source_two_step_lcg32",
            rng_origin: "fresh_constructor_snapshot",
            rng_draws: rng.draws_since_snapshot(),
            rng_state_sha256: super::fingerprint::rng(rng),
            original_rng_history_verified: false,
            native9_constructor_ready: actor_ready,
            state_sha256: super::fingerprint::state(edb, view, registry, rng, selection_calls)?,
            scope: "Owned primary actor definitions and logical grouped cursor state from source spreadsheet group0. The original EDB is shared with retained CharacterCatalog group1, with no second resource load. Secondary source rows contribute provenance; complete manager defaults, specials and supplied-DOL key flattening remain unbound. One shared source-compatible constructor-snapshot RNG and persistent group cursors feed actual Actor12 provider calls. Original RNG/cursor consumer history remains unverified; prepared constructors and their reference scans do not establish spawned entities, player/world readiness or gameplay. Original rows stay private.",
        })
    }
}
