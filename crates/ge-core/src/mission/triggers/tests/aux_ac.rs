use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn aux(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 72,
        class: 0x1e00_0007,
        subtype: 0x1f00_00ac,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn aux_ac_radius_uses_numeric1_not_numeric0_and_retains_every_raw_numeric_word() {
    assert_eq!(
        aux_ac::State::from_serialized(1).radius_squared_bits(),
        0x3c23_d70b
    );
    assert_eq!(
        aux_ac::State::from_serialized(u32::MAX).radius_squared_bits(),
        0x3c23_d70b
    );
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common(999);
    payload.numeric[1] = 20;
    payload.numeric[3] = 0x4200_0007;
    payload.numeric[14] = 0xdead_beef;
    payload.animator_present = true;
    payload.property_mask_raw |= (1 << 1) | (1 << 3) | (1 << 14) | (1 << 26);
    let raw = payload.numeric;
    registry
        .visit(aux(0), || Ok(payload), &mut factory)
        .unwrap();
    registry
        .visit(aux(1), || Ok(common(999)), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.aux_ac().unwrap();
    assert_eq!(object.kind(), TriggerKind::AuxAc);
    assert_eq!(object.runtime_numeric(), &raw);
    assert_eq!(object.common().numeric, raw);
    assert_eq!(state.radius_squared_bits(), 4.0f32.to_bits());
    assert_eq!(state.extra_words(), &[Some(0); 6]);
    assert_eq!(
        registry.objects()[1]
            .aux_ac()
            .unwrap()
            .radius_squared_bits(),
        0x7f7f_ffff
    );
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}
