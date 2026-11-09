use super::*;
use ge_core::animation::{
    layers::Host,
    mode::{Child, ChildPlan, Insertion},
};
use std::sync::Arc;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], at: usize, target: usize) {
    word(data, at, (target as i64 - at as i64) as i32 as u32);
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x600];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00f1),
        (8, 263),
        (0x14, 0x600),
        (0x18, 0x600),
        (0x58, 0x0001_0001),
        (0x68, 0x0001_0001),
        (0x100, 0x0400_00f1),
        (0x108, 0x200),
        (0x10c, u32::MAX),
        (0x140, 0x0300_00f1),
        (0x148, 0x400),
        (0x14c, u32::MAX),
        (0x150, 0x500),
        (0x158, 16),
        (0x200, 0x300),
        (0x204, 8),
        (0x20c, 30.0f32.to_bits()),
        (0x23c, 0x0002_0000),
        (0x240, 4),
        (0x244, 0x023c_0000),
        (0x248, 8),
        (0x250, u32::MAX),
        (0x254, 0x0200_0000),
        (0x258, u32::MAX),
        (0x25c, 0x0d00_00f1),
        (0x260, u32::MAX),
        (0x264, 0x8300_0000),
        (0x268, 0),
        (0x26c, 8),
        (0x270, u32::MAX),
        (0x274, 0x8300_0000),
        (0x278, 1.0f32.to_bits()),
        (0x27c, 8),
        (0x280, 0x1200_0000),
        (0x400, 0x102),
        (0x404, 0x0300_00f1),
        (0x40c, 0x1e02_0008),
        (0x414, 0x0201_0000),
        (0x460, 1),
    ] {
        word(&mut data, at, value);
    }
    for (at, target) in [(0x5c, 0x140), (0x6c, 0x100), (0x234, 0x2c0), (0x2c0, 0x2d0)] {
        relative(&mut data, at, target);
    }
    data
}
fn capture() -> ControlCapture {
    ControlCapture {
        bones: vec![
            ControlBone {
                translation_bits: [0, 0, 0, 1.0f32.to_bits()],
                quaternion_bits: [0, 0, 0, 1.0f32.to_bits()]
            };
            2
        ],
        auxiliary_count: 0,
    }
}
fn child() -> ChildPlan {
    ChildPlan {
        source_ordinal: 0,
        source: Child {
            source_address: 0,
            reference: 0x8400_0000,
            flags_raw: 0x23,
            divisor_raw: 6,
            word8_raw: 0x1234_5678,
        },
        effective_flags: 0x23,
        divisor_u32: 6,
        reciprocal_divisor_bits: (1.0f32 / 6.0).to_bits(),
        scalar_zero_bits: 0,
        factory_class: 4,
        insertion: Insertion::Head,
    }
}

#[test]
fn actual_constructor_nodes_retain_ordered_duplicate_clips_shared_spans_and_scalars() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let control = host.create_control().unwrap().unwrap();
    let script = host.create_child(0, &child()).unwrap().unwrap();
    host.initialize_child(script, &child()).unwrap();
    let owned = host.finish();
    assert_eq!(
        (owned.len(), owned.control_count(), owned.script_count()),
        (2, 1, 1)
    );
    assert_eq!(
        (
            owned.clip_count(),
            owned.animation_count(),
            owned.capture_bone_count()
        ),
        (2, 1, 2)
    );
    assert_eq!(owned.reference(control), None);
    assert_eq!(owned.reference(script), Some(0x8400_0000));
    let Node::Script(bound) = &owned.nodes[&script] else {
        panic!("script owner missing");
    };
    assert_eq!(
        bound
            .clips
            .iter()
            .map(|clip| clip.source.blend_key_bits)
            .collect::<Vec<_>>(),
        vec![0, 1.0f32.to_bits()]
    );
    assert!(Arc::ptr_eq(
        &bound.clips[0].animation,
        &bound.clips[1].animation
    ));
    assert_eq!(
        bound.clips[0]
            .animation
            .compressed_payload(&edb)
            .unwrap()
            .len(),
        16
    );
    assert_eq!(bound.transition_scalar_bits, 0);
    assert_eq!(bound.reciprocal_divisor_bits, (1.0f32 / 6.0).to_bits());
    assert_eq!(bound.word8_raw, 0x1234_5678);
    assert_eq!(owned.bindings_sha256().len(), 64);
}

#[test]
fn unsupported_reached_binding_inputs_never_manufacture_child_owners() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let mut absent = child();
    absent.source.reference = 0;
    absent.factory_class = 0;
    assert!(host.create_child(0, &absent).unwrap().is_none());
    assert_eq!(host.finish().len(), 0);
    for (at, value) in [
        (0x260, 0x0100_00f1),
        (0x254, 0x0100_0001),
        (0x26c, 0),
        (0x2d4, 1),
        (0x244, 0x033c_0000),
        (0x248, 0x0001_0008),
    ] {
        let mut bytes = fixture();
        word(&mut bytes, at, value);
        let edb = Edb::parse(bytes).unwrap();
        let mut host = NodeHost::new(&edb, capture());
        assert!(host.create_child(0, &child()).is_err());
        assert_eq!(host.finish().len(), 0);
    }
    let edb = Edb::parse(fixture()).unwrap();
    let mut bad_capture = capture();
    bad_capture.auxiliary_count = 1;
    let mut host = NodeHost::new(&edb, bad_capture);
    assert!(host.create_control().is_err());
}

#[test]
fn known_zero_head_value_copies_constructor_phase_and_destroy_drops_real_nodes() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let a = host.create_child(0, &child()).unwrap().unwrap();
    let b = host.create_child(0, &child()).unwrap().unwrap();
    let before = host.finish().bindings_sha256();
    let mut host = NodeHost::new(&edb, capture());
    let a2 = host.create_child(0, &child()).unwrap().unwrap();
    let b2 = host.create_child(0, &child()).unwrap().unwrap();
    assert_eq!((a, b), (a2, b2));
    host.copy_head(b2, a2).unwrap();
    let after = host.finish().bindings_sha256();
    assert_ne!(before, after); // Previous phase -1 changed to copied zero.
    let mut host = NodeHost::new(&edb, capture());
    let control = host.create_control().unwrap().unwrap();
    let script = host.create_child(0, &child()).unwrap().unwrap();
    assert!(host.copy_head(script, control).is_err());
    host.destroy_node(script).unwrap();
    assert!(host.destroy_node(script).is_err());
    assert_eq!(host.finish().len(), 1);
}

#[test]
fn blend_setter_skips_empty_or_control_head_and_rejects_dangling_owner() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let control = host.create_control().unwrap().unwrap();
    let script = host.create_child(0, &child()).unwrap().unwrap();
    assert!(!host.set_head_blend(&[], 1.0f32.to_bits()).unwrap());
    assert!(
        !host
            .set_head_blend(&[control, script], 1.0f32.to_bits())
            .unwrap()
    );
    assert!(host.set_head_blend(&[NodeId::new(99).unwrap()], 0).is_err());
    assert_eq!(host.head_blend_writes(), 0);
    let owned = host.finish();
    assert_eq!(owned.script(script).unwrap().blend_scalar_bits, 0);
    assert!(owned.script(control).is_none());
    assert!(owned.script(NodeId::new(99).unwrap()).is_none());
}

#[test]
fn blend_setter_counts_same_zero_writes_and_changes_only_expected_head_scalar() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let first = host.create_child(0, &child()).unwrap().unwrap();
    let second = host.create_child(0, &child()).unwrap().unwrap();
    host.initialize_child(first, &child()).unwrap();
    assert!(host.set_head_blend(&[first, second], 0).unwrap());
    assert!(host.set_head_blend(&[first, second], 0).unwrap());
    assert_eq!(host.head_blend_writes(), 2);
    let marker = 0x7fc1_2345;
    assert!(host.set_head_blend(&[first, second], marker).unwrap());
    let owned = host.finish();
    assert_eq!(owned.head_blend_writes(), 3);
    let bound = owned.script(first).unwrap();
    assert_eq!(bound.blend_scalar_bits, marker); // Raw input is not sanitized.
    assert_eq!(bound.normalized_position_bits, 0);
    assert_eq!(bound.script_phase_bits, 0);
    assert_eq!(bound.previous_phase_bits, (-1.0f32).to_bits());
    assert_eq!(bound.transition_scalar_bits, 0);
    assert_eq!(bound.reciprocal_divisor_bits, (1.0f32 / 6.0).to_bits());
    assert_eq!(bound.word8_raw, child().source.word8_raw);
    assert_eq!(owned.script(second).unwrap().blend_scalar_bits, 0);
}

#[test]
fn separate_source_slot_calls_write_independent_heads_without_touching_control() {
    use super::super::scalars::SlotInputs;
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let control = host.create_control().unwrap().unwrap();
    let primary = host.create_child(1, &child()).unwrap().unwrap();
    assert!(host.set_head_blend(&[primary], 0).unwrap());
    let secondary = host.create_child(0, &child()).unwrap().unwrap();
    let value = SlotInputs {
        player_9a0_bits: 0,
        player_ec4_bits: 1.0f32.to_bits(),
        upper_tuning_bits: 0.5f32.to_bits(),
        lower_tuning_bits: 0,
    }
    .secondary_blend_bits();
    assert_eq!(value, 0);
    assert!(host.set_head_blend(&[secondary, control], value).unwrap());
    let owned = host.finish();
    assert_eq!(owned.head_blend_writes(), 2);
    assert_eq!(owned.script(primary).unwrap().slot, 1);
    assert_eq!(owned.script(secondary).unwrap().slot, 0);
    assert_eq!(owned.control_count(), 1);
    assert_eq!(owned.capture_bone_count(), 2);
}

#[test]
fn secondary_clamp_preserves_upper_first_order_ties_and_unordered_value() {
    use super::super::scalars::SlotInputs;
    let clamp = |value: f32, ec4: f32, upper: f32, lower: f32| {
        SlotInputs {
            player_9a0_bits: value.to_bits(),
            player_ec4_bits: ec4.to_bits(),
            upper_tuning_bits: upper.to_bits(),
            lower_tuning_bits: lower.to_bits(),
        }
        .secondary_blend_bits()
    };
    assert_eq!(clamp(0.0, 1.0, 0.5, 0.0), 0);
    assert_eq!(clamp(0.75, 1.0, 0.5, 0.0), 0.5f32.to_bits());
    assert_eq!(clamp(0.25, 0.5, 0.5, 0.0), 0.25f32.to_bits());
    assert_eq!(clamp(0.75, 0.5, 0.5, 0.0), 0.75f32.to_bits());
    assert_eq!(clamp(-0.0, 1.0, 0.5, 0.0), 0); // Lower tie uses stored +0.
    assert_eq!(clamp(0.5, 1.0, 0.5, 1.0), 0.5f32.to_bits());
    let nan = f32::from_bits(0x7fc1_2345);
    assert_eq!(clamp(nan, 1.0, 0.5, 0.0), nan.to_bits());
}

#[test]
fn direct_script_phase_assignment_preserves_raw_bits_and_other_child_state() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let id = host.create_child(0, &child()).unwrap().unwrap();
    let Node::Script(bound) = host.nodes.get_mut(&id).unwrap() else {
        unreachable!()
    };
    assert_eq!(bound.previous_phase_bits, (-1.0f32).to_bits());
    bound.assign_script_phase((-0.0f32).to_bits());
    assert_eq!(
        (bound.script_phase_bits, bound.previous_phase_bits),
        (0x8000_0000, 0x8000_0000)
    );
    bound.assign_script_phase(0x7fc1_2345);
    assert_eq!(
        (bound.script_phase_bits, bound.previous_phase_bits),
        (0x7fc1_2345, 0x7fc1_2345)
    );
    assert_eq!(bound.normalized_position_bits, 0);
    assert_eq!(bound.blend_scalar_bits, 0);
    assert_eq!(bound.transition_scalar_bits, 1.0f32.to_bits());
}

#[test]
fn explicit_position_conversions_assign_instead_of_adding_and_distinguish_rate() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    let id = host.create_child(0, &child()).unwrap().unwrap();
    let Node::Script(bound) = host.nodes.get_mut(&id).unwrap() else {
        unreachable!()
    };
    // Authored duration8/FPS30. Calls explicitly assign script cursor fields;
    // they neither change normalized clip position nor request a decoded frame.
    bound.assign_rate_scaled_position(2.0f32.to_bits());
    assert_eq!(bound.script_phase_bits, 7.5f32.to_bits());
    bound.assign_rate_scaled_position(1.0f32.to_bits());
    assert_eq!(bound.script_phase_bits, 3.75f32.to_bits());
    assert_eq!(bound.previous_phase_bits, 3.75f32.to_bits());
    bound.assign_position(2.0f32.to_bits());
    assert_eq!(bound.script_phase_bits, 0.25f32.to_bits());
    assert_eq!(bound.previous_phase_bits, 0.25f32.to_bits());
    assert_eq!(bound.normalized_position_bits, 0);
    assert_eq!(bound.blend_scalar_bits, 0);
    assert_eq!(bound.transition_scalar_bits, 1.0f32.to_bits());
}

#[test]
fn fresh_control_capture_refresh_clears_update_suppression_before_retaining_locals() {
    let edb = Edb::parse(fixture()).unwrap();
    let mut host = NodeHost::new(&edb, capture());
    assert_eq!(host.update_flags_raw, 0xb);
    host.create_control().unwrap().unwrap();
    assert_eq!(host.finish().update_flags_raw(), 3);
    let mut active = NodeHost::new(&edb, capture());
    active.create_child(0, &child()).unwrap().unwrap();
    assert!(active.create_control().is_err());
    assert_eq!(active.update_flags_raw, 0xb);
}
