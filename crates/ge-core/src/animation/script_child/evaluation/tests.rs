use super::*;
use std::cell::Cell;

// Independent synthetic header fixtures; no original payload or address.
fn returned() -> (Store, NodeId) {
    let (mut store, _, id) = super::super::tests::fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    store.headers.get_mut(&id).unwrap().stage = super::super::Stage::ReturnedWithoutBody;
    (store, id)
}
fn words(store: &Store) -> Vec<u32> {
    let mut words = Vec::new();
    store.visit_state_words(|w| words.push(w));
    words
}
fn denied() -> Result<Option<u32>> {
    panic!("animator reference must stay unread behind the clip sentinel")
}

#[test]
fn constructor_inline_sentinel_returns_zero_without_reading_animator_or_mutating() {
    let (store, id) = returned();
    assert_eq!(
        store.get(id).unwrap().current_clip(),
        ClipPointer::ConstructorInline
    );
    let before = words(&store);
    let trace = store.evaluate_guard(id, denied).unwrap();
    assert_eq!(trace.pointer, ClipPointer::ConstructorInline);
    assert_eq!(trace.clip_reference, Some(NO_REFERENCE));
    assert!(!trace.animator_reference_read);
    assert_eq!(trace.outcome, Outcome::Returned(0));
    assert_eq!(words(&store), before);
}

#[test]
fn guard_requires_the_actual_factory_return() {
    let (store, _, id) = super::super::tests::fixture();
    assert!(store.evaluate_guard(id, denied).is_err());
    let (store, _) = returned();
    let missing = NodeId::new(99).unwrap();
    assert!(store.evaluate_guard(missing, denied).is_err());
}

#[test]
fn real_clip_reference_reads_animator_lazily_and_pauses_before_sampling() {
    let (mut store, id) = returned();
    store.headers.get_mut(&id).unwrap().w(0x44, 0x8300_0003);
    let reads = Cell::new(0);
    let trace = store
        .evaluate_guard(id, || {
            reads.set(reads.get() + 1);
            Ok(Some(NO_REFERENCE))
        })
        .unwrap();
    assert_eq!(reads.get(), 1);
    assert_eq!(trace.animator_reference, Some(NO_REFERENCE));
    assert_eq!(trace.outcome, Outcome::Returned(0));
    let trace = store.evaluate_guard(id, || Ok(None)).unwrap();
    assert!(trace.animator_reference_read);
    assert_eq!(trace.outcome, Outcome::Pending(Boundary::AnimatorReference));
    let trace = store.evaluate_guard(id, || Ok(Some(0x8400_0007))).unwrap();
    assert_eq!(trace.outcome, Outcome::Pending(Boundary::Sampling));
    assert!(
        store
            .evaluate_guard(id, || anyhow::bail!("authored provider failure"))
            .is_err()
    );
}

#[test]
fn unknown_reference_or_unmodeled_pointer_stays_pending_without_inferring_max() {
    let (mut store, id) = returned();
    store.headers.get_mut(&id).unwrap().bytes[0x44..0x48].fill(None);
    let trace = store.evaluate_guard(id, denied).unwrap();
    assert_eq!(trace.outcome, Outcome::Pending(Boundary::ClipReference));
    // A record role without populated storage is not a NULL pointer or MAX.
    let (mut store, id) = returned();
    store.headers.get_mut(&id).unwrap().current_clip = ClipPointer::Record(0);
    let trace = store.evaluate_guard(id, denied).unwrap();
    assert_eq!(trace.clip_reference, None);
    assert_eq!(trace.outcome, Outcome::Pending(Boundary::ClipReference));
    for pointer in [ClipPointer::Null, ClipPointer::Unknown] {
        let (mut store, id) = returned();
        store.headers.get_mut(&id).unwrap().current_clip = pointer;
        let trace = store.evaluate_guard(id, denied).unwrap();
        assert_eq!(trace.outcome, Outcome::Pending(Boundary::ClipPointer));
    }
}

#[test]
fn replaced_inline_storage_uses_its_actual_record_instead_of_the_constructor_sentinel() {
    let (mut store, id) = super::super::initialization::tests::fixture();
    store
        .initialize_phase_zero(
            id,
            &mut super::super::initialization::tests::reader(),
            super::super::initialization::tests::inputs(),
        )
        .unwrap();
    let header = store.headers.get_mut(&id).unwrap();
    assert_eq!(header.current_clip(), ClipPointer::Record(0));
    header.stage = super::super::Stage::ReturnedInitialized;
    let reads = Cell::new(0);
    let trace = store
        .evaluate_guard(id, || {
            reads.set(reads.get() + 1);
            Ok(Some(0x8400_0007))
        })
        .unwrap();
    assert_eq!(trace.clip_reference, Some(0x8300_0003));
    assert_eq!(reads.get(), 1);
    assert_eq!(trace.outcome, Outcome::Pending(Boundary::Sampling));
}
