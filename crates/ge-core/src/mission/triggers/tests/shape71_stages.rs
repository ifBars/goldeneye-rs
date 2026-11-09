use super::*;

fn input() -> GeometryInput {
    GeometryInput {
        type_raw: 0,
        word_04_raw: 7,
        center_bits: [0; 3],
        orientation_bits: [0, 0, 0, 0x3f80_0000],
        dimensions_bits: [0x3f80_0000; 3],
    }
}

#[test]
fn constructor_knows_only_tag_and_sparse_fallback_does_not_invent_storage() {
    let mut state = State::from_serialized(0);
    assert_eq!(
        (state.radius_bits(), state.radius_squared_bits()),
        (0, 0x461c_3c00)
    );
    assert_eq!(state.geometry(), &Geometry::Uninitialized);
    assert_eq!(state.geometry().tag(), -1);
    assert_eq!(state.geometry_stage(), Stage::Uninitialized);
    assert_eq!(state.placement_matrix_bits(), None);
    let center_bits = [0x7fc0_1234, 0x8000_0000, 3, 0];
    state.fallback(center_bits);
    assert_eq!(
        state.geometry(),
        &Geometry::Type0 {
            center_bits,
            radius_bits: 0
        }
    );
    assert_eq!(state.geometry_stage(), Stage::FallbackComplete);
    assert_eq!(state.placement_matrix_bits(), None);
}

#[test]
fn later_unsupported_math_preserves_materialized_and_placement_stages() {
    let mut state = State::from_serialized(10);
    assert!(
        state
            .prepare_present(input(), [0; 3], [f32::MAX.to_bits(), 0, 0])
            .is_err()
    );
    assert_eq!(state.geometry().tag(), 2);
    assert_eq!(state.geometry_stage(), Stage::Materialized);
    assert_eq!(state.placement_matrix_bits(), None);
    let mut geometry = input();
    geometry.center_bits[0] = f32::MAX.to_bits();
    let mut state = State::from_serialized(10);
    assert!(
        state
            .prepare_present(geometry, [f32::MAX.to_bits(), 0, 0], [0; 3])
            .is_err()
    );
    assert_eq!(state.geometry_stage(), Stage::PlacementBuilt);
    assert!(state.placement_matrix_bits().is_some());
    let Geometry::Type2 { center_bits, .. } = state.geometry() else {
        panic!("materialized geometry missing")
    };
    assert_eq!(center_bits[0], f32::MAX.to_bits());
    assert_eq!(center_bits[3], 1.0f32.to_bits());
}

#[test]
fn unsupported_type_is_not_an_initialized_default_shape() {
    let mut state = State::from_serialized((-20i32) as u32);
    let mut unsupported = input();
    unsupported.type_raw = 3;
    assert!(state.prepare_present(unsupported, [0; 3], [0; 3]).is_err());
    assert_eq!(state.geometry(), &Geometry::Uninitialized);
    assert_eq!(state.geometry_stage(), Stage::Uninitialized);
    assert_eq!(
        (state.radius_bits(), state.radius_squared_bits()),
        ((-2.0f32).to_bits(), 4.0f32.to_bits())
    );
}
