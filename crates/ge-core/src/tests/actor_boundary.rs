use super::*;
fn sample(x: f32, y: f32) -> ActorBoundarySample {
    ActorBoundarySample {
        actor: [x, y, 0.],
        target: [0.; 3],
    }
}
fn check(band: ActorBoundaryBand, x: f32, y: f32) -> ActorBoundaryResult {
    actor_boundary_gate(band, 0, true, || Ok(sample(x, y)), |_, _| Ok(true)).unwrap()
}
#[test]
fn every_band_has_strict_inner_outer_thresholds_and_separate_strict_height() {
    for band in [
        ActorBoundaryBand::Radius45,
        ActorBoundaryBand::Radius34,
        ActorBoundaryBand::Radius345,
        ActorBoundaryBand::Radius36,
    ] {
        let radius = band.radius();
        for x in [radius - 0.25, radius + 0.25, 0., radius + 1.] {
            assert!(matches!(
                check(band, x, 0.),
                ActorBoundaryResult::OutsideBand { .. }
            ));
        }
        for x in [radius - 0.249, radius, radius + 0.249, -radius] {
            assert!(check(band, x, 0.999).accepted());
        }
        for y in [-1., 1., 10.] {
            assert!(matches!(
                check(band, radius, y),
                ActorBoundaryResult::HeightMismatch { .. }
            ));
        }
    }
}
#[test]
fn mode_and_earlier_gates_suppress_later_reads_and_boundary_calls() {
    for mode in [2, 6, u32::MAX] {
        assert_eq!(
            actor_boundary_gate(
                ActorBoundaryBand::Radius45,
                mode,
                false,
                || panic!("mode must reject before resolving positions"),
                |_, _| panic!()
            )
            .unwrap(),
            ActorBoundaryResult::UnsupportedMode
        );
    }
    for (x, y, has_binding) in [(0., 100., false), (4.5, 1., false), (4.5, 0., false)] {
        let mut reads = 0;
        let r = actor_boundary_gate(
            ActorBoundaryBand::Radius45,
            1,
            has_binding,
            || {
                reads += 1;
                Ok(sample(x, y))
            },
            |_, _| panic!("must not query boundary"),
        )
        .unwrap();
        assert!(!r.accepted());
        assert_eq!(reads, 1);
    }
}
#[test]
fn second_resolution_is_used_raw_without_repeating_range_or_height_gates() {
    let samples = [
        sample(4.5, 0.),
        ActorBoundarySample {
            actor: [100., 200., 300.],
            target: [-20., -30., -40.],
        },
    ];
    let mut reads = 0;
    let r = actor_boundary_gate(
        ActorBoundaryBand::Radius45,
        0,
        true,
        || {
            let s = samples[reads];
            reads += 1;
            Ok(s)
        },
        |a, b| {
            assert_eq!(a, samples[1].actor);
            assert_eq!(b, samples[1].target);
            Ok(false)
        },
    )
    .unwrap();
    assert_eq!(reads, 2);
    assert_eq!(r, ActorBoundaryResult::Boundary { clear: false });
}
#[test]
fn target_flag_uses_active_record_only_when_set_and_ignores_unselected_records() {
    let positions = [[1., 2., 3.], [4., 5., 6.], [f32::NAN; 3]];
    let mut target = ActorBoundaryTarget {
        flags_raw: 0x80,
        active_record: 255,
        positions: &positions,
    };
    assert_eq!(target.position().unwrap(), positions[0]);
    target.flags_raw |= 1;
    assert!(target.position().is_err());
    target.active_record = 1;
    assert_eq!(target.position().unwrap(), positions[1]);
    target.active_record = 2;
    assert!(target.position().is_err());
    target.positions = &[];
    target.flags_raw = 0;
    assert!(target.position().is_err());
}
#[test]
fn callback_failures_and_host_numeric_errors_are_explicit() {
    let call = |s: ActorBoundarySample| {
        actor_boundary_gate(
            ActorBoundaryBand::Radius45,
            0,
            true,
            || Ok(s),
            |_, _| Ok(true),
        )
    };
    assert!(call(sample(f32::MAX, 0.)).is_err());
    assert!(call(sample(4.5, f32::NAN)).is_err());
    assert!(
        actor_boundary_gate(
            ActorBoundaryBand::Radius45,
            0,
            true,
            || anyhow::bail!("unavailable snapshot"),
            |_, _| panic!()
        )
        .is_err()
    );
    assert!(
        actor_boundary_gate(
            ActorBoundaryBand::Radius45,
            0,
            true,
            || Ok(sample(4.5, 0.)),
            |_, _| anyhow::bail!("invalid cache")
        )
        .is_err()
    );
}

#[test]
fn horizontal_squared_distance_preserves_z_fused_addition_after_rounded_x_square() {
    let result = actor_boundary_gate(
        ActorBoundaryBand::Radius45,
        0,
        true,
        || {
            Ok(ActorBoundarySample {
                actor: [f32::from_bits(0x3fba3e7d), 100., f32::from_bits(0x401089ba)],
                target: [0.; 3],
            })
        },
        |_, _| panic!("outside range must win over height and boundary"),
    )
    .unwrap();
    let ActorBoundaryResult::OutsideBand { distance_squared } = result else {
        panic!("expected range rejection")
    };
    assert_eq!(distance_squared.to_bits(), 0x40e6f5e5); // Separate rounded z square would produce 0x40e6f5e6.
}
