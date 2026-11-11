use super::{Owned as Components, post_trigger_resources, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_assets::{resource::GameAssets, skin::Body};
use ge_core::mission::player::{
    base_handler,
    components::{ResourceBinding, TextureEnumeration},
    item::Item,
};

pub(in crate::game::player) struct Owned {
    pub source: Option<post_trigger_resources::FileBinding>,
    pub body: Option<Body>,
    pub target: Option<post_trigger_resources::Target>,
    pub textures: Vec<post_trigger_resources::Target>,
    pub enumeration: Option<TextureEnumeration>,
}
impl Owned {
    pub fn new() -> Self {
        Self {
            source: None,
            body: None,
            target: None,
            textures: Vec::new(),
            enumeration: None,
        }
    }
}
pub(super) fn initialize(
    owner: &mut Components,
    base: &mut base_handler::Owner,
    item: &mut Item,
    resources: &mut post_trigger_resources::Owned,
    assets: &mut GameAssets,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    owner.preparation.begin_visual_postbind(base, item)?;
    let visual = owner
        .preparation
        .visual()
        .context("visual postbind row is absent")?;
    owner.postbind.source = Some(resources.lookup_player_file(visual.file(), assets)?);
    hook.ensure_current()?;
    let source = owner
        .postbind
        .source
        .as_ref()
        .context("postbind visual wrapper is absent")?;
    ensure!(
        source.same_target(
            owner
                .target
                .as_ref()
                .context("initial visual target is absent")?
        ),
        "postbind lookup differs from the retained visual wrapper"
    );
    owner.postbind.body = Some(source.skin_body(visual.callback_skin())?);
    let body = owner
        .postbind
        .body
        .as_ref()
        .context("postbind visual body is absent")?;
    owner.preparation.visual_skin_bound(ResourceBinding {
        file: body.file(),
        reference: body.reference(),
    })?;
    hook.ensure_current()?;
    // Source repeats this dependency helper after the secondary-skin callback.
    // Every call still reaches the existing wrapper, even when already resident.
    owner.postbind.target =
        Some(resources.request_player_target(visual.file(), visual.skin(), assets)?);
    hook.ensure_current()?;
    let target = owner
        .postbind
        .target
        .as_ref()
        .context("postbind skin request is absent")?;
    ensure!(
        source.same_target(target),
        "postbind skin request changed wrapper"
    );
    owner.postbind.enumeration = Some(target.skin_texture_dependencies()?);
    let textures = owner
        .postbind
        .enumeration
        .as_ref()
        .context("postbind texture traversal is absent")?;
    for &hash in &textures.hashes {
        hook.ensure_current()?;
        let texture = resources.request_player_target(target.file(), hash, assets)?;
        ensure!(
            target.same_source(&texture) && texture.target() == hash,
            "postbind texture source differs"
        );
        owner.postbind.textures.push(texture);
        hook.ensure_current()?;
    }
    owner.preparation.finish_visual_postbind(
        base,
        item,
        ResourceBinding {
            file: target.file(),
            reference: target.target(),
        },
        textures,
        owner.postbind.textures.len(),
    )?;
    hook.ensure_current()?;
    Ok(())
}
