use super::*;
use std::cell::Cell;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
struct Source {
    disabled: bool,
    triangle_calls: Cell<usize>,
    coordinate_calls: Cell<usize>,
}
impl StaticMeshSource for Source {
    fn triangle(&self, index: usize) -> Result<MeshTriangle> {
        assert_eq!(index, 0, "later triangle should remain unread");
        self.triangle_calls.set(self.triangle_calls.get() + 1);
        Ok(MeshTriangle {
            descriptor: TriangleDescriptor {
                flags_raw: if self.disabled { 0x200 } else { 17 },
                position_indices: [0, 1, 2],
                strip_in_batch: 0,
                triangle_in_strip: 0,
            },
            first_corner_key: 888,
        })
    }
    fn coordinate(&self, position: u16, axis: usize) -> Result<f32> {
        assert!(!self.disabled, "disabled positions are unread");
        self.coordinate_calls.set(self.coordinate_calls.get() + 1);
        Ok([[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]][usize::from(position)][axis])
    }
}
fn source(disabled: bool) -> Source {
    Source {
        disabled,
        triangle_calls: Cell::new(0),
        coordinate_calls: Cell::new(0),
    }
}
fn batch() -> BatchDescriptor {
    BatchDescriptor {
        flags_raw: 0,
        material_raw: 42,
        declared_triangles: 1,
        first_triangle: 0,
        triangle_count: 1,
    }
}
fn boxed(center: [f32; 3]) -> OrientedBox {
    OrientedBox {
        center,
        axes: [[1., 0., 0.], [0., 1., 0.]],
        half_extents: [0.25; 3],
    }
}
fn record(matrix: [f32; 16], flags: u16) -> CollectedRecord {
    CollectedRecord {
        matrix,
        geometry_key: 7,
        optional_context_key: 0,
        owner_key: 0,
        slot: 0,
        flags_raw: flags,
    }
}
#[test]
fn current_forward_matrix_transforms_triangles_and_binds_world_contacts() {
    let mut matrix = IDENTITY;
    matrix[12] = 20.;
    let record = record(matrix, 0);
    let stale = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let shapes = [boxed([20., 0., 0.])];
    let request = MeshRequest::new([20., 0., 0.], 2., &IDENTITY, &shapes).unwrap();
    let mut contacts = ContactBuffer::new(4).unwrap();
    let stats = query_record(
        &source(false),
        &[batch()],
        None,
        RecordQuery {
            record: &record,
            cached_frame: &stale,
            collected_record_key: 99,
        },
        &request,
        &mut contacts,
        |origin, _| {
            assert_eq!(origin, [0.; 4]);
            Ok(Some(true))
        },
    )
    .unwrap();
    assert_eq!((stats.primitive_contacts, stats.appended), (1, 1));
    let contact = &contacts.contacts()[0];
    assert_eq!(
        (
            contact.collected_record_key,
            contact.first_corner_key,
            contact.material_raw
        ),
        (99, 888, 42)
    );
    assert!(contact.witness_b[0] >= 20. && contact.witness_b[0] <= 21.);
    assert!(contact.witness_b[1].abs() < 1e-6);
    assert!((contact.normal[1] + 1.).abs() < 1e-6);
}

#[test]
fn rotated_query_frame_and_translated_mesh_require_noncommuting_query_inverse_times_mesh() {
    let mut mesh_frame = IDENTITY;
    mesh_frame[12] = 20.;
    let record = record(mesh_frame, 0);
    let frame = CollisionFrame::from_record(&mesh_frame, 0).unwrap();
    // Q rotates query +Z onto world +X. Q^-1*M places the mesh origin at
    // query (0,0,20); the reversed M*Q^-1 would leave it at query (20,0,0).
    let world_from_query = [
        0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., 0., 0., 0., 1.,
    ];
    let shapes = [boxed([0., 0., 20.])];
    let request = MeshRequest::new([20., 0., 0.], 2., &world_from_query, &shapes).unwrap();
    let mut contacts = ContactBuffer::new(4).unwrap();
    let stats = query_record(
        &source(false),
        &[batch()],
        None,
        RecordQuery {
            record: &record,
            cached_frame: &frame,
            collected_record_key: 41,
        },
        &request,
        &mut contacts,
        |origin, _| {
            assert_eq!(origin, [0.; 4]);
            Ok(None)
        },
    )
    .unwrap();
    assert_eq!((stats.primitive_contacts, stats.appended), (1, 1));
    let contact = &contacts.contacts()[0];
    assert!((20. ..=21.).contains(&contact.witness_b[0]));
    assert!(contact.witness_b[1].abs() < 1e-6);
    assert!((0. ..=1.).contains(&contact.witness_b[2]));
    assert!((contact.normal[1] + 1.).abs() < 1e-6);
    assert_eq!(contact.collected_record_key, 41);
}
#[test]
fn general_flag_skips_coarse_and_disabled_triangle_skips_positions_and_shapes() {
    let record = record(IDENTITY, 2);
    let frame = CollisionFrame::from_record(&IDENTITY, 2).unwrap();
    let shapes = [boxed([f32::NAN; 3])];
    let request = MeshRequest::new([0.; 3], 1., &IDENTITY, &shapes).unwrap();
    let source = source(true);
    let mut contacts = ContactBuffer::new(2).unwrap();
    let stats = query_record(
        &source,
        &[batch()],
        None,
        RecordQuery {
            record: &record,
            cached_frame: &frame,
            collected_record_key: 1,
        },
        &request,
        &mut contacts,
        |_, _| panic!("coarse bypass"),
    )
    .unwrap();
    assert_eq!(
        (
            stats.triangle_tests,
            stats.triangle_rejections,
            stats.primitive_queries
        ),
        (1, 1, 0)
    );
    assert_eq!(source.coordinate_calls.get(), 0);
}
#[test]
fn compact_cache_rejection_uses_declared_cursor_and_addresses_no_triangles() {
    let record = record(IDENTITY, 0);
    let frame = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let mut excluded = batch();
    excluded.flags_raw = 1;
    excluded.declared_triangles = 999;
    let mut included = batch();
    included.declared_triangles = 7;
    included.triangle_count = 999;
    let cache = StaticBatchCache {
        bounds: vec![batch::BatchBounds::EMPTY],
    };
    let source = source(false);
    let request = MeshRequest::new([0.; 3], 1., &IDENTITY, &[]).unwrap();
    let stats = query_record(
        &source,
        &[excluded, included],
        Some(&cache),
        RecordQuery {
            record: &record,
            cached_frame: &frame,
            collected_record_key: 1,
        },
        &request,
        &mut ContactBuffer::new(2).unwrap(),
        |_, _| Ok(None),
    )
    .unwrap();
    assert_eq!(
        (
            stats.included_batches,
            stats.batch_rejections,
            stats.flag_words_consumed
        ),
        (1, 1, 7)
    );
    assert_eq!(source.triangle_calls.get(), 0);
}
#[test]
fn cube_preparation_precedes_reserved_slot_gate_and_coarse_can_short_circuit_frames() {
    let record = record(IDENTITY, 0);
    let frame = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let source = source(false);
    let request = MeshRequest::new([f32::MAX, 0., 0.], f32::MAX, &IDENTITY, &[]).unwrap();
    assert!(
        query_record(
            &source,
            &[batch()],
            None,
            RecordQuery {
                record: &record,
                cached_frame: &frame,
                collected_record_key: 1
            },
            &request,
            &mut ContactBuffer::new(1).unwrap(),
            |_, _| Ok(None)
        )
        .is_err()
    );
    let mut invalid = record.clone();
    invalid.matrix[0] = f32::NAN;
    let request = MeshRequest::new([0.; 3], 1., &IDENTITY, &[]).unwrap();
    let stats = query_record(
        &source,
        &[batch()],
        None,
        RecordQuery {
            record: &invalid,
            cached_frame: &frame,
            collected_record_key: 1,
        },
        &request,
        &mut ContactBuffer::new(1).unwrap(),
        |_, _| Ok(Some(false)),
    )
    .unwrap();
    assert_eq!(stats.coarse_rejections, 1);
    assert_eq!(source.triangle_calls.get(), 0);
}
#[test]
fn multiple_shapes_can_fill_reserved_slot_then_one_more_hit_ends_the_mesh() {
    let record = record(IDENTITY, 0);
    let frame = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let shapes = [
        boxed([0.; 3]),
        boxed([0.; 3]),
        boxed([0.; 3]),
        boxed([0.; 3]),
    ];
    let request = MeshRequest::new([0.; 3], 2., &IDENTITY, &shapes).unwrap();
    let mut batch = batch();
    batch.declared_triangles = 2;
    batch.triangle_count = 2;
    let mut contacts = ContactBuffer::new(2).unwrap();
    let stats = query_record(
        &source(false),
        &[batch],
        None,
        RecordQuery {
            record: &record,
            cached_frame: &frame,
            collected_record_key: 1,
        },
        &request,
        &mut contacts,
        |_, _| Ok(None),
    )
    .unwrap();
    assert_eq!(
        (
            stats.primitive_queries,
            stats.primitive_contacts,
            stats.appended,
            stats.post_hit_full
        ),
        (3, 3, 2, 1)
    );
    assert_eq!(contacts.contacts().len(), 2);
}
#[test]
fn capacity_zero_and_one_have_distinct_native_query_behavior() {
    let record = record(IDENTITY, 0);
    let frame = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let shapes = [boxed([0.; 3])];
    let request = MeshRequest::new([0.; 3], 2., &IDENTITY, &shapes).unwrap();
    for (capacity, queries, skips, post_full) in [(0, 1, 0, 1), (1, 0, 1, 0)] {
        let mut contacts = ContactBuffer::new(capacity).unwrap();
        let stats = query_record(
            &source(false),
            &[batch()],
            None,
            RecordQuery {
                record: &record,
                cached_frame: &frame,
                collected_record_key: 1,
            },
            &request,
            &mut contacts,
            |_, _| Ok(None),
        )
        .unwrap();
        assert_eq!(
            (
                stats.primitive_queries,
                stats.capacity_mesh_skips,
                stats.post_hit_full
            ),
            (queries, skips, post_full)
        );
        assert!(contacts.contacts().is_empty());
    }
}
#[test]
fn request_frame_always_uses_rigid_helper_and_shapes_stay_in_query_coordinates() {
    let record = record(IDENTITY, 2);
    let frame = CollisionFrame::from_record(&IDENTITY, 2).unwrap();
    let mut world = IDENTITY;
    world[0] = 2.;
    let shapes = [boxed([2., 0., 0.])];
    let request = MeshRequest::new([1., 0., 0.], 2., &world, &shapes).unwrap();
    assert_eq!(request.query_from_world[0], 2.);
    let mut contacts = ContactBuffer::new(2).unwrap();
    let stats = query_record(
        &source(false),
        &[batch()],
        None,
        RecordQuery {
            record: &record,
            cached_frame: &frame,
            collected_record_key: 1,
        },
        &request,
        &mut contacts,
        |_, _| panic!("flag2"),
    )
    .unwrap();
    assert_eq!(stats.appended, 1);
    assert!(contacts.contacts()[0].witness_b[0] > 3.5);
}
#[test]
fn primitive_budget_errors_retain_prior_contacts_and_optional_context_is_not_inferred() {
    let mut record = record(IDENTITY, 0);
    let frame = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let shapes = [boxed([0.; 3]), boxed([0.; 3])];
    let mut request = MeshRequest::new([0.; 3], 2., &IDENTITY, &shapes).unwrap();
    request.primitive_budget = 1;
    let mut contacts = ContactBuffer::new(4).unwrap();
    assert!(
        query_record(
            &source(false),
            &[batch()],
            None,
            RecordQuery {
                record: &record,
                cached_frame: &frame,
                collected_record_key: 1
            },
            &request,
            &mut contacts,
            |_, _| Ok(None)
        )
        .is_err()
    );
    assert_eq!(contacts.contacts().len(), 1);
    record.optional_context_key = 1;
    assert!(
        query_record(
            &source(false),
            &[batch()],
            None,
            RecordQuery {
                record: &record,
                cached_frame: &frame,
                collected_record_key: 1
            },
            &request,
            &mut contacts,
            |_, _| panic!("unresolved context")
        )
        .is_err()
    );
}
