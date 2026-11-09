use super::Owned;
mod components;
mod owners;
use crate::game::world_hooks::Attempt;
use anyhow::{Result, ensure};
pub use components::ComponentInitialization;
use ge_core::mission::{
    player::{base_handler, world::World},
    spawn::SelectionResult,
};
pub use owners::{ConstructorFlag, VirtualInitialization};
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
    pub scratch_capacity: usize,
    pub scratch_growth: usize,
    pub scratch_records: usize,
    pub scratch_native_bytes: usize,
    pub scratch_host_capacity_bytes: usize,
    pub scratch_reallocations: usize,
    pub character_resource: usize,
    pub character_key: usize,
    pub character_source_index: usize,
    pub guns_resource: usize,
    pub guns_payload_bytes: usize,
    pub metadata_source_index: usize,
    pub metadata_sorted_index: usize,
    pub weapon_registry_keys: usize,
    pub primary_order_keys: usize,
    pub secondary_order_keys: usize,
    pub inventory_primary_order_index: usize,
    pub inventory_interface_callbacks: usize,
    pub inventory_selected_record: usize,
    pub scratch_prepared: bool,
    pub character_bound: bool,
    pub inventory_selection_complete: bool,
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
    pub scratch_state_sha256: String,
    pub loadout_state_sha256: String,
    pub constructor_flag: Option<ConstructorFlag>,
    pub virtual_initialization: Option<VirtualInitialization>,
    pub component_initialization: Option<ComponentInitialization>,
    pub state_sha256: String,
    pub scope: &'static str,
}
pub(super) fn build(
    owned: &Owned,
    world: &World,
    registry: &ge_core::mission::player::item_registry::Registry,
    hook: &Attempt,
) -> Result<Evidence> {
    ensure!(
        matches!(
            owned.base.stage(),
            base_handler::Stage::RuntimeResetComplete | base_handler::Stage::VirtualBasicComplete
        ),
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
    let selected = inventory
        .selection()
        .ok_or_else(|| anyhow::anyhow!("player inventory selection is incomplete"))?;
    let scratch = hook.player_scratch_snapshot()?;
    let source = owned.loadout.provenance();
    ensure!(
        owned.reset.stage() == ge_core::mission::player::constructor::DerivedResetStage::Complete
            && shared.reset_complete,
        "player reset completion or following constructor boundary differs"
    );
    let character = owned
        .reset
        .character()
        .ok_or_else(|| anyhow::anyhow!("player character is not bound"))?;
    ensure!(
        character.key == source.character_key
            && character.source_index == source.character_source_index
            && selected.metadata_sorted_index == source.metadata_sorted_index,
        "player loadout retained binding differs from actual selection"
    );
    let constructor_flag = owners::flag(owned)?;
    let virtual_initialization = owners::virtual_init(owned, world, registry)?;
    let component_initialization = components::build(owned, world)?;
    let virtual_reached = virtual_initialization.is_some();
    let mut result = Evidence {
        origin: "fresh_constructor_snapshot",
        lifetime: "controller_arena",
        stage: if virtual_reached {
            "player_components_pending"
        } else {
            "player_reset_complete"
        },
        pending_dependency: if virtual_reached {
            "skin_texture_dependencies"
        } else {
            "constructor_source_flag"
        },
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
        shared_reset_complete: shared.reset_complete,
        runtime_base_reset_complete: true,
        derived_reset_prefix_complete: true,
        scratch_capacity: scratch.capacity,
        scratch_growth: scratch.growth,
        scratch_records: scratch.count,
        scratch_native_bytes: scratch.native_bytes,
        scratch_host_capacity_bytes: scratch.host_capacity_bytes,
        scratch_reallocations: scratch.reallocations,
        character_resource: source.character_resource as usize,
        character_key: source.character_key as usize,
        character_source_index: source.character_source_index,
        guns_resource: source.guns_resource as usize,
        guns_payload_bytes: source.guns_payload_bytes as usize,
        metadata_source_index: source.metadata_source_index,
        metadata_sorted_index: source.metadata_sorted_index,
        weapon_registry_keys: source.weapon_registry_keys,
        primary_order_keys: source.primary_order_keys,
        secondary_order_keys: source.secondary_order_keys,
        inventory_primary_order_index: selected.primary_order_index,
        inventory_interface_callbacks: selected.interface_callbacks,
        inventory_selected_record: selected.primary_record,
        scratch_prepared: scratch.capacity > 0 && scratch.native_bytes > 0,
        character_bound: owned.reset.character().is_some(),
        inventory_selection_complete: inventory.selection().is_some(),
        handler_kind_bound: owned.base.word(0x224).is_some(),
        inventory_records: inventory.records().len(),
        inventory_capacity: inventory.capacity(),
        inventory_growth: inventory.growth(),
        world_items: world.item_count(),
        world_actor_lists: 1,
        world_membership_lists: world.membership_count(),
        actor_registrations: world.actors()?.len(),
        physics_bound: virtual_initialization
            .as_ref()
            .is_some_and(|state| state.physics_backlink_bound),
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
        scratch_state_sha256: scratch.state_sha256,
        loadout_state_sha256: source.state_sha256.clone(),
        constructor_flag,
        virtual_initialization,
        component_initialization,
        state_sha256: String::new(),
        scope: "The actual reset and PlayerBasic return precede TP entry. The same inventory and sparse PlayerNew fields receive cleanup and slot writes; the TP allocation and direct visual plan precede an actual shared-wrapper skin request. Geometry/material texture dependencies still block skeleton binding, TP attachment and the FP call. Unknown physics operands remain captured and unreadable. Retained owners survive cancellation. PlayerNew completion, selected spawn placement and camera remain pending. Fresh ownership does not establish original history, timing or numeric parity.",
    };
    let mut digest = Sha256::new();
    digest.update(b"ge_player_attempt_v5\0");
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
        &result.scratch_state_sha256,
        &result.loadout_state_sha256,
    ] {
        digest.update(text.as_bytes());
    }
    for nested in [
        serde_json::to_vec(&result.constructor_flag)?,
        serde_json::to_vec(&result.virtual_initialization)?,
        serde_json::to_vec(&result.component_initialization)?,
    ] {
        digest.update((nested.len() as u64).to_be_bytes());
        digest.update(nested);
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
