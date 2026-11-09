use super::*;
fn fixture() -> Vec<u8> {
    let mut d = vec![0; 0x500];
    let mut put = |p: usize, v: u32| d[p..p + 4].copy_from_slice(&v.to_be_bytes());
    for (p, v) in [
        (0, 0x47454f4d),
        (4, 0x01000001),
        (8, 263),
        (20, 0x500),
        (24, 0x500),
        (0x50, 0x10000),
        (0x54, 0xd0 - 0x54),
        (0xd0, 0x82000010),
        (0xd8, 0x100),
        (0x70, 0x10000),
        (0x74, 0x220 - 0x74),
        (0x220, 0x85000000),
        (0x228, 0x240),
        (0x240, 0x500),
        (0x288, 2),
        (0x28c, 0x300 - 0x28c),
        (0x100, 0x607),
        (0x120, 2f32.to_bits()),
        (0x128, 2f32.to_bits()),
        (0x16c, 3),
        (0x170, 1),
        (0x174, 1),
        (0x178, 0x1c0 - 0x178),
        (0x17c, 0x1e4 - 0x17c),
        (0x180, 0x1f4 - 0x180),
        (0x184, 0x200 - 0x184),
        (0x1cc, 2f32.to_bits()),
        (0x1e0, 2f32.to_bits()),
        (0x1e8, 1),
        (0x1ec, 2),
        (0x1f4, 0xfffff),
        (0x1f8, 0xfffff),
        (0x1fc, 0xfffff),
        (0x200, 1),
        (0x300, 0x02000100),
        (0x304, 10f32.to_bits()),
        (0x330, 0x82000000),
        (0x33c, 0x02000101),
        (0x340, 20f32.to_bits()),
        (0x36c, 0x82000000),
    ] {
        put(p, v);
    }
    d
}
#[test]
fn static_map_preparation_shares_a_mesh_and_queries_the_selected_instances_frame() {
    let edb = Edb::parse(fixture()).unwrap();
    let (resident, slots, metadata, map) = prepare(&edb, Some(0x85000000), None).unwrap();
    assert_eq!(
        (
            resident.len(),
            slots.len(),
            metadata.len(),
            map.unwrap().registrations.len()
        ),
        (1, 2, 2, 2)
    );
    assert_eq!((slots[0].resident, slots[1].resident), (0, 0));
    let instances = slots
        .iter()
        .map(|s| resident[s.resident].instance(s.frame))
        .collect::<Vec<_>>();
    let selected = acquire_navigation(&instances, [20.5, 0., 0.5], 30.)
        .unwrap()
        .selected
        .unwrap();
    assert_eq!(selected.instance, 1);
    let slot = &slots[selected.instance];
    let request = WorldPathRequest {
        frame: slot.frame,
        start: [20.5, 0., 0.5],
        goal: [21., 0., 0.5],
        max_expansions: 100,
    };
    let world = resident[slot.resident].world_query(request).unwrap();
    assert_eq!(
        world.start_endpoint.unwrap().local_hit.position,
        [0.5, 0., 0.5]
    );
    assert!(world.path_search.unwrap().path.is_some());
}
#[test]
fn ready_registration_with_corrupt_mesh_is_an_error_without_a_fallback_context() {
    let mut d = fixture();
    d[0x16c..0x170].copy_from_slice(&0u32.to_be_bytes());
    let edb = Edb::parse(d).unwrap();
    assert!(prepare(&edb, Some(0x85000000), None).is_err());
    assert!(prepare(&edb, Some(0x85000000), Some([0.; 3])).is_err());
}

fn two_face_actor_fixture(flags: u32, normal_y: Option<f32>) -> Vec<u8> {
    let mut d = fixture();
    let mut put = |p: usize, v: u32| d[p..p + 4].copy_from_slice(&v.to_be_bytes());
    for (p, v) in [
        (0x16c, 4),
        (0x170, 2),
        (0x178, 0x3a0 - 0x178),
        (0x17c, 0x3d0 - 0x17c),
        (0x180, 0x3f0 - 0x180),
        (0x184, 0x408 - 0x184),
        (0x188, 0x410 - 0x188),
        (0x194, 0x428 - 0x194),
        (0x408, 2),
        (0x3d4, 1),
        (0x3d8, 2),
        (0x3e0, 1),
        (0x3e4, 3),
        (0x3e8, 2),
        (0x3f0, 1),
        (0x3f4, 0xfffff),
        (0x3f8, 0xfffff),
        (0x3fc, 0),
        (0x400, 0xfffff),
        (0x404, 0xfffff),
        (0x42c, flags),
    ] {
        put(p, v);
    }
    for (i, v) in [[0f32; 3], [2., 0., 0.], [0., 0., 2.], [2., 0., 2.]]
        .into_iter()
        .enumerate()
    {
        for (j, x) in v.into_iter().enumerate() {
            put(0x3a0 + i * 12 + j * 4, x.to_bits());
        }
    }
    for (i, v) in [[2f32 / 3., 0., 2. / 3.], [4. / 3., 0., 4. / 3.]]
        .into_iter()
        .enumerate()
    {
        for (j, x) in v.into_iter().enumerate() {
            put(0x410 + i * 12 + j * 4, x.to_bits());
        }
    }
    if let Some(y) = normal_y {
        put(0x18c, 0x430 - 0x18c);
        put(0x434, 1f32.to_bits());
        put(0x440, y.to_bits());
    }
    d
}
#[test]
fn actor_edges_apply_stored_flags_normals_and_retain_the_previous_world_route_on_rejection() {
    for (flags, normal, expected) in [
        (1, Some(1.), false),
        (0, Some(0.8), false),
        (0, None, true),
        (4, Some(1.), true),
    ] {
        let edb = Edb::parse(two_face_actor_fixture(flags, normal)).unwrap();
        let (resident, slots, _, _) = prepare(&edb, Some(0x85000000), None).unwrap();
        let data = &resident[0];
        let request = WorldGoalPathRequest {
            frame: slots[1].frame,
            start: [20.2, 0., 0.2],
            max_expansions: 100,
        };
        let mut state = NavigationRouteState::default();
        let point = [4. / 3., 0., 4. / 3.];
        assert!(
            data.regenerate_goal_route(&mut state, request, &[1], point, false)
                .unwrap()
                .success
        );
        let retained = state.points().to_vec();
        let found = data
            .regenerate_goal_route(&mut state, request, &[1], point, true)
            .unwrap();
        assert_eq!(found.success, expected);
        assert_eq!(state.points(), retained);
        assert_eq!(state.status_raw(), if expected { 0 } else { 10 });
    }
}

#[test]
fn shared_boundary_cache_keeps_raw_coordinates_for_translated_instances() {
    let mut d = fixture();
    d[0x194..0x198].copy_from_slice(&(0x210u32 - 0x194).to_be_bytes());
    d[0x210..0x214].copy_from_slice(&1u32.to_be_bytes());
    let edb = Edb::parse(d).unwrap();
    let (resident, slots, _, _) = prepare(&edb, Some(0x85000000), None).unwrap();
    assert_eq!(resident.len(), 1);
    assert_eq!(resident[0].boundary_stats().edges, 3);
    assert_eq!(slots[1].frame.translation, [20., 0., 0.]);
    assert!(
        resident[0]
            .boundary_check([-0.1, 100., 0.5], [0.1, -100., 0.5])
            .unwrap()
            .is_some()
    );
    assert!(
        resident[0]
            .boundary_check([19.9, 100., 0.5], [20.1, -100., 0.5])
            .unwrap()
            .is_none()
    );
}

#[test]
fn cached_dispatch_retains_a_route_on_miss_then_rebuilds_from_a_later_authored_position() {
    let edb = Edb::parse(fixture()).unwrap();
    let (residents, slots, _, _) = prepare(&edb, Some(0x85000000), None).unwrap();
    let instances = slots
        .iter()
        .map(|s| residents[s.resident].instance(s.frame))
        .collect::<Vec<_>>();
    let mut request_state = NavigationRequestState {
        flags_raw: 0x300,
        binding: None,
    };
    let mut route = NavigationRouteState::default();
    let good = [20.5, 0., 0.5];
    let goal = [21., 0., 0.5];
    let mut retained = Vec::new();
    for (step, start) in [good, good, [1e6; 3], goal].into_iter().enumerate() {
        request_state
            .dispatch(
                || {
                    Ok(acquire_navigation(&instances, start, 30.)?
                        .selected
                        .map(|s| s.instance))
                },
                |binding| {
                    let slot = &slots[binding];
                    Ok(residents[slot.resident]
                        .regenerate_route(
                            &mut route,
                            WorldPathRequest {
                                frame: slot.frame,
                                start,
                                goal,
                                max_expansions: 100,
                            },
                        )?
                        .success)
                },
            )
            .unwrap();
        assert_eq!(request_state.binding, Some(1));
        match step {
            0 => assert!(route.points().is_empty()),
            1 => {
                assert_eq!(route.status_raw(), 0);
                retained = route.points().to_vec();
                assert_eq!(retained, [good, goal]);
            }
            2 => {
                assert_eq!(request_state.flags_raw, 0x700);
                assert_eq!(route.status_raw(), 6);
                assert_eq!(route.points(), retained);
            }
            3 => {
                assert_eq!(request_state.flags_raw, 0xf00);
                assert_eq!(route.status_raw(), 0);
                assert_eq!(route.points(), [goal, goal]);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn predicate_regeneration_uses_raw_center_for_translated_waypoints_and_retains_on_later_miss() {
    let edb = Edb::parse(fixture()).unwrap();
    let (residents, slots, _, _) = prepare(&edb, Some(0x85000000), None).unwrap();
    let slot = &slots[1];
    let data = &residents[slot.resident];
    let mut state = NavigationRouteState::default();
    let request = WorldGoalPathRequest {
        frame: slot.frame,
        start: [20.5, 0., 0.5],
        max_expansions: 100,
    };
    let found = data
        .regenerate_goal_route(&mut state, request, &[0], [0.; 3], false)
        .unwrap();
    assert!(found.success);
    let selected = found.result.selected_goal.unwrap();
    assert!(selected.raw_center[0] < 1.);
    assert_eq!(
        selected.local_waypoint_goal,
        slot.frame.position_to_local(selected.raw_center).unwrap()
    );
    let points = found.waypoints.unwrap();
    assert_eq!(points.world_points[0], request.start);
    assert_eq!(
        *points.world_points.last().unwrap(),
        selected.returned_world_goal
    );
    assert!(points.world_points.last().unwrap()[0] < 1.); // Do not add the +20 instance translation to the raw center.
    let old = state.points().to_vec();
    let missing = data
        .regenerate_goal_route(
            &mut state,
            WorldGoalPathRequest {
                start: [1e6; 3],
                ..request
            },
            &[0],
            [0.; 3],
            false,
        )
        .unwrap();
    assert!(!missing.success);
    assert_eq!(state.status_raw(), 6);
    assert_eq!(state.points(), old);
}
