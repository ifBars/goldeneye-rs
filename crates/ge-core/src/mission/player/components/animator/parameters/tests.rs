use super::*;
use crate::{
    animation::{
        layers::{self, SlotOverride, command},
        script_child,
        skeleton::LocalPose,
    },
    mission::player::components::{Preparation, Registry},
};

fn owner() -> Animator {
    let player = PlayerId::new(9).unwrap();
    let mut owner =
        Animator::from_constructor(Registry::from_constructor(4).allocate(player).unwrap());
    owner.configure_headers();
    owner.bytes[0x131] = Some(0xfb);
    owner.bytes[0x136] = Some(0xfb);
    owner.layers = Some(
        layers::State::from_empty_snapshot(
            layers::Snapshot {
                active_count: 1,
                selection_mask: 0xfb,
                clear_mask: 0xfb,
                slot_flags: [1; 4],
                weight_bits: [1f32.to_bits(); 4],
            },
            layers::Budget {
                nodes: 8,
                commands: 8,
                groups: 8,
                children: 8,
            },
        )
        .unwrap(),
    );
    owner
}
fn with_controls() -> Animator {
    let mut owner = owner();
    let first = owner.controls.allocate(&mut owner.node_ids).unwrap();
    owner
        .controls
        .capture(
            first,
            vec![LocalPose {
                translation_bits: [0; 4],
                quaternion_bits: [0, 0, 0, 1f32.to_bits()],
            }],
            vec![],
        )
        .unwrap();
    owner
        .layers
        .as_mut()
        .unwrap()
        .capture_fresh_slots(&mut owner.controls, &mut owner.node_ids, first, 15)
        .unwrap();
    owner
}
struct Set;
impl command::Source for Set {
    fn set_reference(&mut self) -> Result<u32> {
        Ok(0x0a00_0001)
    }
    fn resolve_set(&mut self, _: u32) -> Result<Option<usize>> {
        Ok(Some(0))
    }
    fn options(&mut self) -> Result<i16> {
        Ok(0)
    }
    fn divisor_override(&mut self) -> Result<u32> {
        anyhow::bail!("disabled divisor override read")
    }
    fn flags_override(&mut self) -> Result<u16> {
        Ok(0x20)
    }
    fn begin_set(&mut self, _: usize) -> Result<usize> {
        Ok(1)
    }
    fn group_slot(&mut self) -> Result<u16> {
        Ok(1)
    }
    fn group_weight(&mut self) -> Result<u32> {
        Ok(1f32.to_bits())
    }
    fn child_count(&mut self) -> Result<i16> {
        Ok(1)
    }
    fn child_divisor(&mut self, _: usize) -> Result<u16> {
        Ok(0)
    }
    fn child_flags(&mut self, _: usize) -> Result<u16> {
        anyhow::bail!("overridden child flags read")
    }
    fn child_reference(&mut self, _: usize) -> Result<u32> {
        Ok(0x8400_0001)
    }
    fn child_word(&mut self, _: usize) -> Result<u32> {
        Ok(0)
    }
    fn next_group(&mut self, _: u16) -> Result<()> {
        Ok(())
    }
}
struct Body;
impl script_child::Provider for Body {
    fn file(&self) -> u32 {
        0x0102_00aa
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(0)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<script_child::Binding>> {
        Ok(Some(script_child::Binding {
            directory_index: 2,
            hash: 0x0400_00aa,
        }))
    }
    fn bind_body(&mut self, _: script_child::Binding) -> Result<bool> {
        Ok(false)
    }
}
fn with_script() -> (Animator, NodeId) {
    let mut owner = owner();
    let layers = owner.layers.as_mut().unwrap();
    let mut body = command::Pending::new(0x0c00_0002);
    body.prepare(
        &mut Set,
        layers,
        SlotOverride {
            enabled_raw: 0,
            slot_raw: u32::MAX,
        },
    )
    .unwrap();
    let request = body.begin_factory(layers).unwrap();
    let id = owner
        .script_children
        .allocate(&mut owner.node_ids, 0x0102_00aa, request.reference)
        .unwrap();
    body.retain_factory_node(layers, id).unwrap();
    owner
        .script_children
        .bind_factory_context(id, request.slot, request.effective_flags)
        .unwrap();
    owner
        .script_children
        .initialize_prefix(id, &mut Body)
        .unwrap();
    body.finish_script_child(
        layers,
        &mut owner.node_ids,
        &owner.controls,
        &mut owner.script_children,
        &mut Set,
    )
    .unwrap();
    body.advance_completed_child(
        &mut Set,
        layers,
        SlotOverride {
            enabled_raw: 0,
            slot_raw: u32::MAX,
        },
    )
    .unwrap();
    assert_eq!(body.stage(), command::Stage::Returned);
    owner.mirror_mode_layers();
    (owner, id)
}

#[test]
fn physical_weight_write_mirrors_raw_header_without_touching_other_parameters() {
    let mut owner = owner();
    let before = owner.bytes;
    let bits = 0x7fc0_1234;
    owner.set_parameter_weight(3, bits).unwrap();
    let mut expected = before;
    for (i, byte) in bits.to_be_bytes().into_iter().enumerate() {
        expected[0x1a4 + i] = Some(byte);
    }
    assert_eq!(owner.bytes, expected);
    assert_eq!(owner.layers().unwrap().slots()[3].weight_bits(), bits);
    assert_eq!(owner.layers().unwrap().active_count(), 1);
    for slot in 0..3 {
        assert_eq!(
            owner.layers().unwrap().slots()[slot].weight_bits(),
            1f32.to_bits()
        );
    }
    assert_eq!(
        (
            owner.layers().unwrap().selection_mask(),
            owner.layers().unwrap().clear_mask()
        ),
        (0xfb, 0xfb)
    );
    assert!(owner.set_parameter_weight(4, 0).is_err());
    assert_eq!(owner.bytes, expected);
}

#[test]
fn control_virtual_speed_is_not_filtered_by_script_subtype_or_active_count() {
    let mut owner = with_controls();
    let id = owner.layers().unwrap().slots()[3].nodes_head()[0];
    assert!(
        owner
            .set_parameter_child_speed(3, (-2f32).to_bits())
            .unwrap()
    );
    let control = owner.controls.get(id).unwrap();
    assert_eq!(control.speed_bits(), (-2f32).to_bits());
    assert_eq!(control.position_bits(), 0);
    assert_eq!(control.transition_bits(), 1f32.to_bits());
    assert_eq!(control.rate_bits(), 0);
    assert_eq!(owner.layers().unwrap().active_count(), 1);
    for slot in 0..3 {
        let id = owner.layers().unwrap().slots()[slot].nodes_head()[0];
        assert_eq!(owner.controls.get(id).unwrap().speed_bits(), 1f32.to_bits());
    }
}

#[test]
fn script_virtual_speed_keeps_factory_cache_and_retained_update_history() {
    let (mut owner, id) = with_script();
    let h = owner.script_children.get(id).unwrap();
    let before = (h.cached(), h.body_bound(), h.stage(), h.update_count());
    assert!(
        owner
            .set_parameter_child_speed(1, (-1f32).to_bits())
            .unwrap()
    );
    let h = owner.script_children.get(id).unwrap();
    assert_eq!(h.word(0x2c), Some((-1f32).to_bits()));
    assert_eq!(h.byte(0x77), Some(255));
    assert_eq!(h.word(0x68), Some(0));
    assert_eq!(h.word(0x84), Some(0));
    assert_eq!(
        (h.cached(), h.body_bound(), h.stage(), h.update_count()),
        before
    );
    assert_eq!(owner.layers().unwrap().slots()[1].nodes_head(), [id]);
}

#[test]
fn null_head_is_a_real_noop_and_missing_or_wrong_slot_ownership_is_not() {
    let mut empty = owner();
    assert!(!empty.set_parameter_child_speed(0, 2f32.to_bits()).unwrap());
    assert!(empty.set_parameter_child_speed(4, 2f32.to_bits()).is_err());
    let mut full = with_controls();
    let id = full.layers().unwrap().slots()[0].nodes_head()[0];
    // A different identity allocator cannot authorize the same numeric ID.
    full.node_ids = crate::animation::nodes::Ids::from_constructor(
        crate::animation::nodes::OwnerId::new(77).unwrap(),
        8,
    );
    assert!(full.set_parameter_child_speed(0, 2f32.to_bits()).is_err());
    assert_eq!(full.controls.get(id).unwrap().speed_bits(), 1f32.to_bits());
}

#[test]
fn public_parameter_methods_reject_receiver_use_before_component_returns() {
    let receiver = owner().identity();
    let mut preparation = Preparation::new();
    assert!(
        preparation
            .set_mode_speed(receiver, 1f32.to_bits())
            .is_err()
    );
    assert!(
        preparation
            .set_mode_weight(receiver, 1, 1f32.to_bits())
            .is_err()
    );
    assert!(
        preparation
            .set_mode_child_speed(receiver, 1, 1f32.to_bits())
            .is_err()
    );
}
