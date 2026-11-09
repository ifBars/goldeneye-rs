use super::*;

fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], at: usize, target: usize) {
    put(data, at, (target as i32 - at as i32) as u32);
}
fn fixture() -> (Vec<u8>, Vec<Table>, Vec<Texture>) {
    let mut data = vec![0; 0x954];
    let mut tables = (0..16)
        .map(|_| Table {
            name: "test",
            count: 0,
            hash_size_raw: 0,
            offset: 0,
        })
        .collect::<Vec<_>>();
    tables[6].count = 1;
    tables[6].offset = 0x100;
    tables[1].count = 2;
    tables[1].offset = 0x110;
    tables[14].count = 2;
    tables[14].offset = 0x130;
    for (a, v) in [
        (0x100, 0x85000000),
        (0x108, 0x200),
        (0x118, 0x400),
        (0x128, 0x480),
        (0x130, 0xa5000000),
        (0x138, 0x180),
        (0x140, 0xa5000001),
        (0x148, 0x18c),
        (0x180, u32::MAX),
        (0x184, 1),
        (0x18c, u32::MAX),
        (0x200, 0x500),
        (0x248, 7),
        (0x284, 1),
        (0x400, 0x608),
        (0x458, 1),
        (0x480, 0x603),
        (0x4d4, 1),
        (0x600, 0x601),
        (0x604, 1),
        (0x6a4, 1),
        (0x6a8, 4),
        (0x6b0, 0x30000203),
        (0x6c0, 0x20000),
        (0x6c8, 64),
        (0x6e0, 0x980004),
        (0x950, 0x10001),
    ] {
        put(&mut data, a, v);
    }
    for (a, t) in [
        (0x4dc, 0x600),
        (0x654, 0x950),
        (0x658, 0x6c0),
        (0x65c, 0x800),
        (0x660, 0x840),
        (0x664, 0x850),
    ] {
        relative(&mut data, a, t);
    }
    for (i, pos) in [[-1f32, 0., -1.], [1., 0., -1.], [-1., 0., 1.], [1., 0., 1.]]
        .iter()
        .enumerate()
    {
        for (j, v) in pos.iter().enumerate() {
            put(&mut data, 0x800 + i * 16 + j * 4, v.to_bits());
        }
        put(&mut data, 0x800 + i * 16 + 12, 0x12345678);
        for attribute in 0..4 {
            let a = 0x6e4 + i * 8 + attribute * 2;
            data[a..a + 2].copy_from_slice(&(i as u16).to_be_bytes());
        }
        let uv = if i % 2 == 0 { -8192i16 } else { 8192 };
        data[0x840 + i * 4..0x842 + i * 4].copy_from_slice(&uv.to_be_bytes());
        data[0x850 + i * 4..0x854 + i * 4].copy_from_slice(&[i as u8, 128, 255, 255]);
    }
    let textures = (0..2)
        .map(|i| Texture {
            hash: 0x86000000 + i,
            section: 0,
            debug: 0,
            address: 0,
            runtime_pointer_raw: 0,
            width: 1,
            height: 1,
            game_flags: 0,
            flags: 0,
        })
        .collect();
    (data, tables, textures)
}

#[test]
fn zone_split_strip_material_remap_and_independent_attributes_are_preserved() {
    let (d, t, tx) = fixture();
    let scene = read(&d, &t, &tx, 0x85000000).unwrap();
    assert_eq!(
        (
            scene.geometry.meshes,
            scene.geometry.splits,
            scene.geometry.zone_links
        ),
        (1, 1, 1)
    );
    assert_eq!(scene.placements_not_loaded, 7);
    assert_eq!(scene.geometry.indices, [0, 1, 2, 2, 1, 3]);
    assert_eq!(scene.geometry.bounds, [[-1., 0., -1.], [1., 0., 1.]]);
    assert_eq!(scene.geometry.vertices[0].uv, [-1., 0.]);
    assert_eq!(scene.geometry.vertices[1].color, [1, 128, 255, 255]);
    assert_eq!(scene.geometry.vertices[0].packed_normal_raw, 0x12345678);
    assert_eq!(scene.geometry.batches[0].binding_index, 1);
    assert_eq!(
        scene.geometry.texture_bindings[scene.geometry.batches[0].binding_index],
        0x86000001
    );
    assert_eq!(
        scene.geometry.material_references[1].texture_hash,
        Some(0x86000000)
    );
    assert_eq!(
        scene.geometry.material_references[0].texture_hash,
        Some(0x86000001)
    );
}

#[test]
fn malformed_graph_indices_commands_counts_and_floats_are_rejected() {
    let (original, t, tx) = fixture();
    for (at, value) in [
        (0x458, 2),
        (0x4d4, u32::MAX),
        (0x6a8, 0),
        (0x6b0, 0x30000204),
        (0x6c0, 0x30000),
        (0x6e0, 0x900004),
        (0x6e4, 0xffff0000),
        (0x800, f32::NAN.to_bits()),
        (0x950, 0x1ffff),
    ] {
        let mut d = original.clone();
        put(&mut d, at, value);
        assert!(read(&d, &t, &tx, 0x85000000).is_err(), "field {at:#x}");
    }
    let mut d = original;
    relative(&mut d, 0x4dc, 0x400);
    assert!(format!("{:#}", read(&d, &t, &tx, 0x85000000).unwrap_err()).contains("cyclic"));
}

#[test]
fn empty_splits_do_not_address_optional_fields_or_children() {
    // Ends immediately after the count. Neither +0x58 nor +0x5c is readable.
    let mut data = vec![0; 0x58];
    put(&mut data, 0, 0x603);
    let refs = Table {
        name: "references",
        count: 0,
        hash_size_raw: 0,
        offset: 0,
    };
    let mut geometry = Geometry::new(Vec::new(), &[]);
    let mut reader = Reader {
        b: Bytes(&data),
        refs: &refs,
        geometry: &mut geometry,
        active: BTreeSet::new(),
        seen: BTreeSet::new(),
    };
    reader.visit(0, 0).unwrap();
    assert!(reader.active.is_empty());
    assert_eq!(reader.geometry.splits, 1);
    assert!(reader.geometry.vertices.is_empty());
    assert!(reader.geometry.indices.is_empty());

    // A nonempty/truncated or over-budget child list remains an error.
    for count in [1, 4097, u32::MAX] {
        put(&mut data, 0x54, count);
        let mut geometry = Geometry::new(Vec::new(), &[]);
        let mut reader = Reader {
            b: Bytes(&data),
            refs: &refs,
            geometry: &mut geometry,
            active: BTreeSet::new(),
            seen: BTreeSet::new(),
        };
        assert!(reader.visit(0, 0).is_err(), "count {count}");
    }
}

#[test]
fn empty_split_sibling_does_not_discard_later_render_geometry() {
    let (mut data, tables, textures) = fixture();
    put(&mut data, 0x300, 0x603);
    put(&mut data, 0x4d4, 2);
    relative(&mut data, 0x4dc, 0x300);
    relative(&mut data, 0x4e0, 0x600);
    let scene = read(&data, &tables, &textures, 0x85000000).unwrap();
    assert_eq!((scene.geometry.splits, scene.geometry.meshes), (2, 1));
    assert_eq!((scene.vertex_count(), scene.triangle_count()), (4, 2));
}

#[test]
fn every_required_prefix_truncation_is_an_error() {
    let (d, t, tx) = fixture();
    for end in 0..d.len() {
        assert!(
            read(&d[..end], &t, &tx, 0x85000000).is_err(),
            "truncation {end}"
        );
    }
}

fn placed_fixture() -> (Vec<u8>, Vec<Table>, Vec<Texture>) {
    let (mut d, mut tables, textures) = fixture();
    d.resize(0xb00, 0);
    tables[2].count = 2;
    tables[2].offset = 0x150;
    for (at, value) in [
        (0x150, 0x82000001),
        (0x158, 0x600),
        (0x164, 0x82000002),
        (0x16c, 0x980),
        (0x980, 0x609),
        (0x248, 3),
    ] {
        put(&mut d, at, value);
    }
    relative(&mut d, 0x24c, 0xa00);
    for i in 0..3 {
        let at = 0xa00 + i * 60;
        put(&mut d, at, u32::MAX);
        put(&mut d, at + 4, ((i + 1) as f32 * 10.).to_bits());
        for axis in 0..3 {
            put(&mut d, at + 32 + axis * 4, 1f32.to_bits());
        }
        put(&mut d, at + 44, 0x110000);
        put(&mut d, at + 48, if i < 2 { 0x82000001 } else { 0x82000002 });
        d[at + 54..at + 56].copy_from_slice(&(-1i16).to_be_bytes());
    }
    (d, tables, textures)
}

#[test]
fn repeated_placements_share_geometry_but_keep_models_bounds_and_draw_ranges() {
    let (d, t, tx) = placed_fixture();
    let scene =
        read_with_placements(&d, &t, &tx, 0x85000000, PlacementPolicy::AllowUnsupported).unwrap();
    assert_eq!(
        (scene.placements_loaded, scene.placements_not_loaded),
        (2, 1)
    );
    assert_eq!(scene.placements.len(), 3);
    assert_eq!(scene.placements[0].group, -1);
    assert_eq!(scene.placements[0].engine_flags, 17);
    assert_eq!(scene.geometry.vertices.len(), 8);
    assert_eq!(scene.geometry.indices.len(), 12);
    assert_eq!(scene.triangle_count(), 6);
    assert_eq!(scene.geometry.meshes, 2);
    assert_eq!(scene.geometry.batches.len(), 3);
    let a = &scene.geometry.batches[1];
    let b = &scene.geometry.batches[2];
    assert_eq!(
        (a.first_index, a.index_count),
        (b.first_index, b.index_count)
    );
    assert_eq!(a.model[12], 10.);
    assert_eq!(b.model[12], 20.);
    assert_eq!(b.placement_index, Some(1));
    assert_eq!(scene.geometry.vertices[4].position, [-1., 0., -1.]);
    assert_eq!(scene.geometry.bounds, [[-1., 0., -1.], [21., 0., 1.]]);
    assert!(matches!(
        scene.placements[2].status,
        Some(PlacementState::Unsupported {
            entity_type: 0x609,
            address: 0x980
        })
    ));
    assert!(
        read_with_placements(&d, &t, &tx, 0x85000000, PlacementPolicy::RequireAll)
            .unwrap_err()
            .downcast_ref::<UnsupportedEntity>()
            .is_some()
    );
}

#[test]
fn partial_inspection_does_not_hide_corruption_or_missing_entities() {
    let (original, t, tx) = placed_fixture();
    for (at, value) in [
        (0x248, u32::MAX),
        (0x24c, u32::MAX),
        (0xa04, f32::NAN.to_bits()),
        (0xa30, 0x8200ffff),
        (0x158, 0xfffffff0),
    ] {
        let mut d = original.clone();
        put(&mut d, at, value);
        assert!(
            read_with_placements(&d, &t, &tx, 0x85000000, PlacementPolicy::AllowUnsupported)
                .is_err(),
            "field {at:#x}"
        );
    }
}

#[test]
fn batch_texture_index_is_not_a_material_reference_index() {
    let (d, mut t, tx) = fixture();
    t[14].count = 1;
    let scene = read(&d, &t, &tx, 0x85000000).unwrap();
    assert_eq!(scene.geometry.material_references.len(), 1);
    assert_eq!(scene.geometry.texture_bindings.len(), 2);
    let binding = scene.geometry.batches[0].binding_index;
    assert_eq!(binding, 1);
    assert_eq!(scene.geometry.texture_bindings[binding], 0x86000001);
}

fn navigation_fixture() -> (Vec<u8>, Vec<Table>, Vec<Texture>) {
    let (mut d, t, tx) = placed_fixture();
    let graph = crate::fixtures::graph_fixture();
    d.resize(0x1800, 0);
    d[0x1600..0x1600 + graph.len() - 0x100].copy_from_slice(&graph[0x100..]);
    put(&mut d, 0x16c, 0x1600);
    put(&mut d, 0x248, 4);
    d.copy_within(0xa78..0xab4, 0xab4);
    put(&mut d, 0xab8, 1000f32.to_bits());
    (d, t, tx)
}

#[test]
fn navigation_placements_decode_strictly_share_data_and_add_no_gpu_triangles() {
    let (d, t, tx) = navigation_fixture();
    let scene = read_with_placements(&d, &t, &tx, 0x85000000, PlacementPolicy::RequireAll).unwrap();
    assert_eq!(
        (
            scene.placements_loaded,
            scene.render_placements,
            scene.navigation_placements,
            scene.placements_not_loaded
        ),
        (4, 2, 2, 0)
    );
    assert_eq!(scene.nav_meshes.len(), 1);
    assert_eq!(scene.nav_meshes[0].vertices.len(), 4);
    assert_eq!(scene.nav_meshes[0].triangles.len(), 2);
    assert_eq!(scene.nav_meshes[0].vertices[0], [0.; 3]);
    for index in [2, 3] {
        assert!(matches!(
            scene.placements[index].status,
            Some(PlacementState::Navigation {
                nav_mesh: 0,
                vertices: 4,
                triangles: 2
            })
        ));
    }
    assert_eq!(scene.placements[2].model[12], 30.);
    assert_eq!(scene.placements[3].model[12], 1000.);
    assert_eq!(
        (
            scene.vertex_count(),
            scene.triangle_count(),
            scene.geometry.batches.len()
        ),
        (8, 6, 3)
    );
    assert_eq!(scene.geometry.bounds, [[-1., 0., -1.], [21., 0., 1.]]);
}

#[test]
fn incomplete_mode_cannot_silently_discard_a_corrupt_known_navigation_mesh() {
    let (original, t, tx) = navigation_fixture();
    for (at, value) in [
        (0x166c, 0),
        (0x1678, 0x80000000),
        (0x16c0, f32::NAN.to_bits()),
        (0x16f0, 4),
        (0x1718, 4),
        (0x1764, 1),
    ] {
        let mut d = original.clone();
        put(&mut d, at, value);
        for policy in [
            PlacementPolicy::RequireAll,
            PlacementPolicy::AllowUnsupported,
        ] {
            let error = read_with_placements(&d, &t, &tx, 0x85000000, policy).unwrap_err();
            assert!(
                error.downcast_ref::<UnsupportedEntity>().is_none(),
                "known navigation data at {at:#x} was mislabeled unsupported"
            );
        }
    }
}
