use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn radius(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 39,
        class: 0x1e00_0004,
        subtype: 0x1f00_0088,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn radius88_extra_word_is_unknown_in_constructor_then_full_zero_after_ordered_scalar_post() {
    let mut state = radius88::State::constructor();
    assert_eq!(state.stage(), radius88::Stage::Constructor);
    assert_eq!(state.radius_squared_bits(), 0);
    assert_eq!(state.word_118(), None);
    state.initialize(1);
    assert_eq!(state.stage(), radius88::Stage::PostCommon);
    assert_eq!(state.radius_squared_bits(), 0x3c23_d70a);
    assert_eq!(state.word_118(), Some(0));
    assert_eq!(
        radius88::State::from_serialized(0).radius_squared_bits(),
        0x7f7f_ffff
    );
    assert_eq!(
        radius88::State::from_serialized(u32::MAX).radius_squared_bits(),
        0x3c23_d70a
    );
    assert_eq!(
        radius88::State::from_serialized(i32::MAX as u32).radius_squared_bits(),
        0x5b23_d70a
    );
}

#[test]
fn filtered_map_keeps_radius88_original_ordinals_and_raw_tokens_without_provider_or_activation() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(2), 3, Some(&[0x1f00_0088])).unwrap();
    let mut factory = FactoryProbe::default();
    registry
        .visit(
            row(0),
            || panic!("filtered common must remain lazy"),
            &mut factory,
        )
        .unwrap();
    let mut payload = common(1);
    payload.header_flags_raw = 0x200;
    payload.numeric[1] = 0x4200_0007;
    payload.numeric[7] = 0x0200_0011;
    payload.animator_present = true;
    payload.property_mask_raw |= (1 << 1) | (1 << 7) | (1 << 26);
    let raw = payload.numeric;
    registry
        .visit(radius(1), || Ok(payload), &mut factory)
        .unwrap();
    registry
        .visit(radius(2), || Ok(common(0)), &mut factory)
        .unwrap();
    assert_eq!(registry.slots(0).unwrap()[0], Slot::Filtered);
    let object = &registry.objects()[0];
    assert_eq!(
        (object.kind(), object.ordinal()),
        (TriggerKind::Radius88, 1)
    );
    assert_eq!(object.runtime_numeric(), &raw);
    assert_eq!(object.common().numeric, raw);
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert_eq!(object.radius88().unwrap().word_118(), Some(0));
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}
