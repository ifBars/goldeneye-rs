use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i32 - field as i32) as u32);
}

fn fixture() -> (Vec<u8>, Table) {
    let mut bytes = vec![0; 0x400];
    let maps = Table {
        name: "maps",
        count: 1,
        hash_size_raw: 1,
        offset: 0x20,
    };
    word(&mut bytes, 0x20, 0x0500_00a5);
    word(&mut bytes, 0x28, 0x80);
    word(&mut bytes, 0x80, 0x500);
    relative(&mut bytes, 0xd8, 0x100);
    word(&mut bytes, 0x100, 3);
    relative(&mut bytes, 0x104, 0x140);
    relative(&mut bytes, 0x10c, 0x180);
    for (index, native_type) in [17, 28, 28].into_iter().enumerate() {
        word(&mut bytes, 0x180 + index * 16, native_type);
        let record = 0x200 + index * 0x40;
        relative(&mut bytes, 0x140 + index * 8, record);
        bytes[record..record + 2].copy_from_slice(&(index as u16).to_be_bytes());
        word(&mut bytes, record + 8, 0x8000_0001);
        word(&mut bytes, record + 0x30, 70 + index as u32);
        for (axis, value) in [0x3fa0_0000, 0x8000_0000, 0x7fc0_0001]
            .into_iter()
            .enumerate()
        {
            word(&mut bytes, record + 0x0c + axis * 4, value);
        }
        for (axis, value) in [0x3e80_0000, 0xbf00_0000, 0x8000_0000]
            .into_iter()
            .enumerate()
        {
            word(&mut bytes, record + 0x18 + axis * 4, value);
        }
    }
    (bytes, maps)
}

fn budget() -> Budget {
    Budget {
        maps: 1,
        rows: 3,
        types: 3,
        points: 2,
    }
}

#[test]
fn source_classification_preserves_raw_pose_and_positive_zero_fourth_lanes() {
    let (bytes, maps) = fixture();
    let points = read(&bytes, &maps, 0x8500_0000, budget()).unwrap().unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(
        points.iter().map(|point| point.id).collect::<Vec<_>>(),
        [71, 72]
    );
    assert_eq!(
        points[0].position_bits,
        [0x3fa0_0000, 0x8000_0000, 0x7fc0_0001, 0]
    );
    assert_eq!(
        points[0].rotation_bits,
        [0x3e80_0000, 0xbf00_0000, 0x8000_0000, 0]
    );
    assert_eq!(points[0].property_mask_raw, 0x8000_0001);
    assert_eq!(points[0].locator.map_hash, 0x0500_00a5);
    assert_eq!(points[0].locator.trigger_ordinal, 1);
    assert_eq!(points[0].locator.record_address, 0x240);
    assert_eq!(points[0].locator.native_type_index, 28);
}

#[test]
fn absent_id_defaults_zero_without_reading_the_unavailable_payload_word() {
    let (mut bytes, maps) = fixture();
    bytes.truncate(0x300);
    word(&mut bytes, 0x100, 1);
    word(&mut bytes, 0x180, 28);
    relative(&mut bytes, 0x140, 0x2dc);
    bytes[0x2dc..0x300].fill(0);
    let points = read(&bytes, &maps, 0x8500_0000, budget()).unwrap().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].id, 0);
    word(&mut bytes, 0x2e4, 1);
    assert!(read(&bytes, &maps, 0x8500_0000, budget()).is_err());
}

#[test]
fn unmatched_short_records_leave_type_specific_flags_and_pose_unread() {
    let (mut bytes, maps) = fixture();
    bytes.truncate(0x300);
    word(&mut bytes, 0x100, 1);
    relative(&mut bytes, 0x140, 0x2fe);
    bytes[0x2fe..].copy_from_slice(&0u16.to_be_bytes());
    assert!(
        read(&bytes, &maps, 0x8500_0000, budget())
            .unwrap()
            .unwrap()
            .is_empty()
    );
    word(&mut bytes, 0x180, 28);
    assert!(read(&bytes, &maps, 0x8500_0000, budget()).is_err());
}

#[test]
fn disabled_and_duplicate_ids_remain_records_for_the_separate_selector() {
    let (mut bytes, maps) = fixture();
    word(&mut bytes, 0x244, 1);
    word(&mut bytes, 0x2b0, 71);
    let points = read(&bytes, &maps, 0x8500_0000, budget()).unwrap().unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!((points[0].id, points[1].id), (71, 71));
    assert_eq!(points[0].trigger_flags_raw, 1);
    assert_eq!(points[1].trigger_flags_raw, 0);
}

#[test]
fn budgets_and_missing_maps_are_distinct_from_present_empty_trigger_lists() {
    let (mut bytes, maps) = fixture();
    for limited in [
        Budget {
            maps: 0,
            ..budget()
        },
        Budget {
            rows: 2,
            ..budget()
        },
        Budget {
            types: 1,
            ..budget()
        },
        Budget {
            points: 1,
            ..budget()
        },
    ] {
        assert!(read(&bytes, &maps, 0x8500_0000, limited).is_err());
    }
    assert!(
        read(&bytes, &maps, 0x8500_0001, budget())
            .unwrap()
            .is_none()
    );
    word(&mut bytes, 0x104, 0);
    word(&mut bytes, 0x10c, 0);
    for count in [0, u32::MAX, 0x8000_0000] {
        word(&mut bytes, 0x100, count);
        assert!(
            read(
                &bytes,
                &maps,
                0x8500_0000,
                Budget {
                    maps: 1,
                    rows: 0,
                    types: 0,
                    points: 0
                }
            )
            .unwrap()
            .unwrap()
            .is_empty()
        );
    }
}
