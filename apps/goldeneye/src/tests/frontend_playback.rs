use super::*;
#[test]
fn direct_title_primes_music_before_an_audio_format_has_been_configured() {
    assert!(needs_music_chunk(false, false, 0, 0));
    assert!(needs_music_chunk(false, false, 32000, 0));
    assert!(needs_music_chunk(false, true, 32000, 15999));
    assert!(!needs_music_chunk(false, true, 32000, 16000));
    assert!(!needs_music_chunk(true, false, 0, 0));
}
#[test]
fn logo_clock_follows_consumed_pcm_and_completes_its_interpolation_tail() {
    assert_eq!(sample_clock_delta(1., 32000, 32000, 100000, 3.125), 0.);
    assert_eq!(sample_clock_delta(1., 48000, 32000, 100000, 3.125), 0.5);
    assert_eq!(sample_clock_delta(2., 32000, 32000, 100000, 3.125), 0.);
    assert!(sample_clock_delta(3.124, 99999, 32000, 100000, 3.125) > 0.001);
    // Long presentation stalls don't run a logo ahead of its queued audio.
    assert_eq!(sample_clock_delta(0., 0, 32000, 100000, 3.125), 0.);
}
