//! The actual one-shot player constructor prefix on the retained CPU worker.
//! The reset precedes still-required constructor flags and virtual initialization.
mod actions;
pub(super) mod components;
mod evidence;
mod loadout;
mod providers;
mod virtual_init;
use super::{actors::Definitions, resource_states, world_hooks};
use anyhow::{Result, ensure};
pub use evidence::Evidence;
use ge_core::mission::{
    player::{
        base_handler,
        constructor::{Lease, PendingPrelude, PendingReset},
        identity::PlayerId,
        world::World,
    },
    session::{Session, player_load::PlayerRequest},
};
use providers::Provider;

pub(super) struct Bindings<'a> {
    pub session: &'a Session,
    pub mission: &'a crate::mission::MissionContext,
    pub assets: &'a mut ge_assets::resource::GameAssets,
}

pub(super) struct Owned {
    request: PlayerRequest,
    base: base_handler::Owner,
    prelude: PendingPrelude,
    lease: Lease,
    reset: PendingReset,
    loadout: loadout::Sources,
    flag: Option<base_handler::ConstructorFlagReceipt>,
    virtual_init: virtual_init::Owned,
    components: components::Owned,
    action: Option<actions::Owned>,
    repeat_action: Option<actions::repeat::Owned>,
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
        bindings: Bindings<'_>,
        definitions: &mut Definitions,
        resources: &mut resource_states::Attempt,
        hook: &world_hooks::Attempt,
    ) -> Result<Self> {
        hook.ensure_current()?;
        // Player identity is scoped to the actual creation generation, not the
        // user slot reused across cancelled attempts.
        let player = PlayerId::new(request.attempt().get())?;
        let mut base = base_handler::Owner::new(player);
        let mut provider = Provider {
            definitions,
            resources,
            hook,
            session: bindings.session,
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
        let mut prelude = PendingPrelude::after_basic(&basic);
        ensure!(
            prelude.player() == player,
            "player constructor receipt identity differs"
        );
        let lease = hook.begin_player_constructor(player)?;
        base.apply_reset(hook.player_reset_input()?, &mut provider)?;
        base.finish_reset_prefix()?;
        base.finish_runtime_reset(&mut provider)?;
        let mut reset = PendingReset::after_base(&base)?;
        hook.prepare_player_scratch(lease, &mut reset)?;
        let loadout = loadout::Sources::load(bindings.assets, bindings.mission, bindings.session)?;
        hook.ensure_current()?;
        let character = loadout.selected_character();
        reset.bind_character(ge_core::mission::player::constructor::CharacterBinding {
            registry_resource: ge_assets::character::CATALOG_RESOURCE,
            key: character.key(),
            source_index: character.source_index(),
        })?;
        reset.begin_inventory()?;
        let inputs = ge_core::mission::player::inventory::PoseInputs {
            blend_bits: reset
                .word(0xd10)
                .ok_or_else(|| anyhow::anyhow!("player reset blend is unknown"))?,
            c54_raw: reset.byte(0xc54),
            player_flags_raw: reset
                .halfword(0xc56)
                .ok_or_else(|| anyhow::anyhow!("inventory flags are unknown"))?,
            flag_5ee_raw: base
                .byte(0x5ee)
                .ok_or_else(|| anyhow::anyhow!("player multiplier flag is unknown"))?,
            flag_95c_raw: base
                .byte(0x95c)
                .ok_or_else(|| anyhow::anyhow!("player modifier flag is unknown"))?,
            multiplayer_raw: hook.player_multiplayer_byte()?,
        };
        let mut flags = providers::InventoryFlags { base: &base, hook };
        loadout.initialize_inventory(prelude.inventory_mut(), inputs, &mut flags)?;
        reset.record_inventory(prelude.inventory())?;
        let mut tail = providers::Tail {
            hook,
            sources: &loadout,
        };
        reset.finish_tail(&mut prelude, &mut tail)?;
        hook.complete_player_reset(lease, &reset)?;
        let flag = if let Some(raw) = request.source_flag_raw() {
            Some(base.apply_constructor_flag(
                &reset.completion()?,
                raw,
                0,
                &mut providers::FlagNotifications { hook },
            )?)
        } else {
            None
        };
        hook.ensure_current()?;
        Ok(Self {
            request,
            base,
            prelude,
            lease,
            reset,
            loadout,
            flag,
            virtual_init: virtual_init::Owned::new(),
            components: components::Owned::new(),
            action: None,
            repeat_action: None,
            operations,
            catalog_entries,
            rng_reads: provider.rng_reads,
            resource_resets: provider.resource_resets,
        })
    }
    pub fn identity(&self) -> PlayerId {
        self.base.player()
    }
    pub fn matches_attempt(
        &self,
        attempt: ge_core::mission::session::player_load::PlayerAttempt,
    ) -> bool {
        self.request.attempt() == attempt.attempt()
            && self.request.session() == attempt.session()
            && self.request.user() == attempt.user()
            && self.request.source_flag_raw() == attempt.source_flag_raw()
    }
    pub fn dependency(&self) -> ge_core::mission::session::player_load::PlayerDependency {
        use ge_core::mission::session::player_load::PlayerDependency;
        if self.flag.is_none() {
            PlayerDependency::SourceUserFlag
        } else if self.virtual_init.stage() == virtual_init::Stage::BasicComplete {
            PlayerDependency::ComponentInitialization
        } else {
            PlayerDependency::OwnerInitialization
        }
    }
    pub fn initialize_virtual(
        &mut self,
        context: &crate::mission::MissionContext,
        session: &Session,
        world: &mut World,
        registry: &mut ge_core::mission::player::item_registry::Registry,
        hook: &world_hooks::Attempt,
    ) -> Result<()> {
        hook.ensure_current()?;
        if self.flag.is_none() {
            return Ok(());
        }
        self.virtual_init.initialize(
            &mut self.base,
            world,
            registry,
            &mut providers::VirtualEnvironment {
                context,
                session,
                hook,
            },
        )?;
        hook.ensure_current()?;
        Ok(())
    }
    pub fn evidence(
        &self,
        world: &World,
        registry: &ge_core::mission::player::item_registry::Registry,
        components: &ge_core::mission::player::components::Registry,
        hook: &world_hooks::Attempt,
    ) -> Result<Evidence> {
        hook.ensure_current()?;
        evidence::build(self, world, registry, components, hook)
    }
    pub fn initialize_components(
        &mut self,
        sources: components::Sources<'_>,
        world: &mut World,
        hook: &world_hooks::Attempt,
    ) -> Result<()> {
        components::initialize(self, sources, world, hook)?;
        actions::initialize(self, world, hook)
    }
}
