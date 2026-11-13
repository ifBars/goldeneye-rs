use crate::game::world_hooks::Shared;
use ge_core::mission::player::{identity::PlayerId, transform_cache::Slot};

#[test]
fn cache_headers_are_shared_across_attempts_and_stale_calls_cannot_change_them() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    old.with_player_transform_caches(|cache, _| {
        assert_eq!(cache.selected(), None);
        assert_eq!(cache.header(Slot::FirstPerson).count(), 0);
        assert_eq!(cache.header(Slot::FirstPerson).association(), None);
        // Actual absence of matrix backing prevents selection and association.
        assert_eq!(cache.select(0), None);
        assert!(!cache.bind_selected(Some(PlayerId::new(1)?), None)?);
        Ok(())
    })
    .unwrap();
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    assert!(
        old.with_player_transform_caches(|cache, _| {
            cache.select(1);
            Ok(())
        })
        .is_err()
    );
    current
        .with_player_transform_caches(|cache, _| {
            assert_eq!(cache.selected(), None);
            assert_eq!(cache.header(Slot::ThirdPerson).association(), None);
            assert_eq!(cache.invocation_count(), 0);
            Ok(())
        })
        .unwrap();
}
