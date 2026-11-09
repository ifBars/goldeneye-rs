//! Owned single-player CPU session controller for the existing native window.
//! All non-Send source ownership remains in its worker; only typed state/counts
//! reach the window. Retained map data never promotes world/player readiness.
mod actors;
mod evidence;
mod loading;
mod player;
mod resource_states;
pub mod view;
mod world;
mod world_hooks;

use std::{path::PathBuf, sync::mpsc::TryRecvError, thread::JoinHandle};

use anyhow::{Result, ensure};
use ge_core::mission::session::NewGameRequest;

pub use actors::Evidence as ActorDefinitionEvidence;
pub use evidence::{Evidence, FailureCategory, SessionStage, Snapshot};
pub use player::Evidence as PlayerInitializationEvidence;
pub use resource_states::Evidence as ResourceStateEvidence;
pub use world::Evidence as WorldEvidence;
pub use world::NavigationEvidence;
pub use world::PostTriggerResourceEvidence;
pub use world_hooks::Evidence as WorldHookEvidence;
pub use world_hooks::loader::Evidence as LoaderEvidence;

/// Host budget, not an original provider/thread count. A new attempt is refused
/// until an old non-interruptible asset read exits when this budget is full.
const MAX_RETIRED: usize = 2;

pub struct Controller {
    path: PathBuf,
    resource_states: resource_states::Shared,
    world_hooks: world_hooks::Shared,
    current: Option<loading::Worker>,
    retired: Vec<JoinHandle<()>>,
    snapshot: Option<Snapshot>,
    next_identity: u64,
    requested: u64,
    loaded: u64,
    cancelled: u64,
    failed: u64,
    retired_worker_failures: u64,
}

impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            resource_states: resource_states::Shared::from_constructor(),
            world_hooks: world_hooks::Shared::from_constructor(),
            current: None,
            retired: Vec::new(),
            snapshot: None,
            next_identity: 1,
            requested: 0,
            loaded: 0,
            cancelled: 0,
            failed: 0,
            retired_worker_failures: 0,
        }
    }

    pub fn start(&mut self, request: NewGameRequest) -> Result<()> {
        let resource_states = self.resource_states.clone();
        let world_hooks = self.world_hooks.clone();
        self.start_recovering(request, move |path, request, identity| {
            let attempt = world_hooks.begin(identity)?;
            let result = loading::spawn(path, request, identity, resource_states, attempt);
            if result.is_err() {
                world_hooks.invalidate();
            }
            result
        })
    }

    pub fn advance_frame_clock(&self, elapsed: std::time::Duration) -> Result<()> {
        self.world_hooks.advance(elapsed)
    }

    fn start_recovering(
        &mut self,
        request: NewGameRequest,
        spawn: impl FnOnce(PathBuf, NewGameRequest, u64) -> Result<loading::Worker>,
    ) -> Result<()> {
        ensure!(
            !self.is_active(),
            "a game session attempt is already active"
        );
        if self.start_with(request, spawn).is_err() {
            self.requested += 1;
            self.snapshot = Some(Snapshot::admitting(request));
            self.fail(FailureCategory::Worker);
        }
        Ok(())
    }

    fn start_with(
        &mut self,
        request: NewGameRequest,
        spawn: impl FnOnce(PathBuf, NewGameRequest, u64) -> Result<loading::Worker>,
    ) -> Result<()> {
        ensure!(
            !self.is_active(),
            "a game session attempt is already active"
        );
        self.reap_finished();
        ensure!(
            self.retired.len() < MAX_RETIRED,
            "previous session workers are still stopping"
        );
        let next = self
            .next_identity
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("session identity budget exhausted"))?;
        let worker = spawn(self.path.clone(), request, self.next_identity)?;
        self.next_identity = next;
        self.current = Some(worker);
        self.snapshot = Some(Snapshot::admitting(request));
        self.requested += 1;
        Ok(())
    }

    pub fn poll(&mut self) -> Result<()> {
        self.reap_finished();
        // The fixed protocol sends at most three events; this is additionally
        // bounded so no event callback can become an unbounded drain loop.
        for _ in 0..4 {
            let result = match self.current.as_ref() {
                Some(worker) => worker.events.try_recv(),
                None => break,
            };
            match result {
                Ok(loading::Event::Loading(snapshot)) => {
                    if self
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.stage == SessionStage::Admitting)
                    {
                        self.snapshot = Some(snapshot);
                    } else {
                        self.fail(FailureCategory::Worker);
                    }
                }
                Ok(loading::Event::Loaded(counts)) => {
                    if let Some(snapshot) = self
                        .snapshot
                        .as_mut()
                        .filter(|s| s.stage == SessionStage::Loading)
                    {
                        snapshot.stage = SessionStage::AwaitingWorld;
                        snapshot.resources_retained = counts.resources;
                        snapshot.map_instances = counts.maps;
                        snapshot.source_start_points = counts.start_points;
                        snapshot.world_initialization = counts.world;
                        self.loaded += 1;
                    } else {
                        self.fail(FailureCategory::Worker);
                    }
                }
                Ok(loading::Event::Ready {
                    manager_flags,
                    user_flags,
                }) => {
                    if let Some(snapshot) = self
                        .snapshot
                        .as_mut()
                        .filter(|snapshot| snapshot.stage == SessionStage::AwaitingWorld)
                    {
                        ensure!(
                            manager_flags & 0x100 != 0 && user_flags & 0x100 != 0,
                            "world readiness did not reach the admitted user"
                        );
                        snapshot.stage = SessionStage::AwaitingPlayer;
                        snapshot.world_registered = true;
                        snapshot.manager_ready_flags_raw = manager_flags;
                        snapshot.user_ready_flags_raw = user_flags;
                    } else {
                        self.fail(FailureCategory::Worker);
                    }
                }
                Ok(loading::Event::Failed(category)) => self.fail(category),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.stage == SessionStage::Failed)
                    {
                        self.fail(FailureCategory::Worker);
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    fn fail(&mut self, category: FailureCategory) {
        self.world_hooks.invalidate();
        if let Some(snapshot) = self.snapshot.as_mut() {
            if snapshot.stage != SessionStage::Failed {
                self.failed += 1;
            }
            snapshot.stage = SessionStage::Failed;
            snapshot.session_active = false;
            snapshot.failure = Some(category);
            snapshot.local_users = 0;
            snapshot.mode = 0;
            snapshot.backend = 0;
            snapshot.created_session_selected = false;
            snapshot.resources_retained = 0;
            snapshot.map_instances = 0;
            snapshot.source_start_points = 0;
            snapshot.world_initialization = None;
            snapshot.world_registered = false;
            snapshot.player_created = false;
            snapshot.camera_ready = false;
            snapshot.manager_ready_flags_raw = 0;
            snapshot.user_ready_flags_raw = 0;
        }
    }

    /// Signal/drop channels and retire the handle only. No join, disc read or
    /// wait runs in this window-event callback.
    pub fn cancel(&mut self) -> bool {
        if self.snapshot.take().is_none() {
            return false;
        }
        self.world_hooks.invalidate();
        self.cancelled += 1;
        if let Some(mut worker) = self.current.take() {
            worker.cancel.take();
            self.retired.push(worker.handle);
        }
        true
    }

    pub fn is_active(&self) -> bool {
        self.snapshot.is_some()
    }

    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    pub fn evidence(&self) -> Evidence {
        Evidence {
            requested: self.requested,
            loaded: self.loaded,
            cancelled: self.cancelled,
            failed: self.failed,
            retired_workers: self.retired.len(),
            retired_worker_failures: self.retired_worker_failures,
            active: self.snapshot.clone(),
            provider: "host_local_semantic",
            native_provider_timing_verified: false,
            scope: "Owned fresh one-user session admission, retained CPU mission resources and partial live trigger initialization. Original ordinal slots, supported StartPoint/generic/common-BE objects and required fresh base animators remain on the session worker. Inactive membership and active objects are reported separately. Unsupported classes/references block catalog completion and readiness. Mode3/backend2 and semantic session selection are retained; native provider/event scheduling, host zone arithmetic parity and profile history are unverified. AwaitingWorld is not completed world/catalog initialization, player creation, a camera or gameplay readiness.",
        }
    }

    fn reap_finished(&mut self) {
        let mut index = 0;
        while index < self.retired.len() {
            if self.retired[index].is_finished() {
                let handle = self.retired.swap_remove(index);
                if handle.join().is_err() {
                    self.retired_worker_failures += 1;
                }
            } else {
                index += 1;
            }
        }
    }

    /// Called only after the native event loop exits. Asset reads may finish
    /// before their workers observe cancellation, so this is the join boundary.
    pub fn shutdown(&mut self) -> Result<()> {
        self.cancel();
        let mut panicked = false;
        for handle in std::mem::take(&mut self.retired) {
            panicked |= handle.join().is_err();
        }
        ensure!(!panicked, "session worker failed during shutdown");
        Ok(())
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        // Best-effort cancellation if an earlier host error bypassed shutdown.
        // Joining remains explicit, never hidden in a window callback/drop.
        self.current.take();
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
