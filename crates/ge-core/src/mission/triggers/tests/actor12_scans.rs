use super::*;

#[test]
fn rotation_repair_needs_both_ordered_pi_gates_and_preserves_unreached_raw_bits() {
    let pi = 0x4049_0fdb;
    let (repaired, changed) = repair_rotation([pi, 0, pi, 0x8000_0000]);
    assert!(changed);
    assert_eq!(repaired, [0, pi, 0, 0x8000_0000]);
    for raw in [
        [0, 0x8000_0000, pi, 0],
        [pi, 0x7fc0_1234, 0, 0],
        [0x7fc0_1234, 0, pi, 0],
        [pi, 0, 0x7fc0_1234, 0],
    ] {
        assert_eq!(repair_rotation(raw), (raw, false));
    }
}

#[test]
fn four_scans_choose_first_storage_match_and_keep_completed_prefix_on_blocker() {
    let a = TriggerId::new(1).unwrap();
    let b = TriggerId::new(2).unwrap();
    let mut state = State::fresh([0; 4], 0, 0);
    let mut references = [ScanReference::Null; 7];
    references[0] = ScanReference::Target {
        trigger: a,
        subtype: 0x1f00_00a5,
    };
    references[1] = ScanReference::Target {
        trigger: b,
        subtype: 0x1f00_00a5,
    };
    references[2] = ScanReference::Target {
        trigger: b,
        subtype: 0x1f00_00a9,
    };
    state.initialize_map(&references);
    assert_eq!(state.scan_matches(), [Some(a), Some(b), None, None]);
    assert_eq!(state.map_initialization(), MapInitialization::Complete);
    let mut blocked = State::fresh([0; 4], 0, 0);
    references[1] = ScanReference::Blocked { ordinal: 9 };
    blocked.initialize_map(&references);
    assert_eq!(blocked.scan_matches(), [Some(a), None, None, None]);
    assert_eq!(
        blocked.map_initialization(),
        MapInitialization::Blocked {
            scan: 1,
            ordinal: Some(9)
        }
    );
}
