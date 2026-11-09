use super::*;
use crate::mission::{world::InstanceId, world_hooks::buffer::Buffer};

#[test]
fn prehook_constructor_does_not_invent_resource_or_animator_values_and_resets_are_sparse() {
    let mut owner = Owner::from_constructor();
    assert_eq!(owner.contexts(), &[ContextSlot::Unknown; 4]);
    assert_eq!(
        owner.animator(),
        AnimatorFields {
            word: None,
            byte: None
        }
    );
    assert_eq!(owner.world_buffers()[0].growth(), 0);
    assert_eq!(owner.world_buffers()[1].growth(), 4);
    *owner.context_mut(1).unwrap() = ContextSlot::Bound(InstanceId::new(17).unwrap());
    *owner.world_buffer_mut(0).unwrap() =
        Buffer::from_owned(4, 31, 2, Some(vec![0x71; 16])).unwrap();
    let pointer = owner.world_buffers()[0].backing().unwrap().as_ptr();
    assert_eq!(owner.reset_contexts(), 4);
    assert_eq!(owner.contexts(), &[ContextSlot::Absent; 4]);
    assert_eq!(
        owner.animator(),
        AnimatorFields {
            word: None,
            byte: None
        }
    );
    assert_eq!(
        owner.world_buffers()[0].backing().unwrap().as_ptr(),
        pointer
    );
    owner.reset_animator();
    assert_eq!(
        owner.animator(),
        AnimatorFields {
            word: Some(0),
            byte: Some(0)
        }
    );
    assert_eq!(owner.world_buffers()[0].count(), 2);
}

#[test]
fn world_cleanup_releases_backings_then_overrides_both_growth_words_to_sixteen() {
    let mut owner = Owner::from_constructor();
    *owner.world_buffer_mut(0).unwrap() = Buffer::from_owned(5, 33, 0, Some(vec![1; 20])).unwrap();
    *owner.world_buffer_mut(1).unwrap() = Buffer::from_owned(3, 77, 2, Some(vec![2; 12])).unwrap();
    let first = owner.run_pre_hooks();
    assert_eq!(
        (
            first.context_writes,
            first.animator_word_writes,
            first.animator_byte_writes
        ),
        (4, 1, 1)
    );
    assert!(
        first
            .world_buffers
            .iter()
            .all(|receipt| receipt.had_backing)
    );
    assert_eq!(first.world_buffers[0].previous_count, 0);
    assert_eq!(first.world_buffers[1].previous_count, 2);
    assert!(owner.world_buffers().iter().all(|buffer| {
        buffer.growth() == 16
            && buffer.capacity() == 0
            && buffer.count() == 0
            && !buffer.has_backing()
    }));
    let second = owner.run_pre_hooks();
    assert!(
        second
            .world_buffers
            .iter()
            .all(|receipt| !receipt.had_backing)
    );
    assert!(
        owner
            .world_buffers()
            .iter()
            .all(|buffer| buffer.growth() == 16)
    );
}
