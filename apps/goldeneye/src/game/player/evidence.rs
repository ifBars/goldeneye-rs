use super::Owned;
use crate::game::world_hooks::Attempt;
use anyhow::{Result, ensure};
use ge_core::mission::{
    player::{base_handler, world::World},
    spawn::SelectionResult,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub origin: &'static str,
    pub lifetime: &'static str,
    pub stage: &'static str,
    pub pending_dependency: &'static str,
    pub operation_complete: bool,
    pub attempt_bound: bool,
    pub catalog_bound: bool,
    pub operation_entries: usize,
    pub creation_dispatches: usize,
    pub catalog_entries: usize,
    pub selected_catalog_index: usize,
    pub selected_source_ordinal: usize,
    pub requested_start_point_before: u32,
    pub requested_start_point_after: u32,
    pub base_constructor_complete: bool,
    pub handler_rng_reads: usize,
    pub base_reset_calls: usize,
    pub persistent_base_resets: usize,
    pub shared_count: usize,
    pub shared_live_leases: usize,
    pub shared_allocations: usize,
    pub shared_native_bytes: usize,
    pub shared_host_capacity_bytes: usize,
    pub shared_reset_calls: usize,
    pub shared_entry_noops: usize,
    pub shared_cache_stores: usize,
    pub shared_reset_complete: bool,
    pub runtime_base_reset_complete: bool,
    pub derived_reset_prefix_complete: bool,
    pub handler_kind_bound: bool,
    pub inventory_records: usize,
    pub inventory_capacity: usize,
    pub inventory_growth: usize,
    pub world_items: usize,
    pub world_actor_lists: usize,
    pub world_membership_lists: usize,
    pub actor_registrations: usize,
    pub physics_bound: bool,
    pub completion_published: bool,
    pub player_created: bool,
    pub camera_ready: bool,
    pub original_history_verified: bool,
    pub native_timing_verified: bool,
    pub base_state_sha256: String,
    pub prelude_state_sha256: String,
    pub world_state_sha256: String,
    pub persistent_state_sha256: String,
    pub shared_state_sha256: String,
    pub reset_state_sha256: String,
    pub state_sha256: String,
    pub scope: &'static str,
}
pub(super) fn build(owned: &Owned, world: &World, hook: &Attempt) -> Result<Evidence> {
    ensure!(
        owned.base.stage() == base_handler::Stage::RuntimeResetComplete,
        "player base constructor has no complete receipt"
    );
    ensure!(
        owned.reset.player() == owned.base.player(),
        "derived reset identity differs"
    );
    ensure!(
        owned.base.completed_resets() == owned.resource_resets,
        "player reset provider receipt differs"
    );
    let selection = owned.request.selection();
    let index = selection
        .selected_index()
        .ok_or_else(|| anyhow::anyhow!("player request has no selected live start point"))?;
    let base = words(b"ge_player_base_handler_v1\0", |emit| {
        owned.base.visit_state_words(emit)
    });
    let prelude = words(b"ge_player_prelude_v1\0", |emit| {
        owned.prelude.visit_state_words(emit)
    });
    let world_hash = words(b"ge_player_actor_world_v1\0", |emit| {
        world.visit_state_words(emit)
    });
    let persistent = hook.player_inputs_fingerprint()?;
    let shared = hook.player_constructor_snapshot(owned.lease)?;
    let reset = words(b"ge_player_derived_reset_prefix_v1\0", |emit| {
        owned.reset.visit_state_words(emit)
    });
    let inventory = owned.prelude.inventory();
    let mut result = Evidence {
        origin: "fresh_constructor_snapshot",
        lifetime: "session_worker",
        stage: "player_reset_scratch_pending",
        pending_dependency: "shared_reset_scratch_capacity",
        operation_complete: false,
        attempt_bound: true,
        catalog_bound: true,
        operation_entries: owned.operations,
        creation_dispatches: 1,
        catalog_entries: owned.catalog_entries,
        selected_catalog_index: index,
        selected_source_ordinal: usize::from(owned.request.ordinal()),
        requested_start_point_before: selection.requested_id_before,
        requested_start_point_after: selection.requested_id_after,
        base_constructor_complete: true,
        handler_rng_reads: owned.rng_reads,
        base_reset_calls: owned.base.completed_resets(),
        persistent_base_resets: hook.player_base_reset_writes()?,
        shared_count: shared.count,
        shared_live_leases: shared.live_leases,
        shared_allocations: shared.allocations,
        shared_native_bytes: shared.native_bytes,
        shared_host_capacity_bytes: shared.host_capacity_bytes,
        shared_reset_calls: shared.reset_calls,
        shared_entry_noops: shared.entry_noops,
        shared_cache_stores: shared.cache_stores,
        shared_reset_complete: false,
        runtime_base_reset_complete: true,
        derived_reset_prefix_complete: true,
        handler_kind_bound: owned.base.word(0x224).is_some(),
        inventory_records: inventory.records().len(),
        inventory_capacity: inventory.capacity(),
        inventory_growth: inventory.growth(),
        world_items: world.item_count(),
        world_actor_lists: 1,
        world_membership_lists: world.membership_count(),
        actor_registrations: world.actors()?.len(),
        physics_bound: false,
        completion_published: false,
        player_created: false,
        camera_ready: false,
        original_history_verified: false,
        native_timing_verified: false,
        base_state_sha256: base,
        prelude_state_sha256: prelude,
        world_state_sha256: world_hash,
        persistent_state_sha256: persistent,
        shared_state_sha256: shared.state_sha256,
        reset_state_sha256: reset,
        state_sha256: String::new(),
        scope: "One generation-bound player attempt uses the actual live StartPoint catalog and shared RNG/resource owners. Three controller-lifetime allocations and their count precede the second base reset. The actual runtime base reset and derived clear/flag prefix execute in source order; the larger reset stops before its unbound shared scratch-capacity owner. Character/inventory selection, EXItem/physics, TP/FP setup, completion and camera remain incomplete. Cancelled partial constructors retain reached shared leases/backings until native cache cleanup is implemented; later attempts reuse storage with distinct player identities. The empty inventory and absent actor membership remain real. No selected pose is applied early. Original history, timing and numeric parity remain unverified; only counts and fingerprints are exported.",
    };
    let mut digest = Sha256::new();
    digest.update(b"ge_player_attempt_v2\0");
    for id in [
        owned.request.attempt().get(),
        owned.request.session().get(),
        owned.request.user().get(),
        owned.request.trigger().get(),
        owned.request.map().instance.get(),
    ] {
        digest.update(id.to_be_bytes());
    }
    digest.update(owned.request.map().map_selector.to_be_bytes());
    digest.update(owned.request.ordinal().to_be_bytes());
    digest.update([match selection.result {
        SelectionResult::Requested { .. } => 1,
        SelectionResult::Fallback { .. } => 2,
        SelectionResult::Missing => 0,
    }]);
    for value in [
        index,
        selection.visits,
        owned.catalog_entries,
        owned.operations,
        owned.rng_reads,
        owned.resource_resets,
    ] {
        digest.update((value as u64).to_be_bytes());
    }
    for word in owned
        .request
        .pose()
        .position_bits
        .into_iter()
        .chain(owned.request.pose().rotation_bits)
    {
        digest.update(word.to_be_bytes());
    }
    digest.update([owned.request.pose().rotation_mode]);
    for text in [
        &result.base_state_sha256,
        &result.prelude_state_sha256,
        &result.world_state_sha256,
        &result.persistent_state_sha256,
        &result.shared_state_sha256,
        &result.reset_state_sha256,
    ] {
        digest.update(text.as_bytes());
    }
    result.state_sha256 = format!("{:x}", digest.finalize());
    Ok(result)
}
fn words(domain: &[u8], visit: impl FnOnce(&mut dyn FnMut(u32))) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    visit(&mut |word| digest.update(word.to_be_bytes()));
    format!("{:x}", digest.finalize())
}
