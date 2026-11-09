use super::*;
use ge_core::mission::world_hooks::{
    buffer::Buffer,
    final_reset::{Binding, BindingId, BindingSlot},
};

#[test]
fn actual_frame_clock_and_constructor_owner_survive_cancellation_and_reentry() {
    let shared = Shared::from_constructor();
    {
        let state = shared.0.state.lock().unwrap();
        assert_eq!(state.reset.total_reads(), 16);
        assert_eq!(state.reset.completed_resets(), 1);
        assert_eq!(state.clock.bits(), 0);
    }
    shared.advance(Duration::from_millis(1250)).unwrap();
    shared.advance(Duration::from_millis(750)).unwrap();
    let first = shared.begin(1).unwrap().run().unwrap();
    assert_eq!(first.clock_seconds_bits, 2.0f32.to_bits());
    assert_eq!(
        (
            first.clock_frame_updates,
            first.clock_reads,
            first.constructor_clock_reads
        ),
        (2, 16, 16)
    );
    assert_eq!(first.completed_clock_resets, 2);
    shared.invalidate();
    shared.advance(Duration::from_millis(500)).unwrap();
    let second = shared.begin(2).unwrap().run().unwrap();
    assert_eq!(second.clock_seconds_bits, 2.5f32.to_bits());
    assert_eq!(
        (
            second.completed_clock_resets,
            second.constructor_clock_reads
        ),
        (3, 16)
    );
    assert_eq!(second.literal_noops_reached, 2);
    assert_eq!(first.final_state_sha256, second.final_state_sha256);
    assert_ne!(first.clock_state_sha256, second.clock_state_sha256);
    assert_ne!(first.state_sha256, second.state_sha256);
    assert!(!second.world_ready_emitted);
    assert!(!second.native_timing_verified);
    assert_eq!(shared.0.state.lock().unwrap().reset.total_reads(), 48);
}

#[test]
fn superseded_attempt_cannot_overwrite_a_newer_completed_hook() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    let evidence = current.run().unwrap();
    assert!(old.run().is_err());
    let state = shared.0.state.lock().unwrap();
    assert_eq!(
        state.reset.completed_resets(),
        evidence.completed_clock_resets
    );
    assert_eq!(state.reset.total_reads(), 32);
}

#[test]
fn waiting_old_worker_rechecks_its_lease_after_obtaining_the_manager_lock() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    let guard = shared.0.state.lock().unwrap();
    let (started, observed) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        old.run()
    });
    observed.recv().unwrap();
    // Activation/cancellation use the atomic identity and need no manager lock.
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    drop(guard);
    assert!(worker.join().unwrap().is_err());
    assert_eq!(current.run().unwrap().completed_clock_resets, 2);
}

#[test]
fn reset_frees_real_backings_but_preserves_untouched_buffers_and_growth() {
    let shared = Shared::from_constructor();
    let (released, retained) = {
        let mut state = shared.0.state.lock().unwrap();
        let first = vec![7; 8];
        let top = vec![9; 5];
        let kept = vec![11; 16];
        let sizes = (first.capacity() + top.capacity(), kept.capacity());
        *state.reset.buffer_mut(0).unwrap() = Buffer::from_owned(3, 32, 1, Some(first)).unwrap();
        *state.reset.buffer_mut(3).unwrap() = Buffer::from_owned(4, 77, 1, Some(kept)).unwrap();
        *state.final_owners.buffers_mut().top_buffer_mut(0).unwrap() =
            Buffer::from_owned(2, 64, 0, Some(top)).unwrap();
        sizes
    };
    let receipt = shared.begin(1).unwrap().run().unwrap();
    assert!(receipt.operation_complete);
    assert_eq!(
        (
            receipt.scoped_buffers,
            receipt.buffers_cleared,
            receipt.buffers_preserved
        ),
        (24, 22, 2)
    );
    assert_eq!(
        (
            receipt.backings_released,
            receipt.bytes_released,
            receipt.retained_backing_bytes
        ),
        (2, released, retained)
    );
    let state = shared.0.state.lock().unwrap();
    assert!(!state.reset.buffers()[0].has_backing());
    assert_eq!(state.reset.buffers()[0].growth(), 32);
    assert_eq!(state.final_owners.buffers().top_buffers()[0].growth(), 64);
    assert_eq!(
        state.reset.buffers()[3].backing(),
        Some([11u8; 16].as_slice())
    );
    assert_eq!(state.reset.buffers()[3].growth(), 77);
}

#[test]
fn optional_binding_is_released_by_flags_and_stays_owned_after_its_link_clears() {
    let shared = Shared::from_constructor();
    let id = BindingId::new(29).unwrap();
    shared
        .0
        .state
        .lock()
        .unwrap()
        .final_owners
        .buffers_mut()
        .bind(Binding::from_owned(id, 0x7a, 0xbc))
        .unwrap();
    let receipt = shared.begin(1).unwrap().run().unwrap();
    assert!(receipt.binding_released);
    {
        let state = shared.0.state.lock().unwrap();
        let buffers = state.final_owners.buffers();
        assert_eq!(buffers.binding_slot(), BindingSlot::Absent);
        let binding = buffers.binding(id).unwrap();
        assert_eq!((binding.byte_40(), binding.byte_41()), (1, 0));
    }
    shared.invalidate();
    let second = shared.begin(2).unwrap().run().unwrap();
    assert!(!second.binding_released);
    assert_eq!(receipt.final_state_sha256, second.final_state_sha256);
    assert!(
        shared
            .0
            .state
            .lock()
            .unwrap()
            .final_owners
            .buffers()
            .binding(id)
            .is_some()
    );
}
