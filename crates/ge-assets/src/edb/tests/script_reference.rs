use super::super::Edb;
fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x200];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00ab),
        (8, 263),
        (0x14, 0x200),
        (0x18, 0x200),
        (0x68, 0x0001_0001),
        (0x6c, 0x100 - 0x6c),
        (0x100, 0x0400_00ab),
        (0x108, 0x140),
        (0x10c, 0xdead_beef),
        (0x140, 0x300),
    ] {
        word(&mut data, at, value);
    }
    data
}
#[test]
fn descriptor_lookup_does_not_touch_a_later_unavailable_body() {
    let mut data = fixture();
    word(&mut data, 0x18, 0x142);
    let edb = Edb::parse(data).unwrap();
    let descriptor = edb
        .base_animation_script_reference(0x8400_0000)
        .unwrap()
        .unwrap();
    assert_eq!(descriptor.directory_index, 0);
    assert_eq!(descriptor.hash, 0x0400_00ab);
    assert!(edb.base_animation_script_body(0).is_err());
    assert!(edb.frontend_script(0x8400_0000).unwrap().is_some());
}
#[test]
fn cached_nonbase_body_is_not_live_residency_and_zero_body_stays_distinct_from_missing() {
    let mut data = fixture();
    word(&mut data, 0x104, 0x0001_0000);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.base_animation_script_reference(0x8400_0000)
            .unwrap()
            .is_some()
    );
    assert!(edb.base_animation_script_body(0).is_err());
    let mut data = fixture();
    word(&mut data, 0x108, 0);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.base_animation_script_reference(0x8400_0000)
            .unwrap()
            .is_some()
    );
    assert!(edb.base_animation_script_body(0).unwrap().is_none());
    assert!(
        edb.base_animation_script_reference(0x0400_00ac)
            .unwrap()
            .is_none()
    );
}
#[test]
fn resident_body_validation_is_reached_after_metadata_and_bounds_the_descriptor_ordinal() {
    let edb = Edb::parse(fixture()).unwrap();
    assert!(edb.base_animation_script_body(0).unwrap().is_some());
    assert!(edb.base_animation_script_body(1).is_err());
    assert!(edb.base_animation_script_body(32767).is_err());
    let mut data = fixture();
    word(&mut data, 0x140, 0xdead_beef);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.base_animation_script_reference(0x8400_0000)
            .unwrap()
            .is_some()
    );
    assert!(edb.base_animation_script_body(0).is_err());
}
