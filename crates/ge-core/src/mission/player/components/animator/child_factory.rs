use super::*;
use crate::animation::{layers::command, script_child};
use anyhow::Context;
impl super::super::Preparation {
    pub fn prepare_third_person_child_position(
        &mut self,
        command: &mut command::Pending,
        source: &mut impl script_child::position::Source,
    ) -> Result<()> {
        let receiver = self
            .animator()
            .context("TP position owner absent")?
            .identity();
        self.prepare_mode_child_position(receiver, command, source)
    }
    pub fn prepare_mode_child_position(
        &mut self,
        receiver: Identity,
        command: &mut command::Pending,
        source: &mut impl script_child::position::Source,
    ) -> Result<()> {
        let owner = self.mode_animator_mut(receiver)?;
        let result = (|| {
            ensure!(
                command.stage() == command::Stage::ChildFactoryPending,
                "position continuation command differs"
            );
            let id = command
                .factory_node()
                .context("position child factory allocation absent")?;
            owner
                .script_children
                .prepare_phase_zero_position(id, source)
        })();
        if result.is_err() {
            command.fail_factory(owner.layers.as_mut().context("mode layers absent")?);
        }
        result
    }
    pub fn initialize_third_person_child_phase_zero(
        &mut self,
        command: &mut command::Pending,
        source: &mut impl script_child::initialization::Source,
        scaled_step_bits: u32,
        maximum_entries: usize,
    ) -> Result<()> {
        let receiver = self
            .animator()
            .context("TP phase-zero owner absent")?
            .identity();
        self.initialize_mode_child_phase_zero(
            receiver,
            command,
            source,
            scaled_step_bits,
            maximum_entries,
        )
    }
    pub fn initialize_mode_child_phase_zero(
        &mut self,
        receiver: Identity,
        command: &mut command::Pending,
        source: &mut impl script_child::initialization::Source,
        scaled_step_bits: u32,
        maximum_entries: usize,
    ) -> Result<()> {
        let owner = self.mode_animator_mut(receiver)?;
        let result = (|| {
            ensure!(
                command.stage() == command::Stage::ChildFactoryPending,
                "phase-zero initializer command differs"
            );
            let id = command
                .factory_node()
                .context("phase-zero child factory allocation absent")?;
            let inputs = script_child::initialization::Inputs {
                component_speed_bits: owner.word(0x108).context("animator speed unknown")?,
                scaled_step_bits,
                maximum_entries,
            };
            owner
                .script_children
                .initialize_phase_zero(id, source, inputs)
        })();
        if result.is_err() {
            command.fail_factory(owner.layers.as_mut().context("mode layers absent")?);
        }
        result
    }
    pub fn initialize_third_person_child_prefix(
        &mut self,
        command: &mut command::Pending,
        source: &mut impl script_child::Provider,
    ) -> Result<()> {
        let receiver = self
            .animator()
            .context("TP child factory owner absent")?
            .identity();
        self.initialize_mode_child_prefix(receiver, command, source)
    }
    pub fn initialize_mode_child_prefix(
        &mut self,
        receiver: Identity,
        command: &mut command::Pending,
        source: &mut impl script_child::Provider,
    ) -> Result<()> {
        let owner = self.mode_animator_mut(receiver)?;
        let result = (|| {
            let request = command.begin_factory(
                owner
                    .layers
                    .as_mut()
                    .context("mode child factory layers absent")?,
            )?;
            ensure!(
                ((request.reference >> 24) & 0x7f) == 4,
                "unsupported reached child factory class"
            );
            let binding = owner
                .component
                .context("mode child source wrapper absent")?;
            let id = owner.script_children.allocate(
                &mut owner.node_ids,
                binding.file,
                request.reference,
            )?;
            command.retain_factory_node(owner.layers.as_mut().expect("retained layers"), id)?;
            owner.script_children.bind_factory_context(
                id,
                request.slot,
                request.effective_flags,
            )?;
            owner.component = Some(ResourceBinding {
                file: binding.file,
                reference: request.reference,
            });
            owner.word_store(0x20, request.reference);
            owner.word_store(0x24, u32::MAX);
            owner.script_children.initialize_prefix(id, source)
        })();
        if result.is_err()
            && let Some(layers) = owner.layers.as_mut()
        {
            command.fail_factory(layers);
        }
        result
    }
}
impl Animator {
    pub fn script_children(&self) -> &script_child::Store {
        &self.script_children
    }
    pub fn node_ids(&self) -> &crate::animation::nodes::Ids {
        &self.node_ids
    }
}
