use super::*;
use crate::mission::player::components::{Preparation, Registry};

fn owner() -> Animator {
    let player = PlayerId::new(9).unwrap();
    Animator::from_constructor(Registry::from_constructor(4).allocate(player).unwrap())
}

#[test]
fn empty_mode_snapshot_uses_actual_headers_and_keeps_mode_bytes_and_masks() {
    let mut owner = owner();
    owner.bytes[0x130] = Some(3);
    owner.bytes[0x131] = Some(0xb5);
    owner.bytes[0x136] = Some(0x6a);
    let flags = [0x8001, 0x0102, 0, 0xffff];
    let weights = [0.25f32.to_bits(), 0, 0x7fc0_1234, (-1f32).to_bits()];
    for i in 0..4 {
        owner.half_store(0x140 + i * 0x20, flags[i]);
        owner.word_store(0x144 + i * 0x20, weights[i]);
        owner.bytes[0x142 + i * 0x20] = Some(i as u8 + 1);
    }
    let bytes = owner.bytes;
    owner.bind_empty_mode_layers().unwrap();
    assert_eq!(owner.bytes, bytes);
    let layers = owner.layers().unwrap();
    assert_eq!(layers.active_count(), 3);
    assert_eq!(layers.selection_mask(), 0xb5);
    assert_eq!(layers.clear_mask(), 0x6a);
    for i in 0..4 {
        assert_eq!(layers.slots()[i].flags_raw(), flags[i]);
        assert_eq!(layers.slots()[i].weight_bits(), weights[i]);
        assert!(layers.slots()[i].nodes_head().is_empty());
    }
    assert!(owner.controls().is_empty());
    assert!(owner.script_children().is_empty());
    assert!(owner.capture_refresh().is_none());
    assert_eq!(owner.capture_active_count(), None);
}

#[test]
fn existing_mode_layers_are_not_replaced_or_resnapshotted() {
    let mut owner = owner();
    owner.bind_empty_mode_layers().unwrap();
    let first = owner.controls.allocate(&mut owner.node_ids).unwrap();
    // A retained owner is enough to prove the second call takes the existing
    // state path rather than rerunning the empty-owner preconditions.
    assert!(owner.controls.get(first).is_some());
    owner.bytes[0x138..0x140].fill(None);
    let mut before = Vec::new();
    owner
        .layers()
        .unwrap()
        .visit_state_words(|word| before.push(word));
    owner.bind_empty_mode_layers().unwrap();
    let mut after = Vec::new();
    owner
        .layers()
        .unwrap()
        .visit_state_words(|word| after.push(word));
    assert_eq!(before, after);
    assert_eq!(owner.controls.len(), 1);
    assert_eq!(owner.word(0x138), None);
}

#[test]
fn unknown_or_nonempty_native_lists_do_not_create_empty_host_layers() {
    for (at, value) in [(0x138, Some(1)), (0x15c, Some(1)), (0x178, None)] {
        let mut owner = owner();
        if let Some(value) = value {
            owner.word_store(at, value);
        } else {
            owner.bytes[at..at + 4].fill(None);
        }
        let bytes = owner.bytes;
        assert!(owner.bind_empty_mode_layers().is_err());
        assert!(owner.layers().is_none());
        assert_eq!(owner.bytes, bytes);
    }
}

#[test]
fn unknown_fields_and_invalid_active_count_preserve_unbound_state() {
    for at in [0x130, 0x131, 0x136, 0x140, 0x144] {
        let mut owner = owner();
        owner.bytes[at] = None;
        let bytes = owner.bytes;
        assert!(owner.bind_empty_mode_layers().is_err());
        assert!(owner.layers().is_none());
        assert_eq!(owner.bytes, bytes);
    }
    let mut owner = owner();
    owner.bytes[0x130] = Some(5);
    assert!(owner.bind_empty_mode_layers().is_err());
    assert!(owner.layers().is_none());
}

#[test]
fn retained_control_or_script_allocations_prevent_an_empty_snapshot() {
    for script in [false, true] {
        let mut owner = owner();
        if script {
            owner
                .script_children
                .allocate(&mut owner.node_ids, 0x0102_00ab, 0x8400_0001)
                .unwrap();
        } else {
            owner.controls.allocate(&mut owner.node_ids).unwrap();
        }
        assert!(owner.bind_empty_mode_layers().is_err());
        assert!(owner.layers().is_none());
    }
}

#[test]
fn receiver_lookup_and_binding_require_real_component_helper_returns() {
    let mut preparation = Preparation::new();
    let receiver = owner().identity();
    assert!(preparation.mode_animator(receiver).is_err());
    assert!(preparation.bind_empty_mode_layers(receiver).is_err());
    assert!(
        preparation
            .write_mode_dispatch_scalar(receiver, 1, 0)
            .is_err()
    );
}
