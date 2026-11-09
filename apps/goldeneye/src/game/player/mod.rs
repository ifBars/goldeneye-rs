//! The actual one-shot player constructor prefix on the retained CPU worker.
//! Full PlayerNew reset, shared allocation and completion remain later work.
mod evidence;
use super::{actors::Definitions, resource_states, world_hooks};
use anyhow::{Result, ensure};
pub use evidence::Evidence;
use ge_core::mission::{
    player::{base_handler, constructor::PendingPrelude, identity::PlayerId, world::World},
    session::player_load::PlayerRequest,
};

pub(super) struct Owned {
    request: PlayerRequest,
    base: base_handler::Owner,
    prelude: PendingPrelude,
    operations: usize,
    catalog_entries: usize,
    rng_reads: usize,
    resource_resets: usize,
}
impl Owned {
    pub fn build(
        request: PlayerRequest,
        operations: usize,
        catalog_entries: usize,
        definitions: &mut Definitions,
        resources: &mut resource_states::Attempt,
        hook: &world_hooks::Attempt,
    ) -> Result<Self> {
        hook.ensure_current()?;
        let player = PlayerId::new(request.user().get())?;
        let mut base = base_handler::Owner::new(player);
        let mut provider = Provider {
            definitions,
            resources,
            hook,
            rng_reads: 0,
            resource_resets: 0,
        };
        base.initialize_generic(&mut provider)?;
        hook.ensure_current()?;
        base.initialize_basic()?;
        base.initialize_player_basic_prefix()?;
        base.apply_reset(hook.player_reset_input()?, &mut provider)?;
        hook.ensure_current()?;
        let basic = base.finish_basic()?;
        let prelude = PendingPrelude::after_basic(&basic);
        ensure!(
            prelude.player() == player,
            "player constructor receipt identity differs"
        );
        Ok(Self {
            request,
            base,
            prelude,
            operations,
            catalog_entries,
            rng_reads: provider.rng_reads,
            resource_resets: provider.resource_resets,
        })
    }
    pub fn evidence(&self, world: &World, hook: &world_hooks::Attempt) -> Result<Evidence> {
        hook.ensure_current()?;
        evidence::build(self, world, hook)
    }
}
struct Provider<'a> {
    definitions: &'a mut Definitions,
    resources: &'a mut resource_states::Attempt,
    hook: &'a world_hooks::Attempt,
    rng_reads: usize,
    resource_resets: usize,
}
impl base_handler::Provider for Provider<'_> {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        self.hook.ensure_current()?;
        let seed = self.definitions.draw_handler_seed()?;
        self.rng_reads += 1;
        Ok(seed)
    }
    fn reset_base_global_byte(&mut self) -> Result<()> {
        self.hook.reset_player_base_byte()
    }
    fn reset_resource_state(&mut self, token: u32, value: u32) -> Result<()> {
        self.resources
            .set(token, value, || self.hook.ensure_current())?;
        self.resource_resets += 1;
        Ok(())
    }
}
