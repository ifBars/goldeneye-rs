use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::Registry;

#[test]
fn catalog_ready_requires_actual_rebuild_even_for_empty_maps_and_resets_on_begin_map() {
    let mut empty = Registry::new(budget());
    empty.begin_map(map(1), 0, None).unwrap();
    finish(&mut empty);
    assert!(empty.is_complete());
    assert!(empty.catalog().is_empty());
    assert!(!empty.catalog_ready());
    empty.rebuild_catalog().unwrap();
    assert!(empty.catalog_ready());
    assert!(empty.catalog().is_empty());
    empty.begin_map(map(2), 0, None).unwrap();
    assert!(!empty.catalog_ready());
    finish(&mut empty);
    assert!(empty.is_complete());
    assert!(!empty.catalog_ready());
    empty.rebuild_catalog().unwrap();
    assert!(empty.catalog_ready());

    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    registry
        .visit(row(0), || Ok(common(7)), &mut FactoryProbe::default())
        .unwrap();
    finish(&mut registry);
    assert!(registry.is_complete());
    assert!(!registry.catalog_ready());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog_ready());
    assert_eq!(registry.catalog().len(), 1);
    registry.begin_map(map(2), 0, None).unwrap();
    assert!(!registry.catalog_ready());
    finish(&mut registry);
    assert!(registry.is_complete());
    assert!(!registry.catalog_ready());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog_ready());
    assert_eq!(registry.catalog().len(), 1);
}
