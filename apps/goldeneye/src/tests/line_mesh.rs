use super::*;
use ge_core::collision::batch::CacheDisposition;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(bytes: &mut [u8], at: usize, target: usize) {
    put(bytes, at, (target as i32 - at as i32) as u32);
}
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 0x900];
    for (at, value) in [
        (0, 0x47454f4d),
        (4, 0x01000001),
        (8, 263),
        (20, 0x900),
        (24, 0x900),
        (0x50, 0x10000),
        (0x54, 0x7c),
        (0xd0, 0x82000000),
        (0xd8, 0x100),
        (0x100, 0x601),
        (0x1a4, 3),
        (0x1a8, 3),
        (0x1b0, 0x203),
    ] {
        put(&mut bytes, at, value);
    }
    for axis in 0..3 {
        put(&mut bytes, 0x110 + axis * 4, (-10f32).to_bits());
        put(&mut bytes, 0x120 + axis * 4, 10f32.to_bits());
    }
    relative(&mut bytes, 0x158, 0x200);
    relative(&mut bytes, 0x15c, 0x400);
    relative(&mut bytes, 0x168, 0x500);
    for batch in 0..3usize {
        let base = 0x200 + batch * 0x3c;
        half(&mut bytes, base, 1);
        half(&mut bytes, base + 2, (7 + batch * 2) as u16);
        put(&mut bytes, base + 8, 28);
        half(&mut bytes, base + 0x20, 0x98);
        half(&mut bytes, base + 0x22, 3);
        for corner in 0..3 {
            half(&mut bytes, base + 0x24 + corner * 8, corner as u16);
        }
    }
    for (index, point) in [[0f32, 0., 0.], [1., 0., 0.], [0., 0., 1.]]
        .into_iter()
        .enumerate()
    {
        for (axis, value) in point.into_iter().enumerate() {
            put(&mut bytes, 0x400 + index * 16 + axis * 4, value.to_bits());
        }
    }
    for (index, flags) in [0x200, 2, 4].into_iter().enumerate() {
        half(&mut bytes, 0x500 + index * 2, flags);
    }
    bytes
}
fn request(maximum: &mut f32) -> MeshQuery<'_> {
    MeshQuery {
        origin: [0.2, 1., 0.2],
        delta: [0., -2., 0.],
        local_bounds: StoredBounds {
            minimum: [0.2, -1., 0.2],
            maximum: [0.2, 1., 0.2],
        },
        maximum_parameter: maximum,
        required_any: 0,
        excluded_any: 0,
        optional_context_key: 0,
        dispatch_flags: 0,
        stop_first: false,
    }
}

#[test]
fn resident_planes_include_disabled_triangles_and_last_equal_hit_replaces_provenance() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut meshes = Meshes::default();
    let mut maximum = 1.;
    let probe = meshes
        .query(&edb, 0x100, &mut request(&mut maximum), || Ok(None))
        .unwrap();
    assert_eq!(meshes.resident_count(), 1);
    assert!(probe.planes_generated);
    assert_eq!(probe.planes_count, 3);
    assert_eq!(
        probe.cache_preparation.disposition,
        CacheDisposition::Generated
    );
    assert_eq!(
        (
            probe.batches_tested,
            probe.batches_rejected,
            probe.triangles_tested
        ),
        (3, 1, 2)
    );
    assert_eq!(
        (probe.skipped_flag_words, probe.skipped_plane_records),
        (1, 1)
    );
    assert_eq!(
        (probe.flag_words_advanced, probe.plane_records_advanced),
        (3, 3)
    );
    let hit = probe.hit.unwrap();
    assert_eq!(hit.parameter, 0.5);
    assert_eq!(maximum, 0.5);
    assert_eq!(
        (
            hit.geometry_key,
            hit.flag_index,
            hit.batch_index,
            hit.triangle_flags_raw,
            hit.material_raw
        ),
        (0x100, 2, 2, 4, 11)
    );
    assert_eq!(hit.first_corner_key, 0x29c);
    assert_eq!(hit.local_plane, [0., -1., 0., 0.]);
    maximum = 1.;
    let probe = meshes
        .query(&edb, 0x100, &mut request(&mut maximum), || Ok(None))
        .unwrap();
    assert!(!probe.planes_generated);
    assert_eq!(probe.planes_count, 3);
    assert_eq!(
        probe.cache_preparation.disposition,
        CacheDisposition::Reused
    );
}

#[test]
fn stop_first_keeps_first_accepted_provenance_and_does_not_advance_its_cursors() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut meshes = Meshes::default();
    let mut maximum = 1.;
    let mut query = request(&mut maximum);
    query.stop_first = true;
    let probe = meshes
        .query(&edb, 0x100, &mut query, || Ok(Some(true)))
        .unwrap();
    assert!(probe.coarse_tested);
    assert_eq!(probe.triangles_tested, 1);
    assert_eq!(
        (probe.flag_words_advanced, probe.plane_records_advanced),
        (1, 1)
    );
    let hit = probe.hit.unwrap();
    assert_eq!(
        (hit.flag_index, hit.batch_index, hit.material_raw),
        (1, 1, 9)
    );
}

#[test]
fn preparation_precedes_gates_but_disabled_xyz_plane_reads_wait_until_after_them() {
    let mut bytes = fixture();
    half(&mut bytes, 0x224, u16::MAX);
    let edb = Edb::parse(bytes).unwrap();
    let mut meshes = Meshes::default();
    let mut maximum = 1.;
    let mut query = request(&mut maximum);
    query.local_bounds = StoredBounds {
        minimum: [20.; 3],
        maximum: [30.; 3],
    };
    let probe = meshes
        .query(&edb, 0x100, &mut query, || {
            panic!("bounds rejection must keep coarse unread")
        })
        .unwrap();
    assert!(probe.bounds_rejected);
    assert!(!probe.planes_generated);
    assert_eq!(probe.planes_count, 0);
    assert_eq!(
        probe.cache_preparation.disposition,
        CacheDisposition::Generated
    );
    let probe = meshes
        .query(&edb, 0x100, &mut request(&mut maximum), || Ok(Some(false)))
        .unwrap();
    assert!(probe.coarse_rejected);
    assert!(!probe.planes_generated);
    assert_eq!(
        probe.cache_preparation.disposition,
        CacheDisposition::Reused
    );
    let mut query = request(&mut maximum);
    query.dispatch_flags = 1;
    assert!(
        meshes
            .query(&edb, 0x100, &mut query, || panic!("flag1 bypasses coarse"))
            .is_err()
    );
    assert_eq!(maximum, 1.);
}

#[test]
fn excluded_batches_consume_no_stream_cursors_and_static_context_errors_are_explicit() {
    let mut bytes = fixture();
    half(&mut bytes, 0x204, 0x100);
    bytes[0x220..0x23c].fill(0xff);
    let edb = Edb::parse(bytes).unwrap();
    let mut meshes = Meshes::default();
    let mut maximum = 1.;
    let probe = meshes
        .query(&edb, 0x100, &mut request(&mut maximum), || Ok(None))
        .unwrap();
    assert_eq!(probe.planes_count, 2);
    assert_eq!(probe.batches_tested, 2);
    assert_eq!(
        (probe.skipped_flag_words, probe.flag_words_advanced),
        (1, 2)
    );
    assert_eq!(probe.hit.unwrap().flag_index, 1);
    let mut fresh = Meshes::default();
    let mut query = request(&mut maximum);
    query.optional_context_key = 42;
    let error = fresh
        .query(&edb, 0x100, &mut query, || {
            panic!("unsupported context precedes coarse")
        })
        .unwrap_err();
    assert!(error.to_string().contains("dynamic position context"));
    assert_eq!(fresh.resident_count(), 0);
}
