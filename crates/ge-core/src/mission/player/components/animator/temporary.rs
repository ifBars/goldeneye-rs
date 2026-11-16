//! Fresh direct-bind leaves for a separate preload animator, never a live mode call.
use super::*;
use crate::animation::{layers::NodeId, script_child};
use anyhow::Context;

impl Animator {
    pub(in crate::mission::player::components) fn bind_temporary_skeleton(
        &mut self,
        input: SkeletonInput<'_>,
        component: ResourceBinding,
        budget: SkeletonBudget,
    ) -> Result<()> {
        ensure!(
            self.item.is_none()
                && self.callbacks.is_none()
                && self.skeleton.is_none()
                && self.word(0x114) == Some(0)
                && self.byte(0x130) == Some(1)
                && self.word(0x138) == Some(0)
                && self.word(0x13c) == Some(0)
                && self.script_children.is_empty()
                && self.controls.is_empty(),
            "temporary bind requires its actual independent fresh owner"
        );
        self.word_store(0x1c, 0);
        self.word_store(0x20, 0);
        self.word_store(0x24, u32::MAX);
        let secondary = input.primary;
        let mut runtime = SkeletonRuntime::initialize(input, budget)?;
        runtime.bind_animator(self.identity);
        self.skeleton = Some(runtime);
        self.bytes[0x114..0x118].fill(None);
        self.bytes[0x134] = Some(0);
        self.bytes[0x135] = Some(0);
        self.word_store(0x11c, 0);
        self.skeleton
            .as_mut()
            .expect("retained temporary skeleton")
            .bind_secondary(secondary);
        self.component = Some(component);
        self.bytes[0x1c..0x20].fill(None);
        self.word_store(0x20, component.reference);
        self.word_store(0x24, u32::MAX);
        Ok(())
    }
    pub(in crate::mission::player::components) fn allocate_temporary_script(
        &mut self,
    ) -> Result<NodeId> {
        let binding = self
            .component
            .context("temporary component binding absent")?;
        let id =
            self.script_children
                .allocate(&mut self.node_ids, binding.file, binding.reference)?;
        self.script_children.bind_factory_context(id, 0, 3)?;
        // Direct factory source writes these fields before virtual initializer.
        self.word_store(0x20, binding.reference);
        self.word_store(0x24, u32::MAX);
        Ok(id)
    }
    pub(in crate::mission::player::components) fn temporary_scripts_mut(
        &mut self,
    ) -> &mut script_child::Store {
        &mut self.script_children
    }
    pub(in crate::mission::player::components) fn insert_temporary_script(
        &mut self,
        id: NodeId,
    ) -> Result<()> {
        ensure!(
            self.script_children
                .get(id)
                .is_some_and(|header| header.initializer_returned())
                && self.word(0x138) == Some(0)
                && self.word(0x13c) == Some(0)
                && self.layers.is_none(),
            "temporary prepend requires actual returned child and empty list"
        );
        self.node_ids.link_head(0, id, None)?;
        // This direct list is retained by node Ids and Temporary.head. Mode
        // command Layers stay absent; no mode command/capture has occurred.
        self.bytes[0x138..0x140].fill(None);
        Ok(())
    }
    /// Supported nonempty direct-list owner: bound skeleton backlink, no item,
    /// callbacks, mode command layers or controls. Slot fields are then read
    /// from this owner's actual bytes and node links, never from live FP.
    pub(in crate::mission::player::components) fn ensure_temporary_evaluation_owner(
        &self,
    ) -> Result<()> {
        ensure!(
            self.skeleton
                .as_ref()
                .is_some_and(|s| s.animator() == Some(self.identity))
                && self.item.is_none()
                && self.callbacks.is_none()
                && self.layers.is_none()
                && self.controls.is_empty()
                && !self.script_children.is_empty(),
            "temporary nonempty evaluation requires its actual bound direct list"
        );
        Ok(())
    }
    /// Caller-suffix evaluation: the same bound owner, whether or not its
    /// direct list is empty. No item, callbacks, mode layers or controls.
    pub(in crate::mission::player::components) fn ensure_temporary_post_bind_owner(
        &self,
    ) -> Result<()> {
        ensure!(
            self.skeleton
                .as_ref()
                .is_some_and(|s| s.animator() == Some(self.identity))
                && self.item.is_none()
                && self.callbacks.is_none()
                && self.layers.is_none()
                && self.controls.is_empty(),
            "temporary post-bind evaluation requires its actual bound owner"
        );
        Ok(())
    }
    /// Skeleton virtual destructor (803322b0, delete=1): the retained runtime
    /// has no linked owner or LOD arrays by construction. +0x114 is not cleared.
    pub(in crate::mission::player::components) fn take_temporary_skeleton(
        &mut self,
    ) -> Result<SkeletonRuntime> {
        ensure!(
            self.skeleton
                .as_ref()
                .is_some_and(|s| s.animator() == Some(self.identity)),
            "temporary skeleton destructor requires its actual bound skeleton"
        );
        Ok(self.skeleton.take().expect("validated temporary skeleton"))
    }
    /// Component binding words written by the animator destructor.
    pub(in crate::mission::player::components) fn clear_temporary_component_words(&mut self) {
        self.word_store(0x1c, 0);
        self.word_store(0x20, 0);
        self.word_store(0x24, u32::MAX);
        self.component = None;
    }
    /// Script store and node identities mutated by slot-child destruction.
    pub(in crate::mission::player::components) fn temporary_children_mut(
        &mut self,
    ) -> (
        &mut crate::animation::nodes::Ids,
        &mut crate::animation::script_child::Store,
    ) {
        (&mut self.node_ids, &mut self.script_children)
    }
    pub(in crate::mission::player::components) fn temporary_empty_refresh(
        &self,
        context: &mut crate::animation::scratch::Context,
        arena: &mut crate::animation::scratch::Arena,
    ) -> Result<crate::animation::scratch::Refresh> {
        ensure!(
            self.skeleton
                .as_ref()
                .is_some_and(|s| s.animator() == Some(self.identity))
                && self.item.is_none()
                && self.callbacks.is_none()
                && self.byte(0x130) == Some(1)
                && self.word(0x138) == Some(0)
                && self.word(0x13c) == Some(0)
                && self.word(0x144) == Some(1f32.to_bits())
                && self.byte(0x140) == Some(0)
                && self.byte(0x141) == Some(0),
            "temporary empty refresh requires bound fresh NULL lists"
        );
        context.refresh_empty(arena)
    }
}

#[cfg(test)]
#[path = "temporary/tests/fixtures.rs"]
pub(crate) mod fixtures;
#[cfg(test)]
#[path = "temporary/tests.rs"]
mod tests;
