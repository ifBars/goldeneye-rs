use super::*;
use crate::animation::{
    layers::{Budget, SlotOverride, Snapshot},
    nodes::OwnerId,
    scratch::{Arena, Context as Scratch},
    skeleton::LocalPose,
};

struct Operand {
    mask: u16,
    reads: usize,
}
impl Source for Operand {
    fn flags_override(&mut self) -> Result<u16> {
        self.reads += 1;
        Ok(self.mask)
    }
    fn set_reference(&mut self) -> Result<u32> {
        anyhow::bail!("mask read set reference")
    }
    fn resolve_set(&mut self, _: u32) -> Result<Option<usize>> {
        anyhow::bail!("mask resolved set")
    }
    fn options(&mut self) -> Result<i16> {
        anyhow::bail!("mask read options")
    }
    fn divisor_override(&mut self) -> Result<u32> {
        anyhow::bail!("mask read divisor")
    }
    fn begin_set(&mut self, _: usize) -> Result<usize> {
        anyhow::bail!("mask bound set")
    }
    fn group_slot(&mut self) -> Result<u16> {
        anyhow::bail!("mask read group slot")
    }
    fn group_weight(&mut self) -> Result<u32> {
        anyhow::bail!("mask read group weight")
    }
    fn child_count(&mut self) -> Result<i16> {
        anyhow::bail!("mask read child count")
    }
    fn child_divisor(&mut self, _: usize) -> Result<u16> {
        anyhow::bail!("mask read child divisor")
    }
    fn child_flags(&mut self, _: usize) -> Result<u16> {
        anyhow::bail!("mask read child flags")
    }
    fn child_reference(&mut self, _: usize) -> Result<u32> {
        anyhow::bail!("mask read child reference")
    }
    fn next_group(&mut self, _: u16) -> Result<()> {
        anyhow::bail!("mask advanced group")
    }
}
struct Runtime {
    command: Pending,
    layers: State,
    controls: controls::Store,
    ids: Ids,
}
fn runtime(active: u8, selection: u8, clear: u8, mask: u16) -> Runtime {
    let owner = OwnerId::new(4).unwrap();
    let mut layers = State::from_empty_snapshot(
        Snapshot {
            active_count: active,
            selection_mask: selection,
            clear_mask: clear,
            slot_flags: [0x100, 0x200, 0x400, 0x800],
            weight_bits: [1f32.to_bits(), 0x7fc0_1234, 0, (-1f32).to_bits()],
        },
        Budget {
            nodes: 8,
            commands: 1,
            groups: 0,
            children: 0,
        },
    )
    .unwrap();
    let mut command = Pending::new(0x0c00_0005);
    let mut source = Operand { mask, reads: 0 };
    command
        .prepare(
            &mut source,
            &mut layers,
            SlotOverride {
                enabled_raw: 0,
                slot_raw: u32::MAX,
            },
        )
        .unwrap();
    assert_eq!(source.reads, 1);
    Runtime {
        command,
        layers,
        controls: controls::Store::from_constructor(owner, controls::Budget::default()),
        ids: Ids::from_constructor(owner, 16),
    }
}
fn locals() -> Vec<LocalPose> {
    vec![LocalPose {
        translation_bits: [1, 2, 3, 1f32.to_bits()],
        quaternion_bits: [0, 0, 0, 1f32.to_bits()],
    }]
}
fn refresh() -> Refresh {
    Scratch::default()
        .refresh_empty(&mut Arena::from_constructor(1).unwrap())
        .unwrap()
}
fn capture(r: &mut Runtime) -> super::super::super::NodeId {
    capture_in_arena(r, 1)
}
fn capture_in_arena(r: &mut Runtime, owner: u64) -> super::super::super::NodeId {
    let first = r
        .command
        .begin_mask_effects(&mut r.layers, &mut r.controls, &mut r.ids)
        .unwrap()
        .unwrap();
    assert_eq!(r.command.stage(), Stage::MaskRefreshPending);
    assert!(r.command.mask_effects().unwrap().refresh.is_none());
    assert_eq!(r.command.mask_effects().unwrap().selected_mask, None);
    r.command
        .record_mask_refresh(
            Scratch::default()
                .refresh_empty(&mut Arena::from_constructor(owner).unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(r.command.stage(), Stage::MaskBackingPending);
    assert!(r.command.mask_effects().unwrap().refresh.is_some());
    assert!(r.controls.get(first).unwrap().locals().is_err());
    r.controls.capture(first, locals(), vec![7, 9]).unwrap();
    r.command.mask_capture_returned().unwrap();
    assert_eq!(r.command.stage(), Stage::MaskEffectsPending);
    first
}
fn finish(r: &mut Runtime) -> Result<()> {
    r.command
        .finish_mask_effects(&mut r.layers, &mut r.controls, &mut r.ids)
}
fn old_head(r: &mut Runtime, slot: usize) -> super::super::super::NodeId {
    let id = r.controls.allocate(&mut r.ids).unwrap();
    r.controls.capture(id, locals(), vec![7, 9]).unwrap();
    r.layers.register_node(id).unwrap();
    r.ids.link_head(slot as u8, id, None).unwrap();
    r.layers.slots[slot].nodes.push(id);
    id
}

#[test]
fn mask3_on_one_active_slot_inserts_only_original_and_preserves_headers() {
    let mut r = runtime(1, 0xfb, 0xfb, 3);
    let flags = std::array::from_fn::<_, 4, _>(|i| r.layers.slots()[i].flags_raw());
    let weights = std::array::from_fn::<_, 4, _>(|i| r.layers.slots()[i].weight_bits());
    let first = capture(&mut r);
    assert_eq!(r.layers.created_nodes(), [first]);
    assert!(r.layers.slots()[0].nodes_head().is_empty());
    finish(&mut r).unwrap();
    assert_eq!(r.command.stage(), Stage::Returned);
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (
            e.active_slots_before,
            e.slots_visited,
            e.selected_mask,
            e.clear_mask
        ),
        (Some(1), 1, Some(3), Some(0xfb))
    );
    assert_eq!((e.controls_allocated, e.controls_inserted), (1, 1));
    assert!(e.capture_returned);
    assert_eq!(e.refresh.as_ref().unwrap().live_after(), 0);
    assert_eq!(e.first, Some(first));
    assert_eq!(r.layers.active_count(), 1);
    assert_eq!(
        (r.layers.selection_mask(), r.layers.clear_mask()),
        (0xfb, 0xfb)
    );
    assert_eq!(r.layers.slots()[0].nodes_head(), [first]);
    assert!(
        r.layers.slots()[1..]
            .iter()
            .all(|s| s.nodes_head().is_empty())
    );
    assert_eq!(
        r.ids.links(first).unwrap(),
        crate::animation::nodes::Links {
            slot: 0,
            previous: None,
            next: None
        }
    );
    assert_eq!(r.controls.len(), 1);
    assert_eq!(
        r.controls.get(first).unwrap().transition_bits(),
        1f32.to_bits()
    );
    assert_eq!(r.controls.get(first).unwrap().rate_bits(), 0);
    assert_eq!(r.controls.get(first).unwrap().position_bits(), 0);
    assert_eq!(r.layers.work.commands, 1);
    assert_eq!(r.layers.work.nodes, 1);
    for i in 0..4 {
        assert_eq!(r.layers.slots()[i].flags_raw(), flags[i]);
        assert_eq!(r.layers.slots()[i].weight_bits(), weights[i]);
    }
    assert!(finish(&mut r).is_err());
}

#[test]
fn inactive_mask_bits_still_allocate_capture_but_never_activate_a_slot() {
    let mut r = runtime(1, 0xfb, 0xfb, 2);
    let first = capture(&mut r);
    finish(&mut r).unwrap();
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (e.controls_allocated, e.controls_inserted, e.slots_visited),
        (1, 0, 1)
    );
    assert_eq!(e.selected_mask, Some(2));
    assert!(r.ids.links(first).is_none());
    assert!(r.layers.slots().iter().all(|s| s.nodes_head().is_empty()));
    assert_eq!(r.layers.active_count(), 1);
    assert_eq!(r.command.stage(), Stage::Returned);
}

#[test]
fn zero_mask_clears_empty_active_lists_without_allocating_or_refreshing() {
    let mut r = runtime(2, 0xff, 0xfb, 0);
    assert_eq!(
        r.command
            .begin_mask_effects(&mut r.layers, &mut r.controls, &mut r.ids)
            .unwrap(),
        None
    );
    finish(&mut r).unwrap();
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (
            e.active_slots_before,
            e.slots_visited,
            e.selected_mask,
            e.clear_mask
        ),
        (Some(2), 2, Some(0), Some(0xfb))
    );
    assert_eq!((e.controls_allocated, e.controls_inserted), (0, 0));
    assert!(!e.capture_returned && e.refresh.is_none() && e.first.is_none());
    assert!(r.controls.is_empty());
    assert_eq!(r.layers.work.nodes, 0);
    assert_eq!(r.layers.work.commands, 1);
    assert_eq!(r.command.stage(), Stage::Returned);
}

#[test]
fn selected_active_slots_clone_fresh_headers_and_independent_backing() {
    let mut r = runtime(4, 0xff, 0, 15);
    let first = capture(&mut r);
    finish(&mut r).unwrap();
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (e.controls_allocated, e.controls_inserted, e.slots_visited),
        (4, 4, 4)
    );
    assert_eq!(r.layers.slots()[0].nodes_head(), [first]);
    let original = r.controls.get(first).unwrap();
    for slot in 0..4 {
        let id = r.layers.slots()[slot].nodes_head()[0];
        let control = r.controls.get(id).unwrap();
        assert_eq!(
            r.ids.links(id).unwrap(),
            crate::animation::nodes::Links {
                slot: slot as u8,
                previous: None,
                next: None
            }
        );
        assert_eq!(control.locals().unwrap(), original.locals().unwrap());
        assert_eq!(control.auxiliary().unwrap(), original.auxiliary().unwrap());
        assert_eq!(control.transition_bits(), 1f32.to_bits());
        assert_eq!(control.rate_bits(), 0);
        if slot > 0 {
            assert_ne!(
                control.locals().unwrap().as_ptr(),
                original.locals().unwrap().as_ptr()
            );
            assert_ne!(
                control.auxiliary().unwrap().as_ptr(),
                original.auxiliary().unwrap().as_ptr()
            );
        }
    }
}

#[test]
fn original_goes_to_first_selected_active_slot_then_clones_follow() {
    let mut r = runtime(3, 0xff, 0, 6);
    let first = capture(&mut r);
    finish(&mut r).unwrap();
    assert!(r.layers.slots()[0].nodes_head().is_empty());
    assert_eq!(r.layers.slots()[1].nodes_head(), [first]);
    assert_ne!(r.layers.slots()[2].nodes_head()[0], first);
    let e = r.command.mask_effects().unwrap();
    assert_eq!((e.controls_allocated, e.controls_inserted), (2, 2));
}

#[test]
fn clone_budget_failure_keeps_original_insertion_refresh_and_rejects_replay() {
    let mut r = runtime(2, 0xff, 0, 3);
    r.layers.budget.nodes = 1;
    let first = capture(&mut r);
    assert!(finish(&mut r).is_err());
    assert_eq!(r.command.stage(), Stage::Failed);
    assert!(r.layers.failed());
    assert_eq!(r.layers.slots()[0].nodes_head(), [first]);
    assert!(r.layers.slots()[1].nodes_head().is_empty());
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (e.controls_allocated, e.controls_inserted, e.slots_visited),
        (1, 1, 2)
    );
    assert!(e.capture_returned && e.refresh.is_some());
    assert!(finish(&mut r).is_err());
    assert!(
        r.command
            .begin_mask_effects(&mut r.layers, &mut r.controls, &mut r.ids)
            .is_err()
    );
    assert_eq!(r.controls.len(), 1);
}

#[test]
fn genuine_old_clear_target_stays_intact_at_destructor_dependency() {
    let mut r = runtime(1, 0xff, 0xfb, 3);
    let old = old_head(&mut r, 0);
    let first = capture(&mut r);
    finish(&mut r).unwrap();
    assert_eq!(r.command.stage(), Stage::MaskDestroyPending);
    assert!(!r.layers.failed());
    assert_eq!(r.layers.slots()[0].nodes_head(), [old]);
    assert_eq!(r.ids.links(old).unwrap().previous, None);
    assert!(r.ids.links(first).is_none());
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (e.controls_allocated, e.controls_inserted, e.slots_visited),
        (1, 0, 1)
    );
    assert_eq!(e.destroy_pending, Some(old));
    assert!(e.capture_returned && e.refresh.is_some());
    assert!(finish(&mut r).is_err());
}

#[test]
fn retained_head_is_linked_behind_new_control_when_clear_bit_is_unset() {
    let mut r = runtime(1, 0xff, 0, 1);
    let old = old_head(&mut r, 0);
    let first = capture(&mut r);
    finish(&mut r).unwrap();
    assert_eq!(r.layers.slots()[0].nodes_head(), [first, old]);
    assert_eq!(r.ids.links(first).unwrap().next, Some(old));
    assert_eq!(r.ids.links(first).unwrap().previous, None);
    assert_eq!(r.ids.links(old).unwrap().previous, Some(first));
    assert_eq!(r.ids.links(old).unwrap().next, None);
    assert_eq!(r.layers.active_count(), 1);
}

#[test]
fn no_active_slots_skip_clear_mask_read_even_after_capture() {
    let mut r = runtime(0, 0xff, 0xfb, 1);
    capture(&mut r);
    finish(&mut r).unwrap();
    let e = r.command.mask_effects().unwrap();
    assert_eq!(
        (
            e.active_slots_before,
            e.slots_visited,
            e.selected_mask,
            e.clear_mask
        ),
        (Some(0), 0, Some(1), None)
    );
    assert_eq!((e.controls_allocated, e.controls_inserted), (1, 0));
    assert_eq!(r.layers.active_count(), 0);
}

#[test]
fn allocation_failure_retains_effect_entry_before_any_selection_read() {
    let mut r = runtime(1, 0xff, 0xfb, 3);
    r.layers.budget.nodes = 0;
    assert!(
        r.command
            .begin_mask_effects(&mut r.layers, &mut r.controls, &mut r.ids)
            .is_err()
    );
    let e = r.command.mask_effects().unwrap();
    assert_eq!(e.controls_allocated, 0);
    assert_eq!(e.first, None);
    assert_eq!(e.active_slots_before, None);
    assert_eq!(e.selected_mask, None);
    assert_eq!(e.clear_mask, None);
    assert!(e.refresh.is_none());
    assert_eq!(r.command.stage(), Stage::Failed);
    assert!(r.layers.failed());
    assert!(r.controls.is_empty());
}

#[test]
fn refresh_must_precede_backing_return_and_actual_backing_is_checked_before_masks() {
    let mut r = runtime(1, 0xff, 0xfb, 3);
    r.command
        .begin_mask_effects(&mut r.layers, &mut r.controls, &mut r.ids)
        .unwrap();
    assert!(r.command.mask_capture_returned().is_err());
    assert!(finish(&mut r).is_err());
    assert_eq!(r.command.stage(), Stage::MaskRefreshPending);
    r.command.record_mask_refresh(refresh()).unwrap();
    // Simulate a faulty caller's premature return token. It still cannot cause
    // insertion or selection reads without the actual store backing.
    r.command.mask_capture_returned().unwrap();
    assert!(finish(&mut r).is_err());
    assert_eq!(r.command.mask_effects().unwrap().selected_mask, None);
    assert_eq!(r.command.mask_effects().unwrap().controls_inserted, 0);
    assert_eq!(r.command.stage(), Stage::Failed);
    assert!(r.layers.failed());
}

#[test]
fn state_hash_distinguishes_real_refresh_owner_and_each_return_boundary() {
    fn words(command: &Pending) -> Vec<u32> {
        let mut words = Vec::new();
        command.visit_state_words(|v| words.push(v));
        words
    }
    let mut a = runtime(1, 0xff, 0, 1);
    let mut b = runtime(1, 0xff, 0, 1);
    let prepared = words(&a.command);
    capture(&mut a);
    let captured = words(&a.command);
    assert_ne!(prepared, captured);
    capture_in_arena(&mut b, 2);
    assert_ne!(captured, words(&b.command));
    finish(&mut a).unwrap();
    assert_ne!(captured, words(&a.command));
}
