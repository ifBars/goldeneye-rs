use super::*;
fn put(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}
fn fixture(normal_count: u32) -> Edb {
    let mut b = vec![0; 0x900];
    for (at, v) in [
        (0, 0x47454f4d),
        (4, RESOURCE),
        (8, 263),
        (0x14, 0x900),
        (0x18, 0x900),
        (0x98, 0x00010001),
        (0x9c, 0x100 - 0x9c),
        (0x100, SHEET),
        (0x108, 0x180),
        (0x184, 0x200 - 0x184),
        (0x188, 0x400 - 0x188),
        (0x194, 0x700 - 0x194),
        (0x18c, u32::MAX),
        (0x200, 3),
        (0x400, normal_count),
        (0x700, 3),
    ] {
        put(&mut b, at, v);
    }
    for (i, key) in [0x4b000030, 0x4b000010, 0x4b000010].into_iter().enumerate() {
        let p = 0x204 + i * 0x44;
        let n = 0x404 + i * 0x84;
        let q = 0x704 + i * 0x24;
        put(&mut b, p, key);
        put(&mut b, p + 4, 0x4b200000);
        put(&mut b, p + 0x28, 0x4b100001);
        put(&mut b, q, key);
        put(&mut b, q + 4, 0x02000830);
        put(&mut b, n, key);
        for k in 0..10 {
            put(&mut b, n + 0x14 + k * 4, (i as u32) * 100 + k as u32);
        }
        for (off, v) in [(0x3c, 11), (0x48, 12), (0x40, 13), (0x44, 14)] {
            put(&mut b, n + off, v + i as u32 * 100);
        }
    }
    Edb::parse(b).unwrap()
}
#[test]
fn selected_source_row_firstwins_and_runtime_sorted_index_are_separate() {
    let e = fixture(3);
    let m = find_default_inventory(&e, 0x02000830, 3).unwrap().unwrap();
    assert_eq!(m.inventory.source_index, 1);
    assert_eq!(m.native_sorted_index, 0);
    assert_eq!(m.inventory.primary_key, 0x4b000010);
    assert_eq!(m.byte_8_raw, 0);
    assert_eq!(
        m.normal_values_bits,
        [100, 101, 102, 103, 104, 105, 106, 107, 108, 109]
    );
    assert_eq!(m.modifier_bits, [111, 112, 113, 114]);
}
#[test]
fn normal_group_reached_only_after_inventory_match_and_bad_counts_fail() {
    let e = fixture(1);
    assert!(find_default_inventory(&e, 0x020000a5, 3).unwrap().is_none());
    assert!(find_default_inventory(&e, 0x02000830, 3).is_err());
    assert!(find_default_inventory(&fixture(3), 0x02000830, 2).is_err());
}
