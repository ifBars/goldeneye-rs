use super::*;
fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn input() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00ab),
        (8, 263),
        (0x14, 0x300),
        (0x18, 0x200),
        (0x60, 0x0001_0001),
        (0x64, 0x100 - 0x64),
        (0x100, 0x0d00_00ab),
        (0x108, 0x180),
        (0x180, 0x200),
    ] {
        word(&mut data, at, value);
    }
    data
}
#[test]
fn metadata_skips_poisoned_body_and_unused_topology() {
    let mut data = input();
    word(&mut data, 0x108, u32::MAX);
    let edb = Edb::parse(data).unwrap();
    let binding = edb.base_skin_reference(0x8d00_0000).unwrap().unwrap();
    assert_eq!(binding.directory_index(), 0);
    assert_eq!(binding.hash(), 0x0d00_00ab);
    assert_eq!(binding.authored_address(), u32::MAX);
    assert!(edb.base_skin_body(0x8d00_0000).is_err());
}
#[test]
fn missing_descriptor_and_nonbase_body_are_distinct() {
    let mut data = input();
    word(&mut data, 0x104, 0x8001_0000);
    word(&mut data, 0x108, 0x240);
    word(&mut data, 0x240, 0x200);
    let edb = Edb::parse(data).unwrap();
    assert!(edb.base_skin_reference(0x0d00_00ac).unwrap().is_none());
    assert!(edb.base_skin_body(0x0d00_00ac).unwrap().is_none());
    assert_eq!(
        edb.base_skin_reference(0x0d00_00ab)
            .unwrap()
            .unwrap()
            .raw_section(),
        0x8001
    );
    assert!(edb.base_skin_body(0x0d00_00ab).is_err());
}
#[test]
fn base_header_read_skips_topology_but_never_uses_tail_bytes() {
    let mut data = input();
    word(&mut data, 0x184, u32::MAX); // bone count is not consumed by body binding
    let edb = Edb::parse(data.clone()).unwrap();
    assert_eq!(
        edb.base_skin_body(0x0d00_00ab).unwrap().unwrap().address(),
        0x180
    );
    word(&mut data, 0x108, 0x240);
    word(&mut data, 0x240, 0x200);
    assert!(
        Edb::parse(data)
            .unwrap()
            .base_skin_body(0x0d00_00ab)
            .is_err()
    );
}
#[test]
fn metadata_is_bounded_to_base_and_checks_retained_identity() {
    let mut data = input();
    word(&mut data, 0x18, 0x108);
    assert!(
        Edb::parse(data)
            .unwrap()
            .base_skin_reference(0x8d00_0000)
            .is_err()
    );
    let mut edb = Edb::parse(input()).unwrap();
    edb.header.hash ^= 1;
    assert!(edb.base_skin_reference(0x8d00_0000).is_err());
}
