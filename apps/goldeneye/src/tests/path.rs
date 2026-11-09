use super::*;

#[test]
fn observed_actor_goal_search_uses_all_three_callbacks_and_propagates_unsupported_crossings() {
    let vertices = vec![[0.; 3], [10., 0., 0.], [0., 0., 10.], [10., 0., 10.]];
    let ids = vec![[0, 1, 2], [1, 3, 2]];
    let triangles = ids
        .iter()
        .enumerate()
        .map(|(index, ids)| SurfaceTriangle {
            index,
            vertices: ids.map(|i| vertices[i]),
        })
        .collect();
    let mut data = PathData {
        bounds: [[0.; 3], [10., 0., 10.]],
        neighbors: vec![[Some(1), None, None], [Some(0), None, None]],
        centers: vec![[10. / 3., 0., 10. / 3.], [20. / 3., 0., 20. / 3.]],
        weights: vec![None; 2],
        triangles,
        vertices,
        vertex_ids: ids,
        flags: vec![None; 2],
        normals: vec![None; 2],
        boundary: isolated_boundary(),
    };
    let request = GoalPathSearch {
        start_triangle: 0,
        max_expansions: 100,
    };
    let point = data.centers[1];
    let found = data
        .query_actor_path(request, [1., 0., 1.], point, 100)
        .unwrap();
    assert_eq!(found.result.search.path.unwrap().triangles, [0, 1]);
    assert_eq!(found.result.goal_tests, 2);
    assert_eq!(found.goal_tests.too_far, 1);
    assert_eq!(found.goal_tests.accepted, 1);
    assert_eq!(found.last_goal_test.unwrap().walked_faces, [0, 1]);
    assert_eq!(found.last_center, Some(point));
    let target = data
        .query_actor_target(
            ge_core::navigation::ActorTargetRequest {
                target: point,
                active_position: [1., 0., 1.],
                owner_frame: NavigationFrame {
                    translation: [0.; 3],
                },
                max_face_tests: 100,
            },
            || {
                Ok(Some(NavigationFrame {
                    translation: [0.; 3],
                }))
            },
            100,
            100,
        )
        .unwrap();
    assert_eq!(target.binding.prepared.unwrap().start_triangle, 0);
    assert_eq!(
        target.search.unwrap().result.search.path.unwrap().triangles,
        [0, 1]
    );
    use ge_core::navigation::{
        ActorTargetActionState, ActorTargetOwner, ActorTargetRecord, run_actor_target_action,
    };
    let mut action = ActorTargetActionState {
        state_raw: 91,
        active_index: 0,
        mode_raw: 0,
        target: point,
        records: vec![ActorTargetRecord {
            position: [1., 0., 1.],
            motion: [99.; 3],
        }],
    };
    let owner = ActorTargetOwner {
        frame: NavigationFrame {
            translation: [0.; 3],
        },
        yaw: std::f32::consts::FRAC_PI_4,
    };
    let result = run_actor_target_action(&mut action, owner, 100, |r| {
        data.query_actor_target(r, || Ok(Some(owner.frame)), 100, 100)?
            .selection()
    })
    .unwrap();
    assert_eq!(result.return_code, 2);
    assert_eq!(action.state_raw, 2);
    assert_eq!(action.records[0].position, [1., 0., 1.]);
    assert!((action.records[0].motion[0] - 17. / 6.).abs() < 1e-5);
    assert_eq!(action.records[0].motion[1], 1.);
    data.flags[1] = Some(2);
    let missed = data
        .query_actor_path(request, [1., 0., 1.], point, 100)
        .unwrap();
    assert!(missed.result.search.path.is_none());
    assert_eq!(missed.last_center, Some(data.centers[0]));
    data.flags[1] = None;
    assert!(
        data.query_actor_path(request, point, point, 100)
            .err()
            .unwrap()
            .to_string()
            .contains("no supported forward crossing")
    );
}

#[test]
fn dispatch_keeps_a_stored_bounds_rejection_even_when_direct_query_finds_a_surface() {
    let data = PathData {
        bounds: [[0.; 3], [1., 0., 1.]],
        neighbors: vec![[None; 3]],
        centers: vec![[2. / 3., 0., 2. / 3.]],
        weights: vec![None],
        vertices: vec![[0.; 3], [2., 0., 0.], [0., 0., 2.]],
        vertex_ids: vec![[0, 1, 2]],
        flags: vec![None],
        normals: vec![None],
        boundary: isolated_boundary(),
        triangles: vec![SurfaceTriangle {
            index: 0,
            vertices: [[0.; 3], [2., 0., 0.], [0., 0., 2.]],
        }],
    };
    let request = WorldPathRequest {
        frame: NavigationFrame {
            translation: [0.; 3],
        },
        start: [1.5, 0., 0.1],
        goal: [0.2, 0., 0.2],
        max_expansions: 100,
    };
    let world = data.world_query(request).unwrap();
    assert!(world.start_endpoint.is_some() && world.path_search.as_ref().unwrap().path.is_some());
    let probe = data.probe_dispatch(request, &world).unwrap();
    assert_eq!(
        (probe.acquisitions, probe.queries, probe.successes),
        (2, 0, 0)
    );
    assert_eq!(
        (
            probe.misses,
            probe.bounds_rejections,
            probe.near_surface_excluded
        ),
        (2, 2, 2)
    );
}

#[test]
fn independent_corridor_oracle_distinguishes_boundaries_forward_gate_and_limits() {
    let points = [[0.; 3], [2., 0., 0.], [0.; 3]];
    assert_eq!(
        corridor_oracle(&points, [-0.1, 0., 0.], 0.5, Some(1)),
        Some(None)
    );
    assert_eq!(
        corridor_oracle(&points, [-0.1, 0., 0.], 0.5, None),
        Some(Some(1))
    );
    assert_eq!(corridor_oracle(&points, [1., 0.5, 0.], 0.5, None), None);
    assert_eq!(
        corridor_oracle(&points, [1., 0.4, 0.], 0.5, None),
        Some(Some(0))
    );
    assert_eq!(
        corridor_oracle(&points, [1., 0., 0.], 0.5, Some(0)),
        Some(None)
    );
    assert_eq!(
        corridor_oracle(&points, [1., 0.75, 0.], 0.5, None),
        Some(None)
    );
}

#[test]
fn consumer_audit_rejects_wrong_target_or_length_metadata() {
    let mut points = WorldWaypoints {
        local: WaypointResult {
            points: vec![[0.; 3], [0., 3., 4.]],
            retained_indices: vec![0, 1],
            seed_count: 2,
            shortcut_tests: 0,
            fan_steps: 0,
            flag_rejections: 0,
        },
        world_points: vec![[0.; 3], [0., 3., 4.]],
        next_world_target: Some([0., 3., 4.]),
        length_3d: 5.,
    };
    let checked = validate_route_consumers(&points).unwrap();
    assert!(checked.corridor_validated > 0);
    points.length_3d = 4.;
    assert!(validate_route_consumers(&points).is_err());
    points.length_3d = 5.;
    points.next_world_target = Some([0.; 3]);
    assert!(validate_route_consumers(&points).is_err());
}

#[test]
fn endpoint_miss_oracle_rejects_a_hidden_hit_and_accepts_window_or_face_misses() {
    let data = PathData {
        bounds: [[0.; 3], [1., 0., 1.]],
        neighbors: vec![[None; 3]],
        centers: vec![[0.; 3]],
        weights: vec![None],
        vertices: vec![[0., 0., 0.], [2., 0., 0.], [0., 0., 2.]],
        vertex_ids: vec![[0, 1, 2]],
        flags: vec![None],
        normals: vec![None],
        boundary: isolated_boundary(),
        triangles: vec![SurfaceTriangle {
            index: 0,
            vertices: [[0., 0., 0.], [2., 0., 0.], [0., 0., 2.]],
        }],
    };
    let frame = NavigationFrame {
        translation: [0.; 3],
    };
    assert!(data.validate_endpoint_miss(frame, [0.5, 0., 0.5]).is_err());
    assert!(data.validate_endpoint_miss(frame, [0.5, 2., 0.5]).is_ok());
    assert!(data.validate_endpoint_miss(frame, [10., 0., 10.]).is_ok());
}

fn isolated_boundary() -> BoundaryCache {
    build_boundary_cache(BoundaryMesh {
        vertex_count: 3,
        faces: &[[0, 1, 2]],
        neighbors: &[[None; 3]],
        flags: &[None],
    })
    .unwrap()
}
