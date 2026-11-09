use super::*;

const A: u32 = 0x2d70_0001;
const B: u32 = 0x2d70_0010;
const C: u32 = 0x2d70_0100;

fn le(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn be(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture(rows: &[[u32; 4]]) -> Vec<u8> {
    // Trailing file padding is legitimate and is not a second row directory.
    let mut data = vec![0; HEADER_BYTES + rows.len() * ROW_BYTES + 8];
    data[..4].copy_from_slice(b"MUSX");
    le(&mut data, 4, 0x2d20_0123);
    le(&mut data, 8, 10);
    let size = data.len() as u32;
    le(&mut data, 0xc, size);
    data[0x10..0x14].copy_from_slice(b"WII_");
    data[0x14..0x20].copy_from_slice(&[
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
    ]);
    be(&mut data, 0x20, A);
    be(&mut data, 0x24, C);
    be(&mut data, 0x28, rows.len() as u32);
    for (index, row) in rows.iter().enumerate() {
        for (word, value) in row.iter().enumerate() {
            be(
                &mut data,
                HEADER_BYTES + index * ROW_BYTES + word * 4,
                *value,
            );
        }
    }
    data
}

#[test]
fn mixed_header_and_big_endian_rows_preserve_raw_ranges_and_flags() {
    let data = fixture(&[[A, 3f32.to_bits(), 9f32.to_bits(), 0xa5ff_0123]]);
    let table = read(&data, 1).unwrap();
    assert_eq!(table.header().resource_hash, 0x2d20_0123);
    assert_eq!(table.header().version, 10);
    assert_eq!(table.header().file_bytes as usize, data.len());
    assert_eq!(table.header().reserved_raw, data[0x14..0x20]);
    let row = table.lookup(A).unwrap().unwrap();
    assert_eq!(
        (row.index, row.address, row.sound_hash),
        (0, HEADER_BYTES, A)
    );
    assert_eq!((row.minimum_range(), row.maximum_range()), (3., 9.));
    assert_eq!(row.flags_raw, 0xa5ff_0123);
}

#[test]
fn duplicate_hashes_follow_biased_midpoint_even_for_small_tables() {
    for (count, expected) in [(2, 1), (3, 1), (4, 2)] {
        let rows: Vec<_> = (0..count).map(|index| [B, 0, 0, index as u32]).collect();
        let data = fixture(&rows);
        let table = read(&data, count).unwrap();
        let row = table.lookup(B).unwrap().unwrap();
        assert_eq!(row.index, expected);
        assert_eq!(row.flags_raw, expected as u32);
    }
}

#[test]
fn search_keeps_storage_order_and_inclusive_declared_hash_gate() {
    let mut data = fixture(&[[A, 0, 0, 1], [B, 0, 0, 2], [C, 0, 0, 3]]);
    let table = read(&data, 3).unwrap();
    assert_eq!(table.lookup(A).unwrap().unwrap().index, 0);
    assert_eq!(table.lookup(C).unwrap().unwrap().index, 2);
    assert!(table.lookup(A + 1).unwrap().is_none());
    be(&mut data, 0x20, B);
    let narrowed = read(&data, 3).unwrap();
    assert!(narrowed.lookup(A).unwrap().is_none());
    assert_eq!(narrowed.entry(0).unwrap().sound_hash, A);
    // Native trusts the header gate; an inverted gate is not repaired.
    be(&mut data, 0x24, A);
    assert!(read(&data, 3).unwrap().lookup(B).unwrap().is_none());
    let unsorted = fixture(&[[C, 0, 0, 1], [A, 0, 0, 2], [B, 0, 0, 3]]);
    assert!(read(&unsorted, 3).unwrap().lookup(C).unwrap().is_none());
}

#[test]
fn sound_class_gate_is_distinct_from_range_membership_and_empty_tables() {
    let mut data = fixture(&[[0x2d50_0001, 0, 0, 7]]);
    be(&mut data, 0x20, 0);
    be(&mut data, 0x24, u32::MAX);
    let table = read(&data, 1).unwrap();
    assert!(table.lookup(0x2d50_0001).unwrap().is_none());
    assert_eq!(table.entry(0).unwrap().flags_raw, 7);
    let empty = fixture(&[]);
    let table = read(&empty, 0).unwrap();
    assert!(table.lookup(A).unwrap().is_none());
    assert!(table.entry(0).is_err());
}

#[test]
fn exceptional_ranges_are_lazy_raw_metadata_and_owner_release_is_bit_twenty_four() {
    let data = fixture(&[
        [A, 0x7fc0_0012, f32::INFINITY.to_bits(), 1 << 24],
        [B, 0xff80_0000, 0x8000_0000, 0x8000_01fc],
    ]);
    let table = read(&data, 2).unwrap();
    let first = table.lookup(A).unwrap().unwrap();
    assert_eq!(first.minimum_range_bits, 0x7fc0_0012);
    assert!(first.minimum_range().is_nan());
    assert_eq!(first.maximum_range(), f32::INFINITY);
    assert!(first.stop_on_owner_release());
    let second = table.lookup(B).unwrap().unwrap();
    assert_eq!(second.minimum_range(), f32::NEG_INFINITY);
    assert_eq!(second.maximum_range_bits, (-0f32).to_bits());
    assert!(!second.stop_on_owner_release());
    // Exceptional unselected fields never prevent a missing lookup.
    assert!(table.lookup(A + 2).unwrap().is_none());
}

#[test]
fn unsupported_wrapper_byte_orders_versions_platforms_and_sizes_are_errors() {
    let data = fixture(&[[A, 0, 0, 0]]);
    for (at, bytes) in [
        (0, *b"XSUM"),
        (8, 10u32.to_be_bytes()),
        (8, 11u32.to_le_bytes()),
        (0x10, *b"PC__"),
        (0xc, 1u32.to_le_bytes()),
        // Whole little-endian table layout is not guessed from its row count.
        (0x28, 1u32.to_le_bytes()),
    ] {
        let mut malformed = data.clone();
        malformed[at..at + 4].copy_from_slice(&bytes);
        assert!(read(&malformed, 1).is_err(), "field {at:#x}");
    }
}

#[test]
fn row_budget_short_headers_and_declared_payload_bounds_are_checked() {
    let data = fixture(&[[A, 0, 0, 0], [B, 0, 0, 0]]);
    assert!(read(&data, 1).is_err());
    assert!(read(&data[..HEADER_BYTES - 1], 2).is_err());
    let mut truncated = data[..HEADER_BYTES + ROW_BYTES + 1].to_vec();
    let len = truncated.len() as u32;
    le(&mut truncated, 0xc, len);
    assert!(read(&truncated, 2).is_err());
    let table = read(&data, 2).unwrap();
    assert!(table.entry(2).is_err());
    assert!(table.entry(usize::MAX).is_err());
    let mut huge = data;
    be(&mut huge, 0x28, u32::MAX);
    assert!(read(&huge, 2).is_err());
}
