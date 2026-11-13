//! Initializer stores and the actual shared transform-cache calls following the
//! component helper. Animation preload and the final constructor stay pending.
use super::{PendingReset, Shared, component_tail};
use crate::mission::player::{
    base_handler,
    components::{Identity, Preparation},
    identity::PlayerId,
    item::Item,
    transform_cache::{self, Input, Owner, Snapshot, Trace},
};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    ItemFlags,
    ItemMode,
    ItemDirty,
    BaseScalars,
    FirstPerson,
    ThirdPerson,
    Caches,
    CacheFlag,
    Invalidate,
    AwaitingPreload,
    AwaitingTail,
}

pub struct Bindings<'a> {
    pub base: &'a mut base_handler::Owner,
    pub reset: &'a PendingReset,
    pub preparation: &'a Preparation,
    pub item: &'a mut Item,
}

#[derive(Debug)]
pub struct Pending {
    player: PlayerId,
    stage: Stage,
    flags: Option<(u16, u16)>,
    mode_written: bool,
    dirty_cleared: bool,
    scalars_written: bool,
    receivers: [Option<Identity>; 2],
    searches: usize,
    cache_trace: Option<Trace>,
    cache_snapshot: Option<Snapshot>,
    cache_enabled: Option<u8>,
    invalidated: bool,
}
impl Pending {
    pub fn begin(component: &component_tail::Pending) -> Result<Self> {
        ensure!(
            component.stage() == component_tail::Stage::Returned
                && component.helper_return() == Some(1),
            "initializer prefix precedes component helper success"
        );
        Ok(Self::new(component.player()))
    }
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            stage: Stage::ItemFlags,
            flags: None,
            mode_written: false,
            dirty_cleared: false,
            scalars_written: false,
            receivers: [None; 2],
            searches: 0,
            cache_trace: None,
            cache_snapshot: None,
            cache_enabled: None,
            invalidated: false,
        }
    }

    pub fn advance(
        &mut self,
        bindings: Bindings<'_>,
        caches: &mut Owner,
        shared: &Shared,
        budget: transform_cache::Budget,
    ) -> Result<()> {
        let Bindings {
            base,
            reset,
            preparation,
            item,
        } = bindings;
        ensure!(
            base.player() == self.player
                && reset.player() == self.player
                && base.item() == Some(item.id())
                && item.handler() == Some(self.player),
            "initializer prefix item/player ownership differs"
        );
        loop {
            match self.stage {
                Stage::ItemFlags => {
                    self.flags = Some(item.enable_initializer_transform_flags());
                    self.stage = Stage::ItemMode;
                }
                Stage::ItemMode => {
                    item.store_initializer_rotation_mode();
                    self.mode_written = true;
                    self.stage = Stage::ItemDirty;
                }
                Stage::ItemDirty => {
                    item.invalidate_constructor_pose()?;
                    self.dirty_cleared = true;
                    self.stage = Stage::BaseScalars;
                }
                Stage::BaseScalars => {
                    base.initialize_post_component_scalars()?;
                    self.scalars_written = true;
                    self.stage = Stage::FirstPerson;
                }
                Stage::FirstPerson => {
                    self.receivers[0] =
                        preparation.item_animator_by_file(item, self.player, 0x0102_0016)?;
                    self.searches = 1;
                    self.stage = Stage::ThirdPerson;
                }
                Stage::ThirdPerson => {
                    self.receivers[1] =
                        preparation.item_animator_by_file(item, self.player, 0x0102_0017)?;
                    self.searches = 2;
                    self.stage = if self.receivers.iter().all(Option::is_some) {
                        Stage::Caches
                    } else {
                        Stage::AwaitingTail
                    };
                }
                Stage::Caches => {
                    let fp = preparation.mode_animator(
                        self.receivers[0].context("initializer FP receiver absent")?,
                    )?;
                    let tp = preparation.mode_animator(
                        self.receivers[1].context("initializer TP receiver absent")?,
                    )?;
                    // Borrowing the retained skeleton owner does not evaluate a
                    // transform or grant access to unwritten matrix payloads.
                    let result = caches
                        .initialize_if_unselected(
                            Some(Input {
                                receiver: fp.identity(),
                                skeleton: fp.skeleton().map(|s| s.skeleton()),
                            }),
                            Some(Input {
                                receiver: tp.identity(),
                                skeleton: tp.skeleton().map(|s| s.skeleton()),
                            }),
                            budget,
                        )
                        .map(|_| ());
                    if let Some(trace) = caches.trace() {
                        self.cache_trace = Some(trace.clone());
                    }
                    self.cache_snapshot = Some(caches.snapshot());
                    result?;
                    if !self.cache_trace.as_ref().is_some_and(|t| t.returned) {
                        return Ok(());
                    }
                    self.stage = Stage::CacheFlag;
                }
                Stage::CacheFlag => {
                    let flag = shared.cache_enabled();
                    self.cache_enabled = Some(flag);
                    self.stage = if flag == 1 {
                        Stage::AwaitingPreload
                    } else {
                        Stage::Invalidate
                    };
                }
                Stage::Invalidate => {
                    caches.invalidate_selected()?;
                    self.invalidated = true;
                    self.cache_snapshot = Some(caches.snapshot());
                    self.stage = Stage::AwaitingTail;
                }
                Stage::AwaitingPreload | Stage::AwaitingTail => return Ok(()),
            }
        }
    }

    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn flags(&self) -> Option<(u16, u16)> {
        self.flags
    }
    pub fn mode_written(&self) -> bool {
        self.mode_written
    }
    pub fn dirty_cleared(&self) -> bool {
        self.dirty_cleared
    }
    pub fn scalars_written(&self) -> bool {
        self.scalars_written
    }
    pub fn receivers(&self) -> [Option<Identity>; 2] {
        self.receivers
    }
    pub fn searches(&self) -> usize {
        self.searches
    }
    pub fn cache_trace(&self) -> Option<&Trace> {
        self.cache_trace.as_ref()
    }
    pub fn cache_snapshot(&self) -> Option<Snapshot> {
        self.cache_snapshot
    }
    pub fn cache_enabled(&self) -> Option<u8> {
        self.cache_enabled
    }
    pub fn invalidated(&self) -> bool {
        self.invalidated
    }
    pub fn dependency(&self) -> &'static str {
        match self.stage {
            Stage::AwaitingPreload => "player_animation_preload",
            Stage::AwaitingTail => "player_initializer_tail",
            Stage::Caches => "player_transform_caches",
            _ => "player_initializer_prefix",
        }
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.stage as u32);
        emit(u32::from(self.flags.is_some()));
        if let Some((before, after)) = self.flags {
            emit(u32::from(before));
            emit(u32::from(after));
        }
        for v in [
            self.mode_written,
            self.dirty_cleared,
            self.scalars_written,
            self.invalidated,
        ] {
            emit(u32::from(v));
        }
        emit(self.searches as u32);
        for receiver in self.receivers {
            emit(u32::from(receiver.is_some()));
            if let Some(receiver) = receiver {
                receiver.visit_state_words(&mut emit);
            }
        }
        emit(u32::from(self.cache_trace.is_some()));
        if let Some(trace) = &self.cache_trace {
            trace.visit_state_words(&mut emit);
        }
        emit(u32::from(self.cache_snapshot.is_some()));
        if let Some(snapshot) = self.cache_snapshot {
            snapshot.visit_state_words(&mut emit);
        }
        emit(u32::from(self.cache_enabled.is_some()));
        if let Some(flag) = self.cache_enabled {
            emit(u32::from(flag));
        }
    }
}

#[cfg(test)]
mod tests;
