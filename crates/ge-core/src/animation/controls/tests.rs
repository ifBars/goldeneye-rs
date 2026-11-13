use super::*;
use crate::animation::layers::{Snapshot, State};

fn store(nodes: usize) -> Store {
    let identity = OwnerId::new(4).unwrap();
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
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 8);
    let first = store.allocate(&mut ids).unwrap();
    assert!(store.get(first).unwrap().locals().is_err());
    store
        .capture(first, locals(), vec![0, 0x3f80_0000, 0, 0])
        .unwrap();
    let mut layers = layers();
    layers
        .capture_fresh_slots(&mut store, &mut ids, first, 0xf)
        .unwrap();
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
    assert!(
        layers
            .capture_fresh_slots(&mut store, &mut ids, first, 0xf)
            .is_err()
    );
}
#[test]
fn clone_budget_failure_retains_header_and_inserted_prefix_and_latches_layer_failure() {
    let mut store = store(2);
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 8);
    let first = store.allocate(&mut ids).unwrap();
    store.capture(first, locals(), vec![0; 4]).unwrap();
    let mut layers = layers();
    assert!(
        layers
            .capture_fresh_slots(&mut store, &mut ids, first, 0xf)
            .is_err()
    );
    assert!(layers.failed());
    assert_eq!(store.len(), 2);
    assert_eq!(layers.active_count(), 1);
    assert_eq!(layers.slots()[0].nodes_head().len(), 1);
    assert_eq!(layers.slots()[1].nodes_head().len(), 1);
    assert!(layers.slots()[2].nodes_head().is_empty());
    assert!(layers.slots()[3].nodes_head().is_empty());
    assert!(
        layers
            .capture_fresh_slots(&mut store, &mut ids, first, 0xf)
            .is_err()
    );
}
#[test]
fn unbound_payloads_cannot_be_published_as_captured_controls() {
    let mut store = store(4);
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 8);
    let first = store.allocate(&mut ids).unwrap();
    let mut layers = layers();
    assert!(
        layers
            .capture_fresh_slots(&mut store, &mut ids, first, 0xf)
            .is_err()
    );
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

#[test]
fn controls_and_scripts_share_one_sequence_without_aliasing_later_clones() {
    let mut store = store(8);
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 8);
    let first = store.allocate(&mut ids).unwrap();
    store.capture(first, locals(), vec![]).unwrap();
    let script = ids.allocate(Kind::Script).unwrap();
    let clone = store.clone_control(first, &mut ids).unwrap();
    assert_eq!((first.get(), script.get(), clone.get()), (1, 2, 3));
    assert_eq!(ids.kind(script), Some(Kind::Script));
    assert_eq!(ids.kind(clone), Some(Kind::Control));
    assert!(store.get(script).is_none());
    assert!(store.get(clone).is_some());
    let mut other = Ids::from_constructor(OwnerId::new(5).unwrap(), 8);
    assert!(store.allocate(&mut other).is_err());
    assert_eq!(other.allocate(Kind::Control).unwrap().get(), 1);
    let mut limited = Ids::from_constructor(OwnerId::new(4).unwrap(), 0);
    assert!(store.allocate(&mut limited).is_err());
    assert_eq!(store.len(), 2);
}

#[test]
fn common_speed_defaults_to_one_and_raw_setter_preserves_other_header_words() {
    let mut store = store(2);
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 4);
    let id = store.allocate(&mut ids).unwrap();
    assert_eq!(store.get(id).unwrap().speed_bits(), 1f32.to_bits());
    let control = store.controls.get_mut(&id).unwrap();
    control.rate_bits = 0x3dcc_cccd;
    control.transition_bits = 0.25f32.to_bits();
    control.position_bits = 0.75f32.to_bits();
    control.flags = 0x204;
    for bits in [(-2f32).to_bits(), 0x8000_0000, 0x7fc0_1234] {
        store.set_speed(id, bits).unwrap();
        let control = store.get(id).unwrap();
        assert_eq!(control.speed_bits(), bits);
        assert_eq!(control.rate_bits(), 0x3dcc_cccd);
        assert_eq!(control.transition_bits(), 0.25f32.to_bits());
        assert_eq!(control.position_bits(), 0.75f32.to_bits());
        assert_eq!(control.flags(), 0x204);
        assert!(control.locals().is_err() && control.auxiliary().is_err());
    }
}

#[test]
fn clone_copies_backing_but_uses_fresh_speed_and_scalar_constructor_words() {
    let mut store = store(2);
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 4);
    let first = store.allocate(&mut ids).unwrap();
    store.capture(first, locals(), vec![3, 5, 8]).unwrap();
    store.set_speed(first, (-2f32).to_bits()).unwrap();
    let original = store.controls.get_mut(&first).unwrap();
    original.rate_bits = 0.5f32.to_bits();
    original.transition_bits = 0.25f32.to_bits();
    original.position_bits = 0.75f32.to_bits();
    original.flags = 0x204;
    let clone = store.clone_control(first, &mut ids).unwrap();
    let original = store.get(first).unwrap();
    let cloned = store.get(clone).unwrap();
    assert_ne!(first, clone);
    assert_eq!(original.speed_bits(), (-2f32).to_bits());
    assert_eq!(original.rate_bits(), 0.5f32.to_bits());
    assert_eq!(cloned.speed_bits(), 1f32.to_bits());
    assert_eq!(cloned.rate_bits(), 0);
    assert_eq!(cloned.transition_bits(), 1f32.to_bits());
    assert_eq!(cloned.position_bits(), 0);
    assert_eq!(cloned.flags(), 0);
    assert_eq!(cloned.locals().unwrap(), original.locals().unwrap());
    assert_eq!(cloned.auxiliary().unwrap(), original.auxiliary().unwrap());
    assert_ne!(
        cloned.locals().unwrap().as_ptr(),
        original.locals().unwrap().as_ptr()
    );
    assert_ne!(
        cloned.auxiliary().unwrap().as_ptr(),
        original.auxiliary().unwrap().as_ptr()
    );
}
