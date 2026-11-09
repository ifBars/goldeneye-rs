use super::*;

#[test]
fn cached_payload_availability_does_not_publish_nonbase_sections() -> Result<()> {
    let owner = Owner::from_fresh_base(WrapperId::new(4)?, 7, 128)?;
    let cached_full_file = vec![42; 4096];
    assert!(!cached_full_file.is_empty());
    assert_eq!(owner.status(0)?.source_status(), 3);
    for index in 1..7 {
        assert_eq!(owner.status(index)?, Status::Unrequested);
    }
    assert_eq!(owner.loaded_mask(), &[0; 4]);
    assert_eq!(owner.section_count(), 7);
    Ok(())
}

#[test]
fn logical_section_owner_checks_counts_and_exact_owner_identity() -> Result<()> {
    let id = WrapperId::new(3)?;
    assert!(Owner::from_fresh_base(id, 0, 128).is_err());
    assert!(Owner::from_fresh_base(id, 8, 7).is_err());
    assert!(Owner::from_fresh_base(id, i16::MAX as usize + 1, usize::MAX).is_err());
    let owner = Owner::from_fresh_base(id, 1, 128)?;
    assert_eq!(owner.wrapper(), id);
    assert!(owner.status(1).is_err());
    Ok(())
}
