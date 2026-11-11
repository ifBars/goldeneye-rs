use super::*;
use crate::animation::layers::{Snapshot, State};
use crate::mission::player::{components::Registry, identity::PlayerId};

fn store(nodes: usize) -> Store {
    let identity = Registry::from_constructor(1)
        .allocate(PlayerId::new(4).unwrap())
        .unwrap();
    Store::from_constructor(
        identity,
        Budget {
            nodes,
            bones: 4,
            auxiliary_words: 8,
        },
    )
}
fn locals() -> Vec<LocalPose> {
    vec![
        LocalPose {
            translation_bits: [0, 0, 0, 1f32.to_bits()],
            quaternion_bits: [0, 0, 0, 1f32.to_bits()]
        };
        2
    ]
}
fn layers() -> State {
    State::from_empty_snapshot(
        Snapshot {
            active_count: 1,
            selection_mask: 0xff,
            clear_mask: 0xff,
            slot_flags: [1; 4],
            weight_bits: [1f32.to_bits(); 4],
        },
        crate::animation::layers::Budget {
            nodes: 4,
            commands: 16,
            groups: 16,
            children: 16,
        },
    )
    .unwrap()
}
#[test]
fn snapshot_copies_auxiliary_and_four_distinct_physical_controls_without_activating_slots() {
    let mut store = store(4);
    let first = store.allocate().unwrap();
    assert!(store.get(first).unwrap().locals().is_err());
    store
        .capture(first, locals(), vec![0, 0x3f80_0000, 0, 0])
        .unwrap();
    let mut layers = layers();
    layers.capture_fresh_slots(&mut store, first, 0xf).unwrap();
    assert_eq!(store.len(), 4);
    assert_eq!(layers.active_count(), 1);
    let source = store.get(first).unwrap();
    for slot in layers.slots() {
        assert_eq!(slot.nodes_head().len(), 1);
        let control = store.get(slot.nodes_head()[0]).unwrap();
        assert_eq!(control.locals().unwrap(), locals());
        assert_eq!(control.auxiliary().unwrap(), &[0, 0x3f80_0000, 0, 0]);
        assert_eq!(control.transition_bits(), 1f32.to_bits());
        assert_eq!(control.rate_bits(), 0);
        assert_eq!(control.flags(), 0);
        if slot.nodes_head()[0] != first {
            assert_ne!(
                control.locals().unwrap().as_ptr(),
                source.locals().unwrap().as_ptr()
            );
            assert_ne!(
                control.auxiliary().unwrap().as_ptr(),
                source.auxiliary().unwrap().as_ptr()
            );
        }
    }
    assert_eq!(layers.created_nodes().len(), 4);
    assert!(layers.capture_fresh_slots(&mut store, first, 0xf).is_err());
}
#[test]
fn clone_budget_failure_retains_header_and_inserted_prefix_and_latches_layer_failure() {
    let mut store = store(2);
    let first = store.allocate().unwrap();
    store.capture(first, locals(), vec![0; 4]).unwrap();
    let mut layers = layers();
    assert!(layers.capture_fresh_slots(&mut store, first, 0xf).is_err());
    assert!(layers.failed());
    assert_eq!(store.len(), 2);
    assert_eq!(layers.active_count(), 1);
    assert_eq!(layers.slots()[0].nodes_head().len(), 1);
    assert_eq!(layers.slots()[1].nodes_head().len(), 1);
    assert!(layers.slots()[2].nodes_head().is_empty());
    assert!(layers.slots()[3].nodes_head().is_empty());
    assert!(layers.capture_fresh_slots(&mut store, first, 0xf).is_err());
}
#[test]
fn unbound_payloads_cannot_be_published_as_captured_controls() {
    let mut store = store(4);
    let first = store.allocate().unwrap();
    let mut layers = layers();
    assert!(layers.capture_fresh_slots(&mut store, first, 0xf).is_err());
    assert!(
        layers
            .slots()
            .iter()
            .all(|slot| slot.nodes_head().is_empty())
    );
    assert!(store.capture(first, vec![locals()[0]; 5], vec![]).is_err());
    assert!(store.get(first).unwrap().locals().is_err());
    assert_eq!(store.len(), 1);
}
