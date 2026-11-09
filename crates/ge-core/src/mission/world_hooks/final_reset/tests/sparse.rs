use super::*;

#[test]
fn constructor_fields_are_known_only_where_source_writes_and_reset_is_sparse() {
    let mut leaf = Leaf::fresh();
    assert_eq!(leaf.byte(0x01), Some(0xff));
    assert_eq!(leaf.byte(0x03), None);
    assert_eq!(leaf.word(0x14), Some(0x3f4c_cccd));
    assert_eq!(leaf.word(0x18), Some(1f32.to_bits()));
    for field in [
        0x00, 0x02, 0x04, 0x05, 0x06, 0x10, 0x01, 0x03, 0x07, 0x08, 0x50,
    ] {
        leaf.set_byte(field, Some(0x55)).unwrap();
    }
    for field in [
        0x0c, 0x28, 0x54, 0x5c, 0x20, 0x58, 0x60, 0x40, 0x44, 0x48, 0x4c,
    ] {
        leaf.set_word(field, Some(0x1122_3344)).unwrap();
    }
    assert_eq!(
        leaf.reset(),
        LeafReset {
            byte_writes: 6,
            word_writes: 7
        }
    );
    for field in [0x00, 0x02, 0x04, 0x05, 0x06, 0x10] {
        assert_eq!(leaf.byte(field), Some(0));
    }
    for field in [0x01, 0x03, 0x07, 0x08, 0x50] {
        assert_eq!(leaf.byte(field), Some(0x55));
    }
    for field in [0x0c, 0x28, 0x54, 0x5c, 0x20, 0x58, 0x60] {
        assert_eq!(leaf.word(field), Some(0));
    }
    for field in [0x40, 0x44, 0x48, 0x4c] {
        assert_eq!(leaf.word(field), Some(0x1122_3344));
    }
    assert!(leaf.set_word(0x24, Some(0)).is_err());
    assert!(leaf.set_byte(0x0c, Some(0)).is_err());
}

#[test]
fn fresh_scoped_buffers_prove_growth_four_but_do_not_invent_tail_fields() {
    let buffers = Buffers::fresh();
    assert_eq!(
        buffers.top_buffers().len() + buffers.record_buffers().len(),
        16
    );
    for buffer in buffers.top_buffers().iter().chain(buffers.record_buffers()) {
        assert_eq!(
            (buffer.capacity(), buffer.growth(), buffer.count()),
            (0, 4, 0)
        );
        assert!(!buffer.has_backing());
    }
    assert_eq!(buffers.binding_slot(), BindingSlot::Absent);
    assert_eq!(buffers.tail().flag_1c, Some(0));
    assert_eq!(buffers.tail().word_28, Some(0));
    assert_eq!(buffers.tail().flag_3b0, None);
    assert_eq!(buffers.tail().word_e0, None);
    assert_eq!(buffers.tail().word_e4, None);
}

#[test]
fn literal_noops_preserve_supplied_live_state_and_callback_install_does_not_invoke() {
    let mut owners = Owners::fresh();
    owners.leaf_mut().set_byte(0x03, Some(9)).unwrap();
    *owners.buffers_mut().top_buffer_mut(0).unwrap() =
        Buffer::from_owned(1, 19, 1, Some(vec![7])).unwrap();
    assert_eq!(owners.callback(), CallbackSlot::Unknown);
    assert_eq!(
        owners.reach_noop(Noop::First),
        NoopReached { hook: Noop::First }
    );
    owners.reach_noop(Noop::Second);
    assert_eq!(owners.leaf().byte(0x03), Some(9));
    assert_eq!(
        owners.buffers().top_buffers()[0].backing(),
        Some([7].as_slice())
    );
    let installed = owners.install_callback();
    assert_eq!(installed.callback, InstalledCallback::WorldTraversal);
    assert_eq!(
        owners.callback(),
        CallbackSlot::Installed(InstalledCallback::WorldTraversal)
    );
    assert_eq!(owners.leaf().byte(0x03), Some(9));
    assert!(owners.buffers().top_buffers()[0].has_backing());
    assert_eq!(owners.noop_reaches().len(), 2);
}
