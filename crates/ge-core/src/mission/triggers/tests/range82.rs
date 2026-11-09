use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn range(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 38,
        class: 0x1e00_0005,
        subtype: 0x1f00_0082,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn subtype82_defaults_signed_scalars_and_tokens_are_distinct_from_serialized_words() {
    assert_eq!(range82::initialize_runtime_flags(0x2004), 0x2004);
    assert_eq!(range0c::initialize_runtime_flags(0x2004), 4);
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common(0);
    payload.numeric[2] = 20;
    payload.numeric[4] = 0x0900_0003;
    payload.numeric[5] = (-30i32) as u32;
    payload.numeric[7] = 0;
    payload.numeric[8] = 0x0b00_0002;
    payload.numeric[13] = 0x4200_0000;
    payload.animator_present = true;
    payload.property_mask_raw |= 1 << 26;
    let serialized = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.range82().unwrap();
    assert_eq!(object.common().numeric, serialized);
    assert_eq!(object.kind(), TriggerKind::Range82);
    assert_eq!(state.radius_bits(), 200f32.to_bits());
    assert_eq!(state.radius_squared_bits(), 40000f32.to_bits());
    assert_eq!(state.secondary_radius_bits(), 2f32.to_bits());
    assert_eq!(state.secondary_radius_squared_bits(), 4f32.to_bits());
    assert_eq!(
        (state.numeric5_bits(), state.numeric7_bits()),
        (Some((-3f32).to_bits()), Some(0))
    );
    assert_eq!(state.numeric8_reference(), Some(0x0b00_0002));
    assert_eq!(state.word_11c(), Some(0));
    assert_eq!(state.mode_low_byte(), Some(0));
    assert_eq!(state.diagnostic_reached(), Some(true));
    assert_eq!(object.runtime_numeric()[13], 0);
    assert_eq!(object.runtime_numeric()[4], 0x0900_0003);
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn accepted_resource_stages_before_extra_scalars_and_zero_word_without_fake_mode() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut payload = common(10);
    payload.numeric[5] = 30;
    payload.numeric[7] = 40;
    payload.numeric[8] = 0x0b00_0002;
    payload.numeric[13] = 0x4200_0001;
    let mut factory = FactoryProbe::default();
    assert!(matches!(
        registry
            .visit(range(0), || Ok(payload), &mut factory)
            .unwrap(),
        Slot::Unsupported(Unsupported::Range82Provider {
            resource: 0x4200_0001,
            ..
        })
    ));
    let state = registry.objects()[0].range82().unwrap();
    assert_eq!(state.radius_bits(), 1f32.to_bits());
    assert_eq!(state.secondary_radius_bits(), 1f32.to_bits());
    assert_eq!(state.mode_low_byte(), None);
    assert_eq!(
        (
            state.numeric5_bits(),
            state.numeric7_bits(),
            state.numeric8_reference(),
            state.word_11c(),
            state.diagnostic_reached()
        ),
        (None, None, None, None, None)
    );
    assert_eq!(registry.objects()[0].runtime_numeric()[5], 30);
    assert_eq!(registry.scratch().len(), 1);
    assert!(registry.membership().is_empty());
    let mut other = common(3);
    other.reference_ordinals[0] = 0;
    registry.visit(row(1), || Ok(other), &mut factory).unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[1].references()[0],
        Reference::Blocked { ordinal: 0 }
    );
    assert!(!registry.is_complete());
}

#[test]
fn provider_free_constructor_rewrites_only_proven_slots_and_resolves_live_references() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common(10);
    payload.numeric[4] = 0x0900_0000;
    payload.numeric[8] = 0x0b00_0000;
    payload.numeric[9] = 0xdead_beef;
    payload.numeric[13] = 0x4300_0001;
    payload.reference_ordinals[0] = 1;
    registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    let mut other = common(8);
    other.reference_ordinals[0] = 0;
    registry.visit(row(1), || Ok(other), &mut factory).unwrap();
    finish(&mut registry);
    let first = &registry.objects()[0];
    assert_eq!(first.runtime_numeric()[9], 0xdead_beef);
    assert_eq!(
        [
            first.runtime_numeric()[4],
            first.runtime_numeric()[8],
            first.runtime_numeric()[13]
        ],
        [0; 3]
    );
    assert_eq!(
        first.references()[0],
        Reference::Trigger(registry.objects()[1].id())
    );
    assert_eq!(
        registry.objects()[1].references()[0],
        Reference::Trigger(first.id())
    );
    assert!(registry.is_complete());
}
