use super::*;
fn fixture() -> Edb {
    let mut bytes = vec![0; 0x600];
    let mut put = |at: usize, value: u32| bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
    for (at, value) in [
        (0, 0x47454f4d),
        (8, 263),
        (20, 0x600),
        (24, 0x600),
        (0x48, 0x10000),
        (0x4c, 0x84),
        (0x70, 0x10000),
        (0x74, 0x7c),
        (0xd8, 0x100),
        (0xdc, u32::MAX),
        (0xf0, 0x85000000),
        (0xf8, 0x400),
        (0x100, 0x601),
        (0x104, 0x100),
        (0x140, 0x7fffffff),
        (0x400, 0x500),
        (0x404, 0x7fffffff),
        (0x484, 1),
        (0x4f0, f32::NAN.to_bits()),
    ] {
        put(at, value);
    }
    for at in [0x110, 0x114, 0x118] {
        put(at, (-1f32).to_bits());
    }
    for at in [0x120, 0x124, 0x128, 0x13c] {
        put(at, 1f32.to_bits());
    }
    Edb::parse(bytes).unwrap()
}
#[test]
fn zone_collection_keeps_filter_order_and_does_not_read_unused_bsp_or_placements() {
    let result = snapshot(
        &fixture(),
        0x85000000,
        0,
        QuerySphere {
            origin: [0.; 3],
            radius: 1.,
        },
        GeometryFilter {
            current_slot: 0,
            flags_raw: 0xc00,
        },
        2,
    )
    .unwrap();
    let records = result["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["geometry_key"], 0x100);
    assert_eq!(records[0]["owner_key"], 0);
    assert_eq!(records[0]["slot"], -1);
    assert!(result["refresh"].is_null() && result["candidates"].is_null());
    assert_eq!(result["reference_compared"], false);
}
#[test]
fn full_zero_capacity_does_not_grow_or_hide_full_rejection() {
    let result = snapshot(
        &fixture(),
        0x85000000,
        0,
        QuerySphere {
            origin: [0.; 3],
            radius: 1.,
        },
        GeometryFilter {
            current_slot: 0,
            flags_raw: 0xc00,
        },
        0,
    )
    .unwrap();
    assert_eq!(result["records"].as_array().unwrap().len(), 0);
    assert_eq!(result["dispatch"][0]["full_rejections"], 1);
    assert_eq!(result["dispatch"][0]["returned"], false);
}
