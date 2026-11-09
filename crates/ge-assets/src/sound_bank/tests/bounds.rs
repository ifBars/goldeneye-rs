use super::*;

#[test]
fn residency_keeps_unsorted_directories_composites_and_unknown_codec() -> Result<()> {
    let bytes = fixture();
    let bank = Bank::read(&bytes, BANK, Budget::default())?;
    assert_eq!(
        (bank.resource_hash(), bank.file_bytes()),
        (BANK, bytes.len())
    );
    assert_eq!((bank.sound_count(), bank.wave_count()), (2, 2));
    assert_eq!(bank.sound(0)?.hash(), 0xf000_0001);
    assert_eq!(bank.sound(1)?.layer_count(), 21);
    let wave = bank.wave(0)?;
    assert_eq!(
        (wave.hash(), wave.sample_frames(), wave.sample_rate()),
        (0xff00_0001, 123, 22050)
    );
    assert_eq!((wave.codec_raw(), wave.channels_raw()), (0xee, 9));
    assert_eq!(bank.wave(1)?.hash(), 0x0100_0001);
    assert_eq!(bank.audio_payload(), &bytes[AUDIO..AUDIO + 8]);
    assert_eq!(wave.payload(), &bytes[AUDIO..AUDIO + 4]);
    assert!(std::ptr::eq(bank.bytes(), bytes.as_slice()));
    assert!(bank.sound(2).is_err());
    assert!(bank.wave(2).is_err());
    Ok(())
}

#[test]
fn identities_versions_and_total_size_are_independent_checks() {
    for (at, replacement) in [
        (0, b'X'),
        (8, 11),
        (0x10, b'X'),
        (0x800, b'X'),
        (0x807, 0x1f),
    ] {
        let mut bytes = fixture();
        bytes[at] = replacement;
        assert!(Bank::read(&bytes, BANK, Budget::default()).is_err());
    }
    let mut bytes = fixture();
    assert!(Bank::read(&bytes, BANK + 1, Budget::default()).is_err());
    be(&mut bytes, 0x80c, BANK + 1);
    assert!(Bank::read(&bytes, BANK, Budget::default()).is_err());
    let mut bytes = fixture();
    bytes.pop();
    assert!(Bank::read(&bytes, BANK, Budget::default()).is_err());
}

#[test]
fn preflight_budgets_reject_before_reached_directory_access() {
    let mut bytes = fixture();
    be(&mut bytes, 0x818, 0x7fff_ffff);
    for budget in [
        Budget {
            file_bytes: 0,
            ..Budget::default()
        },
        Budget {
            sounds: 1,
            ..Budget::default()
        },
        Budget {
            waves: 1,
            ..Budget::default()
        },
        Budget {
            directory_bytes: 111,
            ..Budget::default()
        },
    ] {
        let error = Bank::read(&bytes, BANK, budget).unwrap_err().to_string();
        assert!(error.contains("budget"), "{error}");
    }
}

#[test]
fn directory_and_payload_spans_cannot_escape_retained_owner() {
    for field in [0x818, 0x838] {
        let mut bytes = fixture();
        relative(&mut bytes, field, bytes_len());
        assert!(Bank::read(&bytes, BANK, Budget::default()).is_err());
    }
    let mut bytes = fixture();
    be(&mut bytes, 0x850, u32::MAX);
    assert!(Bank::read(&bytes, BANK, Budget::default()).is_err());
    let mut bytes = fixture();
    be(&mut bytes, WAVE + 0x1c, 8);
    assert!(Bank::read(&bytes, BANK, Budget::default()).is_err());
    let bytes = fixture();
    for end in [0, 0x13, 0x853] {
        assert!(Bank::read(&bytes[..end], BANK, Budget::default()).is_err());
    }
}

fn bytes_len() -> usize {
    0x960
}

#[test]
fn empty_directories_and_zero_payloads_do_not_address_unused_offsets() -> Result<()> {
    let mut bytes = fixture();
    for at in [0x814, 0x834, 0x84c] {
        be(&mut bytes, at, 0);
    }
    for at in [0x818, 0x838, 0x850] {
        be(&mut bytes, at, 0x8000_0000);
    }
    let bank = Bank::read(
        &bytes,
        BANK,
        Budget {
            sounds: 0,
            waves: 0,
            directory_bytes: 0,
            ..Budget::default()
        },
    )?;
    assert!(bank.audio_payload().is_empty());
    assert!(bank.find_sound(0)?.is_none());
    assert!(bank.sound(0).is_err());
    let mut bytes = fixture();
    be(&mut bytes, WAVE + 0x20, 0);
    be(&mut bytes, WAVE + 0x1c, u32::MAX);
    assert!(
        Bank::read(&bytes, BANK, Budget::default())?
            .wave(0)?
            .payload()
            .is_empty()
    );
    Ok(())
}
