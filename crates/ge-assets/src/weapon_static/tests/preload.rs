use super::*;

fn identity() -> Identity {
    Identity {
        game_id: "TEST00".into(),
        revision: 0,
        main_dol_partition_offset: 0,
        dol_bytes: 0,
        dol_sha256: "a".repeat(64),
        data_section_index: 0,
        data_virtual_address: 0x1000,
        data_bytes: 0x100,
        data_sha256: "b".repeat(64),
    }
}
fn tables() -> WeaponTables {
    let mut bytes = vec![0; 3 * 0x2c + 4];
    for (i, (key, enabled)) in [(0x020000f0, 0), (0x02000001, 0x80), (0x02000020, 0xff)]
        .into_iter()
        .enumerate()
    {
        bytes[i * 0x2c..i * 0x2c + 4].copy_from_slice(&u32::to_be_bytes(key));
        bytes[i * 0x2c + 0x24] = enabled;
        // Bad unused pointers and neighboring bytes must never be consumed.
        bytes[i * 0x2c + 0x0c..i * 0x2c + 0x10].fill(0xff);
        bytes[i * 0x2c + 0x25..i * 0x2c + 0x28].fill(0xab);
    }
    WeaponTables::initialize(
        Data {
            address: 0x1000,
            bytes: bytes.into_boxed_slice(),
        },
        identity(),
        0x1000,
        u32::MAX,
        4,
    )
    .unwrap()
}

#[test]
fn source_registry_order_is_independent_of_sorted_keys_or_weapon_order() {
    let tables = tables();
    assert_eq!(tables.registry_count(), 3);
    let records = (0..3)
        .map(|i| tables.record_at_source_ordinal(i).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        records.iter().map(WeaponRecord::key).collect::<Vec<_>>(),
        [0x020000f0, 0x02000001, 0x02000020]
    );
    assert_eq!(
        records
            .iter()
            .map(WeaponRecord::source_index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        records
            .iter()
            .map(|r| r.preload_enabled_byte().unwrap())
            .collect::<Vec<_>>(),
        [0, 0x80, 0xff]
    );
    assert!(tables.record_at_source_ordinal(3).is_err());
    assert!(tables.record_at_source_ordinal(usize::MAX).is_err());
    // An unrelated unsupported order table is not needed by the source loop.
    assert!(tables.weapon_order(1).is_err());
}

#[test]
fn enabled_reads_exact_byte24_even_when_following_bytes_do_not_exist() {
    let mut bytes = vec![0; 0x25];
    bytes[0x18] = 0;
    bytes[0x24] = 0xff;
    let data = Data {
        address: 0x1000,
        bytes: bytes.into_boxed_slice(),
    };
    let record = WeaponRecord {
        data: &data,
        index: RecordIndex {
            key: 1,
            address: 0x1000,
            ordinal: 0,
        },
    };
    assert_eq!(record.preload_enabled_byte().unwrap(), 0xff);
    assert!(record.word(0x24).is_err()); // A word getter would over-read.
}

#[test]
fn missing_byte_is_error_instead_of_disabled_row() {
    let data = Data {
        address: 0x1000,
        bytes: vec![0; 0x24].into_boxed_slice(),
    };
    let record = WeaponRecord {
        data: &data,
        index: RecordIndex {
            key: 1,
            address: 0x1000,
            ordinal: 0,
        },
    };
    assert!(record.preload_enabled_byte().is_err());
}

#[test]
fn disabled_row_does_not_need_its_maps_or_neighbor_fields() {
    let data = Data {
        address: 0x1000,
        bytes: vec![0; 0x25].into_boxed_slice(),
    };
    let record = WeaponRecord {
        data: &data,
        index: RecordIndex {
            key: 1,
            address: 0x1000,
            ordinal: 0,
        },
    };
    assert_eq!(record.preload_enabled_byte().unwrap(), 0);
}
