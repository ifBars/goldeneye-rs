//! Caller-owned suffix after the genuine inner bind returned. The historical
//! bind/forward/evaluation is retained; this drives a separate evaluation.
use super::super::Owned;
use crate::game::{world::post_trigger_resources::FileBinding, world_hooks::Attempt};
use anyhow::{Context, Result};
use ge_core::mission::player::constructor::preload::post_bind::{PostBind, Stage};

pub(super) fn advance(owned: &mut Owned, skin: &FileBinding, hook: &Attempt) -> Result<()> {
    hook.ensure_current()?;
    let temporary = owned
        .temporary
        .as_mut()
        .context("post-bind temporary owner absent")?;
    if temporary.bind_return() != Some(1) {
        return Ok(());
    }
    if owned.post_bind.is_none() {
        let call = owned
            .state
            .current_call()
            .context("post-bind actual current call absent")?;
        owned.post_bind = Some(PostBind::begin(call, temporary)?);
    }
    let post = owned.post_bind.as_mut().expect("retained caller suffix");
    if post.stage() == Stage::AwaitingBoneOutputs {
        return Ok(()); // no invented outputs, teardown or caller completion
    }
    hook.advance_preload_post_bind(post, temporary, &mut skin.evaluation_skin())
}
