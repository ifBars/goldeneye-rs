//! Direct mode parameters on the exact retained receiver and physical slot.
//! Typed layer state mirrors raw headers without creating controls or poses.
use super::*;
use crate::animation::{layers::NodeId, nodes::Kind};
use anyhow::Context;

impl super::super::Preparation {
    pub fn set_mode_speed(&mut self, receiver: Identity, bits: u32) -> Result<()> {
        self.mode_animator_mut(receiver)?.word_store(0x108, bits);
        Ok(())
    }

    pub fn set_mode_weight(&mut self, receiver: Identity, slot: u8, bits: u32) -> Result<()> {
        self.mode_animator_mut(receiver)?
            .set_parameter_weight(slot, bits)
    }

    pub fn set_mode_child_speed(
        &mut self,
        receiver: Identity,
        slot: u8,
        bits: u32,
    ) -> Result<bool> {
        self.mode_animator_mut(receiver)?
            .set_parameter_child_speed(slot, bits)
    }
}

impl Animator {
    fn set_parameter_weight(&mut self, slot: u8, bits: u32) -> Result<()> {
        ensure!(slot < 4, "parameter weight physical slot exceeds four");
        ensure!(self.layers.is_some(), "parameter weight layer owner absent");
        self.word_store(0x144 + usize::from(slot) * 0x20, bits);
        self.layers
            .as_mut()
            .expect("checked layer owner")
            .set_slot_weight(usize::from(slot), bits)
    }

    fn parameter_head(&self, slot: u8) -> Result<Option<NodeId>> {
        let head = self
            .layers
            .as_ref()
            .context("parameter head layers absent")?
            .slots()
            .get(usize::from(slot))
            .context("parameter head physical slot exceeds four")?
            .nodes_head()
            .first()
            .copied();
        if let Some(id) = head {
            let links = self
                .node_ids
                .links(id)
                .context("parameter head typed links absent")?;
            ensure!(
                links.slot == slot && links.previous.is_none(),
                "parameter head slot or link differs"
            );
        }
        Ok(head)
    }

    fn set_parameter_child_speed(&mut self, slot: u8, bits: u32) -> Result<bool> {
        let Some(id) = self.parameter_head(slot)? else {
            return Ok(false);
        };
        match self
            .node_ids
            .kind(id)
            .context("parameter head kind provider absent")?
        {
            Kind::Control => {
                ensure!(
                    self.controls
                        .get(id)
                        .is_some_and(|c| c.component() == self.node_ids.owner()),
                    "parameter control owner differs"
                );
                // Unlike coordinate/phase helpers, virtual+20 has no subtype
                // gate here. Controls use the common direct speed setter.
                self.controls.set_speed(id, bits)?;
            }
            Kind::Script => {
                ensure!(
                    self.script_children.get(id).is_some_and(|h| h.owner()
                        == self.node_ids.owner()
                        && h.slot() == Some(slot)),
                    "parameter script owner or slot differs"
                );
                self.script_children.set_speed(id, bits)?;
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "parameters/tests.rs"]
mod tests;
