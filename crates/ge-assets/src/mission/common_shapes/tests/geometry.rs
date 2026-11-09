use super::*;
use crate::{edb::Edb, mission::common_shapes::Unsupported};

fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    put(
        data,
        field,
        i32::try_from(target as i64 - field as i64).unwrap() as u32,
    );
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x400];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0181_00a5),
        (8, 263),
        (0x14, 0x400),
        (0x18, 0x400),
        (0x70, 0x0001_0001),
        (0xd0, 0x0500_00a5),
        (0xd8, 0x100),
        (0x100, 0x500),
    ] {
        put(&mut data, at, value);
    }
    relative(&mut data, 0x74, 0xd0);
    relative(&mut data, 0x158, 0x200);
    relative(&mut data, 0x210, 0x280);
    // First prefix word and byte7 are not interpreted as geometry semantics.
    put(&mut data, 0x280, u32::MAX);
    data[0x284..0x286].copy_from_slice(&0xa5b6u16.to_be_bytes());
    data[0x287] = 0xfe;
    for (i, value) in [
        0x8000_0000,
        0x7fc0_00a5,
        0x7f80_0000,
        0x3fa0_0000,
        0xc020_0000,
        0xff80_0000,
        0x8000_0000,
        0x3e80_0000,
        0xbf00_0000,
        0x4000_0000,
    ]
    .into_iter()
    .enumerate()
    {
        put(&mut data, 0x288 + i * 4, value);
    }
    data
}
fn budget() -> Budget {
    Budget {
        addressed_bytes: 48,
        ..Budget::default()
    }
}

#[test]
fn full_geometry_preserves_raw_fields_without_finite_checks_or_orientation_repair() {
    let edb = Edb::parse(fixture()).unwrap();
    let shape = edb
        .map_common_shape_geometry(0x8500_0000, 0, budget())
        .unwrap()
        .unwrap();
    assert_eq!(shape.word_04_raw(), 0xa5b6);
    assert_eq!(shape.type_raw(), 0);
    assert_eq!(
        shape.dimensions_bits(),
        [0x8000_0000, 0x7fc0_00a5, 0x7f80_0000]
    );
    assert_eq!(shape.center_bits(), [0x3fa0_0000, 0xc020_0000, 0xff80_0000]);
    assert_eq!(
        shape.orientation_bits(),
        [0x8000_0000, 0x3e80_0000, 0xbf00_0000, 0x4000_0000]
    );
    let metadata = edb
        .map_common_shape(0x8500_0000, 0, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(shape.binding(), &metadata);
}

#[test]
fn same_descriptor_locator_in_distinct_owners_reads_each_actual_owners_fields() {
    let first = Edb::parse(fixture()).unwrap();
    let mut different = fixture();
    put(&mut different, 0x294, 0x4120_0000);
    let second = Edb::parse(different).unwrap();
    let a = first
        .map_common_shape_geometry(0, 0, budget())
        .unwrap()
        .unwrap();
    let b = second
        .map_common_shape_geometry(0, 0, budget())
        .unwrap()
        .unwrap();
    assert_eq!(a.binding(), b.binding());
    assert_ne!(a.center_bits(), b.center_bits());
    assert_eq!(b.center_bits()[0], 0x4120_0000);
}

#[test]
fn coded_and_literal_maps_resolve_the_same_checked_shape_not_a_caller_locator() {
    let edb = Edb::parse(fixture()).unwrap();
    let coded = edb
        .map_common_shape_geometry(0x8500_0000, 0, budget())
        .unwrap()
        .unwrap();
    let literal = edb
        .map_common_shape_geometry(0x0500_00a5, 0, budget())
        .unwrap()
        .unwrap();
    assert_eq!(coded.center_bits(), literal.center_bits());
    assert_eq!(coded.binding().locator.descriptor_address, 0x280);
    assert_eq!(
        literal.binding().locator.canonical_map_reference,
        0x8500_0000
    );
    assert!(
        edb.map_common_shape_geometry(0x8500_0001, u32::MAX, budget())
            .unwrap()
            .is_none()
    );
}

#[test]
fn metadata_remains_lazy_when_full_geometry_span_is_truncated() {
    let mut data = fixture();
    data.truncate(0x287);
    put(&mut data, 0x14, 0x287);
    put(&mut data, 0x18, 0x287);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.map_common_shape(0, 0, Budget::default())
            .unwrap()
            .is_some()
    );
    assert!(edb.map_common_shape_geometry(0, 0, budget()).is_err());
}

#[test]
fn full_span_exact_end_and_explicit_byte_budget_are_checked() {
    let mut data = fixture();
    data.truncate(0x2b0);
    put(&mut data, 0x14, 0x2b0);
    put(&mut data, 0x18, 0x2b0);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.map_common_shape_geometry(0, 0, budget())
            .unwrap()
            .is_some()
    );
    assert!(
        edb.map_common_shape_geometry(
            0,
            0,
            Budget {
                addressed_bytes: 47,
                ..budget()
            }
        )
        .is_err()
    );
    assert!(
        edb.map_common_shape_geometry(0, 0, Budget::default())
            .is_err()
    );
}

#[test]
fn unsupported_types_and_negative_selectors_fail_before_geometry_payload_reads() {
    let mut data = fixture();
    data.truncate(0x287);
    put(&mut data, 0x14, 0x287);
    put(&mut data, 0x18, 0x287);
    data[0x286] = 3;
    let edb = Edb::parse(data).unwrap();
    let error = edb.map_common_shape_geometry(0, 0, budget()).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<Unsupported>(),
        Some(Unsupported::Type { type_raw: 3, .. })
    ));
    let error = edb
        .map_common_shape_geometry(0, u32::MAX, budget())
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<Unsupported>(),
        Some(Unsupported::Selector {
            raw_selector: u32::MAX
        })
    ));
}
