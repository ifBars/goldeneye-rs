use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i32 - field as i32) as u32);
}

fn budget() -> Budget {
    Budget {
        group_fields: 2,
        rows: 4,
        row_bytes: 52,
    }
}

fn table() -> Table {
    Table {
        name: "spreadsheets",
        count: 2,
        hash_size_raw: 2,
        offset: 0x20,
    }
}

fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x200];
    for (index, hash, address) in [(0, 0x1400_0100, 0x100), (1, 0x1400_0101, 0x140)] {
        let at = 0x20 + index * DIRECTORY_STRIDE;
        word(&mut data, at, hash);
        word(&mut data, at + 4, 0x1234_5678 + index as u32);
        word(&mut data, at + 8, address);
        // This invalid runtime pointer must never replace the raw body offset.
        word(&mut data, at + 12, 0xffff_ffff);
        word(&mut data, at + 16, 0xabcd_ef00 + index as u32);
    }
    word(&mut data, 0x100, 0xdead_beef);
    relative(&mut data, 0x104, 0x180);
    relative(&mut data, 0x108, 0x80);
    word(&mut data, 0x180, 2);
    for (index, value) in [9, 0x7fc0_0001, u32::MAX, 0, 5, 8, 7, 6, 5, 4]
        .into_iter()
        .enumerate()
    {
        word(&mut data, 0x184 + index * 4, value);
    }
    word(&mut data, 0x80, 1);
    for index in 0..13 {
        word(&mut data, 0x84 + index * 4, 100 + index as u32);
    }
    word(&mut data, 0x140, 0);
    relative(&mut data, 0x144, 0x1c0);
    word(&mut data, 0x1c0, 0);
    data
}

#[test]
fn directory_uses_twenty_byte_stride_and_preserves_opaque_descriptor_and_header() {
    let data = fixture();
    let sheet = read(&data, &table(), 0x1400_0101).unwrap().unwrap();
    let descriptor = sheet.descriptor().unwrap();
    assert_eq!(descriptor.directory_index, 1);
    assert_eq!(descriptor.address, 0x140);
    assert_eq!(
        (descriptor.section_raw, descriptor.debug_raw),
        (0x1234, 0x5679)
    );
    assert_eq!(descriptor.runtime_pointer_raw, u32::MAX);
    assert_eq!(descriptor.word_10_raw, 0xabcd_ef01);
    assert_eq!(sheet.address(), 0x140);
    assert_eq!(sheet.header_word_raw().unwrap(), 0);
    let indexed = read(&data, &table(), 0x8000_0001).unwrap().unwrap();
    assert_eq!(indexed.address(), sheet.address());
    assert!(read(&data, &table(), 0x1400_9999).unwrap().is_none());
    assert!(read(&data, &table(), 2).unwrap().is_none());
}

#[test]
fn signed_group_links_are_field_relative_and_rows_keep_raw_word_bits() {
    let data = fixture();
    let sheet = read_at(&data, 0x100).unwrap();
    assert!(sheet.descriptor().is_none());
    assert_eq!(sheet.header_word_raw().unwrap(), 0xdead_beef);
    let forward = sheet.group(0, 20, budget()).unwrap().unwrap();
    assert_eq!(
        (
            forward.pointer_field,
            forward.block_address,
            forward.relative_raw
        ),
        (0x104, 0x180, 0x7c)
    );
    assert_eq!(
        forward.words::<5>(0).unwrap(),
        [9, 0x7fc0_0001, u32::MAX, 0, 5]
    );
    assert_eq!(forward.words::<5>(1).unwrap(), [8, 7, 6, 5, 4]);
    assert_eq!(forward.row(1).unwrap().len(), 20);
    let backward = sheet.group(1, 52, budget()).unwrap().unwrap();
    assert_eq!(
        (
            backward.pointer_field,
            backward.block_address,
            backward.relative_raw
        ),
        (0x108, 0x80, -0x88)
    );
    assert_eq!(
        backward.words::<13>(0).unwrap(),
        std::array::from_fn(|index| 100 + index as u32)
    );
}

#[test]
fn unrequested_group_pointers_and_row_layouts_remain_unread() {
    let mut data = fixture();
    word(&mut data, 0x108, 0x7fff_ffff);
    let sheet = read_at(&data, 0x100).unwrap();
    let only_first = Budget {
        group_fields: 1,
        ..budget()
    };
    assert_eq!(
        sheet.group(0, 20, only_first).unwrap().unwrap().row_count,
        2
    );
    assert!(sheet.group(1, 52, only_first).is_err());
    assert!(sheet.group(1, 52, budget()).is_err());
    // Even a body with no addressed group field is a valid opaque header view.
    let header_only = read_at(&data[..0x104], 0x100).unwrap();
    assert_eq!(header_only.header_word_raw().unwrap(), 0xdead_beef);
    assert!(header_only.group(0, 20, budget()).is_err());
}

#[test]
fn absent_group_is_not_a_present_empty_group() {
    let mut data = fixture();
    word(&mut data, 0x104, 0);
    assert!(
        read_at(&data, 0x100)
            .unwrap()
            .group(0, 20, budget())
            .unwrap()
            .is_none()
    );
    let empty = read_at(&data[..0x1c4], 0x140)
        .unwrap()
        .group(
            0,
            52,
            Budget {
                rows: 0,
                ..budget()
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(empty.row_count, 0);
    assert!(empty.row(0).is_err());
}

#[test]
fn counts_strides_rows_and_group_access_have_explicit_host_bounds() {
    let data = fixture();
    let sheet = read_at(&data, 0x100).unwrap();
    assert!(sheet.group(0, 0, budget()).is_err());
    assert!(sheet.group(0, 53, budget()).is_err());
    assert!(
        sheet
            .group(
                0,
                20,
                Budget {
                    rows: 1,
                    ..budget()
                }
            )
            .is_err()
    );
    assert!(
        sheet
            .group(
                0,
                20,
                Budget {
                    group_fields: 0,
                    ..budget()
                }
            )
            .is_err()
    );
    assert!(
        sheet
            .group(
                usize::MAX - 1,
                20,
                Budget {
                    group_fields: usize::MAX,
                    ..budget()
                }
            )
            .is_err()
    );
    let rows = sheet.group(0, 20, budget()).unwrap().unwrap();
    assert!(rows.row(2).is_err());
    assert!(rows.row(usize::MAX).is_err());
    assert!(rows.words::<4>(0).is_err());
    assert!(rows.words::<6>(0).is_err());
}

#[test]
fn malformed_group_targets_and_truncated_payloads_fail_at_addressed_data() {
    let data = fixture();
    assert!(read_at(&data, usize::MAX).is_err());
    assert!(read_at(&data[..0x103], 0x100).is_err());
    assert!(
        read_at(&data[..0x1ab], 0x100)
            .unwrap()
            .group(0, 20, budget())
            .is_err()
    );
    for raw in [0x8000_0000, 0x7fff_ffff, (data.len() - 0x104) as u32] {
        let mut bad = data.clone();
        word(&mut bad, 0x104, raw);
        assert!(
            read_at(&bad, 0x100)
                .unwrap()
                .group(0, 20, budget())
                .is_err()
        );
    }
    let mut huge_count = data;
    word(&mut huge_count, 0x180, u32::MAX);
    assert!(
        read_at(&huge_count, 0x100)
            .unwrap()
            .group(0, 20, budget())
            .is_err()
    );
}

#[test]
fn wrong_directory_short_descriptor_and_bad_raw_body_are_rejected() {
    let mut data = fixture();
    let mut wrong = table();
    wrong.name = "animation_scripts";
    assert!(read(&data, &wrong, 0x1400_0100).is_err());
    assert!(read(&data[..0x47], &table(), 0x8000_0001).is_err());
    word(&mut data, 0x20 + 8, u32::MAX);
    assert!(read(&data, &table(), 0x1400_0100).is_err());
}
