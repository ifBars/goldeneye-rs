use super::*;
fn request(start: usize, goal: usize) -> PathSearch {
    PathSearch {
        start_triangle: start,
        goal_triangle: goal,
        max_expansions: 1000,
    }
}
fn zero_centers(count: usize) -> Vec<[f32; 3]> {
    vec![[0.; 3]; count]
}

#[test]
fn equal_cost_open_replacement_changes_parent_instead_of_retaining_first_route() {
    let neighbors = [
        [Some(1), Some(2), None],
        [Some(3), None, None],
        [Some(3), None, None],
        [None; 3],
    ];
    let centers = zero_centers(4);
    let weights = [Some([1.; 3]); 4];
    let result = select_triangle_path(
        request(0, 3),
        PathGraph {
            neighbors: &neighbors,
            centers: &centers,
            weights: &weights,
        },
    )
    .unwrap();
    let path = result.path.unwrap();
    assert_eq!(path.triangles, [0, 2, 3]);
    assert_eq!(path.search_cost, 2.);
    assert_eq!(
        (result.expanded, result.replacements, result.reopened),
        (4, 1, 0)
    );
}

#[test]
fn equal_scores_are_fifo_in_neighbor_insertion_order_without_face_id_sorting() {
    let neighbors = [[Some(2), Some(1), None], [None; 3], [None; 3]];
    let centers = zero_centers(3);
    let weights = [Some([1.; 3]); 3];
    let result = select_triangle_path(
        request(0, 1),
        PathGraph {
            neighbors: &neighbors,
            centers: &centers,
            weights: &weights,
        },
    )
    .unwrap();
    assert_eq!(result.expanded, 3);
    assert_eq!(result.path.unwrap().triangles, [0, 1]);
}

#[test]
fn equal_g_closed_record_is_reopened_with_a_new_parent() {
    let neighbors = [
        [Some(1), Some(2), None],
        [Some(3), None, None],
        [Some(1), None, None],
        [None; 3],
    ];
    let result = select_triangle_path_with_policy(
        request(0, 3),
        &neighbors,
        |_| Ok(true),
        |e| {
            Ok(match (e.from_triangle, e.to_triangle) {
                (1, 3) => 100.,
                (2, 1) => 0.,
                _ => 1.,
            })
        },
        |from, _| Ok(if from == 2 { 10. } else { 0. }),
    )
    .unwrap();
    assert_eq!((result.expanded, result.reopened), (5, 1));
    assert_eq!(result.path.unwrap().triangles, [0, 2, 1, 3]);
}

#[test]
fn boundary_stops_expansion_and_default_policy_blocks_immediate_backtracking() {
    let centers = zero_centers(2);
    let weights = [Some([1.; 3]); 2];
    let boundary = [[None, Some(1), None], [None; 3]];
    let result = select_triangle_path(
        request(0, 1),
        PathGraph {
            neighbors: &boundary,
            centers: &centers,
            weights: &weights,
        },
    )
    .unwrap();
    assert!(result.path.is_none());
    assert_eq!((result.expanded, result.edge_checks), (1, 0));
    let neighbors = [[Some(1), None, None], [Some(0), None, None], [None; 3]];
    let centers = zero_centers(3);
    let weights = [Some([0.; 3]); 3];
    let result = select_triangle_path(
        request(0, 2),
        PathGraph {
            neighbors: &neighbors,
            centers: &centers,
            weights: &weights,
        },
    )
    .unwrap();
    assert!(result.path.is_none());
    assert_eq!(
        (result.expanded, result.edge_checks, result.reopened),
        (2, 2, 0)
    );
}

#[test]
fn weights_override_distance_and_missing_weights_use_centers_with_small_distance_clamp() {
    let neighbors = [[Some(1), None, None], [None; 3]];
    let mut centers = [[0.; 3], [10., 0., 0.]];
    for (weight, expected) in [(Some([2.; 3]), 2.), (None, 10.)] {
        let weights = [weight, None];
        let result = select_triangle_path(
            request(0, 1),
            PathGraph {
                neighbors: &neighbors,
                centers: &centers,
                weights: &weights,
            },
        )
        .unwrap();
        assert_eq!(result.path.unwrap().search_cost, expected);
    }
    centers[1] = [0.0001, 0., 0.];
    let result = select_triangle_path(
        request(0, 1),
        PathGraph {
            neighbors: &neighbors,
            centers: &centers,
            weights: &[None; 2],
        },
    )
    .unwrap();
    assert_eq!(result.path.unwrap().search_cost, 0.);
    let t = SurfaceTriangle {
        index: 0,
        vertices: [[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]],
    };
    assert_eq!(surface_centroid(t).unwrap(), [4., 5., 6.]);
}

#[test]
fn host_budget_is_distinct_from_no_path_and_same_face_returns_a_single_node() {
    let neighbors = [[Some(1), None, None], [None; 3]];
    let centers = zero_centers(2);
    let weights = [Some([1.; 3]); 2];
    let mut req = request(0, 1);
    req.max_expansions = 1;
    assert!(
        select_triangle_path(
            req,
            PathGraph {
                neighbors: &neighbors,
                centers: &centers,
                weights: &weights
            }
        )
        .unwrap_err()
        .to_string()
        .contains("budget exhausted")
    );
    req.goal_triangle = 0;
    let result = select_triangle_path(
        req,
        PathGraph {
            neighbors: &neighbors,
            centers: &centers,
            weights: &weights,
        },
    )
    .unwrap();
    assert_eq!(result.path.unwrap().triangles, [0]);
    assert_eq!(result.expanded, 1);
}

#[test]
fn paths_can_span_multiple_original_sized_record_blocks() {
    let neighbors = (0..258)
        .map(|i| [if i < 257 { Some(i + 1) } else { None }, None, None])
        .collect::<Vec<_>>();
    let centers = zero_centers(258);
    let weights = vec![Some([1.; 3]); 258];
    let result = select_triangle_path(
        request(0, 257),
        PathGraph {
            neighbors: &neighbors,
            centers: &centers,
            weights: &weights,
        },
    )
    .unwrap();
    assert_eq!(result.expanded, 258);
    let path = result.path.unwrap();
    assert_eq!(path.triangles, (0..258).collect::<Vec<_>>());
    assert_eq!(path.search_cost, 257.);
}

#[test]
fn pool_prefers_newest_nonfull_block_and_parent_slots_follow_reuse() {
    let mut pool = Records::default();
    let node = |face| Node {
        face,
        parent: None,
        g: 0.,
        score: 0.,
    };
    let first = pool.allocate(node(0)).unwrap();
    for i in 1..257 {
        pool.allocate(node(i)).unwrap();
    }
    pool.release(first).unwrap();
    let replacement = pool.allocate(node(500)).unwrap();
    assert_ne!(replacement, first);
    assert_eq!(pool.get(first).unwrap().face, 0);
    for i in 0..254 {
        pool.allocate(node(1000 + i)).unwrap();
    }
    let reused = pool.allocate(node(999)).unwrap();
    assert_eq!(reused, first);
    assert_eq!(pool.get(first).unwrap().face, 999);
    let mut one_block = Records::default();
    let sole = one_block.allocate(node(7)).unwrap();
    one_block.release(sole).unwrap();
    assert!(one_block.get(sole).is_err()); // Whole-block heap reuse remains unverified.
}

#[test]
fn invalid_tables_callbacks_and_unsupported_self_expansion_report_errors() {
    let neighbors = [[Some(0), None, None]];
    assert!(
        select_triangle_path(
            request(0, 1),
            PathGraph {
                neighbors: &neighbors,
                centers: &[[0.; 3]],
                weights: &[None]
            }
        )
        .is_err()
    );
    let neighbors = [[Some(1), None, None], [None; 3]];
    let centers = zero_centers(2);
    assert!(
        select_triangle_path(
            request(0, 1),
            PathGraph {
                neighbors: &neighbors,
                centers: &centers,
                weights: &[None]
            }
        )
        .is_err()
    );
    assert!(
        select_triangle_path_with_policy(
            request(0, 1),
            &neighbors,
            |_| Ok(true),
            |_| Ok(f32::NAN),
            |_, _| Ok(0.)
        )
        .is_err()
    );
    assert!(
        select_triangle_path_with_policy(
            request(0, 1),
            &[[Some(0), None, None], [None; 3]],
            |_| Ok(true),
            |_| Ok(0.),
            |_, _| Ok(0.)
        )
        .is_err()
    );
    assert!(
        select_triangle_path_with_policy(
            request(0, 1),
            &[[Some(usize::MAX), None, None], [None; 3]],
            |_| Ok(true),
            |_| Ok(0.),
            |_, _| Ok(0.)
        )
        .is_err()
    );
}
