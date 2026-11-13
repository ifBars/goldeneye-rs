//! Re-enter an initialized script through initflag=1. The factory's traces stay
//! retained; each later invocation owns its own cache/selection/position prefix.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AwaitingRate,
    AwaitingSelection,
    AwaitingPosition,
    Pending,
    Returned,
    Failed,
}
#[derive(Debug)]
pub struct Trace {
    pub ordinal: u64,
    pub stage: Stage,
    pub resource_checked: bool,
    pub body_bound: Option<bool>,
    pub rate: Option<rate::Trace>,
    pub selection: Option<initialization::Trace>,
    pub position: Option<position::Trace>,
    pub boundary: Option<super::Stage>,
}
impl Header {
    pub fn update_count(&self) -> u64 {
        self.update_count
    }
    pub fn updates(&self) -> &[Trace] {
        &self.updates
    }
}
impl Store {
    pub fn begin_initflag_update(&mut self, id: NodeId, source: &mut impl Provider) -> Result<()> {
        let h = self
            .headers
            .get_mut(&id)
            .context("script update owner absent")?;
        ensure!(
            h.initializer_returned() && h.updates.last().is_none_or(|t| t.stage == Stage::Returned),
            "script update precedes factory return or replays a pending invocation"
        );
        h.updates.try_reserve(1)?;
        let ordinal = h
            .update_count
            .checked_add(1)
            .context("script update ordinal overflow")?;
        let old = h.stage;
        let mut t = Trace {
            ordinal,
            stage: Stage::AwaitingRate,
            resource_checked: false,
            body_bound: None,
            rate: None,
            selection: None,
            position: None,
            boundary: None,
        };
        let result = h.resolve(source);
        t.resource_checked = result.is_ok();
        if result.is_ok() {
            t.body_bound = h.body_bound;
        }
        if result.is_err() {
            t.stage = Stage::Failed;
            t.boundary = Some(h.stage);
        } else if h.stage == super::Stage::ReturnedWithoutBody {
            t.stage = Stage::Returned;
        }
        h.stage = old;
        // Diagnostic history is bounded independently of simulation lifetime.
        // The factory traces and total invocation counter remain retained.
        if h.updates.len() == 8 {
            h.updates.remove(0);
        }
        h.update_count = ordinal;
        h.updates.push(t);
        result
    }
    pub fn prepare_update_rate(
        &mut self,
        id: NodeId,
        source: &mut impl position::Source,
        inputs: rate::Inputs,
    ) -> Result<()> {
        self.with_update(id, Stage::AwaitingRate, |h, t| {
            ensure!(t.rate.is_none(), "script update rate replays");
            t.rate = Some(rate::Trace::new(inputs));
            rate::evaluate(h, source, t.rate.as_mut().expect("retained rate"))?;
            if t.rate.as_ref().and_then(|r| r.rate_bits).is_some() {
                t.stage = Stage::AwaitingSelection;
            } else {
                t.stage = Stage::Pending;
                t.boundary = Some(h.stage);
            }
            Ok(())
        })
    }
    pub fn select_update(
        &mut self,
        id: NodeId,
        source: &mut impl initialization::Source,
        inputs: initialization::Inputs,
    ) -> Result<()> {
        self.with_update(id, Stage::AwaitingSelection, |h, t| {
            let rate = t
                .rate
                .as_ref()
                .and_then(|r| r.rate_bits)
                .context("script update rate has not returned")?;
            t.selection = Some(initialization::Trace::new(inputs));
            h.select_with_rate(
                source,
                t.selection.as_mut().expect("retained selection"),
                rate,
            )?;
            t.stage = match h.stage {
                super::Stage::AwaitingAnimation => Stage::AwaitingPosition,
                super::Stage::ReturnedWithoutPlay => Stage::Returned,
                _ => {
                    t.boundary = Some(h.stage);
                    Stage::Pending
                }
            };
            Ok(())
        })
    }
    pub fn prepare_update_position(
        &mut self,
        id: NodeId,
        source: &mut impl position::Source,
    ) -> Result<()> {
        self.with_update(id, Stage::AwaitingPosition, |h, t| {
            let s = t
                .selection
                .as_ref()
                .context("script update selection absent")?;
            let selection = position::Selection {
                offset: s.selected_offset.context("updated play absent")?,
                frame_bits: s.frame_bits.context("updated frame absent")?,
                start: s.selected_start.context("updated play start absent")?,
            };
            t.position = Some(position::Trace::new(0));
            h.prepare_selected_position(
                source,
                t.position.as_mut().expect("retained position"),
                Some(selection),
            )?;
            t.stage = if h.stage == super::Stage::ReturnedInitialized {
                Stage::Returned
            } else {
                t.boundary = Some(h.stage);
                Stage::Pending
            };
            Ok(())
        })
    }
    fn with_update(
        &mut self,
        id: NodeId,
        expected: Stage,
        run: impl FnOnce(&mut Header, &mut Trace) -> Result<()>,
    ) -> Result<()> {
        let h = self
            .headers
            .get_mut(&id)
            .context("script update owner absent")?;
        ensure!(
            h.updates.last().is_some_and(|t| t.stage == expected),
            "script update continuation order differs or replays"
        );
        let mut t = h.updates.pop().expect("validated invocation");
        let old = h.stage;
        h.stage = super::Stage::Initializing;
        let result = run(h, &mut t);
        if result.is_err() {
            t.stage = Stage::Failed;
            t.boundary = Some(h.stage);
        }
        h.stage = old;
        h.updates.push(t);
        result
    }
}
impl Trace {
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(1);
        emit((self.ordinal >> 32) as u32);
        emit(self.ordinal as u32);
        emit(self.stage as u32);
        emit(u32::from(self.resource_checked));
        emit(self.body_bound.map_or(0, |v| 1 + u32::from(v)));
        emit(self.boundary.map_or(0, |s| 1 + s as u32));
        emit(u32::from(self.rate.is_some()));
        if let Some(t) = &self.rate {
            t.visit_state_words(&mut *emit);
        }
        emit(u32::from(self.selection.is_some()));
        if let Some(t) = &self.selection {
            t.visit_state_words(&mut *emit);
        }
        emit(u32::from(self.position.is_some()));
        if let Some(t) = &self.position {
            t.visit_state_words(&mut *emit);
        }
    }
}

#[cfg(test)]
#[path = "update/tests.rs"]
mod tests;
