use super::*;

fn limits() -> WaypointLimits {
    WaypointLimits {
        max_shortcut_tests: 100,
        max_fan_steps: 100,
    }
}
struct Fixture {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
    centers: Vec<[f32; 3]>,
    neighbors: Vec<[Option<usize>; 3]>,
    flags: Vec<Option<u32>>,
}
impl Fixture {
    fn portal(half_width: f32) -> Self {
        Self {
            vertices: vec![
                [1., 0., -half_width],
                [1., 0., half_width],
                [0., 0., 0.],
                [2., 0., 0.],
                [10., 0., 10.],
                [11., 0., 10.],
                [10., 0., 11.],
            ],
            triangles: vec![[4, 5, 6], [0, 1, 2], [1, 0, 3]],
            centers: vec![[10., 0., 10.], [0.5, 0., 0.], [1.5, 0., 0.]],
            neighbors: vec![[None; 3], [Some(2), None, None], [Some(1), None, None]],
            flags: vec![None; 3],
        }
    }
    fn mesh(&self) -> WaypointMesh<'_> {
        WaypointMesh {
            vertices: &self.vertices,
            triangles: &self.triangles,
            centers: &self.centers,
            neighbors: &self.neighbors,
            flags: &self.flags,
        }
    }
}

#[test]
fn seed_keeps_original_endpoints_and_only_interior_face_centers() {
    let mut f = Fixture::portal(1.);
    f.flags[1] = Some(1); // Reject the only shortcut, retaining the seed line.
    let result = build_waypoints(
        f.mesh(),
        &[0, 1, 2],
        [0., 0.25, 0.],
        [2., 0.75, 0.],
        limits(),
    )
    .unwrap();
    assert_eq!(
        result.points,
        [[0., 0.25, 0.], [0.5, 0., 0.], [2., 0.75, 0.]]
    );
    assert_eq!(result.retained_indices, [0, 1, 2]);
    assert_eq!(result.flag_rejections, 1);
    for chain in [&[][..], &[0][..], &[0, 1][..]] {
        let same = [0.; 3];
        let r = build_waypoints(f.mesh(), chain, same, same, limits()).unwrap();
        assert_eq!(r.points, [same, same]); // No deduplication, including same-face requests.
    }
}

#[test]
fn open_strip_shortcut_skips_first_portal_and_endpoint_flags() {
    let mut f = Fixture::portal(1.);
    f.flags[0] = Some(1);
    f.flags[2] = Some(1);
    let r = build_waypoints(f.mesh(), &[0, 1, 2], [0., 0., 0.], [2., 0., 0.], limits()).unwrap();
    // Face zero shares no vertex with face one. The observed check starts at 1->2.
    assert_eq!(r.points, [[0., 0., 0.], [2., 0., 0.]]);
    assert_eq!(r.retained_indices, [0, 2]);
    assert_eq!(
        (r.shortcut_tests, r.fan_steps, r.flag_rejections),
        (1, 0, 0)
    );
}

#[test]
fn portal_tests_exclude_start_parameters_include_end_and_preserve_determinant_cutoff() {
    let s = [0., 0., 0.];
    let t = [1., 0., 0.];
    assert!(intersects_xz(s, t, [0.5, 0., -1.], [0.5, 0., 1.]).unwrap());
    assert!(!intersects_xz(s, t, [0., 0., -1.], [0., 0., 1.]).unwrap());
    assert!(intersects_xz(s, t, [1., 0., -1.], [1., 0., 1.]).unwrap());
    assert!(!intersects_xz(s, t, [0.5, 0., 0.], [0.5, 0., 1.]).unwrap());
    assert!(intersects_xz(s, t, [0.5, 0., -1.], [0.5, 0., 0.]).unwrap());
    assert!(intersects_xz(s, t, [0.5, 0., -0.0005], [0.5, 0., 0.0005]).unwrap());
    assert!(!intersects_xz(s, t, [0.5, 0., -0.00025], [0.5, 0., 0.00025]).unwrap());
    assert!(!intersects_xz(s, t, [0., 0., 0.], [1., 0., 0.]).unwrap());
}

#[test]
fn shared_edge_uses_ids_in_first_face_order_and_missing_edge_skips_constraint() {
    assert_eq!(shared_edge([7, 4, 9], [9, 7, 8]), Some([7, 9]));
    assert_eq!(shared_edge([7, 4, 9], [1, 2, 3]), None);
    assert_eq!(shared_edge([7, 4, 9], [7, 7, 8]), Some([7, 7]));
    let mut f = Fixture::portal(0.1);
    f.triangles[2] = [4, 5, 6]; // No portal: the observed routine continues, not rejects.
    let r = build_waypoints(f.mesh(), &[0, 1, 2], [0., 0., 0.], [2., 0., 0.], limits()).unwrap();
    assert_eq!(r.retained_indices, [0, 2]);
}

#[test]
fn half_unit_clearance_is_inclusive_and_uses_full_3d_distance() {
    for half_width in [0.1, 0.5] {
        let f = Fixture::portal(half_width);
        let r =
            build_waypoints(f.mesh(), &[0, 1, 2], [0., 0., 0.], [2., 0., 0.], limits()).unwrap();
        assert_eq!(r.retained_indices, [0, 1, 2]); // Open vertex fan rejects shortcut.
        assert!(r.fan_steps > 0);
    }
    let f = Fixture::portal(0.5001);
    assert_eq!(
        build_waypoints(f.mesh(), &[0, 1, 2], [0., 0., 0.], [2., 0., 0.], limits())
            .unwrap()
            .retained_indices,
        [0, 2]
    );
    let f = Fixture::portal(0.1);
    assert_eq!(
        build_waypoints(f.mesh(), &[0, 1, 2], [0., 1., 0.], [2., 1., 0.], limits())
            .unwrap()
            .retained_indices,
        [0, 2]
    );
}

#[test]
fn closed_fans_skip_boundary_slots_and_work_exhaustion_is_an_error() {
    let mut f = Fixture::portal(0.1);
    f.triangles.push([0, 1, 4]);
    f.centers.push([0.; 3]);
    f.flags.push(Some(1));
    f.neighbors[2] = [Some(1), None, Some(3)];
    f.neighbors.push([Some(2), None, Some(1)]);
    let r = build_waypoints(f.mesh(), &[0, 1, 2], [0., 0., 0.], [2., 0., 0.], limits()).unwrap();
    assert_eq!(r.retained_indices, [0, 2]);
    assert_eq!(r.fan_steps, 4);
    let error = build_waypoints(
        f.mesh(),
        &[0, 1, 2],
        [0., 0., 0.],
        [2., 0., 0.],
        WaypointLimits {
            max_fan_steps: 3,
            ..limits()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("fan budget exhausted"));
    assert!(
        build_waypoints(
            f.mesh(),
            &[0, 1, 2],
            [0., 0., 0.],
            [2., 0., 0.],
            WaypointLimits {
                max_shortcut_tests: 0,
                ..limits()
            }
        )
        .is_err()
    );
}

#[test]
fn greedy_reduction_stops_at_first_failed_candidate_instead_of_trying_farther() {
    let mut f = Fixture::portal(1.);
    // Seed candidate 2 crosses the portal's line outside the portal segment.
    // A farther candidate would cross both portals, but must never be tried from 0.
    f.vertices.extend([[1., 0., -2.], [1., 0., 2.]]);
    f.triangles[1] = [0, 1, 7];
    f.triangles[2] = [1, 0, 8];
    f.triangles.push([0, 1, 3]);
    f.centers[2] = [1., 0., 2.];
    f.centers.push([2., 0., 0.]);
    f.neighbors.push([None; 3]);
    f.flags.push(None);
    let r = build_waypoints(
        f.mesh(),
        &[0, 1, 2, 3],
        [0., 0., 0.],
        [2., 0., 0.],
        limits(),
    )
    .unwrap();
    assert_eq!(r.retained_indices[1], 1);
}

#[test]
fn malformed_geometry_and_numerical_overflow_fail_explicitly() {
    let mut f = Fixture::portal(1.);
    assert!(build_waypoints(f.mesh(), &[99], [0.; 3], [1.; 3], limits()).is_err());
    assert!(build_waypoints(f.mesh(), &[0], [f32::NAN; 3], [1.; 3], limits()).is_err());
    f.triangles[1][0] = usize::MAX;
    assert!(build_waypoints(f.mesh(), &[0, 1, 2], [0.; 3], [1.; 3], limits()).is_err());
    assert!(
        intersects_xz(
            [f32::MAX, 0., 0.],
            [-f32::MAX, 0., 0.],
            [1., 0., -1.],
            [1., 0., 1.]
        )
        .is_err()
    );
}

#[test]
fn vertex_fan_keeps_neighbor_order_and_reports_a_nonclosing_cycle_as_budget_error() {
    let mut f = Fixture::portal(0.1);
    f.triangles.extend([[0, 1, 4], [0, 1, 5]]);
    f.centers.extend([[0.; 3]; 2]);
    f.flags.extend([None; 2]);
    f.neighbors[2] = [Some(3), Some(1), None];
    f.neighbors
        .extend([[Some(4), Some(2), None], [Some(2), Some(3), None]]);
    let error = build_waypoints(
        f.mesh(),
        &[0, 1, 2],
        [0., 0., 0.],
        [2., 0., 0.],
        WaypointLimits {
            max_fan_steps: 5,
            ..limits()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("fan budget exhausted"));
}
