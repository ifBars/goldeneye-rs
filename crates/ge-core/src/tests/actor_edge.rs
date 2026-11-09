use super::*;
fn edge() -> TraversalEdge {
    TraversalEdge {
        from_triangle: 0,
        to_triangle: 1,
        slot: 0,
        previous_triangle: None,
    }
}
fn mesh<'a>(
    centers: &'a [[f32; 3]],
    flags: &'a [Option<u32>],
    normals: &'a [Option<[f32; 3]>],
) -> ActorEdgeMesh<'a> {
    ActorEdgeMesh {
        neighbors: &[[Some(1), None, None], [None; 3]],
        centers,
        flags,
        normals,
    }
}
#[test]
fn backtracking_and_low_flags_reject_before_unused_tables_or_positions() {
    let mut e = edge();
    e.previous_triangle = Some(1);
    assert_eq!(
        actor_target_edge(mesh(&[], &[], &[]), e, [f32::NAN; 3]).unwrap(),
        ActorEdgeDecision::ImmediateBacktrack
    );
    for flags in [1, 2, 3, 0x81] {
        assert_eq!(
            actor_target_edge(mesh(&[], &[None, Some(flags)], &[]), edge(), [f32::NAN; 3]).unwrap(),
            ActorEdgeDecision::FaceFlags { flags_raw: flags }
        );
    }
}
#[test]
fn optional_normal_uses_only_destination_y_with_inclusive_point_nine() {
    let c = [[0.; 3], [1., 0., 0.]];
    let d = actor_target_edge(
        mesh(&c, &[Some(3), Some(4)], &[None, Some([0., 0.8999, 0.])]),
        edge(),
        [f32::NAN; 3],
    )
    .unwrap();
    assert!(matches!(d, ActorEdgeDecision::NormalY { .. }));
    for normal in [None, Some([f32::NAN, 0.9, f32::NAN]), Some([0., 1., 0.])] {
        assert!(
            actor_target_edge(
                mesh(&c, &[Some(3), Some(4)], &[None, normal]),
                edge(),
                [1., 0., 0.]
            )
            .unwrap()
            .allowed()
        );
    }
}
#[test]
fn full_3d_direction_rejects_away_accepts_sideways_and_keeps_zero_target() {
    for (to, point, allowed) in [
        ([1., 0., 0.], [-1., 0., 0.], false),
        ([1., 0., 0.], [0., 0., 1.], true),
        ([0., 1., 0.], [0., -1., 0.], false),
        ([1., 0., 0.], [0.; 3], true),
    ] {
        let c = [[0.; 3], to];
        let d = actor_target_edge(mesh(&c, &[None; 2], &[None; 2]), edge(), point).unwrap();
        assert_eq!(d.allowed(), allowed);
    }
}
#[test]
fn short_vectors_remain_unscaled_and_dot_threshold_is_strict_engine_constant() {
    assert_eq!(normalize([1e-5, 0., 0.]).unwrap(), [1e-5, 0., 0.]);
    assert_eq!(normalize([2e-5, 0., 0.]).unwrap(), [1., 0., 0.]);
    let d = actor_target_edge(
        mesh(&[[0.; 3], [1e-6, 0., 0.]], &[None; 2], &[None; 2]),
        edge(),
        [-1., 0., 0.],
    )
    .unwrap();
    assert!(matches!(d,ActorEdgeDecision::Direction{dot,allowed:true} if dot==-1e-6));
    assert_eq!(ACTOR_EDGE_DOT_MIN.to_bits(), 0xbeaf1d42);
    for (bits, allowed) in [(0xbeaf1d42, false), (0xbeaf1d43, false), (0xbeaf1d41, true)] {
        let decision = direction_decision(f32::from_bits(bits));
        assert!(matches!(decision,ActorEdgeDecision::Direction{dot,..} if dot.to_bits()==bits));
        assert_eq!(decision.allowed(), allowed);
    }
}
#[test]
fn malformed_slots_tables_and_used_numeric_values_are_errors() {
    let c = [[0.; 3], [1., 0., 0.]];
    let mut e = edge();
    e.slot = 1;
    assert!(actor_target_edge(mesh(&c, &[None; 2], &[None; 2]), e, [0.; 3]).is_err());
    assert!(actor_target_edge(mesh(&[], &[None; 2], &[None; 2]), edge(), [0.; 3]).is_err());
    assert!(
        actor_target_edge(
            mesh(&c, &[None; 2], &[None, Some([0., f32::NAN, 0.])]),
            edge(),
            [0.; 3]
        )
        .is_err()
    );
    assert!(
        actor_target_edge(
            mesh(&[[f32::MAX; 3], [-f32::MAX; 3]], &[None; 2], &[None; 2]),
            edge(),
            [0.; 3]
        )
        .is_err()
    );
}
