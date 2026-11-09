use super::{Animator, Visual};
use crate::mission::player::{
    base_handler,
    constructor::{PendingPrelude, PendingReset},
    identity::{ItemId, PlayerId},
    item::{Item, Registration},
};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fresh,
    Clearing,
    AwaitingVisual,
    AwaitingResidency,
    AwaitingTextures,
    Failed,
}
pub trait Provider {
    fn session_manager_present(&mut self) -> Result<bool>;
    fn mark_interface_dirty(&mut self, channel: u32) -> Result<()>;
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
        }
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn animator(&self) -> Option<&Animator> {
        self.animator.as_ref()
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
            self.animator = Some(Animator::from_constructor(base.player()));
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
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(match self.stage {
            Stage::Fresh => 0,
            Stage::Clearing => 1,
            Stage::AwaitingVisual => 2,
            Stage::AwaitingResidency => 3,
            Stage::AwaitingTextures => 4,
            Stage::Failed => 5,
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
            visual.visit_state_words(emit);
        }
    }
}
