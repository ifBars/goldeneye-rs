use super::*;

fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn xyz(bytes: &mut [u8], at: usize, value: [f32; 3]) {
    for (axis, value) in value.into_iter().enumerate() {
        word(bytes, at + axis * 4, value.to_bits());
    }
}
fn relative(bytes: &mut [u8], at: usize, target: usize) {
    word(bytes, at, (target as i32 - at as i32) as u32);
}
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 0x200];
    word(&mut bytes, 0x20, 2);
    relative(&mut bytes, 0x24, 0x80);
    word(&mut bytes, 0x28, 2);
    relative(&mut bytes, 0x2c, 0xc0);
    bytes[0x80] = 1;
    xyz(&mut bytes, 0x84, [1., 2., 3.]);
    xyz(&mut bytes, 0x94, [f32::NAN; 3]);
    bytes[0xc0] = 5;
    for (i, value) in [[-4., 8., 0.], [7., -2., 5.], [2., 3., -6.]]
        .into_iter()
        .enumerate()
    {
        xyz(&mut bytes, 0xd0 + i * 16, value);
        word(&mut bytes, 0xdc + i * 16, f32::NAN.to_bits());
    }
    bytes
}

#[test]
fn aggregate_preserves_first_record_repetition_and_first_polygon_vertex_walk() {
    let mut bytes = fixture();
    // Another possible record/polygon is deliberately invalid and never read.
    xyz(&mut bytes, 0x114, [f32::NAN; 3]);
    let actual = read(&bytes, 0x20).unwrap();
    assert_eq!(actual.minimum, [-4., -2., -6., 0.]);
    assert_eq!(actual.maximum, [7., 8., 5., -0.]);
    assert_eq!(
        (actual.first_point_updates, actual.second_point_updates),
        (4, 6)
    );
    assert_eq!(actual.maximum[3].to_bits(), (-0f32).to_bits());
    // A later first-stream record is not an alternative source of bounds.
    xyz(&mut bytes, 0x94, [-1000., 1000., -1000.]);
    let changed = read(&bytes, 0x20).unwrap();
    assert_eq!(changed.minimum, actual.minimum);
    assert_eq!(changed.maximum, actual.maximum);
}

#[test]
fn empty_and_negative_counts_keep_finite_sentinel_and_unused_targets_unread() {
    let mut bytes = vec![0; 12];
    word(&mut bytes, 0, u32::MAX);
    word(&mut bytes, 4, 0x7fff_ffff);
    word(&mut bytes, 8, 0x8000_0000);
    let actual = read(&bytes, 0).unwrap();
    assert_eq!(actual.minimum, [1e6, 1e6, 1e6, 0.]);
    assert_eq!(actual.maximum, [-1e6, -1e6, -1e6, -0.]);
    assert_eq!(actual.maximum[3].to_bits(), (-0f32).to_bits());
    assert_eq!(
        (actual.first_count_raw, actual.second_count_raw),
        (u32::MAX, 0x8000_0000)
    );
    assert_eq!(actual.first_point_updates + actual.second_point_updates, 0);
    // The second pointer itself is not required when its signed count is zero.
    word(&mut bytes, 0, 0);
    word(&mut bytes, 8, 0);
    assert!(read(&bytes, 0).is_ok());
}

#[test]
fn short_polygons_read_no_vertex_payload_and_counts_are_not_repaired() {
    let mut bytes = vec![0; 17];
    word(&mut bytes, 8, 3);
    relative(&mut bytes, 12, 16);
    for count in [0, 1, 2] {
        bytes[16] = count;
        let actual = read(&bytes, 0).unwrap();
        assert_eq!(actual.second_point_updates, 0);
        assert_eq!(actual.minimum[0], 1e6);
    }
    bytes[16] = 3;
    assert!(read(&bytes, 0).is_err());
}

#[test]
fn backward_relative_stream_and_extreme_sentinel_clamping_are_preserved() {
    let mut bytes = vec![0; 0x60];
    bytes[0] = 0;
    xyz(&mut bytes, 4, [2e6, -2e6, 4.]);
    word(&mut bytes, 0x40, 1);
    relative(&mut bytes, 0x44, 0);
    let actual = read(&bytes, 0x40).unwrap();
    assert_eq!(actual.minimum, [1e6, -2e6, 4., 0.]);
    assert_eq!(actual.maximum, [2e6, -1e6, 4., -0.]);
}

#[test]
fn used_nonfinite_null_truncated_and_excessive_work_are_errors() {
    let mut bytes = fixture();
    xyz(&mut bytes, 0x84, [f32::NAN, 0., 0.]);
    assert!(read(&bytes, 0x20).is_err());
    let mut bytes = fixture();
    word(&mut bytes, 0x24, 0);
    assert!(read(&bytes, 0x20).is_err());
    let bytes = fixture();
    assert!(read(&bytes[..0x8c], 0x20).is_err());
    let mut bytes = fixture();
    word(&mut bytes, 0x20, 1_000_001);
    assert!(read(&bytes, 0x20).is_err());
    word(&mut bytes, 0x20, 1_000_000);
    assert!(read(&bytes, 0x20).is_err());
}

#[test]
fn point_difference_overflow_is_not_treated_as_successful_bounds() {
    let mut bytes = fixture();
    xyz(&mut bytes, 0x84, [f32::MAX, 0., 0.]);
    xyz(&mut bytes, 0xd0, [-f32::MAX, 0., 0.]);
    assert!(read(&bytes, 0x20).is_err());
}

#[test]
fn authored_point_sets_agree_with_independent_f64_extrema() {
    let mut word_state = 813u32;
    let mut coordinate = || {
        word_state = word_state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((word_state >> 16) as i32 - 32768) as f32 / 32.
    };
    for _ in 0..100 {
        let mut bytes = vec![0; 0x220];
        word(&mut bytes, 0x20, 3);
        relative(&mut bytes, 0x24, 0x80);
        word(&mut bytes, 0x28, 2);
        relative(&mut bytes, 0x2c, 0xc0);
        bytes[0x80] = 7;
        bytes[0xc0] = 18;
        let first = std::array::from_fn(|_| coordinate());
        xyz(&mut bytes, 0x84, first);
        let mut points = vec![first];
        for vertex in 0..16 {
            let point = std::array::from_fn(|_| coordinate());
            xyz(&mut bytes, 0xd0 + vertex * 16, point);
            points.push(point);
        }
        let actual = read(&bytes, 0x20).unwrap();
        for axis in 0..3 {
            let minimum = points
                .iter()
                .map(|p| f64::from(p[axis]))
                .fold(1e6, f64::min);
            let maximum = points
                .iter()
                .map(|p| f64::from(p[axis]))
                .fold(-1e6, f64::max);
            assert_eq!(f64::from(actual.minimum[axis]), minimum);
            assert_eq!(f64::from(actual.maximum[axis]), maximum);
        }
        assert_eq!(
            (actual.first_point_updates, actual.second_point_updates),
            (24, 32)
        );
    }
}
