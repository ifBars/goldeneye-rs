use super::*;

#[test]
fn supported_host_dependencies_hold_legal_without_pretending_native_work_is_complete() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.seek_stage(Stage::Legal);
    sequence.set_host_dependencies_ready(false);
    sequence.advance_seconds(0.).unwrap();
    sequence.advance_seconds(10.).unwrap();
    assert_eq!(sequence.stage(), Stage::Legal);
    assert!(!sequence.fade().is_active());
    sequence.set_host_dependencies_ready(true);
    sequence.advance_seconds(0.).unwrap();
    assert_eq!(sequence.stage(), Stage::Title);
}

fn timing() -> Timing {
    Timing {
        activision: MediaTiming {
            frame_count: 8,
            fps: 4.,
        },
        eurocom: MediaTiming {
            frame_count: 3,
            fps: 2.,
        },
        mgm: MediaTiming {
            frame_count: 9,
            fps: 3.,
        },
        background: MediaTiming {
            frame_count: 10,
            fps: 4.,
        },
        legal_seconds: 5.,
    }
}

fn state(sequence: &Sequence) -> (Stage, f64, f64, u64, bool, Fade) {
    (
        sequence.stage,
        sequence.elapsed_seconds,
        sequence.background_seconds,
        sequence.transition_count,
        sequence.warning_fading,
        sequence.fade.clone(),
    )
}

fn finish_warning(sequence: &mut Sequence) {
    sequence.advance_seconds(0.25).unwrap();
    sequence.advance_seconds(0.25).unwrap();
    sequence.advance_seconds(0.25).unwrap();
}

#[test]
fn warning_minimum_and_timeout_start_independent_fades_without_changing_stage() {
    let mut sequence = Sequence::new(timing()).unwrap();
    assert_eq!(sequence.stage(), Stage::Strap);
    assert_eq!(sequence.video_frame(), None);
    assert!(!sequence.confirm());
    sequence.advance_seconds(1.499).unwrap();
    assert!(!sequence.confirm());
    sequence.advance_seconds(0.001).unwrap();
    assert!(sequence.confirm());
    assert_eq!(sequence.stage(), Stage::Strap);
    assert_eq!(sequence.elapsed_seconds(), 1.5);
    assert_eq!(sequence.transition_count(), 0);
    assert!(sequence.warning_fading());
    assert!(!sequence.confirm());
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.fade().elapsed_seconds(), 0.);
    assert_eq!(sequence.overlay_rgba(), [0; 4]);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Strap);
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 127]);
    assert!(!sequence.confirm());
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Zapper);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    assert_eq!(sequence.transition_count(), 1);
    assert_eq!(sequence.overlay_rgba(), [0; 4]);
    assert!(!sequence.confirm());
    sequence.advance(Duration::from_secs(21)).unwrap();
    assert_eq!(sequence.stage(), Stage::Zapper);
    assert!(sequence.warning_fading());
    assert_eq!(sequence.transition_count(), 1);
    finish_warning(&mut sequence);
    assert_eq!(sequence.stage(), Stage::Activision);
    assert_eq!(sequence.transition_count(), 2);
    assert_eq!(sequence.video_frame(), Some(0));
}

#[test]
fn warning_artwork_time_continues_while_the_separate_fade_clock_skips_and_advances() {
    for boundary in [170., 325., 480.] {
        let mut sequence = Sequence::new(timing()).unwrap();
        sequence.advance_seconds((boundary - 1.) / 30.).unwrap();
        assert!(sequence.confirm());
        sequence.advance_seconds(0.125).unwrap();
        assert_eq!(sequence.stage(), Stage::Strap);
        assert!(sequence.elapsed_seconds() * 30. > boundary);
        assert_eq!(sequence.fade().elapsed_seconds(), 0.);
        sequence.advance_seconds(0.25).unwrap();
        assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 127]);
        assert!(sequence.elapsed_seconds() * 30. > boundary);
    }
}

#[test]
fn repeated_warning_updates_round_as_singles_before_minimum_and_timeout_tests() {
    let delta = f64::from(1. / 60_f32);
    let mut sequence = Sequence::new(timing()).unwrap();
    for _ in 0..90 {
        sequence.advance_seconds(delta).unwrap();
    }
    // Widening each input and accumulating in double would reach 1.5 already.
    assert!(delta * 90. >= WARNING_MINIMUM_SECONDS);
    assert_eq!(
        sequence.elapsed_seconds(),
        f64::from(f32::from_bits(0x3fbf_fffa))
    );
    assert!(!sequence.confirm());
    sequence.advance_seconds(delta).unwrap();
    assert!(sequence.confirm());
    assert!(sequence.fade().first_update_pending());

    sequence.seek_stage(Stage::Strap);
    for _ in 0..1260 {
        sequence.advance_seconds(delta).unwrap();
    }
    assert!(delta * 1260. >= WARNING_TIMEOUT_SECONDS);
    assert_eq!(
        sequence.elapsed_seconds(),
        f64::from(f32::from_bits(0x41a7_ffb2))
    );
    assert!(!sequence.warning_fading());
    assert!(!sequence.fade().first_update_pending());
    sequence.advance_seconds(delta).unwrap();
    assert_eq!(
        sequence.elapsed_seconds(),
        f64::from(f32::from_bits(0x41a8_21d4))
    );
    assert!(sequence.warning_fading());
    assert!(sequence.fade().first_update_pending());
    assert_eq!(sequence.overlay_rgba(), [0; 4]);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.fade().elapsed_seconds(), 0.);
    assert!(!sequence.fade().first_update_pending());
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 127]);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Zapper);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    assert_eq!(sequence.transition_count(), 1);
}

#[test]
fn single_precision_warning_overflow_is_atomic_even_before_the_fades_first_update() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.advance_seconds(f64::from(f32::MAX)).unwrap();
    assert!(sequence.warning_fading());
    assert!(sequence.fade().first_update_pending());
    let before = state(&sequence);
    assert!(sequence.advance_seconds(f64::from(f32::MAX)).is_err());
    assert_eq!(state(&sequence), before);
}

#[test]
fn asset_frame_metadata_controls_logo_order_and_legal_entry_starts_a_global_fade() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.seek_stage(Stage::Activision);
    sequence.advance_seconds(1.75).unwrap();
    assert_eq!(sequence.video_frame(), Some(7));
    assert!(!sequence.confirm());
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Eurocom);
    assert_eq!(sequence.video_frame(), Some(0));
    sequence.advance_seconds(1.5).unwrap();
    assert_eq!(sequence.stage(), Stage::Mgm);
    assert!(!sequence.confirm());
    sequence.advance_seconds(3.).unwrap();
    assert_eq!(sequence.stage(), Stage::Legal);
    assert_eq!(sequence.video_frame(), None);
    assert!(!sequence.confirm());
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 255]);
    assert!(sequence.fade().first_update_pending());
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 255]);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 127]);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.overlay_rgba(), [0; 4]);
    assert_eq!(sequence.stage(), Stage::Legal);
    sequence.advance_seconds(4.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Title);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    assert_eq!(sequence.transition_count(), 4);
}

#[test]
fn large_updates_stop_at_fade_boundaries_without_fabricating_first_updates() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.advance_seconds(100.).unwrap();
    assert_eq!(sequence.stage(), Stage::Strap);
    assert_eq!(sequence.elapsed_seconds(), 100.);
    assert!(sequence.warning_fading());
    assert!(sequence.fade().first_update_pending());
    sequence.advance_seconds(100.).unwrap();
    assert_eq!(sequence.stage(), Stage::Strap);
    assert_eq!(sequence.fade().elapsed_seconds(), 0.);
    sequence.advance_seconds(0.5).unwrap();
    assert_eq!(sequence.stage(), Stage::Zapper);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    sequence.seek_stage(Stage::Activision);
    sequence.advance_seconds(100.).unwrap();
    assert_eq!(sequence.stage(), Stage::Legal);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 255]);
    sequence.advance_seconds(100.).unwrap();
    assert_eq!(sequence.stage(), Stage::Legal);
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 255]);
    sequence.advance_seconds(0.5).unwrap();
    assert_eq!(sequence.stage(), Stage::Title);
    assert_eq!(sequence.elapsed_seconds(), 0.);
}

#[test]
fn short_host_legal_hold_cannot_cut_short_the_source_entry_fade() {
    let mut settings = timing();
    settings.legal_seconds = 0.1;
    let mut sequence = Sequence::new(settings).unwrap();
    sequence.seek_stage(Stage::Legal);
    sequence.advance_seconds(1.).unwrap();
    assert_eq!(sequence.stage(), Stage::Legal);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Legal);
    assert_eq!(sequence.overlay_rgba(), [0, 0, 0, 127]);
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.stage(), Stage::Title);
    assert_eq!(sequence.overlay_rgba(), [0; 4]);
}

#[test]
fn title_never_automatically_enters_menu_and_confirmation_preserves_background_clock() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.seek_stage(Stage::Title);
    sequence.advance_seconds(7.75).unwrap();
    assert_eq!(sequence.video_frame(), Some(1));
    sequence.advance_seconds(100.).unwrap();
    assert_eq!(sequence.stage(), Stage::Title);
    assert_eq!(sequence.transition_count(), 0);
    assert!(sequence.confirm());
    assert_eq!(sequence.stage(), Stage::MainMenu);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    assert_eq!(sequence.video_frame(), Some(1));
    sequence.advance_seconds(0.25).unwrap();
    assert_eq!(sequence.video_frame(), Some(2));
    assert!(!sequence.confirm());
}

#[test]
fn explicit_stage_seek_clears_prior_fades_and_never_claims_skipped_boot_transitions() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.advance_seconds(2.).unwrap();
    assert!(sequence.confirm());
    finish_warning(&mut sequence);
    assert_eq!(sequence.transition_count(), 1);
    for stage in [
        Stage::Strap,
        Stage::Zapper,
        Stage::Activision,
        Stage::Eurocom,
        Stage::Mgm,
        Stage::Legal,
        Stage::Title,
        Stage::MainMenu,
    ] {
        sequence.seek_stage(stage);
        assert_eq!(sequence.stage(), stage);
        assert_eq!(sequence.elapsed_seconds(), 0.);
        assert_eq!(sequence.transition_count(), 0);
        assert!(!sequence.warning_fading());
        assert_eq!(
            sequence.overlay_rgba(),
            if stage == Stage::Legal {
                [0, 0, 0, 255]
            } else {
                [0; 4]
            }
        );
        assert_eq!(sequence.fade().is_active(), stage == Stage::Legal);
        if matches!(stage, Stage::Title | Stage::MainMenu) {
            assert_eq!(sequence.video_frame(), Some(0));
        }
    }
}

#[test]
fn explicit_host_skip_is_idempotent_clears_fade_and_starts_background_at_zero() {
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.advance_seconds(2.).unwrap();
    assert!(sequence.confirm());
    assert!(sequence.skip_to_menu());
    assert_eq!(sequence.stage(), Stage::MainMenu);
    assert_eq!(sequence.transition_count(), 1);
    assert_eq!(sequence.overlay_rgba(), [0; 4]);
    assert_eq!(sequence.video_frame(), Some(0));
    sequence.advance_seconds(0.5).unwrap();
    assert!(!sequence.skip_to_menu());
    assert_eq!(sequence.video_frame(), Some(2));
    assert_eq!(sequence.elapsed_seconds(), 0.5);
}

#[test]
fn logo_skip_uses_strict_frame_90_threshold_instead_of_a_fixed_seconds_limit() {
    let mut settings = timing();
    settings.activision = MediaTiming {
        frame_count: 100,
        fps: 10.,
    };
    settings.eurocom = MediaTiming {
        frame_count: 200,
        fps: 20.,
    };
    let mut sequence = Sequence::new(settings).unwrap();
    sequence.seek_stage(Stage::Activision);
    sequence.advance_seconds(9.).unwrap();
    assert_eq!(sequence.video_frame(), Some(90));
    assert!(!sequence.confirm());
    sequence.advance_seconds(0.1).unwrap();
    assert_eq!(sequence.video_frame(), Some(91));
    assert!(sequence.confirm());
    assert_eq!(sequence.stage(), Stage::Eurocom);
    assert_eq!(sequence.elapsed_seconds(), 0.);
    sequence.advance_seconds(4.5).unwrap();
    assert_eq!(sequence.video_frame(), Some(90));
    assert!(!sequence.confirm());
    sequence.advance_seconds(0.05).unwrap();
    assert_eq!(sequence.video_frame(), Some(91));
    assert!(sequence.confirm());
    assert_eq!(sequence.stage(), Stage::Mgm);
}

#[test]
fn invalid_metadata_and_elapsed_values_never_mutate_fade_clock_or_stage() {
    for fps in [0., -1., f32::NAN, f32::INFINITY] {
        let mut invalid = timing();
        invalid.eurocom.fps = fps;
        assert!(Sequence::new(invalid).is_err());
    }
    let mut invalid = timing();
    invalid.background.frame_count = 0;
    assert!(Sequence::new(invalid).is_err());
    for seconds in [0., -1., f64::NAN, f64::INFINITY] {
        invalid = timing();
        invalid.legal_seconds = seconds;
        assert!(Sequence::new(invalid).is_err());
    }
    let mut sequence = Sequence::new(timing()).unwrap();
    sequence.advance_seconds(1.5).unwrap();
    sequence.confirm();
    for seconds in [-1., f64::NAN, f64::INFINITY, f64::MAX] {
        let before = state(&sequence);
        assert!(sequence.advance_seconds(seconds).is_err());
        assert_eq!(state(&sequence), before);
    }
    sequence.advance_seconds(0.).unwrap();
    for seconds in [-1., f64::NAN, f64::INFINITY, f64::MAX] {
        let before = state(&sequence);
        assert!(sequence.advance_seconds(seconds).is_err());
        assert_eq!(state(&sequence), before);
    }
}
