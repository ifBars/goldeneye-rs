use super::*;
fn word(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_be_bytes());
}
fn relative(b: &mut [u8], p: usize, to: usize) {
    word(b, p, (to as i32 - p as i32) as u32);
}
fn fixture() -> (Vec<u8>, Vec<Table>) {
    let mut b = vec![0; 0x700];
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
    for (p, v) in [
        (0x40, 0x85000000),
        (0x48, 0x100),
        (0x100, 0x500),
        (0x138, 3),
        (0x184, 2),
        (0x1a8, 4),
        (0x608, 4),
    ] {
        word(&mut b, p, v);
    }
    relative(&mut b, 0x13c, 0x500);
    relative(&mut b, 0x1ac, 0x400);
    // Group 0: two rows, one target. The second target/length are opaque here.
    for (p, bytes) in [
        (0x400, [0xaa, 0x55, 0, 1, 0, 0, 1, 0x99]),
        (0x408, [0, 0, 0xff, 0x9d, 0, 1, 255, 7]),
        (0x410, [0, 0, 0xff, 0xff, 0x7f, 0xff, 0, 0]),
        (0x418, [0, 0, 0, 1, 0, 1, 0, 0]),
    ] {
        b[p..p + 8].copy_from_slice(&bytes);
    }
    for (i, point) in [[0f32, 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]]
        .into_iter()
        .enumerate()
    {
        for (axis, v) in point.into_iter().enumerate() {
            word(&mut b, 0x514 + i * 12 + axis * 4, v.to_bits());
            word(&mut b, 0x610 + i * 32 + axis * 4, v.to_bits());
        }
        // Linked vertex W/attributes are neither needed nor interpreted as floats.
        word(&mut b, 0x61c + i * 32, f32::NAN.to_bits());
    }
    relative(&mut b, 0x554, 0x600);
    b[0x588..0x5cc].fill(0xff); // Unreferenced descriptor two is deliberately corrupt.
    (b, tables)
}
#[test]
fn grouped_rows_preserve_raw_fields_and_decode_only_addressed_shape_forms() {
    let (b, tables) = fixture();
    let map = read(&b, &tables, 0x85000000).unwrap();
    assert_eq!(map.zone_groups[0].len(), 3);
    assert!(map.zone_groups[1].is_empty());
    let group = &map.zone_groups[0][0];
    assert_eq!(group.target_zone, 1);
    assert_eq!(group.rows.len(), 2);
    assert_eq!(group.rows[0].prefix_raw, 0xaa55);
    assert_eq!(group.rows[0].tail_raw, 0x99);
    assert_eq!(group.rows[1].target_zone_raw, -99);
    assert_eq!(group.rows[1].extra_rows_raw, 255);
    assert_eq!(map.shapes.keys().copied().collect::<Vec<_>>(), [0, 1]);
    assert!(matches!(map.shapes[&0], PortalShape::InlineQuad { .. }));
    assert!(matches!(map.shapes[&1], PortalShape::LinkedPolygon { .. }));
    assert_eq!(map.shapes[&0].vertices(), map.shapes[&1].vertices());
    assert!(read(&b[..0x67c], &tables, 0x85000000).is_ok());
}
#[test]
fn negative_groups_do_not_address_portal_indices_or_unused_directory() {
    let (mut b, tables) = fixture();
    word(&mut b, 0x1a8, 1);
    b[0x402..0x404].copy_from_slice(&(-1i16).to_be_bytes());
    b[0x406] = 0;
    word(&mut b, 0x13c, 0);
    word(&mut b, 0x138, 0);
    let map = read(&b, &tables, 0x85000000).unwrap();
    assert!(map.shapes.is_empty());
    assert_eq!(map.zone_groups[0][0].target_zone, -1);
}
#[test]
fn malformed_group_ranges_indices_and_used_vertex_values_fail_explicitly() {
    let (b, tables) = fixture();
    for (p, v) in [
        (0x1a8, 1),
        (0x1ac, 0),
        (0x404, 0xffff0100),
        (0x13c, 0),
        (0x608, 2),
        (0x608, 65537),
        (0x610, f32::NAN.to_bits()),
        (0x514, f32::INFINITY.to_bits()),
    ] {
        let mut bad = b.clone();
        word(&mut bad, p, v);
        assert!(
            read(&bad, &tables, 0x85000000).is_err(),
            "mutation {p:x}={v:x}"
        );
    }
    for end in 0..0x67c {
        assert!(
            read(&b[..end], &tables, 0x85000000).is_err(),
            "prefix {end:x}"
        );
    }
}
