//! The reached PlayerBasic virtual initializer. Missing handler-kind input
//! retains real factory/actor/physics ownership before the activation boundary.
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    player::{
        base_handler,
        identity::{ComponentId, ItemId, PhysicsId},
        item_registry::{self, Registry},
        physics,
        world::{self, World},
    },
    triggers::zone,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Fresh,
    Running,
    HandlerKindPending,
    PhysicsInitialized,
    Failed,
}

pub(super) struct Owned {
    stage: Stage,
    item: Option<ItemId>,
    physics: Option<physics::Owner>,
    origin: Option<zone::Context>,
    membership: Option<u32>,
}
impl Owned {
    pub fn new() -> Self {
        Self {
            stage: Stage::Fresh,
            item: None,
            physics: None,
            origin: None,
            membership: None,
        }
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn item(&self) -> Option<ItemId> {
        self.item
    }
    pub fn physics(&self) -> Option<&physics::Owner> {
        self.physics.as_ref()
    }
    pub fn origin(&self) -> Option<zone::Context> {
        self.origin
    }
    pub fn membership(&self) -> Option<u32> {
        self.membership
    }

    pub fn initialize(
        &mut self,
        base: &mut base_handler::Owner,
        world: &mut World,
        registry: &mut Registry,
        origin: impl FnOnce([u32; 4]) -> Result<zone::Context>,
        registration_mask: impl FnOnce(ItemId) -> Result<u32>,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::Fresh,
            "virtual initialization was already dispatched"
        );
        ensure!(
            base.stage() == base_handler::Stage::ConstructorFlagComplete,
            "virtual initialization requires actual constructor-flag completion"
        );
        self.stage = Stage::Running;
        let result = self.run(base, world, registry, origin, registration_mask);
        if result.is_err() {
            self.stage = Stage::Failed;
        }
        result
    }
    fn run(
        &mut self,
        base: &mut base_handler::Owner,
        world: &mut World,
        registry: &mut Registry,
        origin: impl FnOnce([u32; 4]) -> Result<zone::Context>,
        registration_mask: impl FnOnce(ItemId) -> Result<u32>,
    ) -> Result<()> {
        let item = world.construct_item()?;
        self.item = Some(item);
        world.item_mut(item)?.begin_wrapperless_factory()?;
        let receipt = registry.register(
            item,
            item_registry::Budget {
                records: 65_536,
                native_bytes: 65_536 * 8,
                growth_steps: 4096,
            },
        )?;
        world.item_mut(item)?.finish_wrapperless_factory(&receipt)?;
        world.item_mut(item)?.prepare_player_owner();
        world.item_mut(item)?.bind_handler(Some(base.player()));
        base.bind_item(world.item(item)?)?;
        // This is the actual constructor-origin query. The selected StartPoint
        // is not written into the unfinished owner before this call.
        let zone = origin(world.item(item)?.position_bits())?;
        world.item_mut(item)?.set_zone(zone.callback_zone);
        self.origin = Some(zone);
        let desired = registration_mask(item)?;
        self.membership = Some(desired);
        world.register_if_absent(item, desired, &mut Components)?;
        base.initialize_virtual_motion(world.item(item)?)?;
        ensure!(
            world.item(item)?.physics().is_none(),
            "existing physics requires its actual cleanup/rebind provider"
        );
        self.physics = Some(physics::Owner::from_constructor());
        world
            .item_mut(item)?
            .bind_physics(Some(PhysicsId::new(base.player().get())?));
        let physics = self
            .physics
            .as_mut()
            .context("physics factory did not return an owner")?;
        physics.bind_item(item.get())?;
        let result = physics.apply_player_initialization(base.word(0x224), || {
            let mask = world.item(item)?.membership_mask();
            world.update_membership(item, mask | 0x10)?;
            Ok(())
        });
        match result {
            Ok(()) => self.stage = Stage::PhysicsInitialized,
            Err(error)
                if error
                    .downcast_ref::<physics::MissingHandlerKind>()
                    .is_some() =>
            {
                self.stage = Stage::HandlerKindPending;
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }
}
struct Components;
impl world::Components for Components {
    fn activate(&mut self, _: ItemId, _: ComponentId, _: bool) -> Result<()> {
        anyhow::bail!("reached component activation requires an actual owned component provider")
    }
}

#[cfg(test)]
#[path = "tests/virtual_init.rs"]
mod tests;
