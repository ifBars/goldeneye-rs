use super::*;

#[test]
fn sixteen_buffers_clear_in_source_order_and_preserve_all_supplied_growth() {
    let mut owner = Buffers::fresh();
    for index in 0..5 {
        *owner.top_buffer_mut(index).unwrap() =
            Buffer::from_owned(1, index as u32 + 20, 1, Some(vec![index as u8])).unwrap();
    }
    for index in 0..11 {
        *owner.record_buffer_mut(index).unwrap() =
            Buffer::from_owned(1, index as u32 + 40, 0, Some(vec![index as u8])).unwrap();
    }
    let reset = owner.reset().unwrap();
    assert!(reset.complete);
    assert_eq!(
        reset
            .cleared
            .iter()
            .map(|r| r.source_offset)
            .collect::<Vec<_>>(),
        [
            0x7c, 0x90, 0xa4, 0xb8, 0xcc, 0xf0, 0x128, 0x160, 0x198, 0x1d0, 0x208, 0x240, 0x278,
            0x2b0, 0x2e8, 0x320
        ]
    );
    assert!(reset.cleared.iter().all(|r| r.stats.had_backing));
    assert_eq!(
        owner
            .top_buffers()
            .iter()
            .map(Buffer::growth)
            .collect::<Vec<_>>(),
        [20, 21, 22, 23, 24]
    );
    assert_eq!(
        owner
            .record_buffers()
            .iter()
            .map(Buffer::growth)
            .collect::<Vec<_>>(),
        (40..51).collect::<Vec<_>>()
    );
    assert_eq!(
        *owner.tail(),
        TailFields {
            flag_1c: Some(0),
            flag_3b0: Some(1),
            word_28: Some(0),
            word_e0: Some(0),
            word_e4: Some(0)
        }
    );
}

#[test]
fn actual_binding_survives_release_after_ordered_flag_writes_and_link_clear() {
    let mut owner = Buffers::fresh();
    let id = BindingId::new(7).unwrap();
    owner.bind(Binding::from_owned(id, 9, 8)).unwrap();
    let reset = owner.reset().unwrap();
    assert_eq!(
        reset.release.unwrap().writes,
        [
            BindingWrite {
                offset: 0x41,
                before: 8,
                after: 0
            },
            BindingWrite {
                offset: 0x40,
                before: 9,
                after: 1
            },
        ]
    );
    assert!(reset.binding_slot_cleared);
    assert_eq!(owner.binding_slot(), BindingSlot::Absent);
    assert_eq!(owner.bindings().len(), 1);
    let binding = owner.binding(id).unwrap();
    assert_eq!((binding.byte_40(), binding.byte_41()), (1, 0));
    owner.clear_receipt();
    assert!(owner.binding(id).is_some());
    owner.bind_existing(id).unwrap();
    assert_eq!(owner.binding_slot(), BindingSlot::Bound(id));
    assert!(BindingId::new(0).is_err());
}

#[test]
fn unknown_binding_keeps_reached_clear_and_flag_writes_but_not_tail_completion() {
    let mut owner = Buffers::fresh();
    *owner.top_buffer_mut(0).unwrap() = Buffer::from_owned(2, 27, 1, Some(vec![1])).unwrap();
    owner.tail_mut().word_e0 = Some(99);
    owner.mark_binding_unknown();
    assert!(owner.reset().is_err());
    assert!(!owner.top_buffers()[0].has_backing());
    assert_eq!(owner.top_buffers()[0].growth(), 27);
    assert_eq!(owner.tail().flag_3b0, Some(1));
    assert_eq!(owner.tail().flag_1c, Some(0));
    assert_eq!(owner.tail().word_e0, Some(99));
    assert_eq!(owner.binding_slot(), BindingSlot::Unknown);
    let reset = owner.last_reset().unwrap();
    assert_eq!(reset.cleared.len(), 16);
    assert!(!reset.complete);
    assert_eq!(reset.release, None);
}

#[test]
fn two_buffer_cleanup_releases_independently_and_preserves_growth() {
    let mut owner = TwoBuffers::fresh();
    *owner.buffer_mut(1).unwrap() = Buffer::from_owned(8, 31, 0, Some(vec![1, 2])).unwrap();
    let reset = owner.reset();
    assert_eq!(reset.cleared[0].source_offset, 0);
    assert!(!reset.cleared[0].stats.had_backing);
    assert_eq!(reset.cleared[1].source_offset, 0x14);
    assert!(reset.cleared[1].stats.had_backing);
    assert_eq!(owner.buffers()[0].growth(), 4);
    assert_eq!(owner.buffers()[1].growth(), 31);
    assert!(owner.buffer_mut(2).is_err());
}

#[test]
fn begin_operation_clears_receipts_only_across_persistent_attempts() {
    let mut owner = Owners::fresh();
    owner.reach_noop(Noop::First);
    owner.reach_noop(Noop::Second);
    owner.reset_leaf();
    owner.reset_buffers().unwrap();
    owner.reset_two_buffers();
    owner.install_callback();
    let id = BindingId::new(9).unwrap();
    owner
        .buffers_mut()
        .bind(Binding::from_owned(id, 3, 4))
        .unwrap();
    *owner.two_buffers_mut().buffer_mut(0).unwrap() =
        Buffer::from_owned(1, 23, 1, Some(vec![5])).unwrap();
    owner.begin_operation();
    assert!(owner.noop_reaches().is_empty());
    assert!(owner.buffers().last_reset().is_none());
    assert_eq!(
        owner.callback(),
        CallbackSlot::Installed(InstalledCallback::WorldTraversal)
    );
    assert_eq!(owner.buffers().binding_slot(), BindingSlot::Bound(id));
    assert!(owner.buffers().binding(id).is_some());
    assert_eq!(
        owner.two_buffers().buffers()[0].backing(),
        Some([5].as_slice())
    );
    owner.reach_noop(Noop::First);
    owner.reach_noop(Noop::Second);
    assert_eq!(owner.noop_reaches().len(), 2);
}
