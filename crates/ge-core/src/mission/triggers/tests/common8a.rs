use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;
use std::cell::Cell;

fn common8a_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 40,
        class: 0x1e00_000b,
        subtype: 0x1f00_008a,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn hook_gate_checks_header_before_reading_numeric0() {
    assert_eq!(
        common8a::State::from_serialized(1, || panic!("header-disabled numeric read")).gate(),
        common8a::Gate::HeaderDisabled
    );
    let reads = Cell::new(0);
    assert_eq!(
        common8a::State::from_serialized(0, || {
            reads.set(reads.get() + 1);
            2
        })
        .gate(),
        common8a::Gate::NumericBitClear
    );
    assert_eq!(reads.get(), 1);
    assert_eq!(
        common8a::State::from_serialized(0, || 1).gate(),
        common8a::Gate::RequiresHook
    );
}

#[test]
fn skipped_hook_keeps_raw_fields_and_status1_but_reached_hook_stays_staged() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    for (ordinal, (header, numeric0)) in [(1, 1), (0, 2), (0, 1)].into_iter().enumerate() {
        let mut payload = common(numeric0);
        payload.header_flags_raw = header;
        payload.numeric[1] = 0xdead_beef;
        payload.property_mask_raw |= (1 << 1) | (1 << 26);
        payload.animator_present = true;
        if ordinal == 0 {
            payload.reference_ordinals[0] = 2;
            payload.property_mask_raw |= 1 << 16;
        }
        registry
            .visit(common8a_row(ordinal), || Ok(payload), &mut factory)
            .unwrap();
    }
    for index in 0..2 {
        let object = &registry.objects()[index];
        assert_eq!(object.runtime_numeric(), &object.common().numeric);
        assert_eq!(object.runtime_flags_raw(), 0);
        assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
        assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    }
    assert_eq!(
        registry.objects()[0].common8a().unwrap().gate(),
        common8a::Gate::HeaderDisabled
    );
    assert_eq!(
        registry.objects()[1].common8a().unwrap().gate(),
        common8a::Gate::NumericBitClear
    );
    let staged = &registry.objects()[2];
    assert_eq!(
        staged.common8a().unwrap().gate(),
        common8a::Gate::RequiresHook
    );
    assert_eq!(staged.runtime_numeric(), &staged.common().numeric);
    assert!(!staged.is_registered());
    assert!(matches!(
        registry.slots(0).unwrap()[2],
        Slot::Unsupported(Unsupported::Common8AHook { .. })
    ));
    assert_eq!(
        (registry.scratch().len(), registry.membership().len()),
        (3, 2)
    );
    finish(&mut registry);
    assert_eq!(
        registry.objects()[0].references()[0],
        Reference::Blocked { ordinal: 2 }
    );
    assert!(!registry.is_complete());
    assert!(factory.requests.is_empty());
}
