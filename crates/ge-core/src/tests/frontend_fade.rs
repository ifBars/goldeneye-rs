use super::*;

#[test]
fn fresh_component_is_clear_and_does_not_advance_without_setup() {
    let mut fade = Fade::default();
    assert_eq!(fade.color(), [0; 4]);
    assert!(!fade.is_active());
    assert!(!fade.first_update_pending());
    assert_eq!(fade.elapsed_seconds(), 0.);
    assert_eq!(fade.remaining_seconds(), 0.);
    assert!(!fade.update(1.).unwrap());
    assert_eq!(fade, Fade::default());
}

#[test]
fn warning_fade_skips_first_update_then_truncates_midpoint_and_clears_at_completion() {
    let mut fade = Fade::black_out();
    assert_eq!(fade.duration_seconds(), 0.5);
    assert!(fade.first_update_pending());
    assert!(!fade.update(0.25).unwrap());
    assert!(!fade.first_update_pending());
    assert_eq!(fade.elapsed_seconds(), 0.);
    assert_eq!(fade.color(), [0; 4]);
    assert!(!fade.update(0.25).unwrap());
    assert_eq!(fade.color(), [0, 0, 0, 127]);
    assert_eq!(fade.remaining_seconds(), 0.25);
    assert!(fade.is_active());
    assert!(fade.update(0.25).unwrap());
    assert_eq!(fade.elapsed_seconds(), 0.5);
    assert_eq!(fade.remaining_seconds(), 0.);
    assert_eq!(fade.color(), [0; 4]);
    assert!(!fade.is_active());
    assert!(!fade.update(0.25).unwrap());
    assert_eq!(fade.elapsed_seconds(), 0.5);
}

#[test]
fn legal_fade_starts_with_exact_opaque_bytes_and_a_large_first_update_is_skipped() {
    let mut fade = Fade::black_in();
    assert_eq!(fade.color(), [0, 0, 0, 255]);
    assert!(!fade.update(f32::MAX).unwrap());
    assert_eq!(fade.elapsed_seconds(), 0.);
    assert_eq!(fade.color(), [0, 0, 0, 255]);
    assert!(!fade.update(0.25).unwrap());
    assert_eq!(fade.color(), [0, 0, 0, 127]);
    assert!(fade.update(0.25).unwrap());
    assert_eq!(fade.color(), [0; 4]);
    assert!(!fade.is_active());
}

#[test]
fn normalized_red_green_blue_and_alpha_lanes_interpolate_independently() {
    let mut fade = Fade::new([0, 255, 0, 255], [255, 0, 255, 0], 1.).unwrap();
    assert_eq!(fade.color(), [0, 255, 0, 255]);
    fade.update(0.).unwrap();
    fade.update(0.5).unwrap();
    assert_eq!(fade.color(), [127; 4]);
    fade.update(0.25).unwrap();
    assert_eq!(fade.color(), [191, 63, 191, 63]);
}

#[test]
fn overshooting_completion_retains_elapsed_diagnostics_but_never_latches_black() {
    let mut fade = Fade::black_out();
    fade.update(0.).unwrap();
    assert!(fade.update(0.75).unwrap());
    assert_eq!(fade.elapsed_seconds(), 0.75);
    assert_eq!(fade.remaining_seconds(), -0.25);
    assert_eq!(fade.color(), [0; 4]);
    assert!(!fade.is_active());
}

#[test]
fn invalid_durations_and_deltas_never_mutate_an_existing_fade() {
    for seconds in [0., -0., -1., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(Fade::new([0; 4], [255; 4], seconds).is_err());
    }
    for mut fade in [Fade::default(), Fade::black_in(), Fade::black_out()] {
        for seconds in [-1., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let before = fade.clone();
            assert!(fade.update(seconds).is_err());
            assert_eq!(fade, before);
        }
        fade.update(0.).unwrap();
        for seconds in [-1., f32::NAN, f32::INFINITY] {
            let before = fade.clone();
            assert!(fade.update(seconds).is_err());
            assert_eq!(fade, before);
        }
    }
}

#[test]
fn elapsed_overflow_is_rejected_before_color_or_clock_changes() {
    let mut fade = Fade::new([0; 4], [255; 4], f32::MAX).unwrap();
    fade.update(0.).unwrap();
    fade.update(f32::MAX * 0.75).unwrap();
    let before = fade.clone();
    assert!(fade.update(f32::MAX).is_err());
    assert_eq!(fade, before);
}
