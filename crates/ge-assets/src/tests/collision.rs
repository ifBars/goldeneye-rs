use super::*;
fn put(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(b: &mut [u8], at: usize, value: u16) {
    b[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(b: &mut [u8], at: usize, to: usize) {
    put(b, at, (to as i32 - at as i32) as u32);
}
fn fixture() -> Vec<u8> {
    let mut b = crate::fixtures::graph_fixture();
    b.resize(0x800, 0);
    put(&mut b, 20, 0x800);
    put(&mut b, 24, 0x800);
    b[0x100..0x1b4].fill(0);
    for (at, value) in [
        (0x100, 0x601),
        (0x1a4, 2),
        (0x1a8, 4),
        (0x1b0, 0x30000203),
        (0x200, 0x20000),
        (0x208, 36),
        (0x220, 0x980004),
        (0x244, 0x10000),
        (0x24c, 28),
        (0x264, 0x980003),
    ] {
        put(&mut b, at, value);
    }
    relative(&mut b, 0x158, 0x200);
    relative(&mut b, 0x15c, 0x400);
    relative(&mut b, 0x168, 0x500);
    for (i, p) in [[0f32, 0., 0.], [1., 0., 0.], [0., 0., 1.], [1., 0., 1.]]
        .into_iter()
        .enumerate()
    {
        for (k, v) in p.into_iter().enumerate() {
            put(&mut b, 0x400 + i * 16 + k * 4, v.to_bits());
        }
        put(&mut b, 0x40c + i * 16, u32::MAX); // Packed normal is not a fourth float.
        half(&mut b, 0x224 + i * 8, i as u16);
    }
    for i in 0..3 {
        half(&mut b, 0x268 + i * 8, i as u16);
    }
    half(&mut b, 0x504, 0x200);
    b
}
fn parse(b: Vec<u8>) -> Result<CollisionMesh> {
    crate::edb::Edb::parse(b)?.collision_mesh_at(0)
}

#[test]
fn collision_keeps_raw_odd_strip_order_and_separate_triangle_flags() {
    let mesh = parse(fixture()).unwrap();
    assert_eq!(mesh.triangles.len(), 3);
    assert_eq!(mesh.positions_read, 4);
    assert_eq!(mesh.triangles[0].position_indices, [0, 1, 2]);
    assert_eq!(mesh.triangles[1].position_indices, [1, 2, 3]);
    assert_eq!(mesh.triangles[1].triangle_in_strip, 1);
    assert_eq!(
        mesh.triangles
            .iter()
            .map(|t| (t.first_corner_offset, t.flag_index))
            .collect::<Vec<_>>(),
        [(0x224, 0), (0x22c, 1), (0x268, 2)]
    );
    assert_eq!(
        mesh.batches
            .iter()
            .map(|b| (
                b.first_triangle,
                b.first_flag_index,
                b.declared_triangles,
                b.triangle_count
            ))
            .collect::<Vec<_>>(),
        [(0, 0, 2, 2), (2, 2, 1, 1)]
    );
    assert_eq!(mesh.address, 0x100);
    assert_eq!(mesh.entity.as_ref().unwrap().hash, 0x82000010);
    assert_eq!(mesh.triangles[2].flags_raw, 0x200);
    assert!(mesh.triangle_vertices(2).unwrap().is_none());
    assert_eq!(mesh.triangle_vertices(1).unwrap().unwrap()[0], [1., 0., 0.]);
}
#[test]
fn skipped_batch_does_not_consume_triangle_flags_or_parse_its_commands() {
    let mut b = fixture();
    half(&mut b, 0x204, 0x100);
    b[0x220..0x244].fill(0xff);
    let mesh = parse(b).unwrap();
    assert_eq!(mesh.skipped_batches, 1);
    assert_eq!(mesh.triangles.len(), 1);
    assert_eq!(mesh.triangles[0].batch_index, 1);
    assert_eq!(mesh.triangles[0].flags_raw, 0);
    assert_eq!(
        (
            mesh.triangles[0].first_corner_offset,
            mesh.triangles[0].flag_index
        ),
        (0x268, 0)
    );
    assert_eq!(
        (
            mesh.batches[0].first_flag_index,
            mesh.batches[0].triangle_count
        ),
        (0, 0)
    );
    assert_eq!(
        (
            mesh.batches[1].first_flag_index,
            mesh.batches[1].triangle_count
        ),
        (0, 1)
    );
    assert_eq!(mesh.positions_read, 3);
    assert!(mesh.positions[3].is_none());
}
#[test]
fn disabled_triangles_do_not_address_position_payloads_or_indices() {
    let mut b = fixture();
    put(&mut b, 0x15c, 0);
    put(&mut b, 0x1a8, 0);
    for at in [0x500, 0x502, 0x504] {
        half(&mut b, at, 0x200);
    }
    for at in [0x224, 0x22c, 0x234, 0x23c, 0x268, 0x270, 0x278] {
        half(&mut b, at, u16::MAX);
    }
    let mesh = parse(b.clone()).unwrap();
    assert_eq!(mesh.disabled_triangles, 3);
    assert_eq!(mesh.positions_read, 0);
    half(&mut b, 0x504, 0);
    assert!(parse(b).is_err());
}
#[test]
fn resident_line_planes_read_included_disabled_positions_without_changing_contact_laziness() {
    let mut b = fixture();
    for at in [0x500, 0x502, 0x504] {
        half(&mut b, at, 0x200);
    }
    let edb = crate::edb::Edb::parse(b.clone()).unwrap();
    assert_eq!(
        edb.collision_mesh_at_address(0x100).unwrap().positions_read,
        0
    );
    let mesh = edb.collision_line_mesh_at_address(0x100).unwrap();
    assert_eq!(mesh.positions_read, 4);
    assert_eq!(mesh.disabled_triangles, 3);
    assert!(mesh.entity.is_none());
    put(&mut b, 0x400, f32::NAN.to_bits());
    let edb = crate::edb::Edb::parse(b).unwrap();
    assert!(edb.collision_mesh_at_address(0x100).is_ok());
    assert!(edb.collision_line_mesh_at_address(0x100).is_err());
}
#[test]
fn unused_pools_and_format_are_not_required_when_all_batches_are_excluded() {
    let mut b = fixture();
    half(&mut b, 0x204, 1);
    half(&mut b, 0x248, 0x100);
    put(&mut b, 0x15c, 0);
    put(&mut b, 0x168, 0);
    put(&mut b, 0x1b0, 0xdeadbeef);
    let mesh = parse(b).unwrap();
    assert!(mesh.triangles.is_empty());
    assert_eq!(mesh.skipped_batches, 2);
}
#[test]
fn collision_stops_at_declared_count_and_preserves_unused_command_words() {
    let mut b = fixture();
    put(&mut b, 0x1a4, 1);
    put(&mut b, 0x208, 40);
    put(&mut b, 0x244, 0xdeadbeef);
    half(&mut b, 0x220, 0xbeef);
    let mesh = parse(b).unwrap();
    assert_eq!(mesh.batches[0].unconsumed_stream_bytes, 4);
    assert_eq!(mesh.triangles[0].command_raw, 0xbeef);
}
#[test]
fn malformed_used_ranges_counts_formats_and_positions_are_errors() {
    for (at, value) in [
        (0x158, u32::MAX),
        (0x168, 0),
        (0x15c, 0),
        (0x1a4, 65536),
        (0x1a8, 65537),
        (0x1b0, 0x999),
        (0x208, 2),
        (0x220, 0x980002),
        (0x200, 0x10000),
        (0x400, f32::NAN.to_bits()),
    ] {
        let mut b = fixture();
        put(&mut b, at, value);
        assert!(parse(b).is_err(), "offset {at:x}");
    }
    let mut b = fixture();
    half(&mut b, 0x224, 4);
    assert!(parse(b).is_err());
    let mut b = fixture();
    put(&mut b, 0x100, 0x607);
    assert!(parse(b).is_err());
}
#[test]
fn directory_selection_preserves_duplicates_and_only_reads_its_selected_descriptor() {
    let mut b = fixture();
    put(&mut b, 0x50, 0x20000);
    put(&mut b, 0xe4, 0x82000010);
    put(&mut b, 0xec, 0x100);
    let edb = crate::edb::Edb::parse(b.clone()).unwrap();
    assert!(edb.collision_mesh(0x82000010).is_err());
    assert!(edb.collision_mesh_at(1).is_ok());
    put(&mut b, 0xd8, u32::MAX);
    assert!(
        crate::edb::Edb::parse(b)
            .unwrap()
            .collision_mesh_at(1)
            .is_ok()
    );
}

#[test]
fn addressed_nonroot_mesh_has_body_provenance_without_invented_directory_registration() {
    let mut b = fixture();
    let body = b[0x100..0x1b4].to_vec();
    b[0x600..0x6b4].copy_from_slice(&body);
    relative(&mut b, 0x658, 0x200);
    relative(&mut b, 0x65c, 0x400);
    relative(&mut b, 0x668, 0x500);
    let mesh = read_at(&b, 0x600).unwrap();
    assert_eq!(mesh.address, 0x600);
    assert!(mesh.entity.is_none());
    assert_eq!(mesh.triangles.len(), 3);
    assert_eq!(mesh.positions_read, 4);
    assert_eq!(mesh.triangles[1].position_indices, [1, 2, 3]);
    assert_eq!(mesh.triangles[1].first_corner_offset, 0x22c);
    // Broken, duplicated or absent root registrations cannot change address decoding.
    put(&mut b, 0x50, 0x20000);
    put(&mut b, 0xe4, 0x82000010);
    put(&mut b, 0xec, u32::MAX);
    put(&mut b, 0xd8, u32::MAX);
    assert!(
        crate::edb::Edb::parse(b.clone())
            .unwrap()
            .collision_mesh(0x82000010)
            .is_err()
    );
    assert!(read_at(&b, 0x600).unwrap().entity.is_none());
    put(&mut b, 0x50, 0);
    assert!(read_at(&b, 0x600).is_ok());
}

#[test]
fn addressed_mesh_rejects_unknown_tags_ranges_and_keeps_disabled_and_excluded_rules() {
    let mut b = fixture();
    assert!(read_at(&b, usize::MAX).is_err());
    assert!(read_at(&b, b.len() - 4).is_err());
    for kind in [0x607, 0x603, 0x608, 0x999] {
        put(&mut b, 0x100, kind);
        assert!(read_at(&b, 0x100).is_err());
    }
    put(&mut b, 0x100, 0x601);
    half(&mut b, 0x204, 0x101);
    put(&mut b, 0x15c, 0);
    put(&mut b, 0x1a8, 0);
    half(&mut b, 0x500, 0x200);
    for at in [0x268, 0x270, 0x278] {
        half(&mut b, at, u16::MAX);
    }
    let mesh = read_at(&b, 0x100).unwrap();
    assert!(mesh.entity.is_none());
    assert_eq!(mesh.skipped_batches, 1);
    assert_eq!(mesh.disabled_triangles, 1);
    assert_eq!(mesh.positions_read, 0);
    assert_eq!(mesh.triangles[0].flag_index, 0);
    assert!(mesh.triangle_vertices(0).unwrap().is_none());
    half(&mut b, 0x500, 0);
    assert!(read_at(&b, 0x100).is_err());
}
