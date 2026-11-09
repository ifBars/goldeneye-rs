use super::super::{Owned, virtual_init};
use anyhow::{Result, ensure};
use ge_core::mission::player::{item_registry::Registry, world::World};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstructorFlag {
    pub origin: &'static str,
    pub raw: u8,
    pub changed: bool,
    pub interface_callbacks: usize,
    pub original_history_verified: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VirtualInitialization {
    pub stage: &'static str,
    pub pending_dependency: &'static str,
    pub item_registry_records: usize,
    pub item_registry_capacity: usize,
    pub item_registry_growth: usize,
    pub item_registry_counter: usize,
    pub item_registry_native_bytes: usize,
    pub item_registry_host_capacity_bytes: usize,
    pub origin_zone_queries: usize,
    pub origin_zone_nodes: usize,
    pub desired_membership: u32,
    pub item_membership: u32,
    pub item_flags: u16,
    pub item_priority: u8,
    pub factory_complete: bool,
    pub item_registry_bound: bool,
    pub actor_registered: bool,
    pub handler_bound: bool,
    pub origin_zone_queried: bool,
    pub physics_constructed: bool,
    pub physics_backlink_bound: bool,
    pub physics_activated: bool,
    pub initializer_complete: bool,
    pub scalar_pending: bool,
    pub scalar_value_known: bool,
    pub item_state_sha256: String,
    pub physics_state_sha256: String,
    pub registry_state_sha256: String,
    pub scope: &'static str,
}
pub(super) fn flag(owned: &Owned) -> Result<Option<ConstructorFlag>> {
    let Some(flag) = &owned.flag else {
        return Ok(None);
    };
    ensure!(
        flag.player() == owned.base.player() && Some(flag.raw()) == owned.request.source_flag_raw(),
        "constructor flag differs from the captured same-user argument"
    );
    Ok(Some(ConstructorFlag {
        origin: "captured_member_snapshot",
        raw: flag.raw(),
        changed: flag.changed(),
        interface_callbacks: flag.interface_callbacks(),
        original_history_verified: false,
    }))
}
pub(super) fn virtual_init(
    owned: &Owned,
    world: &World,
    registry: &Registry,
) -> Result<Option<VirtualInitialization>> {
    if owned.flag.is_none() {
        return Ok(None);
    }
    ensure!(
        owned.virtual_init.stage() == virtual_init::Stage::BasicComplete,
        "virtual initializer has not completed its actual basic suffix"
    );
    let id = owned
        .virtual_init
        .item()
        .ok_or_else(|| anyhow::anyhow!("virtual initializer has no item"))?;
    let item = world.item(id)?;
    let factory = item
        .factory()
        .ok_or_else(|| anyhow::anyhow!("item factory has not completed"))?;
    let physics = owned
        .virtual_init
        .physics()
        .ok_or_else(|| anyhow::anyhow!("physics owner is absent"))?;
    let origin = owned
        .virtual_init
        .origin()
        .ok_or_else(|| anyhow::anyhow!("constructor-origin zone query is absent"))?;
    let membership = owned
        .virtual_init
        .membership()
        .ok_or_else(|| anyhow::anyhow!("registration mask is absent"))?;
    let registry_bound = registry
        .records()
        .iter()
        .any(|row| row.item() == id && row.numeric_id() == factory.numeric_id());
    ensure!(
        registry_bound
            && item.handler() == Some(owned.base.player())
            && owned.base.item() == Some(id),
        "item registry or handler backlink differs"
    );
    ensure!(
        item.physics()
            .is_some_and(|id| id.get() == owned.base.player().get())
            && physics.item() == Some(id.get())
            && physics.item_identity() == Some(id),
        "physics backlink differs from its actual retained owner"
    );
    let completion = owned
        .virtual_init
        .completion()
        .ok_or_else(|| anyhow::anyhow!("basic initializer has no return receipt"))?;
    ensure!(
        completion.player() == owned.base.player() && completion.item() == id,
        "basic initializer return receipt differs from actual owners"
    );
    Ok(Some(VirtualInitialization {
        stage: "player_basic_complete",
        pending_dependency: "tp_fp_components",
        item_registry_records: registry.count(),
        item_registry_capacity: registry.capacity() as usize,
        item_registry_growth: registry.growth() as usize,
        item_registry_counter: registry.counter() as usize,
        item_registry_native_bytes: registry.native_storage_bytes(),
        item_registry_host_capacity_bytes: registry.host_capacity_bytes(),
        origin_zone_queries: 1,
        origin_zone_nodes: origin.nodes_visited,
        desired_membership: membership,
        item_membership: item.membership_mask(),
        item_flags: item.flags(),
        item_priority: item.priority(),
        factory_complete: item.factory().is_some(),
        item_registry_bound: registry_bound,
        actor_registered: world.contains_actor(id)?,
        handler_bound: item.handler() == Some(owned.base.player()),
        origin_zone_queried: owned.virtual_init.origin().is_some(),
        physics_constructed: owned.virtual_init.physics().is_some(),
        physics_backlink_bound: physics.item() == Some(id.get()),
        physics_activated: physics.byte(0x1ff) == Some(1),
        initializer_complete: true,
        scalar_pending: physics.scalar_pending(),
        scalar_value_known: physics.handler_scalar_bits().is_ok(),
        item_state_sha256: super::words(b"ge_player_item_v1\0", |emit| {
            item.visit_state_words(emit)
        }),
        physics_state_sha256: super::words(b"ge_player_physics_v1\0", |emit| {
            physics.visit_state_words(emit)
        }),
        registry_state_sha256: super::words(b"ge_player_item_registry_v1\0", |emit| {
            registry.visit_state_words(emit)
        }),
        scope: "The controller retains actual player, EXItem, numeric-ID registry, actor memberships and sparse physics ownership across cancellation. Constructor-origin placement, physics activation and the PlayerBasic suffix have executed. The handler scalar owns unresolved captured operands and its getter fails explicitly. The suffix counts actual kind2 session records and returns a typed basic receipt; this does not complete PlayerNew, TP/FP components, player publication, simulation or camera. Original history and numeric parity are unverified.",
    }))
}
