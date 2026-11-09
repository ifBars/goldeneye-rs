use super::*;

fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    put(
        data,
        field,
        i32::try_from(target as i64 - field as i64).unwrap() as u32,
    );
}
fn fixture(mask: u32, properties: &[u32]) -> (Vec<u8>, Table) {
    let mut data = vec![0; 0x500];
    let maps = Table {
        name: "maps",
        count: 1,
        hash_size_raw: 1,
        offset: 0x20,
    };
    put(&mut data, 0x20, 0x0500_00a5);
    put(&mut data, 0x28, 0x80);
    put(&mut data, 0x80, 0x500);
    relative(&mut data, 0xd8, 0x100);
    put(&mut data, 0x100, 2);
    relative(&mut data, 0x104, 0x140);
    relative(&mut data, 0x10c, 0x180);
    put(&mut data, 0x180, 28);
    relative(&mut data, 0x140, 0x200);
    relative(&mut data, 0x148, 0x400);
    for record in [0x200, 0x400] {
        put(&mut data, record + 4, 0x201);
        put(&mut data, record + 8, mask);
        for (offset, value) in [
            0x8000_0000,
            0x7fc0_00a5,
            0x3f80_0000,
            0x4000_0000,
            0x4040_0000,
            0x4080_0000,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut data, record + 0xc + offset * 4, value);
        }
        for (offset, &value) in properties.iter().enumerate() {
            put(&mut data, record + 0x30 + offset * 4, value);
        }
    }
    (data, maps)
}
fn descriptor(enable_byte_raw: i8) -> Descriptor {
    Descriptor {
        native_type_index: 28,
        source_address: 0x8056_c578,
        class: 0x1e00_00a5,
        subtype: 0x1f00_00a5,
        enable_byte_raw,
    }
}
fn common(data: &[u8], maps: &Table, words: usize) -> Result<Common> {
    match read(data, maps, 0x8500_0000, Budget::default())?
        .unwrap()
        .row(0)?
        .read_common(&descriptor(1), None, words)?
    {
        Admission::Admitted(common) => Ok(common),
        _ => anyhow::bail!("authored enabled record was not admitted"),
    }
}

#[test]
fn packed_fields_preserve_source_pose_and_original_ordinals_without_arithmetic() {
    let mask = (1 << 0)
        | (1 << 3)
        | (1 << 15)
        | (1 << 16)
        | (1 << 23)
        | (1 << 24)
        | (1 << 25)
        | (1 << 26)
        | (1 << 31);
    let (data, maps) = fixture(mask, &[100, 103, 115, 1, u32::MAX, 0xa5, 0xa6, 0]);
    let result = common(&data, &maps, 8).unwrap();
    assert_eq!(result.header_flags_raw, 0x201);
    assert_eq!(result.property_mask_raw, mask);
    assert_eq!(
        (result.numeric[0], result.numeric[3], result.numeric[15]),
        (100, 103, 115)
    );
    assert!(
        result
            .numeric
            .iter()
            .enumerate()
            .all(|(index, &value)| matches!(index, 0 | 3 | 15) || value == 0)
    );
    assert_eq!(result.reference_ordinals[0], 1);
    assert!(
        result.reference_ordinals[1..]
            .iter()
            .all(|&value| value == u32::MAX)
    );
    assert_eq!((result.word_78_raw, result.word_74_raw), (0xa5, 0xa6));
    assert!(result.animator_present);
    assert_eq!(result.animator_index_raw, 0);
    assert_eq!(
        result.position_bits,
        [0x8000_0000, 0x7fc0_00a5, 0x3f80_0000, 0]
    );
    assert_eq!(
        result.rotation_bits,
        [0x4000_0000, 0x4040_0000, 0x4080_0000, 0]
    );
    assert_eq!(result.property_words_read, 8);
    let view = read(&data, &maps, 0, Budget::default()).unwrap().unwrap();
    assert_eq!(view.row_count(), 2);
    assert_eq!(view.row(1).unwrap().locator().trigger_ordinal, 1);
}

#[test]
fn absent_properties_read_neither_padding_nor_property_cursor_at_eof() {
    let (mut data, maps) = fixture(0, &[]);
    put(&mut data, 0x100, 1);
    data.truncate(0x224);
    let result = common(&data, &maps, 0).unwrap();
    assert_eq!(result.numeric, [0; 16]);
    assert_eq!(result.reference_ordinals, [u32::MAX; 8]);
    assert_eq!((result.word_74_raw, result.word_78_raw), (0, 0));
    assert!(!result.animator_present);
    assert_eq!(result.animator_index_raw, 0);
    assert_eq!(result.property_words_read, 0);
}

#[test]
fn disabled_then_filter_admission_skips_malformed_common_payload() {
    let (mut data, maps) = fixture(0, &[]);
    relative(&mut data, 0x140, 0x4fe);
    let view = read(&data, &maps, 0x8500_0000, Budget::default())
        .unwrap()
        .unwrap();
    let row = view.row(0).unwrap();
    assert_eq!(
        row.read_common(&descriptor(0), None, 0).unwrap(),
        Admission::Disabled
    );
    assert_eq!(
        row.read_common(&descriptor(0), Some(&[]), 0).unwrap(),
        Admission::Disabled
    );
    assert_eq!(
        row.read_common(&descriptor(1), Some(&[]), 0).unwrap(),
        Admission::Filtered
    );
    assert_eq!(
        row.read_common(&descriptor(1), Some(&[0x1f00_00b0]), 0)
            .unwrap(),
        Admission::Filtered
    );
    assert!(row.read_common(&descriptor(1), None, 0).is_err());
    assert!(
        row.read_common(&descriptor(1), Some(&[0x1f00_00a5]), 0)
            .is_err()
    );
}

#[test]
fn callback_query_position_precedes_admission_and_does_not_read_common_rotation_or_properties() {
    let (mut data, maps) = fixture(u32::MAX, &[]);
    // XYZ ends at+18. Common rotation and every property are outside the file.
    data.truncate(0x218);
    let view = read(&data, &maps, 0, Budget::default()).unwrap().unwrap();
    let row = view.row(0).unwrap();
    assert_eq!(
        row.query_position_bits().unwrap(),
        [0x8000_0000, 0x7fc0_00a5, 0x3f80_0000, 0x3f80_0000]
    );
    assert_eq!(
        row.read_common(&descriptor(0), None, 0).unwrap(),
        Admission::Disabled
    );
    assert!(row.read_common(&descriptor(1), None, 28).is_err());
    data.truncate(0x217);
    let view = read(&data, &maps, 0, Budget::default()).unwrap().unwrap();
    assert!(view.row(0).unwrap().query_position_bits().is_err());
}

#[test]
fn bit25_coupling_does_not_shift_animator_words() {
    let (data, maps) = fixture((1 << 25) | (1 << 26), &[44]);
    let result = common(&data, &maps, 1).unwrap();
    assert_eq!((result.word_74_raw, result.word_78_raw), (0, 0));
    assert_eq!(result.animator_index_raw, 44);
    assert_eq!(result.property_words_read, 1);
    let (data, maps) = fixture((1 << 24) | (1 << 26), &[0xa7, u32::MAX]);
    let result = common(&data, &maps, 2).unwrap();
    assert_eq!((result.word_74_raw, result.word_78_raw), (0, 0xa7));
    assert_eq!(result.animator_index_raw, -1);
}

#[test]
fn animator_presence_distinguishes_absent_zero_and_negative() {
    for (mask, properties, present, index) in [
        (0, vec![], false, 0),
        (1 << 26, vec![0], true, 0),
        (1 << 26, vec![0xffff_fffe], true, -2),
    ] {
        let (data, maps) = fixture(mask, &properties);
        let result = common(&data, &maps, 1).unwrap();
        assert_eq!(result.animator_present, present);
        assert_eq!(result.animator_index_raw, index);
    }
}

#[test]
fn shape_selector_is_retained_as_an_unprocessed_dependency_instead_of_ignored_or_guessed() {
    let (mut data, maps) = fixture(1 << 27, &[3]);
    let input = common(&data, &maps, 1).unwrap();
    assert_eq!(input.shape_index_raw, Some(3));
    assert_eq!(input.property_words_read, 1);
    assert!(common(&data, &maps, 0).is_err());
    data.truncate(0x230);
    assert!(common(&data, &maps, 1).is_err());
}

#[test]
fn property_budgets_and_reached_pose_truncation_are_errors() {
    let (data, maps) = fixture(3, &[10, 11]);
    assert!(
        common(&data, &maps, 1)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert_eq!(common(&data, &maps, 2).unwrap().property_words_read, 2);
    let (mut data, maps) = fixture(0, &[]);
    data.truncate(0x223);
    assert!(common(&data, &maps, 0).is_err());
}

#[test]
fn row_metadata_is_indexed_lazy_and_classification_must_match_source() {
    let (mut data, maps) = fixture(0, &[]);
    put(&mut data, 0x100, 3);
    put(&mut data, 0x150, 0x7fff_ffff);
    let view = read(&data, &maps, 0, Budget::default()).unwrap().unwrap();
    assert_eq!(view.row_count(), 3);
    assert_eq!(view.map_locator().map_hash, 0x0500_00a5);
    let row = view.row(0).unwrap();
    assert!(matches!(
        row.read_common(&descriptor(-1), None, 0).unwrap(),
        Admission::Admitted(_)
    ));
    let mut wrong = descriptor(0);
    wrong.native_type_index = 0;
    assert!(row.read_common(&wrong, None, 0).is_err());
    assert_eq!(view.row(1).unwrap().locator().trigger_ordinal, 1);
    assert!(view.row(2).is_err());
    assert!(view.row(3).is_err());
}

#[test]
fn empty_signed_counts_skip_unused_directories_and_preserve_missing_map() {
    let (mut data, maps) = fixture(0, &[]);
    put(&mut data, 0x104, 0);
    put(&mut data, 0x10c, 0);
    for count in [0, u32::MAX, 0x8000_0000] {
        put(&mut data, 0x100, count);
        let view = read(
            &data,
            &maps,
            0,
            Budget {
                maps: 1,
                rows: 0,
                types: 0,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(view.row_count(), 0);
        assert_eq!(view.row_count_raw(), count as i32);
        assert!(view.row(0).is_err());
    }
    assert!(
        read(&data, &maps, 0x8500_0001, Budget::default())
            .unwrap()
            .is_none()
    );
    assert!(
        read(
            &data,
            &maps,
            0,
            Budget {
                maps: 0,
                rows: 0,
                types: 0
            }
        )
        .is_err()
    );
}
