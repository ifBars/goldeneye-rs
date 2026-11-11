use super::{Identity, Item, ItemId, Preparation, ResourceBinding, Stage, TextureEnumeration};
use crate::mission::player::base_handler;
use anyhow::{Context, Result, ensure};

#[derive(Debug)]
pub struct VisualPostbind {
    component: Identity,
    item: ItemId,
    secondary: ResourceBinding,
    texture_requests: usize,
    overrides: base_handler::VisualOverrides,
}
impl Preparation {
    fn check_visual_item(&self, base: &base_handler::Owner, item: &Item) -> Result<()> {
        let animator = self
            .animator
            .as_ref()
            .context("visual animator is absent")?;
        ensure!(
            self.player == Some(base.player())
                && base.item() == Some(item.id())
                && self.item == Some(item.id())
                && item.handler() == self.player
                && animator.item() == Some(item.id())
                && item.components() == [animator.identity().id()],
            "visual postbind owner or item-list lookup differs"
        );
        Ok(())
    }
    pub fn begin_visual_postbind(
        &mut self,
        base: &mut base_handler::Owner,
        item: &Item,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingVisualPostbind,
            "visual postbind clear is out of order"
        );
        self.check_visual_item(base, item)?;
        base.clear_visual_slots()?;
        self.stage = Stage::AwaitingVisualWrapper;
        Ok(())
    }
    /// Only after the actual same-wrapper body lookup. The vtable callback finds
    /// this attached Animator by type; no mode, bone resize or pose work runs.
    pub fn visual_skin_bound(&mut self, binding: ResourceBinding) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingVisualWrapper,
            "visual skin callback is out of order"
        );
        let visual = self.visual.context("visual postbind row is absent")?;
        ensure!(
            binding
                == ResourceBinding {
                    file: visual.file(),
                    reference: visual.callback_skin()
                },
            "visual callback binding differs from the source row"
        );
        self.animator
            .as_mut()
            .context("visual animator is absent")?
            .bind_visual_skin(binding)?;
        self.stage = Stage::AwaitingPostbindResidency;
        Ok(())
    }
    /// Host acknowledgement follows the second real skin request and every
    /// repeated ordered texture request. Earlier target owners remain retained.
    pub fn finish_visual_postbind(
        &mut self,
        base: &mut base_handler::Owner,
        item: &mut Item,
        binding: ResourceBinding,
        textures: &TextureEnumeration,
        retained_targets: usize,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingPostbindResidency,
            "visual postbind residency is out of order"
        );
        self.check_visual_item(base, item)?;
        let visual = self.visual.context("visual postbind row is absent")?;
        ensure!(
            binding
                == ResourceBinding {
                    file: visual.file(),
                    reference: visual.skin()
                }
                && Some(textures) == self.textures.as_ref()
                && retained_targets == textures.hashes.len(),
            "postbind residency differs from repeated actual visual requests"
        );
        self.stage = Stage::VisualPostbinding;
        let result = (|| {
            let animator = self
                .animator
                .as_mut()
                .context("visual animator is absent")?;
            base.bind_visual_skeleton(animator.identity())?;
            ensure!(
                visual.extra_skins() == [u32::MAX; 2],
                "extra visual skin branches require actual runtime providers"
            );
            let overrides = base.reset_empty_visual_overrides()?;
            ensure!(
                visual.override_textures() == [u32::MAX; 3],
                "explicit visual overrides require actual material providers"
            );
            item.bind_visual_overrides(base.player())?;
            animator.configure_tp_headers();
            self.postbind = Some(VisualPostbind {
                component: animator.identity(),
                item: item.id(),
                secondary: ResourceBinding {
                    file: visual.file(),
                    reference: visual.callback_skin(),
                },
                texture_requests: retained_targets,
                overrides,
            });
            Ok(())
        })();
        self.stage = if result.is_ok() {
            Stage::AwaitingCallbacks
        } else {
            Stage::Failed
        };
        result
    }
    pub fn visual_postbind(&self) -> Option<&VisualPostbind> {
        self.postbind.as_ref()
    }
}
impl VisualPostbind {
    pub fn component(&self) -> Identity {
        self.component
    }
    pub fn item(&self) -> ItemId {
        self.item
    }
    pub fn secondary(&self) -> ResourceBinding {
        self.secondary
    }
    pub fn texture_requests(&self) -> usize {
        self.texture_requests
    }
    pub fn overrides(&self) -> base_handler::VisualOverrides {
        self.overrides
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.component.visit_state_words(&mut emit);
        for id in [self.item.world().get(), self.item.get()] {
            emit((id >> 32) as u32);
            emit(id as u32);
        }
        emit(self.secondary.file);
        emit(self.secondary.reference);
        emit(self.texture_requests as u32);
        for raw in [
            self.overrides.generation,
            self.overrides.capacity,
            self.overrides.growth,
            self.overrides.count,
        ] {
            emit(raw);
        }
    }
}
