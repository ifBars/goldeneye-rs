use super::*;
use crate::mission::player::components::{FirstPerson, callbacks::Table};
use std::sync::Arc;

impl Preparation {
    pub(in crate::mission::player::components) fn first_person_mut_for_modes(
        &mut self,
    ) -> Result<&mut FirstPerson> {
        ensure!(
            self.stage == Stage::AwaitingInitialModes,
            "FP mode mutation precedes component helper returns"
        );
        self.first_person
            .as_mut()
            .context("actual FP mode owner absent")
    }
    pub fn begin_first_person(
        &mut self,
        identity: Identity,
        character: crate::mission::player::constructor::CharacterBinding,
        skin_file: u32,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingFirstPerson
                && self.third_person_return.is_some()
                && self.first_person.is_none(),
            "FP construction requires the actual TP helper return"
        );
        ensure!(
            Some(character) == self.character
                && Some(identity.player()) == self.player
                && self
                    .animator
                    .as_ref()
                    .is_some_and(|tp| tp.identity() != identity),
            "FP constructor identity or retained character differs"
        );
        self.first_person = Some(FirstPerson::new(identity, character, skin_file));
        self.stage = Stage::BuildingFirstPerson;
        Ok(())
    }
    pub fn first_person(&self) -> Option<&FirstPerson> {
        self.first_person.as_ref()
    }
    pub fn first_person_mut(&mut self) -> Result<&mut FirstPerson> {
        ensure!(
            self.stage == Stage::BuildingFirstPerson,
            "FP owner mutation is out of source order"
        );
        self.first_person
            .as_mut()
            .context("actual FP owner is absent")
    }
    pub fn finish_first_person(
        &mut self,
        base: &base_handler::Owner,
        item: &mut Item,
        table: Arc<Table>,
        limit: usize,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::BuildingFirstPerson,
            "FP helper completion is out of source order"
        );
        ensure!(
            self.first_person
                .as_ref()
                .and_then(FirstPerson::refresh)
                .zip(self.bind_return.as_ref().map(BindReturn::refresh))
                .is_some_and(|(fp, tp)| fp.follows_in_same_arena(tp)),
            "FP bind must follow TP through the same shared animation arena"
        );
        let result = self
            .first_person
            .as_mut()
            .context("FP owner is absent")?
            .finish_helper(
                self.animator.as_mut().context("TP owner is absent")?,
                base,
                item,
                table,
                limit,
            );
        self.stage = if result.is_ok() {
            Stage::AwaitingFirstPersonMasks
        } else {
            Stage::Failed
        };
        result
    }
    pub fn configure_first_person_masks(&mut self, item: &Item) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingFirstPersonMasks && Some(item.id()) == self.item,
            "FP mask configuration is out of source order"
        );
        // Both original searches follow the real item's ordered list and test
        // each bound component's resource header. A detached ID is insufficient.
        for at in [0x136, 0x131] {
            let found = item
                .components()
                .iter()
                .find_map(|id| {
                    [
                        self.animator.as_ref(),
                        self.first_person.as_ref().map(FirstPerson::animator),
                    ]
                    .into_iter()
                    .flatten()
                    .find(|owner| {
                        owner.identity().id() == *id
                            && owner
                                .component()
                                .is_some_and(|binding| binding.file == 0x0102_0016)
                    })
                    .map(Animator::identity)
                })
                .context("actual FP component is absent from item resource lookup")?;
            let fp = self.first_person.as_mut().context("FP owner is absent")?;
            ensure!(
                fp.animator().identity() == found,
                "FP resource lookup resolved another owner"
            );
            fp.set_mask(at)?;
        }
        self.stage = Stage::AwaitingInitialModes;
        Ok(())
    }
}
