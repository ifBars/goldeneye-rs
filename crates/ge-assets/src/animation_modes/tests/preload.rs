use super::*;
use crate::edb::Table;

const REQUEST: u32 = 0x09000021;
fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i64 - field as i64) as i32 as u32);
}
struct Fixture {
    data: Vec<u8>,
    modes: Table,
    sets: Table,
}
impl Fixture {
    fn new() -> Self {
        let mut data = vec![0; 0x130];
        for (at, value) in [
            (0x20, 0x0900000b),
            (0x28, 0x80),
            (0x30, 2),
            (0x34, REQUEST),
            (0x38, 0x00030000),
            (0x3c, u32::MAX),
            (0x50, 0x0a000011),
            (0x58, 0x100),
            (0x60, 2),
            (0x64, 0x0a000022),
            (0x80, 777),
            (0x84, 0x7fffffff),
            (0x88, 1),
            (0xa0, 2),
            (0xc0, 0x0c000005),
            (0xd0, 0x0c000004),
            (0xd4, 0),
            (0x104, 0x7fc00001),
            (0x108, 0x03000011),
            (0x114, 0x84000021),
        ] {
            word(&mut data, at, value);
        }
        for (field, target) in [(0x8c, 0xa0), (0xa4, 0xc0), (0xa8, 0xd0)] {
            relative(&mut data, field, target);
        }
        half(&mut data, 0x100, 0xffff);
        half(&mut data, 0x102, 2);
        Self {
            data,
            modes: Table {
                name: "animation_modes",
                count: 2,
                hash_size_raw: 2,
                offset: 0x20,
            },
            sets: Table {
                name: "animation_sets",
                count: 2,
                hash_size_raw: 2,
                offset: 0x50,
            },
        }
    }
    fn view(&self) -> View<'_> {
        View {
            data: &self.data,
            modes: &self.modes,
            sets: &self.sets,
        }
    }
    fn extract(&self, budget: ExtractionBudget) -> Result<Option<ClassFourReference>> {
        self.view().first_preload_class_four(REQUEST, budget)
    }
}
fn budget() -> ExtractionBudget {
    ExtractionBudget {
        transitions: 8,
        commands: 8,
        groups: 8,
        children: 8,
    }
}

#[test]
fn requested_lookup_precedes_baseline_body_and_missing_is_known_none() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x28, u32::MAX);
    word(&mut f.data, 0x30, u32::MAX);
    assert_eq!(
        f.view()
            .first_preload_class_four(0x090000fe, budget())
            .unwrap(),
        None
    );
    assert!(f.extract(budget()).is_err());
    word(&mut f.data, 0x20, 0x0900000c);
    assert_eq!(f.extract(budget()).unwrap(), None); // Requested exists, baseline absent.
}

#[test]
fn mismatching_transition_skips_bad_pointer_and_op4_accepts_highbit_class4() {
    let f = Fixture::new();
    let found = f.extract(budget()).unwrap().unwrap();
    assert_eq!(found.reference, 0x84000021);
    assert_eq!(
        (
            found.requested_index,
            found.baseline_index,
            found.transition_ordinal
        ),
        (1, 0, 1)
    );
    assert_eq!(
        (
            found.command_ordinal,
            found.set_index,
            found.group_ordinal,
            found.child_ordinal
        ),
        (1, 0, 0, 1)
    );
    assert_eq!(
        (
            found.transitions_read,
            found.commands_read,
            found.groups_read,
            found.children_read
        ),
        (2, 2, 1, 2)
    );
}

#[test]
fn first_reference_stops_before_child_flags_weight_and_later_records() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x108, 0x04000031);
    f.data.truncate(0x10c);
    let found = f.extract(budget()).unwrap().unwrap();
    assert_eq!(found.reference, 0x04000031);
    assert_eq!(found.children_read, 1);
    // A smaller logical resident slice errors even though its backing payload
    // contains the required word. Cached bytes do not repair that live view.
    let view = View {
        data: &f.data[..0x108],
        modes: &f.modes,
        sets: &f.sets,
    };
    assert!(view.first_preload_class_four(REQUEST, budget()).is_err());
}

#[test]
fn unused_command_operands_and_requested_descriptor_section_stay_lazy() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x12c, 0x0c000003);
    relative(&mut f.data, 0xa4, 0x12c);
    // Unsupported live op3 is ignored here, without a set operand at end+4.
    assert_eq!(f.extract(budget()).unwrap().unwrap().command_ordinal, 1);
    // The requested descriptor has section3 and an invalid body already.
    assert_eq!(f.view().resolve_mode(REQUEST).unwrap(), Some(1));
}

#[test]
fn op2_and_missing_set_continue_without_live_flags_divisors_or_eager_sets() {
    let mut f = Fixture::new();
    word(&mut f.data, 0xc0, 0x0c000004);
    word(&mut f.data, 0xc4, 0x0a0000ff);
    word(&mut f.data, 0xd0, 0x0c000002);
    assert_eq!(f.extract(budget()).unwrap().unwrap().command_ordinal, 1);
    word(&mut f.data, 0xd4, 0x0a0000ff);
    assert_eq!(f.extract(budget()).unwrap(), None);
}

#[test]
fn exhaustive_no_reference_is_none_but_negative_or_unresident_data_is_error() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x114, 0x05000021);
    assert_eq!(f.extract(budget()).unwrap(), None);
    half(&mut f.data, 0x102, 0xffff);
    assert!(f.extract(budget()).is_err());
    half(&mut f.data, 0x102, 2);
    f.data.truncate(0x117);
    assert!(f.extract(budget()).is_err());
}

#[test]
fn budgets_aggregate_across_matching_commands_groups_and_children() {
    let f = Fixture::new();
    for limited in [
        ExtractionBudget {
            transitions: 1,
            ..budget()
        },
        ExtractionBudget {
            commands: 1,
            ..budget()
        },
        ExtractionBudget {
            groups: 0,
            ..budget()
        },
        ExtractionBudget {
            children: 1,
            ..budget()
        },
    ] {
        assert!(f.extract(limited).is_err());
    }
    let mut f = Fixture::new();
    word(&mut f.data, 0xc0, 0x0c000004);
    word(&mut f.data, 0xc4, 0);
    word(&mut f.data, 0x114, 0x05000021);
    assert_eq!(f.extract(budget()).unwrap(), None);
    assert!(
        f.extract(ExtractionBudget {
            groups: 3,
            ..budget()
        })
        .is_err()
    );
    assert!(
        f.extract(ExtractionBudget {
            children: 3,
            ..budget()
        })
        .is_err()
    );
}

#[test]
fn declared_later_counts_do_not_defeat_first_reference_short_circuit() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x80, 1);
    relative(&mut f.data, 0x84, 0xa0);
    word(&mut f.data, 0x30, u32::MAX);
    word(&mut f.data, 0xa0, u32::MAX);
    word(&mut f.data, 0xc0, 0x0c000004);
    word(&mut f.data, 0xc4, 0);
    word(&mut f.data, 0x60, u32::MAX);
    word(&mut f.data, 0x108, 0x04000031);
    let found = f
        .extract(ExtractionBudget {
            transitions: 1,
            commands: 1,
            groups: 1,
            children: 1,
        })
        .unwrap()
        .unwrap();
    assert_eq!(found.reference, 0x04000031);
}

#[test]
fn zero_baseline_count_skips_body_but_zero_set_groups_still_reads_body_binding() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x30, 0);
    word(&mut f.data, 0x24, 0x00030000);
    word(&mut f.data, 0x28, u32::MAX);
    assert_eq!(f.extract(budget()).unwrap(), None);
    let mut f = Fixture::new();
    word(&mut f.data, 0x60, 0);
    word(&mut f.data, 0x54, 0x00030000);
    assert!(f.extract(budget()).is_err());
}

#[test]
fn absent_required_pointer_is_not_a_known_lookup_miss() {
    let mut f = Fixture::new();
    word(&mut f.data, 0x8c, 0);
    assert!(f.extract(budget()).is_err());
    let mut f = Fixture::new();
    word(&mut f.data, 0xa8, 0);
    assert!(f.extract(budget()).is_err());
}

#[test]
fn preload_op4_does_not_expand_live_set_stream_opcode_support() {
    let f = Fixture::new();
    let view = f.view();
    let location = view.command_location(0, 1, 1, 2).unwrap();
    assert_eq!(location.opcode(), 0x0c00_0004);
    let live = crate::animation_modes::CommandStream::new(location);
    assert!(live.set_reference(&view).is_err());
    assert!(f.extract(budget()).unwrap().is_some());
}

#[test]
fn op2_extraction_does_not_require_its_live_command_options() {
    let mut f = Fixture::new();
    f.data.resize(0x138, 0);
    word(&mut f.data, 0x130, 0x0c00_0002);
    word(&mut f.data, 0x134, 0);
    relative(&mut f.data, 0xa8, 0x130);
    let view = f.view();
    let live =
        crate::animation_modes::CommandStream::new(view.command_location(0, 1, 1, 2).unwrap());
    assert!(live.options(&view).is_err());
    assert_eq!(f.extract(budget()).unwrap().unwrap().reference, 0x84000021);
}
