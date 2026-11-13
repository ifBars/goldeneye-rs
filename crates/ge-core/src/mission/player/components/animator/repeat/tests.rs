use super::*;
use crate::{
    animation::{layers, layers::command, script_child},
    mission::player::components::{Preparation, Registry},
};

fn owner() -> Animator {
    let player = PlayerId::new(9).unwrap();
    let mut owner =
        Animator::from_constructor(Registry::from_constructor(4).allocate(player).unwrap());
    owner.configure_headers();
    owner.bind_empty_mode_layers().unwrap();
    owner
}

fn controls() -> Animator {
    let mut owner = owner();
    let id = owner.controls.allocate(&mut owner.node_ids).unwrap();
    owner.controls.capture(id, vec![], vec![]).unwrap();
    owner
        .layers
        .as_mut()
        .unwrap()
        .capture_fresh_slots(&mut owner.controls, &mut owner.node_ids, id, 15)
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
        anyhow::bail!("disabled divisor override")
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
        anyhow::bail!("overridden child flags")
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
fn script() -> (Animator, NodeId) {
    let mut owner = owner();
    let layers = owner.layers.as_mut().unwrap();
    let mut command = command::Pending::new(0x0c00_0002);
    let overrides = layers::SlotOverride {
        enabled_raw: 0,
        slot_raw: u32::MAX,
    };
    command.prepare(&mut Set, layers, overrides).unwrap();
    let request = command.begin_factory(layers).unwrap();
    let id = owner
        .script_children
        .allocate(&mut owner.node_ids, 0x0102_00aa, request.reference)
        .unwrap();
    command.retain_factory_node(layers, id).unwrap();
    owner
        .script_children
        .bind_factory_context(id, request.slot, request.effective_flags)
        .unwrap();
    owner
        .script_children
        .initialize_prefix(id, &mut Body)
        .unwrap();
    command
        .finish_script_child(
            layers,
            &mut owner.node_ids,
            &owner.controls,
            &mut owner.script_children,
            &mut Set,
        )
        .unwrap();
    command
        .advance_completed_child(&mut Set, layers, overrides)
        .unwrap();
    assert_eq!(command.stage(), command::Stage::Returned);
    (owner, id)
}

#[test]
fn null_head_returns_default_and_seek_is_a_noop() {
    let mut owner = owner();
    let before = owner.bytes;
    assert_eq!(
        owner.animation_position(3).unwrap(),
        AnimationPosition {
            head: None,
            matched: false,
            bits: 0,
        }
    );
    assert_eq!(owner.begin_seek(3, 0x7fc0_1234).unwrap(), None);
    assert_eq!(owner.bytes, before);
    assert!(owner.script_children.is_empty());
}

#[test]
fn control_head_remains_visible_but_never_matches_animation_or_seeks() {
    let mut owner = controls();
    let id = owner.layers().unwrap().slots()[3].nodes_head()[0];
    let mut before = Vec::new();
    owner.controls.visit_state_words(|word| before.push(word));
    assert_eq!(
        owner.animation_position(3).unwrap(),
        AnimationPosition {
            head: Some(id),
            matched: false,
            bits: 0,
        }
    );
    assert_eq!(owner.begin_seek(3, (-1f32).to_bits()).unwrap(), None);
    let mut after = Vec::new();
    owner.controls.visit_state_words(|word| after.push(word));
    assert_eq!(before, after);
    assert_eq!(owner.layers().unwrap().active_count(), 1);
}

#[test]
fn script_seek_writes_only_both_phase_words_without_running_an_update() {
    let (mut owner, id) = script();
    let header = owner.script_children.get(id).unwrap();
    let before = std::array::from_fn::<_, { script_child::HEADER_BYTES }, _>(|i| header.byte(i));
    let history = (
        header.cached(),
        header.body_bound(),
        header.stage(),
        header.update_count(),
    );
    let bits = 0x7fc0_1234u32;
    assert_eq!(owner.begin_seek(1, bits).unwrap(), Some(id));
    let header = owner.script_children.get(id).unwrap();
    let mut expected = before;
    for start in [0x80, 0x84] {
        for (i, byte) in bits.to_be_bytes().into_iter().enumerate() {
            expected[start + i] = Some(byte);
        }
    }
    assert_eq!(
        std::array::from_fn::<_, { script_child::HEADER_BYTES }, _>(|i| header.byte(i)),
        expected
    );
    assert_eq!(
        (
            header.cached(),
            header.body_bound(),
            header.stage(),
            header.update_count()
        ),
        history
    );
    assert!(header.updates().is_empty());
    // Getter reads +28, not the newly changed +80/+84 or a control getter.
    assert_eq!(
        owner.animation_position(1).unwrap(),
        AnimationPosition {
            head: Some(id),
            matched: true,
            bits: before[0x28..0x2c]
                .iter()
                .map(|byte| byte.unwrap())
                .fold(0u32, |word, byte| (word << 8) | u32::from(byte)),
        }
    );
}

#[test]
fn invalid_slots_or_missing_typed_owners_error_without_writing() {
    let mut empty = owner();
    assert!(empty.animation_position(4).is_err());
    assert!(empty.begin_seek(4, 1).is_err());
    let (mut owner, _) = script();
    owner.script_children = script_child::Store::from_constructor(owner.node_ids.owner(), 4);
    assert!(owner.animation_position(1).is_err());
    assert!(owner.begin_seek(1, 1).is_err());
    assert!(owner.script_children.is_empty());
}

#[test]
fn public_helpers_reject_calls_before_actual_component_returns() {
    let receiver = owner().identity();
    let mut preparation = Preparation::new();
    assert!(preparation.mode_animation_position(receiver, 0).is_err());
    assert!(preparation.begin_mode_seek(receiver, 0, 1).is_err());
}
