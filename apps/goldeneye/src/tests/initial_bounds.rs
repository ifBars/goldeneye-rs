use super::*;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn packed_pointer(bytes: &mut [u8], at: usize, target: usize, flags: u8) {
    put(
        bytes,
        at,
        ((target as i32 - at as i32) << 8) as u32 | u32::from(flags),
    );
}
fn spatial(bytes: &mut [u8], at: usize, minimum: [f32; 3], maximum: [f32; 3]) {
    put(bytes, at, 0x601);
    for (side, xyz) in [minimum, maximum].into_iter().enumerate() {
        for (axis, value) in xyz.into_iter().enumerate() {
            put(bytes, at + 0x10 + side * 16 + axis * 4, value.to_bits());
        }
        put(bytes, at + 0x1c + side * 16, f32::NAN.to_bits());
    }
}
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 0x500];
    for (at, value) in [
        (0, 0x47454f4d),
        (4, 0x01000001),
        (8, 263),
        (20, 0x500),
        (24, 0x500),
        (0x50, 0x10000),
        (0x54, 0x7c),
        (0xd0, 0x82000000),
        (0xd8, 0x100),
    ] {
        put(&mut bytes, at, value);
    }
    spatial(&mut bytes, 0x100, [-1., -2., -3.], [4., 5., 6.]);
    spatial(&mut bytes, 0x240, [-10.; 3], [10.; 3]);
    bytes
}
fn input() -> Placement {
    Placement {
        hash: 1,
        position: [0.; 3],
        flags: 0,
        rotation: [0.; 3],
        scale: [1.; 3],
        engine_flags: 0,
        map_on: 0,
        object_hash: 0x82000000,
        light_set: 0,
        group: -1,
        unknown_word: 0,
        model: [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ],
        status: None,
    }
}

#[test]
fn unresolved_reference_uses_sentinel_without_matrix_or_spatial_header_reads() {
    let mut bytes = fixture();
    put(&mut bytes, 0x100, 0x999);
    put(&mut bytes, 0x110, f32::NAN.to_bits());
    put(&mut bytes, 0x140, 0xffff_ffff);
    let edb = Edb::parse(bytes).unwrap();
    let mut p = input();
    p.object_hash = 0x82000001;
    p.model = [f32::NAN; 16];
    let actual = placement(&edb, &p).unwrap();
    assert_eq!(actual.minimum, Bounds::MISSING_ENTITY.minimum);
    assert_eq!(actual.maximum, Bounds::MISSING_ENTITY.maximum);
    assert_eq!(actual.maximum[3].to_bits(), (-0f32).to_bits());
}

#[test]
fn ordinary_components_are_lazy_and_unused_mask_slots_do_not_follow_bad_data() {
    let mut bytes = fixture();
    packed_pointer(&mut bytes, 0x140, 0x200, 0xa1);
    packed_pointer(&mut bytes, 0x200, 0x240, 0xa5);
    // Nested component links and unused slots five/seven are not expanded here.
    put(&mut bytes, 0x280, 0x7fff_ffff);
    put(&mut bytes, 0x204, 0x7fff_ffff);
    let edb = Edb::parse(bytes).unwrap();
    let mut p = input();
    p.model[0] = 2.;
    p.model[12] = 10.;
    let actual = placement(&edb, &p).unwrap();
    assert_eq!(actual.minimum, [-10., -10., -10., 0.]);
    assert_eq!(actual.maximum, [30., 10., 10., 0.]);
}

#[test]
fn addressed_unknown_ordinary_component_errors_before_special_shape_aggregation() {
    let mut bytes = fixture();
    packed_pointer(&mut bytes, 0x140, 0x200, 0x41);
    packed_pointer(&mut bytes, 0x200, 0x240, 0);
    packed_pointer(&mut bytes, 0x204, 0x300, 0);
    put(&mut bytes, 0x240, 0x999);
    put(&mut bytes, 0x300, 1); // Invalid shape stream would fail if eagerly read.
    let edb = Edb::parse(bytes).unwrap();
    let error = placement(&edb, &input()).unwrap_err();
    assert!(format!("{error:#}").contains("unsupported static spatial header type"));
}

#[test]
fn empty_slot_six_aggregate_is_transformed_and_unioned_instead_of_skipped() {
    let mut bytes = fixture();
    packed_pointer(&mut bytes, 0x140, 0x200, 0x40);
    packed_pointer(&mut bytes, 0x200, 0x300, 0);
    let edb = Edb::parse(bytes).unwrap();
    let actual = placement(&edb, &input()).unwrap();
    assert_eq!(actual.minimum, [-1_000_000., -1_000_000., -1_000_000., 0.]);
    assert_eq!(actual.maximum, [1_000_000., 1_000_000., 1_000_000., 0.]);
}
