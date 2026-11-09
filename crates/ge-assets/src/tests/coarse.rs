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
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x200];
    put(&mut data, 0x20, 0x601);
    relative(&mut data, 0x64, 0x80);
    relative(&mut data, 0x80, 0x100);
    half(&mut data, 0x84, 2);
    half(&mut data, 0x86, 1);
    half(&mut data, 0x88, 0x18);
    half(&mut data, 0x8a, 0x26);
    half(&mut data, 0xa0, 0);
    half(&mut data, 0xa2, 4);
    half(&mut data, 0xb0, 0);
    half(&mut data, 0xb2, 6);
    for (at, values) in [
        (0x110, [1f32, 2., 3., 4.]),
        (0x120, [5., 6., 7., 8.]),
        (0x140, [-1., -2., -3., f32::NAN]),
        (0x150, [4., 5., 6., f32::NAN]),
    ] {
        for (lane, value) in values.into_iter().enumerate() {
            put(&mut data, at + lane * 4, value.to_bits());
        }
    }
    data
}
#[test]
fn optional_header_and_lazy_pool_reads_preserve_absence_and_bad_pool_metadata() {
    let mut data = fixture();
    put(&mut data, 0x64, 0);
    assert!(read_at(&data, 0x20).unwrap().is_none());
    relative(&mut data, 0x64, 0x80);
    put(&mut data, 0x80, 0);
    let view = read_at(&data, 0x20).unwrap().unwrap();
    assert!(view.pool.is_none());
    assert_eq!(view.node_count(view.root).unwrap(), 2);
    assert!(view.sphere(1, 0).is_err());
    assert!(view.root_bounds().is_err());
    put(&mut data, 0x64, 0x7fff_ffff);
    assert!(read_at(&data, 0x20).is_err());
    put(&mut data, 0x20, 0x607);
    assert!(read_at(&data, 0x20).is_err());
}
#[test]
fn spheres_and_descendant_bounds_use_sixteen_byte_pool_indices_and_padded_aabbs() {
    let data = fixture();
    let view = read_at(&data, 0x20).unwrap().unwrap();
    assert_eq!((view.root, view.pool), (0x84, Some(0x100)));
    assert_eq!(view.pool_index(view.root).unwrap(), 1);
    assert_eq!(view.sphere(1, 0).unwrap(), [1., 2., 3., 4.]);
    assert_eq!(view.sphere(1, 1).unwrap(), [5., 6., 7., 8.]);
    assert_eq!(view.child_target(view.root, 0).unwrap(), 0xa0);
    assert_eq!(view.child_target(view.root, 1).unwrap(), 0xb0);
    let bounds = view.leaf_bounds(4).unwrap();
    assert_eq!(bounds.minimum, [-1., -2., -3.]);
    assert_eq!(bounds.maximum, [4., 5., 6.]);
}
#[test]
fn root_bounds_ignore_stored_index_and_w_lanes_and_preserve_raw_xyz_bits() {
    let mut data = fixture();
    half(&mut data, 0x84, 0);
    half(&mut data, 0x86, u16::MAX);
    for (at, values) in [(0x100, [-1f32, f32::NAN, -3.]), (0x110, [4., 5., 6.])] {
        for (lane, value) in values.into_iter().enumerate() {
            put(&mut data, at + lane * 4, value.to_bits());
        }
    }
    put(&mut data, 0x10c, f32::NAN.to_bits());
    // Last maximum W is absent, not merely finite; it must remain unread.
    let view = read_at(&data[..0x11c], 0x20).unwrap().unwrap();
    let bounds = view.root_bounds().unwrap();
    assert_eq!(bounds.minimum[0], -1.);
    assert!(bounds.minimum[1].is_nan());
    assert_eq!(bounds.maximum, [4., 5., 6.]);
}
#[test]
fn child_offsets_are_signed_relative_to_each_halfword_and_zero_is_not_absence() {
    let mut data = fixture();
    half(&mut data, 0x88, (-4i16) as u16);
    half(&mut data, 0x8a, 0);
    let view = read_at(&data, 0x20).unwrap().unwrap();
    assert_eq!(view.child_target(view.root, 0).unwrap(), 0x84);
    assert_eq!(view.child_target(view.root, 1).unwrap(), 0x8a);
    half(&mut data, 0x88, i16::MIN as u16);
    let view = read_at(&data, 0x20).unwrap().unwrap();
    assert!(view.child_target(view.root, 0).is_err());
    assert!(view.sphere(u16::MAX, u16::MAX).is_err());
}

#[test]
fn geometry_body_views_share_split_header_without_inventing_root_descriptors() {
    for tag in [0x601, 0x603, 0x608] {
        let mut data = fixture();
        put(&mut data, 0x20, tag);
        let view = read_geometry_at(&data, 0x20).unwrap().unwrap();
        assert_eq!((view.root, view.pool), (0x84, Some(0x100)));
        assert_eq!(view.sphere(1, 0).unwrap(), [1., 2., 3., 4.]);
        assert_eq!(view.child_target(view.root, 1).unwrap(), 0xb0);
        assert_eq!(view.leaf_bounds(4).unwrap().minimum, [-1., -2., -3.]);
        if tag != 0x601 {
            assert!(read_at(&data, 0x20).is_err());
        }
        put(&mut data, 0x64, 0);
        assert!(read_geometry_at(&data, 0x20).unwrap().is_none());
    }
    let mut data = fixture();
    put(&mut data, 0x20, 0x607);
    assert!(read_geometry_at(&data, 0x20).is_err());
}

#[test]
fn split_geometry_pool_remains_lazy_and_addressed_bad_headers_fail() {
    let mut data = fixture();
    put(&mut data, 0x20, 0x603);
    put(&mut data, 0x80, 0);
    let view = read_geometry_at(&data, 0x20).unwrap().unwrap();
    assert_eq!(view.node_count(view.root).unwrap(), 2);
    assert!(view.sphere(1, 0).is_err());
    put(&mut data, 0x64, 0x7fff_ffff);
    assert!(read_geometry_at(&data, 0x20).is_err());
    // A four-byte header suffices: the root and pool are addressed separately.
    relative(&mut data, 0x64, 0x80);
    put(&mut data, 0x80, 0);
    let view = read_geometry_at(&data[..0x84], 0x20).unwrap().unwrap();
    assert_eq!(view.root, 0x84);
    assert!(view.node_count(view.root).is_err());
}
