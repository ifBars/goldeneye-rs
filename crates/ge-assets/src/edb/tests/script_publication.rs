use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x400];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00ab),
        (8, 263),
        (12, 0x1234_5678),
        (0x14, 0x400),
        (0x18, 0x200),
        (0x40, 0x0002_0002),
        (0x44, 0x100 - 0x44),
        (0x68, 0x0001_0001),
        (0x6c, 0xd0 - 0x6c),
        (0xd0, 0x0400_00ab),
        (0xd4, 0x8001_0042),
        (0xd8, 0x380),
        (0xdc, 0xfeed_1234),
        (0x100, 0x0800_0000),
        (0x104, 0x180),
        (0x108, 0x200),
        (0x10c, 0xaabb_ccdd),
        (0x110, 0x0800_0001),
        (0x114, 0x300),
        (0x118, 0x400),
        (0x11c, 0),
        (0x184, 8),
    ] {
        word(&mut data, at, value);
    }
    data
}

#[test]
fn callback_mask_comes_from_section_zero_header_and_skips_unused_fields() {
    let mut data = fixture();
    word(&mut data, 0x108, 0xffff_ffff);
    word(&mut data, 0x10c, 0xffff_ffff);
    let edb = Edb::parse(data).unwrap();
    let view = edb.script_publication().unwrap();
    assert_eq!(view.resource_hash(), 0x0102_00ab);
    assert_eq!(view.base_callback_mask().unwrap(), 8);
    let mut data = fixture();
    word(&mut data, 0x184, 0);
    assert_eq!(
        Edb::parse(data)
            .unwrap()
            .script_publication()
            .unwrap()
            .base_callback_mask()
            .unwrap(),
        0
    );
}

#[test]
fn descriptor_and_section_preserve_actual_raw_metadata_without_body_reads() {
    let mut data = fixture();
    word(&mut data, 0xd8, 0xffff_ffff);
    word(&mut data, 0x114, 0xffff_ffff);
    let edb = Edb::parse(data).unwrap();
    let script = edb
        .script_publication()
        .unwrap()
        .script(0x8400_0000)
        .unwrap()
        .unwrap();
    let descriptor = script.descriptor();
    assert_eq!(descriptor.directory_index, 0);
    assert_eq!(descriptor.hash, 0x0400_00ab);
    assert_eq!(descriptor.section, 0x8001);
    assert_eq!(descriptor.debug, 0x42);
    assert_eq!(descriptor.address, u32::MAX);
    assert_eq!(descriptor.runtime_pointer_raw, 0xfeed_1234);
    let section = script.section().unwrap();
    assert_eq!(section.directory_index(), 1);
    assert_eq!(section.key().unwrap(), 0x0800_0001);
    assert_eq!(section.runtime_job_raw().unwrap(), 0);
    assert!(section.span().is_err());
}

#[test]
fn native_lookup_supports_hash_and_coded_index_without_section_preflight() {
    let mut data = fixture();
    word(&mut data, 0xd4, 0x7fff_0042);
    word(&mut data, 0x104, 0xffff_ffff);
    let edb = Edb::parse(data).unwrap();
    let view = edb.script_publication().unwrap();
    let by_hash = view.script(0x0400_00ab).unwrap().unwrap();
    assert_eq!(by_hash.descriptor().directory_index, 0);
    assert!(by_hash.section().is_err());
    assert!(view.script(0x8400_0001).unwrap().is_none());
    assert!(view.script(0x0400_00ac).unwrap().is_none());
}

#[test]
fn section_span_is_checked_metadata_and_never_reads_nonbase_payload() {
    let edb = Edb::parse(fixture()).unwrap();
    let script = edb
        .script_publication()
        .unwrap()
        .script(0x8400_0000)
        .unwrap()
        .unwrap();
    assert_eq!(script.section().unwrap().span().unwrap(), 0x300..0x400);
    assert!(edb.base_animation_script_body(0).is_err());
    for (start, end) in [(0x380, 0x300), (0x300, 0x401)] {
        let mut data = fixture();
        word(&mut data, 0x114, start);
        word(&mut data, 0x118, end);
        let edb = Edb::parse(data).unwrap();
        let script = edb
            .script_publication()
            .unwrap()
            .script(0x8400_0000)
            .unwrap()
            .unwrap();
        assert!(script.section().unwrap().span().is_err());
        assert_eq!(script.section().unwrap().runtime_job_raw().unwrap(), 0);
    }
}

#[test]
fn cached_later_bytes_cannot_supply_truncated_base_metadata() {
    let mut data = fixture();
    word(&mut data, 0x18, 0x11c);
    let edb = Edb::parse(data).unwrap();
    let view = edb.script_publication().unwrap();
    let script = view.script(0x8400_0000).unwrap().unwrap();
    let section = script.section().unwrap();
    assert_eq!(section.key().unwrap(), 0x0800_0001);
    assert_eq!(section.span().unwrap(), 0x300..0x400);
    assert!(section.runtime_job_raw().is_err());
    assert!(view.base_callback_mask().is_err());

    let mut data = fixture();
    word(&mut data, 0x18, 0xda);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.script_publication()
            .unwrap()
            .script(0x8400_0000)
            .is_err()
    );
}

#[test]
fn callback_header_and_row_are_individually_bounded_to_base() {
    for (field, value) in [(0x104, 0x1fc), (0x104, u32::MAX), (0x44, 0x240 - 0x44)] {
        let mut data = fixture();
        word(&mut data, field, value);
        let edb = Edb::parse(data).unwrap();
        assert!(
            edb.script_publication()
                .unwrap()
                .base_callback_mask()
                .is_err()
        );
    }
    let mut data = fixture();
    word(&mut data, 0x40, 0);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.script_publication()
            .unwrap()
            .base_callback_mask()
            .is_err()
    );
}

#[test]
fn supported_version_and_retained_resource_identity_are_checked() {
    let mut data = fixture();
    word(&mut data, 8, 262);
    assert!(Edb::parse(data).unwrap().script_publication().is_err());
    let mut edb = Edb::parse(fixture()).unwrap();
    edb.header.hash ^= 1;
    assert!(edb.script_publication().is_err());
    let mut edb = Edb::parse(fixture()).unwrap();
    edb.header.base_bytes -= 4;
    assert!(edb.script_publication().is_err());
}
