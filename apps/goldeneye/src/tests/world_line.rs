use super::*;
use crate::world_query::{BoundsInput, World};
use std::cell::RefCell;

const MAP: u32 = 0x85000000;
const ZONES: [usize; 2] = [0x488, 0x514];
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(bytes: &mut [u8], at: usize, target: usize) {
    put(bytes, at, (target as i32 - at as i32) as u32);
}
fn xyz(bytes: &mut [u8], at: usize, point: [f32; 3]) {
    for (axis, value) in point.into_iter().enumerate() {
        put(bytes, at + axis * 4, value.to_bits());
    }
}
/// Authored two-zone EDB. Both zones refer to the same static horizontal
/// triangle. The BSP crosses Y=0, so the downward segment visits zone zero then
/// zone one, providing two equal-parameter hits through one resident mesh.
fn data() -> Vec<u8> {
    let mut bytes = vec![0; 0x1000];
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
        (0x484, 2),
        (0x610, 0xffff),
    ] {
        put(&mut bytes, at, value);
    }
    for (at, target) in [
        (0x4c, 0xd0),
        (0x74, 0xf0),
        (0x158, 0x200),
        (0x15c, 0x300),
        (0x168, 0x380),
        (0x404, 0x600),
    ] {
        relative(&mut bytes, at, target);
    }
    xyz(&mut bytes, 0x110, [-20.; 3]);
    xyz(&mut bytes, 0x120, [20.; 3]);
    xyz(&mut bytes, 0x300, [0.; 3]);
    xyz(&mut bytes, 0x310, [1., 0., 0.]);
    xyz(&mut bytes, 0x320, [0., 0., 1.]);
    for at in [0x11c, 0x12c, 0x30c, 0x31c, 0x32c] {
        put(&mut bytes, at, f32::NAN.to_bits()); // Unused homogeneous lanes.
    }
    for zone in ZONES {
        xyz(&mut bytes, zone + 0x68, [-2.; 3]);
        xyz(&mut bytes, zone + 0x74, [2.; 3]);
    }
    xyz(&mut bytes, 0x600, [0., 1., 0.]);
    bytes
}
fn line() -> LineSnapshot {
    LineSnapshot {
        origin: [0.25, 1., 0.25],
        delta: [0., -2., 0.],
        constructor_delta: None,
        maximum_parameter: 1.,
        filter_slot: 0,
        filter_flags: 0xc00,
        required_any: 0,
        excluded_any: 0,
        stop_first_raw: 0,
    }
}
#[test]
fn caller_delta_mutation_keeps_constructor_broad_bounds_until_an_actual_mesh_hit_refreshes_them() {
    let mut bytes = data();
    for zone in ZONES {
        put(&mut bytes, zone, 0x00ff_ffff);
    }
    xyz(&mut bytes, 0x110, [0.; 3]);
    xyz(&mut bytes, 0x120, [1., 0., 1.]);
    put(&mut bytes, 0x50, 0x10000);
    relative(&mut bytes, 0x54, 0xb00);
    put(&mut bytes, 0xb00, 0x82000000);
    put(&mut bytes, 0xb08, 0x100);
    put(&mut bytes, 0x448, 1);
    relative(&mut bytes, 0x44c, 0xb40);
    xyz(&mut bytes, 0xb40 + 0x20, [1.; 3]);
    for (zone, collection, leaf) in [(ZONES[0], 0xc40, 0xc60), (ZONES[1], 0xc80, 0xca0)] {
        relative(&mut bytes, zone + 0x28, collection);
        relative(&mut bytes, collection + 8, leaf);
        put(&mut bytes, collection, 1);
        relative(&mut bytes, collection + 4, collection + 0x10);
        put(&mut bytes, collection + 0x10, 0);
        put(&mut bytes, leaf, 0x00080001);
        put(&mut bytes, leaf + 4, 0x800);
    }
    let edb = Edb::parse(bytes).unwrap();
    let mut input = line();
    input.filter_flags = 0x400;
    input.constructor_delta = Some([0.; 3]);
    let stale = query_map(&edb, MAP, &input).unwrap();
    assert!(stale["line"]["hit"].is_null());
    assert_eq!(
        stale["initial_world_bounds"]["minimum"],
        json!([0.25, 1., 0.25])
    );
    assert!(
        stale["zones"]
            .as_array()
            .unwrap()
            .iter()
            .all(|zone| zone["placement_bounds_rejected"] == 1)
    );
    input.constructor_delta = None;
    let hit = query_map(&edb, MAP, &input).unwrap();
    assert_eq!(hit["line"]["maximum_parameter"], 0.5);
    assert_eq!(hit["line"]["binding"]["placement_index"], 0);
    assert_eq!(hit["initial_world_bounds"]["minimum"][1], -1.);
    assert_eq!(hit["line"]["world_bounds"]["minimum"][1], 0.);
}
fn world(resource: &str, minimum: [f32; 3], maximum: [f32; 3]) -> World {
    World {
        resource: resource.into(),
        map: MAP,
        bounds: BoundsInput { minimum, maximum },
    }
}

#[test]
fn nearest_equal_hit_replaces_binding_across_bsp_zones_and_reuses_resident_mesh() {
    let result = query_map(&Edb::parse(data()).unwrap(), MAP, &line()).unwrap();
    assert_eq!(result["zone_binding"]["zones"], json!([0, 1]));
    assert_eq!(result["resident_meshes"], 1);
    assert_eq!(result["roots"].as_array().unwrap().len(), 2);
    assert_eq!(result["line"]["maximum_parameter"], 0.5);
    assert_eq!(result["line"]["binding"]["zone"], 1);
    assert!(result["line"]["binding"]["placement_index"].is_null());
    assert!(result["line"]["binding"]["candidate_ordinal"].is_null());
    assert_eq!(result["line"]["parameter_point"], json!([0.25, 0., 0.25]));
    assert_eq!(result["line"]["world_normal"], json!([0., -1., 0., 0.]));
    assert!(!result["line"]["hit"].is_null());
    assert_eq!(result["line"]["hit"]["triangle_flags_raw"], 0x11);
    assert_eq!(result["line"]["hit"]["material_raw"], 0x2a);
    assert_eq!(result["line"]["hit"]["first_corner_key"], 0x224);
    for (index, disposition, generated) in [(0, "generated", true), (1, "reused", false)] {
        let mesh = &result["roots"][index]["meshes"][0]["mesh"];
        assert_eq!(mesh["cache_preparation"]["disposition"], disposition);
        assert_eq!(mesh["planes_generated"], generated);
        assert_eq!(mesh["planes_count"], 1);
    }
    assert_eq!(result["reference_compared"], false);
}

#[test]
fn any_nonzero_stop_byte_returns_first_zone_before_world_normal_conversion() {
    for byte in [1, 0x80, 0xff] {
        let mut request = line();
        request.stop_first_raw = byte;
        let result = query_map(&Edb::parse(data()).unwrap(), MAP, &request).unwrap();
        // The full original segment zone list is bound before the first root.
        assert_eq!(result["zone_binding"]["zones"], json!([0, 1]));
        assert_eq!(result["roots"].as_array().unwrap().len(), 1);
        assert_eq!(result["line"]["binding"]["zone"], 0);
        assert_eq!(result["line"]["maximum_parameter"], 0.5);
        assert!(result["line"]["world_normal"].is_null());
    }
}

#[test]
fn flags_gate_both_zones_without_becoming_geometry_filter_or_material_masks() {
    let edb = Edb::parse(data()).unwrap();
    for (required, excluded, expected) in [
        (0, 0, true),
        (0x10, 0, true),
        (0x18, 2, true),
        (2, 0, false),
        (0x10000, 0, false),
        (0, 0x10, false),
        (0x10, 1, false),
    ] {
        let request = LineSnapshot {
            required_any: required,
            excluded_any: excluded,
            ..line()
        };
        let result = query_map(&edb, MAP, &request).unwrap();
        assert_eq!(
            !result["line"]["hit"].is_null(),
            expected,
            "{required:#x}, {excluded:#x}"
        );
        assert_eq!(result["roots"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn maximum_parameter_is_closed_and_rejection_keeps_explicit_miss_state() {
    let edb = Edb::parse(data()).unwrap();
    for (maximum, expected) in [(0.49, false), (0.5, true)] {
        let result = query_map(
            &edb,
            MAP,
            &LineSnapshot {
                maximum_parameter: maximum,
                ..line()
            },
        )
        .unwrap();
        assert_eq!(!result["line"]["hit"].is_null(), expected);
        if !expected {
            assert_eq!(result["line"]["maximum_parameter"], json!(maximum));
            assert!(result["line"]["binding"].is_null());
            assert!(result["line"]["world_normal"].is_null());
            assert!(result["line"]["parameter_point"].is_null());
        }
    }
}

#[test]
fn disabled_triangle_is_not_a_hit_but_its_resident_plane_still_consumes_xyz() {
    let mut bytes = data();
    put(&mut bytes, 0x380, 0x2110000);
    let result = query_map(&Edb::parse(bytes.clone()).unwrap(), MAP, &line()).unwrap();
    assert!(result["line"]["hit"].is_null());
    // Unlike bounds-cache preparation, resident line planes include disabled
    // triangles. An invalid consumed XYZ must remain an error, not clear space.
    put(&mut bytes, 0x300, f32::NAN.to_bits());
    assert!(query_map(&Edb::parse(bytes).unwrap(), MAP, &line()).is_err());
}

#[test]
fn root_bounds_reject_before_addressing_malformed_optional_coarse_tree() {
    let mut bytes = data();
    put(&mut bytes, 0x144, 0x7fffffff);
    xyz(&mut bytes, 0x110, [10.; 3]);
    xyz(&mut bytes, 0x120, [20.; 3]);
    let result = query_map(&Edb::parse(bytes.clone()).unwrap(), MAP, &line()).unwrap();
    assert!(result["line"]["hit"].is_null());
    assert_eq!(result["roots"].as_array().unwrap().len(), 2);
    let first_mesh = &result["roots"][0]["meshes"][0]["mesh"];
    assert_eq!(first_mesh["cache_preparation"]["disposition"], "generated");
    assert_eq!(first_mesh["bounds_rejected"], true);
    assert_eq!(first_mesh["coarse_tested"], false);
    assert_eq!(first_mesh["planes_generated"], false);
    xyz(&mut bytes, 0x110, [-20.; 3]);
    assert!(query_map(&Edb::parse(bytes).unwrap(), MAP, &line()).is_err());
}

#[test]
fn line_zone_binding_never_reads_radius_portal_rows_or_shapes() {
    let mut bytes = data();
    put(&mut bytes, 0x438, u32::MAX);
    put(&mut bytes, 0x43c, 0x7fffffff);
    for zone in ZONES {
        put(&mut bytes, zone + 0x20, u32::MAX);
        put(&mut bytes, zone + 0x24, 0x7fffffff);
    }
    let result = query_map(&Edb::parse(bytes).unwrap(), MAP, &line()).unwrap();
    assert_eq!(result["zone_binding"]["zones"], json!([0, 1]));
    assert_eq!(result["line"]["binding"]["zone"], 1);
}

#[test]
fn zero_one_first_match_and_fallback_worlds_load_only_selected_resource() {
    let empty = query_loaded(
        &Snapshot {
            worlds: vec![],
            line: line(),
        },
        |_| panic!("no loaded world"),
    )
    .unwrap();
    assert!(empty["selected_world"].is_null() && empty["query"].is_null());
    assert_eq!(empty["bounds_tested"], json!([]));
    for (worlds, selected, tested, expected_resource) in [
        (
            vec![world("single", [f32::NAN; 3], [f32::NAN; 3])],
            0,
            vec![],
            "single",
        ),
        (
            vec![
                world("unused-fallback", [f32::NAN; 3], [f32::NAN; 3]),
                world("first", [0.; 3], [2.; 3]),
                world("unused-later", [f32::NAN; 3], [f32::NAN; 3]),
            ],
            1,
            vec![1],
            "first",
        ),
        (
            vec![
                world("fallback", [f32::NAN; 3], [f32::NAN; 3]),
                world("unused-rejected", [10., f32::NAN, f32::NAN], [f32::NAN; 3]),
            ],
            0,
            vec![1],
            "fallback",
        ),
    ] {
        let loaded = RefCell::new(vec![]);
        let snapshot = Snapshot {
            worlds,
            line: line(),
        };
        let result = query_loaded(&snapshot, |resource| {
            loaded.borrow_mut().push(resource.to_owned());
            Edb::parse(data())
        })
        .unwrap();
        assert_eq!(result["selected_world"], selected);
        assert_eq!(result["bounds_tested"], json!(tested));
        assert_eq!(*loaded.borrow(), vec![expected_resource.to_owned()]);
        assert_eq!(result["query"]["line"]["maximum_parameter"], 0.5);
    }
}

#[test]
fn selected_resource_error_cannot_be_reported_as_an_unobstructed_world() {
    let snapshot = Snapshot {
        worlds: vec![world("selected", [0.; 3], [2.; 3])],
        line: line(),
    };
    let error = query_loaded(&snapshot, |_| {
        anyhow::bail!("selected resource unavailable")
    })
    .unwrap_err();
    assert!(format!("{error:#}").contains("selected resource unavailable"));
}
