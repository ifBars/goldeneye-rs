use super::*;

fn face(index: usize, y: f32, x: f32) -> SurfaceTriangle {
    SurfaceTriangle {
        index,
        vertices: [[x, y, 0.], [x + 2., y, 0.], [x, y, 2.]],
    }
}
fn request(depth: i32) -> SegmentSearch {
    SegmentSearch {
        origin: [0.5, 1., 0.5],
        displacement: [0., -1., 0.],
        start_triangle: 0,
        previous_triangle: None,
        max_depth: depth,
        max_face_tests: 100,
    }
}

#[test]
fn starting_face_wins_before_any_policy_call_even_with_negative_depth() {
    let faces = [face(0, 0., 0.), face(1, 0.8, 0.)];
    let neighbors = [[Some(1), None, None], [Some(0), None, None]];
    for depth in [i32::MIN, -1, 0, 10] {
        let result = connected_segment_hit_with_policy(request(depth), &faces, &neighbors, |_| {
            panic!("current face already hits")
        })
        .unwrap();
        assert_eq!(result.hit.unwrap().triangle, 0);
        assert_eq!(result.hit.unwrap().parameter, 1.);
        assert_eq!(
            (
                result.face_tests,
                result.policy_checks,
                result.maximum_depth_reached
            ),
            (1, 0, 0)
        );
    }
}

#[test]
fn depth_first_slot_order_wins_over_breadth_first_or_nearest_selection() {
    let faces = [
        face(0, 0., 10.),
        face(1, 0., 10.),
        face(2, 0.5, 0.),
        face(3, 0.8, 0.),
    ];
    let neighbors = [
        [Some(1), Some(3), None],
        [Some(0), Some(2), None],
        [None; 3],
        [None; 3],
    ];
    let deep = connected_segment_hit(request(2), &faces, &neighbors).unwrap();
    assert_eq!(deep.hit.unwrap().triangle, 2);
    assert_eq!(
        (
            deep.face_tests,
            deep.policy_checks,
            deep.maximum_depth_reached
        ),
        (3, 3, 2)
    );
    let shallow = connected_segment_hit(request(1), &faces, &neighbors).unwrap();
    assert_eq!(shallow.hit.unwrap().triangle, 3);
    assert_eq!(
        (
            shallow.face_tests,
            shallow.policy_checks,
            shallow.maximum_depth_reached
        ),
        (3, 2, 1)
    );
    for depth in [i32::MIN, -1, 0] {
        let no_transition = connected_segment_hit(request(depth), &faces, &neighbors).unwrap();
        assert!(no_transition.hit.is_none());
        assert_eq!(
            (no_transition.face_tests, no_transition.policy_checks),
            (1, 0)
        );
    }
}

#[test]
fn default_policy_prevents_only_immediate_backtracking_without_global_deduplication() {
    let faces = [face(0, 0., 10.), face(1, 0., 10.), face(2, 0., 10.)];
    let cycle = [
        [Some(1), None, None],
        [Some(2), None, None],
        [Some(0), None, None],
    ];
    let result = connected_segment_hit(request(4), &faces, &cycle).unwrap();
    assert!(result.hit.is_none());
    assert_eq!(
        (
            result.face_tests,
            result.policy_checks,
            result.maximum_depth_reached
        ),
        (5, 4, 4)
    );
    let pair = [[Some(1), None, None], [Some(0), None, None], [None; 3]];
    let result = connected_segment_hit(request(100), &faces, &pair).unwrap();
    assert_eq!(
        (
            result.face_tests,
            result.policy_checks,
            result.maximum_depth_reached
        ),
        (2, 2, 1)
    );
    let self_link = [[Some(0), None, None], [None; 3], [None; 3]];
    let result = connected_segment_hit(request(100), &faces, &self_link).unwrap();
    assert_eq!(
        (
            result.face_tests,
            result.policy_checks,
            result.maximum_depth_reached
        ),
        (2, 2, 1)
    );
}

#[test]
fn policy_replacement_receives_previous_face_and_can_override_default_rejection() {
    let faces = [face(0, 0., 10.), face(1, 0.5, 0.), face(2, 0.8, 0.)];
    let neighbors = [[Some(1), Some(2), None], [None; 3], [None; 3]];
    let mut req = request(1);
    req.previous_triangle = Some(1);
    assert_eq!(
        connected_segment_hit(req, &faces, &neighbors)
            .unwrap()
            .hit
            .unwrap()
            .triangle,
        2
    );
    let mut seen = Vec::new();
    let result = connected_segment_hit_with_policy(req, &faces, &neighbors, |edge| {
        seen.push(edge);
        true
    })
    .unwrap();
    assert_eq!(result.hit.unwrap().triangle, 1);
    assert_eq!(
        seen,
        [TraversalEdge {
            from_triangle: 0,
            to_triangle: 1,
            previous_triangle: Some(1),
            slot: 0
        }]
    );
}

#[test]
fn callbacks_interleave_with_subtree_visits_in_original_slot_order() {
    let faces = (0..4).map(|i| face(i, 0., 10.)).collect::<Vec<_>>();
    let neighbors = [
        [Some(1), Some(2), None],
        [Some(3), Some(0), None],
        [None; 3],
        [None; 3],
    ];
    let mut edges = Vec::new();
    let result = connected_segment_hit_with_policy(request(2), &faces, &neighbors, |edge| {
        edges.push((edge.from_triangle, edge.to_triangle));
        Some(edge.to_triangle) != edge.previous_triangle
    })
    .unwrap();
    assert!(result.hit.is_none());
    assert_eq!(edges, [(0, 1), (1, 3), (1, 0), (0, 2)]);
    assert_eq!((result.face_tests, result.policy_checks), (4, 4));
}

#[test]
fn exhausted_host_budget_is_an_error_while_a_completed_search_can_use_the_last_test() {
    let faces = [face(0, 0., 10.), face(1, 0., 10.)];
    let neighbors = [[Some(1), None, None], [Some(0), None, None]];
    let mut req = request(10);
    req.max_face_tests = 2;
    assert!(
        connected_segment_hit(req, &faces, &neighbors)
            .unwrap()
            .hit
            .is_none()
    );
    req.max_face_tests = 1;
    let error = connected_segment_hit(req, &faces, &neighbors).unwrap_err();
    assert!(error.to_string().contains("budget exhausted after 1 tests"));
    req.max_depth = 0;
    assert!(
        connected_segment_hit(req, &faces, &neighbors)
            .unwrap()
            .hit
            .is_none()
    );
    req.max_face_tests = 0;
    assert!(connected_segment_hit(req, &faces, &neighbors).is_err());
}

#[test]
fn invalid_topology_ids_and_numeric_inputs_are_checked_without_panics() {
    let faces = [face(0, 0., 10.)];
    let neighbors = [[None; 3]];
    assert!(connected_segment_hit(request(1), &faces, &[]).is_err());
    let mut req = request(1);
    req.start_triangle = usize::MAX;
    assert!(connected_segment_hit(req, &faces, &neighbors).is_err());
    req = request(1);
    req.previous_triangle = Some(1);
    assert!(connected_segment_hit(req, &faces, &neighbors).is_err());
    assert!(connected_segment_hit(request(1), &faces, &[[Some(1), None, None]]).is_err());
    assert!(connected_segment_hit(request(1), &[face(7, 0., 10.)], &neighbors).is_err());
    req = request(1);
    req.origin[0] = f32::NAN;
    assert!(connected_segment_hit(req, &faces, &neighbors).is_err());
}

#[test]
fn first_boundary_ends_the_neighbor_list_instead_of_skipping_to_later_slots() {
    let faces = [face(0, 0., 10.), face(1, 0.5, 0.), face(2, 0., 10.)];
    for neighbors in [
        [[None, Some(1), None], [None; 3], [None; 3]],
        [[Some(2), None, Some(1)], [None; 3], [None; 3]],
    ] {
        let found = connected_segment_hit(request(3), &faces, &neighbors).unwrap();
        assert!(found.hit.is_none());
        assert!(found.face_tests <= 2);
        assert!(found.policy_checks <= 1);
    }
}
