use super::*;
fn tri() -> [V; 3] {
    [[0., 0., 0.], [2., 0., 0.], [0., 2., 0.]]
}
#[test]
fn portal_triangle_projects_into_face_edges_and_all_three_corners() {
    for (point, closest, expected) in [
        ([0.5, 0.5, 3.], [0.5, 0.5, 0.], 3.),
        ([-1., -1., 0.], [0., 0., 0.], 2f32.sqrt()),
        ([3., -1., 0.], [2., 0., 0.], 2f32.sqrt()),
        ([-1., 3., 0.], [0., 2., 0.], 2f32.sqrt()),
        ([1., -1., 0.], [1., 0., 0.], 1.),
        ([-1., 1., 0.], [0., 1., 0.], 1.),
        ([2., 2., 0.], [1., 1., 0.], 2f32.sqrt()),
        ([-1., 4., 0.], [0., 2., 0.], 5f32.sqrt()),
        ([4., -1., 0.], [2., 0., 0.], 5f32.sqrt()),
    ] {
        let result = distance(&tri(), point, 1).unwrap();
        assert!((result.distance - expected).abs() < 1e-6, "{point:?}");
        assert_eq!(result.point, closest);
    }
}
#[test]
fn alternating_fan_overwrites_closest_point_on_a_distance_tie() {
    let vertices = [[0., 0., 0.], [2., 0., 0.], [0., 2., 0.], [0., 0., 2.]];
    let result = distance(&vertices, [1., 0.5, 1.], 2).unwrap();
    assert_eq!(result.distance, 1.);
    assert_eq!(result.point, [0., 0.5, 1.]);
    assert_eq!(result.triangles_tested, 2);
    assert!(distance(&vertices, [1., 0.5, 1.], 1).is_err());
}
#[test]
fn degenerate_triangles_return_the_native_sentinel_and_tiny_distances_become_zero() {
    let result = distance(&[[1., 2., 3.]; 3], [100., 100., 100.], 1).unwrap();
    assert_eq!(result.degenerate_triangles, 1);
    assert_eq!(result.point, [1., 2., 3.]);
    assert_eq!(result.distance.to_bits(), 0x4f7fffff);
    assert_eq!(
        distance(&tri(), [0., 0., 1. / 4096.], 1).unwrap().distance,
        0.
    );
    assert_eq!(
        distance(&tri(), [0., 0., 1. / 2048.], 1).unwrap().distance,
        1. / 2048.
    );
    let small = [[0., 0., 0.], [0.1, 0., 0.], [0., 0.01, 0.]];
    assert_eq!(
        distance(&small, [0.; 3], 1).unwrap().degenerate_triangles,
        1
    );
    assert!(distance(&tri()[..2], [0.; 3], 1).is_err());
    assert!(distance(&tri(), [f32::NAN; 3], 1).is_err());
    assert!(distance(&tri(), [0.; 3], 0).is_err());
    // Native quadratic cancellation can erase a nonzero geometric distance.
    // Replacing the return value with length(point-closest) changes admission.
    let large = [[0., 0., 0.], [1000., 0., 0.], [0., 1000., 0.]];
    // Quarter-edge parameters are exactly representable, isolating cancellation
    // of the 0.0001 squared height from barycentric rounding.
    let cancelled = distance(&large, [250., 250., 0.01], 1).unwrap();
    assert_eq!(cancelled.distance, 0.);
    assert_eq!(cancelled.point, [250., 250., 0.]);
}
#[test]
fn portal_groups_keep_duplicates_strict_radius_and_lazy_first_hit_order() {
    let groups = [
        PortalGroup {
            target_zone: -1,
            portal_indices: vec![-1],
        },
        PortalGroup {
            target_zone: 1,
            portal_indices: vec![0, 1, 99],
        },
        PortalGroup {
            target_zone: 1,
            portal_indices: vec![2],
        },
        PortalGroup {
            target_zone: 2,
            portal_indices: vec![3],
        },
    ];
    let mut visited = Vec::new();
    let result = expand(0, 3, &groups, 1., 10, 10, |index| {
        visited.push(index);
        Ok(match index {
            0 | 3 => 1.,
            1 | 2 => 0.5,
            _ => panic!("unread portal"),
        })
    })
    .unwrap();
    assert_eq!(visited, [0, 1, 2, 3]);
    assert_eq!(result.zones, [0, 1, 1]);
    assert_eq!(result.portal_tests, 4);
    assert!(expand(0, 3, &groups, 1., 1, 10, |_| Ok(1.)).is_err());
    assert!(expand(0, 3, &groups, 1., 10, 1, |_| Ok(0.)).is_err());
    assert!(
        expand(0, 3, &groups, 1., 10, 10, |_| anyhow::bail!(
            "backend unavailable"
        ))
        .is_err()
    );
    assert!(expand(0, 3, &groups, 1., 10, 10, |_| Ok(f32::NAN)).is_err());
    let invalid = [PortalGroup {
        target_zone: 20,
        portal_indices: vec![0],
    }];
    assert_eq!(
        expand(0, 3, &invalid, 1., 10, 10, |_| Ok(2.))
            .unwrap()
            .zones,
        [0]
    );
    assert!(expand(0, 3, &invalid, 1., 10, 10, |_| Ok(0.)).is_err());
}

type D = [f64; 3];
fn sub64(a: D, b: D) -> D {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot64(a: D, b: D) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn segment_squared(a: D, b: D, p: D) -> f64 {
    let e = sub64(b, a);
    let d = sub64(p, a);
    let t = (dot64(d, e) / dot64(e, e)).clamp(0., 1.);
    let v = std::array::from_fn(|i| d[i] - t * e[i]);
    dot64(v, v)
}
fn independent_squared(vertices: [V; 3], point: V) -> f64 {
    let [a, b, c] = vertices.map(|v| v.map(f64::from));
    let p = point.map(f64::from);
    let u = sub64(b, a);
    let v = sub64(c, a);
    let q = sub64(p, a);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let nn = dot64(n, n);
    let h = dot64(n, q) / nn;
    let on_plane: D = std::array::from_fn(|i| q[i] - h * n[i]);
    // Oriented edge tests, independent of the production region classifier.
    let projected = std::array::from_fn(|i| a[i] + on_plane[i]);
    let inside = [(a, b), (b, c), (c, a)].into_iter().all(|(a, b)| {
        let e = sub64(b, a);
        let d = sub64(projected, a);
        let cross = [
            e[1] * d[2] - e[2] * d[1],
            e[2] * d[0] - e[0] * d[2],
            e[0] * d[1] - e[1] * d[0],
        ];
        dot64(cross, n) >= 0.
    });
    if inside {
        h * h * nn
    } else {
        [(a, b), (b, c), (c, a)]
            .into_iter()
            .map(|(a, b)| segment_squared(a, b, p))
            .fold(f64::INFINITY, f64::min)
    }
}
#[test]
fn deterministic_samples_agree_with_independent_f64_plane_and_segment_geometry() {
    let mut word = 117u32;
    let mut tested = 0;
    for _ in 0..1200 {
        let mut points = [[0.; 3]; 4];
        for p in &mut points {
            for v in p {
                word = word.wrapping_mul(1664525).wrapping_add(1013904223);
                *v = ((word >> 24) as i32 - 128) as f32 / 32.;
            }
        }
        let vertices = [points[0], points[1], points[2]];
        let result = distance(&vertices, points[3], 1).unwrap();
        if result.degenerate_triangles > 0 {
            continue;
        }
        let expected = independent_squared(vertices, points[3]).sqrt();
        assert!(
            (f64::from(result.distance) - expected).abs() < 0.0004,
            "distance {} vs {expected} for {points:?}",
            result.distance
        );
        let point_distance = dot64(
            sub64(result.point.map(f64::from), points[3].map(f64::from)),
            sub64(result.point.map(f64::from), points[3].map(f64::from)),
        )
        .sqrt();
        assert!((point_distance - expected).abs() < 0.0001);
        tested += 1;
    }
    assert!(tested > 1190);
}
