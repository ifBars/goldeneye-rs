use super::*;
#[test]
fn fresh_admission_never_silently_drops_ammo_or_alternate_metadata() {
    let valid = metadata::InventoryMetadata {
        inventory_key: DEFAULT_ITEM,
        primary_key: 0x4b00_00a5,
        type_key: 0x4b20_0000,
        category_key: 0x4b10_0001,
        byte_9_raw: 0,
        ammunition_raw: [0, 0],
        source_index: 0,
        group_zero_address: 0,
        group_four_address: 0,
    };
    validate_metadata(valid).unwrap();
    for field in 0..4 {
        let mut row = valid;
        match field {
            0 => row.ammunition_raw[0] = 1,
            1 => row.byte_9_raw = 1,
            2 => row.category_key = 0x4b10_00a5,
            _ => row.type_key = 0x4b20_00a5,
        }
        assert!(validate_metadata(row).is_err());
    }
}
