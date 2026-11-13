use super::*;
use crate::animation::layers::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStage {
    AwaitingSetter,
    Scalars,
    Updating,
    Returned,
    Failed,
}
#[derive(Debug)]
pub struct ThirdPersonDispatch {
    pub(super) stage: DispatchStage,
    pub(super) captured_position_bits: u32,
    pub(super) setter: Option<SetterReturn>,
    scalar_visits: usize,
    scalar_writes: usize,
    updates_started: usize,
    updates_completed: usize,
    pending_slot: Option<u8>,
    pending_node: Option<NodeId>,
}
impl ThirdPersonDispatch {
    pub(super) fn new(captured_position_bits: u32) -> Self {
        Self {
            stage: DispatchStage::AwaitingSetter,
            captured_position_bits,
            setter: None,
            scalar_visits: 0,
            scalar_writes: 0,
            updates_started: 0,
            updates_completed: 0,
            pending_slot: None,
            pending_node: None,
        }
    }
    pub fn stage(&self) -> DispatchStage {
        self.stage
    }
    pub fn captured_position_bits(&self) -> u32 {
        self.captured_position_bits
    }
    pub fn setter(&self) -> Option<SetterReturn> {
        self.setter
    }
    pub fn scalar_visits(&self) -> usize {
        self.scalar_visits
    }
    pub fn scalar_writes(&self) -> usize {
        self.scalar_writes
    }
    pub fn updates_started(&self) -> usize {
        self.updates_started
    }
    pub fn updates_completed(&self) -> usize {
        self.updates_completed
    }
    pub fn pending_slot(&self) -> Option<u8> {
        self.pending_slot
    }
    pub fn pending_node(&self) -> Option<NodeId> {
        self.pending_node
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(1);
        emit(self.stage as u32);
        emit(self.captured_position_bits);
        emit(u32::from(self.setter.is_some()));
        if let Some(s) = self.setter {
            emit(s.return_word);
            emit(u32::from(s.history_updated));
            emit(s.primary_changed.map_or(0, |v| 1 + u32::from(v)));
        }
        for v in [
            self.scalar_visits,
            self.scalar_writes,
            self.updates_started,
            self.updates_completed,
        ] {
            emit(v as u32);
        }
        emit(self.pending_slot.map_or(0, |s| 1 + u32::from(s)));
        emit(u32::from(self.pending_node.is_some()));
        if let Some(id) = self.pending_node {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
        }
    }
}
impl Pending {
    pub fn third_person_dispatch(&self) -> Option<&ThirdPersonDispatch> {
        self.dispatch.as_ref()
    }
    pub fn begin_third_person_dispatch_tail(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonDispatchTail && reset.player() == self.player,
            "TP dispatch tail owner/order differs"
        );
        let d = self
            .dispatch
            .as_mut()
            .context("TP dispatch snapshot absent")?;
        ensure!(
            d.stage == DispatchStage::AwaitingSetter && d.setter.is_some(),
            "TP dispatch scalars precede setter or replay"
        );
        d.stage = DispatchStage::Scalars;
        let result =
            (|| {
                for (slot, at) in [(0, 0x9a0), (2, 0x9a4), (3, 0x9a4)] {
                    let bits = reset.word(at).context("TP dispatch blend input unknown")?;
                    d.scalar_visits += 1;
                    d.scalar_writes += usize::from(
                        preparation.write_third_person_dispatch_scalar(self.player, slot, bits)?,
                    );
                }
                d.stage = DispatchStage::Updating;
                d.pending_slot = Some(2);
                Ok(())
            })();
        if result.is_err() {
            d.stage = DispatchStage::Failed;
            self.stage = Stage::ThirdPersonDispatchFailed;
        }
        result
    }
    pub fn begin_third_person_dispatch_update(
        &mut self,
        preparation: &mut Preparation,
    ) -> Result<Option<NodeId>> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonDispatchTail,
            "TP dispatch update order differs"
        );
        let d = self.dispatch.as_mut().context("TP dispatch absent")?;
        ensure!(
            d.stage == DispatchStage::Updating
                && d.pending_node.is_none()
                && d.updates_started == d.updates_completed,
            "TP dispatch update replays"
        );
        let slot = d.pending_slot.context("TP dispatch update slot absent")?;
        let result = preparation.begin_third_person_dispatch_update(
            self.player,
            slot,
            d.captured_position_bits,
        );
        match result {
            Ok(node) => {
                d.pending_node = node;
                d.updates_started += 1;
                Ok(node)
            }
            Err(e) => {
                d.stage = DispatchStage::Failed;
                self.stage = Stage::ThirdPersonDispatchFailed;
                Err(e)
            }
        }
    }
    pub fn complete_third_person_dispatch_update(
        &mut self,
        preparation: &Preparation,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonDispatchTail,
            "TP dispatch return order differs"
        );
        let d = self.dispatch.as_mut().context("TP dispatch absent")?;
        ensure!(
            d.stage == DispatchStage::Updating && d.updates_started == d.updates_completed + 1,
            "TP dispatch update has not started"
        );
        if let Some(id) = d.pending_node {
            preparation.ensure_third_person_dispatch_update_returned(self.player, id)?;
        }
        d.updates_completed += 1;
        d.pending_node = None;
        if d.pending_slot == Some(2) {
            d.pending_slot = Some(3);
        } else {
            ensure!(d.pending_slot == Some(3), "TP dispatch update slot differs");
            d.pending_slot = None;
            d.stage = DispatchStage::Returned;
            self.stage = Stage::AwaitingFirstPersonDispatch;
        }
        Ok(())
    }
}
