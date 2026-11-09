use super::*;
fn placement(scale: [f32; 3]) -> Placement {
    Placement {
        hash: 1,
        position: [10., 20., 30.],
        flags: 0,
        rotation: [0.; 3],
        scale,
        engine_flags: 4,
        map_on: 0,
        object_hash: 2,
        light_set: 0,
        group: -1,
        unknown_word: 0,
        model: ge_assets::transform::placement([10., 20., 30.], [0.; 3], scale).unwrap(),
        status: None,
    }
}
#[test]
fn both_authored_modes_are_probed_and_singular_is_not_validated() {
    let mut audit = Audit::default();
    audit
        .include(&[placement([2., 4., 8.]), placement([0., 1., 1.])])
        .unwrap();
    assert_eq!((audit.rigid_frames, audit.general_frames), (2, 2));
    assert_eq!(
        (
            audit.validated_general_frames,
            audit.singular_general_frames
        ),
        (1, 1)
    );
    assert_eq!((audit.query_probes, audit.unvalidated_queries), (20, 5));
    assert_eq!(
        audit.strict_queries + audit.rounding_queries + audit.unvalidated_queries,
        audit.query_probes
    );
    assert_eq!(
        audit.strict_inverse_frames + audit.rounding_inverse_frames,
        3
    );
}
#[test]
fn ill_conditioned_mode_and_nonfinite_inputs_are_explicit() {
    let mut audit = Audit::default();
    audit.include(&[placement([1e-8, 1., 1.])]).unwrap();
    assert_eq!(audit.ill_conditioned_general_frames, 1);
    assert_eq!(audit.validated_general_frames, 0);
    assert_eq!(audit.singular_general_frames, 0);
    assert_eq!(audit.unvalidated_queries, 5);
    let mut invalid = placement([1.; 3]);
    invalid.model[0] = f32::NAN;
    assert!(Audit::default().include(&[invalid]).is_err());
}
