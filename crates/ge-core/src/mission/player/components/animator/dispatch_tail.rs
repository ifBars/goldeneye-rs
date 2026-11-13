use super::*;
use crate::animation::{
    layers::NodeId,
    nodes::Kind,
    script_child::{self, update},
};
use anyhow::Context;
impl super::super::Preparation {
    pub fn write_third_person_dispatch_scalar(
        &mut self,
        player: PlayerId,
        slot: u8,
        bits: u32,
    ) -> Result<bool> {
        let owner = self.dispatch_owner(player)?;
        let Some(id) = owner.dispatch_script(slot)? else {
            return Ok(false);
        };
        owner.script_children.set_dispatch_coordinate(id, bits)?;
        Ok(true)
    }
    pub fn begin_third_person_dispatch_update(
        &mut self,
        player: PlayerId,
        slot: u8,
        bits: u32,
    ) -> Result<Option<NodeId>> {
        let owner = self.dispatch_owner(player)?;
        let Some(id) = owner.dispatch_script(slot)? else {
            return Ok(None);
        };
        owner.script_children.copy_parent_phase(id, bits)?;
        Ok(Some(id))
    }
    pub fn ensure_third_person_dispatch_update_returned(
        &self,
        player: PlayerId,
        id: NodeId,
    ) -> Result<()> {
        let owner = self.animator().context("TP dispatch owner absent")?;
        ensure!(
            owner.player() == player
                && owner.script_children.get(id).is_some_and(|h| h
                    .updates()
                    .last()
                    .is_some_and(|t| t.stage == update::Stage::Returned)),
            "TP dispatch child update has not returned"
        );
        Ok(())
    }
    pub fn begin_third_person_script_update(
        &mut self,
        player: PlayerId,
        id: NodeId,
        source: &mut impl script_child::Provider,
    ) -> Result<()> {
        self.dispatch_owner(player)?
            .script_children
            .begin_initflag_update(id, source)
    }
    pub fn rate_third_person_script_update(
        &mut self,
        player: PlayerId,
        id: NodeId,
        source: &mut impl script_child::position::Source,
        inputs: script_child::rate::Inputs,
    ) -> Result<()> {
        self.dispatch_owner(player)?
            .script_children
            .prepare_update_rate(id, source, inputs)
    }
    pub fn select_third_person_script_update(
        &mut self,
        player: PlayerId,
        id: NodeId,
        source: &mut impl script_child::initialization::Source,
        step: u32,
    ) -> Result<()> {
        let owner = self.dispatch_owner(player)?;
        let inputs = script_child::initialization::Inputs {
            component_speed_bits: owner
                .word(0x108)
                .context("TP update component speed unknown")?,
            scaled_step_bits: step,
            maximum_entries: 65536,
        };
        owner.script_children.select_update(id, source, inputs)
    }
    pub fn position_third_person_script_update(
        &mut self,
        player: PlayerId,
        id: NodeId,
        source: &mut impl script_child::position::Source,
    ) -> Result<()> {
        self.dispatch_owner(player)?
            .script_children
            .prepare_update_position(id, source)
    }
    fn dispatch_owner(&mut self, player: PlayerId) -> Result<&mut Animator> {
        ensure!(
            self.stage() == super::super::Stage::AwaitingInitialModes,
            "TP dispatch precedes component returns"
        );
        let owner = self.animator_mut_for_capture()?;
        ensure!(owner.player() == player, "TP dispatch player differs");
        Ok(owner)
    }
}
impl Animator {
    pub(super) fn dispatch_script(&self, slot: u8) -> Result<Option<NodeId>> {
        let layers = self.layers.as_ref().context("TP dispatch layers absent")?;
        let head = layers
            .slots()
            .get(usize::from(slot))
            .context("TP dispatch physical slot exceeds four")?
            .nodes_head()
            .first()
            .copied();
        let Some(id) = head else {
            return Ok(None);
        };
        match self
            .node_ids
            .kind(id)
            .context("TP dispatch head type provider absent")?
        {
            Kind::Control => {
                ensure!(
                    self.controls.get(id).is_some(),
                    "TP dispatch control absent"
                );
                Ok(None)
            }
            Kind::Script => {
                ensure!(
                    self.script_children.get(id).is_some_and(|h| h.owner()
                        == self.node_ids.owner()
                        && h.slot() == Some(slot)),
                    "TP dispatch script membership differs"
                );
                Ok(Some(id))
            }
        }
    }
}
