use super::*;
fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> (Vec<u8>, Vec<Table>) {
    let mut data = vec![0; 0x500];
    let mut tables = (0..16)
        .map(|_| Table {
            name: "authored",
            count: 0,
            hash_size_raw: 0,
            offset: 0,
        })
        .collect::<Vec<_>>();
    tables[6].count = 1;
    tables[6].offset = 0x40;
    word(&mut data, 0x40, 0x85000000);
    word(&mut data, 0x48, 0x100);
    word(&mut data, 0x100, 0x500);
    word(&mut data, 0x104, 0x400 - 0x104);
    word(&mut data, 0x184, 2);
    for (zone, at) in [0x188, 0x214].into_iter().enumerate() {
        word(&mut data, at, zone as u32 + 7);
        for (i, value) in [-1f32, -2., -3., 1., 2., 3.].into_iter().enumerate() {
            word(&mut data, at + 0x68 + i * 4, value.to_bits());
        }
    }
    word(&mut data, 0x400, 1f32.to_bits());
    word(&mut data, 0x410, (2 << 16) | 0xffff);
    // Index one is unreachable and must not be interpreted.
    word(&mut data, 0x420, f32::NAN.to_bits());
    word(&mut data, 0x444, 1f32.to_bits());
    word(&mut data, 0x450, 0x0000ffff);
    (data, tables)
}
#[test]
fn bsp_reader_keeps_sparse_indices_signed_children_and_zone_metadata() {
    let (data, tables) = fixture();
    let map = read(&data, &tables, 0x85000000).unwrap();
    assert!(map.bsp_present);
    assert_eq!(map.nodes.keys().copied().collect::<Vec<_>>(), [0, 2]);
    assert_eq!(map.nodes[&0].children, [2, -1]);
    assert_eq!(map.nodes[&2].children, [0, -1]);
    assert_eq!(map.zones[1].entity_reference, 8);
    assert_eq!(map.zones[0].bounds, [[-1., -2., -3.], [1., 2., 3.]]);
    // The twelve trailing bytes of the last node are not used by this consumer.
    assert!(read(&data[..0x454], &tables, 0x85000000).is_ok());
    for end in 0..0x454 {
        assert!(
            read(&data[..end], &tables, 0x85000000).is_err(),
            "prefix {end:x}"
        );
    }
}
#[test]
fn bsp_absence_and_cycles_remain_distinct_from_empty_geometry() {
    let (mut data, tables) = fixture();
    word(&mut data, 0x450, (2 << 16) | 0xffff);
    let map = read(&data, &tables, 0x85000000).unwrap();
    assert_eq!(map.nodes[&2].children[0], 2);
    word(&mut data, 0x104, 0);
    let map = read(&data, &tables, 0x85000000).unwrap();
    assert!(!map.bsp_present && map.nodes.is_empty());
    assert_eq!(map.zones.len(), 2);
}
#[test]
fn malformed_used_bsp_inputs_and_ambiguous_maps_are_errors() {
    let (data, tables) = fixture();
    for (at, value) in [
        (0x100, 0x601),
        (0x104, 0x7fffffff),
        (0x184, 65537),
        (0x400, f32::INFINITY.to_bits()),
        (0x410, 0x7fff0000),
        (0x450, 0x00008000),
        (0x188 + 0x68, f32::NAN.to_bits()),
    ] {
        let mut bad = data.clone();
        word(&mut bad, at, value);
        assert!(read(&bad, &tables, 0x85000000).is_err(), "mutation {at:x}");
    }
    let mut duplicate = data.clone();
    duplicate.copy_within(0x40..0x50, 0x50);
    let mut tables = tables;
    tables[6].count = 2;
    assert!(read(&duplicate, &tables, 0x85000000).is_err());
    assert!(read(&data, &[], 0x85000000).is_err());
}
