use super::{budget, instance, nav, placement, wrapper};
use crate::navigation::runtime::{Entity, MapView, PlacementLocation, resolve_binding};

#[test]
fn max_is_a_hash_and_global_capture_keeps_level_entity_owner_and_raw_bits() {
    let first = [placement(
        u32::MAX,
        33,
        [0x8000_0000, 0x7fc0_1234, 0x3f80_0000],
    )];
    let source = [placement(u32::MAX, 7, [1, 2, 3])];
    let maps = [
        MapView {
            instance: instance(1),
            wrapper: wrapper(11),
            placements: &first,
        },
        MapView {
            instance: instance(2),
            wrapper: wrapper(22),
            placements: &source,
        },
    ];
    let mut calls = Vec::new();
    let bound = resolve_binding(
        wrapper(99),
        PlacementLocation {
            instance: instance(2),
            index: 0,
        },
        &maps,
        |owner, raw| {
            calls.push((owner.get(), raw));
            Ok(Some(nav(if raw == 7 { 41 } else { 67 })))
        },
        budget(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(calls, [(99, 7), (99, 33)]);
    assert_eq!(bound.source.instance, instance(2));
    assert_eq!(
        bound.found,
        PlacementLocation {
            instance: instance(1),
            index: 0
        }
    );
    assert_eq!(bound.source_entity_directory_index, 41);
    assert_eq!(bound.entity_directory_index, 67);
    assert_eq!(bound.cached_entity_reference_raw, 33);
    assert_eq!(bound.key.raw(), u32::MAX);
    assert_eq!(
        bound.translation_bits,
        [0x8000_0000, 0x7fc0_1234, 0x3f80_0000, 0]
    );
}

#[test]
fn zero_top_byte_selects_index_in_first_map_that_contains_it_not_matching_hash() {
    let first = [
        placement(0x1100_0000, 5, [0; 3]),
        placement(0x2200_0000, 6, [6; 3]),
    ];
    let source = [placement(1, 9, [9; 3])];
    let maps = [
        MapView {
            instance: instance(1),
            wrapper: wrapper(1),
            placements: &first,
        },
        MapView {
            instance: instance(2),
            wrapper: wrapper(2),
            placements: &source,
        },
    ];
    let mut calls = Vec::new();
    let result = resolve_binding(
        wrapper(2),
        PlacementLocation {
            instance: instance(2),
            index: 0,
        },
        &maps,
        |_, raw| {
            calls.push(raw);
            Ok(Some(nav(raw as usize)))
        },
        budget(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(calls, [9, 6]);
    assert_eq!(
        result.found,
        PlacementLocation {
            instance: instance(1),
            index: 1
        }
    );
    assert_eq!(result.translation_bits, [6, 6, 6, 0]);
}

#[test]
fn high_bit_key_is_first_hash_match_and_does_not_select_low_index() {
    let rows = [
        placement(0x8000_0001, 11, [1; 3]),
        placement(0x8000_0001, 12, [2; 3]),
    ];
    let maps = [MapView {
        instance: instance(1),
        wrapper: wrapper(1),
        placements: &rows,
    }];
    let bound = resolve_binding(
        wrapper(1),
        PlacementLocation {
            instance: instance(1),
            index: 1,
        },
        &maps,
        |_, raw| Ok(Some(nav(raw as usize))),
        budget(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(bound.found.index, 0);
    assert_eq!(bound.source_entity_reference_raw, 12);
    assert_eq!(bound.cached_entity_reference_raw, 11);
}

#[test]
fn missing_or_wrong_initial_type_skips_without_required_lookup() {
    let rows = [placement(65_535, 1, [0; 3])];
    let maps = [MapView {
        instance: instance(1),
        wrapper: wrapper(1),
        placements: &rows,
    }];
    for initial in [
        None,
        Some(Entity {
            directory_index: 0,
            entity_type: 0x601,
        }),
    ] {
        let mut calls = 0;
        let result = resolve_binding(
            wrapper(1),
            PlacementLocation {
                instance: instance(1),
                index: 0,
            },
            &maps,
            |_, _| {
                calls += 1;
                Ok(initial)
            },
            budget(),
        )
        .unwrap();
        assert!(result.is_none());
        assert_eq!(calls, 1);
    }
}

#[test]
fn required_first_global_match_does_not_skip_wrong_type_for_a_later_navmesh() {
    let first = [placement(0x1234_5678, 1, [0; 3])];
    let source = [placement(0x1234_5678, 2, [0; 3])];
    let maps = [
        MapView {
            instance: instance(1),
            wrapper: wrapper(1),
            placements: &first,
        },
        MapView {
            instance: instance(2),
            wrapper: wrapper(2),
            placements: &source,
        },
    ];
    let mut calls = Vec::new();
    let error = resolve_binding(
        wrapper(2),
        PlacementLocation {
            instance: instance(2),
            index: 0,
        },
        &maps,
        |_, raw| {
            calls.push(raw);
            Ok(Some(Entity {
                directory_index: raw as usize,
                entity_type: if raw == 2 { 0x607 } else { 0x601 },
            }))
        },
        budget(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("not NavMesh"));
    assert_eq!(calls, [2, 1]);
}

#[test]
fn eligible_missing_placement_or_entity_and_provider_error_are_errors_not_empty_owners() {
    let rows = [placement(65_535, 1, [0; 3])];
    let maps = [MapView {
        instance: instance(1),
        wrapper: wrapper(1),
        placements: &rows,
    }];
    let source = PlacementLocation {
        instance: instance(1),
        index: 0,
    };
    assert!(resolve_binding(wrapper(1), source, &maps, |_, _| Ok(Some(nav(0))), budget()).is_err());
    let rows = [placement(0x1234_5678, 1, [0; 3])];
    let maps = [MapView {
        instance: instance(1),
        wrapper: wrapper(1),
        placements: &rows,
    }];
    let mut calls = 0;
    let result = resolve_binding(
        wrapper(1),
        source,
        &maps,
        |_, _| {
            calls += 1;
            Ok(if calls == 1 { Some(nav(0)) } else { None })
        },
        budget(),
    );
    assert!(result.is_err());
    assert_eq!(calls, 2);
    let error = resolve_binding(
        wrapper(1),
        source,
        &maps,
        |_, _| anyhow::bail!("checked descriptor corrupt"),
        budget(),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "checked descriptor corrupt");
}

#[test]
fn map_and_placement_budgets_and_duplicate_instance_validation_precede_provider() {
    let rows = [placement(0, 1, [0; 3])];
    let maps = [MapView {
        instance: instance(1),
        wrapper: wrapper(1),
        placements: &rows,
    }];
    for limited in [
        super::super::Budget {
            maps: 0,
            ..budget()
        },
        super::super::Budget {
            placements: 0,
            ..budget()
        },
    ] {
        let result = resolve_binding(
            wrapper(1),
            PlacementLocation {
                instance: instance(1),
                index: 0,
            },
            &maps,
            |_, _| panic!("provider must remain untouched"),
            limited,
        );
        assert!(result.is_err());
    }
    let duplicate = [
        MapView {
            instance: instance(1),
            wrapper: wrapper(1),
            placements: &rows,
        },
        MapView {
            instance: instance(1),
            wrapper: wrapper(2),
            placements: &rows,
        },
    ];
    assert!(
        resolve_binding(
            wrapper(1),
            PlacementLocation {
                instance: instance(1),
                index: 0
            },
            &duplicate,
            |_, _| panic!("duplicate must fail before provider"),
            budget()
        )
        .is_err()
    );
}
