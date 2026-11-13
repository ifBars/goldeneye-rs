use super::*;

struct Body;
impl Provider for Body {
    fn file(&self) -> u32 {
        0x0102_00aa
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(0)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<Binding>> {
        Ok(Some(Binding {
            directory_index: 2,
            hash: 0x0400_00aa,
        }))
    }
    fn bind_body(&mut self, _: Binding) -> Result<bool> {
        Ok(false)
    }
}
fn fixture() -> (Store, NodeId) {
    let owner = OwnerId::new(3).unwrap();
    let mut ids = Ids::from_constructor(owner, 4);
    let mut store = Store::from_constructor(owner, 4);
    let id = store.allocate(&mut ids, 0x0102_00aa, 0x8400_0002).unwrap();
    store.bind_factory_context(id, 1, 0x27).unwrap();
    store.initialize_prefix(id, &mut Body).unwrap();
    assert_eq!(store.get(id).unwrap().stage(), Stage::ReturnedWithoutBody);
    (store, id)
}

#[test]
fn same_speed_sign_does_not_read_or_synchronize_unknown_history() {
    let (mut store, id) = fixture();
    let h = store.headers.get_mut(&id).unwrap();
    for at in [0x28, 0x68, 0x80, 0x84] {
        h.bytes[at..at + 4].fill(None);
    }
    h.bytes[0x77] = None;
    let write = store.set_speed(id, 0.5f32.to_bits()).unwrap();
    assert_eq!(
        write,
        SpeedWrite {
            previous_bits: 1f32.to_bits(),
            current_bits: 0.5f32.to_bits(),
            direction_changed: false
        }
    );
    let h = store.get(id).unwrap();
    assert_eq!(h.word(0x2c), Some(0.5f32.to_bits()));
    for at in [0x28, 0x68, 0x80, 0x84] {
        assert_eq!(h.word(at), None);
    }
    assert_eq!(h.byte(0x77), None);
    assert_eq!(h.stage(), Stage::ReturnedWithoutBody);
    assert!(h.initialization().is_none() && h.position().is_none() && h.updates().is_empty());
}

#[test]
fn changed_sign_copies_actual_position_and_phase_before_raw_speed_store() {
    let (mut store, id) = fixture();
    let h = store.headers.get_mut(&id).unwrap();
    h.w(0x28, 0.25f32.to_bits());
    h.w(0x80, 0.5f32.to_bits());
    let cache = (h.observed_generation(), h.cached(), h.body_bound());
    let write = store.set_speed(id, (-2f32).to_bits()).unwrap();
    assert!(write.direction_changed);
    let h = store.get(id).unwrap();
    assert_eq!(h.byte(0x77), Some(255));
    assert_eq!(h.word(0x68), Some(0.25f32.to_bits()));
    assert_eq!(h.word(0x84), Some(0.5f32.to_bits()));
    assert_eq!(h.word(0x2c), Some((-2f32).to_bits()));
    assert_eq!((h.observed_generation(), h.cached(), h.body_bound()), cache);
    assert!(h.initialization().is_none() && h.position().is_none() && h.updates().is_empty());
}

#[test]
fn failed_history_load_does_not_publish_direction_or_any_speed_prefix() {
    for unknown in [0x28, 0x80] {
        let (mut store, id) = fixture();
        let h = store.headers.get_mut(&id).unwrap();
        h.bytes[unknown..unknown + 4].fill(None);
        let before = h.bytes;
        assert!(store.set_speed(id, (-1f32).to_bits()).is_err());
        assert_eq!(store.get(id).unwrap().bytes, before);
    }
}

#[test]
fn direction_comparison_uses_old_speed_instead_of_retained_update_direction() {
    let (mut store, id) = fixture();
    let h = store.headers.get_mut(&id).unwrap();
    h.bytes[0x77] = Some(255);
    let before = (h.word(0x68), h.word(0x84));
    assert!(
        !store
            .set_speed(id, 2f32.to_bits())
            .unwrap()
            .direction_changed
    );
    let h = store.get(id).unwrap();
    assert_eq!(h.byte(0x77), Some(255));
    assert_eq!((h.word(0x68), h.word(0x84)), before);
}

#[test]
fn signed_zero_and_nan_use_ordered_comparison_and_preserve_input_bits() {
    let (mut store, id) = fixture();
    store.headers.get_mut(&id).unwrap().w(0x2c, 0);
    assert!(
        !store
            .set_speed(id, (-0f32).to_bits())
            .unwrap()
            .direction_changed
    );
    assert_eq!(store.get(id).unwrap().word(0x2c), Some(0x8000_0000));
    let nan = 0x7fc0_1234;
    assert!(!store.set_speed(id, nan).unwrap().direction_changed);
    assert_eq!(store.get(id).unwrap().word(0x2c), Some(nan));
    assert!(
        store
            .set_speed(id, 1f32.to_bits())
            .unwrap()
            .direction_changed
    );
    assert_eq!(store.get(id).unwrap().byte(0x77), Some(1));
    assert!(store.set_speed(id, nan).unwrap().direction_changed);
    assert_eq!(store.get(id).unwrap().byte(0x77), Some(0));
}

#[test]
fn constructor_only_or_unknown_old_speed_cannot_publish_a_parameter_write() {
    let (mut store, id) = fixture();
    store.headers.get_mut(&id).unwrap().bytes[0x2c..0x30].fill(None);
    let before = store.get(id).unwrap().bytes;
    assert!(store.set_speed(id, 2f32.to_bits()).is_err());
    assert_eq!(store.get(id).unwrap().bytes, before);
    store.headers.get_mut(&id).unwrap().stage = Stage::Constructed;
    assert!(store.set_speed(id, 2f32.to_bits()).is_err());
}

#[test]
fn paired_speed_formula_keeps_both_ratios_and_separate_single_rounding() {
    assert_eq!(
        blended_speeds(0, 30f32.to_bits(), 20f32.to_bits()),
        [0x3f80_0000, 0x3f2a_aaab]
    );
    assert_eq!(
        blended_speeds(0.5f32.to_bits(), 30f32.to_bits(), 20f32.to_bits()),
        [0x3fa0_0000, 0x3f55_5556]
    );
    assert_eq!(
        blended_speeds(1f32.to_bits(), 30f32.to_bits(), 20f32.to_bits()),
        [0x3fc0_0000, 0x3f80_0000]
    );
    assert_eq!(
        blended_speeds(0.3f32.to_bits(), 1f32.to_bits(), 3f32.to_bits()),
        [0x3f4c_cccd, 0x4019_999a]
    );
    let exceptional = blended_speeds(0, 0, 5f32.to_bits());
    assert!(f32::from_bits(exceptional[1]).is_nan());
}
