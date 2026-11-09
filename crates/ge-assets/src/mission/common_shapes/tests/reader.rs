use super::*;

fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    put(
        data,
        field,
        i32::try_from(target as i64 - field as i64).unwrap() as u32,
    );
}
fn fixture() -> (Vec<u8>, Table) {
    let mut data = vec![0; 0x227];
    let maps = Table {
        name: "maps",
        count: 1,
        hash_size_raw: 1,
        offset: 0x20,
    };
    put(&mut data, 0x20, 0x0500_00a5);
    put(&mut data, 0x28, 0x80);
    put(&mut data, 0x80, 0x500);
    relative(&mut data, 0xd8, 0x180);
    relative(&mut data, 0x190, 0x1c0);
    for at in [0x1c0, 0x1f0, 0x220] {
        put(&mut data, at, u32::MAX);
    }
    (data, maps)
}

#[test]
fn zero_selector_is_present_and_coded_or_literal_map_resolves_same_descriptor() {
    let (data, maps) = fixture();
    let coded = read(&data, &maps, 0x8500_0000, 0, Budget::default())
        .unwrap()
        .unwrap();
    let literal = read(&data, &maps, 0x0500_00a5, 0, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(coded.raw_selector, 0);
    assert_eq!(coded.type_raw, 0);
    assert_eq!(coded.locator.descriptor_address, 0x1c0);
    assert_eq!(coded.locator.trigger_block, 0x180);
    assert_eq!(coded.locator.canonical_map_reference, 0x8500_0000);
    assert_eq!(
        literal.locator.descriptor_address,
        coded.locator.descriptor_address
    );
    assert_eq!(literal.locator.map_hash, 0x0500_00a5);
}

#[test]
fn selector_stride_is_48_and_only_seven_byte_prefix_is_required() {
    let (data, maps) = fixture();
    let binding = read(&data, &maps, 0, 2, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(binding.locator.descriptor_address, 0x220);
    assert_eq!(data.len(), binding.locator.descriptor_address + 7);
    // No dimensions, whole-row span, trigger count or schema bytes are needed.
}

#[test]
fn signed_relative_pointers_can_address_earlier_valid_blocks_and_directories() {
    let (mut data, maps) = fixture();
    relative(&mut data, 0xd8, 0x60);
    relative(&mut data, 0x70, 0x40);
    data[0x46] = 0;
    let binding = read(&data, &maps, 0, 0, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            binding.locator.trigger_block,
            binding.locator.shape_directory
        ),
        (0x60, 0x40)
    );
}

#[test]
fn map_absence_skips_all_shape_addressing_and_is_the_only_none() {
    let (mut data, maps) = fixture();
    put(&mut data, 0x28, u32::MAX);
    assert!(
        read(&data, &maps, 0x8500_0001, u32::MAX, Budget::default())
            .unwrap()
            .is_none()
    );
    assert!(
        read(&data, &maps, 0x0500_00ff, 0, Budget::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn negative_selector_is_typed_unsupported_not_absence() {
    let (mut data, maps) = fixture();
    put(&mut data, 0xd8, 0);
    for raw_selector in [u32::MAX, 0x8000_0000] {
        let error = read(&data, &maps, 0, raw_selector, Budget::default()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<Unsupported>(),
            Some(&Unsupported::Selector { raw_selector })
        );
    }
}

#[test]
fn reached_nonzero_types_are_typed_scope_errors_without_dimension_reads() {
    let (mut data, maps) = fixture();
    data.truncate(0x1c7);
    for type_raw in [1, 3, 4, 0xff] {
        data[0x1c6] = type_raw;
        let error = read(&data, &maps, 0, 0, Budget::default()).unwrap_err();
        assert!(
            matches!(error.downcast_ref::<Unsupported>(), Some(Unsupported::Type { type_raw: value, locator }) if *value == type_raw && locator.descriptor_address == 0x1c0)
        );
    }
}

#[test]
fn reached_null_bad_relative_and_truncated_type_are_malformed() {
    for (field, raw) in [
        (0xd8, 0),
        (0x190, 0),
        (0xd8, 0x8000_0000),
        (0x190, 0x7fff_ffff),
        (0x190, (-0x190i32) as u32),
    ] {
        let (mut data, maps) = fixture();
        put(&mut data, field, raw);
        let error = read(&data, &maps, 0, 0, Budget::default()).unwrap_err();
        assert!(error.downcast_ref::<Unsupported>().is_none());
    }
    let (mut data, maps) = fixture();
    data.truncate(0x226);
    assert!(read(&data, &maps, 0, 2, Budget::default()).is_err());
}

#[test]
fn host_budgets_do_not_claim_a_native_shape_count() {
    let (data, maps) = fixture();
    for budget in [
        Budget {
            maps: 0,
            ..Budget::default()
        },
        Budget {
            selectors: 2,
            ..Budget::default()
        },
        Budget {
            addressed_bytes: 6,
            ..Budget::default()
        },
    ] {
        let error = read(&data, &maps, 0, 2, budget).unwrap_err();
        assert!(error.to_string().contains("budget"));
    }
    assert!(
        read(
            &data,
            &maps,
            0,
            2,
            Budget {
                selectors: 3,
                addressed_bytes: 7,
                ..Budget::default()
            }
        )
        .unwrap()
        .is_some()
    );
}
