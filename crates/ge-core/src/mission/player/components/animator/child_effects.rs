use super::*;
use crate::animation::layers::{SlotOverride, command};
use anyhow::Context;
impl super::super::Preparation {
    pub fn finish_third_person_child_effects(
        &mut self,
        command: &mut command::Pending,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        let receiver = self
            .animator()
            .context("TP child effect owner absent")?
            .identity();
        self.finish_mode_child_effects(receiver, command, source, slot_override)
    }
    pub fn finish_mode_child_effects(
        &mut self,
        receiver: Identity,
        command: &mut command::Pending,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        let owner = self.mode_animator_mut(receiver)?;
        let result = (|| {
            let layers = owner
                .layers
                .as_mut()
                .context("mode child effect layers absent")?;
            command.finish_script_child(
                layers,
                &mut owner.node_ids,
                &owner.controls,
                &mut owner.script_children,
                source,
            )?;
            if command.stage() == command::Stage::ChildContinuationPending {
                command.advance_completed_child(source, layers, slot_override)?;
            }
            Ok(())
        })();
        owner.mirror_mode_layers();
        if result.is_err() {
            command.fail_factory(
                owner
                    .layers
                    .as_mut()
                    .context("mode child effect layers absent")?,
            );
        }
        result
    }
}
