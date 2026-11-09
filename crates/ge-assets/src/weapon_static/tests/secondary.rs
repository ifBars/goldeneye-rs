use super::*;
#[test]
fn secondary_order_reads_its_distinct_supplied_region_preserving_duplicates() {
    let mut fixture = Fixture::new();
    fixture.patch(0x178, WEAPON_A);
    fixture.patch(0x17c, WEAPON_A);
    fixture.patch(0x180, WEAPON_B);
    let tables = fixture.load().unwrap();
    assert_eq!(
        tables.weapon_order(4).unwrap().keys(),
        &[WEAPON_B, WEAPON_A, WEAPON_B]
    );
    assert_eq!(
        tables.secondary_order(4).unwrap().keys(),
        &[WEAPON_A, WEAPON_A, WEAPON_B]
    );
    assert!(tables.secondary_order(3).is_err());
}
#[test]
fn absent_secondary_terminator_is_a_bound_error_not_primary_fallback() {
    let mut fixture = Fixture::new();
    for offset in (0x178..DATA_BYTES).step_by(4) {
        fixture.patch(offset, WEAPON_A);
    }
    let tables = fixture.load().unwrap();
    assert!(tables.secondary_order(128).is_err());
    assert_eq!(tables.weapon_order(4).unwrap().len(), 3);
}
