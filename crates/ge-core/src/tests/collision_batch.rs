use super::*;
use std::cell::RefCell;
fn batch(flags_raw: u16, first_triangle: usize, triangle_count: u16) -> BatchDescriptor {
    BatchDescriptor {
        flags_raw,
        material_raw: 7,
        declared_triangles: triangle_count,
        first_triangle,
        triangle_count: usize::from(triangle_count),
    }
}
fn triangle(flags_raw: u16, indices: [u16; 3], strip: usize, ordinal: usize) -> TriangleDescriptor {
    TriangleDescriptor {
        flags_raw,
        position_indices: indices,
        strip_in_batch: strip,
        triangle_in_strip: ordinal,
    }
}

#[test]
fn excluded_batches_consume_no_cache_slots_and_empty_included_batches_keep_sentinel() {
    let excluded = BatchDescriptor {
        flags_raw: 0x100,
        declared_triangles: u16::MAX,
        first_triangle: usize::MAX,
        triangle_count: usize::MAX,
        ..batch(0, 0, 0)
    };
    let mut format = 0x203;
    let mut cache = None;
    let stats = prepare_static_cache(
        &mut format,
        &mut cache,
        &[excluded, batch(0, usize::MAX, 0)],
        |_| panic!("no triangles"),
        |_, _| panic!("no positions"),
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.included_batches, 1);
    assert_eq!(stats.triangle_visits, 0);
    let cache = cache.unwrap();
    assert_eq!(cache.bounds.len(), 1);
    assert_eq!(cache.bounds[0].minimum, [999999.; 3]);
    assert_eq!(cache.bounds[0].maximum, [-999999.; 3]);
    assert_eq!(format, 0x203);
    let mut cache = None;
    let stats = prepare_static_cache(
        &mut format,
        &mut cache,
        &[excluded],
        |_| panic!("excluded"),
        |_, _| panic!("excluded"),
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::NoIncludedBatches);
    assert_eq!(format, 0x20b);
    assert!(cache.is_none());
}

#[test]
fn cache_tracks_sliding_strip_corners_and_reloads_them_after_each_strip() {
    let triangles = [
        triangle(0, [0, 1, 2], 0, 0),
        triangle(0x200, [1, 2, 9], 0, 1),
        triangle(0, [1, 2, 3], 1, 0),
    ];
    let points = [[-1., 2., 3.], [4., -2., 0.], [0., 8., -3.], [7., 1., 5.]];
    let reads = RefCell::new(Vec::new());
    let mut format = 0x203;
    let mut cache = None;
    let stats = prepare_static_cache(
        &mut format,
        &mut cache,
        &[batch(0, 0, 3)],
        |index| Ok(triangles[index]),
        |position, axis| {
            reads.borrow_mut().push((position, axis));
            Ok(points
                .get(usize::from(position))
                .expect("disabled corner must stay unread")[axis])
        },
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(
        (
            stats.triangle_visits,
            stats.position_visits,
            stats.coordinate_reads
        ),
        (3, 6, 36)
    );
    let bound = cache.unwrap().bounds[0];
    assert_eq!(bound.minimum, [-1., -2., -3.]);
    assert_eq!(bound.maximum, [7., 8., 5.]);
    assert_eq!(
        &reads.borrow()[..6],
        &[(0, 0), (0, 0), (0, 1), (0, 1), (0, 2), (0, 2)]
    );
    assert_eq!(
        reads
            .borrow()
            .iter()
            .filter(|&&(position, _)| position == 1)
            .count(),
        12
    );
}

#[test]
fn disabled_middle_triangle_still_advances_the_strip_visit_mask() {
    let triangles = [
        triangle(0, [0, 1, 2], 0, 0),
        triangle(0x200, [1, 2, 3], 0, 1),
        triangle(0, [2, 3, 4], 0, 2),
    ];
    let visited = RefCell::new(Vec::new());
    let mut format = 0x203;
    let mut cache = None;
    let stats = prepare_static_cache(
        &mut format,
        &mut cache,
        &[batch(0, 0, 3)],
        |index| Ok(triangles[index]),
        |position, axis| {
            if axis == 0 {
                visited.borrow_mut().push(position);
            }
            Ok(f32::from(position))
        },
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.position_visits, 5);
    assert_eq!(*visited.borrow(), [0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);
}

#[test]
fn cache_format_skip_and_existing_reuse_are_lazy_and_do_not_clear_old_cache() {
    let original = StaticBatchCache {
        bounds: vec![BatchBounds {
            minimum: [f32::NAN; 3],
            maximum: [f32::NAN; 3],
        }],
    };
    let mut cache = Some(original);
    let mut format = 0x20b;
    let no_work = CacheLimits {
        triangle_visits: 0,
        coordinate_reads: 0,
    };
    let stats = prepare_static_cache(
        &mut format,
        &mut cache,
        &[],
        |_| panic!("skipped"),
        |_, _| panic!("skipped"),
        no_work,
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::SkippedFormat);
    assert_eq!(stats.included_batches, 0);
    assert!(cache.as_ref().unwrap().bounds[0].minimum[0].is_nan());
    format = 0x203;
    let stats = prepare_static_cache(
        &mut format,
        &mut cache,
        &[batch(0, usize::MAX, 1)],
        |_| panic!("reused"),
        |_, _| panic!("reused"),
        no_work,
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::Reused);
    assert_eq!(stats.included_batches, 1);
    assert_eq!(format, 0x203);
}

#[test]
fn preparation_failures_leave_format_and_cache_unpublished() {
    let mut format = 0x203;
    let mut cache = None;
    assert!(
        prepare_static_cache(
            &mut format,
            &mut cache,
            &[batch(0, 0, 1)],
            |_| Ok(triangle(0, [0, 1, 2], 0, 0)),
            |_, _| Ok(f32::NAN),
            CacheLimits::default()
        )
        .is_err()
    );
    assert_eq!(format, 0x203);
    assert!(cache.is_none());
    assert!(
        prepare_static_cache(
            &mut format,
            &mut cache,
            &[batch(0, 0, 1)],
            |_| Ok(triangle(0, [0, 1, 2], 0, 0)),
            |_, _| Ok(0.),
            CacheLimits {
                triangle_visits: 1,
                coordinate_reads: 5
            }
        )
        .is_err()
    );
    assert_eq!(format, 0x203);
    assert!(cache.is_none());
    let inconsistent = BatchDescriptor {
        triangle_count: 2,
        ..batch(0, 0, 1)
    };
    assert!(
        prepare_static_cache(
            &mut format,
            &mut cache,
            &[inconsistent],
            |_| panic!("invalid metadata"),
            |_, _| panic!("invalid metadata"),
            CacheLimits::default()
        )
        .is_err()
    );
    let order = [triangle(0, [0, 1, 2], 0, 0), triangle(0, [3, 4, 5], 0, 1)];
    assert!(
        prepare_static_cache(
            &mut format,
            &mut cache,
            &[batch(0, 0, 2)],
            |i| Ok(order[i]),
            |_, _| Ok(0.),
            CacheLimits::default()
        )
        .is_err()
    );
    assert!(cache.is_none());
}

#[test]
fn strict_extrema_keep_zero_ties_then_zero_margin_finalizes_maximum_to_positive_zero() {
    let points = [[-0.; 3], [0.; 3], [-0.; 3]];
    let mut cache = None;
    let mut format = 0x203;
    prepare_static_cache(
        &mut format,
        &mut cache,
        &[batch(0, 0, 1)],
        |_| Ok(triangle(0, [0, 1, 2], 0, 0)),
        |p, a| Ok(points[usize::from(p)][a]),
        CacheLimits::default(),
    )
    .unwrap();
    let bounds = cache.unwrap().bounds[0];
    assert!(
        bounds
            .minimum
            .into_iter()
            .all(|value| value.to_bits() == (-0f32).to_bits())
    );
    assert!(
        bounds
            .maximum
            .into_iter()
            .all(|value| value.to_bits() == 0f32.to_bits())
    );
    let mut cache = None;
    prepare_static_cache(
        &mut format,
        &mut cache,
        &[batch(0, 0, 1)],
        |_| Ok(triangle(0, [0, 1, 2], 0, 0)),
        |_, _| Ok(2e6),
        CacheLimits::default(),
    )
    .unwrap();
    let bounds = cache.unwrap().bounds[0];
    assert_eq!(bounds.minimum, [999999.; 3]);
    assert!(!cache_passes(&bounds, &QueryCube::new([2e6; 3], 10.).unwrap()).unwrap());
}

#[test]
fn cache_pruning_is_closed_and_checks_minimum_xyz_before_maximum_xyz() {
    let cube = QueryCube::new([0.; 3], 1.).unwrap();
    let touch = BatchBounds {
        minimum: [1., -1., -1.],
        maximum: [2., 1., 1.],
    };
    assert!(cache_passes(&touch, &cube).unwrap());
    let early = BatchBounds {
        minimum: [2., f32::NAN, f32::NAN],
        maximum: [f32::NAN; 3],
    };
    assert!(!cache_passes(&early, &cube).unwrap());
    let sentinel = BatchBounds {
        minimum: [999999., f32::NAN, f32::NAN],
        maximum: [f32::NAN; 3],
    };
    let invalid_cube = QueryCube {
        minimum: [f32::NAN; 3],
        maximum: [f32::NAN; 3],
    };
    assert!(!cache_passes(&sentinel, &invalid_cube).unwrap());
    let used = BatchBounds {
        minimum: [0., f32::NAN, 0.],
        maximum: [1.; 3],
    };
    assert!(cache_passes(&used, &cube).is_err());
    assert!(QueryCube::new([f32::MAX; 3], f32::MAX).is_err());
    let shrunk = QueryCube::new([0.; 3], -1.).unwrap();
    assert_eq!((shrunk.minimum, shrunk.maximum), ([1.; 3], [-1.; 3]));
}

#[test]
fn triangle_flags_and_axis_rejection_keep_later_coordinates_unread() {
    let cube = QueryCube::new([0.; 3], 1.).unwrap();
    assert!(
        !triangle_passes(&triangle(0x200, [u16::MAX; 3], 0, 0), &cube, |_, _| panic!(
            "disabled"
        ))
        .unwrap()
    );
    let reads = RefCell::new(Vec::new());
    let passes = triangle_passes(&triangle(0, [4, 8, 12], 0, 1), &cube, |p, a| {
        reads.borrow_mut().push((p, a));
        assert_eq!(a, 0, "later axes must stay unread");
        Ok(2.)
    })
    .unwrap();
    assert!(!passes);
    assert_eq!(*reads.borrow(), [(8, 0), (4, 0), (12, 0)]);
    let reads = RefCell::new(Vec::new());
    assert!(
        !triangle_passes(&triangle(0, [0, 1, 2], 0, 0), &cube, |p, a| {
            reads.borrow_mut().push((p, a));
            assert!(a < 2, "Z must remain unread");
            Ok(if a == 0 { 0. } else { 2. })
        })
        .unwrap()
    );
    assert_eq!(reads.borrow().len(), 6);
    assert!(
        triangle_passes(&triangle(0, [0, 1, 2], 0, 1), &cube, |_, a| Ok(if a == 0 {
            1.
        } else {
            0.
        }))
        .unwrap()
    );
}

#[test]
fn skipped_flag_words_use_declared_count_and_never_actual_strip_count() {
    let input = BatchDescriptor {
        triangle_count: 999,
        ..batch(0, 0, 2)
    };
    assert_eq!(skipped_flag_words(&input), 2);
    assert_eq!(
        skipped_flag_words(&BatchDescriptor {
            flags_raw: 1,
            ..input
        }),
        0
    );
    assert_eq!(
        skipped_flag_words(&BatchDescriptor {
            flags_raw: 0x100,
            ..input
        }),
        0
    );
}

#[test]
fn authored_triangle_pruning_agrees_with_independent_f64_interval_geometry() {
    let mut word = 345u32;
    let mut coordinate = || {
        word = word.wrapping_mul(1664525).wrapping_add(1013904223);
        ((word >> 24) as i32 - 128) as f32 / 8.
    };
    let descriptor = triangle(0, [0, 1, 2], 0, 0);
    for _ in 0..1000 {
        let points: [[f32; 3]; 3] = std::array::from_fn(|_| std::array::from_fn(|_| coordinate()));
        let origin = std::array::from_fn(|_| coordinate());
        let radius = coordinate().abs();
        let cube = QueryCube::new(origin, radius).unwrap();
        let expected = (0..3).all(|axis| {
            let min = points
                .iter()
                .map(|p| f64::from(p[axis]))
                .fold(f64::INFINITY, f64::min);
            let max = points
                .iter()
                .map(|p| f64::from(p[axis]))
                .fold(f64::NEG_INFINITY, f64::max);
            min <= f64::from(origin[axis]) + f64::from(radius)
                && max >= f64::from(origin[axis]) - f64::from(radius)
        });
        let actual =
            triangle_passes(&descriptor, &cube, |p, a| Ok(points[usize::from(p)][a])).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn line_cache_reuses_strip_read_order_then_expands_completed_extrema() {
    let triangles = [
        triangle(0, [0, 1, 2], 0, 0),
        triangle(0x200, [1, 2, 9], 0, 1),
        triangle(0, [1, 2, 3], 1, 0),
    ];
    let points = [
        [-1.25, 2., 3.],
        [4., -2.5, 0.],
        [0., 8., -3.75],
        [7., 1., 5.],
    ];
    let mut box_reads = Vec::new();
    let mut line_reads = Vec::new();
    let mut format = 0;
    let mut box_cache = None;
    let mut line_cache = None;
    let batches = [batch(0x101, usize::MAX, 0), batch(0, 0, 3)];
    let box_stats = prepare_static_cache(
        &mut format,
        &mut box_cache,
        &batches,
        |i| Ok(triangles[i]),
        |p, a| {
            box_reads.push((p, a));
            Ok(points[usize::from(p)][a])
        },
        CacheLimits::default(),
    )
    .unwrap();
    let line_stats = prepare_line_cache(
        &mut format,
        &mut line_cache,
        &batches,
        |i| Ok(triangles[i]),
        |p, a| {
            line_reads.push((p, a));
            Ok(points[usize::from(p)][a])
        },
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(line_reads, box_reads);
    assert_eq!(
        (
            line_stats.triangle_visits,
            line_stats.position_visits,
            line_stats.coordinate_reads
        ),
        (
            box_stats.triangle_visits,
            box_stats.position_visits,
            box_stats.coordinate_reads
        )
    );
    assert_eq!(line_stats.included_batches, 1);
    let box_bound = box_cache.unwrap().bounds[0];
    let line_bound = line_cache.unwrap().bounds[0];
    let margin = f64::from(f32::from_bits(0x3c23_d70a));
    assert_eq!(box_bound.minimum, [-1.25, -2.5, -3.75]);
    assert_eq!(box_bound.maximum, [7., 8., 5.]);
    for axis in 0..3 {
        assert_eq!(
            line_bound.minimum[axis].to_bits(),
            ((f64::from(box_bound.minimum[axis]) - margin) as f32).to_bits()
        );
        assert_eq!(
            line_bound.maximum[axis].to_bits(),
            ((f64::from(box_bound.maximum[axis]) + margin) as f32).to_bits()
        );
    }
    assert_eq!(format, 0);
}

#[test]
fn line_and_box_skip_bits_and_resident_slots_are_independent() {
    let mut format = 8;
    let mut line_cache = None;
    let empty = [batch(0, 0, 0)];
    let stats = prepare_line_cache(
        &mut format,
        &mut line_cache,
        &empty,
        |_| panic!("no triangles"),
        |_, _| panic!("no coordinates"),
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::Generated);
    assert!(line_cache.is_some());
    assert_eq!(format, 8);
    // The line skip bit wins even over an established cache and invalid budgets.
    format |= 0x10;
    let invalid = CacheLimits {
        triangle_visits: 0,
        coordinate_reads: 0,
    };
    let stats = prepare_line_cache(
        &mut format,
        &mut line_cache,
        &[],
        |_| panic!("skip before metadata"),
        |_, _| panic!("skip before positions"),
        invalid,
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::SkippedFormat);
    assert!(line_cache.is_some());
    format = 0x10;
    let mut box_cache = None;
    let stats = prepare_static_cache(
        &mut format,
        &mut box_cache,
        &empty,
        |_| panic!("empty"),
        |_, _| panic!("empty"),
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::Generated);
    assert!(box_cache.is_some());
    // Clearing the bit permits reuse without metadata or budget inspection.
    format = 8;
    let stats = prepare_line_cache(
        &mut format,
        &mut line_cache,
        &[],
        |_| panic!("reused"),
        |_, _| panic!("reused"),
        invalid,
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::Reused);
    assert_eq!(stats.included_batches, 1);
}

#[test]
fn line_empty_sentinels_are_expanded_with_f32_rounding_and_no_included_sets_bit16() {
    let mut format = 8;
    let mut cache = None;
    let stats = prepare_line_cache(
        &mut format,
        &mut cache,
        &[batch(0, 0, 0), batch(0, 0, 1)],
        |_| Ok(triangle(0x200, [u16::MAX; 3], 0, 0)),
        |_, _| panic!("disabled positions stay unread"),
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(
        (
            stats.included_batches,
            stats.triangle_visits,
            stats.coordinate_reads
        ),
        (2, 1, 0)
    );
    let margin = f64::from(f32::from_bits(0x3c23_d70a));
    for bound in cache.unwrap().bounds {
        assert_eq!(bound.minimum, [(999999f64 - margin) as f32; 3]);
        assert_eq!(bound.maximum, [(-999999f64 + margin) as f32; 3]);
        assert_eq!(bound.minimum, BatchBounds::EMPTY.minimum);
        assert_eq!(bound.maximum, BatchBounds::EMPTY.maximum);
    }
    let mut cache = None;
    let stats = prepare_line_cache(
        &mut format,
        &mut cache,
        &[batch(0x100, usize::MAX, 0)],
        |_| panic!("excluded"),
        |_, _| panic!("excluded"),
        CacheLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.disposition, CacheDisposition::NoIncludedBatches);
    assert_eq!(format, 0x18);
    assert!(cache.is_none());
}

#[test]
fn line_cache_capacity_and_coordinate_errors_do_not_publish_runtime_state() {
    let mut format = 0x200;
    let mut cache = None;
    let too_many = vec![batch(0, 0, 0); 0x2ab];
    let error = prepare_line_cache(
        &mut format,
        &mut cache,
        &too_many,
        |_| panic!("empty"),
        |_, _| panic!("empty"),
        CacheLimits::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("capacity"));
    assert_eq!(format, 0x200);
    assert!(cache.is_none());
    let error = prepare_line_cache(
        &mut format,
        &mut cache,
        &[batch(0, 0, 1)],
        |_| Ok(triangle(0, [0, 1, 2], 0, 0)),
        |_, _| Ok(0.),
        CacheLimits {
            coordinate_reads: 5,
            ..CacheLimits::default()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("coordinate budget"));
    assert_eq!(format, 0x200);
    assert!(cache.is_none());
}
