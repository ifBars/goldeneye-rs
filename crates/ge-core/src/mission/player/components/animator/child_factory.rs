use super::*;
use crate::animation::{layers::command, script_child};
use anyhow::Context;
impl super::super::Preparation {
    pub fn initialize_third_person_child_prefix(
        &mut self,
        command: &mut command::Pending,
        source: &mut impl script_child::Provider,
    ) -> Result<()> {
        ensure!(
            self.stage() == super::super::Stage::AwaitingInitialModes,
            "child factory precedes component helper return"
        );
        let owner = self.animator_mut_for_capture()?;
        let result = (|| {
            let request = command.begin_factory(
                owner
                    .layers
                    .as_mut()
                    .context("TP child factory layers absent")?,
            )?;
            ensure!(
                ((request.reference >> 24) & 0x7f) == 4,
                "unsupported reached child factory class"
            );
            let binding = owner.component.context("TP child source wrapper absent")?;
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
        if result.is_err() {
            command.fail_factory(owner.layers.as_mut().expect("retained layers"));
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
