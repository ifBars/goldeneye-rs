use super::{PlayerId, Preparation, Stage};
use crate::animation::scratch::{Arena, Context as RefreshContext, Refresh};
use anyhow::{Context, Result, ensure};

/// Actual native bind return, after the shared zero-time callback and release.
/// This receipt does not attach a component or complete PlayerNew.
#[derive(Debug)]
pub struct BindReturn {
    player: PlayerId,
    refresh: Refresh,
}
impl Preparation {
    pub fn bind_return(&self) -> Option<&BindReturn> {
        self.bind_return.as_ref()
    }
    pub fn finish_bind_zero_time(
        &mut self,
        context: &mut RefreshContext,
        arena: &mut Arena,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingAnimationRefresh,
            "TP bind refresh is out of source order"
        );
        let animator = self
            .animator
            .as_ref()
            .context("TP refresh animator is absent")?;
        ensure!(
            self.player == Some(animator.player()),
            "TP bind player identity differs"
        );
        self.stage = Stage::Refreshing;
        match animator.refresh_empty_bind(context, arena) {
            Ok(refresh) => {
                self.bind_return = Some(BindReturn {
                    player: animator.player(),
                    refresh,
                });
                self.stage = Stage::AwaitingAttachment;
                Ok(())
            }
            Err(error) => {
                self.stage = Stage::Failed;
                Err(error)
            }
        }
    }
}
impl BindReturn {
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn refresh(&self) -> &Refresh {
        &self.refresh
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        self.refresh.visit_state_words(emit);
    }
}
