use super::super::super::State;
use super::*;
use ge_core::animation::{
    layers::NodeId,
    scratch::evaluation::{Evaluation, Layers, Read, Stage, Step, Tail},
};

struct Waiting;
impl Layers for Waiting {
    fn skeleton_present(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn active_count(&mut self) -> Result<Read<u8>> {
        Ok(Read::Unknown)
    }
    fn weight_bits(&mut self, _: u8) -> Result<Read<u32>> {
        panic!("unreached weight")
    }
    fn tail(&mut self, _: u8) -> Result<Read<Tail>> {
        panic!("unreached tail")
    }
    fn prepare(&mut self, _: u8) -> Result<Step<()>> {
        panic!("unreached prepare")
    }
    fn invoke(&mut self, _: NodeId, _: u8, _: u32) -> Result<Step<u8>> {
        panic!("unreached invoke")
    }
    fn previous(&mut self, _: NodeId) -> Result<Read<Option<NodeId>>> {
        panic!("unreached previous")
    }
}
fn pending(shared: &Shared, attempt: u64) -> Evaluation {
    let mut state = shared.0.state.lock().unwrap();
    let State {
        animation_context,
        animation_scratch,
        ..
    } = &mut *state;
    let mut evaluation =
        Evaluation::begin(animation_context, animation_scratch, &mut Waiting, 8).unwrap();
    evaluation
        .advance(animation_context, animation_scratch, &mut Waiting)
        .unwrap();
    state
        .retain_animation_scope(attempt, evaluation.lease())
        .unwrap();
    evaluation
}

#[test]
fn new_attempt_reaps_before_old_worker_drops_and_old_abort_cannot_release_new_scope() {
    let shared = Shared::from_constructor();
    let first = shared.begin(1).unwrap();
    let mut old = pending(&shared, 1);
    shared.invalidate();
    let second = shared.begin(2).unwrap();
    assert!(first.ensure_current().is_err());
    second.ensure_current().unwrap();
    {
        let mut state = shared.0.state.lock().unwrap();
        state.reap_animation_scope(Some(2)).unwrap();
        assert_eq!(state.animation_scratch.live_reservations(), 0);
        assert!(state.animation_pending_evaluation.is_none());
    }
    // Retain the old evaluator, as an asynchronously retired worker may do.
    assert_eq!(old.stage(), Stage::Aborted);
    let mut new = pending(&shared, 2);
    let mut state = shared.0.state.lock().unwrap();
    let before = state.animation_scratch.cursor();
    let State {
        animation_context,
        animation_scratch,
        ..
    } = &mut *state;
    old.abort(animation_context, animation_scratch).unwrap();
    assert_eq!(animation_scratch.cursor(), before);
    assert_eq!(animation_scratch.live_reservations(), 1);
    assert!(
        old.advance(animation_context, animation_scratch, &mut Waiting)
            .is_err()
    );
    new.abort(animation_context, animation_scratch).unwrap();
    assert_eq!(animation_scratch.live_reservations(), 0);
}

#[test]
fn shutdown_cleanup_releases_pending_scope_without_another_scratch_operation() {
    let shared = Shared::from_constructor();
    shared.begin(1).unwrap();
    let old = pending(&shared, 1);
    shared.invalidate();
    shared.abort_pending_evaluation().unwrap();
    assert_eq!(old.stage(), Stage::Aborted);
    let state = shared.0.state.lock().unwrap();
    assert_eq!(state.animation_scratch.cursor(), 0);
    assert_eq!(state.animation_scratch.live_reservations(), 0);
    assert!(state.animation_pending_evaluation.is_none());
}

#[test]
fn current_attempt_pause_is_retained_and_duplicate_registration_requires_same_lease() {
    let shared = Shared::from_constructor();
    shared.begin(1).unwrap();
    let old = pending(&shared, 1);
    let mut state = shared.0.state.lock().unwrap();
    state.reap_animation_scope(Some(1)).unwrap();
    assert_eq!(state.animation_scratch.live_reservations(), 1);
    state.retain_animation_scope(1, old.lease()).unwrap();
    assert!(state.retain_animation_scope(2, old.lease()).is_err());
    state.reap_animation_scope(None).unwrap();
}
