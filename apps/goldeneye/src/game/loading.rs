//! CPU ownership never crosses the worker boundary. In particular GameAssets'
//! non-Send disc partition is created, retained and dropped on this thread.
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
};

use anyhow::{Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::mission::session::{NewGameRequest, OperationResult, Session, SessionId, UserId};

use super::{FailureCategory, SessionStage, Snapshot, WorldEvidence, world::OwnedWorld};
use crate::mission::MissionContext;

#[derive(Debug, Clone)]
pub(super) struct Counts {
    pub resources: usize,
    pub maps: usize,
    pub start_points: usize,
    pub world: Option<WorldEvidence>,
}

pub(super) trait Retained {
    fn counts(&self) -> Counts;
    fn finish_session_load(
        &mut self,
        _session: &mut Session,
        _binding: &ge_core::mission::session::LoadBinding,
    ) -> Result<bool> {
        Ok(false)
    }
}

struct LoadedMission {
    _assets: GameAssets,
    context: MissionContext,
    world: OwnedWorld,
}

/// Every exit path releases the admitted host session, including load failures
/// and cancellation during I/O. Native provider scheduling remains separate.
struct Admission(Session);
impl Drop for Admission {
    fn drop(&mut self) {
        let _ = self.0.teardown();
    }
}

impl Retained for LoadedMission {
    fn counts(&self) -> Counts {
        Counts {
            resources: self.context.retained_resources().len(),
            maps: self.context.retained_map_count(),
            start_points: self.context.prepared_start_point_count(),
            world: Some(self.world.evidence().clone()),
        }
    }
    fn finish_session_load(
        &mut self,
        session: &mut Session,
        binding: &ge_core::mission::session::LoadBinding,
    ) -> Result<bool> {
        self.world
            .finish_session_load(session, binding, &self.context, &mut self._assets)
    }
}

pub(super) enum Event {
    Loading(Snapshot),
    Loaded(Counts),
    Ready { manager_flags: u32, user_flags: u32 },
    Failed(FailureCategory),
}

pub(super) struct Worker {
    pub events: Receiver<Event>,
    pub cancel: Option<SyncSender<()>>,
    pub handle: JoinHandle<()>,
}

pub(super) fn spawn(
    path: PathBuf,
    request: NewGameRequest,
    identity: u64,
    resource_states: super::resource_states::Shared,
    world_hooks: super::world_hooks::Attempt,
) -> Result<Worker> {
    spawn_with(request, identity, move |request| {
        // The typed NewGame request corresponds to source handoff flag1.
        // Reset values before mission loading while retaining the shared owner.
        world_hooks.ensure_current()?;
        world_hooks.new_game_loader_handoff()?;
        resource_states.new_game_handoff()?;
        let attempt = ge_core::mission::loader::phase::AttemptId::new(identity)?;
        let mut work = ge_core::mission::loader::work::WorkOwner::new(
            attempt,
            ge_core::mission::loader::work::Budget { tickets: 128 },
        )?;
        work.enqueue(attempt, request.level_hash())?;
        let mut inputs = None;
        work.execute_next(attempt, |_| {
            let mut assets = GameAssets::open(&path, "Filelist.bin")?;
            ensure!(
                assets.disc.identity.game_id == "SJBE52" && assets.disc.identity.revision == 0,
                "game session requires SJBE52 revision zero"
            );
            let context = MissionContext::load_with_start_point(
                &mut assets,
                request.level_hash(),
                request.difficulty(),
                request.start_point_id(),
            )?;
            let data = assets
                .disc
                .supplied_data(ge_assets::supplied_data::LoadBudget {
                    max_dol_bytes: 16 * 1024 * 1024,
                    retained_data_bytes: 1024 * 1024,
                })?;
            inputs = Some((assets, context, data));
            Ok(())
        })?;
        let (mut assets, context, data) = inputs
            .ok_or_else(|| anyhow::anyhow!("mission input callback did not retain its owners"))?;
        let world = OwnedWorld::build(
            &context,
            data,
            resource_states,
            &mut assets,
            world_hooks,
            work,
        )?;
        Ok(LoadedMission {
            _assets: assets,
            context,
            world,
        })
    })
}

/// R deliberately has no Send bound: its value is constructed and destroyed
/// inside the spawned closure, and only Counts leave the thread.
pub(super) fn spawn_with<F, R>(request: NewGameRequest, identity: u64, load: F) -> Result<Worker>
where
    F: FnOnce(NewGameRequest) -> Result<R> + Send + 'static,
    R: Retained + 'static,
{
    let (events_tx, events) = mpsc::sync_channel(4);
    let (cancel, cancelled) = mpsc::sync_channel(1);
    let handle = thread::Builder::new()
        .name("goldeneye-game-session".to_owned())
        .spawn(move || {
            let admitted = admit(request, identity);
            let mut session = match admitted {
                Ok(session) => Admission(session),
                Err(_) => {
                    let _ = events_tx.send(Event::Failed(FailureCategory::Admission));
                    return;
                }
            };
            if stopped(&cancelled) {
                return;
            }
            let binding = match session.0.bind_loader(
                ge_core::mission::loader::phase::AttemptId::new(identity)
                    .expect("admission identity is nonzero"),
            ) {
                Ok(binding) => binding,
                Err(_) => {
                    let _ = events_tx.send(Event::Failed(FailureCategory::Admission));
                    return;
                }
            };
            let mut snapshot = Snapshot::admitting(request);
            let managers = session.0.managers().expect("admitted owned user managers");
            snapshot.stage = SessionStage::Loading;
            snapshot.session_active = true;
            snapshot.local_users = managers.local_user_count;
            snapshot.mode = session.0.mode();
            snapshot.backend = session.0.backend();
            snapshot.manager_ready_flags_raw = managers.readiness_flags_raw;
            snapshot.user_ready_flags_raw = managers.user.flags_raw;
            snapshot.source_create_counter = session.0.source_create_counter();
            snapshot.created_session_selected = session.0.active() == Some(session.0.identity());
            if events_tx.send(Event::Loading(snapshot)).is_err() {
                return;
            }
            let mut retained = match load(request) {
                Ok(retained) => retained,
                Err(error) => {
                    eprintln!("mission provider failed: {error:#}");
                    let _ = events_tx.send(Event::Failed(FailureCategory::Loading));
                    return;
                }
            };
            if stopped(&cancelled) {
                return;
            }
            let ready = match retained.finish_session_load(&mut session.0, &binding) {
                Ok(ready) => ready,
                Err(error) => {
                    eprintln!("mission finalizer failed: {error:#}");
                    let _ = events_tx.send(Event::Failed(FailureCategory::Loading));
                    return;
                }
            };
            if stopped(&cancelled) {
                return;
            }
            let counts = retained.counts();
            if events_tx.send(Event::Loaded(counts)).is_err() {
                return;
            }
            if ready {
                let managers = session
                    .0
                    .managers()
                    .expect("admitted manager owner remains retained");
                if events_tx
                    .send(Event::Ready {
                        manager_flags: managers.readiness_flags_raw,
                        user_flags: managers.user.flags_raw,
                    })
                    .is_err()
                {
                    return;
                }
            }
            // Hold the actual session, partition and mission until cancellation
            // or controller shutdown; no JSON reconstruction or detached owner.
            let _ = cancelled.recv();
            drop(retained);
            drop(session);
        })?;
    Ok(Worker {
        events,
        cancel: Some(cancel),
        handle,
    })
}

fn stopped(cancelled: &Receiver<()>) -> bool {
    !matches!(cancelled.try_recv(), Err(TryRecvError::Empty))
}

/// Explicit single-player host provider adaptation. Native operation scheduling,
/// external provider handles and account/profile history are not reproduced.
fn admit(request: NewGameRequest, identity: u64) -> Result<Session> {
    let id = SessionId::new(identity)?;
    let user = UserId::new(identity)?;
    let mut session = Session::begin(request, id, user);
    session.mode_result(OperationResult::Succeeded)?;
    session.begin_create()?;
    session.create_result(OperationResult::Succeeded)?;
    ensure!(
        session.select_pending(id)?,
        "owned session selection failed"
    );
    session.admission_result(OperationResult::Succeeded)?;
    ensure!(
        session.active() == Some(id),
        "owned session was not admitted"
    );
    Ok(session)
}
