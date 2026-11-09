use super::*;
use crate::edb::Edb;

fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
use crate::fixtures::graph_fixture as fixture;

#[test]
fn optional_stored_normals_are_preserved_without_normalization_and_checked_for_bounds_and_finiteness()
 {
    let original = Edb::parse(fixture())
        .unwrap()
        .entity_graph(0x82000010)
        .unwrap();
    assert!(original.triangles.iter().all(|t| t.normal.is_none()));
    let mut data = fixture();
    data.resize(0x280, 0);
    put(&mut data, 20, 0x280);
    put(&mut data, 24, 0x280);
    put(&mut data, 0x18c, 0x268 - 0x18c);
    let normals = [[0f32, 0.9, 2.], [-3., 0.8999, 4.]];
    for (i, n) in normals.iter().enumerate() {
        for (j, v) in n.iter().enumerate() {
            put(&mut data, 0x268 + i * 12 + j * 4, v.to_bits());
        }
    }
    let g = Edb::parse(data.clone())
        .unwrap()
        .entity_graph(0x82000010)
        .unwrap();
    assert_eq!(g.triangles[0].normal, Some(normals[0]));
    assert_eq!(g.triangles[1].normal, Some(normals[1]));
    assert_eq!(
        (
            g.diagnostics.normal_samples,
            g.diagnostics.low_actor_normal_faces
        ),
        (2, 1)
    );
    put(&mut data, 0x26c, f32::NAN.to_bits());
    assert!(
        Edb::parse(data.clone())
            .unwrap()
            .entity_graph(0x82000010)
            .is_err()
    );
    put(&mut data, 0x26c, 1f32.to_bits());
    put(&mut data, 0x18c, 0x27c - 0x18c);
    assert!(Edb::parse(data).unwrap().entity_graph(0x82000010).is_err());
}

#[test]
fn preserves_graph_topology_weights_attributes_and_unknown_headers() {
    let edb = Edb::parse(fixture()).unwrap();
    let graph = edb.entity_graph(0x82000010).unwrap();
    assert_eq!(graph.entity.section, 1);
    assert_eq!(graph.entity.debug, 2);
    assert_eq!(graph.entity.runtime_pointer_raw, 0x1234);
    assert_eq!(graph.entity.unknown_word, 0xabcd);
    assert_eq!(graph.bounds, [[0.; 3], [2., 0., 2.]]);
    assert_eq!(graph.header_words[0xb0 / 4], 0x0badf00d);
    assert_eq!(graph.vertices.len(), 4);
    assert_eq!(graph.triangles[0].vertices, [0, 1, 2]);
    assert_eq!(graph.triangles[0].attribute_raw, 0x123);
    assert_eq!(graph.triangles[1].attribute_raw, 0x456);
    assert_eq!(graph.triangles[0].neighbors, [None, None, Some(1)]);
    assert_eq!(graph.triangles[1].neighbors, [Some(0), None, None]);
    assert_eq!(graph.diagnostics.neighbors_after_boundary, 1);
    assert_eq!(graph.triangles[0].weights.unwrap()[0], 1e8);
    assert_eq!(graph.groups.len(), 1);
    assert_eq!(graph.groups[0].triangle_count, 2);
    assert_eq!(graph.groups[0].first_triangle, 0);
    let d = graph.diagnostics;
    assert_eq!(
        (d.directed_links, d.boundary_slots, d.connected_components),
        (2, 4, 1)
    );
    assert_eq!(
        (
            d.non_reciprocal_links,
            d.links_without_shared_edge,
            d.degenerate_triangles
        ),
        (0, 0, 0)
    );
    assert!(d.max_centroid_error < 1e-6 && d.max_distance_weight_error < 1e-6);
    assert_eq!((d.centroid_samples, d.distance_weight_samples), (2, 2));
}

#[test]
fn disconnected_components_and_geometric_inconsistencies_are_visible() {
    let mut b = fixture();
    put(&mut b, 0x218, NO_NEIGHBOR);
    put(&mut b, 0x21c, NO_NEIGHBOR);
    let g = Edb::parse(b).unwrap().entity_graph(0x82000010).unwrap();
    assert_eq!(g.diagnostics.connected_components, 2);
    assert_eq!(g.diagnostics.boundary_slots, 6);

    let mut b = fixture();
    put(&mut b, 0x21c, NO_NEIGHBOR);
    for p in [0x200, 0x204, 0x208] {
        put(&mut b, p, 3);
    }
    let d = Edb::parse(b)
        .unwrap()
        .entity_graph(0x82000010)
        .unwrap()
        .diagnostics;
    assert_eq!(d.non_reciprocal_links, 1);
    assert_eq!(d.links_without_shared_edge, 1);
    assert_eq!(d.degenerate_triangles, 1);
    assert!(d.max_centroid_error > 0.5);
}

#[test]
fn malformed_graph_counts_indices_pointers_and_floats_fail() {
    for (p, v) in [
        (0x110, f32::NAN.to_bits()),
        (0x120, (-1f32).to_bits()),
        (0x16c, 0),
        (0x16c, u32::MAX),
        (0x170, 0),
        (0x170, u32::MAX),
        (0x174, 0),
        (0x174, 3),
        (0x178, 0x80000000),
        (0x178, 0),
        (0x1c0, f32::NAN.to_bits()),
        (0x1f0, 4),
        (0x218, 2),
        (0x260, 1),
        (0x264, 1),
        (0x188, 4),
        (0x190, 4),
        (0x22c, f32::INFINITY.to_bits()),
        (0x244, (-1f32).to_bits()),
    ] {
        let mut b = fixture();
        put(&mut b, p, v);
        assert!(
            Edb::parse(b).unwrap().entity_graph(0x82000010).is_err(),
            "{p:#x}: {v:#x}"
        );
    }
}

#[test]
fn every_required_graph_prefix_truncation_is_an_error() {
    let b = fixture();
    let entity = Edb::parse(b.clone()).unwrap().entities().unwrap().remove(0);
    for end in 0..b.len() {
        assert!(read(&b[..end], entity.clone()).is_err(), "prefix {end:#x}");
    }
}

#[test]
fn missing_duplicate_and_wrong_type_entities_are_rejected() {
    let b = fixture();
    assert!(
        Edb::parse(b.clone())
            .unwrap()
            .entity_graph(0xdeadbeef)
            .is_err()
    );
    let mut duplicate = b.clone();
    duplicate.copy_within(0xd0..0xe4, 0xe4);
    put(&mut duplicate, 0x50, 0x20000);
    let edb = Edb::parse(duplicate).unwrap();
    assert!(edb.entity_graph(0x82000010).is_err());
    assert!(edb.entity_graph_at(0).is_ok() && edb.entity_graph_at(1).is_ok());
    assert!(edb.entity_graph_at(2).is_err());
    let mut wrong_type = b;
    put(&mut wrong_type, 0x100, 0x601);
    assert!(
        Edb::parse(wrong_type)
            .unwrap()
            .entity_graph(0x82000010)
            .is_err()
    );
}

#[test]
fn optional_centers_and_weights_remain_absent_instead_of_being_fabricated() {
    for (center, weight) in [(false, false), (true, false), (false, true)] {
        let mut b = fixture();
        if !center {
            put(&mut b, 0x188, 0);
        }
        if !weight {
            put(&mut b, 0x190, 0);
        }
        let graph = Edb::parse(b).unwrap().entity_graph(0x82000010).unwrap();
        assert_eq!(graph.triangles[0].center.is_some(), center);
        assert_eq!(graph.triangles[0].weights.is_some(), weight);
        assert_eq!(graph.diagnostics.directed_links, 2);
        assert_eq!(
            graph.diagnostics.centroid_samples,
            if center { 2 } else { 0 }
        );
        assert_eq!(graph.diagnostics.distance_weight_samples, 0);
    }
}

#[test]
fn tagged_eight_byte_groups_partition_the_triangle_ranges() {
    let mut b = fixture();
    b.resize(0x270, 0);
    put(&mut b, 20, 0x270);
    put(&mut b, 24, 0x270);
    put(&mut b, 0x174, 2);
    put(&mut b, 0x260, 0x04000001);
    put(&mut b, 0x268, 0x99000001);
    put(&mut b, 0x26c, 0xabc00001);
    let graph = Edb::parse(b.clone())
        .unwrap()
        .entity_graph(0x82000010)
        .unwrap();
    assert_eq!(
        graph
            .groups
            .iter()
            .map(|g| (g.tag_raw, g.first_triangle, g.triangle_count))
            .collect::<Vec<_>>(),
        [(4, 0, 1), (0x99, 1, 1)]
    );
    assert_eq!(graph.groups[1].start_high_bits_raw, 0xabc);
    put(&mut b, 0x26c, 0);
    assert!(Edb::parse(b).unwrap().entity_graph(0x82000010).is_err());
}

#[test]
fn packed_vertex_indices_preserve_high_bits_without_using_them_as_addresses() {
    let mut b = fixture();
    put(&mut b, 0x1f8, 0xabc00002);
    let graph = Edb::parse(b).unwrap().entity_graph(0x82000010).unwrap();
    assert_eq!(graph.triangles[0].vertices, [0, 1, 2]);
    assert_eq!(graph.triangles[0].vertex_high_bits_raw, [0, 0, 0xabc]);
}

#[test]
fn packed_neighbors_keep_tags_and_mask_boundary_sentinels_and_indices() {
    let mut b = fixture();
    put(&mut b, 0x210, 0xfff00001);
    put(&mut b, 0x214, 0x123fffff);
    let graph = Edb::parse(b).unwrap().entity_graph(0x82000010).unwrap();
    assert_eq!(graph.triangles[0].neighbors, [Some(1), None, Some(1)]);
    assert_eq!(graph.triangles[0].neighbor_high_bits_raw, [0xfff, 0x123, 0]);
    let mut invalid = fixture();
    put(&mut invalid, 0x210, 0xfff00002);
    assert!(
        Edb::parse(invalid)
            .unwrap()
            .entity_graph(0x82000010)
            .is_err()
    );
}

#[test]
fn self_links_are_reported_instead_of_rejected_before_bounded_traversal() {
    let mut b = fixture();
    put(&mut b, 0x218, 0xabc00000);
    let graph = Edb::parse(b).unwrap().entity_graph(0x82000010).unwrap();
    assert_eq!(graph.triangles[0].neighbors[2], Some(0));
    assert_eq!(graph.diagnostics.self_links, 1);
    assert_eq!(graph.diagnostics.links_without_shared_edge, 1);
}

#[test]
fn optional_face_flags_preserve_all_bits_and_report_the_shortcut_bit() {
    let mut b = fixture();
    assert!(
        Edb::parse(b.clone())
            .unwrap()
            .entity_graph(0x82000010)
            .unwrap()
            .triangles[0]
            .flags_raw
            .is_none()
    );
    b.resize(0x270, 0);
    put(&mut b, 20, 0x270);
    put(&mut b, 24, 0x270);
    put(&mut b, 0x194, 0x268 - 0x194);
    put(&mut b, 0x268, 0xabcd0101);
    put(&mut b, 0x26c, 0xff001000);
    let g = Edb::parse(b.clone())
        .unwrap()
        .entity_graph(0x82000010)
        .unwrap();
    assert_eq!(g.triangles[0].flags_raw, Some(0xabcd0101));
    assert_eq!(g.triangles[1].flags_raw, Some(0xff001000));
    assert_eq!(
        (
            g.diagnostics.flag_samples,
            g.diagnostics.shortcut_blocked_faces
        ),
        (2, 1)
    );
    put(&mut b, 0x194, 4);
    assert!(Edb::parse(b).unwrap().entity_graph(0x82000010).is_err());
}
