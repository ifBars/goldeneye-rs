//! Initial action mapping and ordered TP/FP dispatch. Receiver ownership,
//! command returns and history writes stay separate from callback completion.
use super::{
    base_handler,
    components::{Identity, Preparation, Stage as ComponentStage},
    constructor::PendingReset,
    identity::PlayerId,
    inventory::Inventory,
    item::Item,
};
use anyhow::{Context, Result, ensure};
mod capture;
mod child_effects;
mod command;
mod factory;
mod first_person;
mod selector;
mod setter;
pub mod suffix;
mod third_person_tail;
pub use first_person::{FirstPersonDispatch, FirstPersonStage};
pub use setter::SetterReturn;
pub use third_person_tail::{DispatchStage, ThirdPersonDispatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AwaitingComponents,
    AwaitingInventory,
    Remapping,
    AwaitingThirdPersonDispatch,
    CapturingThirdPerson,
    AwaitingThirdPersonSelector,
    SelectingThirdPerson,
    AwaitingThirdPersonCommand,
    AwaitingThirdPersonSetterTail,
    ThirdPersonCommandFailed,
    AwaitingThirdPersonDispatchTail,
    ThirdPersonDispatchFailed,
    AwaitingFirstPersonDispatch,
    FirstPersonDispatch,
    AwaitingActionSuffix,
    ApplyingActionSuffix,
    Returned,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    FirstPerson,
    ThirdPerson,
    ThirdPersonSecondary,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub ordinal: usize,
    pub target: Target,
    pub component: Identity,
    pub weapon: u32,
    pub variant: u32,
    pub generic: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    pub request: Request,
    pub output: u32,
    pub mapped: bool,
}
pub struct Pending {
    player: PlayerId,
    stage: Stage,
    generic: u32,
    components: Option<[Identity; 2]>,
    weapon: Option<u32>,
    variant: Option<u32>,
    resolved: Vec<Resolved>,
    fallback: Option<u32>,
    fallback_writes: Option<u64>,
    selector: Option<crate::animation::mode::runtime::Cursor>,
    tracks_player_modes: Option<bool>,
    primary_head: Option<(Option<crate::animation::layers::NodeId>, u32)>,
    command: Option<crate::animation::layers::command::Pending>,
    completed_commands: Vec<crate::animation::layers::command::Pending>,
    dispatch: Option<ThirdPersonDispatch>,
    first_person: Option<FirstPersonDispatch>,
    suffix: Option<suffix::Suffix>,
}
impl Pending {
    pub fn begin(base: &base_handler::Owner, reset: &mut PendingReset) -> Result<Self> {
        let generic = reset.begin_fresh_action(base)?;
        Ok(Self {
            player: base.player(),
            stage: Stage::AwaitingComponents,
            generic,
            components: None,
            weapon: None,
            variant: None,
            resolved: Vec::new(),
            fallback: None,
            fallback_writes: None,
            selector: None,
            tracks_player_modes: None,
            primary_head: None,
            command: None,
            completed_commands: Vec::new(),
            dispatch: None,
            first_person: None,
            suffix: None,
        })
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn generic(&self) -> u32 {
        self.generic
    }
    pub fn weapon(&self) -> Option<u32> {
        self.weapon
    }
    pub fn variant(&self) -> Option<u32> {
        self.variant
    }
    pub fn resolved(&self) -> &[Resolved] {
        &self.resolved
    }
    pub fn fallback(&self) -> Option<u32> {
        self.fallback
    }
    pub fn fallback_writes(&self) -> Option<u64> {
        self.fallback_writes
    }
    pub fn bind_components(&mut self, preparation: &Preparation, item: &Item) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingComponents
                && preparation.stage() == ComponentStage::AwaitingInitialModes,
            "initial action component lookup is out of source order"
        );
        let tp = preparation
            .animator()
            .context("initial action TP owner is absent")?;
        let fp = preparation
            .first_person()
            .context("initial action FP owner is absent")?
            .animator();
        // Resolve FP first, then TP, from the actual item's resource headers.
        let mut found = [None, None];
        for (slot, file) in [0x0102_0016, 0x0102_0017].into_iter().enumerate() {
            for id in item.components() {
                let owner = [tp, fp]
                    .into_iter()
                    .find(|owner| owner.identity().id() == *id)
                    .context("initial action item component owner provider is absent")?;
                if owner
                    .component()
                    .is_some_and(|binding| binding.file == file)
                {
                    found[slot] = Some(owner.identity());
                    break;
                }
            }
        }
        let fp = found[0].context("initial action FP component lookup returned NULL")?;
        let tp = found[1].context("initial action TP component lookup returned NULL")?;
        ensure!(
            fp.player() == self.player
                && tp.player() == self.player
                && item.handler() == Some(self.player)
                && preparation
                    .animator()
                    .is_some_and(|owner| owner.item() == Some(item.id())),
            "initial action component ownership differs"
        );
        self.components = Some([fp, tp]);
        self.stage = Stage::AwaitingInventory;
        Ok(())
    }
    pub fn bind_inventory(
        &mut self,
        base: &base_handler::Owner,
        inventory: &Inventory,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingInventory && base.player() == self.player,
            "initial action inventory lookup is out of order"
        );
        let slot = inventory
            .slots()
            .c96
            .context("current inventory slot is unknown")? as i8;
        // Source returns NULL for any negative or out-of-count slot.
        let record = if slot < 0 {
            None
        } else {
            inventory.records().get(slot as usize)
        };
        ensure!(
            base.byte(0x5ed) == Some(0),
            "initial action variant requires alternate metadata provider"
        );
        self.weapon = Some(record.map_or(0, |record| record.key));
        self.variant = Some(u32::from(
            record.is_some_and(|record| record.flags & 0x100 != 0),
        ));
        self.stage = Stage::Remapping;
        Ok(())
    }
    pub fn next_remap(&self) -> Result<Option<Request>> {
        ensure!(
            matches!(
                self.stage,
                Stage::Remapping | Stage::AwaitingThirdPersonDispatch
            ),
            "initial action remap is out of source order"
        );
        let Some(target) = [
            Target::FirstPerson,
            Target::ThirdPerson,
            Target::ThirdPersonSecondary,
        ]
        .get(self.resolved.len())
        .copied() else {
            return Ok(None);
        };
        let components = self
            .components
            .context("initial action components are absent")?;
        Ok(Some(Request {
            ordinal: self.resolved.len(),
            target,
            component: components[usize::from(target != Target::FirstPerson)],
            weapon: self.weapon.context("initial action weapon is unknown")?,
            variant: self.variant.context("initial action variant is unknown")?,
            generic: self.generic,
        }))
    }
    pub fn mapped(&mut self, request: Request, output: u32, mapped: bool) -> Result<()> {
        ensure!(
            self.stage == Stage::Remapping && self.next_remap()? == Some(request),
            "initial action mapping owner or order differs"
        );
        self.resolved.push(Resolved {
            request,
            output,
            mapped,
        });
        if self.resolved.len() == 3 {
            self.stage = Stage::AwaitingThirdPersonDispatch;
        }
        Ok(())
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(8);
        emit(self.stage as u32);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.generic);
        emit(u32::from(self.components.is_some()));
        if let Some(components) = self.components {
            for component in components {
                component.visit_state_words(&mut emit);
            }
        }
        for value in [self.weapon, self.variant] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        emit(self.resolved.len() as u32);
        for value in &self.resolved {
            value.request.component.visit_state_words(&mut emit);
            emit(value.request.target as u32);
            emit(value.output);
            emit(u32::from(value.mapped));
        }
        emit(u32::from(self.fallback.is_some()));
        if let Some(value) = self.fallback {
            emit(value);
        }
        emit(u32::from(self.fallback_writes.is_some()));
        if let Some(value) = self.fallback_writes {
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        emit(u32::from(self.tracks_player_modes.is_some()));
        if let Some(value) = self.tracks_player_modes {
            emit(u32::from(value));
        }
        emit(u32::from(self.primary_head.is_some()));
        if let Some((head, word)) = self.primary_head {
            emit(u32::from(head.is_some()));
            if let Some(head) = head {
                emit((head.get() >> 32) as u32);
                emit(head.get() as u32);
            }
            emit(word);
        }
        emit(u32::from(self.selector.is_some()));
        if let Some(selector) = &self.selector {
            selector.visit_state_words(&mut emit);
        }
        emit(u32::from(self.command.is_some()));
        if let Some(command) = &self.command {
            command.visit_state_words(&mut emit);
        }
        emit(self.completed_commands.len() as u32);
        for command in &self.completed_commands {
            command.visit_state_words(&mut emit);
        }
        emit(u32::from(self.dispatch.is_some()));
        if let Some(dispatch) = &self.dispatch {
            dispatch.visit_state_words(&mut emit);
        }
        emit(u32::from(self.first_person.is_some()));
        if let Some(fp) = &self.first_person {
            fp.visit_state_words(&mut emit);
        }
        emit(u32::from(self.suffix.is_some()));
        if let Some(suffix) = &self.suffix {
            suffix.visit_state_words(&mut emit);
        }
    }
}

#[cfg(test)]
mod tests;
