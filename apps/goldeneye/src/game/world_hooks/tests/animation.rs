use super::super::*;
use ge_core::mission::player::components::Preparation;

#[test]
fn animation_arena_and_context_persist_after_cancellation_without_stale_callback_writes() {
    let shared = Shared::from_constructor();
    let first = shared.begin(1).unwrap();
    {
        let mut state = shared.0.state.lock().unwrap();
        let State {
            animation_context,
            animation_scratch,
            ..
        } = &mut *state;
        animation_context.refresh_empty(animation_scratch).unwrap();
    }
    shared.invalidate();
    let mut unreached = Preparation::new();
    assert!(first.finish_tp_bind(&mut unreached).is_err());
    assert!(first.finish_fp_bind(&mut unreached).is_err());
    let second = shared.begin(2).unwrap();
    assert!(second.finish_tp_bind(&mut unreached).is_err());
    assert!(second.finish_fp_bind(&mut unreached).is_err());
    {
        let mut state = shared.0.state.lock().unwrap();
        assert_eq!(state.animation_scratch.cursor(), 0);
        assert_eq!(state.animation_scratch.live_reservations(), 0);
        assert_eq!(state.animation_context.flags(), Some(0));
        let State {
            animation_context,
            animation_scratch,
            ..
        } = &mut *state;
        assert_eq!(
            animation_context
                .refresh_empty(animation_scratch)
                .unwrap()
                .ordinal(),
            2
        );
    }
    assert_eq!(
        unreached.stage(),
        ge_core::mission::player::components::Stage::Fresh
    );
}

#[test]
fn actual_startup_callback_table_owner_is_shared_across_attempts_and_outlives_them() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    let first = old.animation_callbacks().unwrap();
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    assert!(old.animation_callbacks().is_err());
    let second = current.animation_callbacks().unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(first.handlers().len(), 4);
    drop(shared);
    drop(current);
    drop(old);
    first.noop(1).unwrap();
    second.noop(2).unwrap();
}
