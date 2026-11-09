use super::*;
fn word(b: &mut [u8], a: usize, v: u32) {
    b[a..a + 4].copy_from_slice(&v.to_be_bytes());
}
fn half(b: &mut [u8], a: usize, v: u16) {
    b[a..a + 2].copy_from_slice(&v.to_be_bytes());
}
fn rel(b: &mut [u8], a: usize, to: usize) {
    word(b, a, (to as i32 - a as i32) as u32);
}
fn fixture() -> (Vec<u8>, Vec<Table>) {
    let mut b = vec![0; 0x600];
    let mut tables = (0..7)
        .map(|_| Table {
            name: "unused",
            count: 0,
            hash_size_raw: 0,
            offset: 0,
        })
        .collect::<Vec<_>>();
    tables[0] = Table {
        name: "sections",
        count: 2,
        hash_size_raw: 2,
        offset: 0x20,
    };
    tables[1] = Table {
        name: "references",
        count: 1,
        hash_size_raw: 1,
        offset: 0x50,
    };
    tables[2] = Table {
        name: "entities",
        count: 1,
        hash_size_raw: 1,
        offset: 0x70,
    };
    tables[6] = Table {
        name: "maps",
        count: 1,
        hash_size_raw: 1,
        offset: 0x90,
    };
    word(&mut b, 0x20, 0x0800_0000);
    word(&mut b, 0x30, 0x0800_0001);
    word(&mut b, 0x58, 0x400);
    word(&mut b, 0x78, 0xffff_ffff);
    word(&mut b, 0x90, 0x0500_002a);
    word(&mut b, 0x98, 0x100);
    word(&mut b, 0x100, 0x500);
    word(&mut b, 0x140, 2);
    word(&mut b, 0x148, 3);
    word(&mut b, 0x184, 1);
    let zone = 0x188;
    rel(&mut b, zone + 4, 0x300);
    word(&mut b, zone + 0x34, 0);
    rel(&mut b, zone + 0x28, 0x350);
    word(&mut b, 0x350, 3);
    rel(&mut b, 0x354, 0x380);
    half(&mut b, 0x380, 2);
    half(&mut b, 0x382, 0);
    half(&mut b, 0x384, 2);
    word(&mut b, 0x300, 0x7fc0_3456);
    word(&mut b, 0x304, 0x8000_0000);
    word(&mut b, 0x310, 0x8000_0000);
    word(&mut b, 0x32c, 0x1234);
    word(&mut b, 0x400, 0x608);
    b[0x454] = 2;
    b[0x455] = 0;
    (b, tables)
}
#[test]
fn raw_constructor_fields_and_ordered_duplicate_placement_links_are_retained() {
    let (b, t) = fixture();
    let v = read(&b, &t, 0x8500_0000, Budget::default())
        .unwrap()
        .unwrap();
    let i = v.initializer().unwrap();
    assert_eq!(i.component_slots, 2);
    assert_eq!(i.section_count, 2);
    assert_eq!(
        i.zones[0].initial_values_bits[..2],
        [0x7fc0_3456, 0x8000_0000]
    );
    assert_eq!(i.zones[0].word2c_raw, 0x1234);
    assert_eq!(v.zone_count(), 1);
    assert_eq!(v.zone_placement_indices(0).unwrap(), vec![2, 0, 2]);
    assert_eq!(v.zone_root(0).unwrap().binding_count, 2);
    assert_eq!(
        v.section_keys().unwrap().keys,
        vec![0x0800_0000, 0x0800_0001]
    );
}
#[test]
fn zone_metadata_does_not_touch_null_identifier_bad_root_or_bad_link_body() {
    let (mut b, t) = fixture();
    word(&mut b, 0x18c, 0);
    word(&mut b, 0x1b0, 0);
    word(&mut b, 0x58, u32::MAX);
    let v = read(&b, &t, 0x8500_0000, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(v.zone_publication(0).unwrap().section_index, 0);
    assert!(v.initializer().is_err());
    assert!(v.zone_root(0).is_err());
    assert!(v.zone_placement_indices(0).is_err());
}
#[test]
fn unrequested_root_section_rejected_before_poisoned_body_address() {
    let (mut b, t) = fixture();
    half(&mut b, 0x54, 1);
    word(&mut b, 0x58, u32::MAX);
    let v = read(&b, &t, 0x8500_0000, Budget::default())
        .unwrap()
        .unwrap();
    assert!(
        v.zone_root(0)
            .unwrap_err()
            .to_string()
            .contains("unrequested")
    );
    // Entity metadata access likewise does not inspect poisoned +8.
    half(&mut b, 0x74, 0x8003);
    assert_eq!(entity_section(&b, &t[2], 0).unwrap(), 3);
}
#[test]
fn zero_links_and_nonpositive_section_keys_are_lazy() {
    let (mut b, mut t) = fixture();
    word(&mut b, 0x350, 0);
    word(&mut b, 0x354, u32::MAX);
    t[0].count = 0xffff;
    t[0].offset = usize::MAX;
    let v = read(&b, &t, 0x8500_0000, Budget::default())
        .unwrap()
        .unwrap();
    assert!(v.zone_placement_indices(0).unwrap().is_empty());
    assert!(v.section_keys().unwrap().keys.is_empty());
    assert!(v.initializer().is_err());
}
#[test]
fn budgets_missing_references_and_out_of_range_links_fail_explicitly() {
    let (mut b, t) = fixture();
    assert!(
        read(
            &b,
            &t,
            0x8500_0000,
            Budget {
                zones: 0,
                ..Budget::default()
            }
        )
        .is_err()
    );
    assert!(
        read(&b, &t, 0x8500_0001, Budget::default())
            .unwrap()
            .is_none()
    );
    half(&mut b, 0x380, 3);
    let v = read(
        &b,
        &t,
        0x8500_0000,
        Budget {
            placement_links: 2,
            ..Budget::default()
        },
    )
    .unwrap()
    .unwrap();
    assert!(v.zone_placement_indices(0).is_err());
    let v = read(&b, &t, 0x8500_0000, Budget::default())
        .unwrap()
        .unwrap();
    assert!(v.zone_placement_indices(0).is_err());
}
