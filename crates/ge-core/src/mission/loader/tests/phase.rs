use super::super::work::Budget;
use super::*;

fn attempt(value: u64) -> AttemptId {
    AttemptId::new(value).unwrap()
}
fn work_owner(id: AttemptId) -> WorkOwner {
    WorkOwner::new(id, Budget { tickets: 4 }).unwrap()
}

#[test]
fn wait_shortcircuits_busy_work_and_returns_without_recursive_dispatch() {
    let id = attempt(1);
    let mut phase = PhaseState::new(id, 7).unwrap();
    let mut work = work_owner(id);
    work.enqueue(id, 1).unwrap();
    phase.queue(id, 8).unwrap();
    assert_eq!(
        phase
            .poll(id, &work, || panic!(
                "work busy must short circuit resource polling"
            ))
            .unwrap(),
        WaitPoll::WorkBusy
    );
    assert_eq!((phase.current(), phase.pending()), (28, Some(8)));
    work.execute_next(id, |_| Ok(())).unwrap();
    assert_eq!(
        phase.poll(id, &work, || Ok(ResourcePoll::Busy)).unwrap(),
        WaitPoll::ResourceBusy
    );
    assert_eq!((phase.current(), phase.pending()), (28, Some(8)));
    assert_eq!(
        phase
            .poll(id, &work, || Ok(ResourcePoll::Complete))
            .unwrap(),
        WaitPoll::Promoted { phase: 8 }
    );
    assert_eq!((phase.current(), phase.pending()), (8, None));
    // No phase8 write occurred merely because Wait28 promoted its successor.
    let byte = Phase8Byte::from_snapshot(Some(99));
    assert_eq!(byte.value(), Some(99));
    assert!(phase.poll(id, &work, || panic!("not waiting")).is_err());
}

#[test]
fn error_cancel_and_stale_identity_preserve_the_queued_phase() {
    let id = attempt(2);
    let other = attempt(3);
    let mut phase = PhaseState::new(id, 17).unwrap();
    phase.queue(id, 18).unwrap();
    let work = work_owner(id);
    assert!(
        phase
            .poll(id, &work, || anyhow::bail!(
                "authored bank provider failure"
            ))
            .is_err()
    );
    assert!(
        phase
            .poll(other, &work, || panic!("stale phase identity"))
            .is_err()
    );
    assert!(
        phase
            .poll(id, &work_owner(other), || panic!("foreign work owner"))
            .is_err()
    );
    assert!(phase.cancel(other).is_err());
    assert_eq!((phase.current(), phase.pending()), (28, Some(18)));
    phase.cancel(id).unwrap();
    assert!(phase.poll(id, &work, || panic!("cancelled phase")).is_err());
    assert_eq!((phase.current(), phase.pending()), (28, Some(18)));
}

#[test]
fn a_failed_work_callback_does_not_become_ready_when_its_fifo_is_empty() {
    let id = attempt(4);
    let mut work = work_owner(id);
    work.enqueue(id, 1).unwrap();
    assert!(
        work.execute_next(id, |_| anyhow::bail!("finalizer failed after read"))
            .is_err()
    );
    let mut phase = PhaseState::new(id, 18).unwrap();
    phase.queue(id, 19).unwrap();
    assert!(
        phase
            .poll(id, &work, || panic!("failed work must not poll resources"))
            .is_err()
    );
    assert_eq!((phase.current(), phase.pending()), (28, Some(19)));
}

#[test]
fn bounded_source_edges_reject_invalid_states_and_early_catalog_jumps() {
    let id = attempt(5);
    assert!(AttemptId::new(0).is_err());
    for initial in [0, 9, 20, 25, 28, 255] {
        assert!(PhaseState::new(id, initial).is_err());
    }
    let mut phase = PhaseState::new(id, 7).unwrap();
    for next in [9, 18, 28, 255] {
        assert!(phase.queue(id, next).is_err());
        assert_eq!((phase.current(), phase.pending()), (7, None));
    }
    phase.queue(id, 8).unwrap();
    assert!(phase.queue(id, 8).is_err());
    let mut residency = PhaseState::new(id, 6).unwrap();
    residency.queue(id, 6).unwrap();
    assert_eq!(
        residency
            .poll(id, &work_owner(id), || Ok(ResourcePoll::Complete))
            .unwrap(),
        WaitPoll::Promoted { phase: 6 }
    );
}

#[test]
fn phase8_performs_the_owned_byte_write_before_queueing_ten() {
    let id = attempt(6);
    for initial in [None, Some(0), Some(10), Some(254), Some(255)] {
        let mut byte = Phase8Byte::from_snapshot(initial);
        let mut phase = PhaseState::new(id, 8).unwrap();
        phase.apply_case8(id, &mut byte).unwrap();
        assert_eq!((byte.value(), byte.resets()), (Some(10), 1));
        assert_eq!((phase.current(), phase.pending()), (28, Some(10)));
        assert!(phase.apply_case8(id, &mut byte).is_err());
        let mut repeated = PhaseState::new(id, 8).unwrap();
        repeated.apply_case8(id, &mut byte).unwrap();
        assert_eq!((byte.value(), byte.resets()), (Some(10), 2));
    }
}

#[test]
fn phase16_writes_separate_scalar_owner_in_source_order_without_float_arithmetic() {
    let id = attempt(7);
    let mut scalars = LoaderScalars::from_snapshot(Some(0x8000_0000), Some(0x7fc0_1234));
    let mut phase = PhaseState::new(id, 16).unwrap();
    let writes = phase.apply_case16(id, &mut scalars).unwrap();
    assert_eq!(
        (writes.previous_c4, writes.previous_c0),
        (Some(0x7fc0_1234), Some(0x8000_0000))
    );
    assert_eq!(writes.ordered(), [(0xc4, 0x44bb_8000), (0xc0, 0x3a83_126f)]);
    assert_eq!(
        (scalars.c4_bits(), scalars.c0_bits()),
        (Some(0x44bb_8000), Some(0x3a83_126f))
    );
    assert_eq!((phase.current(), phase.pending()), (28, Some(17)));
    let mut unbound = LoaderScalars::unbound();
    let mut stale = PhaseState::new(id, 16).unwrap();
    assert!(stale.apply_case16(attempt(8), &mut unbound).is_err());
    assert_eq!(unbound, LoaderScalars::unbound());
}
