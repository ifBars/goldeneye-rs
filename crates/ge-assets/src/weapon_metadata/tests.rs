use super::*;
fn word(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> Edb {
    let mut b = vec![0; 0x500];
    for (at, value) in [
        (0, 0x47454f4d),
        (4, RESOURCE),
        (8, 263),
        (0x14, 0x500),
        (0x18, 0x500),
        (0x98, 0x00010001),
        (0x9c, 0x100 - 0x9c),
        (0x100, SHEET),
        (0x108, 0x180),
        (0x184, 0x200 - 0x184),
        (0x194, 0x300 - 0x194),
        (0x188, u32::MAX),
        (0x200, 3),
        (0x300, 3),
    ] {
        word(&mut b, at, value);
    }
    for (i, primary) in [0x4b0000a9, 0x4b0000a5, 0x4b0000a5].into_iter().enumerate() {
        let p = 0x204 + i * 0x44;
        let q = 0x304 + i * 0x24;
        for (at, v) in [
            (p, primary),
            (p + 4, 0x4b200000),
            (p + 0x28, 0x4b100001),
            (q, primary),
            (q + 4, 0x020000a5),
            (q + 0x18, 0),
            (q + 0x1c, 0),
        ] {
            word(&mut b, at, v);
        }
    }
    Edb::parse(b).unwrap()
}
#[test]
fn inventory_key_lookup_uses_first_registered_primary_and_unsigned_order() {
    let owner = fixture();
    let row = find_inventory(&owner, 0x020000a5, 3).unwrap().unwrap();
    assert_eq!(row.source_index, 1);
    assert_eq!(row.primary_key, 0x4b0000a5);
    assert_eq!(row.ammunition_raw, [0, 0]);
    assert_eq!(row.category_key, 0x4b100001);
    assert!(find_inventory(&owner, 0x4b0000a5, 3).unwrap().is_none());
}
#[test]
fn unconsumed_groups_stay_lazy_and_row_budget_and_identity_are_enforced() {
    let mut owner = fixture();
    assert!(find_inventory(&owner, 0x020000a5, 3).unwrap().is_some());
    assert!(find_inventory(&owner, 0x020000a5, 2).is_err());
    owner.header.hash ^= 1;
    assert!(find_inventory(&owner, 0x020000a5, 3).is_err());
}
