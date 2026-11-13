//! Resolve a mode command's actual retained TP/FP receiver. Identity lookup does
//! not substitute one animator for another or construct new mode state.
use super::*;
use crate::animation::layers;
use anyhow::Context;

impl super::super::Preparation {
    pub fn mode_animator(&self, receiver: Identity) -> Result<&Animator> {
        ensure!(
            self.stage() == super::super::Stage::AwaitingInitialModes,
            "mode receiver lookup precedes component helper returns"
        );
        let tp = self.animator().context("mode receiver TP owner absent")?;
        ensure!(
            tp.player() == receiver.player(),
            "mode receiver belongs to another player"
        );
        if tp.identity() == receiver {
            return Ok(tp);
        }
        let fp = self
            .first_person()
            .context("mode receiver FP owner absent")?;
        ensure!(
            fp.stage() == super::super::FirstPersonStage::Returned
                && fp.animator().identity() == receiver,
            "mode receiver differs from actual TP/FP identities"
        );
        Ok(fp.animator())
    }

    pub(super) fn mode_animator_mut(&mut self, receiver: Identity) -> Result<&mut Animator> {
        self.mode_animator(receiver)?;
        if self.animator().is_some_and(|tp| tp.identity() == receiver) {
            return self.animator_mut_for_capture();
        }
        self.first_person_mut_for_modes()?.animator_mut_for_modes()
    }

    pub fn bind_empty_mode_layers(&mut self, receiver: Identity) -> Result<()> {
        self.mode_animator_mut(receiver)?.bind_empty_mode_layers()
    }

    pub fn write_mode_dispatch_scalar(
        &mut self,
        receiver: Identity,
        slot: u8,
        bits: u32,
    ) -> Result<bool> {
        let owner = self.mode_animator_mut(receiver)?;
        let Some(id) = owner.dispatch_script(slot)? else {
            return Ok(false);
        };
        owner.script_children.set_dispatch_coordinate(id, bits)?;
        Ok(true)
    }
}

impl Animator {
    pub(super) fn mirror_mode_layers(&mut self) {
        let Some(layers) = self.layers.as_ref() else {
            return;
        };
        let count = layers.active_count();
        let weights = std::array::from_fn::<_, 4, _>(|i| layers.slots()[i].weight_bits());
        let occupied =
            std::array::from_fn::<_, 4, _>(|i| !layers.slots()[i].nodes_head().is_empty());
        self.bytes[0x130] = Some(count);
        for (i, weight) in weights.into_iter().enumerate() {
            self.word_store(0x144 + i * 0x20, weight);
            if occupied[i] {
                self.invalidate_control_header(0x138 + i * 0x20);
            }
        }
    }
    pub(super) fn bind_empty_mode_layers(&mut self) -> Result<()> {
        if self.layers.is_some() {
            return Ok(());
        }
        ensure!(
            self.controls.is_empty() && self.script_children.is_empty(),
            "empty mode layers cannot replace retained node owners"
        );
        let mut flags = [0; 4];
        let mut weights = [0; 4];
        for (index, start) in [0x138, 0x158, 0x178, 0x198].into_iter().enumerate() {
            ensure!(
                self.word(start) == Some(0) && self.word(start + 4) == Some(0),
                "empty mode snapshot requires actual NULL list head and tail"
            );
            flags[index] = u16::from_be_bytes([
                self.byte(start + 8).context("mode slot flags unknown")?,
                self.byte(start + 9).context("mode slot flags unknown")?,
            ]);
            weights[index] = self.word(start + 12).context("mode slot weight unknown")?;
        }
        let snapshot = layers::Snapshot {
            active_count: self.byte(0x130).context("mode active count unknown")?,
            selection_mask: self.byte(0x131).context("mode selection mask unknown")?,
            clear_mask: self.byte(0x136).context("mode clear mask unknown")?,
            slot_flags: flags,
            weight_bits: weights,
        };
        self.layers = Some(layers::State::from_empty_snapshot(
            snapshot,
            layers::Budget {
                nodes: 4096,
                commands: 4096,
                groups: 4096,
                children: 16384,
            },
        )?);
        Ok(())
    }
}

#[cfg(test)]
#[path = "mode_target/tests.rs"]
mod tests;
