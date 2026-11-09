use super::*;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn data() -> Vec<u8> {
    let mut bytes = vec![0; 0x800];
    for (at, value) in [
        (0, 0x47454f4d),
        (8, 263),
        (20, 0x800),
        (24, 0x800),
        (0x48, 0x10000),
        (0x4c, 0x84),
        (0x70, 0x10000),
        (0x74, 0x7c),
        (0xd8, 0x100),
        (0xf0, 0x85000000),
        (0xf8, 0x400),
        (0x400, 0x500),
        (0x484, 1),
        (0x100, 0x601),
        (0x104, 0x100),
        (0x13c, 2f32.to_bits()),
        (0x158, 0xa8),
        (0x15c, 0x1a4),
        (0x168, 0x218),
        (0x1a4, 1),
        (0x1a8, 3),
        (0x1b0, 0x30000203),
        (0x200, 0x1002a),
        (0x208, 28),
        (0x220, 0x980003),
        (0x224, 0),
        (0x22c, 0x10000),
        (0x234, 0x20000),
        (0x380, 0x110000),
    ] {
        put(&mut bytes, at, value);
    }
    for at in [0x120, 0x124, 0x128] {
        put(&mut bytes, at, 1f32.to_bits());
    }
    put(&mut bytes, 0x310, 1f32.to_bits());
    put(&mut bytes, 0x328, 1f32.to_bits());
    for at in [0x30c, 0x31c, 0x32c] {
        put(&mut bytes, at, f32::NAN.to_bits());
    }
    bytes
}
fn snapshot() -> Snapshot {
    Snapshot {
        world_origin: [0.; 3],
        radius: 2.,
        world_from_query: IDENTITY,
        boxes: vec![OrientedBox {
            center: [0.; 3],
            axes: [[1., 0., 0.], [0., 1., 0.]],
            half_extents: [0.25; 3],
        }],
        filter_slot: 0,
        filter_flags: 0xc00,
        record_capacity: 16,
        contact_capacity: 8,
    }
}
#[test]
fn embedded_mesh_static_collection_cache_and_world_contact_form_one_asset_backed_path() {
    let edb = Edb::parse(data()).unwrap();
    let result = query_map(&edb, 0x85000000, 0, &snapshot()).unwrap();
    assert_eq!(result["contacts"].as_array().unwrap().len(), 1);
    assert_eq!(result["resident_meshes"], 1);
    assert_eq!(result["probes"][0]["cache"]["disposition"], "generated");
    let contact = &result["contacts"][0];
    assert_eq!(contact["collected_record_key"], 1);
    assert_eq!(contact["first_corner_key"], 0x224);
    assert_eq!(contact["material_raw"], 42);
    assert_eq!(contact["position_indices"], serde_json::json!([0, 1, 2]));
    assert_eq!(contact["witness_b"][1], 0.);
    assert_eq!(result["reference_compared"], false);
}
#[test]
fn repeated_mesh_records_reuse_cache_and_inverse_without_deduplicating_contacts() {
    let mut bytes = data();
    put(&mut bytes, 0x48, 0x20000);
    put(&mut bytes, 0xd8, 0x600);
    put(&mut bytes, 0xe8, 0x100);
    put(&mut bytes, 0x600, 0x608);
    put(&mut bytes, 0x604, 0x100);
    bytes[0x654] = 2;
    put(&mut bytes, 0x658, 1);
    put(&mut bytes, 0x66c, 1);
    let result = query_map(&Edb::parse(bytes).unwrap(), 0x85000000, 0, &snapshot()).unwrap();
    assert_eq!(result["contacts"].as_array().unwrap().len(), 2);
    assert_eq!(result["resident_meshes"], 1);
    assert_eq!(result["inverse_preparations"], 1);
    assert_eq!(result["probes"][1]["cache"]["disposition"], "reused");
    assert_eq!(result["contacts"][1]["collected_record_key"], 2);
}
#[test]
fn translated_request_frame_maps_query_shape_and_witnesses_without_moving_mesh_positions() {
    let mut request = snapshot();
    request.world_from_query[12] = 10.;
    request.boxes[0].center[0] = -10.;
    let result = query_map(&Edb::parse(data()).unwrap(), 0x85000000, 0, &request).unwrap();
    assert_eq!(result["contacts"].as_array().unwrap().len(), 1);
    let x = result["contacts"][0]["witness_b"][0].as_f64().unwrap();
    assert!((0. ..=1.).contains(&x));
}
#[test]
fn disabled_positions_remain_unread_and_capacity_limits_remain_visible() {
    let mut bytes = data();
    put(&mut bytes, 0x380, 0x2000000);
    put(&mut bytes, 0x15c, 0x7fffffff);
    let result = query_map(&Edb::parse(bytes).unwrap(), 0x85000000, 0, &snapshot()).unwrap();
    assert_eq!(result["contacts"].as_array().unwrap().len(), 0);
    assert_eq!(result["probes"][0]["mesh"]["batch_rejections"], 1);
    for (capacity, mesh_skips, post_full) in [(0, 0, 1), (1, 1, 0)] {
        let mut request = snapshot();
        request.contact_capacity = capacity;
        let result = query_map(&Edb::parse(data()).unwrap(), 0x85000000, 0, &request).unwrap();
        assert!(result["contacts"].as_array().unwrap().is_empty());
        assert_eq!(
            result["probes"][0]["mesh"]["capacity_mesh_skips"],
            mesh_skips
        );
        assert_eq!(result["probes"][0]["mesh"]["post_hit_full"], post_full);
    }
}
