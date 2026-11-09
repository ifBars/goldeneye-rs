use super::*;

fn title() -> Timeline {
    Timeline::new(
        60.,
        Segment {
            start: 0.,
            end: 29.,
        },
        Segment {
            start: 30.,
            end: 49.,
        },
    )
    .unwrap()
}

#[test]
fn inclusive_intro_endpoint_clamps_frame_but_hold_requires_strictly_later_time() {
    let mut timeline = title();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Intro, 0.));
    timeline.advance_to(0.25).unwrap();
    assert_eq!(timeline.frame(), 15.);
    timeline.advance_to(29. / 60.).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Intro, 29.));
    assert!(!timeline.confirm());
    timeline.advance_to(29. / 60.).unwrap();
    assert_eq!(timeline.phase(), Phase::Intro);
    timeline
        .advance_to(f32::from_bits((29f32 / 60.).to_bits() + 1))
        .unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Hold, 29.));
    timeline.advance_to(100.).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Hold, 29.));
}

#[test]
fn early_confirmation_is_ignored_and_outro_plays_30_through_49_after_hold() {
    let mut timeline = title();
    assert!(!timeline.confirm());
    timeline.advance_to(0.25).unwrap();
    assert!(!timeline.confirm());
    timeline.advance_to(0.5).unwrap();
    assert!(timeline.confirm());
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Outro, 30.));
    assert!(!timeline.confirm());
    timeline.advance_to(0.625).unwrap();
    assert_eq!(timeline.frame(), 37.5);
    timeline.advance_to(0.5 + 19. / 60.).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Outro, 49.));
    timeline
        .advance_to(f32::from_bits((0.5f32 + 19. / 60.).to_bits() + 1))
        .unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Complete, 49.));
    timeline.advance_to(1000.).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Complete, 49.));
    assert!(!timeline.confirm());
}

#[test]
fn zero_delta_and_hold_time_do_not_advance_or_shortcut_the_outro() {
    let mut timeline = title();
    timeline.advance_to(0.).unwrap();
    assert_eq!(timeline.frame(), 0.);
    timeline.advance_to(5.).unwrap();
    assert!(timeline.confirm());
    timeline.advance_to(5.).unwrap();
    timeline.advance_to(5.).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Outro, 30.));
    timeline.advance_to(5.25).unwrap();
    assert_eq!(timeline.frame(), 45.);
}

#[test]
fn invalid_clock_inputs_preserve_frame_phase_and_the_previous_clock() {
    let mut timeline = title();
    timeline.advance_to(0.25).unwrap();
    for time in [0.125, -1., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(timeline.advance_to(time).is_err());
        assert_eq!((timeline.phase(), timeline.frame()), (Phase::Intro, 15.));
    }
    timeline.advance_to(0.25).unwrap();
    timeline.advance_to(0.375).unwrap();
    assert_eq!(timeline.frame(), 22.5);
}

#[test]
fn rejects_bad_rates_ranges_overlap_and_unrepresentable_duration() {
    let intro = Segment {
        start: 0.,
        end: 29.,
    };
    let outro = Segment {
        start: 30.,
        end: 49.,
    };
    for fps in [0., -1., f32::NAN, f32::INFINITY] {
        assert!(Timeline::new(fps, intro, outro).is_err());
    }
    for bad in [
        Segment { start: 2., end: 1. },
        Segment {
            start: f32::NAN,
            end: 29.,
        },
        Segment {
            start: 0.,
            end: f32::INFINITY,
        },
    ] {
        assert!(Timeline::new(60., bad, outro).is_err());
    }
    assert!(
        Timeline::new(
            60.,
            intro,
            Segment {
                start: 29.,
                end: 49.
            }
        )
        .is_err()
    );
    assert!(Timeline::new(60., outro, intro).is_err());
    assert!(Timeline::new(f32::MIN_POSITIVE, intro, outro).is_err());
    assert!(
        Timeline::new(
            1.,
            Segment {
                start: -f32::MAX,
                end: f32::MAX
            },
            outro
        )
        .is_err()
    );
    assert!(
        Timeline::new(
            f32::MAX,
            Segment {
                start: 0.,
                end: f32::from_bits(1)
            },
            outro
        )
        .is_err()
    );
}

#[test]
fn one_frame_ranges_require_positive_time_before_hold_and_complete() {
    let mut timeline = Timeline::new(
        30.,
        Segment { start: 4., end: 4. },
        Segment { start: 8., end: 8. },
    )
    .unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Intro, 4.));
    timeline.advance_to(0.).unwrap();
    assert!(!timeline.confirm());
    timeline.advance_to(0.25).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Hold, 4.));
    assert!(timeline.confirm());
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Outro, 8.));
    timeline.advance_to(0.25).unwrap();
    assert_eq!(timeline.phase(), Phase::Outro);
    timeline.advance_to(0.5).unwrap();
    assert_eq!((timeline.phase(), timeline.frame()), (Phase::Complete, 8.));
    timeline.advance_to(f32::MAX).unwrap();
    assert_eq!(timeline.frame(), 8.);
}
