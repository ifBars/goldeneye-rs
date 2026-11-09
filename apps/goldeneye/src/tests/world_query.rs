use super::*;
use ge_core::collision::OrientedBox;
use std::cell::RefCell;

const MAP: u32 = 0x85000000;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
const ZONE0: usize = 0x488;
const ZONE1: usize = 0x514;
fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], at: usize, target: usize) {
    put(data, at, (target as i32 - at as i32) as u32);
}
fn xyz(data: &mut [u8], at: usize, point: [f32; 3]) {
    for (axis, value) in point.into_iter().enumerate() {
        put(data, at + axis * 4, value.to_bits());
    }
}
fn portal(data: &mut [u8], vertices: [[f32; 3]; 4]) {
    for (index, point) in vertices.into_iter().enumerate() {
        xyz(data, 0x714 + index * 12, point);
    }
}
/// Two zones, one raw static triangle and two separate initial-zone portal
/// groups targeting the same second zone. No game data or files are required.
fn data() -> Vec<u8> {
    let mut data = vec![0; 0x1000];
    for (at, value) in [
        (0, 0x47454f4d),
        (8, 263),
        (20, 0x1000),
        (24, 0x1000),
        (0x48, 0x10000),
        (0x70, 0x10000),
        (0xd8, 0x100),
        (0xf0, MAP),
        (0xf8, 0x400),
        (0x100, 0x601),
        (0x104, 0x100),
        (0x13c, 100f32.to_bits()),
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
        (0x400, 0x500),
        (0x438, 1),
        (0x484, 2),
        (0x610, 0xffff),
        (ZONE0 + 0x20, 2),
        (0x800, 1),
        (0x808, 1),
    ] {
        put(&mut data, at, value);
    }
    for (at, target) in [
        (0x4c, 0xd0),
        (0x74, 0xf0),
        (0x158, 0x200),
        (0x15c, 0x300),
        (0x168, 0x380),
        (0x404, 0x600),
        (0x43c, 0x700),
        (ZONE0 + 0x24, 0x800),
    ] {
        relative(&mut data, at, target);
    }
    xyz(&mut data, 0x110, [-20.; 3]);
    xyz(&mut data, 0x120, [20.; 3]);
    for at in [0x11c, 0x12c, 0x30c, 0x31c, 0x32c] {
        put(&mut data, at, f32::NAN.to_bits());
    }
    xyz(&mut data, 0x300, [0.; 3]);
    xyz(&mut data, 0x310, [1., 0., 0.]);
    xyz(&mut data, 0x320, [0., 0., 1.]);
    for zone in [ZONE0, ZONE1] {
        xyz(&mut data, zone + 0x68, [-2.; 3]);
        xyz(&mut data, zone + 0x74, [2.; 3]);
    }
    xyz(&mut data, 0x600, [1., 0., 0.]);
    portal(
        &mut data,
        [[-1., 0., -1.], [1., 0., -1.], [1., 0., 1.], [-1., 0., 1.]],
    );
    data
}
fn query() -> crate::mesh_query::Snapshot {
    crate::mesh_query::Snapshot {
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
fn world(resource: &str, minimum: [f32; 3], maximum: [f32; 3]) -> World {
    World {
        resource: resource.into(),
        map: MAP,
        bounds: BoundsInput { minimum, maximum },
    }
}
fn manager(worlds: Vec<World>) -> Snapshot {
    Snapshot {
        worlds,
        request_flags_raw: 0,
        query: query(),
    }
}

#[test]
fn portal_zone_duplicates_share_record_inverse_resident_and_contact_sequences() {
    let result = query_map(&Edb::parse(data()).unwrap(), MAP, 2, &query()).unwrap();
    assert_eq!(result["zone_binding"]["initial_zone"], 0);
    assert_eq!(result["zone_binding"]["bsp_nodes_tested"], 1);
    assert_eq!(result["zone_binding"]["zones"], json!([0, 1, 1]));
    assert_eq!(result["zone_binding"]["portal_tests"], 2);
    assert_eq!(result["collection"]["records"].as_array().unwrap().len(), 3);
    assert_eq!(result["contacts"].as_array().unwrap().len(), 3);
    assert_eq!(result["resident_meshes"], 1);
    assert_eq!(result["inverse_preparations"], 1);
    for index in 0..3 {
        assert_eq!(result["collection"]["zones"][index]["record_start"], index);
        assert_eq!(
            result["collection"]["zones"][index]["record_end"],
            index + 1
        );
        assert_eq!(result["contacts"][index]["collected_record_key"], index + 1);
        assert_eq!(result["collection"]["records"][index]["slot"], -1);
        assert_eq!(
            result["probes"][index]["cache"]["disposition"],
            if index == 0 { "generated" } else { "reused" }
        );
    }
}

#[test]
fn record_capacity_is_shared_but_later_zone_dispatch_still_runs() {
    let mut request = query();
    request.record_capacity = 2;
    let result = query_map(&Edb::parse(data()).unwrap(), MAP, 2, &request).unwrap();
    assert_eq!(result["zone_binding"]["zones"], json!([0, 1, 1]));
    assert_eq!(result["collection"]["records"].as_array().unwrap().len(), 2);
    assert_eq!(result["collection"]["zones"].as_array().unwrap().len(), 3);
    let last = &result["collection"]["zones"][2];
    assert_eq!(
        (last["record_start"].as_u64(), last["record_end"].as_u64()),
        (Some(2), Some(2))
    );
    assert_eq!(last["dispatch"][0]["full_rejections"], 1);
    assert_eq!(result["contacts"].as_array().unwrap().len(), 2);
}

#[test]
fn contact_saturation_ends_one_mesh_but_later_zones_still_prepare_their_records() {
    let mut request = query();
    request.contact_capacity = 2;
    request.boxes = vec![request.boxes[0].clone(); 3];
    let result = query_map(&Edb::parse(data()).unwrap(), MAP, 2, &request).unwrap();
    assert_eq!(result["contacts"].as_array().unwrap().len(), 2);
    assert_eq!(result["probes"].as_array().unwrap().len(), 3);
    assert_eq!(result["probes"][0]["mesh"]["primitive_queries"], 3);
    assert_eq!(result["probes"][0]["mesh"]["post_hit_full"], 1);
    for index in [1, 2] {
        assert_eq!(result["probes"][index]["mesh"]["capacity_mesh_skips"], 1);
        assert_eq!(result["probes"][index]["cache"]["disposition"], "reused");
    }
    assert_eq!(result["contacts"][0]["collected_record_key"], 1);
    assert_eq!(result["contacts"][1]["collected_record_key"], 1);
}

#[test]
fn zero_and_one_contact_capacities_preserve_distinct_guards_across_all_zones() {
    for (capacity, skips, after_hit) in [(0, 0, 1), (1, 1, 0)] {
        let mut request = query();
        request.contact_capacity = capacity;
        let result = query_map(&Edb::parse(data()).unwrap(), MAP, 2, &request).unwrap();
        assert!(result["contacts"].as_array().unwrap().is_empty());
        assert_eq!(result["probes"].as_array().unwrap().len(), 3);
        assert_eq!(result["inverse_preparations"], 1);
        for probe in result["probes"].as_array().unwrap() {
            assert_eq!(probe["mesh"]["capacity_mesh_skips"], skips);
            assert_eq!(probe["mesh"]["post_hit_full"], after_hit);
        }
    }
}

#[test]
fn single_zone_mode_does_not_address_malformed_portal_rows_or_directory() {
    let mut bytes = data();
    put(&mut bytes, 0x43c, 0x7fffffff);
    put(&mut bytes, ZONE0 + 0x20, u32::MAX);
    put(&mut bytes, ZONE0 + 0x24, 0x7fffffff);
    let edb = Edb::parse(bytes).unwrap();
    let result = query_map(&edb, MAP, 0, &query()).unwrap();
    assert_eq!(result["zone_binding"]["zones"], json!([0]));
    assert_eq!(result["zone_binding"]["portal_tests"], 0);
    assert_eq!(result["contacts"].as_array().unwrap().len(), 1);
    assert!(query_map(&edb, MAP, 2, &query()).is_err());
}

#[test]
fn expanded_group_stops_at_first_hit_without_reading_later_shape_or_other_zone_groups() {
    let mut bytes = data();
    put(&mut bytes, 0x438, 2);
    bytes[0x806] = 1; // Two rows in ONE target group: portal zero then portal one.
    bytes[0x80c..0x80e].copy_from_slice(&1u16.to_be_bytes());
    bytes[0x744..0x788].fill(0xff); // Portal one is malformed but unnecessary.
    put(&mut bytes, ZONE1 + 0x20, u32::MAX);
    put(&mut bytes, ZONE1 + 0x24, 0x7fffffff);
    let result = query_map(&Edb::parse(bytes).unwrap(), MAP, 2, &query()).unwrap();
    assert_eq!(result["zone_binding"]["zones"], json!([0, 1]));
    assert_eq!(result["zone_binding"]["portal_tests"], 1);
    assert_eq!(result["contacts"].as_array().unwrap().len(), 2);
}

#[test]
fn zero_one_first_matching_and_fallback_worlds_load_only_the_selected_resource() {
    let result = query_loaded(&manager(vec![]), |_| {
        panic!("no worlds must not load assets")
    })
    .unwrap();
    assert!(result["selected_world"].is_null() && result["query"].is_null());
    assert_eq!(result["bounds_tested"], json!([]));
    let cases = [
        (
            vec![world("only", [f32::NAN; 3], [f32::NAN; 3])],
            0,
            "only",
            json!([]),
        ),
        (
            vec![
                world("fallback", [f32::NAN; 3], [f32::NAN; 3]),
                world("bad-one", [10., f32::NAN, f32::NAN], [f32::NAN; 3]),
                world("bad-two", [20., f32::NAN, f32::NAN], [f32::NAN; 3]),
            ],
            0,
            "fallback",
            json!([1, 2]),
        ),
        (
            vec![
                world("bad-fallback", [f32::NAN; 3], [f32::NAN; 3]),
                world("first", [0.; 3], [1.; 3]),
                world("bad-later", [f32::NAN; 3], [f32::NAN; 3]),
            ],
            1,
            "first",
            json!([1]),
        ),
    ];
    for (worlds, selected, name, bounds_tested) in cases {
        let loaded = RefCell::new(Vec::new());
        let result = query_loaded(&manager(worlds), |resource| {
            loaded.borrow_mut().push(resource.to_owned());
            assert_eq!(resource, name, "unselected invalid resource was addressed");
            Edb::parse(data())
        })
        .unwrap();
        assert_eq!(result["selected_world"], selected);
        assert_eq!(result["bounds_tested"], bounds_tested);
        assert_eq!(*loaded.borrow(), [name]);
        assert_eq!(result["query"]["contacts"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn selected_resource_failure_is_an_error_instead_of_an_empty_world() {
    let snapshot = manager(vec![world("missing", [f32::NAN; 3], [f32::NAN; 3])]);
    let error = query_loaded(&snapshot, |_| anyhow::bail!("missing selected asset")).unwrap_err();
    assert!(format!("{error:#}").contains("missing selected asset"));
}

#[test]
fn per_zone_ordinal_restart_reuses_inverse_even_when_current_placement_matrix_changes() {
    let mut bytes = data();
    // Zone geometry is legitimately unresolved; only placement ordinal zero in
    // each zone enters the global record sequence, so no intervening -1 key exists.
    for zone in [ZONE0, ZONE1] {
        put(&mut bytes, zone, 0x00ffffff);
    }
    put(&mut bytes, ZONE0 + 0x20, 1);
    portal(
        &mut bytes,
        [[10., 0., -1.], [10., 2., -1.], [10., 2., 1.], [10., 0., 1.]],
    );
    put(&mut bytes, 0x50, 0x10000);
    relative(&mut bytes, 0x54, 0xb00);
    put(&mut bytes, 0xb00, 0x82000000);
    put(&mut bytes, 0xb08, 0x100);
    put(&mut bytes, 0x448, 2);
    relative(&mut bytes, 0x44c, 0xb40);
    for (index, position, rotation) in [
        (0, [0.; 3], [0.; 3]),
        (1, [10., 0., 0.], [0., 0., std::f32::consts::FRAC_PI_2]),
    ] {
        let at = 0xb40 + index * 60;
        put(&mut bytes, at, index as u32);
        xyz(&mut bytes, at + 4, position);
        xyz(&mut bytes, at + 0x14, rotation);
        xyz(&mut bytes, at + 0x20, [1.; 3]);
        put(&mut bytes, at + 0x30, 0);
    }
    for (zone, collection, leaf, index) in [(ZONE0, 0xc40, 0xc60, 0), (ZONE1, 0xc80, 0xca0, 1)] {
        relative(&mut bytes, zone + 0x28, collection);
        relative(&mut bytes, collection + 8, leaf);
        put(&mut bytes, collection, 1);
        relative(&mut bytes, collection + 4, collection + 0x10);
        put(&mut bytes, collection + 0x10, index << 16);
        put(&mut bytes, leaf, 0x00080001);
        put(&mut bytes, leaf + 4, (index << 16) | 0x800);
    }
    let edb = Edb::parse(bytes).unwrap();
    let mut request = query();
    request.world_origin = [10., 1.1, 0.25];
    request.radius = 0.5;
    request.filter_flags = 0x400;
    request.boxes[0].center = request.world_origin;
    let together = query_map(&edb, MAP, 2, &request).unwrap();
    assert_eq!(together["zone_binding"]["zones"], json!([0, 1]));
    let records = together["collection"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["slot"], 0);
    assert_eq!(records[1]["slot"], 0);
    assert_ne!(records[0]["matrix"], records[1]["matrix"]);
    assert_eq!(together["inverse_preparations"], 1);
    assert!(together["contacts"].as_array().unwrap().is_empty());
    // A fresh single-zone call has a different inverse-cache history and really
    // reaches the rotated triangle, proving the combined miss is not inert data.
    let fresh = crate::mesh_query::query_map(&edb, MAP, 1, &request).unwrap();
    assert_eq!(fresh["contacts"].as_array().unwrap().len(), 1);
}
