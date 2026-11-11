use super::{Identity, Item, ItemId, Preparation, Registration, Stage};
use anyhow::{Context, Result, ensure};

/// Reached first-node tail append. This is membership and an item backlink;
/// activation, visual postbind and later TP/FP setup are separate operations.
#[derive(Debug)]
pub struct Attachment {
    component: Identity,
    item: ItemId,
    ordinal: usize,
}
impl Preparation {
    pub fn attachment(&self) -> Option<&Attachment> {
        self.attachment.as_ref()
    }
    pub fn attach_tp(&mut self, item: &mut Item, component_limit: usize) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingAttachment,
            "TP attachment is out of source order"
        );
        let animator = self
            .animator
            .as_mut()
            .context("TP attachment animator is absent")?;
        let returned = self
            .bind_return
            .as_ref()
            .context("TP attachment has no actual bind return")?;
        ensure!(
            returned.component() == animator.identity()
                && self.player == Some(animator.player())
                && self.item == Some(item.id())
                && item.handler() == self.player
                && item.registration() == Registration::Complete,
            "TP attachment item/component/player identities differ"
        );
        ensure!(
            item.components().is_empty()
                && animator.word(0) == Some(0)
                && animator.word(4) == Some(0),
            "TP attachment requires the reached fresh empty component list"
        );
        self.stage = Stage::Attaching;
        let result = (|| {
            // Source stores this backlink and byte before appending. A checked
            // host list failure retains them without publishing membership.
            animator.prepare_first_item_append(item.id())?;
            item.attach_component(animator.identity().id(), component_limit)?;
            self.attachment = Some(Attachment {
                component: animator.identity(),
                item: item.id(),
                ordinal: 0,
            });
            Ok(())
        })();
        self.stage = if result.is_ok() {
            Stage::AwaitingVisualPostbind
        } else {
            Stage::Failed
        };
        result
    }
}
impl Attachment {
    pub fn component(&self) -> Identity {
        self.component
    }
    pub fn item(&self) -> ItemId {
        self.item
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.component.visit_state_words(&mut emit);
        for id in [self.item.world().get(), self.item.get()] {
            emit((id >> 32) as u32);
            emit(id as u32);
        }
        emit(self.ordinal as u32);
    }
}
