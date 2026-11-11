use super::*;

#[test]
fn backing_and_requests_are_aligned_and_exact_capacity_is_rejected_without_mutation() {
    let mut arena = Arena::from_constructor(7).unwrap();
    assert_eq!(arena.backing.0.as_ptr() as usize % 16, 0);
    assert!(arena.reserve(ARENA_BYTES).unwrap().is_none());
    assert_eq!(arena.cursor(), 0);
    let mut first = arena.reserve(1).unwrap().unwrap();
    assert_eq!(arena.cursor(), 16);
    assert!(arena.reserve(ARENA_BYTES - 16).unwrap().is_none());
    assert!(arena.reserve(usize::MAX).is_err());
    assert_eq!(arena.cursor(), 16);
    arena.release(&mut first).unwrap();
    assert_eq!(arena.cursor(), 0);
    assert_eq!(arena.live_reservations(), 0);
    assert!(Arena::from_constructor(0).is_err());
}

#[test]
fn released_regions_cannot_revive_when_a_new_reservation_reuses_the_same_offset() {
    let mut arena = Arena::from_constructor(7).unwrap();
    let mut outer = arena.reserve(16).unwrap().unwrap();
    let mut inner = arena.reserve(16).unwrap().unwrap();
    let region = arena.region(&inner, 0, 16).unwrap();
    assert!(arena.region_is_live(region));
    assert!(arena.region(&inner, 1, 16).is_err());
    assert!(arena.release(&mut outer).is_err());
    let mut other = Arena::from_constructor(8).unwrap();
    assert!(!other.region_is_live(region));
    assert!(other.release(&mut inner).is_err());
    arena.release(&mut inner).unwrap();
    assert!(!arena.region_is_live(region));
    assert!(arena.region(&inner, 0, 1).is_err());
    let mut replacement = arena.reserve(16).unwrap().unwrap();
    assert!(!arena.region_is_live(region));
    arena.release(&mut replacement).unwrap();
    arena.release(&mut outer).unwrap();
    arena.release(&mut outer).unwrap();
    assert_eq!(arena.cursor(), 0);
    assert_eq!((arena.allocations, arena.releases), (3, 3));
}

#[test]
fn empty_refresh_changes_actual_context_but_preserves_prior_scopes_and_releases_its_own() {
    let mut arena = Arena::from_constructor(7).unwrap();
    let mut prefix = arena.reserve(17).unwrap().unwrap();
    let mut context = Context {
        flags: Some(u32::MAX),
        ..Context::default()
    };
    let receipt = context.refresh_empty(&mut arena).unwrap();
    assert_eq!(receipt.reservation_start(), 32);
    assert_eq!(receipt.cursor_after(), 32);
    assert_eq!(receipt.live_after(), 1);
    assert_eq!(context.flags(), Some(0));
    assert_eq!(arena.cursor(), 32);
    assert_eq!(arena.live_reservations(), 1);
    for (region, offset) in context.regions().iter().zip([0, 0x1000, 0x2000, 0x2100]) {
        let region = region.unwrap();
        assert_eq!(region.start, 32 + offset);
        assert!(!arena.region_is_live(region));
    }
    let next = context.refresh_empty(&mut arena).unwrap();
    assert_eq!(next.ordinal(), 2);
    assert_eq!(next.allocation_ordinal(), 3);
    arena.release(&mut prefix).unwrap();
    assert_eq!((arena.allocations, arena.releases), (3, 3));
}

#[test]
fn exhaustion_preserves_previous_context_flags_regions_and_allocator_state() {
    let mut arena = Arena::from_constructor(7).unwrap();
    let mut context = Context::default();
    let mut prefix = arena.reserve(0x1e00).unwrap().unwrap();
    assert!(context.refresh_empty(&mut arena).is_err());
    assert_eq!(context.flags(), None);
    assert_eq!(context.regions(), &[None; 4]);
    assert_eq!(context.refreshes, 0);
    assert_eq!((arena.allocations, arena.releases), (1, 0));
    assert_eq!(arena.cursor(), 0x1e00);
    arena.release(&mut prefix).unwrap();
    context.refresh_empty(&mut arena).unwrap();
    assert_eq!(context.refreshes, 1);
}
