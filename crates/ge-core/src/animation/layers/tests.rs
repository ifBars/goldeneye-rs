use super::*;
use crate::animation::mode::{
    Child, ChildPlan, CleanupIntent, CommandPlan, GroupPlan, Insertion, StartSet, StartSetPlan,
};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Payload {
    Control([u32; 8]),
    Child {
        reference: u32,
        scalar: u32,
        reciprocal: u32,
        word8: u32,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Control(NodeId),
    Child(NodeId, u8),
    Copy(NodeId, NodeId),
    Init(NodeId),
    Drop(NodeId),
}
#[derive(Default)]
struct Owner {
    next: u64,
    payloads: BTreeMap<NodeId, Payload>,
    events: Vec<Event>,
    no_control: bool,
    no_child: bool,
    fail_init: bool,
    reused: Option<NodeId>,
}
impl Owner {
    fn allocate(&mut self, payload: Payload) -> NodeId {
        self.next += 1;
        let id = NodeId::new(self.next).unwrap();
        self.payloads.insert(id, payload);
        id
    }
}
impl Host for Owner {
    fn create_control(&mut self) -> Result<Option<NodeId>> {
        if self.no_control {
            return Ok(None);
        }
        let id = self.allocate(Payload::Control([
            0,
            0,
            0,
            1.0f32.to_bits(),
            0,
            0,
            0,
            1.0f32.to_bits(),
        ]));
        self.events.push(Event::Control(id));
        Ok(Some(id))
    }
    fn clone_control(&mut self, source: NodeId) -> Result<Option<NodeId>> {
        let payload = self.payloads[&source].clone();
        let id = self.allocate(payload);
        Ok(Some(id))
    }
    fn create_child(&mut self, slot: u8, child: &ChildPlan) -> Result<Option<NodeId>> {
        if self.no_child {
            return Ok(None);
        }
        if let Some(id) = self.reused {
            return Ok(Some(id));
        }
        let id = self.allocate(Payload::Child {
            reference: child.source.reference,
            scalar: u32::MAX,
            reciprocal: u32::MAX,
            word8: u32::MAX,
        });
        self.events.push(Event::Child(id, slot));
        Ok(Some(id))
    }
    fn copy_head(&mut self, new: NodeId, old: NodeId) -> Result<()> {
        ensure!(
            self.payloads.contains_key(&new) && self.payloads.contains_key(&old),
            "missing copy owner"
        );
        self.events.push(Event::Copy(new, old));
        Ok(())
    }
    fn initialize_child(&mut self, node: NodeId, child: &ChildPlan) -> Result<()> {
        ensure!(!self.fail_init, "authored initialization fault");
        self.payloads.insert(
            node,
            Payload::Child {
                reference: child.source.reference,
                scalar: child.scalar_zero_bits,
                reciprocal: child.reciprocal_divisor_bits,
                word8: child.source.word8_raw,
            },
        );
        self.events.push(Event::Init(node));
        Ok(())
    }
    fn destroy_node(&mut self, node: NodeId) -> Result<()> {
        ensure!(
            self.payloads.remove(&node).is_some(),
            "missing destroyed owner"
        );
        self.events.push(Event::Drop(node));
        Ok(())
    }
}
fn id(value: u64) -> NodeId {
    NodeId::new(value).unwrap()
}
fn budget() -> Budget {
    Budget {
        nodes: 20,
        commands: 20,
        groups: 20,
        children: 20,
    }
}
fn no_override() -> SlotOverride {
    SlotOverride {
        enabled_raw: 0,
        slot_raw: 0,
    }
}
fn state() -> State {
    State::from_empty_snapshot(Snapshot::fresh_first_person(), budget()).unwrap()
}
fn mask(value: u16) -> CommandPlan {
    CommandPlan::MaskOperation {
        source_address: 0x100,
        mask_raw: value,
    }
}
fn child(reference: u32, flags: u16) -> ChildPlan {
    ChildPlan {
        source_ordinal: 0,
        source: Child {
            source_address: 0x200,
            reference,
            flags_raw: flags,
            divisor_raw: 6,
            word8_raw: 0x1234_5678,
        },
        effective_flags: flags,
        divisor_u32: 6,
        reciprocal_divisor_bits: (1.0f32 / 6.0).to_bits(),
        scalar_zero_bits: 0,
        factory_class: 4,
        insertion: Insertion::Head,
    }
}
fn set(slot: u16, weight: u32, children: Vec<ChildPlan>) -> CommandPlan {
    CommandPlan::StartSet(StartSetPlan {
        source: StartSet {
            source_address: 0x300,
            set_reference: 0,
            set_index: 0,
            set_hash: 0x0a00_00f1,
            flags_override: 0,
            options_raw: 0,
            divisor_override_raw: 0,
            groups: Vec::new(),
        },
        groups: vec![GroupPlan {
            source_ordinal: 0,
            source_slot: slot,
            weight_bits: weight,
            children,
        }],
        effective_flags_or: 0,
        // The executor must use reached flags/live slots, not trust this label.
        cleanup: CleanupIntent::SuppressedByFlag20,
    })
}

#[test]
fn actual_two_plan_membership_has_owned_payloads_distinct_tick_and_reverse_order() {
    let mut state = state();
    let mut host = Owner::default();
    state
        .execute(
            &mut host,
            &[mask(3), set(1, 0, vec![child(0x8400_00bb, 0x27)])],
            no_override(),
        )
        .unwrap();
    assert_eq!(state.slots()[0].nodes_head(), &[id(1)]);
    assert_eq!(state.slots()[1].nodes_head(), &[id(2)]);
    state
        .execute(
            &mut host,
            &[set(0, 0, vec![child(0x8400_0022, 0x23)])],
            no_override(),
        )
        .unwrap();
    assert_eq!(state.active_count(), 2);
    assert_eq!((state.selection_mask(), state.clear_mask()), (0xfb, 0xfb));
    assert_eq!(state.slots()[0].nodes_head(), &[id(3), id(1)]);
    assert_eq!(
        state.slots()[0]
            .nodes_head()
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>(),
        vec![id(1), id(3)]
    );
    assert_eq!(state.slots()[1].weight_bits(), 0);
    assert_eq!(state.slots()[0].weight_bits(), 1.0f32.to_bits());
    assert_eq!(state.slots()[2].weight_bits(), 1.0f32.to_bits());
    assert_eq!(host.payloads.len(), 3);
    assert_eq!(
        host.payloads[&id(3)],
        Payload::Child {
            reference: 0x8400_0022,
            scalar: 0,
            reciprocal: (1.0f32 / 6.0).to_bits(),
            word8: 0x1234_5678
        }
    );
    assert!(
        !host
            .events
            .iter()
            .any(|event| matches!(event, Event::Copy(..)))
    );
}

#[test]
fn mask_current_count_does_not_visit_higher_bits_and_zero_mask_actually_destroys() {
    let mut state = state();
    let mut host = Owner::default();
    state.execute(&mut host, &[mask(3)], no_override()).unwrap();
    assert_eq!(state.active_count(), 1);
    assert!(state.slots()[1].nodes_head().is_empty());
    state.execute(&mut host, &[mask(0)], no_override()).unwrap();
    assert!(state.slots()[0].nodes_head().is_empty());
    assert!(host.payloads.is_empty());
    assert_eq!(state.active_count(), 1);
    assert_eq!(state.slots()[0].weight_bits(), 1.0f32.to_bits());
    assert_eq!(host.events.last(), Some(&Event::Drop(id(1))));
}

#[test]
fn mask_allocates_before_selection_filter_and_control_none_is_an_error() {
    let mut snapshot = Snapshot::fresh_first_person();
    snapshot.selection_mask = 0;
    let mut state = State::from_empty_snapshot(snapshot, budget()).unwrap();
    let mut host = Owner::default();
    state.execute(&mut host, &[mask(3)], no_override()).unwrap();
    assert_eq!(state.created_nodes(), &[id(1)]);
    assert!(state.slots()[0].nodes_head().is_empty());
    assert_eq!(host.payloads.len(), 1); // Actual detached capture remains owned.
    let mut state = super::tests::state();
    let mut host = Owner {
        no_control: true,
        ..Owner::default()
    };
    assert!(state.execute(&mut host, &[mask(3)], no_override()).is_err());
    assert!(state.failed());
}

#[test]
fn no_child_retains_group_weight_and_flags20_effect_without_count_or_scalar_writes() {
    let mut state = state();
    let mut host = Owner::default();
    state.execute(&mut host, &[mask(3)], no_override()).unwrap();
    host.no_child = true;
    state
        .execute(
            &mut host,
            &[set(1, 0, vec![child(0x8400_00bb, 0x27)])],
            no_override(),
        )
        .unwrap();
    assert_eq!(state.active_count(), 1);
    assert_eq!(state.slots()[0].nodes_head(), &[id(1)]);
    assert!(state.slots()[1].nodes_head().is_empty());
    assert_eq!(state.slots()[1].weight_bits(), 0);
    assert_eq!(host.events, vec![Event::Control(id(1))]);
}

#[test]
fn existing_head_copy_is_reached_before_insert_and_initialization() {
    let mut state = state();
    let mut host = Owner::default();
    state
        .execute(
            &mut host,
            &[mask(3), set(0, 0, vec![child(0x8400_00a1, 0x24)])],
            no_override(),
        )
        .unwrap();
    assert_eq!(state.slots()[0].nodes_head(), &[id(2), id(1)]);
    assert_eq!(
        host.events,
        vec![
            Event::Control(id(1)),
            Event::Child(id(2), 0),
            Event::Copy(id(2), id(1)),
            Event::Init(id(2))
        ]
    );
}

#[test]
fn conditional_cleanup_uses_live_slot_flags_and_clear_mask() {
    for (flags, clear, retained) in [(0, 0xfb, false), (0x0800, 0xfb, true), (0, 0, true)] {
        let mut snapshot = Snapshot::fresh_first_person();
        snapshot.slot_flags[0] = flags;
        snapshot.clear_mask = clear;
        let mut state = State::from_empty_snapshot(snapshot, budget()).unwrap();
        let mut host = Owner::default();
        state
            .execute(
                &mut host,
                &[mask(3), set(1, 0, vec![child(0x8400_00a1, 0)])],
                no_override(),
            )
            .unwrap();
        assert_eq!(!state.slots()[0].nodes_head().is_empty(), retained);
        assert_eq!(state.slots()[0].flags_raw(), flags);
        assert_eq!(host.payloads.contains_key(&id(1)), retained);
    }
}

#[test]
fn cleanup_membership_uses_original_slots_independently_of_effective_override() {
    let mut state = state();
    let mut host = Owner::default();
    state.execute(&mut host, &[mask(3)], no_override()).unwrap();
    state
        .execute(
            &mut host,
            &[set(0, 0, vec![child(0x8400_00a1, 0)])],
            SlotOverride {
                enabled_raw: 1,
                slot_raw: 1,
            },
        )
        .unwrap();
    assert_eq!(state.active_count(), 2);
    assert_eq!(state.slots()[0].nodes_head(), &[id(1)]);
    assert!(state.slots()[1].nodes_head().is_empty());
    assert_eq!(host.events.last(), Some(&Event::Drop(id(2))));
}

#[test]
fn multiple_active_mask_slots_get_distinct_real_clones_and_reused_ids_fail() {
    let mut snapshot = Snapshot::fresh_first_person();
    snapshot.active_count = 2;
    let mut state = State::from_empty_snapshot(snapshot, budget()).unwrap();
    let mut host = Owner::default();
    state.execute(&mut host, &[mask(3)], no_override()).unwrap();
    assert_eq!(state.slots()[0].nodes_head(), &[id(1)]);
    assert_eq!(state.slots()[1].nodes_head(), &[id(2)]);
    assert_eq!(host.payloads[&id(1)], host.payloads[&id(2)]);
    host.reused = Some(id(1));
    assert!(
        state
            .execute(
                &mut host,
                &[set(0, 0, vec![child(0x8400_00a1, 0x20)])],
                no_override()
            )
            .is_err()
    );
    assert!(state.failed());
}

#[test]
fn reached_wrapper_and_host_errors_latch_failure_without_fake_completion() {
    let mut state = state();
    let mut host = Owner::default();
    assert!(
        state
            .execute(
                &mut host,
                &[set(0, 0, vec![child(0x8400_00a1, 8)])],
                no_override()
            )
            .is_err()
    );
    assert_eq!(host.payloads.len(), 1);
    assert!(state.failed());
    assert!(state.slots()[0].nodes_head().is_empty());
    let calls = host.events.len();
    assert!(state.execute(&mut host, &[mask(3)], no_override()).is_err());
    assert_eq!(host.events.len(), calls);
    let mut state = super::tests::state();
    let mut host = Owner {
        fail_init: true,
        ..Owner::default()
    };
    assert!(
        state
            .execute(
                &mut host,
                &[set(0, 0, vec![child(0x8400_00a1, 0)])],
                no_override()
            )
            .is_err()
    );
    assert_eq!(state.slots()[0].nodes_head(), &[id(1)]); // Source insert precedes scalar writes.
    assert!(state.failed());
}

#[test]
fn explicit_weights_overrides_and_aggregate_work_budgets_are_enforced() {
    let mut snapshot = Snapshot::fresh_first_person();
    snapshot.weight_bits[1] = 0.25f32.to_bits();
    let mut state = State::from_empty_snapshot(
        snapshot,
        Budget {
            commands: 2,
            ..budget()
        },
    )
    .unwrap();
    let mut host = Owner::default();
    state
        .execute(&mut host, &[set(1, 0x7fc0_0001, vec![])], no_override())
        .unwrap();
    assert_eq!(state.slots()[1].weight_bits(), 0.25f32.to_bits());
    state
        .execute(
            &mut host,
            &[set(1, (-0.0f32).to_bits(), vec![])],
            no_override(),
        )
        .unwrap();
    assert_eq!(state.slots()[1].weight_bits(), 0);
    assert!(state.execute(&mut host, &[mask(3)], no_override()).is_err());
    assert!(host.events.is_empty());
    let mut state = State::from_empty_snapshot(
        snapshot,
        Budget {
            nodes: 0,
            ..budget()
        },
    )
    .unwrap();
    assert!(state.execute(&mut host, &[mask(3)], no_override()).is_err());
    assert!(host.events.is_empty());
    let mut state = super::tests::state();
    assert!(
        state
            .execute(
                &mut host,
                &[set(0, 0, vec![])],
                SlotOverride {
                    enabled_raw: 1,
                    slot_raw: 4
                }
            )
            .is_err()
    );
    assert!(
        State::from_empty_snapshot(
            Snapshot {
                active_count: 5,
                ..snapshot
            },
            budget()
        )
        .is_err()
    );
    assert!(NodeId::new(0).is_err());
}
