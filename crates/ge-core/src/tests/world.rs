use super::*;

fn face(index: usize, y: f32) -> SurfaceTriangle {
    SurfaceTriangle {
        index,
        vertices: [[0., y, 0.], [2., y, 0.], [0., y, 2.]],
    }
}
fn frame() -> NavigationFrame {
    NavigationFrame {
        translation: [0.; 3],
    }
}

#[test]
fn endpoint_lift_precedes_translation_and_does_not_use_a_render_model_matrix() {
    let translated = NavigationFrame {
        translation: [10., 20., -30.],
    };
    let ray = translated.endpoint_ray([10.5, 20., -29.5]).unwrap();
    assert_eq!(ray.origin, [0.5, 1., 0.5]);
    assert_eq!(ray.direction, [0., -1., 0.]);
    assert_eq!(ray.max_parameter, 3.);
    assert_eq!(
        translated.position_to_local([10.5, 20., -29.5]).unwrap(),
        [0.5, 0., 0.5]
    );
    assert_eq!(
        translated.position_to_world([0.5, 0., 0.5]).unwrap(),
        [10.5, 20., -29.5]
    );
    let cancellation = NavigationFrame {
        translation: [0., 16_777_216., 0.],
    };
    assert_eq!(
        cancellation
            .endpoint_ray([0., 16_777_216., 0.])
            .unwrap()
            .origin[1],
        0.
    );
}

#[test]
fn endpoint_window_includes_top_and_excludes_bottom_and_uses_nearest_face() {
    let ground = [face(0, 0.)];
    for (height, parameter) in [(0., 1.), (-1., 0.)] {
        assert_eq!(
            locate_world_endpoint(frame(), [0.5, height, 0.5], ground)
                .unwrap()
                .unwrap()
                .local_hit
                .parameter,
            parameter
        );
    }
    for height in [2., -1.25] {
        assert!(
            locate_world_endpoint(frame(), [0.5, height, 0.5], ground)
                .unwrap()
                .is_none()
        );
    }
    let hit = locate_world_endpoint(frame(), [0.5, 1., 0.5], [face(0, 0.), face(1, 1.5)])
        .unwrap()
        .unwrap();
    assert_eq!(hit.local_hit.triangle, 1);
    assert_eq!(hit.local_hit.parameter, 0.5);
}

#[test]
fn surface_position_is_diagnostic_and_world_frame_is_restored_by_addition() {
    let f = NavigationFrame {
        translation: [10., 20., -30.],
    };
    let hit = locate_world_endpoint(f, [10.5, 20.25, -29.5], [face(0, 0.)])
        .unwrap()
        .unwrap();
    assert_eq!(hit.local_hit.position, [0.5, 0., 0.5]);
    assert_eq!(hit.world_surface_position, [10.5, 20., -29.5]);
}

#[test]
fn world_endpoints_bind_to_absolute_face_ids_before_path_selection() {
    let triangles = [
        SurfaceTriangle {
            index: 0,
            vertices: [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]],
        },
        SurfaceTriangle {
            index: 1,
            vertices: [[1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        },
    ];
    let neighbors = [[Some(1), None, None], [Some(0), None, None]];
    let centers = [[1. / 3., 0., 1. / 3.], [2. / 3., 0., 2. / 3.]];
    let weights = [Some([2.; 3]); 2];
    let graph = PathGraph {
        neighbors: &neighbors,
        centers: &centers,
        weights: &weights,
    };
    let req = WorldPathRequest {
        frame: NavigationFrame {
            translation: [4., 5., -3.],
        },
        start: [4.25, 5., -2.75],
        goal: [4.75, 5., -2.25],
        max_expansions: 10,
    };
    let result = select_world_triangle_path(req, &triangles, graph).unwrap();
    assert_eq!(result.start_endpoint.unwrap().local_hit.triangle, 0);
    assert_eq!(result.goal_endpoint.unwrap().local_hit.triangle, 1);
    let path = result.path_search.unwrap().path.unwrap();
    assert_eq!(path.triangles, [0, 1]);
    assert_eq!(path.search_cost, 2.);
    assert_eq!(req.start, [4.25, 5., -2.75]);
}

#[test]
fn endpoint_miss_and_completed_disconnected_search_are_distinct() {
    let triangles = [face(0, 0.), face(1, 1.5)];
    let neighbors = [[None; 3]; 2];
    let centers = [[0., 0., 0.], [0., 1.5, 0.]];
    let graph = PathGraph {
        neighbors: &neighbors,
        centers: &centers,
        weights: &[None; 2],
    };
    let req = WorldPathRequest {
        frame: frame(),
        start: [0.5, 0., 0.5],
        goal: [0.5, 1., 0.5],
        max_expansions: 10,
    };
    let disconnected = select_world_triangle_path(req, &triangles, graph).unwrap();
    assert!(disconnected.path_search.unwrap().path.is_none());
    let missing = select_world_triangle_path(
        WorldPathRequest {
            start: [10.; 3],
            ..req
        },
        &triangles,
        graph,
    )
    .unwrap();
    assert!(missing.start_endpoint.is_none());
    assert!(missing.goal_endpoint.is_some());
    assert!(missing.path_search.is_none());
    let missing = select_world_triangle_path(
        WorldPathRequest {
            goal: [10.; 3],
            ..req
        },
        &triangles,
        graph,
    )
    .unwrap();
    assert!(missing.start_endpoint.is_some());
    assert!(missing.goal_endpoint.is_none());
    assert!(missing.path_search.is_none());
}

#[test]
fn invalid_numeric_inputs_and_mismatched_face_ids_fail_explicitly() {
    assert!(
        NavigationFrame {
            translation: [f32::NAN; 3]
        }
        .endpoint_ray([0.; 3])
        .is_err()
    );
    assert!(
        NavigationFrame {
            translation: [-f32::MAX; 3]
        }
        .endpoint_ray([f32::MAX; 3])
        .is_err()
    );
    assert!(locate_world_endpoint(frame(), [f32::INFINITY; 3], [face(0, 0.)]).is_err());
    let req = WorldPathRequest {
        frame: frame(),
        start: [0.5, 0., 0.5],
        goal: [0.5, 0., 0.5],
        max_expansions: 1,
    };
    let graph = PathGraph {
        neighbors: &[[None; 3]],
        centers: &[[0.; 3]],
        weights: &[None],
    };
    assert!(select_world_triangle_path(req, &[face(7, 0.)], graph).is_err());
    assert!(
        select_world_triangle_path(
            WorldPathRequest {
                max_expansions: 0,
                ..req
            },
            &[face(0, 0.)],
            graph
        )
        .is_err()
    );
}
