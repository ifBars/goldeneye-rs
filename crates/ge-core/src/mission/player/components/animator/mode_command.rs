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
        let receiver = self
            .animator()
            .context("TP command owner absent")?
            .identity();
        self.prepare_mode_command(receiver, command, source, slot_override)
    }
    pub fn prepare_mode_command(
        &mut self,
        receiver: Identity,
        command: &mut command::Pending,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        let owner = self.mode_animator_mut(receiver)?;
        let result = command.prepare(
            source,
            owner
                .layers
                .as_mut()
                .context("mode command layers are absent")?,
            slot_override,
        );
        // A later source failure cannot discard already-reached group weights.
        owner.mirror_mode_layers();
        result
    }
}
