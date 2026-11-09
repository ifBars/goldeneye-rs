use super::*;
use crate::frontend_cue::tests::authored_cue;

fn authored() -> MenuAudio {
    MenuAudio {
        cues: SPECS.map(|spec| authored_cue(spec.sound)),
        voices_started: 0,
        capacity_dropped: 0,
    }
}

#[test]
fn none_and_rejected_voice_do_not_claim_successful_submission() {
    let mut menu = authored();
    assert_eq!(
        menu.submit(Effect::None, 1, |_, _, _, _| panic!("no input effect"))
            .unwrap(),
        None
    );
    assert!(
        menu.submit(Effect::Navigate, 1, |_, _, _, _| anyhow::bail!(
            "device failed"
        ))
        .is_err()
    );
    assert_eq!(menu.evidence().voices_started, 0);
}

#[test]
fn effects_select_distinct_cached_pcm_and_keep_source_gain_separate() {
    let mut menu = authored();
    for (index, effect) in [
        Effect::Navigate,
        Effect::Accept,
        Effect::Back,
        Effect::Adjust,
    ]
    .into_iter()
    .enumerate()
    {
        let expected = menu.cues[index].samples();
        for _ in 0..2 {
            assert_eq!(
                menu.submit(effect, 1, |rate, channels, pcm, gain| {
                    assert_eq!((rate, channels, gain), (32_000, 1, 1.0));
                    assert!(Arc::ptr_eq(&expected, &pcm));
                    Ok(17)
                })
                .unwrap(),
                Some(17)
            );
        }
    }
    let evidence = menu.evidence();
    assert_eq!(evidence.voices_started, 8);
    assert_eq!(
        evidence
            .cues
            .iter()
            .map(|cue| cue.info.sound_hash)
            .collect::<Vec<_>>(),
        vec![0x2d70044a, 0x2d70044d, 0x2d700449, 0x2d70044e]
    );
}

#[test]
fn saturated_voice_slots_drop_newest_without_queueing_or_losing_cached_pcm() {
    let mut menu = authored();
    let cached = menu.cues[0].samples();
    assert_eq!(
        menu.submit(Effect::Navigate, 0, |_, _, _, _| {
            panic!("existing voices keep all occupied slots")
        })
        .unwrap(),
        None
    );
    assert_eq!(menu.evidence().capacity_dropped, 1);
    assert_eq!(menu.evidence().voices_started, 0);
    assert!(Arc::ptr_eq(&cached, &menu.cues[0].samples()));
    assert!(
        menu.submit(Effect::Navigate, 1, |_, _, pcm, _| {
            assert!(Arc::ptr_eq(&cached, &pcm));
            Ok(9)
        })
        .unwrap()
        .is_some()
    );
    assert_eq!(menu.evidence().voices_started, 1);
}
