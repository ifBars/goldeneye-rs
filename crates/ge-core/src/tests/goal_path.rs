use super::*;
use std::cell::RefCell;
fn request(start: usize) -> GoalPathSearch {
    GoalPathSearch {
        start_triangle: start,
        max_expansions: 100,
    }
}
#[test]
fn goal_checks_follow_queue_removal_not_neighbor_discovery_and_heuristic_gets_no_destination() {
    let trace = RefCell::new(Vec::new());
    let n = [
        [Some(1), Some(2), None],
        [None; 3],
        [Some(usize::MAX), None, None],
    ];
    let result = select_triangle_path_to_goal_with_policy(
        request(0),
        &n,
        |f| {
            trace.borrow_mut().push(format!("goal {f}"));
            Ok(f != 0)
        },
        |e| {
            trace.borrow_mut().push(format!("allow {}", e.to_triangle));
            Ok(true)
        },
        |e| {
            trace.borrow_mut().push(format!("cost {}", e.to_triangle));
            Ok(1.)
        },
        |f, g| {
            assert_eq!(g, None);
            trace.borrow_mut().push(format!("h {f}"));
            Ok(if f == 1 { 10. } else { 0. })
        },
    )
    .unwrap();
    assert_eq!(result.search.path.unwrap().triangles, [0, 2]);
    assert_eq!((result.goal_tests, result.search.expanded), (2, 2));
    assert_eq!(
        *trace.borrow(),
        [
            "h 0", "goal 0", "allow 1", "cost 1", "h 1", "allow 2", "cost 2", "h 2", "goal 2"
        ]
    );
}
#[test]
fn accepted_start_still_requires_initial_heuristic_and_stops_before_edges() {
    let mut goals = 0;
    let result = select_triangle_path_to_goal_with_policy(
        GoalPathSearch {
            max_expansions: 1,
            ..request(0)
        },
        &[[Some(999); 3]],
        |f| {
            assert_eq!(f, 0);
            goals += 1;
            Ok(true)
        },
        |_| panic!(),
        |_| panic!(),
        |f, g| {
            assert_eq!((f, g), (0, None));
            Ok(7.)
        },
    )
    .unwrap();
    assert_eq!(goals, 1);
    assert_eq!(result.search.path.unwrap().triangles, [0]);
    assert_eq!(result.search.edge_checks, 0);
    assert!(
        select_triangle_path_to_goal_with_policy(
            request(0),
            &[[None; 3]],
            |_| panic!("invalid initial estimate must fail first"),
            |_| panic!(),
            |_| panic!(),
            |_, _| Ok(f32::NAN)
        )
        .is_err()
    );
}
#[test]
fn multiple_accepted_faces_keep_fifo_score_ties_and_storage_slot_order() {
    let result = select_triangle_path_to_goal_with_policy(
        request(0),
        &[[Some(2), Some(1), None], [None; 3], [None; 3]],
        |f| Ok(f != 0),
        |_| Ok(true),
        |_| Ok(1.),
        |_, _| Ok(0.),
    )
    .unwrap();
    assert_eq!(result.search.path.unwrap().triangles, [0, 2]);
    assert_eq!(result.goal_tests, 2);
}
#[test]
fn reopened_records_are_goal_tested_again_and_keep_pooled_parent_semantics() {
    let n = [
        [Some(1), Some(2), None],
        [Some(3), None, None],
        [Some(1), None, None],
        [None; 3],
    ];
    let mut calls = Vec::new();
    let result = select_triangle_path_to_goal_with_policy(
        request(0),
        &n,
        |f| {
            calls.push(f);
            Ok(f == 1 && calls.iter().filter(|&&x| x == 1).count() == 2)
        },
        |_| Ok(true),
        |e| {
            Ok(match (e.from_triangle, e.to_triangle) {
                (1, 3) => 100.,
                (2, 1) => 0.,
                _ => 1.,
            })
        },
        |f, _| Ok(if f == 2 { 10. } else { 0. }),
    )
    .unwrap();
    assert_eq!(calls, [0, 1, 2, 1]);
    assert_eq!(result.search.reopened, 1);
    assert_eq!(result.search.path.unwrap().triangles, [0, 2, 1]);
}
#[test]
fn exhaustion_boundaries_budgets_and_callback_errors_remain_distinct() {
    let n = [[None, Some(1), None], [None; 3]];
    let result = select_triangle_path_to_goal_with_policy(
        request(0),
        &n,
        |_| Ok(false),
        |_| panic!(),
        |_| panic!(),
        |_, _| Ok(0.),
    )
    .unwrap();
    assert!(result.search.path.is_none());
    assert_eq!(result.goal_tests, 1);
    assert!(
        select_triangle_path_to_goal_with_policy(
            GoalPathSearch {
                max_expansions: 0,
                ..request(0)
            },
            &n,
            |_| panic!(),
            |_| panic!(),
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err()
    );
    assert!(
        select_triangle_path_to_goal_with_policy(
            request(2),
            &n,
            |_| panic!(),
            |_| panic!(),
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err()
    );
    assert!(
        select_triangle_path_to_goal_with_policy(
            request(0),
            &n,
            |_| anyhow::bail!("actor state unavailable"),
            |_| panic!(),
            |_| panic!(),
            |_, _| Ok(0.)
        )
        .is_err()
    );
    let n = [[Some(1), None, None], [None; 3]];
    assert!(
        select_triangle_path_to_goal_with_policy(
            GoalPathSearch {
                max_expansions: 1,
                ..request(0)
            },
            &n,
            |f| Ok(f == 1),
            |_| Ok(true),
            |_| Ok(1.),
            |_, _| Ok(0.)
        )
        .is_err()
    );
}
#[test]
fn actor_heuristic_is_squared_3d_distance_without_epsilon_cutoff_or_square_root() {
    assert_eq!(squared_goal_heuristic([0.; 3], [0., 3., 4.]).unwrap(), 25.);
    assert_eq!(
        squared_goal_heuristic([0.; 3], [f32::EPSILON, 0., 0.]).unwrap(),
        f32::EPSILON * f32::EPSILON
    );
    assert_eq!(
        squared_goal_heuristic(
            [0.; 3],
            [f32::from_bits(0x3fba3e7d), 0., f32::from_bits(0x401089ba)]
        )
        .unwrap()
        .to_bits(),
        0x40e6f5e5
    );
    assert!(squared_goal_heuristic([0.; 3], [f32::NAN, 0., 0.]).is_err());
    assert!(squared_goal_heuristic([f32::MAX; 3], [-f32::MAX; 3]).is_err());
}

#[test]
fn point_policy_uses_stored_slot_weights_or_center_distance_and_validates_inputs() {
    let n = [[Some(1), Some(2), None], [None; 3], [None; 3]];
    let centers = [[0.; 3], [1., 0., 0.], [0., 0., 2.]];
    for (weights, end, cost) in [
        ([Some([0.1, 10., 0.]), None, None], 1, 0.1),
        ([None; 3], 2, 2.),
    ] {
        let result = select_triangle_path_to_goal_with_point(
            request(0),
            PathGraph {
                neighbors: &n,
                centers: &centers,
                weights: &weights,
            },
            [0., 0., 2.],
            |f| Ok(f != 0),
        )
        .unwrap();
        let path = result.search.path.unwrap();
        assert_eq!(path.triangles, [0, end]);
        assert_eq!(path.search_cost, cost);
    }
    for point in [[f32::NAN; 3], [f32::MAX; 3]] {
        assert!(
            select_triangle_path_to_goal_with_point(
                request(0),
                PathGraph {
                    neighbors: &n,
                    centers: &centers,
                    weights: &[None; 3]
                },
                point,
                |_| panic!()
            )
            .is_err()
        );
    }
    assert!(
        select_triangle_path_to_goal_with_point(
            request(0),
            PathGraph {
                neighbors: &n,
                centers: &centers,
                weights: &[]
            },
            [0.; 3],
            |_| panic!()
        )
        .is_err()
    );
}
