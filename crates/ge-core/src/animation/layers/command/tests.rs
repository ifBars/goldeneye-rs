use super::*;
#[derive(Clone, Copy)]
struct Row {
    slot: u16,
    weight: u32,
    children: i16,
}
struct Fixture {
    rows: Vec<Row>,
    group: usize,
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
    present: bool,
    options: i16,
    divisor: u32,
    flags: u16,
}
impl Fixture {
    fn read(&mut self, label: &'static str) -> Result<()> {
        self.calls.push(label);
        ensure!(
            self.fail != Some(label),
            "authored provider failure at {label}"
        );
        Ok(())
    }
}
impl Source for Fixture {
    fn set_reference(&mut self) -> Result<u32> {
        self.read("reference")?;
        Ok(0x0a00_00ab)
    }
    fn resolve_set(&mut self, _: u32) -> Result<Option<usize>> {
        self.read("resolve")?;
        Ok(self.present.then_some(0))
    }
    fn options(&mut self) -> Result<i16> {
        self.read("options")?;
        Ok(self.options)
    }
    fn divisor_override(&mut self) -> Result<u32> {
        self.read("divisor")?;
        Ok(self.divisor)
    }
    fn flags_override(&mut self) -> Result<u16> {
        self.read("flags")?;
        Ok(self.flags)
    }
    fn begin_set(&mut self, _: usize) -> Result<usize> {
        self.read("body/count")?;
        Ok(self.rows.len())
    }
    fn group_slot(&mut self) -> Result<u16> {
        self.read("slot")?;
        Ok(self.rows[self.group].slot)
    }
    fn group_weight(&mut self) -> Result<u32> {
        self.read("weight")?;
        Ok(self.rows[self.group].weight)
    }
    fn child_count(&mut self) -> Result<i16> {
        self.read("count")?;
        Ok(self.rows[self.group].children)
    }
    fn child_divisor(&mut self, _: usize) -> Result<u16> {
        self.read("child_divisor")?;
        Ok(3)
    }
    fn child_flags(&mut self, _: usize) -> Result<u16> {
        self.read("child_flags")?;
        Ok(4)
    }
    fn child_reference(&mut self, _: usize) -> Result<u32> {
        self.read("child_reference")?;
        Ok(0x0400_00ab)
    }
    fn next_group(&mut self, _: u16) -> Result<()> {
        self.read("next")?;
        self.group += 1;
        Ok(())
    }
}
fn fixture() -> Fixture {
    Fixture {
        rows: vec![Row {
            slot: 0,
            weight: 0x7fc0_0001,
            children: 1,
        }],
        group: 0,
        calls: Vec::new(),
        fail: None,
        present: true,
        options: 0,
        divisor: 77,
        flags: 0,
    }
}
fn layers() -> State {
    State::from_empty_snapshot(
        super::super::Snapshot {
            active_count: 1,
            selection_mask: 255,
            clear_mask: 255,
            slot_flags: [1; 4],
            weight_bits: [7; 4],
        },
        super::super::Budget {
            nodes: 8,
            commands: 8,
            groups: 8,
            children: 8,
        },
    )
    .unwrap()
}
fn no_override() -> SlotOverride {
    SlotOverride {
        enabled_raw: 0,
        slot_raw: u32::MAX,
    }
}
#[test]
fn first_group_weight_precedes_factory_and_bypassed_fields_and_later_groups_stay_unread() {
    let mut source = fixture();
    source.options = 1;
    source.flags = 0x20;
    source.fail = Some("weight");
    source.rows.push(Row {
        slot: u16::MAX,
        weight: 0,
        children: -1,
    });
    let mut state = layers();
    let mut command = Pending::new(0x0c00_0002);
    command
        .prepare(&mut source, &mut state, no_override())
        .unwrap();
    assert_eq!(
        source.calls,
        [
            "reference",
            "resolve",
            "options",
            "divisor",
            "flags",
            "body/count",
            "slot",
            "count",
            "child_reference"
        ]
    );
    assert_eq!(state.slots()[0].weight_bits(), 1f32.to_bits());
    assert_eq!(command.stage(), Stage::ChildFactoryPending);
    assert_eq!(
        command.child(),
        Some(ChildRequest {
            group: 0,
            ordinal: 0,
            slot: 0,
            reference: 0x0400_00ab,
            effective_flags: 0x20,
            divisor: 77
        })
    );
    assert_eq!(command.flags_or(), 0x20);
    assert_eq!(command.weight_writes(), 1);
    assert!(state.created_nodes().is_empty());
    assert!(state.slots()[0].nodes_head().is_empty());
    let calls = source.calls.clone();
    assert!(
        command
            .prepare(&mut source, &mut state, no_override())
            .is_err()
    );
    assert_eq!(source.calls, calls);
}
#[test]
fn child_count_failure_and_negative_count_retain_the_reached_weight_and_prohibit_replay() {
    for failure in [true, false] {
        let mut source = fixture();
        source.rows[0] = Row {
            slot: 2,
            weight: 0x8000_0000,
            children: -1,
        };
        if failure {
            source.fail = Some("count");
        }
        let mut state = layers();
        let mut command = Pending::new(0x0c00_0002);
        assert!(
            command
                .prepare(&mut source, &mut state, no_override())
                .is_err()
        );
        assert_eq!(state.slots()[2].weight_bits(), 0);
        assert_eq!(command.weight_writes(), 1);
        assert_eq!(
            command.groups()[0].child_count,
            if failure { None } else { Some(-1) }
        );
        assert!(state.failed());
        assert_eq!(command.stage(), Stage::Failed);
        let calls = source.calls.clone();
        assert!(
            command
                .prepare(&mut source, &mut state, no_override())
                .is_err()
        );
        assert_eq!(source.calls, calls);
    }
}
#[test]
fn empty_group_preserves_nonzero_weight_and_advances_before_next_factory() {
    let mut source = fixture();
    source.rows.insert(
        0,
        Row {
            slot: 1,
            weight: 0x7fc0_0001,
            children: 0,
        },
    );
    let mut state = layers();
    let mut command = Pending::new(0x0c00_0002);
    command
        .prepare(&mut source, &mut state, no_override())
        .unwrap();
    assert_eq!(state.slots()[1].weight_bits(), 7);
    assert_eq!(command.groups()[0].weight_write, None);
    assert_eq!(command.child().unwrap().group, 1);
    assert_eq!(command.groups().len(), 2);
    assert_eq!(
        source
            .calls
            .iter()
            .filter(|label| **label == "next")
            .count(),
        1
    );
    assert!(!source.calls.contains(&"divisor"));
    assert_eq!(command.child().unwrap().divisor, 3);
    assert_eq!(command.child().unwrap().effective_flags, 4);
}
#[test]
fn effective_slot_override_keeps_original_serialized_slot_and_skips_zero_slot_weight_read() {
    let mut source = fixture();
    source.rows[0].slot = 3;
    source.fail = Some("weight");
    let mut state = layers();
    let mut command = Pending::new(0x0c00_0002);
    command
        .prepare(
            &mut source,
            &mut state,
            SlotOverride {
                enabled_raw: 0x80,
                slot_raw: 0,
            },
        )
        .unwrap();
    assert_eq!(
        (
            command.groups()[0].source_slot,
            command.groups()[0].effective_slot
        ),
        (3, 0)
    );
    assert_eq!(command.child().unwrap().slot, 0);
    assert_eq!(state.slots()[3].weight_bits(), 7);
}
#[test]
fn additive_opcode_uses_positive_signed_override_and_always_reads_child_divisor() {
    for (raw, expected) in [(5, 8), (0xffff_ffff, 3), (0, 3)] {
        let mut source = fixture();
        source.options = 1;
        source.divisor = raw;
        let mut state = layers();
        let mut command = Pending::new(0x0c00_0003);
        command
            .prepare(&mut source, &mut state, no_override())
            .unwrap();
        assert_eq!(command.child().unwrap().divisor, expected);
        assert!(source.calls.contains(&"child_divisor"));
    }
}
#[test]
fn absent_set_still_reads_command_options_before_failing_without_weight_changes() {
    let mut source = fixture();
    source.present = false;
    let mut state = layers();
    let mut command = Pending::new(0x0c00_0002);
    assert!(
        command
            .prepare(&mut source, &mut state, no_override())
            .is_err()
    );
    assert_eq!(source.calls, ["reference", "resolve", "options", "flags"]);
    assert_eq!(command.set_reference(), Some(0x0a00_00ab));
    assert_eq!(command.set_index(), None);
    assert_eq!(command.weight_writes(), 0);
    assert_eq!(state.slots()[0].weight_bits(), 7);
}
#[test]
fn mask_and_unhandled_opcodes_stop_at_their_actual_dependencies_without_allocations() {
    for (opcode, mask, expected) in [
        (0x0c00_0005, 15, Stage::MaskControlPending),
        (0x0c00_0005, 0, Stage::MaskClearPending),
        (0x0c00_00ff, 15, Stage::OpcodePending),
    ] {
        let mut source = fixture();
        source.flags = mask;
        let mut state = layers();
        let mut command = Pending::new(opcode);
        command
            .prepare(&mut source, &mut state, no_override())
            .unwrap();
        assert_eq!(command.stage(), expected);
        assert_eq!(
            source.calls,
            if opcode == 0x0c00_0005 {
                vec!["flags"]
            } else {
                vec![]
            }
        );
        assert!(state.created_nodes().is_empty());
        assert_eq!(state.slots()[0].weight_bits(), 7);
    }
}
#[test]
fn zero_group_set_reaches_cleanup_and_group_budget_failure_keeps_completed_group_prefix() {
    let mut source = fixture();
    source.rows.clear();
    let mut state = layers();
    let mut command = Pending::new(0x0c00_0002);
    command
        .prepare(&mut source, &mut state, no_override())
        .unwrap();
    assert_eq!(command.stage(), Stage::CleanupPending);
    assert_eq!(command.group_count(), Some(0));
    assert_eq!(command.weight_writes(), 0);
    let mut source = fixture();
    source.rows[0].children = 0;
    source.rows.push(source.rows[0]);
    let mut state = layers();
    state.budget.groups = 1;
    let mut command = Pending::new(0x0c00_0002);
    assert!(
        command
            .prepare(&mut source, &mut state, no_override())
            .is_err()
    );
    assert_eq!(command.groups().len(), 1);
    assert_eq!(command.weight_writes(), 1);
    assert_eq!(
        source
            .calls
            .iter()
            .filter(|label| **label == "slot")
            .count(),
        1
    );
}
