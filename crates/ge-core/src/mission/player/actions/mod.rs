//! Reached initial action caller. Mapping prepares dispatch inputs; it does not
//! execute mode controls, change mode history or complete the enclosing callback.
use super::{
    base_handler,
    components::{Identity, Preparation, Stage as ComponentStage},
    constructor::PendingReset,
    identity::PlayerId,
    inventory::Inventory,
    item::Item,
};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AwaitingComponents,
    AwaitingInventory,
    Remapping,
    AwaitingThirdPersonDispatch,
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
        emit(1);
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
    }
}

#[cfg(test)]
mod tests;
