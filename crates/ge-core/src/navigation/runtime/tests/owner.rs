use super::{binding, budget, instance, nav, placement, wrapper};
use crate::navigation::{
    BoundaryMesh,
    runtime::{MapView, Owner, PlacementLocation, resolve_binding},
};

#[test]
fn categories_use_flag_nibble_and_source_order_not_partition_ranges_or_all_flags() {
    let faces = [[0, 1, 2]; 6];
    let neighbors = [[None; 3]; 6];
    let flags = [
        Some(0x100),
        Some(0x201),
        Some(0xf01),
        Some(0xf00),
        None,
        Some(0x1800),
    ];
    let owner = Owner::build(
        binding(),
        BoundaryMesh {
            vertex_count: 3,
            faces: &faces,
            neighbors: &neighbors,
            flags: &flags,
        },
        budget(),
    )
    .unwrap();
    assert_eq!(owner.categories().len(), 15);
    assert_eq!(owner.categories()[0], [0]);
    assert_eq!(owner.categories()[1], [1]);
    assert_eq!(owner.categories()[14], [2, 3]);
    assert_eq!(owner.categories()[7], [5]);
    assert_eq!(owner.categories().iter().map(Vec::len).sum::<usize>(), 5);
    let pairs: Vec<_> = owner
        .cache()
        .edges()
        .iter()
        .map(|edge| edge.vertices)
        .collect();
    assert_eq!(pairs, [[0, 1], [1, 2], [2, 0], [0, 1], [1, 2], [2, 0]]);
    assert_eq!(owner.face_count(), 6);
    assert_eq!(*owner.binding(), binding());
}

#[test]
fn optional_flags_and_zero_faces_are_valid_actual_empty_cache_inputs() {
    let faces = [[0, 1, 2]];
    let owner = Owner::build(
        binding(),
        BoundaryMesh {
            vertex_count: 3,
            faces: &faces,
            neighbors: &[[None; 3]],
            flags: &[None],
        },
        budget(),
    )
    .unwrap();
    assert!(owner.categories().iter().all(Vec::is_empty));
    assert!(owner.cache().edges().is_empty());
    assert_eq!(owner.cache().stats().isolated_faces, 1);
    let owner = Owner::build(
        binding(),
        BoundaryMesh {
            vertex_count: 0,
            faces: &[],
            neighbors: &[],
            flags: &[],
        },
        budget(),
    )
    .unwrap();
    assert_eq!(owner.face_count(), 0);
    assert!(owner.cache().edges().is_empty());
}

#[test]
fn cache_allocation_and_categories_are_bounded_before_initialized_owner_is_returned() {
    let faces = [[0, 1, 2]];
    let neighbors = [[None; 3]];
    let flags = [Some(0x101)];
    for limited in [
        super::super::Budget {
            faces: 0,
            ..budget()
        },
        super::super::Budget {
            cache_edges: 2,
            ..budget()
        },
        super::super::Budget {
            category_members: 0,
            ..budget()
        },
    ] {
        assert!(
            Owner::build(
                binding(),
                BoundaryMesh {
                    vertex_count: 3,
                    faces: &faces,
                    neighbors: &neighbors,
                    flags: &flags
                },
                limited
            )
            .is_err()
        );
    }
    assert!(
        Owner::build(
            binding(),
            BoundaryMesh {
                vertex_count: 3,
                faces: &faces,
                neighbors: &[],
                flags: &flags
            },
            budget()
        )
        .is_err()
    );
    assert!(
        Owner::build(
            binding(),
            BoundaryMesh {
                vertex_count: 2,
                faces: &faces,
                neighbors: &neighbors,
                flags: &flags
            },
            budget()
        )
        .is_err()
    );
}

#[test]
fn duplicate_source_keys_produce_distinct_owners_in_call_order_without_deduplication() {
    let rows = [
        placement(0x1234_5678, 3, [1; 3]),
        placement(0x1234_5678, 4, [2; 3]),
    ];
    let maps = [MapView {
        instance: instance(1),
        wrapper: wrapper(1),
        placements: &rows,
    }];
    let mut owners = Vec::new();
    for index in 0..rows.len() {
        let resolved = resolve_binding(
            wrapper(1),
            PlacementLocation {
                instance: instance(1),
                index,
            },
            &maps,
            |_, raw| Ok(Some(nav(raw as usize))),
            budget(),
        )
        .unwrap()
        .unwrap();
        owners.push(
            Owner::build(
                resolved,
                BoundaryMesh {
                    vertex_count: 0,
                    faces: &[],
                    neighbors: &[],
                    flags: &[],
                },
                budget(),
            )
            .unwrap(),
        );
    }
    assert_eq!(owners.len(), 2);
    assert_eq!(owners[0].binding().source.index, 0);
    assert_eq!(owners[1].binding().source.index, 1);
    assert_eq!(owners[0].binding().found.index, 0);
    assert_eq!(owners[1].binding().found.index, 0);
    assert_eq!(owners[1].binding().source_entity_reference_raw, 4);
    assert_eq!(owners[1].binding().cached_entity_reference_raw, 3);
}
