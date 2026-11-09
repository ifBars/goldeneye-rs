use super::*;
fn path(ids: &[usize]) -> TrianglePath {
    TrianglePath {
        triangles: ids.to_vec(),
        search_cost: 1.,
    }
}
fn ready() -> NavigationRouteState {
    let mut state = NavigationRouteState::default();
    assert!(
        state
            .regenerate(
                true,
                || Ok(NavigationRouteSearch::Found(path(&[1, 2, 3]))),
                |_| Ok(vec![[1.; 3], [2.; 3], [3.; 3]])
            )
            .unwrap()
    );
    state
}
#[test]
fn all_normal_failures_replace_status_but_preserve_existing_targets_and_triangles() {
    let mut state = ready();
    for (outcome, status) in [
        (NavigationRouteSearch::MissingStart, 6),
        (NavigationRouteSearch::MissingGoal, 10),
        (NavigationRouteSearch::NoRoute, 2),
    ] {
        let mut outcome = Some(outcome);
        assert!(
            !state
                .regenerate(
                    true,
                    || Ok(outcome.take().unwrap()),
                    |_| panic!("miss must not rebuild points")
                )
                .unwrap()
        );
        assert_eq!(state.status_raw(), status);
        assert_eq!(state.triangles(), &[1, 2, 3]);
        assert_eq!(state.points(), &[[1.; 3], [2.; 3], [3.; 3]]);
    }
    assert!(
        !state
            .regenerate(
                false,
                || panic!("missing mesh must not search"),
                |_| panic!()
            )
            .unwrap()
    );
    assert_eq!(state.status_raw(), 1);
    assert_eq!(state.points().len(), 3);
    let route = super::super::WaypointRoute::new(state.points()).unwrap();
    assert_eq!(route.target(), Some([2.; 3]));
}
#[test]
fn a_later_success_replaces_both_lists_instead_of_popping_or_appending() {
    let mut state = ready();
    state
        .regenerate(
            true,
            || Ok(NavigationRouteSearch::MissingStart),
            |_| panic!(),
        )
        .unwrap();
    assert!(
        state
            .regenerate(
                true,
                || Ok(NavigationRouteSearch::Found(path(&[9]))),
                |p| {
                    assert_eq!(p.triangles, [9]);
                    Ok(vec![[10.; 3], [11.; 3]])
                }
            )
            .unwrap()
    );
    assert_eq!(state.status_raw(), 0);
    assert_eq!(state.triangles(), &[9]);
    assert_eq!(state.points(), &[[10.; 3], [11.; 3]]);
}
#[test]
fn host_errors_preserve_the_stage_already_reached_without_becoming_normal_misses() {
    let mut state = ready();
    state.regenerate(false, || panic!(), |_| panic!()).unwrap();
    assert!(
        state
            .regenerate(true, || anyhow::bail!("query budget"), |_| panic!())
            .is_err()
    );
    assert_eq!(state.status_raw(), 0);
    assert_eq!(state.triangles(), &[1, 2, 3]);
    assert_eq!(state.points().len(), 3);
    assert!(
        state
            .regenerate(
                true,
                || Ok(NavigationRouteSearch::Found(path(&[8, 9]))),
                |_| anyhow::bail!("waypoint budget")
            )
            .is_err()
    );
    assert_eq!(state.triangles(), &[8, 9]);
    assert!(state.points().is_empty());
    assert_eq!(state.status_raw(), 0);
    for points in [vec![], vec![[f32::NAN; 3]; 2]] {
        assert!(
            state
                .regenerate(
                    true,
                    || Ok(NavigationRouteSearch::Found(path(&[8]))),
                    |_| Ok(points.clone())
                )
                .is_err()
        );
        assert!(state.points().is_empty());
    }
    assert!(
        state
            .regenerate(
                true,
                || Ok(NavigationRouteSearch::Found(path(&[]))),
                |_| panic!()
            )
            .is_err()
    );
}
#[test]
fn ordinary_world_mapping_gives_start_miss_priority_and_rejects_inconsistent_results() {
    use super::super::{PathSearchResult, SurfaceHit, WorldEndpoint};
    let endpoint = Some(WorldEndpoint {
        local_hit: SurfaceHit {
            normal: [0., 1., 0.],
            triangle: 0,
            parameter: 0.,
            position: [0.; 3],
        },
        world_surface_position: [0.; 3],
    });
    let mut world = WorldPathResult {
        start_endpoint: None,
        goal_endpoint: None,
        path_search: None,
    };
    assert!(matches!(
        NavigationRouteSearch::from_world(&world).unwrap(),
        NavigationRouteSearch::MissingStart
    ));
    world.start_endpoint = endpoint;
    assert!(matches!(
        NavigationRouteSearch::from_world(&world).unwrap(),
        NavigationRouteSearch::MissingGoal
    ));
    world.goal_endpoint = endpoint;
    assert!(NavigationRouteSearch::from_world(&world).is_err());
    world.path_search = Some(PathSearchResult::default());
    assert!(matches!(
        NavigationRouteSearch::from_world(&world).unwrap(),
        NavigationRouteSearch::NoRoute
    ));
    world.path_search.as_mut().unwrap().path = Some(path(&[4]));
    assert!(matches!(
        NavigationRouteSearch::from_world(&world).unwrap(),
        NavigationRouteSearch::Found(_)
    ));
    world.start_endpoint = None;
    assert!(NavigationRouteSearch::from_world(&world).is_err());
}
