use super::{Animator, Identity, ResourceBinding, SkeletonBudget, SkeletonInput, Visual};
use crate::mission::player::{
    base_handler,
    constructor::{PendingPrelude, PendingReset},
    identity::{ItemId, PlayerId},
    item::{Item, Registration},
};
use anyhow::{Context, Result, ensure};
mod attachment;
mod binding;
mod callbacks;
mod first_person;
mod postbind;
pub use attachment::Attachment;
pub use binding::BindReturn;
pub use callbacks::ThirdPersonReturn;
pub use postbind::VisualPostbind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fresh,
    Clearing,
    AwaitingVisual,
    AwaitingResidency,
    AwaitingTextures,
    AwaitingTextureResidency,
    AwaitingSkeleton,
    BindingSkeleton,
    AwaitingAnimationRefresh,
    Refreshing,
    AwaitingAttachment,
    Attaching,
    AwaitingVisualPostbind,
    AwaitingVisualWrapper,
    AwaitingPostbindResidency,
    VisualPostbinding,
    AwaitingCallbacks,
    BindingCallbacks,
    AwaitingFirstPerson,
    BuildingFirstPerson,
    AwaitingFirstPersonMasks,
    AwaitingInitialModes,
    Failed,
}
/// Directory absence ends the source loop before any part is read. It stays
/// distinct from an enumeration that reached parts and found no textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureOutcome {
    Enumerated,
    EntityDirectoryAbsent,
    TextureDirectoryAbsent,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureEnumeration {
    pub outcome: TextureOutcome,
    /// First-encounter hashes in source order, without duplicates.
    pub hashes: Vec<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureTargetRequest {
    pub file: u32,
    pub target: u32,
    pub ordinal: usize,
}
pub trait Provider {
    fn session_manager_present(&mut self) -> Result<bool>;
    fn mark_interface_dirty(&mut self, channel: u32) -> Result<()>;
    fn allocate_animator(&mut self, player: PlayerId) -> Result<Identity>;
}
pub struct Bindings<'a> {
    pub base: &'a base_handler::Owner,
    pub basic: &'a base_handler::VirtualBasicReceipt,
    pub reset: &'a mut PendingReset,
    pub prelude: &'a mut PendingPrelude,
    pub item: &'a Item,
}
pub struct Preparation {
    stage: Stage,
    player: Option<PlayerId>,
    item: Option<ItemId>,
    animator: Option<Animator>,
    visual: Option<Visual>,
    character: Option<crate::mission::player::constructor::CharacterBinding>,
    textures: Option<TextureEnumeration>,
    texture_requests: usize,
    bind_return: Option<BindReturn>,
    attachment: Option<Attachment>,
    postbind: Option<VisualPostbind>,
    third_person_return: Option<ThirdPersonReturn>,
    first_person: Option<super::FirstPerson>,
}
impl Default for Preparation {
    fn default() -> Self {
        Self::new()
    }
}
impl Preparation {
    pub fn new() -> Self {
        Self {
            stage: Stage::Fresh,
            player: None,
            item: None,
            animator: None,
            visual: None,
            character: None,
            textures: None,
            texture_requests: 0,
            bind_return: None,
            attachment: None,
            postbind: None,
            third_person_return: None,
            first_person: None,
        }
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn animator(&self) -> Option<&Animator> {
        self.animator.as_ref()
    }
    pub fn textures(&self) -> Option<&TextureEnumeration> {
        self.textures.as_ref()
    }
    pub fn texture_requests(&self) -> usize {
        self.texture_requests
    }
    pub fn visual(&self) -> Option<Visual> {
        self.visual
    }
    pub fn prepare(&mut self, bindings: Bindings<'_>, provider: &mut impl Provider) -> Result<()> {
        ensure!(
            self.stage == Stage::Fresh,
            "component preparation already dispatched"
        );
        let Bindings {
            base,
            basic,
            reset,
            prelude,
            item,
        } = bindings;
        ensure!(
            base.stage() == base_handler::Stage::VirtualBasicComplete
                && basic.player() == base.player()
                && reset.player() == base.player()
                && prelude.player() == base.player()
                && basic.item() == item.id()
                && base.item() == Some(item.id())
                && item.handler() == Some(base.player())
                && item.registration() == Registration::Complete,
            "component entry requires matching actual basic owners"
        );
        self.player = Some(base.player());
        self.item = Some(item.id());
        self.stage = Stage::Clearing;
        self.character = reset.character();
        let result = (|| {
            // This is a distinct reached clear, even though the fresh reset
            // previously wrote these bytes. It precedes the cleanup provider.
            reset.clear_component_entry()?;
            if provider.session_manager_present()? {
                ensure!(
                    reset.word(0xbe8) == Some(0),
                    "component cleanup requires the actual previous target owner"
                );
            }
            ensure!(
                item.components().is_empty(),
                "component reinitialization requires real child cleanup/destructors"
            );
            reset.clear_component_fields();
            prelude.inventory_mut().prepare_component_slots()?;
            reset.retain_component_slots(prelude.inventory())?;
            let interface = base
                .word(0x4f8)
                .context("component interface link is unwritten")?;
            if interface != 0 {
                provider.mark_interface_dirty(0x40)?;
            }
            // Source allocates and constructs TP before its visual lookup.
            let identity = provider.allocate_animator(base.player())?;
            ensure!(
                identity.player() == base.player(),
                "TP allocation player differs"
            );
            self.animator = Some(Animator::from_constructor(identity));
            Ok(())
        })();
        self.stage = if result.is_ok() {
            Stage::AwaitingVisual
        } else {
            Stage::Failed
        };
        result
    }
    pub fn bind_visual(&mut self, visual: Visual) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingVisual,
            "component visual binding is out of order"
        );
        ensure!(
            self.character == Some(visual.character()),
            "component visual differs from retained reset character"
        );
        self.visual = Some(visual);
        self.stage = Stage::AwaitingResidency;
        Ok(())
    }
    /// Only after the actual checked skin target request. Texture enumeration
    /// still has no receipt here; it must not be replaced by an empty list.
    pub fn skin_requested(&mut self, file: u32, skin: u32) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingResidency
                && self
                    .visual
                    .is_some_and(|v| v.file() == file && v.skin() == skin),
            "component skin request differs from the retained visual plan"
        );
        self.stage = Stage::AwaitingTextures;
        Ok(())
    }
    /// Retain the actual skin traversal from the same requested target. The
    /// ordered residency requests follow before default skeleton loading.
    pub fn textures_enumerated(
        &mut self,
        file: u32,
        skin: u32,
        textures: TextureEnumeration,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingTextures
                && self
                    .visual
                    .is_some_and(|v| v.file() == file && v.skin() == skin),
            "component texture enumeration differs from the requested skin"
        );
        ensure!(
            textures.outcome == TextureOutcome::Enumerated || textures.hashes.is_empty(),
            "absent texture directories cannot carry hashes"
        );
        ensure!(
            textures.hashes.len() <= 4096,
            "component texture request budget exhausted"
        );
        let mut seen = std::collections::BTreeSet::new();
        ensure!(
            textures.hashes.iter().all(|hash| seen.insert(*hash)),
            "component texture enumeration repeats a hash"
        );
        self.textures = Some(textures);
        self.stage = Stage::AwaitingTextureResidency;
        Ok(())
    }
    /// One same-wrapper target request per unique hash, in encounter order.
    pub fn next_texture_request(&self) -> Result<Option<TextureTargetRequest>> {
        ensure!(
            self.stage == Stage::AwaitingTextureResidency,
            "texture request is out of source order"
        );
        let textures = self
            .textures
            .as_ref()
            .context("texture enumeration is absent")?;
        let visual = self.visual.context("texture visual source is absent")?;
        Ok(textures
            .hashes
            .get(self.texture_requests)
            .map(|&target| TextureTargetRequest {
                file: visual.file(),
                target,
                ordinal: self.texture_requests,
            }))
    }
    /// The host calls this after retaining the actual resolved target owner.
    /// Failed requests leave this request pending and preserve prior progress.
    pub fn texture_requested(&mut self, request: TextureTargetRequest) -> Result<()> {
        ensure!(
            self.next_texture_request()? == Some(request),
            "texture target receipt differs from the next request"
        );
        self.texture_requests = self
            .texture_requests
            .checked_add(1)
            .context("texture request count overflow")?;
        Ok(())
    }
    pub fn finish_texture_requests(&mut self) -> Result<()> {
        ensure!(
            self.next_texture_request()?.is_none(),
            "texture residency requests remain pending"
        );
        self.stage = Stage::AwaitingSkeleton;
        Ok(())
    }
    pub fn bind_default_skeleton(
        &mut self,
        input: SkeletonInput<'_>,
        component: ResourceBinding,
        budget: SkeletonBudget,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingSkeleton,
            "default skeleton bind is out of source order"
        );
        self.stage = Stage::BindingSkeleton;
        let result = self
            .animator
            .as_mut()
            .context("TP animator owner is absent")?
            .bind_skeleton(input, component, budget);
        self.stage = if result.is_ok() {
            Stage::AwaitingAnimationRefresh
        } else {
            Stage::Failed
        };
        result
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(8);
        emit(match self.stage {
            Stage::Fresh => 0,
            Stage::Clearing => 1,
            Stage::AwaitingVisual => 2,
            Stage::AwaitingResidency => 3,
            Stage::AwaitingTextures => 4,
            Stage::Failed => 5,
            Stage::AwaitingSkeleton => 6,
            Stage::AwaitingTextureResidency => 7,
            Stage::BindingSkeleton => 8,
            Stage::AwaitingAnimationRefresh => 9,
            Stage::Refreshing => 10,
            Stage::AwaitingAttachment => 11,
            Stage::Attaching => 12,
            Stage::AwaitingVisualPostbind => 13,
            Stage::AwaitingVisualWrapper => 14,
            Stage::AwaitingPostbindResidency => 15,
            Stage::VisualPostbinding => 16,
            Stage::AwaitingCallbacks => 17,
            Stage::BindingCallbacks => 18,
            Stage::AwaitingFirstPerson => 19,
            Stage::BuildingFirstPerson => 20,
            Stage::AwaitingFirstPersonMasks => 21,
            Stage::AwaitingInitialModes => 22,
        });
        emit(u32::from(self.player.is_some()));
        if let Some(player) = self.player {
            emit((player.get() >> 32) as u32);
            emit(player.get() as u32);
        }
        emit(u32::from(self.item.is_some()));
        if let Some(item) = self.item {
            for id in [item.world().get(), item.get()] {
                emit((id >> 32) as u32);
                emit(id as u32);
            }
        }
        emit(u32::from(self.character.is_some()));
        if let Some(character) = self.character {
            emit(character.registry_resource);
            emit(character.key);
            emit((character.source_index as u64 >> 32) as u32);
            emit(character.source_index as u32);
        }
        emit(u32::from(self.animator.is_some()));
        if let Some(owner) = &self.animator {
            owner.visit_state_words(&mut emit);
        }
        emit(u32::from(self.visual.is_some()));
        if let Some(visual) = self.visual {
            visual.visit_state_words(&mut emit);
        }
        emit(u32::from(self.textures.is_some()));
        if let Some(textures) = &self.textures {
            emit(match textures.outcome {
                TextureOutcome::Enumerated => 0,
                TextureOutcome::EntityDirectoryAbsent => 1,
                TextureOutcome::TextureDirectoryAbsent => 2,
            });
            emit(textures.hashes.len() as u32);
            for hash in &textures.hashes {
                emit(*hash);
            }
        }
        emit((self.texture_requests as u64 >> 32) as u32);
        emit(self.texture_requests as u32);
        emit(u32::from(self.bind_return.is_some()));
        if let Some(receipt) = &self.bind_return {
            receipt.visit_state_words(&mut emit);
        }
        emit(u32::from(self.attachment.is_some()));
        if let Some(attachment) = &self.attachment {
            attachment.visit_state_words(&mut emit);
        }
        emit(u32::from(self.postbind.is_some()));
        if let Some(postbind) = &self.postbind {
            postbind.visit_state_words(&mut emit);
        }
        emit(u32::from(self.third_person_return.is_some()));
        if let Some(returned) = &self.third_person_return {
            returned.visit_state_words(&mut emit);
        }
        emit(u32::from(self.first_person.is_some()));
        if let Some(fp) = &self.first_person {
            fp.visit_state_words(emit);
        }
    }
}

#[cfg(test)]
#[path = "tests/skeleton.rs"]
mod skeleton_tests;

#[cfg(test)]
#[path = "tests/attachment.rs"]
mod attachment_tests;
