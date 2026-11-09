use super::*;
#[test]
fn clipping_is_closed_and_keeps_previous_vertex_first() {
    let p = vec![[-1., -1., 0.], [1., -1., 0.], [1., 1., 0.], [-1., 1., 0.]];
    assert_eq!(
        clip(p, [1., 0., 0.], 0.).unwrap(),
        vec![[0., -1., 0.], [1., -1., 0.], [1., 1., 0.], [0., 1., 0.]]
    );
    let p = vec![[0., 0., 0.], [0., 1., 0.], [0., 2., 0.]];
    assert_eq!(
        clip(p, [1., 0., 0.], 0.).unwrap(),
        vec![[0., 2., 0.], [0., 0., 0.], [0., 1., 0.]]
    );
    assert_eq!(
        clip(
            vec![[0., 0., 0.], [1., 0., 0.], [-1., 1., 0.]],
            [1., 0., 0.],
            0.
        )
        .unwrap(),
        vec![[0., 0., 0.], [1., 0., 0.], [0., 0.5, 0.]]
    );
}
#[test]
fn weighted_point_uses_positive_penetration_and_rejects_zero_weight_contacts() {
    let points = [[0., 0., 0.], [1., 0., 0.], [0., 2., 0.]];
    let p = weighted(&points, [0., -1., 0.], 0.).unwrap().unwrap();
    assert_eq!(p, [0., 2., 0.]);
    assert!(
        weighted(&[[0.; 3]; 3], [0., -1., 0.], 0.)
            .unwrap()
            .is_none()
    );
    assert!(clip(Vec::new(), [1., 0., 0.], 0.).unwrap().is_empty());
}
