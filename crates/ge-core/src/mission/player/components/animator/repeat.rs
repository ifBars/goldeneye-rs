//! Position and seek helpers apply the animation subtype gate to the actual head.
use super::*;
use crate::animation::layers::NodeId;
use anyhow::Context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationPosition {
    pub head: Option<NodeId>,
    pub matched: bool,
    pub bits: u32,
}

impl super::super::Preparation {
    pub fn mode_animation_position(
        &self,
        receiver: Identity,
        slot: u8,
    ) -> Result<AnimationPosition> {
        self.mode_animator(receiver)?.animation_position(slot)
    }

    /// Writes the matching child's phase fields before its separate initflag1 call.
    pub fn begin_mode_seek(
        &mut self,
        receiver: Identity,
        slot: u8,
        bits: u32,
    ) -> Result<Option<NodeId>> {
        self.mode_animator_mut(receiver)?.begin_seek(slot, bits)
    }
}

impl Animator {
    fn animation_position(&self, slot: u8) -> Result<AnimationPosition> {
        let head = self
            .layers
            .as_ref()
            .context("animation position layers absent")?
            .slots()
            .get(usize::from(slot))
            .context("animation position physical slot exceeds four")?
            .nodes_head()
            .first()
            .copied();
        let script = self.dispatch_script(slot)?;
        let bits = if let Some(id) = script {
            self.script_children
                .get(id)
                .context("animation position script absent")?
                .word(0x28)
                .context("animation position unknown")?
        } else {
            // A control's virtual+10 is outside this type gate and is not read.
            0
        };
        Ok(AnimationPosition {
            head,
            matched: script.is_some(),
            bits,
        })
    }

    fn begin_seek(&mut self, slot: u8, bits: u32) -> Result<Option<NodeId>> {
        let Some(id) = self.dispatch_script(slot)? else {
            return Ok(None);
        };
        self.script_children.copy_parent_phase(id, bits)?;
        Ok(Some(id))
    }
}

#[cfg(test)]
#[path = "repeat/tests.rs"]
mod tests;
