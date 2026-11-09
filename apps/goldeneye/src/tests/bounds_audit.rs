use super::*;
#[test]
fn initial_assembly_binds_ordinary_and_shape_components_and_reports_unsupported() {
    let mut bytes = vec![0; 0x400];
    let put = |bytes: &mut [u8], at: usize, value: u32| {
        bytes[at..at + 4].copy_from_slice(&value.to_be_bytes())
    };
    for (at, value) in [
        (0, 0x47454f4d),
        (4, 0x01000001),
        (8, 263),
        (20, 0x400),
        (24, 0x400),
        (0x50, 0x10000),
        (0x54, 0x7c),
        (0xd0, 0x82000000),
        (0xd8, 0x100),
        (0x100, 0x601),
        (0x140, (0x40 << 8) | 0x41),
        (0x180, 0x80 << 8),
        (0x184, 0xfc << 8),
        (0x200, 0x601),
        (0x280, 1),
        (0x284, 0x7c),
    ] {
        put(&mut bytes, at, value);
    }
    for (at, values) in [
        (0x110, [0f32; 3]),
        (0x120, [1.; 3]),
        (0x210, [-2.; 3]),
        (0x220, [2.; 3]),
        (0x304, [3., 4., 5.]),
    ] {
        for (axis, value) in values.into_iter().enumerate() {
            put(&mut bytes, at + axis * 4, value.to_bits());
        }
    }
    let placement = Placement {
        hash: 0,
        position: [0.; 3],
        rotation: [0.; 3],
        scale: [1.; 3],
        flags: 0,
        engine_flags: 0,
        map_on: 0,
        object_hash: 0,
        light_set: 0,
        group: -1,
        unknown_word: 0,
        model: ge_assets::transform::placement([0.; 3], [0.; 3], [1.; 3]).unwrap(),
        status: None,
    };
    let mut audit = Audit::default();
    audit
        .include(
            &Edb::parse(bytes.clone()).unwrap(),
            std::slice::from_ref(&placement),
        )
        .unwrap();
    assert_eq!(
        (
            audit.ordinary_components_decoded,
            audit.shape_components_decoded,
            audit.shape_point_updates
        ),
        (1, 1, 1)
    );
    assert_eq!(
        (
            audit.assembled_bounds_probed,
            audit.assembled_bounds_pending
        ),
        (1, 0)
    );
    put(&mut bytes, 0x200, 0x999);
    let mut audit = Audit::default();
    audit
        .include(&Edb::parse(bytes).unwrap(), &[placement])
        .unwrap();
    assert_eq!(audit.unsupported_components, 1);
    assert_eq!(
        (
            audit.assembled_bounds_probed,
            audit.assembled_bounds_pending
        ),
        (0, 1)
    );
}
#[test]
fn corner_comparator_handles_rotated_reflected_and_inverted_source_bounds() {
    let mut audit = Audit::default();
    for (minimum, maximum) in [
        ([-1., -2., -3., 0.], [4., 5., 6., 0.]),
        ([4., 5., 6., f32::NAN], [-1., -2., -3., f32::NAN]),
    ] {
        let model =
            ge_assets::transform::placement([100., 20., -30.], [0.3, -1.2, 2.4], [-2., 0., 4.])
                .unwrap();
        audit.probe(Bounds { minimum, maximum }, &model).unwrap();
    }
    assert_eq!(audit.base_bounds_probed, 2);
    assert_eq!(audit.strict_bounds + audit.rounding_bounds, 2);
    let bounds = Bounds {
        minimum: [f32::NAN; 4],
        maximum: [0.; 4],
    };
    assert!(audit.probe(bounds, &[0.; 16]).is_err());
}
