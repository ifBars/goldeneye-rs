use super::*;
fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], at: usize, target: usize) {
    put(data, at, (target as i32 - at as i32) as u32);
}
fn tables(count: u16) -> Vec<Table> {
    vec![
        Table {
            name: "sections",
            count: 0,
            hash_size_raw: 0,
            offset: 0,
        },
        Table {
            name: "references",
            count,
            hash_size_raw: 0,
            offset: 0x300,
        },
        Table {
            name: "entities",
            count: 1,
            hash_size_raw: 1,
            offset: 0x380,
        },
    ]
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x400];
    for (at, kind) in [
        (0x100, 0x601),
        (0x180, 0x603),
        (0x200, 0x608),
        (0x280, 0x607),
    ] {
        put(&mut data, at, kind);
    }
    data
}

#[test]
fn common_metadata_is_lazy_and_retains_ignored_homogeneous_lanes() {
    let mut data = fixture();
    for (offset, value) in [
        (0x10, -1f32),
        (0x14, -2.),
        (0x18, -3.),
        (0x20, 4.),
        (0x24, 5.),
        (0x28, 6.),
        (0x30, 7.),
        (0x34, 8.),
        (0x38, 9.),
        (0x3c, -10.),
    ] {
        put(&mut data, 0x100 + offset, value.to_bits());
    }
    put(&mut data, 0x11c, f32::NAN.to_bits());
    put(&mut data, 0x12c, f32::INFINITY.to_bits());
    half(&mut data, 0x152, 0xfffe);
    let directory = tables(0);
    let view = GeometryView::new(&data, &directory);
    let bounds = view.bounds(0x100).unwrap();
    assert_eq!(bounds.minimum, [-1., -2., -3.]);
    assert_eq!(bounds.maximum, [4., 5., 6.]);
    assert_eq!(
        bounds.packed_w_raw,
        [f32::NAN.to_bits(), f32::INFINITY.to_bits()]
    );
    let sphere = view.sphere(0x100).unwrap();
    assert_eq!(sphere.center, [7., 8., 9.]);
    assert_eq!(sphere.radius, -10.);
    assert_eq!(view.optional_context_index(0x100).unwrap(), 0xfffe);
    put(&mut data, 0x110, f32::NAN.to_bits());
    let view = GeometryView::new(&data, &directory);
    assert!(view.sphere(0x100).is_ok());
    assert!(view.bounds(0x100).is_err());
    put(&mut data, 0x110, 0);
    put(&mut data, 0x130, f32::NAN.to_bits());
    let view = GeometryView::new(&data, &directory);
    assert!(view.bounds(0x100).is_ok());
    assert!(view.sphere(0x100).is_err());
}

#[test]
fn split_metadata_addresses_only_the_requested_relative_pointer() {
    let mut data = fixture();
    put(&mut data, 0x1d4, 3);
    relative(&mut data, 0x1dc, 0x100);
    relative(&mut data, 0x1e0, 0x100);
    put(&mut data, 0x1e4, u32::MAX / 2);
    let directory = tables(0);
    let view = GeometryView::new(&data, &directory);
    assert_eq!(view.split_child_count(0x180).unwrap(), 3);
    assert_eq!(view.split_child(0x180, 0).unwrap(), 0x100);
    assert_eq!(view.split_child(0x180, 1).unwrap(), 0x100);
    assert!(view.split_child(0x180, 2).is_err());
    assert!(view.split_child(0x180, 3).is_err());
    put(&mut data, 0x1dc, 0);
    assert!(
        GeometryView::new(&data, &directory)
            .split_child(0x180, 0)
            .is_err()
    );
    put(&mut data, 0x1d4, 0);
    let short = &data[..0x1d8];
    let view = GeometryView::new(short, &[]);
    assert_eq!(view.split_child_count(0x180).unwrap(), 0);
    assert!(view.split_child(0x180, 0).is_err());
}

#[test]
fn links_use_reference_directory_stride_sixteen_and_serialized_target_not_runtime_word() {
    let mut data = fixture();
    data[0x254] = 3;
    put(&mut data, 0x258, 0);
    put(&mut data, 0x26c, 0x80000001);
    put(&mut data, 0x280, 0x80000009);
    // A different entity directory target must not be consulted for these links.
    put(&mut data, 0x380, 0);
    put(&mut data, 0x388, 0x280);
    put(&mut data, 0x308, 0x100);
    put(&mut data, 0x30c, u32::MAX);
    put(&mut data, 0x318, 0x180);
    put(&mut data, 0x31c, u32::MAX);
    let directory = tables(2);
    let view = GeometryView::new(&data, &directory);
    assert_eq!(view.link_count(0x200).unwrap(), 3);
    assert_eq!(view.link_target(0x200, 0).unwrap(), Some(0x100));
    assert_eq!(view.resolve_reference(0).unwrap(), Some(0x100));
    assert_eq!(view.link_target(0x200, 1).unwrap(), Some(0x180));
    assert_eq!(view.link_target(0x200, 2).unwrap(), None);
    assert!(view.link_target(0x200, 3).is_err());
}

#[test]
fn flags_and_component_selection_keep_masked_targets_lazy() {
    let mut data = fixture();
    put(&mut data, 0x104, 0x12345678);
    put(&mut data, 0x140, 0x7fffff00);
    let directory = tables(0);
    let view = GeometryView::new(&data, &directory);
    assert_eq!(view.flags(0x100).unwrap(), 0x12345678);
    assert_eq!(view.component_mask(0x100).unwrap(), 0);
    assert_eq!(view.component_target(0x100, 6).unwrap(), None);
    put(&mut data, 0x140, ((0x340i32 - 0x140) << 8) as u32 | 0x10);
    put(&mut data, 0x340, ((0x200i32 - 0x340) << 8) as u32 | 0x55);
    let view = GeometryView::new(&data, &directory);
    assert_eq!(view.component_target(0x100, 4).unwrap(), Some(0x200));
    assert_eq!(view.component_target(0x100, 0).unwrap(), None);
    assert!(view.component_target(0x100, 8).is_err());
    assert!(view.component_target(usize::MAX, 0).is_err());
}

#[test]
fn unresolved_links_do_not_address_descriptors_and_external_links_are_errors() {
    let mut data = fixture();
    data[0x254] = 1;
    put(&mut data, 0x258, 9);
    let mut directory = tables(1);
    directory[1].offset = usize::MAX;
    assert_eq!(
        GeometryView::new(&data, &directory)
            .link_target(0x200, 0)
            .unwrap(),
        None
    );
    put(&mut data, 0x258, 0);
    let directory = tables(1);
    put(&mut data, 0x300, 1);
    put(&mut data, 0x308, u32::MAX);
    assert!(
        GeometryView::new(&data, &directory)
            .link_target(0x200, 0)
            .unwrap_err()
            .to_string()
            .contains("external")
    );
    put(&mut data, 0x300, 0);
    assert!(
        GeometryView::new(&data, &directory)
            .link_target(0x200, 0)
            .is_err()
    );
}

#[test]
fn body_type_range_and_short_zero_count_checks_are_explicit() {
    let mut data = fixture();
    let directory = tables(0);
    let view = GeometryView::new(&data, &directory);
    assert!(view.node_type(data.len()).is_err());
    assert!(view.node_type(usize::MAX).is_err());
    assert!(view.split_child_count(0x100).is_err());
    assert!(view.link_count(0x180).is_err());
    assert!(view.sphere(0x3fe).is_err());
    put(&mut data, 0x100, 0x999);
    let view = GeometryView::new(&data, &directory);
    assert_eq!(view.node_type(0x100).unwrap(), 0x999);
    assert!(view.bounds(0x100).is_err());
    data[0x254] = 0;
    let view = GeometryView::new(&data[..0x255], &[]);
    assert_eq!(view.link_count(0x200).unwrap(), 0);
    assert!(view.link_target(0x200, 0).is_err());
}
