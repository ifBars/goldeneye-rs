use super::*;
use ge_assets::candidates::{Branch, Leaf, Node};
fn fixture() -> (Edb, Vec<Placement>, Hierarchy) {
    let mut bytes = vec![0; 0x200];
    for (at, value) in [
        (0, 0x47454f4du32),
        (4, 0x01000001),
        (8, 263),
        (20, 0x200),
        (24, 0x200),
        (0x50, 0x10000),
        (0x54, 0x7c),
        (0xd0, 0x82000000),
        (0xd8, 0x100),
        (0x100, 0x601),
        (0x140, 0xffffff01),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }
    for at in [0x120, 0x124, 0x128] {
        bytes[at..at + 4].copy_from_slice(&2f32.to_bits().to_be_bytes());
    }
    let placement = Placement {
        hash: 0,
        position: [20., 0., 0.],
        rotation: [0.; 3],
        scale: [1.; 3],
        flags: 0,
        engine_flags: 0,
        map_on: 0,
        object_hash: 0,
        light_set: 0,
        group: -1,
        unknown_word: 0,
        model: ge_assets::transform::placement([20., 0., 0.], [0.; 3], [1.; 3]).unwrap(),
        status: None,
    };
    let tree = Hierarchy {
        root: 1,
        nodes: BTreeMap::from([
            (
                1,
                Node {
                    flags_raw: 8,
                    entries: Entries::Branches(vec![Branch {
                        target: 2,
                        bounds: [[-10.; 3], [10.; 3]],
                    }]),
                },
            ),
            (
                2,
                Node {
                    flags_raw: 8,
                    entries: Entries::Leaves(vec![
                        Leaf {
                            placement: 0,
                            flags_raw: 8,
                            unknown_byte: 0xaa,
                        },
                        Leaf {
                            placement: 0,
                            flags_raw: 8,
                            unknown_byte: 0xbb,
                        },
                    ]),
                },
            ),
        ]),
    };
    (Edb::parse(bytes).unwrap(), vec![placement], tree)
}
#[test]
fn initialized_adapter_uses_base_bounds_and_retains_flags_seeds_and_duplicate_candidates() {
    let (edb, placements, tree) = fixture();
    let flags = membership(&tree);
    let mut runtime = Runtime::new(tree, &edb, &placements);
    let mut accumulated = Bounds::MISSING_ENTITY;
    let stats = runtime.refresh(&mut accumulated).unwrap();
    assert_eq!(
        (
            stats.leaf_entries,
            stats.resolved_placements,
            stats.branch_bounds_written
        ),
        (2, 2, 1)
    );
    assert_eq!(membership(&runtime.hierarchy), flags);
    assert_eq!(
        runtime.branch(1, 0).unwrap().bounds,
        [[-10.; 3], [22., 10., 10.]]
    );
    assert_eq!(runtime.query([21., 0., 0.], 0.).unwrap().placements, [0, 0]);
    assert!(
        runtime
            .query([30., 0., 0.], 0.)
            .unwrap()
            .placements
            .is_empty()
    );
    let (expected, ambiguous) = oracle(&runtime.hierarchy, [21., 0., 0.], 0.).unwrap();
    assert_eq!(expected, [0, 0]);
    assert!(!ambiguous);
    assert!(oracle(&runtime.hierarchy, [22., 0., 0.], 0.).unwrap().1);
}
#[test]
fn malformed_addressed_placement_is_error_and_disabled_membership_is_not_fabricated() {
    let (edb, placements, mut tree) = fixture();
    tree.nodes.get_mut(&2).unwrap().flags_raw = 0;
    let mut runtime = Runtime::new(tree, &edb, &placements);
    let mut accumulated = Bounds::MISSING_ENTITY;
    runtime.refresh(&mut accumulated).unwrap();
    assert!(
        runtime
            .query([21., 0., 0.], 0.)
            .unwrap()
            .placements
            .is_empty()
    );
    let mut missing = Runtime::new(runtime.hierarchy, &edb, &[]);
    assert!(missing.refresh(&mut accumulated).is_err());
}
