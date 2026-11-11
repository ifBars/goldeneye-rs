use super::*;

#[test]
fn real_allocations_mint_distinct_handles_and_resolve_their_exact_player() {
    let mut registry = Registry::from_constructor(3);
    let player = PlayerId::new(7).unwrap();
    let first = registry.allocate(player).unwrap();
    let second = registry.allocate(player).unwrap();
    let third = registry.allocate(PlayerId::new(8).unwrap()).unwrap();
    assert_ne!(first.id(), second.id());
    assert_ne!(second.id(), third.id());
    assert_eq!(registry.resolve(first.id()), Some(first));
    assert_eq!(registry.resolve(second.id()).unwrap().player(), player);
    assert_eq!(registry.len(), 3);
    assert!(registry.allocate(player).is_err());
    assert_eq!(registry.len(), 3);
    assert!(registry.resolve(ComponentId::new(4).unwrap()).is_none());
}

#[test]
fn exhausted_or_overflowed_registry_never_mints_a_successful_handle() {
    let player = PlayerId::new(7).unwrap();
    let mut empty = Registry::from_constructor(0);
    assert!(empty.allocate(player).is_err());
    assert!(empty.is_empty());
    let mut overflow = Registry::from_constructor(1);
    overflow.counter = u64::MAX;
    assert!(overflow.allocate(player).is_err());
    assert!(overflow.is_empty());
}
