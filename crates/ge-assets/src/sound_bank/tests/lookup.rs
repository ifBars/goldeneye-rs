use super::*;

#[test]
fn lookup_preserves_storage_order_and_only_rejects_requested_ambiguity() -> Result<()> {
    let mut bytes = fixture();
    let bank = Bank::read(&bytes, BANK, Budget::default())?;
    assert_eq!(bank.find_sound(0x1000_0002)?.unwrap().index(), 1);
    assert!(bank.find_sound(2)?.is_none());
    be(&mut bytes, SOUND + 16, 0xf000_0001);
    let bank = Bank::read(&bytes, BANK, Budget::default())?;
    assert_eq!(bank.sound_count(), 2);
    assert!(bank.find_sound(2)?.is_none());
    assert!(bank.find_sound(0xf000_0001).is_err());
    Ok(())
}

#[test]
fn unselected_programs_are_lazy_and_composites_remain_resident() -> Result<()> {
    let mut bytes = fixture();
    be(&mut bytes, SOUND + 16 + 8, 0x8000_0000);
    let bank = Bank::read(&bytes, BANK, Budget::default())?;
    assert!(bank.find_sound(7)?.is_none());
    assert_eq!(bank.sound(1)?.layer_count(), 21);
    assert!(
        bank.sound(1)?
            .resident_wave()
            .unwrap_err()
            .to_string()
            .contains("single-layer")
    );
    assert_eq!(bank.sound(0)?.resident_wave()?.unwrap().index(), 1);
    Ok(())
}

#[test]
fn external_sound_does_not_follow_poisoned_nonresident_wave_pointer() -> Result<()> {
    let mut bytes = fixture();
    bytes[PARAMETERS + 11] = 2;
    be(&mut bytes, PAIR + 4, 0x8000_0000);
    let bank = Bank::read(&bytes, BANK, Budget::default())?;
    assert!(bank.sound(0)?.resident_wave()?.is_none());
    bytes[PARAMETERS + 11] = 0;
    assert!(
        Bank::read(&bytes, BANK, Budget::default())?
            .sound(0)?
            .resident_wave()
            .is_err()
    );
    Ok(())
}

#[test]
fn resident_references_require_a_checked_row_of_the_same_bank() -> Result<()> {
    for target in [WAVE + 1, WAVE + 80, 0x10] {
        let mut bytes = fixture();
        relative(&mut bytes, PAIR + 4, target);
        assert!(
            Bank::read(&bytes, BANK, Budget::default())?
                .sound(0)?
                .resident_wave()
                .is_err()
        );
    }
    let first = fixture();
    let mut second = fixture();
    second[AUDIO + 4] = 200;
    let a = Bank::read(&first, BANK, Budget::default())?
        .sound(0)?
        .resident_wave()?
        .unwrap();
    let b = Bank::read(&second, BANK, Budget::default())?
        .sound(0)?
        .resident_wave()?
        .unwrap();
    assert_eq!(a.resource_hash(), BANK);
    assert_eq!(a.payload()[0], 101);
    assert_eq!(b.payload()[0], 200);
    assert!(std::ptr::eq(a.payload(), &first[AUDIO + 4..AUDIO + 8]));
    Ok(())
}

#[test]
fn malformed_reached_program_pointers_fail_after_successful_residency() -> Result<()> {
    for field in [SOUND + 8, PAIR] {
        let mut bytes = fixture();
        be(&mut bytes, field, 0x8000_0000);
        let bank = Bank::read(&bytes, BANK, Budget::default())?;
        assert!(bank.sound(0)?.resident_wave().is_err());
    }
    Ok(())
}

#[test]
fn debug_exposes_metadata_without_dumping_compressed_bytes() -> Result<()> {
    let bytes = fixture();
    let bank = Bank::read(&bytes, BANK, Budget::default())?;
    for text in [
        format!("{bank:?}"),
        format!("{:?}", bank.sound(0)?),
        format!("{:?}", bank.wave(0)?),
    ] {
        assert!(text.len() < 200);
        assert!(!text.contains("101, 103, 107, 109"));
        assert!(!text.contains("descriptor"));
    }
    Ok(())
}
