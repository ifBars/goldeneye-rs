//! One persistent process scalar and the caller's separate second evaluation.
use super::super::{Attempt, State};
use anyhow::Result;
use ge_core::{
    animation::script_child::phase::ScalarCapture,
    mission::player::{
        components::temporary::{Pending, SkinSource},
        constructor::preload::post_bind::PostBind,
    },
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ScalarFacts {
    pub owner: u64,
    pub initial_bits: u32,
    pub current_bits: Option<u32>,
    pub revision: u64,
    pub reads: u64,
    pub capture_issued: Option<bool>,
}
impl Attempt {
    pub(in crate::game) fn advance_preload_post_bind(
        &self,
        post: &mut PostBind,
        temporary: &mut Pending,
        skin: &mut impl SkinSource,
    ) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("post-bind process owner lock poisoned"))?;
        self.ensure_current()?;
        state.reap_animation_scope(Some(self.identity))?;
        let State {
            animation_context,
            animation_scratch,
            animation_preload_scalar,
            ..
        } = &mut *state;
        let result = post.advance(
            temporary,
            animation_preload_scalar,
            animation_context,
            animation_scratch,
            skin,
        );
        // Retain even a failed/pending SECOND invocation before unlocking.
        state.retain_animation_scope(self.identity, post.evaluation_lease())?;
        result?;
        self.ensure_current()?;
        Ok(())
    }

    /// Capture facts before entering another State-owned evidence closure.
    /// This does not perform a source scalar read or mint another capture.
    pub(in crate::game) fn preload_scalar_facts(
        &self,
        capture: Option<&ScalarCapture>,
    ) -> Result<ScalarFacts> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("post-bind scalar snapshot lock poisoned"))?;
        self.ensure_current()?;
        let scalar = &state.animation_preload_scalar;
        Ok(ScalarFacts {
            owner: scalar.owner(),
            initial_bits: scalar.initial_bits(),
            current_bits: scalar.current_bits(),
            revision: scalar.revision(),
            reads: scalar.reads(),
            capture_issued: capture.map(|capture| scalar.issued(capture)),
        })
    }
}

#[cfg(test)]
mod tests;
