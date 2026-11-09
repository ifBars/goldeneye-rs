//! Live TP entry uses the existing inventory, definition registry and resource
//! wrappers. No diagnostic layer initializer or elapsed prewarm runs here.
use super::{
    super::{actors::Definitions, world::post_trigger_resources, world_hooks},
    Owned as Player,
};
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    player::{
        components::{self, Preparation, TextureRequest},
        world::World,
    },
    session::Session,
};

pub(crate) struct Sources<'a> {
    pub session: &'a Session,
    pub definitions: &'a Definitions,
    pub resources: &'a mut post_trigger_resources::Owned,
    pub assets: &'a mut ge_assets::resource::GameAssets,
}
pub(super) struct Owned {
    pub preparation: Preparation,
    pub target: Option<post_trigger_resources::Target>,
}
impl Owned {
    pub fn new() -> Self {
        Self {
            preparation: Preparation::new(),
            target: None,
        }
    }
}
pub(super) fn initialize(
    player: &mut Player,
    sources: Sources<'_>,
    world: &World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    if player.flag.is_none() {
        return Ok(());
    }
    let receipt = player
        .virtual_init
        .completion()
        .context("components require the actual basic initializer return")?;
    let item = world.item(receipt.item())?;
    player.components.preparation.prepare(
        components::Bindings {
            base: &player.base,
            basic: receipt,
            reset: &mut player.reset,
            prelude: &mut player.prelude,
            item,
        },
        &mut Provider {
            session: sources.session,
            hook,
        },
    )?;
    hook.ensure_current()?;
    let visual = sources.definitions.visual(
        player
            .reset
            .character()
            .context("component character owner is absent")?,
    )?;
    player.components.preparation.bind_visual(visual)?;
    // The real helper requests the skin before enumerating its texture refs.
    // Store the returned owner before any subsequent generation/provider check.
    player.components.target = Some(sources.resources.request_player_target(
        visual.file(),
        visual.skin(),
        sources.assets,
    )?);
    hook.ensure_current()?;
    let target = player
        .components
        .target
        .as_ref()
        .context("component target owner disappeared")?;
    ensure!(
        visual.texture_request() == TextureRequest::EnumerateSkin,
        "explicit component texture requests require their actual source provider"
    );
    player
        .components
        .preparation
        .skin_requested(target.file(), target.target())?;
    // Actual geometry/material dependency traversal is still required before
    // default skeleton loading, component bind/attachment and the FP call.
    Ok(())
}
struct Provider<'a> {
    session: &'a Session,
    hook: &'a world_hooks::Attempt,
}
impl components::Provider for Provider<'_> {
    fn session_manager_present(&mut self) -> Result<bool> {
        self.hook.ensure_current()?;
        ensure!(
            self.session.active() == Some(self.session.identity()),
            "component cleanup lost active session ownership"
        );
        Ok(self.session.managers().is_some())
    }
    fn mark_interface_dirty(&mut self, _: u32) -> Result<()> {
        self.hook.ensure_current()?;
        anyhow::bail!("bound component interface requires its actual dirty-channel provider")
    }
}
