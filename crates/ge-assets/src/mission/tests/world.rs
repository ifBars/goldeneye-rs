use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> (Vec<u8>, Table) {
    let mut data = vec![0; 0x204];
    word(&mut data, 0x10, 0x0500_00a5);
    word(&mut data, 0x18, 0x100);
    word(&mut data, 0x100, 0x500);
    for (index, bits) in [
        0x7fc0_00a5,
        (-3.0f32).to_bits(),
        0x8000_0000,
        (-9.0f32).to_bits(),
        2.0f32.to_bits(),
        0x0000_0000,
    ]
    .into_iter()
    .enumerate()
    {
        word(&mut data, 0x16c + index * 4, bits);
    }
    (
        data,
        Table {
            name: "maps",
            count: 1,
            hash_size_raw: 1,
            offset: 0x10,
        },
    )
}

#[test]
fn literal_and_coded_map_references_normalize_to_the_same_native_key() {
    let (data, table) = fixture();
    let literal = read(&data, &table, 0x0500_00a5, Budget { maps: 1 })
        .unwrap()
        .unwrap();
    let coded = read(&data, &table, 0x8500_0000, Budget { maps: 1 })
        .unwrap()
        .unwrap();
    assert_eq!(literal.canonical_reference, coded.canonical_reference);
    assert_eq!(literal.canonical_reference, 0x8500_0000);
    assert_eq!(coded.stored_hash, 0x0500_00a5);
    assert_eq!(
        literal.declared_bounds_bits,
        [
            [0x7fc0_00a5, (-3.0f32).to_bits(), 0x8000_0000],
            [(-9.0f32).to_bits(), 2.0f32.to_bits(), 0]
        ]
    );
}

#[test]
fn descriptor_lookup_obeys_budgets_absence_and_headerless_trigger_data() {
    let (data, table) = fixture();
    assert!(read(&data, &table, 0x8500_0000, Budget { maps: 0 }).is_err());
    assert!(
        read(&data, &table, 0x0500_00a6, Budget { maps: 1 })
            .unwrap()
            .is_none()
    );
    // Trigger/BSP/placement pointers are absent and are not addressed here.
    assert!(
        read(&data[..0x184], &table, 0x8500_0000, Budget { maps: 1 })
            .unwrap()
            .is_some()
    );
    assert!(read(&data[..0x183], &table, 0x8500_0000, Budget { maps: 1 }).is_err());
    let mut bad = data;
    word(&mut bad, 0x100, 0x603);
    assert!(read(&bad, &table, 0x8500_0000, Budget { maps: 1 }).is_err());
}
