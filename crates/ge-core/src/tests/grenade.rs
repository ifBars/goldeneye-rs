use super::*;

fn records() -> Vec<ActorTargetRecord> {
    (0..5)
        .map(|i| ActorTargetRecord {
            position: [i as f32, 2., 3.],
            motion: [3., 4., 0.],
        })
        .collect()
}
fn source(location: usize) -> GrenadeSource {
    GrenadeSource {
        entity_kind_raw: 0,
        location,
    }
}

#[test]
fn source_location_selects_record_and_special_kind_bypasses_location() {
    for (location, expected) in [0, 0, 1, 1, 2, 2, 3, 0].into_iter().enumerate() {
        assert_eq!(grenade_record(source(location)).unwrap(), expected);
    }
    assert!(grenade_record(source(8)).is_err());
    assert_eq!(
        grenade_record(GrenadeSource {
            entity_kind_raw: 0x52000014,
            location: usize::MAX
        })
        .unwrap(),
        4
    );
}

#[test]
fn aligned_record_uses_full_motion_length_without_random_draws() {
    let mut rng = 123;
    let launch = prepare_grenade_launch(source(2), &records(), &mut rng).unwrap();
    assert_eq!(rng, 123);
    assert_eq!(launch.random_draws, 0);
    assert_eq!(launch.angular_velocity, [0., 25., 0.]);
    assert_eq!(launch.angular_damping, 5.);
    assert_eq!(launch.position, [1., 2., 3.]);
    assert_eq!(launch.linear_velocity, [3., 4., 0.]);
    let mut zero = records();
    zero[1].motion = [0.; 3];
    assert_eq!(
        prepare_grenade_launch(source(2), &zero, &mut rng)
            .unwrap()
            .angular_velocity,
        [0.; 3]
    );
}

#[test]
fn aligned_orientation_preserves_negative_z_quarter_turn() {
    let mut input = records();
    input[1].motion = [0., 0., 2.];
    let launch = prepare_grenade_launch(source(2), &input, &mut 0).unwrap();
    let q = launch.quaternion_xyzw;
    assert!(q[0].abs() < 1e-7 && q[1].abs() < 1e-7);
    assert!((q[2] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert!((q[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
}

#[test]
fn random_records_consume_eight_draws_and_replay_identically() {
    let mut expected_word = 0u32;
    for _ in 0..8 {
        expected_word = expected_word
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
    }
    let mut first = None;
    for location in [0, 4, 6] {
        let mut rng = 0;
        let launch = prepare_grenade_launch(source(location), &records(), &mut rng).unwrap();
        assert_eq!(rng, expected_word);
        assert_eq!(launch.random_draws, 8);
        assert_eq!(launch.angular_damping, 20.);
        let norm: f32 = launch.quaternion_xyzw.iter().map(|v| v * v).sum();
        assert!((norm - 1.).abs() < 1e-6);
        let spin_length = length(launch.angular_velocity).unwrap();
        assert!((std::f32::consts::FRAC_PI_2..=std::f32::consts::TAU).contains(&spin_length));
        if let Some((q, spin)) = first {
            assert_eq!(launch.quaternion_xyzw, q);
            assert_eq!(launch.angular_velocity, spin);
        }
        first = Some((launch.quaternion_xyzw, launch.angular_velocity));
    }
}

#[test]
fn random_unit_can_round_to_one_without_clamping() {
    // An invertible LCG permits constructing the prior word for a maximum output.
    let inverse = (0..5).fold(1u32, |x, _| {
        x.wrapping_mul(2u32.wrapping_sub(1_664_525u32.wrapping_mul(x)))
    });
    assert_eq!(inverse.wrapping_mul(1_664_525), 1);
    let mut stream = Stream {
        word: u32::MAX.wrapping_sub(1_013_904_223).wrapping_mul(inverse),
        draws: 0,
    };
    assert_eq!(stream.unit(), 1.);
    assert_eq!(stream.word, u32::MAX);
}

#[test]
fn invalid_selected_record_is_atomic_and_unused_records_are_not_read() {
    let mut input = records();
    input[0].motion[0] = f32::NAN;
    let mut rng = 42;
    assert!(prepare_grenade_launch(source(0), &input, &mut rng).is_err());
    assert_eq!(rng, 42);
    assert!(prepare_grenade_launch(source(2), &input, &mut rng).is_ok());
    input[1].motion = [f32::MAX; 3];
    assert!(prepare_grenade_launch(source(2), &input, &mut rng).is_err());
    assert_eq!(rng, 42);
    assert!(prepare_grenade_launch(source(6), &[], &mut rng).is_err());
}

#[test]
fn only_physics_flag_eight_enables_angular_damping() {
    let mut spin = GrenadeSpin {
        flags_raw: 0x10,
        angular_velocity: [1., 2., 3.],
    };
    let before = spin.clone();
    assert!(
        !spin
            .damp(
                f32::NAN,
                GrenadeTime {
                    clock_delta: f32::NAN,
                    time_scale: 1.
                }
            )
            .unwrap()
            .enabled
    );
    assert_eq!(spin, before);
    spin.flags_raw |= 8;
    let step = spin
        .damp(
            5.,
            GrenadeTime {
                clock_delta: 0.1,
                time_scale: 2.,
            },
        )
        .unwrap();
    assert_eq!(step.scaled_delta, Some(0.2));
    assert_eq!(step.exponent, Some(-1.));
    let factor = (-1f64).exp() as f32;
    assert_eq!(spin.angular_velocity, [factor, 2. * factor, 3. * factor]);
    assert_eq!(spin.flags_raw, 0x18);
}

#[test]
fn angular_damping_handles_zero_underflow_and_atomic_overflow() {
    let mut spin = GrenadeSpin {
        flags_raw: 8,
        angular_velocity: [1., -2., 3.],
    };
    let initial = spin.clone();
    spin.damp(
        20.,
        GrenadeTime {
            clock_delta: 0.,
            time_scale: 1.,
        },
    )
    .unwrap();
    assert_eq!(spin, initial);
    assert!(
        spin.damp(
            -f32::MAX,
            GrenadeTime {
                clock_delta: 1.,
                time_scale: 1.
            }
        )
        .is_err()
    );
    assert_eq!(spin, initial);
    let step = spin
        .damp(
            20.,
            GrenadeTime {
                clock_delta: 100.,
                time_scale: 1.,
            },
        )
        .unwrap();
    assert_eq!(step.factor, Some(0.));
    assert_eq!(spin.angular_velocity, [0.; 3]);
}
