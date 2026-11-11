//! Actual ordered skin texture requests after dependency enumeration. Every
//! successful request keeps its owner before the next provider/cancellation gate.
use crate::game::{world::post_trigger_resources, world_hooks::Attempt};
use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::mission::player::components::Preparation;

pub(super) fn resolve(
    preparation: &mut Preparation,
    skin: &post_trigger_resources::Target,
    retained: &mut Vec<post_trigger_resources::Target>,
    resources: &mut post_trigger_resources::Owned,
    assets: &mut GameAssets,
    hook: &Attempt,
) -> Result<()> {
    ensure!(
        retained.len() == preparation.texture_requests(),
        "retained texture prefix differs from actual request progress"
    );
    while let Some(request) = preparation.next_texture_request()? {
        hook.ensure_current()?;
        retained
            .try_reserve(1)
            .context("allocating retained texture target owners")?;
        let target = resources.request_player_target(request.file, request.target, assets)?;
        ensure!(
            skin.same_source(&target)
                && target.file() == request.file
                && target.target() == request.target,
            "resolved texture owner differs from the original skin wrapper or request"
        );
        retained.push(target);
        preparation.texture_requested(request)?;
        hook.ensure_current()?;
    }
    preparation.finish_texture_requests()
}
