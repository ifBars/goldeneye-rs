use super::*;

fn word(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x480];
    word(&mut data, 0, 0x4745_4f4d);
    word(&mut data, 4, CATALOG_RESOURCE);
    word(&mut data, 8, 263);
    word(&mut data, 0x14, 0x480);
    word(&mut data, 0x18, 0x480);
    word(&mut data, 0x98, 0x0002_0002);
    word(&mut data, 0x9c, 0x100 - 0x9c);
    for (index, hash, address, block) in [
        (0, PRIMARY_SHEET, 0x180, 0x200),
        (1, SECONDARY_SHEET, 0x190, 0x380),
    ] {
        let at = 0x100 + index * 20;
        word(&mut data, at, hash);
        word(&mut data, at + 8, address);
        word(&mut data, at + 12, u32::MAX);
        word(&mut data, address as usize, 0xfedc_ba98);
        word(&mut data, address as usize + 4, block - address - 4);
    }
    word(&mut data, 0x200, 2);
    word(&mut data, 0x380, 1);
    for row in 0..2 {
        for index in 0..45 {
            word(
                &mut data,
                0x204 + row * 180 + index * 4,
                (row * 100 + index) as u32,
            );
        }
    }
    // A raw NaN-like value and duplicate keys are data, not reader policy errors.
    word(&mut data, 0x204, 9);
    word(&mut data, 0x204 + 180, 9);
    word(&mut data, 0x208, 0x7fc0_0001);
    word(&mut data, 0x20c, u32::MAX);
    for index in 0..29 {
        word(&mut data, 0x384 + index * 4, 200 + index as u32);
    }
    data
}

fn budget() -> Budget {
    Budget {
        primary: 2,
        secondary: 1,
    }
}

#[test]
fn distinct_row_widths_preserve_source_order_duplicates_bits_and_locations() {
    let catalog = LevelCatalog::read(&Edb::parse(fixture()).unwrap(), budget()).unwrap();
    assert_eq!(catalog.primary().len(), 2);
    assert_eq!(catalog.secondary().len(), 1);
    assert_eq!(&catalog.primary()[0][..3], &[9, 0x7fc0_0001, u32::MAX]);
    assert_eq!(catalog.primary()[1][0], 9);
    assert_eq!(catalog.primary()[1][44], 144);
    assert_eq!(catalog.secondary()[0][28], 228);
    assert_eq!(catalog.locations().primary_first_row, 0x204);
    assert_eq!(catalog.locations().secondary_first_row, 0x384);
    let (primary, secondary) = catalog.into_rows();
    assert_eq!((primary.len(), secondary.len()), (2, 1));
}

#[test]
fn required_sheets_and_absent_groups_are_errors_but_present_empty_groups_are_not() {
    for offset in [0x100, 0x114] {
        let mut bytes = fixture();
        word(&mut bytes, offset, 0x1400_ffff);
        assert!(LevelCatalog::read(&Edb::parse(bytes).unwrap(), budget()).is_err());
    }
    for offset in [0x184, 0x194] {
        let mut bytes = fixture();
        word(&mut bytes, offset, 0);
        assert!(LevelCatalog::read(&Edb::parse(bytes).unwrap(), budget()).is_err());
    }
    let mut bytes = fixture();
    word(&mut bytes, 0x200, 0);
    word(&mut bytes, 0x380, 0);
    let catalog = LevelCatalog::read(
        &Edb::parse(bytes).unwrap(),
        Budget {
            primary: 0,
            secondary: 0,
        },
    )
    .unwrap();
    assert!(catalog.primary().is_empty() && catalog.secondary().is_empty());
}

#[test]
fn row_budgets_and_truncated_consumed_rows_fail_before_returning_a_catalog() {
    let edb = Edb::parse(fixture()).unwrap();
    assert!(
        LevelCatalog::read(
            &edb,
            Budget {
                primary: 1,
                secondary: 1
            }
        )
        .is_err()
    );
    assert!(
        LevelCatalog::read(
            &edb,
            Budget {
                primary: 2,
                secondary: 0
            }
        )
        .is_err()
    );
    let mut bytes = fixture();
    word(&mut bytes, 0x380, 3);
    assert!(
        LevelCatalog::read(
            &Edb::parse(bytes).unwrap(),
            Budget {
                primary: 2,
                secondary: 3
            }
        )
        .is_err()
    );
}

#[test]
fn resource_identity_and_group_pointer_bounds_are_checked() {
    let mut bytes = fixture();
    word(&mut bytes, 4, 0x0109_ffff);
    assert!(LevelCatalog::read(&Edb::parse(bytes).unwrap(), budget()).is_err());
    let mut bytes = fixture();
    word(&mut bytes, 0x194, 0x7fff_ffff);
    assert!(LevelCatalog::read(&Edb::parse(bytes).unwrap(), budget()).is_err());
}
