use super::*;
use crate::animation::layers::{SlotOverride, command};
use anyhow::Context;
impl super::super::Preparation {
    pub fn prepare_third_person_command(
        &mut self,
        command: &mut command::Pending,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage() == super::super::Stage::AwaitingInitialModes,
            "TP command precedes its component helper returns"
        );
        let owner = self.animator_mut_for_capture()?;
        let result = command.prepare(
            source,
            owner
                .layers
                .as_mut()
                .context("TP command layers are absent")?,
            slot_override,
        );
        // A later source failure cannot discard already-reached group weights.
        if let Some(layers) = owner.layers.as_ref() {
            let weights = std::array::from_fn::<_, 4, _>(|i| layers.slots()[i].weight_bits());
            for (i, weight) in weights.into_iter().enumerate() {
                owner.word_store(0x144 + i * 0x20, weight);
            }
        }
        result
    }
}
