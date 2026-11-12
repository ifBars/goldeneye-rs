use super::*;
use crate::animation::controls::Store;
use anyhow::Context;

impl State {
    /// First caller's physical-slot capture helper. It visits all four slots
    /// while preserving the active count; the mask-command executor is separate.
    pub fn capture_fresh_slots(
        &mut self,
        controls: &mut Store,
        ids: &mut crate::animation::nodes::Ids,
        first: NodeId,
        mask: u8,
    ) -> Result<()> {
        ensure!(
            !self.failed && self.slots.iter().all(|slot| slot.nodes.is_empty()),
            "fresh control capture requires actual empty physical lists"
        );
        ensure!(
            mask == 0x0f,
            "first TP capture requires all four physical slots"
        );
        let control = controls
            .get(first)
            .context("captured control owner is absent")?;
        control.locals()?;
        control.auxiliary()?;
        let result = (|| {
            self.charge_capture_node()?;
            self.created.try_reserve(1)?;
            ensure!(
                self.identities.insert(first),
                "captured control identity was reused"
            );
            self.created.push(first);
            let mut inserted = false;
            for slot in 0..4 {
                if mask & (1 << slot) == 0 {
                    continue;
                }
                let id = if inserted {
                    self.charge_capture_node()?;
                    controls.clone_control(first, ids)?
                } else {
                    first
                };
                if inserted {
                    ensure!(
                        self.identities.insert(id),
                        "control clone identity was reused"
                    );
                    self.created.try_reserve(1)?;
                    self.created.push(id);
                }
                self.slots[slot]
                    .nodes
                    .try_reserve(1)
                    .context("retaining captured physical slot")?;
                self.slots[slot].nodes.insert(0, id);
                inserted = true;
            }
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn charge_capture_node(&mut self) -> Result<()> {
        let next = self
            .work
            .nodes
            .checked_add(1)
            .context("capture allocation work overflow")?;
        ensure!(
            next <= self.budget.nodes,
            "animation layer node factory budget exhausted"
        );
        self.work.nodes = next;
        Ok(())
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(u32::from(self.active_count));
        emit(u32::from(self.selection_mask));
        emit(u32::from(self.clear_mask));
        emit(u32::from(self.failed));
        for slot in &self.slots {
            emit(slot.weight_bits);
            emit(u32::from(slot.flags_raw));
            emit(slot.nodes.len() as u32);
            for id in &slot.nodes {
                emit((id.get() >> 32) as u32);
                emit(id.get() as u32);
            }
        }
    }
}
