//! Live mask-command continuation. Capturing the original control is an actual
//! component operation; list effects cannot run ahead of its backing return.
use super::*;
use crate::animation::{
    controls,
    nodes::{Ids, Kind},
    scratch::Refresh,
};

#[derive(Debug)]
pub struct MaskEffects {
    pub active_slots_before: Option<u8>,
    pub slots_visited: usize,
    pub selected_mask: Option<u16>,
    pub clear_mask: Option<u8>,
    pub controls_allocated: usize,
    pub controls_inserted: usize,
    pub capture_returned: bool,
    pub refresh: Option<Refresh>,
    pub first: Option<super::super::NodeId>,
    next_slot: usize,
    inserted: Vec<(u8, super::super::NodeId)>,
    destroy_pending: Option<super::super::NodeId>,
}
impl MaskEffects {
    fn new() -> Self {
        Self {
            active_slots_before: None,
            slots_visited: 0,
            selected_mask: None,
            clear_mask: None,
            controls_allocated: 0,
            controls_inserted: 0,
            capture_returned: false,
            refresh: None,
            first: None,
            next_slot: 0,
            inserted: Vec::new(),
            destroy_pending: None,
        }
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(emit: &mut impl FnMut(u32), value: usize) {
            emit(((value as u64) >> 32) as u32);
            emit(value as u32);
        }
        fn node(emit: &mut impl FnMut(u32), value: Option<super::super::NodeId>) {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit((value.get() >> 32) as u32);
                emit(value.get() as u32);
            }
        }
        emit(1);
        for value in [
            self.active_slots_before.map(u32::from),
            self.selected_mask.map(u32::from),
            self.clear_mask.map(u32::from),
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        for value in [
            self.slots_visited,
            self.controls_allocated,
            self.controls_inserted,
            self.next_slot,
        ] {
            wide(&mut emit, value);
        }
        emit(u32::from(self.capture_returned));
        emit(u32::from(self.refresh.is_some()));
        if let Some(refresh) = &self.refresh {
            refresh.visit_state_words(&mut emit);
        }
        node(&mut emit, self.first);
        node(&mut emit, self.destroy_pending);
        wide(&mut emit, self.inserted.len());
        for (slot, id) in &self.inserted {
            emit(u32::from(*slot));
            node(&mut emit, Some(*id));
        }
    }
}
impl Pending {
    pub fn mask_effects(&self) -> Option<&MaskEffects> {
        self.mask_effects.as_ref()
    }

    pub(crate) fn begin_mask_effects(
        &mut self,
        layers: &mut State,
        controls: &mut controls::Store,
        ids: &mut Ids,
    ) -> Result<Option<super::super::NodeId>> {
        ensure!(
            self.opcode == 0x0c00_0005
                && !layers.failed
                && matches!(
                    self.stage,
                    Stage::MaskControlPending | Stage::MaskClearPending
                )
                && self.mask_effects.is_none(),
            "mask effects precede their operand or replay"
        );
        let raw = self.mask.context("mask operand absent")?;
        ensure!(
            (raw == 0) == (self.stage == Stage::MaskClearPending),
            "mask allocation gate differs"
        );
        self.mask_effects = Some(MaskEffects::new());
        if raw == 0 {
            self.stage = Stage::MaskEffectsPending;
            return Ok(None);
        }
        let result = (|| {
            layers.charge_capture_node()?;
            let first = controls.allocate(ids)?;
            let effects = self.mask_effects.as_mut().expect("retained mask effects");
            effects.first = Some(first);
            effects.controls_allocated = 1;
            layers.register_node(first)?;
            self.stage = Stage::MaskRefreshPending;
            Ok(Some(first))
        })();
        if result.is_err() {
            self.fail_factory(layers);
        }
        result
    }

    pub(crate) fn record_mask_refresh(&mut self, refresh: Refresh) -> Result<()> {
        ensure!(
            self.stage == Stage::MaskRefreshPending,
            "mask refresh return differs or replays"
        );
        let effects = self.mask_effects.as_mut().context("mask effects absent")?;
        ensure!(
            effects.first.is_some() && effects.refresh.is_none(),
            "mask refresh owner differs or replays"
        );
        effects.refresh = Some(refresh);
        self.stage = Stage::MaskBackingPending;
        Ok(())
    }

    /// Called by the component bridge only after actual control capture returns.
    /// The list continuation independently verifies that backing is bound.
    pub(crate) fn mask_capture_returned(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::MaskBackingPending,
            "mask capture precedes refresh or replays"
        );
        let effects = self.mask_effects.as_mut().context("mask effects absent")?;
        ensure!(
            effects.refresh.is_some() && effects.first.is_some() && !effects.capture_returned,
            "mask capture return differs or replays"
        );
        effects.capture_returned = true;
        self.stage = Stage::MaskEffectsPending;
        Ok(())
    }

    pub(crate) fn finish_mask_effects(
        &mut self,
        layers: &mut State,
        controls: &mut controls::Store,
        ids: &mut Ids,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::MaskEffectsPending && !layers.failed,
            "mask list continuation precedes capture or replays"
        );
        let result = self.finish_mask_effects_inner(layers, controls, ids);
        if result.is_err() {
            self.fail_factory(layers);
        }
        result
    }
    fn finish_mask_effects_inner(
        &mut self,
        layers: &mut State,
        controls: &mut controls::Store,
        ids: &mut Ids,
    ) -> Result<()> {
        let raw = self.mask.context("mask operand absent")?;
        let effects = self.mask_effects.as_mut().context("mask effects absent")?;
        if raw != 0 {
            ensure!(
                effects.capture_returned && effects.refresh.is_some(),
                "mask backing has not returned"
            );
            let id = effects.first.context("mask control allocation absent")?;
            let control = controls.get(id).context("captured mask control absent")?;
            ensure!(
                control.component() == ids.owner() && ids.kind(id) == Some(Kind::Control),
                "mask control owner differs"
            );
            control.locals()?;
            control.auxiliary()?;
        }
        effects.selected_mask = Some(raw & u16::from(layers.selection_mask));
        effects.active_slots_before = Some(layers.active_count);
        // Re-read the current count after each reached insertion. High mask
        // bits alone never make an inactive physical slot participate.
        while effects.next_slot < usize::from(layers.active_count) {
            let slot = effects.next_slot;
            ensure!(slot < 4, "mask physical slot exceeds four");
            effects.slots_visited += 1;
            effects.clear_mask = Some(layers.clear_mask);
            if layers.clear_mask & (1 << slot) != 0
                && let Some(head) = layers.slots[slot].nodes.first().copied()
            {
                // The real list-clear body needs unlink/destructor providers.
                // Keep it unentered rather than treating live nodes as empty.
                effects.destroy_pending = Some(head);
                self.stage = Stage::MaskDestroyPending;
                return Ok(());
            }
            if effects.selected_mask.expect("read selection mask") & (1 << slot) != 0 {
                let first = effects.first.context("selected mask has no control")?;
                let id = if effects.controls_inserted == 0 {
                    first
                } else {
                    layers.charge_capture_node()?;
                    // Keep the fresh clone header/ID before copying backing;
                    // a later backing failure must not hide its allocation.
                    let id = controls.allocate(ids)?;
                    effects.controls_allocated += 1;
                    layers.register_node(id)?;
                    let original = controls.get(first).context("mask clone source absent")?;
                    let source_locals = original.locals()?;
                    let source_auxiliary = original.auxiliary()?;
                    let mut locals = Vec::new();
                    locals.try_reserve_exact(source_locals.len())?;
                    locals.extend_from_slice(source_locals);
                    let mut auxiliary = Vec::new();
                    auxiliary.try_reserve_exact(source_auxiliary.len())?;
                    auxiliary.extend_from_slice(source_auxiliary);
                    controls.capture(id, locals, auxiliary)?;
                    id
                };
                layers.slots[slot].nodes.try_reserve(1)?;
                effects.inserted.try_reserve(1)?;
                ids.link_head(slot as u8, id, layers.slots[slot].nodes.first().copied())?;
                layers.slots[slot].nodes.insert(0, id);
                layers.active_count = layers.active_count.max(slot as u8 + 1);
                effects.inserted.push((slot as u8, id));
                effects.controls_inserted += 1;
            }
            effects.next_slot += 1;
        }
        self.stage = Stage::Returned;
        Ok(())
    }
}

#[cfg(test)]
#[path = "mask/tests.rs"]
mod tests;
