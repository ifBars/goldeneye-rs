use super::*;
use crate::edb::Edb;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i64 - field as i64) as i32 as u32);
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x400];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00f1),
        (8, 263),
        (0x14, 0x400),
        (0x18, 0x400),
        (0x78, 0x0003_0003),
        (0x80, 0x0001_0001),
        (0x100, 0x0900_0011),
        (0x108, 0x200),
        (0x10c, u32::MAX),
        (0x110, 3),
        // Count zero must ignore this source's unsupported section/body.
        (0x114, 0x0900_0022),
        (0x118, 0x0003_1234),
        (0x11c, u32::MAX),
        // A target may resolve without consuming its section/body.
        (0x128, 0x0900_0033),
        (0x12c, 0x0004_abcd),
        (0x130, u32::MAX),
        (0x138, 1),
        (0x160, 0x0a00_0011),
        (0x168, 0x300),
        (0x16c, u32::MAX),
        (0x170, 2),
        (0x200, 1),
        (0x204, 0x7fff_ffff),
        (0x208, 2),
        (0x210, 2),
        (0x214, 0x7fff_ffff),
        (0x260, 2),
        (0x2a0, 0x0c00_0005),
        (0x2a4, u32::MAX),
        (0x2b0, 0x0c00_0002),
        (0x2b4, 0),
        (0x2bc, 0xdead_beef),
        (0x304, 0x7fc0_0001),
        (0x308, 0x8400_0077),
        (0x310, 0xdead_beef),
        (0x318, 0x8000_0000),
    ] {
        word(&mut data, at, value);
    }
    for (field, target) in [
        (0x7c, 0x100),
        (0x84, 0x160),
        (0x20c, 0x260),
        (0x264, 0x2a0),
        (0x268, 0x2b0),
    ] {
        relative(&mut data, field, target);
    }
    half(&mut data, 0x2a8, 3);
    half(&mut data, 0x2b8, 0x1234);
    half(&mut data, 0x300, 2);
    half(&mut data, 0x302, 1);
    half(&mut data, 0x30c, 0xffff);
    half(&mut data, 0x30e, 0xffff);
    data
}
fn budget() -> Budget {
    Budget {
        commands: 2,
        groups: 2,
        children: 1,
    }
}

#[test]
fn command_location_and_one_command_leave_a_later_bad_pointer_unread() {
    let mut data = fixture();
    word(&mut data, 0x268, 0x7fff_ffff);
    let edb = Edb::parse(data).unwrap();
    let view = edb.base_animation_modes().unwrap();
    assert_eq!(view.command_count(0, 1, 2).unwrap(), 2);
    let first = view.command_location(0, 1, 0, 2).unwrap();
    assert_eq!(first.opcode(), 0x0c00_0005);
    assert!(matches!(
        view.command(0, 1, 0, 2).unwrap(),
        Command::MaskOperation { mask_raw: 3, .. }
    ));
    assert!(view.command_location(0, 1, 1, 2).is_err());
    assert!(view.commands(0, 1, budget()).is_err());
}
#[test]
fn cached_payload_does_not_extend_live_mode_base_residency() {
    let mut data = fixture();
    word(&mut data, 0x18, 0x2b0);
    let edb = Edb::parse(data).unwrap();
    let base = edb.base_animation_modes().unwrap();
    assert!(base.command_location(0, 1, 0, 2).is_ok());
    assert!(base.command_location(0, 1, 1, 2).is_err());
    assert!(
        edb.animation_modes()
            .unwrap()
            .command_location(0, 1, 1, 2)
            .is_ok()
    );
}
#[test]
fn unknown_command_opcode_can_be_retained_before_its_execution_provider() {
    let mut data = fixture();
    word(&mut data, 0x2a0, 0x0c00_00ff);
    let edb = Edb::parse(data).unwrap();
    let view = edb.base_animation_modes().unwrap();
    assert_eq!(
        view.command_location(0, 1, 0, 2).unwrap().opcode(),
        0x0c00_00ff
    );
    assert!(view.command(0, 1, 0, 2).is_err());
}
fn truncate(data: &mut Vec<u8>, length: usize) {
    data.truncate(length);
    word(data, 0x14, length as u32);
    word(data, 0x18, length as u32);
}

#[test]
fn resolves_coded_literal_and_missing_targets_without_body_section_reads() {
    let edb = Edb::parse(fixture()).unwrap();
    let view = edb.animation_modes().unwrap();
    assert_eq!(view.resolve_mode(0).unwrap(), Some(0));
    assert_eq!(view.resolve_mode(0x8900_0002).unwrap(), Some(2));
    assert_eq!(view.resolve_mode(0x0900_0033).unwrap(), Some(2));
    assert_eq!(view.resolve_mode(0x0900_ffff).unwrap(), None);
    assert_eq!(view.resolve_mode(3).unwrap(), None);
    assert_eq!(view.mode(2).unwrap().hash, 0x0900_0033);
    assert!(
        view.transition_target(2, 0)
            .unwrap_err()
            .to_string()
            .contains("unsupported reached")
    );
    assert!(view.mode(3).is_err());
}

#[test]
fn requested_coded_index_needs_no_descriptor_bytes() {
    let mut data = fixture();
    // Header parser checks only the directory's first word. Index2 descriptor
    // is intentionally absent, but native coded index resolution remains valid.
    truncate(&mut data, 0x164);
    relative(&mut data, 0x7c, 0x160);
    let edb = Edb::parse(data).unwrap();
    let view = edb.animation_modes().unwrap();
    assert_eq!(view.resolve_mode(0x8900_0002).unwrap(), Some(2));
    assert!(view.mode(2).is_err());
}

#[test]
fn only_matched_row_commands_are_consumed_and_source_order_is_preserved() {
    let edb = Edb::parse(fixture()).unwrap();
    let view = edb.animation_modes().unwrap();
    assert_eq!(view.transition_target(0, 0).unwrap(), 1);
    assert_eq!(view.transition_target(0, 1).unwrap(), 2);
    assert_eq!(view.transition_target(0, 2).unwrap(), 2);
    let commands = view.commands(0, 1, budget()).unwrap();
    assert_eq!(
        commands,
        vec![
            Command::MaskOperation {
                source_address: 0x2a0,
                mask_raw: 3
            },
            Command::StartSet {
                source_address: 0x2b0,
                set_reference: 0,
                flags_override: 0x1234,
                options_raw: 0,
                divisor_override_raw: 0
            },
        ]
    );
    assert!(view.commands(0, 0, budget()).is_err());
    assert!(view.commands(0, 2, budget()).is_err());
}

#[test]
fn empty_sources_and_empty_command_lists_skip_unconsumed_fields() {
    let mut data = fixture();
    word(&mut data, 0x260, 0);
    word(&mut data, 0x264, 0x7fff_ffff);
    word(&mut data, 0x170, 0);
    word(&mut data, 0x164, 0x7fff_ffff);
    word(&mut data, 0x168, u32::MAX);
    let edb = Edb::parse(data).unwrap();
    let view = edb.animation_modes().unwrap();
    assert_eq!(view.mode(1).unwrap().count, 0);
    assert!(view.transition_target(1, 0).is_err());
    assert!(
        view.commands(
            0,
            1,
            Budget {
                commands: 0,
                ..budget()
            }
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        view.set(
            0,
            Budget {
                groups: 0,
                children: 0,
                ..budget()
            }
        )
        .unwrap()
        .unwrap()
        .groups
        .is_empty()
    );
}

#[test]
fn command_optional_fields_are_lazy_and_reached_opcodes_are_explicit() {
    let mut with_override = fixture();
    half(&mut with_override, 0x2ba, 0x8001);
    word(&mut with_override, 0x2bc, 0x7fc0_0123);
    let edb = Edb::parse(with_override).unwrap();
    assert_eq!(
        edb.animation_modes()
            .unwrap()
            .commands(0, 1, budget())
            .unwrap()[1],
        Command::StartSet {
            source_address: 0x2b0,
            set_reference: 0,
            flags_override: 0x1234,
            options_raw: 0x8001u16 as i16,
            divisor_override_raw: 0x7fc0_0123,
        }
    );
    let mut data = fixture();
    truncate(&mut data, 0x2bc);
    let edb = Edb::parse(data.clone()).unwrap();
    assert!(
        edb.animation_modes()
            .unwrap()
            .commands(0, 1, budget())
            .is_ok()
    );
    half(&mut data, 0x2ba, 1);
    assert!(
        Edb::parse(data)
            .unwrap()
            .animation_modes()
            .unwrap()
            .commands(0, 1, budget())
            .is_err()
    );
    let mut data = fixture();
    word(&mut data, 0x260, 1);
    truncate(&mut data, 0x2aa);
    assert_eq!(
        Edb::parse(data)
            .unwrap()
            .animation_modes()
            .unwrap()
            .commands(0, 1, budget())
            .unwrap()
            .len(),
        1
    );
    let mut data = fixture();
    word(&mut data, 0x2a0, 0x0c00_0003);
    let error = Edb::parse(data)
        .unwrap()
        .animation_modes()
        .unwrap()
        .commands(0, 1, budget())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported reached animation opcode 0x0c000003")
    );
}

#[test]
fn command_budgets_absent_pointers_and_signed_relative_offsets_are_checked() {
    let edb = Edb::parse(fixture()).unwrap();
    assert!(
        edb.animation_modes()
            .unwrap()
            .commands(
                0,
                1,
                Budget {
                    commands: 1,
                    ..budget()
                }
            )
            .is_err()
    );
    for field in [0x20c, 0x264] {
        let mut data = fixture();
        word(&mut data, field, 0);
        assert!(
            Edb::parse(data)
                .unwrap()
                .animation_modes()
                .unwrap()
                .commands(0, 1, budget())
                .is_err()
        );
    }
    let mut data = fixture();
    word(&mut data, 0x260, 1);
    relative(&mut data, 0x264, 0x220);
    word(&mut data, 0x220, 0x0c00_0005);
    half(&mut data, 0x228, 0x8001);
    assert_eq!(
        Edb::parse(data)
            .unwrap()
            .animation_modes()
            .unwrap()
            .commands(0, 1, budget())
            .unwrap(),
        vec![Command::MaskOperation {
            source_address: 0x220,
            mask_raw: 0x8001
        }]
    );
}

#[test]
fn variable_groups_children_and_all_raw_fields_are_retained_in_order() {
    let edb = Edb::parse(fixture()).unwrap();
    let view = edb.animation_modes().unwrap();
    let set = view.set(0x0a00_0011, budget()).unwrap().unwrap();
    assert_eq!(set.descriptor.directory_index, 0);
    assert_eq!(set.groups.len(), 2);
    assert_eq!(
        (set.groups[0].source_address, set.groups[1].source_address),
        (0x300, 0x314)
    );
    assert_eq!(
        (set.groups[0].slot, set.groups[0].weight_bits),
        (2, 0x7fc0_0001)
    );
    assert_eq!(
        set.groups[0].children,
        vec![Child {
            source_address: 0x308,
            reference: 0x8400_0077,
            flags_raw: 0xffff,
            divisor_raw: 0xffff,
            word8_raw: 0xdead_beef
        }]
    );
    assert!(set.groups[1].children.is_empty());
    assert_eq!(set.groups[1].weight_bits, 0x8000_0000);
    assert_eq!(view.set(0x8a00_0000, budget()).unwrap(), Some(set));
    assert!(view.set(1, budget()).unwrap().is_none());
}

#[test]
fn set_rejects_negative_counts_sections_budgets_and_reached_truncation() {
    let mut aggregate = fixture();
    half(&mut aggregate, 0x316, 1);
    assert!(
        Edb::parse(aggregate)
            .unwrap()
            .animation_modes()
            .unwrap()
            .set(0, budget())
            .is_err()
    );
    for (at, value) in [(0x300, 0x0002_ffff), (0x164, 0x0001_0000), (0x168, 0x3fe)] {
        let mut data = fixture();
        word(&mut data, at, value);
        assert!(
            Edb::parse(data)
                .unwrap()
                .animation_modes()
                .unwrap()
                .set(0, budget())
                .is_err()
        );
    }
    let edb = Edb::parse(fixture()).unwrap();
    assert!(
        edb.animation_modes()
            .unwrap()
            .set(
                0,
                Budget {
                    groups: 1,
                    ..budget()
                }
            )
            .is_err()
    );
    assert!(
        edb.animation_modes()
            .unwrap()
            .set(
                0,
                Budget {
                    children: 0,
                    ..budget()
                }
            )
            .is_err()
    );
    // The relocation high flag alone still denotes base section zero.
    let mut data = fixture();
    half(&mut data, 0x164, 0x8000);
    assert!(
        Edb::parse(data)
            .unwrap()
            .animation_modes()
            .unwrap()
            .set(0, budget())
            .is_ok()
    );
}
