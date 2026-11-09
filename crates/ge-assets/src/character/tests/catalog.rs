use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, CATALOG_RESOURCE),
        (8, 263),
        (0x14, 0x300),
        (0x18, 0x300),
        (0x98, 0x0001_0001),
        (0x9c, 0x100 - 0x9c),
        (0x100, CHARACTER_SHEET),
        (0x108, 0x180),
        (0x10c, u32::MAX),
        (0x180, 0xaabb_ccdd),
        // Group zero is a different, unconsumed layout with a bad pointer.
        (0x184, 0x7fff_ffff),
        (0x188, 0x200 - 0x188),
        (0x200, 3),
    ] {
        word(&mut data, at, value);
    }
    for (row, words) in [
        [0xf000_0001, 0x010a_0071, 3, 4, 0x7fc0_0012, 6],
        [0x4d00_00b2, 0x010a_0072, 7, 8, 9, u32::MAX],
        [0xf000_0001, 0x010a_0073, 10, 11, 12, 13],
    ]
    .iter()
    .enumerate()
    {
        for (i, value) in words.iter().enumerate() {
            word(&mut data, 0x204 + row * 24 + i * 4, *value);
        }
    }
    data
}

#[test]
fn reads_only_group_one_and_preserves_first_duplicate_and_provenance() {
    let edb = Edb::parse(fixture()).unwrap();
    let catalog = CharacterCatalog::read(&edb, Budget { rows: 3 }).unwrap();
    assert_eq!(catalog.rows().len(), 2);
    assert_eq!(catalog.duplicate_count(), 1);
    let first = catalog.find(0xf000_0001).unwrap();
    assert_eq!(first.fp_skin_file(), 0x010a_0071);
    assert_eq!(first.words_raw()[4], 0x7fc0_0012);
    assert_eq!((first.source_index(), first.source_address()), (0, 0x204));
    let second = catalog.find(0x4d00_00b2).unwrap();
    assert_eq!(
        (second.key(), second.source_index(), second.source_address()),
        (0x4d00_00b2, 1, 0x21c)
    );
    assert!(catalog.find(0x4d00_ffff).is_none());
}

#[test]
fn absent_group_errors_but_present_empty_group_is_a_catalog() {
    let mut data = fixture();
    word(&mut data, 0x188, 0);
    assert!(CharacterCatalog::read(&Edb::parse(data).unwrap(), Budget { rows: 3 }).is_err());
    let mut data = fixture();
    word(&mut data, 0x200, 0);
    let catalog = CharacterCatalog::read(&Edb::parse(data).unwrap(), Budget { rows: 0 }).unwrap();
    assert!(catalog.rows().is_empty());
    assert!(catalog.find(0).is_none());
}

#[test]
fn identity_missing_sheet_budget_and_reached_truncation_are_errors() {
    for (at, value) in [
        (4, 0x0109_ffff),
        (0x100, 0x1400_ffff),
        (0x200, 100),
        (0x188, 0x7fff_ffff),
    ] {
        let mut data = fixture();
        word(&mut data, at, value);
        assert!(CharacterCatalog::read(&Edb::parse(data).unwrap(), Budget { rows: 100 }).is_err());
    }
    assert!(CharacterCatalog::read(&Edb::parse(fixture()).unwrap(), Budget { rows: 2 }).is_err());
}
