//! Ordered suffix of one action invocation. Its return is not the enclosing
//! player callback's return; that callback can submit another action afterward.
use super::*;
mod masks;
mod parameters;
mod repeat;
pub use masks::{MaskHelper, MaskLookup, MaskSource, MaskValue};
pub use repeat::RepeatRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuffixStage {
    FirstPersonMasks,
    ThirdPersonMasks,
    FirstPersonParameters,
    ThirdPersonParameters,
    Returned,
    Failed,
}

#[derive(Debug)]
pub struct Suffix {
    stage: SuffixStage,
    classification: u8,
    helpers: [MaskHelper; 2],
    parameter_visits: usize,
    parameter_writes: usize,
    parameter_dependency: Option<&'static str>,
    repeat_request: Option<RepeatRequest>,
}

impl Suffix {
    fn new(classification: u8) -> Self {
        Self {
            stage: SuffixStage::FirstPersonMasks,
            classification,
            helpers: [
                MaskHelper::new(Target::FirstPerson),
                MaskHelper::new(Target::ThirdPerson),
            ],
            parameter_visits: 0,
            parameter_writes: 0,
            parameter_dependency: None,
            repeat_request: None,
        }
    }
    pub fn stage(&self) -> SuffixStage {
        self.stage
    }
    pub fn classification(&self) -> u8 {
        self.classification
    }
    pub fn mask_helpers(&self) -> &[MaskHelper; 2] {
        &self.helpers
    }
    pub fn parameter_visits(&self) -> usize {
        self.parameter_visits
    }
    pub fn parameter_writes(&self) -> usize {
        self.parameter_writes
    }
    pub fn parameter_dependency(&self) -> Option<&'static str> {
        self.parameter_dependency
    }
    pub fn repeat_request(&self) -> Option<RepeatRequest> {
        self.repeat_request
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(1);
        emit(self.stage as u32);
        emit(u32::from(self.classification));
        for helper in &self.helpers {
            helper.visit_state_words(emit);
        }
        emit(self.parameter_visits as u32);
        emit(self.parameter_writes as u32);
        emit(u32::from(self.parameter_dependency.is_some()));
        if let Some(dependency) = self.parameter_dependency {
            emit(dependency.len() as u32);
            for byte in dependency.bytes() {
                emit(u32::from(byte));
            }
        }
        emit(u32::from(self.repeat_request.is_some()));
        if let Some(request) = self.repeat_request {
            emit(request.generic);
            emit(request.parameter_bits);
        }
    }
}

fn classify(primary: u32, secondary: impl FnOnce() -> Result<u32>) -> Result<u8> {
    Ok(if primary.wrapping_sub(0x0900_071e) <= 3 {
        1
    } else if primary.wrapping_sub(0x0900_0cf7) <= 1 {
        2
    } else if primary == 0x0900_083a {
        3
    } else {
        u8::from(secondary()? != 0)
    })
}

fn find_receiver(
    preparation: &Preparation,
    item: &Item,
    player: PlayerId,
    file: u32,
) -> Result<Option<Identity>> {
    ensure!(
        item.handler() == Some(player),
        "suffix item handler differs"
    );
    for id in item.components() {
        let owner = preparation
            .animator()
            .into_iter()
            .chain(preparation.first_person().map(|fp| fp.animator()))
            .find(|owner| owner.identity().id() == *id)
            .context("suffix item component owner provider absent")?;
        ensure!(
            owner.player() == player && owner.item() == Some(item.id()),
            "suffix receiver item/player differs"
        );
        if owner
            .component()
            .is_some_and(|binding| binding.file == file)
        {
            return Ok(Some(owner.identity()));
        }
    }
    Ok(None)
}

impl Pending {
    pub fn action_suffix(&self) -> Option<&Suffix> {
        self.suffix.as_ref()
    }
    pub fn begin_action_suffix(&mut self, reset: &PendingReset) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingActionSuffix
                && self.suffix.is_none()
                && reset.player() == self.player,
            "action suffix owner/order differs or replays"
        );
        let classification = classify(
            reset
                .word(0x70c)
                .context("suffix primary history unknown")?,
            || {
                reset
                    .word(0x710)
                    .context("suffix secondary history unknown")
            },
        )?;
        self.suffix = Some(Suffix::new(classification));
        self.stage = Stage::ApplyingActionSuffix;
        Ok(())
    }

    pub fn advance_action_masks(
        &mut self,
        preparation: &mut Preparation,
        item: &Item,
        source: &mut impl MaskSource,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::ApplyingActionSuffix,
            "action masks entered out of order"
        );
        let suffix = self.suffix.as_mut().context("action suffix absent")?;
        ensure!(
            matches!(
                suffix.stage,
                SuffixStage::FirstPersonMasks | SuffixStage::ThirdPersonMasks
            ),
            "action masks replay or follow parameters"
        );
        let result = (|| {
            while matches!(
                suffix.stage,
                SuffixStage::FirstPersonMasks | SuffixStage::ThirdPersonMasks
            ) {
                let index = usize::from(suffix.stage == SuffixStage::ThirdPersonMasks);
                let helper = &mut suffix.helpers[index];
                if !helper.searched {
                    helper.receiver = find_receiver(
                        preparation,
                        item,
                        self.player,
                        if index == 0 { 0x0102_0016 } else { 0x0102_0017 },
                    )?;
                    helper.searched = true;
                }
                helper.advance(suffix.classification, source, |receiver, slot, words| {
                    preparation.apply_mode_bone_mask(receiver, slot, words)
                })?;
                if !helper.returned {
                    return Ok(());
                }
                suffix.stage = if index == 0 {
                    SuffixStage::ThirdPersonMasks
                } else {
                    SuffixStage::FirstPersonParameters
                };
            }
            Ok(())
        })();
        if result.is_err() {
            suffix.stage = SuffixStage::Failed;
        }
        result
    }
}

#[cfg(test)]
mod tests;
