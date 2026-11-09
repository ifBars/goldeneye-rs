use super::*;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

const REQUESTED: u32 = 0x0900_00a1;
const FALLBACK: u32 = 0x0900_00a2;
const SOURCE: u32 = 0x0900_00b1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    Resolve(u32),
    Mode(usize),
    Target(usize, usize),
    Commands(usize, usize),
}

#[derive(Default)]
struct Fixture {
    references: BTreeMap<u32, usize>,
    modes: BTreeMap<usize, Mode>,
    targets: BTreeMap<(usize, usize), u32>,
    commands: BTreeMap<(usize, usize), Vec<Command>>,
    reads: Vec<Read>,
    fail_at: Option<Read>,
}

impl Fixture {
    fn record(&mut self, read: Read) -> Result<()> {
        self.reads.push(read);
        ensure!(
            self.fail_at != Some(read),
            "authored malformed/unsupported reached input"
        );
        Ok(())
    }
}

impl Provider for Fixture {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>> {
        self.record(Read::Resolve(reference))?;
        Ok(self.references.get(&reference).copied())
    }

    fn mode(&mut self, index: usize) -> Result<Mode> {
        self.record(Read::Mode(index))?;
        self.modes
            .get(&index)
            .copied()
            .context("unavailable descriptor must stay unread")
    }

    fn transition_target(&mut self, index: usize, ordinal: usize) -> Result<u32> {
        self.record(Read::Target(index, ordinal))?;
        self.targets
            .get(&(index, ordinal))
            .copied()
            .context("unavailable target word")
    }

    fn commands(&mut self, index: usize, ordinal: usize) -> Result<Vec<Command>> {
        self.record(Read::Commands(index, ordinal))?;
        self.commands
            .get(&(index, ordinal))
            .cloned()
            .context("unavailable command list must stay unread")
    }
}

fn inputs() -> Inputs {
    Inputs {
        requested: REQUESTED,
        previous: 0,
        fallback: FALLBACK,
        resource_available: true,
    }
}

fn budget() -> Budget {
    Budget {
        lookups: 3,
        transition_rows: 16,
        commands: 16,
        groups: 16,
        children: 16,
    }
}

fn fixture() -> Fixture {
    Fixture {
        references: BTreeMap::from([(REQUESTED, 7), (0, 0), (FALLBACK, 1)]),
        modes: BTreeMap::from([(
            0,
            Mode {
                hash: SOURCE,
                transition_count: 3,
            },
        )]),
        targets: BTreeMap::from([((0, 0), 11), ((0, 1), 7), ((0, 2), 7)]),
        commands: BTreeMap::from([(
            (0, 1),
            vec![Command::MaskOperation {
                source_address: 128,
                mask_raw: 3,
            }],
        )]),
        ..Fixture::default()
    }
}

fn child(reference: u32, flags: u16, divisor: u16) -> Child {
    Child {
        source_address: 256,
        reference,
        flags_raw: flags,
        divisor_raw: divisor,
        word8_raw: 0xffc0_1234,
    }
}

fn start_set() -> StartSet {
    StartSet {
        source_address: 192,
        set_reference: 3,
        set_index: 3,
        set_hash: 0x0a00_00a5,
        flags_override: 0,
        options_raw: 0,
        divisor_override_raw: 77,
        groups: vec![Group {
            source_address: 240,
            slot: 2,
            weight_bits: 0,
            children: vec![child(0x8400_00a5, 0x8004, 6), child(0x0300_00a6, 0x20, 0)],
        }],
    }
}

fn planned_set(source: StartSet) -> StartSetPlan {
    match super::plan::command_plan(Command::StartSet(source), budget(), &mut Work::default())
        .unwrap()
    {
        CommandPlan::StartSet(plan) => plan,
        _ => panic!("expected authored StartSet plan"),
    }
}

#[test]
fn unavailable_resource_and_missing_requested_mode_stop_before_previous_or_descriptors() {
    let mut provider = fixture();
    let mut unavailable = inputs();
    unavailable.resource_available = false;
    let empty_budget = Budget {
        lookups: 0,
        transition_rows: 0,
        commands: 0,
        groups: 0,
        children: 0,
    };
    let result = select(&mut provider, unavailable, empty_budget).unwrap();
    assert_eq!(result.status, Status::ResourceUnavailable);
    assert!(provider.reads.is_empty() && result.attempts.is_empty());
    provider.references.remove(&REQUESTED);
    let result = select(&mut provider, inputs(), budget()).unwrap();
    assert_eq!(result.status, Status::TargetMissing);
    assert_eq!(result.target_index, None);
    assert_eq!(result.return_word, 0);
    assert_eq!(provider.reads, [Read::Resolve(REQUESTED)]);
}

#[test]
fn target_index_only_coded_previous_zero_and_first_match_keep_later_sources_unread() {
    let mut provider = fixture();
    // Target descriptor7 and fallback descriptor1 are deliberately absent.
    // Unmatched row0 and later duplicate row2 have no readable command lists.
    let result = select(&mut provider, inputs(), budget()).unwrap();
    assert_eq!(result.status, Status::AttemptsComplete);
    assert_eq!(result.target_index, Some(7));
    assert_eq!(result.return_word, SOURCE);
    assert_eq!(result.attempts.len(), 1);
    assert_eq!(result.attempts[0].kind, AttemptKind::Previous);
    assert_eq!(result.attempts[0].reference, 0);
    assert_eq!(result.attempts[0].matched_transition, Some(1));
    assert_eq!(
        result.attempts[0].commands,
        [CommandPlan::MaskOperation {
            source_address: 128,
            mask_raw: 3
        }]
    );
    assert_eq!(
        provider.reads,
        [
            Read::Resolve(REQUESTED),
            Read::Resolve(0),
            Read::Mode(0),
            Read::Target(0, 0),
            Read::Target(0, 1),
            Read::Commands(0, 1)
        ]
    );
    assert_eq!(result.work.lookups, 2);
    assert_eq!(result.work.transition_rows, 2);
}

#[test]
fn absent_empty_or_unmatched_previous_uses_fallback_and_empty_commands_return_source_hash() {
    for previous in 0..3 {
        let mut provider = fixture();
        match previous {
            0 => {
                provider.references.remove(&0);
            }
            1 => {
                provider.modes.get_mut(&0).unwrap().transition_count = 0;
            }
            _ => {
                provider.modes.get_mut(&0).unwrap().transition_count = 1;
            }
        }
        provider.modes.insert(
            1,
            Mode {
                hash: 0x0900_00c1,
                transition_count: 1,
            },
        );
        provider.targets.insert((1, 0), 7);
        provider.commands.insert((1, 0), vec![]);
        let result = select(&mut provider, inputs(), budget()).unwrap();
        assert_eq!(result.return_word, 0x0900_00c1);
        assert_eq!(result.attempts.len(), 2);
        assert_eq!(result.attempts[1].kind, AttemptKind::Fallback);
        assert_eq!(result.attempts[1].matched_transition, Some(0));
        assert!(result.attempts[1].commands.is_empty());
    }
}

#[test]
fn matched_zero_hash_keeps_prior_effects_and_same_fallback_is_attempted_again() {
    let mut provider = fixture();
    provider.modes.get_mut(&0).unwrap().hash = 0;
    let mut request = inputs();
    request.fallback = 0;
    let result = select(&mut provider, request, budget()).unwrap();
    assert_eq!(result.return_word, 0);
    assert_eq!(result.attempts.len(), 2);
    assert_eq!(result.attempts[0].source_hash, Some(0));
    assert_eq!(result.attempts[1].source_index, Some(0));
    assert_eq!(result.attempts[0].commands, result.attempts[1].commands);
    assert_eq!(result.attempts[0].commands.len(), 1);
    assert_eq!(
        provider
            .reads
            .iter()
            .filter(|read| **read == Read::Commands(0, 1))
            .count(),
        2
    );
    assert_eq!(result.work.commands, 2);
    assert_eq!(result.work.transition_rows, 4);

    provider.modes.insert(
        1,
        Mode {
            hash: 0x0900_00c1,
            transition_count: 1,
        },
    );
    provider.targets.insert((1, 0), 7);
    provider.commands.insert(
        (1, 0),
        vec![Command::MaskOperation {
            source_address: 512,
            mask_raw: 9,
        }],
    );
    let result = select(&mut provider, inputs(), budget()).unwrap();
    assert_eq!(result.return_word, 0x0900_00c1);
    assert_eq!(
        result.attempts[0].commands,
        [CommandPlan::MaskOperation {
            source_address: 128,
            mask_raw: 3
        }]
    );
    assert_eq!(
        result.attempts[1].commands,
        [CommandPlan::MaskOperation {
            source_address: 512,
            mask_raw: 9
        }]
    );
}

#[test]
fn reached_provider_failures_and_work_budgets_never_trigger_native_fallback() {
    for read in [
        Read::Resolve(REQUESTED),
        Read::Mode(0),
        Read::Target(0, 0),
        Read::Commands(0, 1),
    ] {
        let mut provider = fixture();
        provider.fail_at = Some(read);
        assert!(select(&mut provider, inputs(), budget()).is_err());
        assert!(!provider.reads.contains(&Read::Resolve(FALLBACK)));
        assert_eq!(provider.reads.last(), Some(&read));
    }
    for field in 0..5 {
        let mut provider = fixture();
        provider
            .commands
            .insert((0, 1), vec![Command::StartSet(start_set())]);
        let mut limit = budget();
        match field {
            0 => limit.lookups = 1,
            1 => limit.transition_rows = 1,
            2 => limit.commands = 0,
            3 => limit.groups = 0,
            _ => limit.children = 1,
        }
        let error = select(&mut provider, inputs(), limit).unwrap_err();
        assert!(error.to_string().contains("budget"));
        assert!(!provider.reads.contains(&Read::Resolve(FALLBACK)));
    }
}

#[test]
fn effective_flags_and_divisors_preserve_override_precedence_raw_bits_and_head_intent() {
    let plan = planned_set(start_set());
    let children = &plan.groups[0].children;
    assert_eq!(children[0].effective_flags, 0x8004);
    assert_eq!(children[1].effective_flags, 0x20);
    assert_eq!(plan.effective_flags_or, 0x8024);
    assert_eq!(plan.cleanup, CleanupIntent::SuppressedByFlag20);
    assert_eq!(children[0].divisor_u32, 6); // options0 leaves override unread/ineffective
    assert_eq!(
        children[0].reciprocal_divisor_bits,
        (1.0f32 / 6.0).to_bits()
    );
    assert_eq!(children[1].divisor_u32, 0);
    assert_eq!(children[1].reciprocal_divisor_bits, 1.0f32.to_bits());
    assert_eq!(children[0].source.word8_raw, 0xffc0_1234);
    assert_eq!(children[0].source.reference, 0x8400_00a5);
    assert_eq!(children[1].source.reference, 0x0300_00a6);
    assert_eq!(
        (children[0].factory_class, children[1].factory_class),
        (4, 3)
    );
    assert!(
        children
            .iter()
            .all(|child| child.insertion == Insertion::Head && child.scalar_zero_bits == 0)
    );
    let mut source = start_set();
    source.flags_override = 0x8108;
    source.options_raw = -32767;
    source.divisor_override_raw = u32::MAX;
    let plan = planned_set(source);
    assert_eq!(plan.effective_flags_or, 0x8108);
    for child in &plan.groups[0].children {
        assert_eq!(child.effective_flags, 0x8108);
        assert_eq!(child.divisor_u32, u32::MAX);
        assert_eq!(child.reciprocal_divisor_bits, 0x2f80_0000);
    }
}

#[test]
fn slot_weight_requests_use_explicit_override_and_existing_bits_without_assigning_source_weights() {
    let mut group = planned_set(start_set()).groups.remove(0);
    let snapshot = SlotInputs {
        override_enabled_raw: 0,
        override_slot_raw: 99,
        current_weight_bits: [0x8000_0000, 0x7fc0_1234, 0xbf00_0000, 0],
    };
    group.source_slot = 0;
    group.weight_bits = 0x7fc0_ffff;
    assert_eq!(
        group.layer_request(snapshot).unwrap().weight,
        WeightRequest::Assign(0x3f80_0000)
    );
    group.source_slot = 1;
    assert_eq!(
        group.layer_request(snapshot).unwrap().weight,
        WeightRequest::Preserve(0x7fc0_1234)
    );
    group.weight_bits = 0x8000_0000;
    assert_eq!(
        group.layer_request(snapshot).unwrap().weight,
        WeightRequest::Assign(0)
    );
    group.weight_bits = 0x3e80_0000;
    assert_eq!(
        group.layer_request(snapshot).unwrap().weight,
        WeightRequest::Preserve(0x7fc0_1234)
    );
    let overridden = SlotInputs {
        override_enabled_raw: 0xff,
        override_slot_raw: 0,
        ..snapshot
    };
    assert_eq!(
        group.layer_request(overridden).unwrap(),
        LayerRequest {
            effective_slot: 0,
            weight: WeightRequest::Assign(0x3f80_0000)
        }
    );
    assert!(
        group
            .layer_request(SlotInputs {
                override_slot_raw: 4,
                ..overridden
            })
            .is_err()
    );
}

#[test]
fn request_order_and_cleanup_membership_keep_serialized_slots_even_with_duplicates_and_empty_groups()
 {
    let mut source = start_set();
    source.groups[0].children[0].flags_raw = 0;
    source.groups[0].children[1].flags_raw = 0;
    source.groups.push(Group {
        source_address: 400,
        slot: 2,
        weight_bits: 0x3f80_0000,
        children: vec![child(0, 0, 1)],
    });
    source.groups.push(Group {
        source_address: 440,
        slot: 0,
        weight_bits: 0,
        children: vec![],
    });
    let plan = planned_set(source);
    assert_eq!(
        plan.groups
            .iter()
            .map(|group| group.source_slot)
            .collect::<Vec<_>>(),
        [2, 2, 0]
    );
    assert_eq!(plan.groups[1].source_ordinal, 1);
    assert_eq!(plan.groups[1].children[0].source.reference, 0);
    assert_eq!(plan.groups[1].children[0].factory_class, 0); // no fabricated allocation
    assert_eq!(
        plan.cleanup,
        CleanupIntent::Conditional {
            source_slots: vec![2, 2, 0]
        }
    );
    let mut empty = start_set();
    empty.flags_override = 0x20;
    empty.groups[0].children.clear();
    // The OR includes reached child flags, not an unused command override.
    assert_eq!(
        planned_set(empty).cleanup,
        CleanupIntent::Conditional {
            source_slots: vec![2]
        }
    );
}
