//! The second live animator. It shares constructor/bind rules with TP, while
//! retaining its own skin, skeleton, identity and reached partial state.
#[cfg(test)]
#[path = "tests/pose_fixture.rs"]
mod pose_fixture;
mod tail;
use super::skeleton::{FP_COMPONENT, ResourceBinding};
use super::{Animator, Identity, SkeletonBudget, SkeletonInput, Visual};
use crate::{
    animation::scratch::{Arena, Context as RefreshContext, Refresh},
    mission::player::constructor::CharacterBinding,
};
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPersonStage {
    AwaitingVisual,
    AwaitingSkeleton,
    Binding,
    AwaitingRefresh,
    Refreshing,
    AwaitingAttachment,
    Attaching,
    Returned,
    Failed,
}
#[derive(Debug)]
pub struct FirstPerson {
    animator: Animator,
    character: CharacterBinding,
    skin_file: u32,
    visual: Option<Visual>,
    stage: FirstPersonStage,
    refresh: Option<Refresh>,
}
impl FirstPerson {
    pub(super) fn animator_mut_for_modes(&mut self) -> Result<&mut Animator> {
        ensure!(
            self.stage == FirstPersonStage::Returned,
            "FP mode mutation precedes helper return"
        );
        Ok(&mut self.animator)
    }
    pub(super) fn new(identity: Identity, character: CharacterBinding, skin_file: u32) -> Self {
        Self {
            animator: Animator::from_constructor(identity),
            character,
            skin_file,
            visual: None,
            stage: FirstPersonStage::AwaitingVisual,
            refresh: None,
        }
    }
    pub fn animator(&self) -> &Animator {
        &self.animator
    }
    pub fn stage(&self) -> FirstPersonStage {
        self.stage
    }
    pub fn skin_file(&self) -> u32 {
        self.skin_file
    }
    pub fn refresh(&self) -> Option<&Refresh> {
        self.refresh.as_ref()
    }
    pub fn visual_lookup(&mut self, visual: Visual) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::AwaitingVisual && visual.character() == self.character,
            "FP visual lookup order or retained character differs"
        );
        self.visual = Some(visual);
        self.stage = FirstPersonStage::AwaitingSkeleton;
        Ok(())
    }
    pub fn bind_skeleton(
        &mut self,
        input: SkeletonInput<'_>,
        component: ResourceBinding,
        budget: SkeletonBudget,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::AwaitingSkeleton,
            "FP skeleton bind is out of source order"
        );
        ensure!(
            input.primary
                == ResourceBinding {
                    file: self.skin_file,
                    reference: 0x0d00_016f
                }
                && component == FP_COMPONENT,
            "FP skeleton does not match the retained character file/fixed skin or component"
        );
        self.stage = FirstPersonStage::Binding;
        let result = self.animator.bind_skeleton_pair(input, component, budget);
        self.stage = if result.is_ok() {
            FirstPersonStage::AwaitingRefresh
        } else {
            FirstPersonStage::Failed
        };
        result
    }
    pub fn finish_bind_zero_time(
        &mut self,
        context: &mut RefreshContext,
        arena: &mut Arena,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::AwaitingRefresh,
            "FP bind refresh is out of source order"
        );
        self.stage = FirstPersonStage::Refreshing;
        match self.animator.refresh_empty_bind(context, arena) {
            Ok(refresh) => {
                self.refresh = Some(refresh);
                self.stage = FirstPersonStage::AwaitingAttachment;
                Ok(())
            }
            Err(error) => {
                self.stage = FirstPersonStage::Failed;
                Err(error)
            }
        }
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.stage as u32);
        emit(self.skin_file);
        self.animator.visit_state_words(&mut emit);
        emit(u32::from(self.visual.is_some()));
        if let Some(visual) = self.visual {
            visual.visit_state_words(&mut emit);
        }
        emit(u32::from(self.refresh.is_some()));
        if let Some(refresh) = &self.refresh {
            refresh.visit_state_words(emit);
        }
    }
    pub(super) fn set_mask(&mut self, at: usize) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::Returned,
            "FP mask write precedes helper return"
        );
        self.animator.set_first_person_mask(at)
    }
}

#[cfg(test)]
mod tests;
