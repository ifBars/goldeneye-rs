use super::*;
fn build(
    faces: &[[usize; 3]],
    neighbors: &[[Option<usize>; 3]],
    flags: &[Option<u32>],
) -> Result<BoundaryCache> {
    build_boundary_cache(BoundaryMesh {
        vertex_count: 20,
        faces,
        neighbors,
        flags,
    })
}
fn pairs(cache: &BoundaryCache) -> Vec<[usize; 2]> {
    cache.edges().iter().map(|e| e.vertices).collect()
}
#[test]
fn flagged_faces_emit_all_ordered_edges_and_keep_duplicates_while_isolated_faces_emit_none() {
    let f = [[0, 1, 2], [0, 1, 2]];
    let n = [[None; 3]; 2];
    let cache = build(&f, &n, &[Some(1), Some(0x81)]).unwrap();
    assert_eq!(
        pairs(&cache),
        vec![[0, 1], [1, 2], [2, 0], [0, 1], [1, 2], [2, 0]]
    );
    assert_eq!(cache.stats().flagged_faces, 2);
    let cache = build(&f, &n, &[None, Some(2)]).unwrap();
    assert_eq!(cache.stats().isolated_faces, 2);
    assert!(cache.edges().is_empty());
}
#[test]
fn one_neighbor_compacts_all_slots_and_emits_from_last_exclusive_vertex() {
    let f = [[0, 1, 2], [0, 1, 3]];
    let n = [[None, Some(1), None], [None; 3]];
    assert_eq!(
        pairs(&build(&f, &n, &[None; 2]).unwrap()),
        vec![[2, 0], [2, 1]]
    );
    let f = [[0, 1, 2], [3, 4, 5]];
    assert_eq!(
        pairs(&build(&f, &n, &[None; 2]).unwrap()),
        vec![[2, 0], [2, 1]]
    );
}
#[test]
fn two_neighbors_choose_their_common_vertex_or_first_current_match_and_keep_fallback_order() {
    let n = [[Some(1), None, Some(2)], [None; 3], [None; 3]];
    let f = [[0, 1, 2], [0, 1, 3], [0, 2, 4]];
    assert_eq!(pairs(&build(&f, &n, &[None; 3]).unwrap()), vec![[1, 2]]);
    let f = [[0, 1, 2], [3, 1, 0], [0, 1, 4]]; // Two shared IDs; current vertex order wins.
    assert_eq!(pairs(&build(&f, &n, &[None; 3]).unwrap()), vec![[1, 2]]);
    let f = [[0, 1, 2], [3, 4, 5], [6, 7, 8]];
    let cache = build(&f, &n, &[None; 3]).unwrap();
    assert_eq!(pairs(&cache), vec![[0, 2]]);
    assert_eq!(cache.stats().missing_exclusions, 1);
}
#[test]
fn three_neighbor_faces_are_ignored_unless_flagged_and_bad_initialized_cases_are_errors() {
    let f = [[0, 1, 2]];
    assert!(
        build(&f, &[[Some(0); 3]], &[None])
            .unwrap()
            .edges()
            .is_empty()
    );
    assert_eq!(
        build(&f, &[[Some(0); 3]], &[Some(1)])
            .unwrap()
            .stats()
            .edges,
        3
    );
    assert!(build(&f, &[[Some(0), None, None]], &[None]).is_err());
    assert!(build(&f, &[], &[None]).is_err());
    assert!(build(&f, &[[Some(9), None, None]], &[None]).is_err());
    assert!(build(&[[0, 1, 21]], &[[None; 3]], &[Some(1)]).is_err());
}
#[test]
fn intersections_include_endpoints_ignore_height_and_preserve_collinear_disjoint_acceptance() {
    let a = [0., 500., 0.];
    let b = [2., -500., 0.];
    assert_eq!(
        intersection([1., 0., -1.], [1., 0., 1.], a, b).unwrap(),
        Some(([1., 0., 0.], false))
    );
    assert!(
        intersection([0., 0., -1.], [0., 0., 0.], a, b)
            .unwrap()
            .is_some()
    );
    assert!(
        intersection([2., 0., 0.], [2., 0., 1.], a, b)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        intersection([4., 0., 0.], [5., 0., 0.], a, b).unwrap(),
        Some(([0.; 3], true))
    );
    assert!(
        intersection([0., 0., 1.], [2., 0., 1.], a, b)
            .unwrap()
            .is_none()
    );
    assert!(
        intersection([4., 0., -1.], [4., 0., 1.], a, b)
            .unwrap()
            .is_none()
    );
}
#[test]
fn minimum_normal_cutoff_degenerate_points_and_first_cache_order_are_preserved() {
    let tiny = f32::MIN_POSITIVE;
    let hit = intersection([0., 0., 0.], [tiny, 0., 0.], [0., 0., 0.], [0., 0., 1.])
        .unwrap()
        .unwrap();
    assert!(!hit.1); // Exactly MIN_POSITIVE uses division, not the collinear branch.
    assert!(
        intersection([0.; 3], [tiny / 2., 0., 0.], [0.; 3], [0., 0., 1.])
            .unwrap()
            .unwrap()
            .1
    );
    assert!(
        intersection([4., 0., 4.], [4., 0., 4.], [0.; 3], [0.; 3])
            .unwrap()
            .unwrap()
            .1
    );
    let cache = build(&[[0, 1, 2]], &[[None; 3]], &[Some(1)]).unwrap();
    let v = [[0.; 3], [2., 0., 0.], [0., 0., 2.]];
    let hit = cache
        .first_blocker(&v, [-1., 9., 0.], [3., -9., 0.])
        .unwrap()
        .unwrap();
    assert_eq!(hit.edge, 0);
    assert!(hit.collinear);
    assert_eq!(hit.position, [0.; 3]);
    assert!(
        cache
            .first_blocker(&v, [20., 0., -1.], [20., 0., 1.])
            .unwrap()
            .is_none()
    );
}
#[test]
fn invalid_query_data_and_overflow_are_errors() {
    assert!(intersection([f32::NAN; 3], [0.; 3], [0.; 3], [0.; 3]).is_err());
    assert!(intersection([-f32::MAX; 3], [f32::MAX; 3], [0.; 3], [1.; 3]).is_err());
    let cache = build(&[[0, 1, 2]], &[[None; 3]], &[Some(1)]).unwrap();
    assert!(cache.first_blocker(&[], [0.; 3], [1.; 3]).is_err());
}
