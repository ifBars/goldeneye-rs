use super::super::fixtures;
use super::*;

const ROOT: u32 = 0x0181_00a5;
const LINKED: u32 = 0x018b_00a5;
const BYTES: u32 = 0x480;

#[test]
fn retains_actual_owners_in_order_and_forces_coded_index_zero_in_linked_file() {
    let mut calls = Vec::new();
    let residency = Residency::from_loader(ROOT, Budget::default(), |file, maximum| {
        calls.push((file, maximum));
        Ok(if file == ROOT {
            fixtures::resource(file, 0x8500_0000, Some((LINKED, 0x0500_ffff, 3)))
        } else {
            assert_eq!(file, LINKED);
            fixtures::resource(file, 0x0500_00a5, None)
        })
    })
    .unwrap();
    assert_eq!(
        calls.iter().map(|&(file, _)| file).collect::<Vec<_>>(),
        [ROOT, LINKED]
    );
    assert_eq!(
        residency
            .resources()
            .iter()
            .map(|edb| edb.header.hash)
            .collect::<Vec<_>>(),
        [ROOT, LINKED]
    );
    let value = serde_json::to_value(residency.evidence().unwrap()).unwrap();
    assert_eq!(value["complete"], true);
    assert_eq!(value["stop_reason"], "no_link");
    assert!(value["repeated_file"].is_null());
    assert_eq!(value["retained_files"], 2);
    assert_eq!(value["total_payload_bytes"], u64::from(BYTES) * 2);
    assert_eq!(value["resources"][0]["next_link"]["map"], 0x0500_ffff);
    assert_eq!(
        value["resources"][1]["requested_map_reference"],
        0x8500_0000u32
    );
    assert_eq!(value["resources"][1]["resolved_map_hash"], 0x0500_00a5);
    assert_eq!(value["resources"][1]["map_directory_index"], 0);
    let text = serde_json::to_string(&value).unwrap();
    assert!(!text.contains("vertices") && !text.contains("words_raw"));
}

#[test]
fn repeated_file_with_a_different_map_stops_without_loading_a_second_owner() {
    let mut calls = Vec::new();
    let residency = Residency::from_loader(ROOT, Budget::default(), |file, _| {
        calls.push(file);
        Ok(fixtures::resource(
            file,
            0x0500_00a5,
            Some((ROOT, 0x0500_ffff, 3)),
        ))
    })
    .unwrap();
    assert_eq!(calls, [ROOT]);
    assert_eq!(residency.resources().len(), 1);
    let value = serde_json::to_value(residency.evidence().unwrap()).unwrap();
    assert_eq!(value["stop_reason"], "repeated_file");
    assert_eq!(value["repeated_file"], ROOT);
    assert_eq!(value["total_payload_bytes"], BYTES);
}

#[test]
fn aggregate_remaining_allowance_reaches_preflight_before_a_linked_payload_is_allocated() {
    let mut calls = Vec::new();
    let budget = Budget {
        total_bytes: u64::from(BYTES) * 2 - 1,
        ..Budget::default()
    };
    let result = Residency::from_loader(ROOT, budget, |file, maximum| {
        calls.push((file, maximum));
        if file == LINKED {
            assert_eq!(maximum, BYTES - 1);
            // This represents the header preflight rejecting the declared size;
            // no second authored EDB is allocated on this path.
            bail!("declared linked payload exceeds remaining allowance");
        }
        Ok(fixtures::resource(file, 0x8500_0000, Some((LINKED, 0, 3))))
    });
    assert!(result.is_err());
    assert_eq!(calls, [(ROOT, BYTES * 2 - 1), (LINKED, BYTES - 1)]);
}

#[test]
fn exhausted_file_or_header_allowance_prevents_the_next_loader_call() {
    for budget in [
        Budget {
            files: 1,
            ..Budget::default()
        },
        Budget {
            total_bytes: u64::from(BYTES) + 100,
            ..Budget::default()
        },
    ] {
        let mut calls = Vec::new();
        let result = Residency::from_loader(ROOT, budget, |file, _| {
            calls.push(file);
            Ok(fixtures::resource(file, 0x8500_0000, Some((LINKED, 0, 3))))
        });
        assert!(result.is_err());
        assert_eq!(calls, [ROOT]);
    }
}

#[test]
fn missing_file_property_is_a_real_failed_load_request_not_a_successful_stop() {
    let mut calls = Vec::new();
    let result = Residency::from_loader(ROOT, Budget::default(), |file, _| {
        calls.push(file);
        if file == u32::MAX {
            bail!("requested resource is absent");
        }
        Ok(fixtures::resource(file, 0x8500_0000, Some((LINKED, 0, 0))))
    });
    assert!(result.is_err());
    assert_eq!(calls, [ROOT, u32::MAX]);
}

#[test]
fn malformed_or_mismatched_linked_resource_cannot_return_partial_residency() {
    for kind in 0..3 {
        let mut calls = Vec::new();
        let result = Residency::from_loader(ROOT, Budget::default(), |file, _| {
            calls.push(file);
            let mut edb =
                fixtures::resource(file, 0x8500_0000, (file == ROOT).then_some((LINKED, 0, 3)));
            if file == LINKED {
                match kind {
                    0 => edb.header.hash ^= 1,
                    1 => edb.tables[6].count = 0,
                    _ => edb.header.payload_bytes = 64 * 1024 * 1024 + 1,
                }
            }
            Ok(edb)
        });
        assert!(result.is_err());
        assert_eq!(calls, [ROOT, LINKED]);
    }
}
