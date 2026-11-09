use super::*;
fn bounds() -> StoredBounds {
    StoredBounds {
        minimum: [-1.; 3],
        maximum: [1.; 3],
    }
}
#[test]
fn loaded_worlds_scan_one_onward_and_short_circuit_at_first_inclusive_match() {
    let mut visited = Vec::new();
    let selected = select_loaded_world(4, [1., -1., 0.], |index| {
        visited.push(index);
        match index {
            1 => Ok(StoredBounds {
                minimum: [3.; 3],
                maximum: [4.; 3],
            }),
            2 => Ok(bounds()),
            _ => panic!("must not address fallback or later worlds"),
        }
    })
    .unwrap();
    assert_eq!(selected, Some(2));
    assert_eq!(visited, [1, 2]);
    let mut visited = Vec::new();
    assert_eq!(
        select_loaded_world(3, [5.; 3], |i| {
            visited.push(i);
            Ok(bounds())
        })
        .unwrap(),
        Some(0)
    );
    assert_eq!(visited, [1, 2]);
}
#[test]
fn absent_or_single_world_never_reads_bounds_or_query_position() {
    for (count, expected) in [(0, None), (1, Some(0))] {
        assert_eq!(
            select_loaded_world(count, [f32::NAN; 3], |_| panic!("unread")).unwrap(),
            expected
        );
    }
    assert!(select_loaded_world(65537, [0.; 3], |_| Ok(bounds())).is_err());
    assert!(select_loaded_world(2, [0.; 3], |_| anyhow::bail!("unavailable bounds")).is_err());
}
#[test]
fn world_bounds_reject_in_native_order_without_reading_later_bad_values() {
    let mut b = bounds();
    b.minimum[0] = 2.;
    b.minimum[1] = f32::NAN;
    assert!(!b.contains([0.; 3]).unwrap());
    b.minimum[0] = -1.;
    assert!(b.contains([0.; 3]).is_err());
    let mut b = bounds();
    b.maximum[0] = -2.;
    b.maximum[2] = f32::NAN;
    assert!(!b.contains([0.; 3]).unwrap());
    let mut b = bounds();
    b.minimum[0] = -f32::MAX;
    assert!(b.contains([f32::MAX, 0., 0.]).is_err());
}
fn bsp() -> MapBsp {
    MapBsp {
        nodes: BTreeMap::from([
            (
                0,
                BspNode {
                    plane: [1., 0., 0., -2.],
                    children: [2, -1],
                },
            ),
            (
                2,
                BspNode {
                    plane: [0., 1., 0., 0.],
                    children: [0, -2],
                },
            ),
        ]),
        zone_count: 3,
    }
}
#[test]
fn bsp_leaf_selection_uses_sign_with_equality_in_slot_zero_and_zero_zone_encoding() {
    let map = bsp();
    let hit = map.zone_at([2., 0., 900.], 2).unwrap();
    assert_eq!((hit.zone, hit.nodes_tested), (0, 2));
    assert_eq!(map.zone_at([1.9999, 0., 0.], 1).unwrap().zone, 1);
    assert_eq!(map.zone_at([3., -0.001, 0.], 2).unwrap().zone, 2);
    assert!(map.zone_at([2., 0., 0.], 1).is_err());
}
#[test]
fn bsp_plane_uses_paired_lane_order_and_handles_the_most_negative_leaf() {
    let map = MapBsp {
        nodes: BTreeMap::from([(
            0,
            BspNode {
                plane: [1., 1., 1., 0.],
                children: [0, i16::MIN],
            },
        )]),
        zone_count: 32769,
    };
    // Fusing Z into X cancels before adding Y, giving -1. A scalar X+Y+Z
    // accumulation rounds away the -1 and would incorrectly select slot zero.
    assert_eq!(
        map.zone_at([33554432., -1., -33554432.], 1).unwrap().zone,
        32768
    );
}
#[test]
fn bsp_cycles_missing_nodes_invalid_leaves_and_used_numeric_errors_are_host_errors() {
    let mut map = bsp();
    map.nodes.get_mut(&2).unwrap().children[0] = 2;
    assert!(
        map.zone_at([2., 0., 0.], 5)
            .unwrap_err()
            .to_string()
            .contains("exhausted")
    );
    assert_eq!(map.zone_at([1., 0., 0.], 1).unwrap().zone, 1);
    map.nodes.remove(&2);
    assert!(map.zone_at([2., 0., 0.], 5).is_err());
    map.nodes.get_mut(&0).unwrap().children[0] = i16::MIN;
    assert!(map.zone_at([2., 0., 0.], 5).is_err());
    map.nodes.get_mut(&0).unwrap().plane[0] = f32::MAX;
    assert!(map.zone_at([2., 0., 0.], 5).is_err());
    assert!(map.zone_at([f32::NAN, 0., 0.], 5).is_err());
    assert!(map.zone_at([0.; 3], 0).is_err());
    assert!(
        MapBsp {
            nodes: BTreeMap::new(),
            zone_count: 1
        }
        .zone_at([0.; 3], 5)
        .is_err()
    );
}
