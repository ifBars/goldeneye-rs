use super::*;
use crate::navigation::{ActorTargetRecord, NavigationFrame};

fn owner() -> ActorTargetOwner {
    ActorTargetOwner {
        frame: NavigationFrame {
            translation: [0.; 3],
        },
        yaw: 0.,
    }
}

#[test]
fn dispatcher_selects_only_the_handler_for_the_current_raw_state() {
    let mut s = state();
    s.action.state_raw = 0;
    let selected = crate::navigation::ActorTargetSelection {
        raw_center: [0., 0., 8.],
        frame: NavigationFrame {
            translation: [0.; 3],
        },
    };
    let result = s
        .dispatch(
            owner(),
            100,
            |_| Ok(Some(selected)),
            |_| panic!("state zero must not use analytic collision"),
        )
        .unwrap();
    assert_eq!(result.return_code, 2);
    assert!(result.target.is_some());
    assert!(result.analytic.is_none());
    assert_eq!(s.action.state_raw, 2);
    let before = s.clone();
    let result = s
        .dispatch(
            owner(),
            0,
            |_| panic!("other state"),
            |_| panic!("other state"),
        )
        .unwrap();
    assert_eq!(result.return_code, 4);
    assert_eq!(s, before);
    let mut s = state();
    let result = s
        .dispatch(
            owner(),
            0,
            |_| panic!("state one must not use navigation"),
            |_| Ok(false),
        )
        .unwrap();
    assert_eq!(result.return_code, 0);
    assert!(result.analytic.is_some());
    assert!(result.target.is_none());
}
fn state() -> GrenadeArcState {
    GrenadeArcState {
        action: ActorTargetActionState {
            state_raw: 1,
            active_index: 0,
            mode_raw: 0,
            target: [0., 0., 10.],
            records: vec![ActorTargetRecord {
                position: [0.; 3],
                motion: [7., 8., 9.],
            }],
        },
        angle_index: 0,
        prepare_arc_raw: 1,
        small_lift_raw: 1,
        cached_apex: [99.; 3],
    }
}

#[test]
fn clearance_box_contains_ascent_with_asymmetric_source_padding() {
    for index in 0..5 {
        for (dx, dz) in [(10., 20.), (-10., 20.), (20., 0.), (0., -20.)] {
            let start = [-3., 2., 7.];
            let target = [start[0] + dx, start[1], start[2] + dz];
            let arc = prepare_grenade_arc(start, target, index, 1)
                .unwrap()
                .arc
                .unwrap();
            let volume = arc.clearance_volume().unwrap();
            let shape = &volume.shape;
            assert_eq!(volume.selection_origin, start);
            let axes = [
                shape.axis(0).unwrap(),
                shape.axis(1).unwrap(),
                shape.axis(2).unwrap(),
            ];
            let length = (arc
                .apex_displacement
                .into_iter()
                .map(|v| f64::from(v).powi(2))
                .sum::<f64>())
            .sqrt();
            assert!((f64::from(volume.selection_radius) - length).abs() < 2e-6);
            // Independent construction from the chord and midpoint sagitta of
            // the physical parabola, rather than repeating the padding formula.
            let chord = arc.apex_displacement.map(|v| f64::from(v) / length);
            let horizontal = chord[0].hypot(chord[2]);
            let normal = [
                -chord[0] * chord[1] / horizontal,
                horizontal,
                -chord[2] * chord[1] / horizontal,
            ];
            let half_time = f64::from(arc.apex_time) * 0.5;
            let sagitta = (0..3)
                .map(|i| {
                    let p = f64::from(arc.motion[i]) * half_time
                        - if i == 1 {
                            0.5 * f64::from(9.81f32) * half_time * half_time
                        } else {
                            0.
                        };
                    (p - f64::from(arc.apex_displacement[i]) * 0.5) * normal[i]
                })
                .sum::<f64>();
            let extents = [0.3 + length * 0.5, (0.3 + sagitta) * 0.5, 0.3];
            for i in 0..3 {
                assert!((f64::from(shape.half_extents[i]) - extents[i]).abs() < 1e-5);
                let expected = f64::from(start[i]) + chord[i] * extents[0] + normal[i] * extents[1];
                assert!((f64::from(shape.center[i]) - expected).abs() < 2e-5);
            }
            for fraction in [0., 0.125, 0.25, 0.5, 0.75, 1.] {
                let t = f64::from(arc.apex_time) * fraction;
                let point = std::array::from_fn::<_, 3, _>(|i| {
                    f64::from(start[i]) + f64::from(arc.motion[i]) * t
                        - if i == 1 {
                            0.5 * f64::from(9.81f32) * t * t
                        } else {
                            0.
                        }
                });
                for (axis, extent) in axes.iter().zip(shape.half_extents) {
                    let projection = (0..3)
                        .map(|i| (point[i] - f64::from(shape.center[i])) * f64::from(axis[i]))
                        .sum::<f64>();
                    assert!(
                        projection.abs() <= f64::from(extent) + 2e-5,
                        "angle {index} sample {fraction}: {projection} > {extent}"
                    );
                }
            }
            // Near faces stay at the start and chord: padding extends forward/up,
            // rather than fitting a symmetric capsule around the trajectory.
            for axis_index in [0, 1] {
                let outside = std::array::from_fn::<_, 3, _>(|i| {
                    f64::from(start[i]) - 0.05 * f64::from(axes[axis_index][i])
                });
                let projection = (0..3)
                    .map(|i| {
                        (outside[i] - f64::from(shape.center[i])) * f64::from(axes[axis_index][i])
                    })
                    .sum::<f64>();
                assert!(projection < -f64::from(shape.half_extents[axis_index]) - 0.04);
            }
        }
    }
}
#[test]
fn very_short_clearance_axes_remain_unnormalized_and_bad_input_is_an_error() {
    let arc = prepare_grenade_arc([0.; 3], [0., -0.1, 1e-5], 0, 1)
        .unwrap()
        .arc
        .unwrap();
    let volume = arc.clearance_volume().unwrap();
    assert_eq!(volume.shape.axes[0], arc.apex_displacement);
    assert!(volume.shape.axes[0].into_iter().all(|v| v.abs() < 1e-4));
    let mut bad = arc.clone();
    bad.horizontal_velocity = 0.;
    assert!(bad.clearance_volume().is_err());
    bad = arc;
    bad.apex_displacement[0] = f32::MAX;
    assert!(bad.clearance_volume().is_err());
}
#[test]
fn oriented_box_accessor_keeps_stored_axes_and_derives_only_the_third() {
    let shape = crate::collision::OrientedBox {
        center: [0.; 3],
        axes: [[2., 0., 0.], [0., 3., 0.]],
        half_extents: [1., 2., 3.],
    };
    assert_eq!(shape.axis(0).unwrap(), [2., 0., 0.]);
    assert_eq!(shape.axis(2).unwrap(), [0., 0., 6.]);
    assert!(shape.axis(3).is_err());
    let mut bad = shape.clone();
    bad.half_extents[0] = -1.;
    assert!(bad.validate().is_err());
    bad = shape;
    bad.axes = [[f32::MAX, 0., 0.], [0., f32::MAX, 0.]];
    assert!(bad.axis(2).is_err());
}
#[test]
fn each_angle_and_lift_hits_its_raised_target_under_independent_ballistics() {
    for index in 0..5 {
        for lift in [0, 1, 255] {
            let candidate = prepare_grenade_arc([2., 3., 4.], [12., 3., 24.], index, lift).unwrap();
            let arc = candidate.arc.unwrap();
            let distance = 20f64.hypot(10.);
            let horizontal = f64::from(arc.motion[0]).hypot(f64::from(arc.motion[2]));
            let t = distance / horizontal;
            let impact = f64::from(arc.motion[1]) * t - 0.5 * f64::from(9.81f32) * t * t;
            let expected = if lift == 0 { 1. } else { 0.1 };
            assert!(
                (impact - expected).abs() < 2e-5,
                "angle {index} lift {lift}: {impact}"
            );
            assert!(
                (f64::from(arc.apex_time) - f64::from(arc.motion[1]) / f64::from(9.81f32)).abs()
                    < 1e-6
            );
        }
    }
}
#[test]
fn normal_rejections_keep_the_source_intermediate_motion() {
    let mut s = state();
    s.action.target = [0., 10., 1.];
    let result = s
        .query(owner(), |_| {
            panic!("elevation rejection must not query world")
        })
        .unwrap();
    assert_eq!(result.rejection, Some(ArcRejection::Elevation));
    assert_eq!(s.action.records[0].motion, [0., 10.1, 1.]);
    assert_eq!(s.angle_index, 1);
    assert_eq!(result.return_code, 0);
    let c = prepare_grenade_arc([0.; 3], [0., 7., 10.], 0, 1).unwrap();
    assert_eq!(c.rejection, Some(ArcRejection::ApexAfterTarget));
    assert!(c.motion[1] > 0. && c.motion[2] > 0.);
}
#[test]
fn clearance_has_two_calls_and_ready_state_only_after_the_second() {
    let mut s = state();
    let result = s
        .query(owner(), |q| {
            let GrenadeClearance::WorldBox { arc, volume } = q else {
                panic!("expected ascent box");
            };
            assert_eq!(volume.selection_origin, arc.start);
            assert_eq!(volume.shape.half_extents[2], 0.3);
            Ok(false)
        })
        .unwrap();
    assert_eq!(result.return_code, 0);
    assert_eq!(s.action.state_raw, 1);
    assert_eq!(s.prepare_arc_raw, 0);
    let cached = s.cached_apex;
    let motion = s.action.records[0].motion;
    let result = s
        .query(owner(), |q| {
            let (GrenadeClearance::WorldLine {
                origin,
                constructor_displacement,
                displacement,
            }
            | GrenadeClearance::ActorLine {
                origin,
                constructor_displacement,
                displacement,
            }) = q
            else {
                panic!("expected line stage");
            };
            assert_eq!(origin, cached);
            assert_eq!(
                constructor_displacement,
                [0. - cached[0], -cached[1], 10. - cached[2]]
            );
            assert_eq!(displacement[1], -cached[1] + 0.1);
            assert_eq!(displacement[2], 10. - cached[2]);
            Ok(false)
        })
        .unwrap();
    assert_eq!(result.return_code, 3);
    assert_eq!(s.action.state_raw, 3);
    assert_eq!(s.action.records[0].motion, motion);
}
#[test]
fn blocked_arc_or_line_retries_and_wrap_preserves_the_current_stage() {
    let mut s = state();
    s.query(owner(), |_| Ok(true)).unwrap();
    assert_eq!(s.angle_index, 1);
    assert_eq!(s.prepare_arc_raw, 1);
    s.angle_index = 4;
    s.prepare_arc_raw = 0;
    let result = s
        .query(owner(), |q| {
            assert!(matches!(q, GrenadeClearance::WorldLine { .. }));
            Ok(true)
        })
        .unwrap();
    assert_eq!(result.return_code, 0);
    assert_eq!(s.angle_index, 0);
    assert_eq!(s.small_lift_raw, 0);
    assert_eq!(s.prepare_arc_raw, 0, "phase wrap must not force a new arc");
    s.angle_index = 4;
    assert_eq!(s.query(owner(), |_| Ok(true)).unwrap().return_code, 1);
    assert_eq!(s.angle_index, 5);
    assert_eq!(s.action.state_raw, 1);
}
#[test]
fn gated_calls_do_not_address_records_or_clearance_and_backend_errors_are_explicit() {
    let mut s = state();
    s.action.target = [10., 0., 0.];
    s.action.records.clear();
    s.angle_index = u32::MAX;
    let before = s.clone();
    assert_eq!(
        s.query(owner(), |_| panic!("gated")).unwrap().return_code,
        1
    );
    assert_eq!(s, before);
    let mut s = state();
    assert!(
        s.query(owner(), |_| anyhow::bail!("collision backend unavailable"))
            .is_err()
    );
    assert_ne!(s.action.records[0].motion, [7., 8., 9.]);
    assert_eq!(s.prepare_arc_raw, 1);
    assert_eq!(s.cached_apex, [99.; 3]);
}
#[test]
fn line_stage_does_not_read_active_records_and_invalid_numeric_inputs_error() {
    let mut s = state();
    s.prepare_arc_raw = 0;
    s.action.records.clear();
    s.cached_apex = [0., 1., 5.];
    assert_eq!(s.query(owner(), |_| Ok(false)).unwrap().return_code, 3);
    assert!(prepare_grenade_arc([0.; 3], [0., 1., 0.], 0, 1).is_err());
    assert!(prepare_grenade_arc([0.; 3], [0., 0., 1.], 5, 1).is_err());
    assert!(prepare_grenade_arc([0.; 3], [0., 0., f32::MAX], 0, 1).is_err());
}

#[test]
fn line_queries_world_before_actor_list_and_short_circuits_on_hit_or_error() {
    for (world_hit, actor_hit, expected) in [(true, false, 0), (false, true, 0), (false, false, 3)]
    {
        let mut s = state();
        s.prepare_arc_raw = 0;
        s.cached_apex = [0., 1., 5.];
        let mut calls = Vec::new();
        let result = s
            .query(owner(), |q| match q {
                GrenadeClearance::WorldLine { .. } => {
                    calls.push("world");
                    Ok(world_hit)
                }
                GrenadeClearance::ActorLine { .. } => {
                    calls.push("actors");
                    Ok(actor_hit)
                }
                GrenadeClearance::WorldBox { .. } => panic!("wrong phase"),
            })
            .unwrap();
        assert_eq!(result.return_code, expected);
        assert_eq!(
            calls,
            if world_hit {
                vec!["world"]
            } else {
                vec!["world", "actors"]
            }
        );
    }
    let mut s = state();
    s.prepare_arc_raw = 0;
    let before = s.clone();
    assert!(
        s.query(owner(), |q| match q {
            GrenadeClearance::WorldLine { .. } => anyhow::bail!("world backend missing"),
            _ => panic!("error must stop later backends"),
        })
        .is_err()
    );
    assert_eq!(s, before);
}

#[test]
fn both_line_callbacks_retain_constructor_displacement_for_nonzero_raw_lift() {
    let mut s = state();
    s.prepare_arc_raw = 0;
    s.small_lift_raw = 255;
    s.cached_apex = [0., 1., 5.];
    s.action.records.clear();
    let mut calls = Vec::new();
    let result = s
        .query(owner(), |request| {
            let (name, origin, constructor, displacement) = match request {
                GrenadeClearance::WorldLine {
                    origin,
                    constructor_displacement,
                    displacement,
                } => ("world", origin, constructor_displacement, displacement),
                GrenadeClearance::ActorLine {
                    origin,
                    constructor_displacement,
                    displacement,
                } => ("actors", origin, constructor_displacement, displacement),
                GrenadeClearance::WorldBox { .. } => panic!("wrong phase"),
            };
            assert_eq!(origin, [0., 1., 5.]);
            assert_eq!(constructor, [0., -1., 5.]);
            assert_eq!(displacement, [0., -0.9, 5.]);
            calls.push(name);
            Ok(false)
        })
        .unwrap();
    assert_eq!(calls, ["world", "actors"]);
    assert_eq!(result.return_code, 3);
    assert_eq!(s.action.state_raw, 3);
}

#[test]
fn rejected_line_gate_does_not_validate_unaddressed_cached_apex() {
    let mut s = state();
    s.prepare_arc_raw = 0;
    s.cached_apex = [f32::NAN; 3];
    s.action.target = [10., 0., 0.];
    s.action.records.clear();
    let result = s
        .query(owner(), |_| {
            panic!("rejected gate must not query a backend")
        })
        .unwrap();
    assert_eq!(result.return_code, 1);
    assert_eq!(s.action.state_raw, 1);
    assert!(s.cached_apex.into_iter().all(f32::is_nan));
}
