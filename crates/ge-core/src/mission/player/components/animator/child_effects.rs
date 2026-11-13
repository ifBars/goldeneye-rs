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
        ensure!(
            self.stage() == super::super::Stage::AwaitingInitialModes,
            "child effects precede component helper return"
        );
        let owner = self.animator_mut_for_capture()?;
        let result = (|| {
            let layers = owner
                .layers
                .as_mut()
                .context("TP child effect layers absent")?;
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
        if let Some(layers) = owner.layers.as_ref() {
            let count = layers.active_count();
            let weights = std::array::from_fn::<_, 4, _>(|i| layers.slots()[i].weight_bits());
            owner.bytes[0x130] = Some(count);
            for (i, weight) in weights.into_iter().enumerate() {
                owner.word_store(0x144 + i * 0x20, weight);
            }
        }
        if result.is_err() {
            command.fail_factory(
                owner
                    .layers
                    .as_mut()
                    .context("TP child effect layers absent")?,
            );
        }
        result
    }
}
