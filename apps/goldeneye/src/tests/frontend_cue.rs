use super::*;

fn beput(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

fn fixture(sound: u32) -> (Vec<u8>, CueSpec, sound_info::Entry, SourceOrigin) {
    // Authored one-block mono DSP residuals, with a deliberately partial final
    // block. No proprietary bank, PCM or recovered instruction bytes.
    let mut bytes = vec![0; 0x1048];
    bytes[..4].copy_from_slice(b"MUSX");
    bytes[4..8].copy_from_slice(&BANK.to_le_bytes());
    bytes[8..12].copy_from_slice(&10u32.to_le_bytes());
    let length = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    bytes[0x10..0x14].copy_from_slice(b"WII_");
    bytes[0x800..0x804].copy_from_slice(b"SBNK");
    for (at, value) in [
        (0x804, 0x1e),
        (0x80c, BANK),
        (0x814, 1),
        (0x818, 0x3c),
        (0x834, 1),
        (0x838, 0x13c),
        (0x84c, 0x48),
        (0x850, 0x1000),
        (0x854, sound),
        (0x85c, 0xa4),
        (0x900, 0x20),
        (0x904, 0x70),
        (0x974, 0x2da00001),
        (0x978, 2),
        (0x994, 0x48),
        (0x1008, 1),
    ] {
        beput(&mut bytes, at, value);
    }
    bytes[0x860] = 1;
    bytes[0x98c..0x98e].copy_from_slice(&32_000u16.to_be_bytes());
    bytes[0x98e] = 0x13;
    bytes[0x98f] = 1;
    bytes[0x1000..0x1004].copy_from_slice(b"NGCA");
    bytes[0x1041] = 0x1f; // +1, -1 with zero predictors.
    let spec = CueSpec {
        sound,
        wave: 0x2da00001,
        rate: 32_000,
        channels: 1,
        frames: 2,
        flags: 0,
        scope: "Authored cue fixture",
    };
    let row = sound_info::Entry {
        index: 0,
        address: 0x2c,
        sound_hash: sound,
        minimum_range_bits: 0,
        maximum_range_bits: 0,
        flags_raw: 0,
    };
    let origin = SourceOrigin {
        bank: ResourceOrigin {
            index: "authored",
            archive: "authored".into(),
            resource_hash: BANK,
            offset: 0,
            bytes: u64::from(length),
        },
        sound_info: ResourceOrigin {
            index: "authored",
            archive: "authored".into(),
            resource_hash: SOUND_INFO,
            offset: 0,
            bytes: 0x3c,
        },
    };
    (bytes, spec, row, origin)
}

pub(crate) fn authored_cue(sound: u32) -> Cue {
    let (bytes, spec, row, origin) = fixture(sound);
    Cue::decode(spec, row, origin, |at, n| {
        Ok(bytes[at as usize..at as usize + n].to_vec())
    })
    .unwrap()
}

#[test]
fn finite_cue_decodes_only_declared_samples_and_reuses_pcm_ownership() {
    let cue = authored_cue(0x2d700001);
    let first = cue.samples();
    let second = cue.samples();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(&*first, &[1, -1]);
    let evidence = cue.evidence();
    assert_eq!(
        (
            evidence.decoded_frames,
            evidence.interleaved_samples,
            evidence.nonzero_samples,
            evidence.peak_absolute_sample
        ),
        (2, 2, 2, 1)
    );
    let json = serde_json::to_value(evidence).unwrap();
    assert!(json.get("samples").is_none());
    assert!(json.get("pcm").is_none());
}

#[test]
fn identity_metadata_and_loop_failures_do_not_decode_pcm() {
    let (bytes, spec, row, origin) = fixture(0x2d700001);
    for altered in [
        sound_info::Entry {
            sound_hash: 0x2d700002,
            ..row
        },
        sound_info::Entry {
            flags_raw: 1,
            ..row
        },
        sound_info::Entry {
            flags_raw: 1 << 24,
            ..row
        },
        sound_info::Entry {
            flags_raw: 1 << 26,
            ..row
        },
    ] {
        assert!(
            Cue::decode(spec, altered, origin.clone(), |_, _| {
                panic!("metadata must be checked before bank reads")
            })
            .is_err()
        );
    }
    let mut looping = bytes.clone();
    looping[0x98e] = 0x1b;
    assert!(
        Cue::decode(spec, row, origin.clone(), |at, n| {
            Ok(looping[at as usize..at as usize + n].to_vec())
        })
        .is_err()
    );
    assert!(
        Cue::decode(
            CueSpec {
                wave: 0x2da00002,
                ..spec
            },
            row,
            origin,
            |at, n| { Ok(bytes[at as usize..at as usize + n].to_vec()) }
        )
        .is_err()
    );
}

#[test]
fn truncated_pcm_is_an_error_instead_of_a_publishable_partial_cue() {
    let (bytes, spec, row, origin) = fixture(0x2d700001);
    assert!(
        Cue::decode(spec, row, origin, |at, n| {
            let count = if at == 0x1040 { n - 1 } else { n };
            Ok(bytes[at as usize..at as usize + count].to_vec())
        })
        .is_err()
    );
}
