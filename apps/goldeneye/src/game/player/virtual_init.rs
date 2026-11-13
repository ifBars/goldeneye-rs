//! The actual PlayerBasic initializer. Unresolved scalar operands are retained
//! by physics; independent activation and the session suffix still execute.
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
    BasicComplete,
    Failed,
}

pub(super) struct Owned {
    stage: Stage,
    item: Option<ItemId>,
    physics: Option<physics::Owner>,
    origin: Option<zone::Context>,
    membership: Option<u32>,
    completion: Option<base_handler::VirtualBasicReceipt>,
}
pub(super) trait Environment: base_handler::VirtualSessionProvider {
    fn origin_zone(&mut self, position: [u32; 4]) -> Result<zone::Context>;
    fn registration_mask(&mut self, item: ItemId) -> Result<u32>;
}
impl Owned {
    pub fn new() -> Self {
        Self {
            stage: Stage::Fresh,
            item: None,
            physics: None,
            origin: None,
            membership: None,
            completion: None,
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
    pub fn physics_mut(&mut self) -> Option<&mut physics::Owner> {
        self.physics.as_mut()
    }
    pub fn origin(&self) -> Option<zone::Context> {
        self.origin
    }
    pub fn membership(&self) -> Option<u32> {
        self.membership
    }
    pub fn completion(&self) -> Option<&base_handler::VirtualBasicReceipt> {
        self.completion.as_ref()
    }

    pub fn initialize(
        &mut self,
        base: &mut base_handler::Owner,
        world: &mut World,
        registry: &mut Registry,
        environment: &mut impl Environment,
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
        let result = self.run(base, world, registry, environment);
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
        environment: &mut impl Environment,
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
        let zone = environment.origin_zone(world.item(item)?.position_bits())?;
        world.item_mut(item)?.set_zone(zone.callback_zone);
        self.origin = Some(zone);
        let desired = environment.registration_mask(item)?;
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
        physics.bind_item_identity(item)?;
        physics.apply_player_initialization(base.word(0x224), || {
            let mask = world.item(item)?.membership_mask();
            world.update_membership(item, mask | 0x10)?;
            Ok(())
        })?;
        self.completion =
            Some(base.finish_virtual_basic(world.item_mut(item)?, physics, environment)?);
        self.stage = Stage::BasicComplete;
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
