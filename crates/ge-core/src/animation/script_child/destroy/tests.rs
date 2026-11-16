use super::*;

// Independent synthetic header fixture; no original payload or address.
fn returned() -> (Store, NodeId) {
    let (mut store, _, id) = super::super::tests::fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    store.headers.get_mut(&id).unwrap().stage = super::super::Stage::ReturnedWithoutBody;
    (store, id)
}
fn with_heap(store: &mut Store, id: NodeId, records: usize) {
    let header = store.headers.get_mut(&id).unwrap();
    header.clips = Some(initialization::Storage::Heap(
        (0..records)
            .map(|_| initialization::Record::unwritten())
            .collect(),
    ));
    header.current_clip = ClipPointer::Record(0);
    header.bytes[0x76] = Some(1);
}

#[test]
fn constructor_zero_fields_return_without_clip_free_and_free_the_header_last() {
    let (mut store, id) = returned();
    let mut destructor = store.begin_destructor(id).unwrap();
    let step = store.advance_destructor(&mut destructor).unwrap();
    assert_eq!(
        step,
        Step::Returned(ChildReceipt {
            node: id,
            heap_records: None
        })
    );
    assert_eq!(destructor.phase(), Phase::Returned);
    assert!(store.get(id).is_none());
    assert!(store.begin_destructor(id).is_err());
}

#[test]
fn heap_clips_are_released_before_a_populated_listener_list_pauses() {
    let (mut store, id) = returned();
    with_heap(&mut store, id, 3);
    store.headers.get_mut(&id).unwrap().w(0x78, 9);
    let mut destructor = store.begin_destructor(id).unwrap();
    assert_eq!(
        store.advance_destructor(&mut destructor).unwrap(),
        Step::Blocked(ChildDependency::ChildReferenceList)
    );
    // The reached clip free is retained; the header itself is not yet freed.
    assert_eq!(destructor.phase(), Phase::ReferenceList);
    assert_eq!(destructor.heap_records(), Some(3));
    let header = store.get(id).unwrap();
    assert!(header.clip_storage().is_none());
    // Resuming re-reads only the listener list, never frees clips twice.
    assert_eq!(
        store.advance_destructor(&mut destructor).unwrap(),
        Step::Blocked(ChildDependency::ChildReferenceList)
    );
    fixtures::store_header_word(&mut store, id, 0x78, Some(0));
    assert_eq!(
        store.advance_destructor(&mut destructor).unwrap(),
        Step::Returned(ChildReceipt {
            node: id,
            heap_records: Some(3)
        })
    );
    assert!(store.get(id).is_none());
}

#[test]
fn linked_base_child_pauses_first_and_nothing_later_runs() {
    for (bits, expected) in [
        (Some(9), ChildDependency::LinkedBaseChild),
        (None, ChildDependency::ChildState),
    ] {
        let (mut store, id) = returned();
        with_heap(&mut store, id, 2);
        fixtures::store_header_word(&mut store, id, 0x8c, bits);
        let mut destructor = store.begin_destructor(id).unwrap();
        assert_eq!(
            store.advance_destructor(&mut destructor).unwrap(),
            Step::Blocked(expected)
        );
        assert_eq!(destructor.phase(), Phase::LinkedChild);
        assert_eq!(destructor.heap_records(), None);
        assert!(store.get(id).unwrap().clip_storage().is_some());
    }
}

#[test]
fn unmodeled_heap_metadata_is_never_freed_and_null_pointer_skips_the_free() {
    // +0x76 nonzero without actual heap storage: typed pause, no fake free.
    let (mut store, id) = returned();
    store.headers.get_mut(&id).unwrap().bytes[0x76] = Some(1);
    let mut destructor = store.begin_destructor(id).unwrap();
    assert_eq!(
        store.advance_destructor(&mut destructor).unwrap(),
        Step::Blocked(ChildDependency::ChildState)
    );
    assert_eq!(destructor.phase(), Phase::Clips);
    // Unknown +0x76 likewise pauses at the clip phase.
    store.headers.get_mut(&id).unwrap().bytes[0x76] = None;
    assert_eq!(
        store.advance_destructor(&mut destructor).unwrap(),
        Step::Blocked(ChildDependency::ChildState)
    );
    // A known NULL +0x5c skips the free; storage is not reported freed.
    let (mut store, id) = returned();
    with_heap(&mut store, id, 2);
    store.headers.get_mut(&id).unwrap().current_clip = ClipPointer::Null;
    let mut destructor = store.begin_destructor(id).unwrap();
    assert!(matches!(
        store.advance_destructor(&mut destructor).unwrap(),
        Step::Returned(ChildReceipt {
            heap_records: None,
            ..
        })
    ));
}
