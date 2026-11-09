use super::*;

fn request(file: u32) -> Request {
    Request {
        file,
        map_reference: 0x8500_0000,
    }
}

#[test]
fn declared_snapshot_prepares_order_flags_and_allowlists_without_constructing_worlds() {
    let incoming = [
        request(0x018b_00a5),
        request(0x0181_00a5),
        request(0x01a4_00a5),
    ];
    let prepared = PreparedRegistration::from_initialization_snapshot(&incoming).unwrap();
    assert_eq!(prepared.plan().incoming(), &incoming);
    let rows = &prepared.evidence().rows;
    assert_eq!(
        rows.iter()
            .map(|row| row.source_ordinal)
            .collect::<Vec<_>>(),
        [1, 0, 2]
    );
    assert_eq!(rows[0].flags_raw, [0xff, 0, 1]);
    assert_eq!(rows[1].flags_raw, [0xff, 1, 1]);
    assert_eq!(rows[2].flags_raw, [0xff, 0, 1]);
    assert_eq!(rows[0].subtype_allowlist, None);
    assert_eq!(
        rows[1].subtype_allowlist.as_deref(),
        Some([0x1f00_0088].as_slice())
    );
    assert_eq!(rows[2].subtype_allowlist.as_ref().unwrap().len(), 6);
    assert!(!prepared.evidence().native_init_timing_verified);
}

#[test]
fn a_single_resource_has_no_subtype_filter_and_empty_input_is_an_error() {
    let prepared =
        PreparedRegistration::from_initialization_snapshot(&[request(0x1234_00a5)]).unwrap();
    assert_eq!(prepared.evidence().rows[0].flags_raw, [0xff, 1, 1]);
    assert_eq!(prepared.evidence().rows[0].subtype_allowlist, None);
    assert!(PreparedRegistration::from_initialization_snapshot(&[]).is_err());
}
