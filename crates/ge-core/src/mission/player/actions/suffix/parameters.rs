use super::*;
use crate::animation::script_child::parameters::blended_speeds;
use crate::mission::player::actions::secondary_parameters::{self, Globals, LowerKind};

/// Source ordered comparisons preserve NaN and the sign of the selected zero.
fn blend_weights(bits: u32) -> [u32; 2] {
    let input = f32::from_bits(bits);
    let first = if 0.5f32 <= input { 0.5f32 } else { input };
    let difference = input - 0.5f32;
    let second = if 0f32 >= difference { 0f32 } else { difference };
    [(2f32 * first).to_bits(), (2f32 * second).to_bits()]
}

fn secondary_coordinate(
    globals: Option<&Globals>,
    inputs: impl FnOnce() -> Result<(u32, u32)>,
) -> Result<Option<u32>> {
    let Some(globals) = globals else {
        return Ok(None);
    };
    let (ec4, coordinate) = inputs()?;
    Ok(Some(secondary_parameters::coordinate_bits(
        ec4,
        coordinate,
        *globals,
        LowerKind::Suffix,
    )))
}

impl Suffix {
    fn apply_parameter(&mut self, operation: impl FnOnce() -> Result<bool>) -> Result<()> {
        self.parameter_visits += 1;
        self.parameter_writes += usize::from(operation()?);
        Ok(())
    }

    fn first_person_parameters(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
        item: &Item,
        globals: Option<&Globals>,
    ) -> Result<()> {
        let Some(receiver) = find_receiver(preparation, item, reset.player(), 0x0102_0016)? else {
            self.stage = SuffixStage::ThirdPersonParameters;
            return Ok(());
        };
        self.apply_parameter(|| {
            preparation.set_mode_speed(receiver, 1f32.to_bits())?;
            Ok(true)
        })?;
        let primary = reset
            .word(0x70c)
            .context("FP parameter primary history unknown")?;
        if primary.wrapping_sub(0x0900_071e) <= 3 || primary.wrapping_sub(0x0900_0cf7) <= 1 {
            let bits = reset
                .word(0x99c)
                .context("FP parameter coordinate unknown")?;
            self.apply_parameter(|| preparation.write_mode_dispatch_scalar(receiver, 1, bits))?;
            self.apply_parameter(|| {
                preparation.set_mode_weight(receiver, 1, 1f32.to_bits())?;
                Ok(true)
            })?;
        }
        let secondary = reset
            .word(0x710)
            .context("FP parameter secondary history unknown")?;
        if matches!(secondary, 0x0900_0765 | 0x0900_07a7) {
            let Some(bits) = secondary_coordinate(globals, || {
                let ec4 = reset
                    .word(0xec4)
                    .context("FP secondary coordinate blend unknown")?;
                let coordinate = reset
                    .word(0x9a0)
                    .context("FP secondary coordinate input unknown")?;
                Ok((ec4, coordinate))
            })?
            else {
                self.parameter_dependency = Some("fp_secondary_speed_parameters");
                return Ok(());
            };
            self.apply_parameter(|| preparation.write_mode_dispatch_scalar(receiver, 0, bits))?;
        }
        self.stage = SuffixStage::ThirdPersonParameters;
        Ok(())
    }

    fn third_person_parameters(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
        item: &Item,
    ) -> Result<()> {
        let Some(receiver) = find_receiver(preparation, item, reset.player(), 0x0102_0017)? else {
            self.stage = SuffixStage::Returned;
            return Ok(());
        };
        self.apply_parameter(|| {
            preparation.set_mode_speed(receiver, 1f32.to_bits())?;
            Ok(true)
        })?;
        let primary = reset
            .word(0x70c)
            .context("TP parameter primary history unknown")?;
        match primary {
            0x0900_071e..=0x0900_071f => {
                let head = preparation
                    .mode_animator(receiver)?
                    .layers()
                    .context("TP parameter layers absent")?
                    .slots()[0]
                    .nodes_head()
                    .first()
                    .copied();
                if head.is_some() {
                    let weights =
                        blend_weights(reset.word(0x99c).context("TP parameter blend unknown")?);
                    self.apply_parameter(|| {
                        preparation.set_mode_weight(receiver, 2, weights[0])?;
                        Ok(true)
                    })?;
                    self.apply_parameter(|| {
                        preparation.set_mode_weight(receiver, 3, weights[1])?;
                        Ok(true)
                    })?;
                    let speeds = blended_speeds(weights[1], 30f32.to_bits(), 20f32.to_bits());
                    for (slot, bits) in [2, 3].into_iter().zip(speeds) {
                        self.apply_parameter(|| {
                            preparation.set_mode_child_speed(receiver, slot, bits)
                        })?;
                    }
                }
                for slot in [2, 3] {
                    let bits = reset
                        .word(0x9a4)
                        .context("TP parameter coordinate unknown")?;
                    self.apply_parameter(|| {
                        preparation.write_mode_dispatch_scalar(receiver, slot, bits)
                    })?;
                }
                let bits = reset
                    .word(0xc8c)
                    .context("TP parameter slot1 weight unknown")?;
                self.apply_parameter(|| {
                    preparation.set_mode_weight(receiver, 1, bits)?;
                    Ok(true)
                })?;
            }
            // These reached source branches require their own scalar/flag
            // providers; retain the preceding common speed store while waiting.
            0x0900_0720..=0x0900_0721 | 0x0900_083a | 0x0900_0857 | 0x0900_0cf7..=0x0900_0cf8 => {
                self.parameter_dependency = Some("tp_primary_mode_parameters");
                return Ok(());
            }
            _ => {}
        }
        let secondary = reset
            .word(0x710)
            .context("TP parameter secondary history unknown")?;
        if matches!(secondary, 0x0900_0765 | 0x0900_07a7) {
            for slot in [0, 1] {
                let bits = reset
                    .word(0x9a0)
                    .context("TP secondary coordinate unknown")?;
                self.apply_parameter(|| {
                    preparation.write_mode_dispatch_scalar(receiver, slot, bits)
                })?;
            }
        }
        self.stage = SuffixStage::Returned;
        Ok(())
    }
}

impl Pending {
    pub fn finish_action_parameters(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
        item: &Item,
    ) -> Result<()> {
        self.finish_action_parameters_with_globals(reset, preparation, item, None)
    }

    pub fn finish_action_parameters_with_globals(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
        item: &Item,
        globals: Option<&Globals>,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::ApplyingActionSuffix && reset.player() == self.player,
            "action parameter owner/order differs"
        );
        let suffix = self.suffix.as_mut().context("parameter suffix absent")?;
        ensure!(
            suffix.stage == SuffixStage::FirstPersonParameters
                && suffix.parameter_dependency.is_none(),
            "action parameters replay or precede masks"
        );
        let result = (|| {
            suffix.first_person_parameters(reset, preparation, item, globals)?;
            if suffix.stage == SuffixStage::ThirdPersonParameters {
                suffix.third_person_parameters(reset, preparation, item)?;
            }
            Ok(())
        })();
        if result.is_err() {
            suffix.stage = SuffixStage::Failed;
        }
        if suffix.stage == SuffixStage::Returned {
            self.stage = Stage::Returned;
        }
        result
    }
}

#[cfg(test)]
#[path = "parameters/tests.rs"]
mod tests;
