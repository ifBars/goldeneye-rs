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
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x400];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, RESOURCE),
        (8, 263),
        (0x14, 0x400),
        (0x18, 0x400),
        (0x98, 0x0001_0001),
        (0x9c, 0x100 - 0x9c),
        (0x100, SHEET),
        (0x108, 0x180),
        (0x10c, u32::MAX),
        (0x180, 0xffff_ffff),
        (0x200, 2),
        (0x300, 2),
    ] {
        put(&mut data, at, value);
    }
    relative(&mut data, 0x184, 0x200);
    relative(&mut data, 0x188, 0x300);
    for (index, words) in [
        [
            0x4d00_00a5,
            0x4d00_0008,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            9,
            10,
            11,
            12,
            13,
            14,
            0x7fc0_00a5,
            u32::MAX,
        ],
        [
            0x4d00_00a5,
            0x4d00_0009,
            12,
            13,
            14,
            15,
            16,
            17,
            18,
            19,
            20,
            21,
            22,
            23,
            24,
            0x8000_0000,
            26,
        ],
    ]
    .iter()
    .enumerate()
    {
        for (i, &word) in words.iter().enumerate() {
            put(&mut data, 0x204 + index * 0x44 + i * 4, word);
        }
    }
    for (index, words) in [[0xa5, 1, 2, 3, 4, 5], [0xb6, 6, 7, 8, 9, u32::MAX]]
        .iter()
        .enumerate()
    {
        for (i, &word) in words.iter().enumerate() {
            put(&mut data, 0x304 + index * 0x18 + i * 4, word);
        }
    }
    data
}

#[test]
fn first_and_last_rows_preserve_typed_strides_words_keys_and_source_locations() {
    let edb = Edb::parse(fixture()).unwrap();
    let definitions = Definitions::read(&edb, Budget::default()).unwrap();
    assert_eq!(
        (definitions.primary_count(), definitions.secondary_count()),
        (Some(2), Some(2))
    );
    let first = definitions.primary(0).unwrap();
    let last = definitions.primary(1).unwrap();
    assert_eq!(
        (first.key(), first.group_word(), first.source_address()),
        (0x4d00_00a5, 0x4d00_0008, 0x204)
    );
    assert_eq!(
        (
            last.source_index(),
            last.source_address(),
            last.source_group()
        ),
        (1, 0x248, 0)
    );
    assert_eq!(first.words()[15..], [0x7fc0_00a5, u32::MAX]);
    assert_eq!(last.words()[15], 0x8000_0000);
    let secondary = definitions.secondary(1).unwrap();
    assert_eq!(
        (
            secondary.key(),
            secondary.source_address(),
            secondary.source_group()
        ),
        (0xb6, 0x31c, 1)
    );
    assert_eq!(secondary.words(), &[0xb6, 6, 7, 8, 9, u32::MAX]);
    assert!(definitions.primary(2).is_err());
    assert!(definitions.secondary(2).is_err());
}

#[test]
fn duplicates_are_not_removed_or_reordered_by_the_asset_reader() {
    let edb = Edb::parse(fixture()).unwrap();
    let view = Definitions::read(&edb, Budget::default()).unwrap();
    assert_eq!(
        view.primary(0).unwrap().key(),
        view.primary(1).unwrap().key()
    );
    assert_ne!(
        view.primary(0).unwrap().group_word(),
        view.primary(1).unwrap().group_word()
    );
}

#[test]
fn missing_and_present_zero_groups_remain_distinct_without_fabricated_defaults() {
    let mut data = fixture();
    put(&mut data, 0x184, 0);
    put(&mut data, 0x300, 0);
    let edb = Edb::parse(data).unwrap();
    let view = Definitions::read(
        &edb,
        Budget {
            primary_rows: 0,
            secondary_rows: 0,
            total_words: 0,
            total_bytes: 0,
        },
    )
    .unwrap();
    assert_eq!(
        (view.primary_count(), view.secondary_count()),
        (None, Some(0))
    );
    assert!(view.primary_locator().is_none());
    assert!(view.secondary_locator().is_some());
    assert!(view.primary(0).is_err());
    assert!(view.secondary(0).is_err());
}

#[test]
fn signed_field_relative_group_addressing_supports_earlier_blocks() {
    let mut data = fixture();
    relative(&mut data, 0x184, 0x120);
    put(&mut data, 0x120, 1);
    put(&mut data, 0x124, 0x4d00_00d7);
    let edb = Edb::parse(data).unwrap();
    let view = Definitions::read(&edb, Budget::default()).unwrap();
    assert_eq!(view.primary_locator().unwrap().relative_raw, -0x64);
    assert_eq!(view.primary(0).unwrap().key(), 0x4d00_00d7);
}

#[test]
fn per_group_and_aggregate_budgets_bound_the_source_rows() {
    let edb = Edb::parse(fixture()).unwrap();
    for budget in [
        Budget {
            primary_rows: 1,
            ..Budget::default()
        },
        Budget {
            secondary_rows: 1,
            ..Budget::default()
        },
        Budget {
            total_words: 45,
            ..Budget::default()
        },
        Budget {
            total_bytes: 183,
            ..Budget::default()
        },
    ] {
        assert!(Definitions::read(&edb, budget).is_err());
    }
    assert!(
        Definitions::read(
            &edb,
            Budget {
                primary_rows: 2,
                secondary_rows: 2,
                total_words: 46,
                total_bytes: 184
            }
        )
        .is_ok()
    );
}

#[test]
fn unsupported_identity_and_missing_sheet_are_errors() {
    for (at, value) in [(4, 0x0109_ffff), (0x100, 0x1400_ffff)] {
        let mut data = fixture();
        put(&mut data, at, value);
        assert!(Definitions::read(&Edb::parse(data).unwrap(), Budget::default()).is_err());
    }
    let mut edb = Edb::parse(fixture()).unwrap();
    edb.header.version = 264;
    assert!(Definitions::read(&edb, Budget::default()).is_err());
}

#[test]
fn reached_bad_relative_count_and_truncated_row_spans_are_errors() {
    for (at, value) in [
        (0x184, 0x8000_0000),
        (0x188, 0x7fff_ffff),
        (0x200, u32::MAX),
        (0x300, 0x100),
    ] {
        let mut data = fixture();
        put(&mut data, at, value);
        assert!(Definitions::read(&Edb::parse(data).unwrap(), Budget::default()).is_err());
    }
    let mut data = fixture();
    // The two-row secondary span needs through0x334, not merely its count.
    data.truncate(0x333);
    put(&mut data, 0x14, 0x333);
    put(&mut data, 0x18, 0x333);
    assert!(Definitions::read(&Edb::parse(data).unwrap(), Budget::default()).is_err());
}
