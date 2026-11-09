use super::*;
fn expected() -> thp::AudioInfo {
    thp::AudioInfo {
        channels: 2,
        sample_rate: 32000,
        total_samples: 2,
        tracks: 1,
        maximum_frame_samples: 14,
    }
}
fn pcm(samples: Vec<i16>, count: u32) -> thp::PcmAudio {
    thp::PcmAudio {
        sample_rate: 32000,
        channels: 2,
        samples_per_channel: count,
        samples,
    }
}
#[test]
fn audio_hash_uses_canonical_little_endian_and_exact_channel_totals() {
    let expected = expected();
    let mut hash = AudioHash::default();
    hash.observe(&pcm(vec![1, -2], 1), &expected).unwrap();
    hash.observe(&pcm(vec![32767, -32768], 1), &expected)
        .unwrap();
    let report = hash.finish(&expected).unwrap();
    // Independently computed with Python hashlib over struct.pack('<4h', ...).
    assert_eq!(
        report.pcm_s16le_sha256,
        "23d04b5c88ae1fb6243c38676a30b686e9b9e4414133d9933319fdbf9d4a05bb"
    );
    assert_eq!(
        (
            report.frames_decoded,
            report.samples_per_channel,
            report.interleaved_samples
        ),
        (2, 2, 4)
    );
    assert_eq!(
        (
            report.nonzero_samples,
            report.peak_absolute_sample,
            report.clipped_samples
        ),
        (4, 32768, 2)
    );
}
#[test]
fn audio_aggregation_rejects_partial_channels_format_drift_and_missing_tail() {
    let expected = expected();
    let mut hash = AudioHash::default();
    assert!(hash.observe(&pcm(vec![1], 1), &expected).is_err());
    assert_eq!(hash.frames, 0);
    let mut wrong = pcm(vec![1, 2], 1);
    wrong.sample_rate = 48000;
    assert!(hash.observe(&wrong, &expected).is_err());
    hash.observe(&pcm(vec![1, 2], 1), &expected).unwrap();
    assert!(hash.finish(&expected).is_err());
    assert!(
        AudioHash::default()
            .observe(&pcm(vec![0; 6], 3), &expected)
            .is_err()
    );
}
#[test]
fn selected_indices_preserve_endpoints_and_deduplicate_short_clips() {
    assert_eq!(sample_indices(1).unwrap(), BTreeSet::from([0]));
    assert_eq!(sample_indices(2).unwrap(), BTreeSet::from([0, 1]));
    assert_eq!(
        sample_indices(311).unwrap(),
        BTreeSet::from([0, 1, 30, 155, 310])
    );
    assert!(sample_indices(0).is_err());
}
