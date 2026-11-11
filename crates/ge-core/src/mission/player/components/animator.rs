use super::super::identity::{ItemId, PlayerId};
use super::{
    Identity, ResourceBinding, SkeletonBudget, SkeletonInput, SkeletonRuntime,
    skeleton::{DEFAULT_SKIN, FP_COMPONENT, TP_COMPONENT},
};
use anyhow::{Result, ensure};
use std::sync::Arc;

pub const SOURCE_BYTES: usize = 0x1b8;

/// Actual pending TP allocation, scoped to its one-shot player initializer.
/// Item/resource/skeleton backlinks are NULL until their real bind operations.
#[derive(Debug)]
pub struct Animator {
    identity: Identity,
    item: Option<ItemId>,
    bytes: [Option<u8>; SOURCE_BYTES],
    // Base ctor clears bits1..7 but preserves allocation bit0 at +8f.
    flag_8f_known_mask: u8,
    skeleton: Option<SkeletonRuntime>,
    component: Option<ResourceBinding>,
    callbacks: Option<Arc<super::callbacks::Table>>,
    previous: Option<Identity>,
    next: Option<Identity>,
}
impl Animator {
    pub(super) fn from_constructor(identity: Identity) -> Self {
        let mut owner = Self {
            identity,
            item: None,
            bytes: [None; SOURCE_BYTES],
            flag_8f_known_mask: 0xfe,
            skeleton: None,
            component: None,
            callbacks: None,
            previous: None,
            next: None,
        };
        for offset in [
            0, 4, 8, 0x10, 0x14, 0x1c, 0x20, 0x28, 0x5c, 0x60, 0x94, 0x114, 0x118, 0x11c, 0x120,
            0x124, 0x128, 0x12c,
        ] {
            owner.word_store(offset, 0);
        }
        owner.word_store(0x24, u32::MAX);
        for offset in (0x2c..0x4c)
            .step_by(4)
            .chain((0xd8..0xe8).step_by(4))
            .chain((0xf8..0x108).step_by(4))
        {
            owner.word_store(offset, 0);
        }
        for offset in (0x4c..0x5c).step_by(4).chain((0xe8..0xf8).step_by(4)) {
            owner.word_store(offset, 1f32.to_bits());
        }
        for index in 0..16 {
            owner.word_store(
                0x98 + index * 4,
                if index.is_multiple_of(5) {
                    1f32.to_bits()
                } else {
                    0
                },
            );
        }
        for offset in [0x64, 0x6c, 0x6e, 0x70, 0x72, 0x76, 0x90] {
            owner.half_store(offset, 0);
        }
        owner.half_store(0x68, 0x7fff);
        owner.half_store(0xc, 0xb);
        for offset in [
            0x66, 0x67, 0x74, 0x75, 0x8d, 0x8e, 0x110, 0x132, 0x133, 0x134, 0x135,
        ] {
            owner.bytes[offset] = Some(0);
        }
        owner.bytes[0x8c] = Some(0x10);
        owner.bytes[0x93] = Some(2);
        owner.word_store(0x108, 1f32.to_bits());
        owner.word_store(0x10c, 0);
        owner.bytes[0x130] = Some(1);
        owner.bytes[0x131] = Some(0xff);
        owner.bytes[0x136] = Some(0xff);
        for start in [0x138, 0x158, 0x178, 0x198] {
            owner.word_store(start, 0);
            owner.word_store(start + 4, 0);
            owner.half_store(start + 8, 0);
            owner.bytes[start + 10] = Some(2);
            owner.word_store(start + 12, 1f32.to_bits());
            for delta in [16, 20, 24, 28] {
                owner.word_store(start + delta, u32::MAX);
            }
        }
        owner
    }
    fn word_store(&mut self, at: usize, bits: u32) {
        for (index, byte) in bits.to_be_bytes().into_iter().enumerate() {
            self.bytes[at + index] = Some(byte);
        }
    }
    fn half_store(&mut self, at: usize, bits: u16) {
        for (index, byte) in bits.to_be_bytes().into_iter().enumerate() {
            self.bytes[at + index] = Some(byte);
        }
    }
    pub fn player(&self) -> PlayerId {
        self.identity.player()
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
    pub fn item(&self) -> Option<ItemId> {
        self.item
    }
    pub fn previous_component(&self) -> Option<Identity> {
        self.previous
    }
    pub fn next_component(&self) -> Option<Identity> {
        self.next
    }
    pub(super) fn append_after(
        &mut self,
        previous: &mut Self,
        item: &mut super::super::item::Item,
        limit: usize,
    ) -> Result<()> {
        ensure!(
            previous.item() == Some(item.id())
                && previous.next.is_none()
                && previous.word(4) == Some(0)
                && self.player() == previous.player(),
            "component tail owner differs or is already linked"
        );
        self.prepare_item_backlink(item.id())?;
        // Intrusive append: new.previous, new.next=NULL, old_tail.next, then
        // the owner's tail. Keep reached links on checked host-list failure.
        self.previous = Some(previous.identity());
        self.bytes[0..4].fill(None);
        self.word_store(4, 0);
        previous.next = Some(self.identity());
        previous.bytes[4..8].fill(None);
        item.attach_component(self.identity().id(), limit)
    }
    pub(super) fn prepare_first_item_append(&mut self, item: ItemId) -> Result<()> {
        self.prepare_item_backlink(item)?;
        // The reached list is empty: append stores NULL previous/next before
        // publishing the new head/tail, even though construction wrote NULL.
        self.word_store(0, 0);
        self.word_store(4, 0);
        Ok(())
    }
    fn prepare_item_backlink(&mut self, item: ItemId) -> Result<()> {
        ensure!(
            self.item.is_none() && self.word(8) == Some(0),
            "component item backlink is already bound"
        );
        self.item = Some(item);
        self.bytes[8..12].fill(None);
        self.bytes[0x8e] = Some(0);
        Ok(())
    }
    /// Fresh bind prefix. The caller must execute the actual shared zero-time
    /// refresh before treating the native bind as returned or attaching TP.
    pub(super) fn bind_skeleton(
        &mut self,
        input: SkeletonInput<'_>,
        component: ResourceBinding,
        budget: SkeletonBudget,
    ) -> Result<()> {
        ensure!(
            component == TP_COMPONENT && input.primary == DEFAULT_SKIN,
            "TP component file/reference differs"
        );
        self.bind_skeleton_pair(input, component, budget)
    }
    pub(super) fn bind_skeleton_pair(
        &mut self,
        input: SkeletonInput<'_>,
        component: ResourceBinding,
        budget: SkeletonBudget,
    ) -> Result<()> {
        ensure!(
            component == TP_COMPONENT || component == FP_COMPONENT,
            "unsupported player component pair"
        );
        ensure!(
            self.skeleton.is_none() && self.word(0x114) == Some(0),
            "skeleton rebind requires actual destruction"
        );
        ensure!(
            self.byte(0x130) == Some(1)
                && self.word(0x138) == Some(0)
                && self.word(0x13c) == Some(0),
            "fresh bind requires empty active controls"
        );
        // Empty slot cleanup has no child callback. These source stores precede
        // primary initialization and remain reached when checked input fails.
        self.word_store(0x1c, 0);
        self.word_store(0x20, 0);
        self.word_store(0x24, u32::MAX);
        let mut runtime = SkeletonRuntime::initialize(input, budget)?;
        runtime.bind_animator(self.identity);
        self.skeleton = Some(runtime);
        // A semantic owner link replaces constructor NULL; do not keep stale
        // zero in the sparse numeric view or manufacture a source pointer.
        self.bytes[0x114..0x118].fill(None);
        self.bytes[0x134] = Some(0);
        self.bytes[0x135] = Some(0);
        self.word_store(0x11c, 0);
        let runtime = self.skeleton.as_mut().expect("installed runtime");
        let primary = runtime.primary();
        runtime.bind_secondary(primary);
        self.component = Some(component);
        self.bytes[0x1c..0x20].fill(None);
        self.word_store(0x20, component.reference);
        self.word_store(0x24, u32::MAX);
        // Raw reference0 has neither class03 nor class04. The mode dispatch
        // returns NULL without reading an animation directory or adding a node.
        Ok(())
    }
    pub fn skeleton(&self) -> Option<&SkeletonRuntime> {
        self.skeleton.as_ref()
    }
    pub(super) fn bind_visual_skin(&mut self, binding: ResourceBinding) -> Result<()> {
        ensure!(
            self.item.is_some(),
            "visual skin callback needs actual item attachment"
        );
        self.skeleton
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("visual skin callback runtime is absent"))?
            .bind_secondary(binding);
        Ok(())
    }
    pub(super) fn configure_headers(&mut self) {
        for start in [0x138, 0x158, 0x178, 0x198] {
            self.half_store(start + 8, 1);
            self.bytes[start + 10] = Some(3);
        }
        self.bytes[0x8c] = self.bytes[0x8c].map(|bits| bits | 1);
    }
    pub fn component(&self) -> Option<ResourceBinding> {
        self.component
    }
    pub(super) fn set_first_person_mask(&mut self, at: usize) -> Result<()> {
        ensure!(
            matches!(at, 0x131 | 0x136) && self.component == Some(FP_COMPONENT),
            "invalid FP mask target"
        );
        self.bytes[at] = Some(0xfb);
        Ok(())
    }
    pub fn callback_table(&self) -> Option<&Arc<super::callbacks::Table>> {
        self.callbacks.as_ref()
    }
    pub(super) fn bind_callback_table(
        &mut self,
        table: Arc<super::callbacks::Table>,
    ) -> Result<()> {
        ensure!(
            self.callbacks.is_none() && self.word(0x118) == Some(0),
            "TP callback table is already bound or unknown"
        );
        self.callbacks = Some(table);
        self.bytes[0x118..0x11c].fill(None);
        Ok(())
    }
    pub(super) fn assign_player_pose_callback(&mut self) -> Result<()> {
        self.skeleton
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("pose callback skeleton is absent"))?
            .assign_player_pose_callback()
    }
    pub(super) fn refresh_empty_bind(
        &self,
        context: &mut crate::animation::scratch::Context,
        arena: &mut crate::animation::scratch::Arena,
    ) -> Result<crate::animation::scratch::Refresh> {
        ensure!(
            self.skeleton
                .as_ref()
                .is_some_and(|runtime| runtime.animator() == Some(self.identity))
                && matches!(self.component, Some(TP_COMPONENT | FP_COMPONENT)),
            "TP refresh requires bound skeletal owners"
        );
        ensure!(
            self.byte(0x130) == Some(1)
                && self.word(0x138) == Some(0)
                && self.word(0x13c) == Some(0)
                && self.byte(0x140) == Some(0)
                && self.byte(0x141) == Some(0)
                && self.word(0x144) == Some(1f32.to_bits()),
            "TP bind refresh requires actual fresh empty controls"
        );
        // Forward tick/reverse child lists are empty. Slot flags bit0 is clear,
        // so list maintenance also returns before any mode or scalar read.
        context.refresh_empty(arena)
    }
    pub fn byte(&self, at: usize) -> Option<u8> {
        self.bytes.get(at).copied().flatten()
    }
    pub fn word(&self, at: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.byte(at)?,
            self.byte(at.checked_add(1)?)?,
            self.byte(at.checked_add(2)?)?,
            self.byte(at.checked_add(3)?)?,
        ]))
    }
    pub fn flag_8f_known_bits(&self) -> (u8, u8) {
        (self.flag_8f_known_mask, 0)
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(5);
        self.identity.visit_state_words(&mut emit);
        emit(u32::from(self.item.is_some()));
        if let Some(item) = self.item {
            for id in [item.world().get(), item.get()] {
                emit((id >> 32) as u32);
                emit(id as u32);
            }
        }
        // Semantic Animator class instead of the original vtable pointer.
        emit(1);
        emit(u32::from(self.flag_8f_known_mask));
        emit(0);
        for (at, byte) in self.bytes.iter().enumerate() {
            emit(at as u32);
            emit(u32::from(byte.is_some()));
            if let Some(byte) = byte {
                emit(u32::from(*byte));
            }
        }
        emit(u32::from(self.skeleton.is_some()));
        if let Some(skeleton) = &self.skeleton {
            skeleton.visit_state_words(&mut emit);
        }
        emit(u32::from(self.component.is_some()));
        if let Some(component) = self.component {
            emit(component.file);
            emit(component.reference);
        }
        emit(u32::from(self.callbacks.is_some()));
        if let Some(table) = &self.callbacks {
            table.visit_state_words(&mut emit);
        }
        for link in [self.previous, self.next] {
            emit(u32::from(link.is_some()));
            if let Some(link) = link {
                link.visit_state_words(&mut emit);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/animator.rs"]
mod tests;
