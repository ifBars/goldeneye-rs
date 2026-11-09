use super::*;

fn packet() -> Vec<u8> {
    let mut bytes = vec![0; 36];
    bytes[0] = 5;
    bytes[4..6].copy_from_slice(&11i16.to_be_bytes());
    bytes[6..8].copy_from_slice(&25i16.to_be_bytes());
    bytes[10] = 7;
    bytes[11] = 255;
    bytes[20] = 1;
    bytes[21] = 100;
    bytes[32..36].copy_from_slice(&0x2d7011adu32.to_be_bytes());
    bytes
}

fn scene() -> Scene {
    let program = Program {
        scripts: BTreeMap::from([(
            ROOT,
            core::Script {
                fps: 60.,
                slots: 8,
                commands: vec![
                    core::Command {
                        start: 11,
                        duration: 25,
                        kind: core::CommandKind::External {
                            opcode: 5,
                            payload: packet(),
                        },
                    },
                    core::Command {
                        start: 36,
                        duration: 30,
                        kind: core::CommandKind::Remove(vec![7]),
                    },
                ],
            },
        )]),
        tracks: BTreeMap::new(),
        durations: BTreeMap::new(),
        geometry: BTreeMap::new(),
        colors: BTreeMap::new(),
        draws: BTreeMap::new(),
        intro: Some(Segment {
            start: 0.,
            end: 29.,
        }),
        outro: Some(Segment {
            start: 30.,
            end: 49.,
        }),
    };
    Scene::new(Arc::new(program)).unwrap()
}

#[test]
fn intro_cue_starts_once_at_its_source_frame_and_never_repeats_on_hold_or_confirm() {
    let mut scene = scene();
    scene.layers(0.).unwrap();
    scene.layers(10. / 60.).unwrap();
    assert!(scene.take_cues().is_empty());
    scene.layers(11. / 60.).unwrap();
    let cues = scene.take_cues();
    assert_eq!(cues.len(), 1);
    assert_eq!(
        (cues[0].sound_hash, cues[0].gain, cues[0].start_frame),
        (0x2d7011ad, 1., 11)
    );
    scene.layers(1.).unwrap();
    assert!(scene.take_cues().is_empty());
    assert!(scene.request_outro(1.).unwrap());
    scene.layers(1.2).unwrap();
    assert!(scene.take_cues().is_empty());
    assert_eq!(scene.evidence.cue_requests, 1);
}

#[test]
fn reconstruction_suppresses_sound_and_removal_before_evaluation_does_not_start_it() {
    let mut running = scene();
    running.layers(1.).unwrap();
    assert_eq!(running.take_cues().len(), 1);
    running.layers(0.2).unwrap();
    running.layers(0.3).unwrap();
    assert!(running.take_cues().is_empty());
    let mut jumped = scene();
    assert!(jumped.request_outro(1.).unwrap());
    jumped.layers(1.2).unwrap();
    assert!(jumped.take_cues().is_empty());
}

#[test]
fn sound_command_scope_rejects_unsupported_tracks_flags_and_partial_records() {
    assert!(CueRequest::decode(&packet()[..35]).is_err());
    for offset in [11, 16, 20, 22, 23, 24, 28] {
        let mut bad = packet();
        bad[offset] = if offset == 20 { 2 } else { 1 };
        assert!(CueRequest::decode(&bad).is_err(), "{offset}");
    }
    let mut quiet = packet();
    quiet[21] = 0;
    assert_eq!(CueRequest::decode(&quiet).unwrap().gain, 0.);
}
