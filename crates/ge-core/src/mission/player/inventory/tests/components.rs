use super::*;

#[test]
fn pending_slot_copies_raw_high_bits_and_only_minus_one_uses_current_slot() {
    for raw in [0, 7, 0x80, 0xfe, 0xff] {
        let mut inventory = Inventory::from_constructor();
        inventory.slots.c55 = Some(raw);
        inventory.slots.c96 = Some(3);
        inventory.slots.c97 = Some(4);
        inventory.prepare_component_slots().unwrap();
        let expected = if raw == 0xff { 3 } else { raw };
        assert_eq!(inventory.slots.c96, Some(expected));
        assert_eq!(inventory.slots.c97, Some(expected));
        assert_eq!(inventory.slots.c55, Some(0xff));
        assert!(inventory.records().is_empty());
    }
}

#[test]
fn missing_slots_are_errors_and_never_replace_unknown_data_with_defaults() {
    let mut inventory = Inventory::from_constructor();
    let before = inventory.slots;
    assert!(inventory.prepare_component_slots().is_err());
    assert_eq!(inventory.slots, before);
    inventory.slots.c55 = Some(0xff);
    assert!(inventory.prepare_component_slots().is_err());
    assert_eq!(inventory.slots.c97, None);
}
