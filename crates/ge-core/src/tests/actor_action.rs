use super::*;
fn owner(yaw: f32) -> ActorTargetOwner {
    ActorTargetOwner {
        frame: NavigationFrame {
            translation: [0.; 3],
        },
        yaw,
    }
}
fn state() -> ActorTargetActionState {
    ActorTargetActionState {
        state_raw: 99,
        active_index: 0,
        mode_raw: 0,
        target: [0., 0., 10.],
        records: vec![ActorTargetRecord {
            position: [0.; 3],
            motion: [7., 8., 9.],
        }],
    }
}
#[test]
fn angular_gate_has_five_pairs_and_mode_swaps_only_the_asymmetric_pair() {
    for index in [0, 2, 3, 4] {
        for mode in [0, 1, 128, 255] {
            let gate = actor_target_gate(owner(0.), [0., 0., 10.], index, mode).unwrap();
            assert_eq!(gate.lower.to_bits(), 0x3f490fdb);
            assert_eq!(gate.upper, gate.lower);
            assert!(gate.allowed);
        }
    }
    let gate = actor_target_gate(owner(0.), [0., 0., 10.], 1, 0).unwrap();
    assert_eq!(
        (gate.lower.to_bits(), gate.upper.to_bits()),
        (0x3edf66f3, 0x3e5f66f3)
    );
    let swapped = actor_target_gate(owner(0.), [0., 0., 10.], 1, 255).unwrap();
    assert_eq!((swapped.lower, swapped.upper), (gate.upper, gate.lower));
    assert!(
        !actor_target_gate(owner(0.), [0.3, 0., 1.], 1, 0)
            .unwrap()
            .allowed
    );
    assert!(
        actor_target_gate(owner(0.), [0.3, 0., 1.], 1, 1)
            .unwrap()
            .allowed
    );
    assert!(
        actor_target_gate(owner(0.), [-0.3, 0., 1.], 1, 0)
            .unwrap()
            .allowed
    );
    assert!(
        !actor_target_gate(owner(0.), [-0.3, 0., 1.], 1, 1)
            .unwrap()
            .allowed
    );
    assert!(actor_target_gate(owner(0.), [0., 0., 1.], 5, 0).is_err());
}
#[test]
fn angular_limits_are_strict_and_wrap_helpers_preserve_pi_ties_and_rounded_tau() {
    let lower = f32::from_bits(0x3edf66f3);
    let upper = f32::from_bits(0x3e5f66f3);
    assert!(!in_band(-lower, lower, upper));
    assert!(!in_band(upper, lower, upper));
    assert!(in_band(-f32::from_bits(lower.to_bits() - 1), lower, upper));
    assert!(in_band(f32::from_bits(upper.to_bits() - 1), lower, upper));
    assert_eq!(angle_difference(0., PI), -PI);
    assert_eq!(angle_difference(PI, 0.), PI);
    assert_eq!(positive_turn(-f32::MIN_POSITIVE), TAU); // No second clamp to zero.
    let y = actor_target_gate(owner(PI), [0., 0., -1.], 0, 0).unwrap();
    assert_eq!(y.normalized_yaw, -PI);
    assert!(y.allowed);
    assert_eq!(
        actor_target_gate(owner(-PI), [0., 0., -1.], 0, 0)
            .unwrap()
            .normalized_yaw,
        PI
    );
    let g = actor_target_gate(
        owner(TAU * 8. + 0.25),
        [0.25f32.sin(), 0., 0.25f32.cos()],
        0,
        0,
    )
    .unwrap();
    assert!(g.allowed && g.difference.abs() < 1e-5);
}
#[test]
fn bearing_uses_translated_x_z_and_ignores_unused_height() {
    let o = ActorTargetOwner {
        frame: NavigationFrame {
            translation: [10., f32::NAN, 20.],
        },
        yaw: 0.,
    };
    let g = actor_target_gate(o, [10., f32::NAN, 30.], 0, 0).unwrap();
    assert_eq!(g.bearing, 0.);
    assert!(g.allowed);
    assert!(
        actor_target_gate(owner(PI / 2.), [10., 0., 0.], 0, 0)
            .unwrap()
            .allowed
    );
    assert!(
        !actor_target_gate(owner(0.), [10., 0., 0.], 0, 0)
            .unwrap()
            .allowed
    );
}
#[test]
fn failed_gate_does_not_touch_records_or_query_and_return_is_not_state() {
    let mut s = state();
    s.target = [10., f32::NAN, 0.];
    s.records.clear();
    let r = run_actor_target_action(&mut s, owner(0.), 100, |_| {
        panic!("gated query must be lazy")
    })
    .unwrap();
    assert_eq!(r.return_code, 1);
    assert!(!r.gate.allowed);
    assert_eq!(s.state_raw, 99);
    assert!(s.records.is_empty());
    let mut s = state();
    let before = s.clone();
    let mut called = false;
    let r = run_actor_target_action(&mut s, owner(0.), 123, |r| {
        called = true;
        assert_eq!(r.target, before.target);
        assert_eq!(r.active_position, before.records[0].position);
        assert_eq!(r.max_face_tests, 123);
        Ok(None)
    })
    .unwrap();
    assert!(called);
    assert_eq!(r.return_code, 1);
    assert_eq!(s, before);
}
#[test]
fn selected_center_updates_only_the_active_motion_and_raw_state() {
    let mut s = state();
    s.active_index = 1;
    s.records.push(ActorTargetRecord {
        position: [10., 20., 30.],
        motion: [-7.; 3],
    });
    let before = s.clone();
    let r = run_actor_target_action(&mut s, owner(0.), 100, |_| {
        Ok(Some(ActorTargetSelection {
            raw_center: [3., 4., 0.],
            frame: NavigationFrame {
                translation: [10., 20., 30.],
            },
        }))
    })
    .unwrap();
    assert_eq!(r.return_code, 2);
    assert_eq!(s.state_raw, 2);
    assert_eq!(s.records[1].motion, [1.5, 3., 0.]);
    assert_eq!(s.records[0], before.records[0]);
    assert_eq!(s.records[1].position, before.records[1].position);
    assert_eq!(
        (s.target, s.active_index, s.mode_raw),
        (before.target, before.active_index, before.mode_raw)
    );
}
#[test]
fn motion_keeps_multiply_boundaries_and_world_add_before_subtraction() {
    let m = actor_target_motion(
        ActorTargetSelection {
            raw_center: [1.; 3],
            frame: NavigationFrame {
                translation: [0.; 3],
            },
        },
        [0.; 3],
    )
    .unwrap();
    assert_eq!(m.exponential_denominator, 1.);
    assert_eq!(m.motion[0].to_bits(), 0x3effffff);
    assert_eq!(m.motion[2], m.motion[0]);
    assert_eq!(m.motion[1], 1.5);
    let m = actor_target_motion(
        ActorTargetSelection {
            raw_center: [1., 0., 0.],
            frame: NavigationFrame {
                translation: [16777216., 0., 0.],
            },
        },
        [16777214., 0., 0.],
    )
    .unwrap();
    assert_eq!(m.world_center[0], 16777216.);
    assert_eq!(m.displacement, [2., 0., 0.]);
    assert_eq!(m.motion, [1., 1., 0.]);
}
#[test]
fn query_errors_invalid_records_and_unsupported_motion_leave_state_unchanged() {
    let mut s = state();
    let before = s.clone();
    assert!(
        run_actor_target_action(&mut s, owner(0.), 100, |_| anyhow::bail!("query failed")).is_err()
    );
    assert_eq!(s, before);
    for raw in [[0.; 3], [1e-30, 0., 0.], [1e30, 0., 0.], [f32::NAN, 0., 0.]] {
        assert!(
            run_actor_target_action(&mut s, owner(0.), 100, |_| Ok(Some(ActorTargetSelection {
                raw_center: raw,
                frame: NavigationFrame {
                    translation: [0.; 3]
                }
            })))
            .is_err()
        );
        assert_eq!(s, before);
    }
    s.records.clear();
    assert!(
        run_actor_target_action(&mut s, owner(0.), 100, |_| panic!("bad active record")).is_err()
    );
    assert!(actor_target_gate(owner(f32::NAN), [0., 0., 1.], 0, 0).is_err());
    assert!(
        run_actor_target_action(&mut s, owner(0.), 0, |_| panic!("invalid host budget")).is_err()
    );
}
