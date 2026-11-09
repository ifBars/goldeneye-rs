use std::{
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use ge_core::mission::Difficulty;

use super::{loading::Counts, *};

struct NonSendMission {
    _local: Rc<()>,
    dropped: Sender<bool>,
    owner: thread::ThreadId,
}

impl loading::Retained for NonSendMission {
    fn counts(&self) -> Counts {
        Counts {
            resources: 3,
            maps: 2,
            start_points: 4,
            world: None,
        }
    }
}

impl Drop for NonSendMission {
    fn drop(&mut self) {
        let _ = self.dropped.send(thread::current().id() == self.owner);
    }
}

fn request() -> NewGameRequest {
    NewGameRequest::new(Difficulty::Classic)
}

fn controller() -> Controller {
    Controller::new(PathBuf::from("authored-not-opened.rvz"))
}

fn start_fake(controller: &mut Controller) -> Receiver<bool> {
    let (dropped, observed) = mpsc::channel();
    controller
        .start_with(request(), |_, request, identity| {
            loading::spawn_with(request, identity, move |_| {
                Ok(NonSendMission {
                    _local: Rc::new(()),
                    dropped,
                    owner: thread::current().id(),
                })
            })
        })
        .unwrap();
    observed
}

fn until(controller: &mut Controller, stage: SessionStage) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        controller.poll().unwrap();
        if controller.snapshot().is_some_and(|s| s.stage == stage) {
            return;
        }
        assert!(Instant::now() < deadline, "worker did not reach {stage:?}");
        thread::yield_now();
    }
}

#[test]
fn idle_evidence_has_no_session_and_start_keeps_exact_request() {
    let mut controller = controller();
    assert!(!controller.is_active());
    assert_eq!(controller.evidence().requested, 0);
    assert!(controller.evidence().active.is_none());
    assert!(!controller.cancel());
    let dropped = start_fake(&mut controller);
    let snapshot = controller.snapshot().unwrap();
    assert_eq!(snapshot.stage, SessionStage::Admitting);
    assert_eq!(snapshot.difficulty, 3);
    assert_eq!(snapshot.level_hash, 0x0181_016b);
    assert_eq!(snapshot.start_point_id, 1);
    assert_eq!(
        Difficulty::try_from(snapshot.difficulty).unwrap().label(),
        "007 Classic"
    );
    assert!(!snapshot.session_active);
    assert_eq!(snapshot.source_create_counter, 8);
    assert!(controller.start(request()).is_err());
    assert_eq!(controller.evidence().requested, 1);
    controller.shutdown().unwrap();
    // Cancellation may occur before the fake resource is constructed.
    assert!(!matches!(dropped.try_recv(), Ok(false)));
}

#[test]
fn non_send_mission_is_retained_and_dropped_on_its_worker_after_shutdown() {
    let main = thread::current().id();
    let mut controller = controller();
    let dropped = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    assert!(dropped.try_recv().is_err(), "resources must remain owned");
    let snapshot = controller.snapshot().unwrap();
    assert_eq!(
        (
            snapshot.resources_retained,
            snapshot.map_instances,
            snapshot.source_start_points
        ),
        (3, 2, 4)
    );
    assert_eq!(
        (snapshot.mode, snapshot.backend, snapshot.local_users),
        (3, 2, 1)
    );
    assert!(snapshot.session_active && snapshot.created_session_selected);
    assert_eq!(snapshot.source_create_counter, 7);
    assert_eq!(
        (
            snapshot.manager_ready_flags_raw,
            snapshot.user_ready_flags_raw
        ),
        (0, 0)
    );
    assert!(!snapshot.world_registered && !snapshot.player_created && !snapshot.camera_ready);
    assert_eq!(controller.evidence().loaded, 1);
    assert!(!controller.evidence().native_provider_timing_verified);
    controller.shutdown().unwrap();
    assert!(dropped.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_eq!(thread::current().id(), main);
}

fn start_blocked(controller: &mut Controller) -> (Sender<()>, Receiver<()>) {
    let (release, wait) = mpsc::channel();
    let (entered, observe) = mpsc::channel();
    controller
        .start_with(request(), |_, request, identity| {
            loading::spawn_with(request, identity, move |_| {
                entered.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(3))?;
                let (dropped, _) = mpsc::channel();
                Ok(NonSendMission {
                    _local: Rc::new(()),
                    dropped,
                    owner: thread::current().id(),
                })
            })
        })
        .unwrap();
    observe.recv_timeout(Duration::from_secs(1)).unwrap();
    (release, observe)
}

#[test]
fn cancellation_does_not_wait_for_asset_load_and_next_attempt_is_independent() {
    let mut controller = controller();
    let (release, _) = start_blocked(&mut controller);
    assert!(controller.cancel());
    assert!(!controller.is_active());
    assert_eq!(controller.evidence().retired_workers, 1);
    assert_eq!(controller.evidence().cancelled, 1);
    let dropped = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    assert_eq!(controller.evidence().requested, 2);
    assert_eq!(controller.evidence().loaded, 1);
    release.send(()).unwrap();
    controller.shutdown().unwrap();
    assert!(dropped.recv_timeout(Duration::from_secs(1)).unwrap());
}

#[test]
fn bounded_retirement_refuses_more_workers_without_publishing_an_attempt() {
    let mut controller = controller();
    let (first, _) = start_blocked(&mut controller);
    controller.cancel();
    let (second, _) = start_blocked(&mut controller);
    controller.cancel();
    assert_eq!(controller.evidence().retired_workers, MAX_RETIRED);
    controller.start(request()).unwrap();
    assert_eq!(controller.evidence().requested, 3);
    assert_eq!(controller.snapshot().unwrap().stage, SessionStage::Failed);
    assert_eq!(
        controller.snapshot().unwrap().failure,
        Some(FailureCategory::Worker)
    );
    assert_eq!(controller.evidence().retired_workers, MAX_RETIRED);
    controller.cancel();
    first.send(()).unwrap();
    second.send(()).unwrap();
    controller.shutdown().unwrap();
    let dropped = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    controller.shutdown().unwrap();
    assert!(dropped.recv_timeout(Duration::from_secs(1)).unwrap());
}

#[test]
fn loader_error_has_generic_category_and_allows_cancel_then_restart() {
    let mut controller = controller();
    controller
        .start_with(request(), |_, request, identity| {
            loading::spawn_with(request, identity, |_| -> Result<NonSendMission> {
                anyhow::bail!("authored private C:/original/game/secret.edb")
            })
        })
        .unwrap();
    until(&mut controller, SessionStage::Failed);
    let snapshot = controller.snapshot().unwrap();
    assert_eq!(snapshot.failure, Some(FailureCategory::Loading));
    assert!(!snapshot.session_active);
    assert_eq!(snapshot.local_users, 0);
    assert_eq!(snapshot.resources_retained, 0);
    let public = serde_json::to_string(&controller.evidence()).unwrap();
    assert!(!public.contains("secret.edb") && !public.contains("C:/original"));
    assert_eq!(controller.evidence().failed, 1);
    controller.poll().unwrap();
    assert_eq!(controller.evidence().failed, 1);
    controller.cancel();
    let dropped = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    controller.shutdown().unwrap();
    assert!(dropped.recv_timeout(Duration::from_secs(1)).unwrap());
}

#[test]
fn invalid_semantic_identity_fails_admission_without_opening_assets() {
    let mut controller = controller();
    controller.next_identity = 0;
    controller
        .start_with(request(), |_, request, identity| {
            loading::spawn_with(request, identity, |_| -> Result<NonSendMission> {
                panic!("failed admission must not call loader")
            })
        })
        .unwrap();
    until(&mut controller, SessionStage::Failed);
    assert_eq!(
        controller.snapshot().unwrap().failure,
        Some(FailureCategory::Admission)
    );
    assert_eq!(controller.evidence().loaded, 0);
    controller.shutdown().unwrap();
}

#[test]
fn unexpected_worker_exit_is_generic_failure_and_never_ready() {
    let mut controller = controller();
    controller
        .start_with(request(), |_, _, _| {
            let (sender, events) = mpsc::sync_channel(1);
            let (cancel, _cancelled) = mpsc::sync_channel(1);
            let handle = thread::spawn(move || drop(sender));
            Ok(loading::Worker {
                events,
                cancel: Some(cancel),
                handle,
            })
        })
        .unwrap();
    until(&mut controller, SessionStage::Failed);
    assert_eq!(
        controller.snapshot().unwrap().failure,
        Some(FailureCategory::Worker)
    );
    assert_eq!(controller.evidence().loaded, 0);
    controller.shutdown().unwrap();
}

#[test]
fn spawn_failure_is_recoverable_and_does_not_replace_an_existing_attempt() {
    let mut controller = controller();
    controller
        .start_recovering(request(), |_, _, _| {
            anyhow::bail!("authored thread launch failure private/path")
        })
        .unwrap();
    assert_eq!(controller.snapshot().unwrap().stage, SessionStage::Failed);
    assert_eq!(
        controller.snapshot().unwrap().failure,
        Some(FailureCategory::Worker)
    );
    assert_eq!(
        (
            controller.evidence().requested,
            controller.evidence().failed
        ),
        (1, 1)
    );
    let before = controller.snapshot().cloned();
    assert!(controller.start(request()).is_err());
    assert_eq!(controller.snapshot(), before.as_ref());
    controller.cancel();
    let dropped = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    controller.shutdown().unwrap();
    assert!(dropped.recv_timeout(Duration::from_secs(1)).unwrap());
}

#[test]
fn retired_worker_panic_is_counted_without_failing_a_healthy_current_session() {
    let mut controller = controller();
    let dropped = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    let retired = thread::spawn(|| panic!("authored retired worker failure"));
    let deadline = Instant::now() + Duration::from_secs(1);
    while !retired.is_finished() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    controller.retired.push(retired);
    controller.poll().unwrap();
    assert_eq!(
        controller.snapshot().unwrap().stage,
        SessionStage::AwaitingWorld
    );
    assert_eq!(controller.evidence().retired_worker_failures, 1);
    assert_eq!(controller.evidence().failed, 0);
    controller.shutdown().unwrap();
    assert!(dropped.recv_timeout(Duration::from_secs(1)).unwrap());
}

struct FinalizingMission {
    retained: NonSendMission,
    entered: Sender<()>,
    resume: Option<Receiver<()>>,
}

#[test]
fn a_failed_ready_session_releases_its_published_readiness() {
    let mut controller = controller();
    let mut snapshot = Snapshot::admitting(request());
    snapshot.stage = SessionStage::AwaitingPlayer;
    snapshot.world_registered = true;
    snapshot.manager_ready_flags_raw = 0x108;
    snapshot.user_ready_flags_raw = 0x108;
    controller.snapshot = Some(snapshot);
    controller.fail(FailureCategory::Worker);
    let snapshot = controller.snapshot().unwrap();
    assert_eq!(snapshot.stage, SessionStage::Failed);
    assert!(!snapshot.world_registered && !snapshot.player_created && !snapshot.camera_ready);
    assert_eq!(snapshot.manager_ready_flags_raw, 0);
    assert_eq!(snapshot.user_ready_flags_raw, 0);
    assert!(snapshot.world_initialization.is_none());
    controller.shutdown().unwrap();
}
impl loading::Retained for FinalizingMission {
    fn counts(&self) -> Counts {
        self.retained.counts()
    }
    fn finish_session_load(
        &mut self,
        _: &mut ge_core::mission::session::Session,
        _: &ge_core::mission::session::LoadBinding,
    ) -> anyhow::Result<bool> {
        self.entered.send(())?;
        if let Some(resume) = &self.resume {
            resume.recv_timeout(Duration::from_secs(3))?;
            Ok(true)
        } else {
            anyhow::bail!("authored actual provider finalizer failure")
        }
    }
}

#[test]
fn finalizer_failure_releases_worker_owned_resources_without_loaded_event() {
    let mut controller = controller();
    let (dropped, observed) = mpsc::channel();
    let (entered, entry) = mpsc::channel();
    controller
        .start_with(request(), |_, request, identity| {
            loading::spawn_with(request, identity, move |_| {
                Ok(FinalizingMission {
                    retained: NonSendMission {
                        _local: Rc::new(()),
                        dropped,
                        owner: thread::current().id(),
                    },
                    entered,
                    resume: None,
                })
            })
        })
        .unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    until(&mut controller, SessionStage::Failed);
    assert_eq!(
        controller.snapshot().unwrap().failure,
        Some(FailureCategory::Loading)
    );
    assert_eq!(controller.evidence().loaded, 0);
    assert!(observed.recv_timeout(Duration::from_secs(1)).unwrap());
    controller.shutdown().unwrap();
}

#[test]
fn cancellation_during_finalization_does_not_publish_old_completion() {
    let mut controller = controller();
    let (dropped, observed) = mpsc::channel();
    let (entered, entry) = mpsc::channel();
    let (resume, wait) = mpsc::channel();
    controller
        .start_with(request(), |_, request, identity| {
            loading::spawn_with(request, identity, move |_| {
                Ok(FinalizingMission {
                    retained: NonSendMission {
                        _local: Rc::new(()),
                        dropped,
                        owner: thread::current().id(),
                    },
                    entered,
                    resume: Some(wait),
                })
            })
        })
        .unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(controller.cancel());
    let next = start_fake(&mut controller);
    until(&mut controller, SessionStage::AwaitingWorld);
    resume.send(()).unwrap();
    assert!(observed.recv_timeout(Duration::from_secs(1)).unwrap());
    controller.poll().unwrap();
    assert_eq!(controller.evidence().loaded, 1);
    assert!(!controller.snapshot().unwrap().world_registered);
    controller.shutdown().unwrap();
    assert!(next.recv_timeout(Duration::from_secs(1)).unwrap());
}
