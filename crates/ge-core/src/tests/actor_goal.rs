use super::*;
struct Fixture {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
    centers: Vec<[f32; 3]>,
    neighbors: Vec<[Option<usize>; 3]>,
    flags: Vec<Option<u32>>,
    normals: Vec<Option<[f32; 3]>>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            vertices: vec![[0., 0., 0.], [10., 0., 0.], [0., 0., 10.], [10., 0., 10.]],
            triangles: vec![[0, 1, 2], [1, 3, 2]],
            centers: vec![[10. / 3., 0., 10. / 3.], [20. / 3., 0., 20. / 3.]],
            neighbors: vec![[None, None, Some(1)], [Some(0), None, None]],
            flags: vec![None; 2],
            normals: vec![None; 2],
        }
    }
    fn mesh(&self) -> ActorGoalMesh<'_> {
        ActorGoalMesh {
            vertices: &self.vertices,
            triangles: &self.triangles,
            centers: &self.centers,
            neighbors: &self.neighbors,
            flags: &self.flags,
            normals: &self.normals,
        }
    }
    fn context(&self) -> ActorGoalContext {
        ActorGoalContext {
            origin: [1., 0., 1.],
            point: self.centers[1],
            start_triangle: 0,
            last_center: None,
        }
    }
}
#[test]
fn distance_is_full_3d_inclusive_and_updates_scratch_before_early_rejection() {
    let mut f = Fixture::new();
    f.normals = vec![Some([f32::NAN, 0.8, f32::NAN])];
    let mut c = f.context();
    c.origin = [f32::NAN; 3];
    let r = actor_target_goal(f.mesh(), &mut c, 0, 10).unwrap();
    assert!(matches!(r.decision, ActorGoalDecision::TooFar { .. }));
    assert_eq!(c.last_center, Some(f.centers[0]));
    assert!(r.walked_faces.is_empty());
    c.origin = [1., 0., 1.];
    c.point = f.centers[1];
    c.point[1] += 2.;
    let r = actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap();
    assert!(matches!(
        r.decision,
        ActorGoalDecision::NormalY { triangle: 0, .. }
    ));
    c.point[1] = f32::from_bits(2f32.to_bits() + 1);
    assert!(matches!(
        actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap().decision,
        ActorGoalDecision::TooFar { .. }
    ));
}
#[test]
fn walk_skips_boundary_slots_uses_raw_height_and_does_not_gate_arrived_normal() {
    let mut f = Fixture::new();
    f.flags = vec![Some(3), Some(2)]; // Source flags ignored; only destination bit zero gates.
    f.normals = vec![Some([f32::NAN, 0.9, f32::NAN]), Some([0., -100., 0.])];
    let mut c = f.context();
    c.origin[1] = 1000.;
    let r = actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap();
    assert!(r.accepted());
    assert_eq!(r.walked_faces, [0, 1]);
    assert_eq!(r.edge_tests, 2);
    f.flags[1] = Some(1);
    let r = actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap();
    assert!(matches!(
        r.decision,
        ActorGoalDecision::NoNeighbor {
            triangle: 0,
            edge: [1, 2]
        }
    ));
    f.flags[1] = None;
    f.normals[0] = Some([0., 0.8999, 0.]);
    assert!(matches!(
        actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap().decision,
        ActorGoalDecision::NormalY { triangle: 0, .. }
    ));
}
#[test]
fn initial_face_uses_first_eligible_crossing_instead_of_nearest() {
    let mut f = Fixture::new();
    f.centers[1] = [15., 0., 1.];
    let mut c = f.context();
    c.origin = [-1., 0., 1.];
    c.point = f.centers[1];
    let r = actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap();
    assert!(r.accepted());
    assert_eq!(r.edge_tests, 2); // Exit edge precedes the nearer entry in storage.
}
#[test]
fn later_faces_choose_a_greater_crossing_and_skip_previous_neighbor() {
    for ids in [[0, 1, 2], [0, 2, 1]] {
        let mut f = Fixture::new();
        f.triangles[1] = ids;
        f.triangles.push([1, 3, 2]);
        f.centers.push([15., 0., 1.]);
        f.normals.push(None);
        f.flags.push(None);
        f.neighbors = vec![[Some(1), None, None], [Some(0), None, Some(2)], [None; 3]];
        let mut c = f.context();
        c.point = f.centers[2];
        c.origin = [-1., 0., 1.];
        let r = actor_target_goal(f.mesh(), &mut c, 2, 10).unwrap();
        assert!(r.accepted());
        assert_eq!(r.walked_faces, [0, 1, 2]);
        assert!(
            actor_target_goal(f.mesh(), &mut c, 2, 1)
                .unwrap_err()
                .to_string()
                .contains("budget exhausted")
        );
    }
}
#[test]
fn first_neighbor_with_exactly_two_matching_occurrences_wins() {
    let mut f = Fixture::new();
    f.triangles.push([1, 2, 3]);
    f.normals.push(Some([0., 0.8, 0.]));
    f.flags.push(None);
    f.neighbors[0] = [Some(2), None, Some(1)];
    let mut c = f.context();
    let r = actor_target_goal(f.mesh(), &mut c, 1, 10).unwrap();
    assert_eq!(r.walked_faces, [0, 2]);
    assert!(matches!(
        r.decision,
        ActorGoalDecision::NormalY { triangle: 2, .. }
    ));
    // Repeated IDs are counted as occurrences, rather than deduplicated.
    f.triangles[2] = [1, 1, 1];
    assert!(
        actor_target_goal(f.mesh(), &mut c, 1, 10)
            .unwrap()
            .accepted()
    );
}
#[test]
fn determinant_threshold_is_inclusive_edge_parameters_are_closed_and_forward_has_no_maximum() {
    for height in [0., 0.004] {
        let mut f = Fixture::new();
        f.vertices = vec![[0.5, 0., 0.], [0.5, 0., 0.004], [0., 0., 0.004]];
        f.triangles = vec![[0, 1, 2], [0, 1, 2]];
        f.neighbors[0] = [Some(1), None, None];
        f.centers[1] = [0.25, 0., height];
        let mut c = f.context();
        c.origin = [0., 0., height];
        c.point = f.centers[1];
        assert!(
            actor_target_goal(f.mesh(), &mut c, 1, 10)
                .unwrap()
                .accepted()
        ); // t=2 is admitted.
        let below = f32::from_bits(0.004f32.to_bits() - 1);
        f.vertices[1][2] = below;
        f.vertices[2][2] = below;
        assert!(actor_target_goal(f.mesh(), &mut c, 1, 10).is_err());
    }
}
#[test]
fn no_entry_shortcut_missing_crossings_and_malformed_inputs_remain_errors() {
    let mut f = Fixture::new();
    let mut c = f.context();
    c.origin = f.centers[0];
    c.point = f.centers[0];
    assert!(
        actor_target_goal(f.mesh(), &mut c, 0, 10)
            .unwrap_err()
            .to_string()
            .contains("no supported forward crossing")
    );
    c = f.context();
    assert!(actor_target_goal(f.mesh(), &mut c, 1, 0).is_err());
    assert!(actor_target_goal(f.mesh(), &mut c, 99, 10).is_err());
    f.vertices[1][0] = f32::NAN;
    assert!(actor_target_goal(f.mesh(), &mut c, 1, 10).is_err());
    f.vertices[1][0] = 10.;
    f.neighbors[0] = [Some(999), None, None];
    assert!(actor_target_goal(f.mesh(), &mut c, 1, 10).is_err());
}
