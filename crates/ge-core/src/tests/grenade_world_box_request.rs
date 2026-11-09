use super::*;
use crate::grenade_arc::prepare_grenade_arc;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
fn volume() -> GrenadeClearanceVolume {
    GrenadeClearanceVolume {
        shape: OrientedBox {
            center: [3., 4., 5.],
            axes: [[0.25, 0.5, 0.], [0., 0.25, 0.5]],
            half_extents: [2., 0.75, 0.3],
        },
        selection_origin: [-7., 2., 1.],
        selection_radius: 17.,
    }
}
#[test]
fn prepared_arc_uses_selection_start_and_apex_distance_instead_of_padded_box_center() {
    let arc = prepare_grenade_arc([10., 2., -7.], [30., 4., 3.], 0, 1)
        .unwrap()
        .arc
        .unwrap();
    let volume = arc.clearance_volume().unwrap();
    let request = volume.world_box_request().unwrap();
    assert_eq!(request.world_origin, arc.start);
    assert_ne!(request.world_origin, volume.shape.center);
    assert_eq!(
        request.radius,
        crate::grenade::length(arc.apex_displacement).unwrap()
    );
    assert_eq!(request.boxes[0].center, volume.shape.center);
    assert_eq!(request.world_from_query, IDENTITY);
    assert_eq!(
        (
            request.shape_type,
            request.filter_slot,
            request.filter_flags,
            request.request_flags_raw
        ),
        (3, 1, 3, 2)
    );
    assert_eq!(
        (request.record_capacity, request.contact_capacity),
        (64, 512)
    );
}
#[test]
fn one_owned_shape_retains_short_axes_extents_and_independent_selection_values() {
    let mut volume = volume();
    let request = GrenadeWorldBoxRequest::from_volume(&volume).unwrap();
    assert_eq!(request.boxes.len(), 1);
    assert_eq!(request.boxes[0].axes, [[0.25, 0.5, 0.], [0., 0.25, 0.5]]);
    assert_eq!(request.boxes[0].half_extents, [2., 0.75, 0.3]);
    assert_eq!(request.world_origin, [-7., 2., 1.]);
    assert_eq!(request.radius, 17.);
    volume.shape.center = [100.; 3];
    volume.shape.axes = [[0.; 3]; 2];
    assert_ne!(request.boxes[0].center, volume.shape.center);
    assert_ne!(request.boxes[0].axes, volume.shape.axes);
    assert_eq!(request.boxes[0].center, [3., 4., 5.]);
    assert_eq!(request.boxes[0].axes[0], [0.25, 0.5, 0.]);
}
#[test]
fn invalid_selection_or_primitive_is_an_error_not_an_empty_request() {
    let mut bad = volume();
    bad.selection_radius = f32::NAN;
    assert!(bad.world_box_request().is_err());
    let mut bad = volume();
    bad.selection_origin[1] = f32::INFINITY;
    assert!(bad.world_box_request().is_err());
    let mut bad = volume();
    bad.shape.half_extents[2] = -0.3;
    assert!(bad.world_box_request().is_err());
}
