use super::*;

fn put(data: &mut [u8], at: usize, word: u32) {
    data[at..at + 4].copy_from_slice(&word.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    let delta = i32::try_from(target as i64 - field as i64).unwrap();
    put(data, field, delta as u32);
}
fn fixture() -> (Vec<u8>, Table) {
    let mut data = vec![0; 0x300];
    let maps = Table {
        name: "maps",
        count: 1,
        hash_size_raw: 1,
        offset: 0x20,
    };
    put(&mut data, 0x20, 0x05000123);
    put(&mut data, 0x28, 0x80);
    put(&mut data, 0x80, 0x500);
    relative(&mut data, 0xd8, 0x100);
    put(&mut data, 0x100, 3);
    relative(&mut data, 0x104, 0x140);
    relative(&mut data, 0x10c, 0x180);
    for (index, native_type) in [17, 62, 62].into_iter().enumerate() {
        put(&mut data, 0x180 + index * 16, native_type);
        let record = 0x200 + index * 0x40;
        relative(&mut data, 0x140 + index * 8, record);
        data[record..record + 2].copy_from_slice(&(index as u16).to_be_bytes());
        put(&mut data, record + 8, 0x80000003);
        put(&mut data, record + 0x30, 0x01000001 + index as u32);
        put(&mut data, record + 0x34, 0x05000001 + index as u32);
    }
    (data, maps)
}
fn read_fixture(data: &[u8], maps: &Table) -> Result<Option<LinkedMap>> {
    read(data, maps, 0x85000000, Budget::default())
}

#[test]
fn coded_reference_and_literal_hash_lookup_retain_actual_map_identity() {
    let (data, mut maps) = fixture();
    let linked = read_fixture(&data, &maps).unwrap().unwrap();
    assert_eq!((linked.file, linked.map), (0x01000002, 0x05000002));
    assert_eq!(
        (
            linked.locator.map_reference,
            linked.locator.map_hash,
            linked.locator.map_directory_index,
            linked.locator.map_address
        ),
        (0x85000000, 0x05000123, 0, 0x80)
    );
    let literal = read(&data, &maps, 0x05000123, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(literal.file, linked.file);
    maps.hash_size_raw = 0;
    assert!(
        read(&data, &maps, 0x05000123, Budget::default())
            .unwrap()
            .is_none()
    );
    assert!(read_fixture(&data, &maps).unwrap().is_some());
    assert!(read(&data, &maps, 0, Budget::default()).unwrap().is_some());
    assert!(
        read(&data, &maps, 0x85000001, Budget::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn first_semantic_match_preserves_provenance_and_skips_later_records() {
    let (mut data, maps) = fixture();
    // This malformed later pointer must remain unread after the earlier match.
    put(&mut data, 0x150, u32::MAX / 2);
    let result = read_fixture(&data, &maps).unwrap().unwrap();
    assert_eq!(
        (
            result.locator.trigger_ordinal,
            result.locator.record_address,
            result.locator.schema_index,
            result.locator.schema_address,
            result.locator.native_type_index,
            result.locator.flags_raw
        ),
        (1, 0x240, 1, 0x190, 62, 0x80000003)
    );
    assert_eq!(result.locator.trigger_block, 0x100);
}

#[test]
fn absent_property_flags_return_sentinels_without_reading_properties_or_later_matches() {
    let (mut data, maps) = fixture();
    relative(&mut data, 0x148, 0x2f4);
    data[0x2f4..0x2f6].copy_from_slice(&1u16.to_be_bytes());
    put(&mut data, 0x2fc, 0); // flags; both property addresses exceed the file.
    let result = read_fixture(&data, &maps).unwrap().unwrap();
    assert_eq!((result.file, result.map), (u32::MAX, u32::MAX));
    assert_eq!(result.locator.trigger_ordinal, 1);
    // A file-only property ending exactly at EOF does not read absent map+34.
    relative(&mut data, 0x148, 0x2cc);
    data[0x2cc..0x2ce].copy_from_slice(&1u16.to_be_bytes());
    put(&mut data, 0x2d4, 1);
    put(&mut data, 0x2fc, 0x01800001);
    let result = read_fixture(&data, &maps).unwrap().unwrap();
    assert_eq!((result.file, result.map), (0x01800001, u32::MAX));
}

#[test]
fn unmatched_record_only_reads_schema_not_flags_or_properties() {
    let (mut data, maps) = fixture();
    relative(&mut data, 0x140, 0x2fe);
    data[0x2fe..].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(
        read_fixture(&data, &maps)
            .unwrap()
            .unwrap()
            .locator
            .trigger_ordinal,
        1
    );
    put(&mut data, 0x190, 17);
    put(&mut data, 0x1a0, 17);
    assert!(read_fixture(&data, &maps).unwrap().is_none());
}

#[test]
fn zero_or_negative_signed_row_count_does_not_read_missing_directories() {
    let (mut data, maps) = fixture();
    put(&mut data, 0x104, 0);
    put(&mut data, 0x10c, 0);
    for count in [0, u32::MAX, 0x80000000] {
        put(&mut data, 0x100, count);
        assert!(
            read(
                &data,
                &maps,
                0x85000000,
                Budget {
                    maps: 1,
                    rows: 0,
                    types: 0
                }
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn reached_schema_classification_is_ordered_lazy_and_can_fail_explicitly() {
    let (data, maps) = fixture();
    let mut observed = Vec::new();
    let linked = read_with_classifier(&data, &maps, 0x85000000, Budget::default(), |index| {
        observed.push(index);
        Ok(index == 62)
    })
    .unwrap()
    .unwrap();
    assert_eq!(observed, vec![17, 62]);
    assert_eq!(linked.locator.trigger_ordinal, 1);
    assert!(
        read_with_classifier(&data, &maps, 0x85000000, Budget::default(), |_| {
            anyhow::bail!("unavailable native type classification")
        })
        .is_err()
    );
}

#[test]
fn work_limits_truncated_reached_records_and_null_pointers_fail_explicitly() {
    let (data, maps) = fixture();
    for budget in [
        Budget {
            maps: 0,
            rows: 3,
            types: 3,
        },
        Budget {
            maps: 1,
            rows: 2,
            types: 3,
        },
        Budget {
            maps: 1,
            rows: 3,
            types: 1,
        },
    ] {
        assert!(read(&data, &maps, 0x85000000, budget).is_err());
    }
    for at in [0xd8, 0x104, 0x10c, 0x148] {
        let mut bad = data.clone();
        put(&mut bad, at, 0);
        assert!(read_fixture(&bad, &maps).is_err(), "null {at:x}");
    }
    let mut bad = data.clone();
    put(&mut bad, 0x274, u32::MAX);
    // Resource/map property values are raw words, including sentinels.
    assert_eq!(read_fixture(&bad, &maps).unwrap().unwrap().map, u32::MAX);
    assert!(read_fixture(&data[..0x274], &maps).is_err());
    let mut bad = data.clone();
    put(&mut bad, 0x80, 0x300);
    assert!(read_fixture(&bad, &maps).is_err());
    let mut wrong = maps;
    wrong.name = "entities";
    assert!(read_fixture(&data, &wrong).is_err());
}
