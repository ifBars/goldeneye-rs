use super::*;
fn table(count: u16, size: i16, offset: usize) -> Table {
    Table {
        name: "authored",
        count,
        hash_size_raw: size,
        offset,
    }
}
fn bytes(keys: &[u32], stride: usize) -> Vec<u8> {
    let mut data = vec![0; keys.len() * stride];
    for (i, key) in keys.iter().enumerate() {
        data[i * stride..i * stride + 4].copy_from_slice(&key.to_be_bytes());
    }
    data
}
#[test]
fn coded_references_use_low_twenty_four_bits_without_matching_descriptor_hash() {
    let t = table(3, 0, 0);
    for reference in [1, 0x82000001, 0xff000001] {
        assert_eq!(resolve(&[], &t, 20, reference).unwrap(), Some(1));
    }
    assert_eq!(resolve(&[], &t, 20, 3).unwrap(), None);
    assert_eq!(resolve(&[], &t, 20, 0x02000001).unwrap(), None);
    assert_eq!(resolve(&[], &table(0x8000, 0, 0), 20, 0).unwrap(), None);
}
#[test]
fn inline_and_prefix_hash_tables_keep_observed_binary_midpoint_and_small_linear_ties() {
    let keys = [0x02000001, 0x02000002, 0x02000002, 0x02000002, 0x02000005];
    let data = bytes(&keys, 20);
    assert_eq!(
        resolve(&data, &table(5, 5, 0), 20, keys[1]).unwrap(),
        Some(2)
    );
    assert_eq!(
        resolve(&data, &table(5, 3, 0), 20, keys[1]).unwrap(),
        Some(1)
    );
    for (i, key) in [(0, keys[0]), (4, keys[4])] {
        assert_eq!(resolve(&data, &table(5, 5, 0), 20, key).unwrap(), Some(i));
    }
    assert_eq!(
        resolve(&data, &table(5, 5, 0), 20, 0x02000006).unwrap(),
        None
    );
    let mut prefix = bytes(&keys, 4);
    prefix.resize(5 * 4 + 5 * 20, 0);
    assert_eq!(
        resolve(&prefix, &table(5, -5, 20), 20, keys[1]).unwrap(),
        Some(2)
    );
    assert_eq!(
        resolve(&prefix, &table(5, -3, 12), 20, keys[1]).unwrap(),
        Some(1)
    );
}
#[test]
fn malformed_hash_ranges_fail_and_unsorted_tables_are_not_repaired_by_sorting() {
    assert!(resolve(&[], &table(5, -5, 0), 20, 0x02000001).is_err());
    assert!(resolve(&[], &table(5, 5, 0), 20, 0x02000001).is_err());
    assert!(resolve(&[], &table(1, 2, 0), 20, 0x02000001).is_err());
    let keys = [0x02000001, 0x02000005, 0x02000003, 0x02000004];
    assert_eq!(
        resolve(&bytes(&keys, 20), &table(4, 4, 0), 20, keys[1]).unwrap(),
        None
    );
}
