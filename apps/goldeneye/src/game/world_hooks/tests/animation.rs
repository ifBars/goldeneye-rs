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
    let second = shared.begin(2).unwrap();
    assert!(second.finish_tp_bind(&mut unreached).is_err());
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
