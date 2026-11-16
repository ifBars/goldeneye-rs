use super::*;
use std::sync::Arc;

// Authored synthetic captures; the owner token stands in for a host owner.
fn returned() -> (Store, NodeId) {
    let (mut store, _, id) = super::super::tests::fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    store.headers.get_mut(&id).unwrap().stage = super::super::Stage::ReturnedWithoutBody;
    (store, id)
}
fn clamp(token: &Arc<()>, sequence: u64, current: Option<u32>) -> OrderedUpperClamp {
    OrderedUpperClamp::new(
        ScalarCapture::new(9, Arc::downgrade(token), sequence, 0, current),
        0,
    )
}

#[test]
fn ordered_selection_preserves_exact_source_operand_bits() {
    let token = Arc::new(());
    let negative = (-0.25f32).to_bits();
    let nan = 0x7fc0_1234;
    for (process, selected) in [
        (0.5f32.to_bits(), 0),
        (negative, negative),
        (0, 0),
        (0x8000_0000, 0x8000_0000),
        (f32::NEG_INFINITY.to_bits(), f32::NEG_INFINITY.to_bits()),
        (f32::INFINITY.to_bits(), 0),
        (nan, 0),
        (0xffc0_0001, 0),
    ] {
        assert_eq!(
            clamp(&token, 1, Some(process)).selected_bits(),
            Some(selected),
            "process {process:#x}"
        );
    }
    assert_eq!(clamp(&token, 1, None).selected_bits(), None);
}

#[test]
fn unknown_capture_defers_phase_and_invalidates_stale_known_zero() {
    let (mut store, id) = returned();
    assert_eq!(store.get(id).unwrap().phase_bits().unwrap(), 0);
    let token = Arc::new(());
    let expression = clamp(&token, 1, None);
    assert_eq!(
        store.assign_phase_clamp(id, expression.clone()).unwrap(),
        PhaseAssignment::Deferred
    );
    let header = store.get(id).unwrap();
    assert_eq!(header.word(0x80), None);
    assert!(
        header
            .phase_bits()
            .unwrap_err()
            .downcast_ref::<DeferredPhase>()
            .is_some()
    );
    assert_eq!(header.phase_expression(), Some(&expression));
    // +84 is a different field and keeps its own value.
    assert_eq!(header.word(0x84), Some((-1f32).to_bits()));
}

#[test]
fn known_capture_stores_bits_and_concrete_writers_replace_the_expression() {
    let (mut store, id) = returned();
    let token = Arc::new(());
    let negative = (-2f32).to_bits();
    assert_eq!(
        store
            .assign_phase_clamp(id, clamp(&token, 1, Some(negative)))
            .unwrap(),
        PhaseAssignment::Known(negative)
    );
    assert_eq!(store.get(id).unwrap().phase_bits().unwrap(), negative);
    assert!(store.get(id).unwrap().phase_expression().is_none());
    store
        .assign_phase_clamp(id, clamp(&token, 2, None))
        .unwrap();
    store.copy_parent_phase(id, 0x3e80_0000).unwrap();
    let header = store.get(id).unwrap();
    assert!(header.phase_expression().is_none());
    assert_eq!(header.phase_bits().unwrap(), 0x3e80_0000);
}

#[test]
fn captures_compare_by_owner_instance_and_read_identity() {
    let first = Arc::new(());
    let second = Arc::new(());
    let a = ScalarCapture::new(9, Arc::downgrade(&first), 1, 0, None);
    assert_eq!(a, ScalarCapture::new(9, Arc::downgrade(&first), 1, 0, None));
    assert_ne!(
        a,
        ScalarCapture::new(9, Arc::downgrade(&second), 1, 0, None)
    );
    assert_ne!(a, ScalarCapture::new(9, Arc::downgrade(&first), 2, 0, None));
}
