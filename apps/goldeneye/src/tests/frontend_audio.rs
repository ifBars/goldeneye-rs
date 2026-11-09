use super::*;

#[test]
fn live_bus_gains_change_active_audio_without_resetting_transport_clocks() {
    let mut mixer = Mixer {
        primary: Buffer::configured(48_000, 1).unwrap(),
        ..Mixer::default()
    };
    mixer.primary.queue(&[8192; 6]).unwrap();
    mixer
        .play_one_shot(48_000, 1, Arc::from([16384; 6]), 1.)
        .unwrap();
    assert_eq!(mixer.next_frame(48_000), [0.75; 2]);
    mixer.primary_gain = 0.5;
    mixer.sound_gain = 0.25;
    assert_eq!(mixer.next_frame(48_000), [0.25; 2]);
    mixer.primary_gain = 0.;
    mixer.sound_gain = 0.;
    assert_eq!(mixer.next_frame(48_000), [0.; 2]);
    assert_eq!(mixer.primary.consumed, 3);
    assert_eq!(mixer.voice_output_frames, 3);
    assert_eq!(mixer.active(), 1);
    mixer.primary_gain = 1.;
    mixer.sound_gain = 1.;
    assert_eq!(mixer.next_frame(48_000), [0.75; 2]);
    mixer.clear();
    assert_eq!((mixer.primary_gain, mixer.sound_gain), (1., 1.));
}

#[test]
fn same_rate_pcm_preserves_stereo_order_and_signed_extrema() {
    let mut buffer = Buffer::configured(48_000, 2).unwrap();
    buffer.queue(&[-32768, 32767, 8192, -8192]).unwrap();
    assert_eq!(buffer.next_frame(48_000), Some([-1., 32767. / 32768.]));
    assert_eq!(buffer.next_frame(48_000), Some([0.25, -0.25]));
    assert_eq!(
        (buffer.queued_frames(), buffer.consumed, buffer.submitted),
        (0, 2, 2)
    );
    assert!(buffer.next_frame(48_000).is_none());
    assert_eq!(buffer.underrun, 1);
}

#[test]
fn mono_upsampling_interpolates_without_swapping_or_synthesizing_content() {
    let mut buffer = Buffer::configured(24_000, 1).unwrap();
    buffer.queue(&[0, 16384, 0]).unwrap();
    assert_eq!(buffer.next_frame(48_000), Some([0., 0.]));
    assert_eq!(buffer.next_frame(48_000), Some([0.25, 0.25]));
    assert_eq!(buffer.next_frame(48_000), Some([0.5, 0.5]));
    assert_eq!(buffer.next_frame(48_000), Some([0.25, 0.25]));
    assert_eq!(buffer.consumed, 2);
}

#[test]
fn downsampling_and_chunk_boundaries_preserve_source_time() {
    let mut buffer = Buffer::configured(48_000, 1).unwrap();
    buffer.queue(&[0, 4096, 8192, 12288]).unwrap();
    assert_eq!(buffer.next_frame(24_000), Some([0., 0.]));
    assert_eq!(buffer.next_frame(24_000), Some([0.25, 0.25]));
    let phase = buffer.phase;
    for _ in 0..10 {
        assert!(buffer.next_frame(24_000).is_none());
    }
    assert_eq!(buffer.phase, phase);
    buffer.queue(&[16384, 20480]).unwrap();
    assert_eq!(buffer.next_frame(24_000), Some([0.5, 0.5]));
    assert_eq!(buffer.consumed, 6);
}

#[test]
fn queue_capacity_and_whole_frame_validation_are_atomic() {
    assert!(Buffer::configured(0, 2).is_err());
    assert!(Buffer::configured(48_000, 3).is_err());
    let mut buffer = Buffer::configured(8_000, 2).unwrap();
    assert!(buffer.queue(&[1]).is_err());
    assert_eq!(buffer.samples.len(), 0);
    buffer.queue(&vec![1; 8_000 * 2 * QUEUE_SECONDS]).unwrap();
    let size = buffer.samples.len();
    assert!(buffer.queue(&[1, 2]).is_err());
    assert_eq!(buffer.samples.len(), size);
    assert!(Buffer::default().queue(&[]).is_err());
}

#[test]
fn transition_clear_resets_pcm_phase_and_clip_counters() {
    let mut buffer = Buffer::configured(24_000, 1).unwrap();
    buffer.queue(&[16384, 16384]).unwrap();
    buffer.next_frame(48_000);
    assert_ne!(buffer.phase, 0);
    buffer.clear();
    assert_eq!(
        (
            buffer.phase,
            buffer.consumed,
            buffer.submitted,
            buffer.underrun
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(buffer.queued_frames(), 0);
    buffer.queue(&[-8192]).unwrap();
    assert_eq!(buffer.next_frame(48_000), Some([-0.25, -0.25]));
}

#[test]
fn pause_and_contended_callback_emit_silence_without_consuming_pcm() {
    let shared = Shared::new();
    shared.buffer.lock().unwrap().primary = Buffer::configured(48_000, 2).unwrap();
    shared
        .buffer
        .lock()
        .unwrap()
        .primary
        .queue(&[8192, -8192])
        .unwrap();
    let mut output = [1.; 2];
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.; 2]);
    assert_eq!(shared.buffer.lock().unwrap().primary.queued_frames(), 1);
    shared.paused.store(false, Ordering::Release);
    let guard = shared.buffer.lock().unwrap();
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.; 2]);
    assert_eq!(shared.lock_misses.load(Ordering::Relaxed), 1);
    drop(guard);
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.25, -0.25]);
    assert_eq!(shared.buffer.lock().unwrap().primary.queued_frames(), 0);
}

#[test]
fn device_channel_mapping_averages_stereo_to_mono_and_silences_extra_outputs() {
    let shared = Shared::new();
    shared.buffer.lock().unwrap().primary = Buffer::configured(48_000, 2).unwrap();
    shared
        .buffer
        .lock()
        .unwrap()
        .primary
        .queue(&[16384, 0, 16384, 0])
        .unwrap();
    shared.paused.store(false, Ordering::Release);
    let mut mono = [0.];
    fill_output(&mut mono, 1, 48_000, &shared, |v| v);
    assert_eq!(mono, [0.25]);
    let mut surround = [9.; 6];
    fill_output(&mut surround, 6, 48_000, &shared, |v| v);
    assert_eq!(surround, [0.5, 0., 0., 0., 0., 0.]);
    let mut partial = [9.; 3];
    fill_output(&mut partial, 2, 48_000, &shared, |v| v);
    assert_eq!(partial, [0.; 3]);
}

#[test]
fn unchanged_pause_state_skips_os_calls_but_failed_device_cannot_resume() {
    let shared = Arc::new(Shared::new());
    shared.paused.store(false, Ordering::Release);
    let mut audio = FrontendAudio {
        #[cfg(windows)]
        stream: None, // A repeated resume must not address the missing stream.
        shared: shared.clone(),
        output_rate: 48000,
        output_channels: 2,
        device: "authored fixture".into(),
    };
    audio.set_paused(false).unwrap();
    shared.device_errors.store(1, Ordering::Release);
    assert!(audio.set_paused(false).is_err());
    shared.paused.store(true, Ordering::Release);
    audio.set_paused(true).unwrap();
    assert!(audio.set_paused(false).is_err());
}

#[test]
fn title_cue_and_nonintegral_music_rate_have_independent_output_clocks() {
    const OUTPUT: u32 = 48_000;
    const MUSIC: u32 = 31_999;
    const CUE: u32 = 22_050;
    const CUE_FRAMES: usize = 11_256;
    let music = vec![4096; MUSIC as usize * 2 * 2];
    let mut baseline = Buffer::configured(MUSIC, 2).unwrap();
    baseline.queue(&music).unwrap();
    let mut mixer = Mixer {
        primary: Buffer::configured(MUSIC, 2).unwrap(),
        ..Mixer::default()
    };
    mixer.primary.queue(&music).unwrap();
    let cue: Arc<[i16]> = vec![8192; CUE_FRAMES * 2].into();
    let weak = Arc::downgrade(&cue);
    let id = mixer.play_one_shot(CUE, 2, cue, 1.).unwrap();
    assert_eq!(id, 1);
    let expected = (CUE_FRAMES as u64 * u64::from(OUTPUT)).div_ceil(u64::from(CUE));
    for frame in 0..expected + 17 {
        let plain = baseline.next_frame(OUTPUT).unwrap();
        let mixed = mixer.next_frame(OUTPUT);
        let expected_mix = if frame < expected { [0.375; 2] } else { plain };
        assert_eq!(mixed, expected_mix);
        assert_eq!(mixer.primary.phase, baseline.phase);
        assert_eq!(mixer.primary.consumed, baseline.consumed);
        assert_eq!(mixer.primary.submitted, baseline.submitted);
        assert_eq!(mixer.primary.queued_frames(), baseline.queued_frames());
        assert_eq!(mixer.primary.underrun, baseline.underrun);
    }
    assert_eq!((mixer.started, mixer.completed, mixer.active()), (1, 1, 0));
    assert_eq!(mixer.voice_output_frames, expected);
    assert!(
        weak.upgrade().is_some(),
        "callback must retain completed PCM"
    );
    mixer.retire_completed();
    assert!(
        weak.upgrade().is_none(),
        "control thread retires final PCM owner"
    );
    assert!(
        mixer.primary.queued_frames() > 0,
        "cue end cannot clear music"
    );
}

#[test]
fn one_shot_pause_and_nonlooping_final_sample_do_not_advance_or_repeat() {
    let shared = Shared::new();
    shared
        .buffer
        .lock()
        .unwrap()
        .play_one_shot(24_000, 1, Arc::from([8192]), 1.)
        .unwrap();
    let mut output = [9.; 2];
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.; 2]);
    assert_eq!(shared.buffer.lock().unwrap().voice_output_frames, 0);
    shared.paused.store(false, Ordering::Release);
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.25; 2]);
    assert_eq!(shared.buffer.lock().unwrap().active(), 1);
    shared.paused.store(true, Ordering::Release);
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.; 2]);
    assert_eq!(shared.buffer.lock().unwrap().voice_output_frames, 1);
    shared.paused.store(false, Ordering::Release);
    fill_output(&mut output, 2, 48_000, &shared, |v| v);
    assert_eq!(output, [0.25; 2]);
    for _ in 0..3 {
        fill_output(&mut output, 2, 48_000, &shared, |v| v);
        assert_eq!(output, [0.; 2]);
    }
    let mixer = shared.buffer.lock().unwrap();
    assert_eq!(
        (mixer.completed, mixer.active(), mixer.voice_output_frames),
        (1, 0, 2)
    );
}

#[test]
fn completed_voices_retire_only_on_control_thread_and_reuse_bounded_slots() {
    let mut mixer = Mixer::default();
    for _ in 0..ONE_SHOT_VOICES {
        mixer.play_one_shot(48_000, 1, Arc::from([1]), 1.).unwrap();
    }
    assert!(mixer.play_one_shot(48_000, 1, Arc::from([1]), 1.).is_err());
    mixer.next_frame(48_000);
    assert_eq!(mixer.completed, ONE_SHOT_VOICES as u64);
    assert_eq!(mixer.active(), 0);
    assert_eq!(mixer.retained_samples(), ONE_SHOT_VOICES);
    assert_eq!(
        mixer.play_one_shot(48_000, 1, Arc::from([2]), 1.).unwrap(),
        9
    );
    assert_eq!(mixer.retained_samples(), 1);
    assert_eq!((mixer.active(), mixer.started), (1, 9));
}

#[test]
fn rejected_one_shots_do_not_mutate_primary_or_voice_state() {
    let mut mixer = Mixer {
        primary: Buffer::configured(48_000, 1).unwrap(),
        ..Mixer::default()
    };
    mixer.primary.queue(&[1234, 4321]).unwrap();
    mixer.primary.next_frame(44_100);
    let before = (
        mixer.primary.phase,
        mixer.primary.consumed,
        mixer.primary.submitted,
        mixer.primary.samples.clone(),
    );
    let invalid: [(u32, u16, Arc<[i16]>, f32); 10] = [
        (0, 1, Arc::from([1]), 1.),
        (48_000, 0, Arc::from([1]), 1.),
        (48_000, 3, Arc::from([1]), 1.),
        (48_000, 2, Arc::from([1]), 1.),
        (48_000, 1, Arc::from([]), 1.),
        (48_000, 1, Arc::from([1]), f32::NAN),
        (48_000, 1, Arc::from([1]), f32::INFINITY),
        (48_000, 1, Arc::from([1]), -1.),
        (48_000, 1, Arc::from([1]), ONE_SHOT_GAIN_MAX + 1.),
        (8_000, 1, vec![1; 8_000 * ONE_SHOT_SECONDS + 1].into(), 1.),
    ];
    for (rate, channels, samples, gain) in invalid {
        assert!(mixer.play_one_shot(rate, channels, samples, gain).is_err());
        assert_eq!(
            (
                mixer.started,
                mixer.completed,
                mixer.active(),
                mixer.next_id
            ),
            (0, 0, 0, 1)
        );
        assert_eq!(
            (
                mixer.primary.phase,
                mixer.primary.consumed,
                mixer.primary.submitted,
                mixer.primary.samples.clone()
            ),
            before
        );
    }
    let full: Arc<[i16]> = vec![1; ONE_SHOT_SAMPLES].into();
    mixer.play_one_shot(192_000, 2, full, 1.).unwrap();
    assert!(mixer.play_one_shot(48_000, 1, Arc::from([1]), 1.).is_err());
    assert_eq!(
        (mixer.started, mixer.active(), mixer.retained_samples()),
        (1, 1, ONE_SHOT_SAMPLES)
    );
    mixer.clear();
    mixer.next_id = u64::MAX;
    assert!(mixer.play_one_shot(48_000, 1, Arc::from([1]), 1.).is_err());
    assert_eq!(mixer.next_id, u64::MAX);
}

#[test]
fn output_mapping_sums_before_final_limiter_and_counts_clipped_device_frames() {
    let shared = Shared::new();
    {
        let mut mixer = shared.buffer.lock().unwrap();
        mixer.primary = Buffer::configured(48_000, 2).unwrap();
        mixer
            .primary
            .queue(&[24576, -24576, 24576, -24576])
            .unwrap();
        mixer
            .play_one_shot(48_000, 2, Arc::from([24576, -24576, 24576, -24576]), 1.)
            .unwrap();
    }
    shared.paused.store(false, Ordering::Release);
    let mut mono = [9.];
    fill_output(&mut mono, 1, 48_000, &shared, |v| v);
    assert_eq!(mono, [0.]); // Cancellation happens before limiting.
    assert_eq!(shared.buffer.lock().unwrap().clipped_frames, 0);
    let mut surround = [9.; 6];
    fill_output(&mut surround, 6, 48_000, &shared, |v| v);
    assert_eq!(surround, [1., -1., 0., 0., 0., 0.]);
    assert_eq!(shared.buffer.lock().unwrap().clipped_frames, 1);
}

#[test]
fn primary_reconfiguration_preserves_sfx_and_explicit_clear_discards_everything() {
    let shared = Arc::new(Shared::new());
    let mut audio = FrontendAudio {
        #[cfg(windows)]
        stream: None,
        shared: shared.clone(),
        output_rate: 48_000,
        output_channels: 2,
        device: "authored fixture".into(),
    };
    let id = audio
        .play_one_shot(22_050, 2, Arc::from([8192, -8192]), 1.)
        .unwrap();
    audio.reset(31_999, 2).unwrap(); // Already paused; no device call needed.
    audio.queue(&[4096, 4096]).unwrap();
    let evidence = audio.evidence().unwrap();
    assert_eq!((evidence.oneshot_started, evidence.oneshot_active), (1, 1));
    assert_eq!((evidence.source_rate, evidence.queued_frames), (31_999, 1));
    assert_eq!(id, 1);
    assert!(audio.reset(0, 1).is_err());
    assert_eq!(audio.evidence().unwrap().oneshot_active, 1);
    audio.clear().unwrap();
    let evidence = audio.evidence().unwrap();
    assert_eq!(
        (
            evidence.oneshot_started,
            evidence.oneshot_completed,
            evidence.oneshot_active
        ),
        (0, 0, 0)
    );
    assert_eq!(evidence.queued_frames, 0);
    assert_eq!(
        audio.play_one_shot(48_000, 1, Arc::from([1]), 1.).unwrap(),
        2
    );
    shared.device_errors.store(1, Ordering::Release);
    assert!(audio.play_one_shot(48_000, 1, Arc::from([1]), 1.).is_err());
    assert_eq!(audio.evidence().unwrap().oneshot_started, 1);
}
