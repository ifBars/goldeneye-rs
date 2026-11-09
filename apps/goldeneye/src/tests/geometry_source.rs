use super::*;
#[test]
fn unsupported_reference_type_is_skipped_only_by_the_original_flag_gate() {
    let mut data = header(0x400);
    put(&mut data, 0x48, 0x10000);
    put(&mut data, 0x4c, 0x300 - 0x4c);
    put(&mut data, 0x308, 0x100);
    put(&mut data, 0x100, 0x999);
    let edb = Edb::parse(data.clone()).unwrap();
    let source = Source(edb.geometry());
    let node = source.reference(0).unwrap().unwrap();
    assert_eq!(
        source
            .filter_node(
                node,
                GeometryFilter {
                    current_slot: 0,
                    flags_raw: 0x400
                }
            )
            .unwrap(),
        None
    );
    put(&mut data, 0x104, 0x100);
    let edb = Edb::parse(data).unwrap();
    let mut source = Source(edb.geometry());
    assert_eq!(
        source
            .filter_node(
                node,
                GeometryFilter {
                    current_slot: 0,
                    flags_raw: 0x400
                }
            )
            .unwrap(),
        Some(node)
    );
    let matrix = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let request = DispatchRequest {
        query: QuerySphere {
            origin: [0.; 3],
            radius: 0.,
        },
        context: CollectionContext::map_zone(matrix),
        optional_context_key: 0,
        traversal_flags: 0,
        collection_flags: 0,
    };
    assert!(
        collect(
            &mut source,
            node,
            &request,
            &mut RecordBuffer::new(2).unwrap(),
            DispatchLimits::default()
        )
        .unwrap_err()
        .to_string()
        .contains("unsupported")
    );
}
use ge_assets::edb::Edb;
use ge_core::collision::{
    collector::{CollectionContext, RecordBuffer},
    dispatch::{DispatchLimits, DispatchRequest, QuerySphere, collect},
};
fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn header(size: usize) -> Vec<u8> {
    let mut data = vec![0; size];
    for (at, value) in [
        (0, 0x47454f4d),
        (8, 263),
        (20, size as u32),
        (24, size as u32),
    ] {
        put(&mut data, at, value);
    }
    data
}
#[test]
fn entity_flag_gate_precedes_bypass_and_missing_component_reads() {
    let mut data = header(0x108);
    put(&mut data, 0x100, 0x603);
    let filter = GeometryFilter {
        current_slot: 4,
        flags_raw: 0x400,
    };
    let edb = Edb::parse(data.clone()).unwrap();
    let source = Source(edb.geometry());
    assert_eq!(source.filter_node(0x100, filter).unwrap(), None);
    put(&mut data, 0x104, 0x100);
    let edb = Edb::parse(data).unwrap();
    let source = Source(edb.geometry());
    assert_eq!(source.filter_node(0x100, filter).unwrap(), Some(0x100));
    assert!(
        source
            .filter_node(
                0x100,
                GeometryFilter {
                    flags_raw: 0,
                    ..filter
                }
            )
            .is_err()
    );
}
#[test]
fn local_reference_and_component_bind_to_runtime_dispatch_without_regating_selected_flags() {
    let mut data = header(0x400);
    put(&mut data, 0x48, 0x00010000);
    put(&mut data, 0x4c, 0x300 - 0x4c);
    put(&mut data, 0x308, 0x100);
    put(&mut data, 0x30c, u32::MAX);
    put(&mut data, 0x100, 0x603);
    put(&mut data, 0x104, 0x100);
    put(&mut data, 0x140, ((0x340i32 - 0x140) << 8) as u32 | 0x10);
    put(&mut data, 0x340, ((0x200i32 - 0x340) << 8) as u32);
    put(&mut data, 0x200, 0x601);
    put(&mut data, 0x21c, f32::NAN.to_bits());
    let edb = Edb::parse(data).unwrap();
    let mut source = Source(edb.geometry());
    assert_eq!(source.reference(0).unwrap(), Some(0x100));
    assert_eq!(source.reference(1).unwrap(), None);
    let selected = source
        .filter_node(
            0x100,
            GeometryFilter {
                current_slot: 4,
                flags_raw: 0,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(selected, 0x200);
    assert!(source.bounds(selected).unwrap().minimum[3].is_nan());
    let matrix = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let request = DispatchRequest {
        query: QuerySphere {
            origin: [0.; 3],
            radius: 0.,
        },
        context: CollectionContext::map_zone(matrix),
        optional_context_key: 0,
        traversal_flags: 1,
        collection_flags: 2,
    };
    let mut records = RecordBuffer::new(1).unwrap();
    let outcome = collect(
        &mut source,
        selected,
        &request,
        &mut records,
        DispatchLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome.appended, 1);
    assert_eq!(records.records()[0].geometry_key, 0x200);
    assert_eq!(
        (records.records()[0].slot, records.records()[0].flags_raw),
        (-1, 2)
    );
}
