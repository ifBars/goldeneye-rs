use super::*;
use ge_core::animation::mode::{CleanupIntent, CommandPlan, Insertion};

fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(bytes: &mut [u8], at: usize, target: usize) {
    word(bytes, at, (target as i32 - at as i32) as u32);
}
fn component(unsupported: bool) -> Edb {
    let mut bytes = vec![0; 0x400];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_0016),
        (8, 263),
        (0x14, 0x400),
        (0x18, 0x400),
        (0x78, 0x0002_0002),
        (0x80, 0x0001_0001),
        (0x100, 0x0900_00a5),
        (0x108, 0x200),
        (0x110, 2),
        (0x114, 0x0900_00a6),
        (0x118, u32::MAX),
        (0x11c, u32::MAX),
        (0x180, 0x0a00_00a5),
        (0x188, 0x300),
        (0x190, 1),
        (0x200, 1),
        (0x208, 1),
        (0x20c, u32::MAX),
        (0x240, 2),
        (
            0x280,
            if unsupported {
                0x0c00_00ff
            } else {
                0x0c00_0005
            },
        ),
        (0x290, 0x0c00_0002),
        (0x294, 0),
        (0x304, 0),
        (0x308, 0x8400_00a5),
        (0x310, 0x7fc0_00a5),
    ] {
        word(&mut bytes, at, value);
    }
    relative(&mut bytes, 0x7c, 0x100);
    relative(&mut bytes, 0x84, 0x180);
    relative(&mut bytes, 0x204, 0x240);
    relative(&mut bytes, 0x244, 0x280);
    relative(&mut bytes, 0x248, 0x290);
    half(&mut bytes, 0x288, 3);
    half(&mut bytes, 0x300, 1);
    half(&mut bytes, 0x302, 1);
    half(&mut bytes, 0x30c, 0x27);
    half(&mut bytes, 0x30e, 6);
    Edb::parse(bytes).unwrap()
}

#[test]
fn retained_owner_drives_real_ordered_plan_without_reading_target_body_or_later_duplicate() {
    let owner = component(false);
    let prepared = PreparedAnimation::prepare(&owner, 0x0900_00a6, 0, 0x09ff_ffff).unwrap();
    let selected = &prepared.selection;
    assert_eq!(selected.target_index, Some(1));
    assert_eq!(selected.return_word, 0x0900_00a5);
    assert_eq!(selected.attempts.len(), 1);
    let commands = &selected.attempts[0].commands;
    assert!(matches!(
        commands[0],
        CommandPlan::MaskOperation { mask_raw: 3, .. }
    ));
    let CommandPlan::StartSet(set) = &commands[1] else {
        panic!("missing start set")
    };
    assert_eq!(set.cleanup, CleanupIntent::SuppressedByFlag20);
    assert_eq!(set.groups[0].source_slot, 1);
    let child = &set.groups[0].children[0];
    assert_eq!(child.effective_flags, 0x27);
    assert_eq!(child.divisor_u32, 6);
    assert_eq!(child.reciprocal_divisor_bits, (1f32 / 6.).to_bits());
    assert_eq!(child.insertion, Insertion::Head);
    let evidence = prepared.evidence();
    assert_eq!(
        (
            evidence.planned_commands,
            evidence.mask_operations,
            evidence.start_sets
        ),
        (2, 1, 1)
    );
    assert_eq!((evidence.layer_groups, evidence.child_requests), (1, 1));
    assert!(!evidence.initial_request_verified && !evidence.runtime_children_created);
    assert!(!evidence.animation_sampled && !evidence.camera_frame_ready);
    let json = serde_json::to_value(evidence).unwrap();
    for field in ["commands", "children", "groups", "pose", "transforms"] {
        assert!(json.get(field).is_none());
    }
}

#[test]
fn fallback_missing_target_and_unsupported_reached_data_are_distinct() {
    let owner = component(false);
    let fallback = PreparedAnimation::prepare(&owner, 1, 0x09ff_ffff, 0).unwrap();
    assert_eq!(fallback.evidence().fallback_matches, 1);
    assert_eq!(fallback.evidence().previous_matches, 0);
    let missing = PreparedAnimation::prepare(&owner, 0x09ff_fffe, 0, 0).unwrap();
    assert_eq!(missing.evidence().state, "target_missing");
    assert_eq!(missing.evidence().attempts, 0);
    assert_eq!(missing.evidence().planned_commands, 0);
    assert!(PreparedAnimation::prepare(&component(true), 1, 0, 0).is_err());
    assert!(PreparedAnimation::prepare(&component(true), 0x09ff_fffe, 0, 0).is_ok());
}

#[test]
fn adapter_allocation_allowances_are_aggregate_across_reached_sets() {
    let owner = component(false);
    let mut source = provider::Source::new(
        owner.animation_modes().unwrap(),
        mode::Budget {
            lookups: 3,
            transition_rows: 2,
            commands: 2,
            groups: 1,
            children: 0,
        },
    );
    assert!(
        mode::select(
            &mut source,
            Inputs {
                requested: 1,
                previous: 0,
                fallback: 0,
                resource_available: true,
            },
            mode::Budget {
                lookups: 3,
                transition_rows: 2,
                commands: 2,
                groups: 1,
                children: 1
            }
        )
        .is_err()
    );
}
