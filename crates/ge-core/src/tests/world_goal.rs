use super::super::{NavigationRouteSearch, NavigationRouteState};
use super::*;
fn floor(index: usize, x: f32) -> SurfaceTriangle {
    SurfaceTriangle {
        index,
        vertices: [[x - 2., 0., -2.], [x + 2., 0., -2.], [x, 0., 2.]],
    }
}
fn request(frame: [f32; 3], start: [f32; 3]) -> WorldGoalPathRequest {
    WorldGoalPathRequest {
        frame: NavigationFrame { translation: frame },
        start,
        max_expansions: 100,
    }
}
#[test]
fn translated_goal_uses_raw_stored_center_then_subtract_add_instead_of_world_translation() {
    let tri = [floor(0, 0.)];
    let graph = PathGraph {
        neighbors: &[[None; 3]],
        centers: &[[1., 2., 3.]],
        weights: &[None],
    };
    let result = select_world_triangle_path_to_goal_with_point(
        request([10., 5., 20.], [10., 5., 20.]),
        &tri,
        graph,
        [0.; 3],
        |_| Ok(true),
    )
    .unwrap();
    let selected = result.selected_goal.unwrap();
    assert_eq!(selected.raw_center, [1., 2., 3.]);
    assert_eq!(selected.local_waypoint_goal, [-9., -3., -17.]);
    assert_eq!(selected.returned_world_goal, [1., 2., 3.]);
    assert_eq!(
        result.path_search.unwrap().search.path.unwrap().triangles,
        [0]
    );
}
#[test]
fn returned_goal_keeps_float_round_trip_loss_separate_from_raw_center() {
    let tri = [floor(0, 0.)];
    let graph = PathGraph {
        neighbors: &[[None; 3]],
        centers: &[[-1., 0., 0.]],
        weights: &[None],
    };
    let selected = select_world_triangle_path_to_goal_with_point(
        request([16777216., 0., 0.], [16777216., 0., 0.]),
        &tri,
        graph,
        [0.; 3],
        |_| Ok(true),
    )
    .unwrap()
    .selected_goal
    .unwrap();
    assert_eq!(selected.raw_center, [-1., 0., 0.]);
    assert_eq!(selected.local_waypoint_goal, [-16777216., 0., 0.]);
    assert_eq!(selected.returned_world_goal, [0.; 3]);
}
#[test]
fn selected_vertical_face_needs_no_admissible_goal_surface_ray() {
    let tri = [
        floor(0, 0.),
        SurfaceTriangle {
            index: 1,
            vertices: [[100., 0., -1.], [100., 2., -1.], [100., 0., 1.]],
        },
    ];
    let graph = PathGraph {
        neighbors: &[[Some(1), None, None], [None; 3]],
        centers: &[[0.; 3], [100., 1., 0.]],
        weights: &[Some([1.; 3]), None],
    };
    assert!(
        locate_world_endpoint(
            NavigationFrame {
                translation: [0.; 3]
            },
            [100., 1., 0.],
            tri.iter().copied()
        )
        .unwrap()
        .is_none()
    );
    let result = select_world_triangle_path_to_goal_with_point(
        request([0.; 3], [0.; 3]),
        &tri,
        graph,
        [100., 1., 0.],
        |f| Ok(f == 1),
    )
    .unwrap();
    assert_eq!(result.selected_goal.unwrap().triangle, 1);
}
#[test]
fn start_miss_suppresses_search_and_completed_predicate_miss_has_distinct_retained_status() {
    let tri = [floor(0, 0.)];
    let graph = PathGraph {
        neighbors: &[[None; 3]],
        centers: &[[0.; 3]],
        weights: &[None],
    };
    let missing = select_world_triangle_path_to_goal_with_point(
        request([0.; 3], [100.; 3]),
        &tri,
        graph,
        [f32::NAN; 3],
        |_| panic!("missing start must not call goal policy"),
    )
    .unwrap();
    assert!(missing.path_search.is_none());
    assert!(matches!(
        NavigationRouteSearch::from_world_goal(&missing).unwrap(),
        NavigationRouteSearch::MissingStart
    ));
    let miss = select_world_triangle_path_to_goal_with_point(
        request([0.; 3], [0.; 3]),
        &tri,
        graph,
        [0.; 3],
        |_| Ok(false),
    )
    .unwrap();
    assert!(miss.start_endpoint.is_some() && miss.selected_goal.is_none());
    assert!(miss.path_search.as_ref().unwrap().search.path.is_none());
    let mut state = NavigationRouteState::default();
    state
        .regenerate(
            true,
            || {
                Ok(NavigationRouteSearch::Found(super::super::TrianglePath {
                    triangles: vec![8],
                    search_cost: 0.,
                }))
            },
            |_| Ok(vec![[8.; 3], [9.; 3]]),
        )
        .unwrap();
    assert!(
        !state
            .regenerate(
                true,
                || NavigationRouteSearch::from_world_goal(&miss),
                |_| panic!()
            )
            .unwrap()
    );
    assert_eq!(state.status_raw(), 10);
    assert_eq!(state.points(), &[[8.; 3], [9.; 3]]);
    assert_eq!(state.triangles(), &[8]);
}
#[test]
fn malformed_world_goal_inputs_and_inconsistent_results_are_host_errors() {
    let tri = [floor(7, 0.)];
    let graph = PathGraph {
        neighbors: &[[None; 3]],
        centers: &[[0.; 3]],
        weights: &[None],
    };
    assert!(
        select_world_triangle_path_to_goal_with_point(
            request([0.; 3], [0.; 3]),
            &tri,
            graph,
            [0.; 3],
            |_| Ok(true)
        )
        .is_err()
    );
    let tri = [floor(0, 0.)];
    let mut req = request([0.; 3], [0.; 3]);
    req.max_expansions = 0;
    assert!(
        select_world_triangle_path_to_goal_with_point(req, &tri, graph, [0.; 3], |_| Ok(true))
            .is_err()
    );
    req.max_expansions = 100;
    req.start[0] = f32::INFINITY;
    assert!(
        select_world_triangle_path_to_goal_with_point(req, &tri, graph, [0.; 3], |_| Ok(true))
            .is_err()
    );
    let mut result = WorldGoalPathResult {
        start_endpoint: None,
        selected_goal: Some(SelectedWorldGoal {
            triangle: 0,
            raw_center: [0.; 3],
            local_waypoint_goal: [0.; 3],
            returned_world_goal: [0.; 3],
        }),
        path_search: None,
    };
    assert!(NavigationRouteSearch::from_world_goal(&result).is_err());
    result = select_world_triangle_path_to_goal_with_point(
        request([0.; 3], [0.; 3]),
        &tri,
        graph,
        [0.; 3],
        |_| Ok(true),
    )
    .unwrap();
    result.selected_goal.as_mut().unwrap().triangle = 9;
    assert!(NavigationRouteSearch::from_world_goal(&result).is_err());
}
