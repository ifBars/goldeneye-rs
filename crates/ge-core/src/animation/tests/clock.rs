use super::*;

fn position() -> PositionInputs {
    PositionInputs {
        phase_bits: 0.0f32.to_bits(),
        script_duration_raw: 40,
        script_fps_bits: 20.0f32.to_bits(),
        play_start_frame: 0,
        first_animation_frames: 40,
        animation_fps: 20,
        child_flags_raw: 3,
        script_flags_raw: 4,
        timing_flags_raw: 0,
        animation_word38_raw: 0,
    }
}

fn phase() -> PhaseInputs {
    PhaseInputs {
        phase_bits: 0.0f32.to_bits(),
        script_duration_raw: 40,
        first_animation_frames: 40,
        selected_span_bits: 40.0f32.to_bits(),
        selected_rate_bits: clip_rate(20, 1.0f32.to_bits(), 60).unwrap(),
        child_flags_raw: 3,
        component_speed_bits: 1.0f32.to_bits(),
        scaled_step_bits: scaled_actor_step(source_prewarm_seconds(), 60).unwrap(),
    }
}

#[test]
fn prewarm_scales_seconds_and_advances_transition_independently() {
    let step = scaled_actor_step(source_prewarm_seconds(), 60).unwrap();
    assert_eq!(step, 12.0f32.to_bits());
    let delta = transition_delta((-1.0f32).to_bits(), step).unwrap();
    assert_eq!(delta, step);
    let state = TransitionState {
        scalar_bits: 0,
        reciprocal_bits: (1.0f32 / 6.0).to_bits(),
    };
    let next = advance_transition(state, delta).unwrap();
    assert_eq!(next.scalar_bits, 1.0f32.to_bits());
    assert_eq!(next.reciprocal_bits, 0);
}

#[test]
fn transition_endpoints_are_strict_and_stopped_state_is_lazy() {
    let state = TransitionState {
        scalar_bits: 0,
        reciprocal_bits: 1.0f32.to_bits(),
    };
    let equal = advance_transition(state, 1.0f32.to_bits()).unwrap();
    assert_eq!(equal.reciprocal_bits, 1.0f32.to_bits());
    let stopped = TransitionState {
        scalar_bits: f32::NAN.to_bits(),
        reciprocal_bits: (-0.0f32).to_bits(),
    };
    assert_eq!(
        advance_transition(stopped, f32::NAN.to_bits()).unwrap(),
        stopped
    );
}

#[test]
fn transition_lower_guard_reads_old_scalar_instead_of_computed_result() {
    let decreasing = TransitionState {
        scalar_bits: 0,
        reciprocal_bits: (-1.0f32).to_bits(),
    };
    assert_eq!(
        advance_transition(decreasing, 1.0f32.to_bits()).unwrap(),
        TransitionState {
            scalar_bits: (-1.0f32).to_bits(),
            reciprocal_bits: decreasing.reciprocal_bits,
        }
    );
    let recovering = TransitionState {
        scalar_bits: (-0.25f32).to_bits(),
        reciprocal_bits: 0.5f32.to_bits(),
    };
    assert_eq!(
        advance_transition(recovering, 1.0f32.to_bits()).unwrap(),
        TransitionState {
            scalar_bits: 0,
            reciprocal_bits: 0,
        }
    );
}

#[test]
fn old_phase_position_precedes_accumulation() {
    let prepared = prepare_position(position()).unwrap();
    let advanced = advance_phase(phase()).unwrap();
    assert_eq!(prepared.normalized_position_bits, 0);
    assert_eq!(prepared.previous_phase_bits, 0);
    assert!(f32::from_bits(advanced.phase_bits) > 0.0);
    let next = prepare_position(PositionInputs {
        phase_bits: advanced.phase_bits,
        ..position()
    })
    .unwrap();
    assert!(f32::from_bits(next.normalized_position_bits) > 0.0);
    assert_eq!(next.previous_phase_bits, advanced.phase_bits);
}

#[test]
fn position_converts_fps_and_uses_first_frame_count() {
    let inputs = PositionInputs {
        phase_bits: 0.5f32.to_bits(),
        animation_fps: 10,
        ..position()
    };
    assert_eq!(
        prepare_position(inputs).unwrap().normalized_position_bits,
        0.25f32.to_bits()
    );
    let inputs = PositionInputs {
        phase_bits: (-0.5f32).to_bits(),
        child_flags_raw: 2,
        ..inputs
    };
    assert_eq!(prepare_position(inputs).unwrap().previous_phase_bits, 0);
    assert_eq!(
        prepare_position(inputs).unwrap().normalized_position_bits,
        0
    );
}

#[test]
fn unsupported_reached_callbacks_and_curves_fail() {
    assert!(
        prepare_position(PositionInputs {
            script_flags_raw: 8,
            ..position()
        })
        .is_err()
    );
    assert!(
        prepare_position(PositionInputs {
            animation_word38_raw: 4,
            ..position()
        })
        .is_err()
    );
    assert!(
        prepare_position(PositionInputs {
            timing_flags_raw: 1,
            ..position()
        })
        .is_err()
    );
    assert!(
        prepare_position(PositionInputs {
            phase_bits: f32::INFINITY.to_bits(),
            ..position()
        })
        .is_err()
    );
}

#[test]
fn phase_wraps_once_and_reports_reprocessing_without_executing_it() {
    let next = advance_phase(PhaseInputs {
        phase_bits: 2.5f32.to_bits(),
        script_duration_raw: 1,
        child_flags_raw: 0x13,
        ..phase()
    })
    .unwrap();
    assert_eq!(next.phase_bits, 1.5f32.to_bits());
    assert_eq!(next.child_flags_raw & 0x4_0400, 0x4_0400);
    assert!(next.reprocess_script);
    let lower = advance_phase(PhaseInputs {
        phase_bits: (-2.0f32).to_bits(),
        script_duration_raw: 1,
        ..phase()
    })
    .unwrap();
    assert_eq!(lower.phase_bits, (-1.0f32).to_bits());
}

#[test]
fn phase_skip_and_rate_bypass_do_not_read_unreached_inputs() {
    let skipped = PhaseInputs {
        first_animation_frames: 0,
        phase_bits: f32::NAN.to_bits(),
        ..phase()
    };
    assert_eq!(
        advance_phase(skipped).unwrap().phase_bits,
        skipped.phase_bits
    );
    let bypass = PhaseInputs {
        child_flags_raw: 0x1_0002,
        selected_rate_bits: f32::NAN.to_bits(),
        ..phase()
    };
    assert!(advance_phase(bypass).is_ok());
    assert!(
        advance_phase(PhaseInputs {
            selected_span_bits: 0,
            ..phase()
        })
        .is_err()
    );
}

#[test]
fn nonloop_endpoint_flag_asymmetry_is_preserved() {
    let upper = advance_phase(PhaseInputs {
        phase_bits: 1.0f32.to_bits(),
        script_duration_raw: 1,
        child_flags_raw: 2,
        ..phase()
    })
    .unwrap();
    assert_eq!(upper.child_flags_raw, 0x2_0002);
    let lower = advance_phase(PhaseInputs {
        phase_bits: (-1.0f32).to_bits(),
        script_duration_raw: 1,
        child_flags_raw: 2,
        ..phase()
    })
    .unwrap();
    assert_eq!(lower.phase_bits, 0);
    assert_eq!(lower.child_flags_raw, 2);
}
