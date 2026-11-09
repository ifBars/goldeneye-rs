use super::*;
fn cube() -> OrientedBox {
    OrientedBox {
        center: [0.; 3],
        axes: [[1., 0., 0.], [0., 1., 0.]],
        half_extents: [1.; 3],
    }
}
fn plane(z: f32) -> [[f32; 3]; 3] {
    [[-3., -3., z], [3., -3., z], [0., 3., z]]
}
#[test]
fn face_contact_has_native_normal_depth_and_output_pair() {
    let q = cube().query_triangle(plane(0.5)).unwrap();
    let c = q.contact.unwrap();
    assert_eq!(c.axis, 4);
    assert_eq!(c.normal, [0., 0., 1.]);
    assert_eq!(c.depth, 0.5);
    assert!(c.point[0].abs() < 1e-6 && c.point[1].abs() < 1e-6);
    assert_eq!(c.point[2], 0.5);
    assert_eq!(c.point_minus_normal[2], -0.5);
}
#[test]
fn touching_candidate_does_not_imply_contact_and_zero_area_has_its_own_axis_tag() {
    let q = cube().query_triangle(plane(1.)).unwrap();
    assert!(q.candidate.is_some());
    assert!(q.contact.is_none());
    let q = cube().query_triangle([[0.; 3]; 3]).unwrap();
    assert_eq!(q.separation_axis, Some(99));
    let q = cube().query_triangle(plane(10.)).unwrap();
    assert!(q.contact.is_none());
    assert!(q.separation_axis.is_some());
}
#[test]
fn triangle_face_projects_weighted_box_face_onto_the_triangle() {
    let tri = [[-2., 4., 0.], [4., -2., 0.], [-2., 0., 4.]];
    let q = cube().query_triangle(tri).unwrap();
    let c = q.contact.unwrap();
    assert_eq!(c.axis, 1);
    let expected = 1f32 / 3f32.sqrt();
    assert!((c.depth - expected).abs() < 2e-6);
    for i in 0..3 {
        assert!((c.normal[i] - expected).abs() < 2e-6);
        assert!((c.point[i] - 2. / 3.).abs() < 2e-6);
    }
}
#[test]
fn invalid_used_values_and_overflow_are_errors_not_misses() {
    assert!(cube().query_triangle([[f32::NAN; 3]; 3]).is_err());
    assert!(
        cube()
            .query_triangle([[f32::MAX, 0., 0.], [-f32::MAX, 0., 0.], [0., 1., 0.]])
            .is_err()
    );
    let mut b = cube();
    b.half_extents[0] = -1.;
    assert!(b.query_triangle(plane(0.)).is_err());
}
#[test]
fn modest_common_translation_preserves_contact_and_triangle_vertices_are_rebuilt() {
    let mut b = cube();
    let shift = [3., -5., 7.];
    b.center = shift;
    let tri = plane(0.5).map(|p| std::array::from_fn(|i| p[i] + shift[i]));
    let c = b.query_triangle(tri).unwrap().contact.unwrap();
    assert_eq!(c.depth, 0.5);
    for (i, component) in shift.into_iter().enumerate() {
        assert!((c.point[i] - (component + if i == 2 { 0.5 } else { 0. })).abs() < 1e-6);
    }
    let t = math::Triangle::new([[1e8, 0., 0.], [1., 1., 0.], [1., 0., 1.]]).unwrap();
    assert_ne!(t.points().unwrap()[1][0], 1.);
}

#[test]
fn edge_contact_matches_independent_unbounded_line_solution() {
    let tri = [[-2.5, -2.5, -2.5], [0., -3., 1.5], [-1., 1.5, -1.]];
    let q = cube().query_triangle(tri).unwrap();
    let c = q.contact.unwrap();
    assert_eq!(c.axis, 13);
    let expected = [
        -0.4705882352941177,
        -0.8823529411764706,
        0.32352941176470584,
    ];
    for (actual, expected) in c.point.into_iter().zip(expected) {
        assert!((f64::from(actual) - expected).abs() < 2e-6);
    }
    assert!((f64::from(c.depth) - 0.5423261445466404).abs() < 2e-6);
}
#[test]
fn edge_bias_keeps_face_candidate_even_when_an_edge_has_smaller_raw_depth() {
    let tri = [[3., -1., -2.5], [-3., 1.5, -1.], [0., 0., 1.]];
    let q = cube().query_triangle(tri).unwrap();
    let c = q.candidate.unwrap();
    assert_eq!(c.axis, 1);
    assert!((f64::from(c.depth) - 1.479137032069964).abs() < 2e-6);
    // An independent full-projection f64 SAT gives raw edge-axis twelve depth
    // 1.264911064; the native 1.5 bias keeps the earlier triangle face.
    assert!(1.2649110640673518 < c.depth as f64);
}

#[test]
fn rotated_basis_preserves_contacts_while_axis_ids_follow_stored_order() {
    // Proper cyclic rotation: the old Z face becomes the world X face.
    let rotate = |p: [f32; 3]| [p[2], p[0], p[1]];
    let mut b = cube();
    b.axes = b.axes.map(rotate);
    let c = b
        .query_triangle(plane(0.5).map(rotate))
        .unwrap()
        .contact
        .unwrap();
    assert_eq!(c.axis, 4);
    assert_eq!(c.normal, [1., 0., 0.]);
    assert_eq!(c.depth, 0.5);
    assert!((c.point[0] - 0.5).abs() < 1e-6);
    assert!(c.point[1].abs() < 1e-6 && c.point[2].abs() < 1e-6);
    // Same world box, different stored basis order: world X is now axis zero.
    let c = cube()
        .query_triangle(plane(0.5).map(rotate))
        .unwrap()
        .contact
        .unwrap();
    assert_eq!(c.axis, 2);
    assert_eq!(c.normal, [1., 0., 0.]);
}

#[test]
fn triangle_plane_is_one_sided_and_short_box_axes_are_not_orthonormalized() {
    assert_eq!(
        cube().query_triangle(plane(-10.)).unwrap().separation_axis,
        Some(1)
    );
    assert_eq!(
        cube().query_triangle(plane(10.)).unwrap().separation_axis,
        Some(4)
    );
    let mut b = cube();
    b.axes[0][0] = 0.5;
    let q = b.query_triangle(plane(0.5)).unwrap();
    let c = q.candidate.unwrap();
    assert_eq!(b.axis(0).unwrap(), [0.5, 0., 0.]);
    assert_eq!(b.axis(2).unwrap(), [0., 0., 0.5]);
    // Native optimized axis eight is Y x E0 = (0,0,-6). Its radius
    // is 3 from the short X axis, and projection -3, giving depth zero.
    // Normalizing the stored box first would instead keep face axis four.
    assert_eq!(c.axis, 8);
    assert_eq!(c.normal, [0., 0., 1.]);
    assert_eq!(c.depth, 0.);
    assert_eq!(q.contact.unwrap().normal, [0., 0., 1.]);
}

type D = [f64; 3];
fn dsub(a: D, b: D) -> D {
    std::array::from_fn(|i| a[i] - b[i])
}
fn ddot(a: D, b: D) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn dcross(a: D, b: D) -> D {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn oracle(points: [D; 3]) -> (Option<u8>, u8, f64, bool) {
    let basis = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
    let a = dsub(points[1], points[0]);
    let b = dsub(points[2], points[0]);
    let edges = [a, b, dsub(b, a)];
    let n = dcross(a, b);
    let length = ddot(n, n).sqrt();
    if length == 0. {
        return (Some(99), 0, 0., false);
    }
    let gap = n.into_iter().map(f64::abs).sum::<f64>() + ddot(n, points[0]);
    if gap < 0. {
        return (Some(1), 0, 0., gap.abs() < 1e-5);
    }
    let (mut id, mut depth, mut ambiguous) = (1, gap / length, gap.abs() < 1e-5);
    for number in 0..12 {
        let axis = if number < 3 {
            basis[number]
        } else {
            dcross(basis[(number - 3) / 3], edges[(number - 3) % 3])
        };
        let length = ddot(axis, axis).sqrt();
        if length == 0. {
            continue;
        }
        // Full point projections and the sum of absolute box-axis projections
        // are algebraically independent of the production triple-product path.
        let projected = points.map(|p| ddot(axis, p));
        let low = projected.into_iter().fold(f64::INFINITY, f64::min);
        let high = projected.into_iter().fold(f64::NEG_INFINITY, f64::max);
        let radius = axis.into_iter().map(f64::abs).sum::<f64>();
        let left = radius - low;
        let right = high + radius;
        if left < 0. || right < 0. {
            return (
                Some(number as u8 + 2),
                id,
                depth,
                ambiguous || left.abs().min(right.abs()) < 1e-5,
            );
        }
        let candidate = left.min(right) / if number < 3 { 1. } else { length };
        let effective = candidate * if number < 3 { 1. } else { 1.5 };
        ambiguous |= (effective - depth).abs() < 1e-5 || left.abs().min(right.abs()) < 1e-5;
        if effective < depth {
            id = number as u8 + 2;
            depth = candidate;
        }
    }
    (None, id, depth, ambiguous)
}
#[test]
fn ordered_candidates_agree_with_independent_full_projection_f64_samples() {
    let mut word = 19u32;
    let mut checked = 0;
    let mut contacts = 0;
    for _ in 0..800 {
        let mut tri = [[0.; 3]; 3];
        for p in &mut tri {
            for v in p {
                word = word.wrapping_mul(1664525).wrapping_add(1013904223);
                *v = ((word >> 24) as i32 - 128) as f32 / 32.;
            }
        }
        let (separation, axis, depth, ambiguous) = oracle(tri.map(|p| p.map(f64::from)));
        let result = cube().query_triangle(tri).unwrap();
        if !ambiguous {
            checked += 1;
            assert_eq!(result.separation_axis, separation);
            if let Some(c) = &result.candidate {
                assert_eq!(c.axis, axis);
                assert!((f64::from(c.depth) - depth).abs() < 3e-5);
            }
        }
        if let Some(contact) = result.contact {
            contacts += 1;
            let origin = tri[0].map(f64::from);
            let n = dcross(
                dsub(tri[1].map(f64::from), origin),
                dsub(tri[2].map(f64::from), origin),
            );
            let distance =
                ddot(n, dsub(contact.point.map(f64::from), origin)).abs() / ddot(n, n).sqrt();
            assert!(distance < 3e-5, "contact left triangle plane by {distance}");
        }
    }
    assert!(checked > 600);
    assert!(contacts > 100);
}
