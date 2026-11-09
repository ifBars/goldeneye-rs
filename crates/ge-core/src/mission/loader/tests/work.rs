use super::*;

fn attempt(value: u64) -> AttemptId {
    AttemptId::new(value).unwrap()
}

#[test]
fn native_wrapping_predicate_is_separate_from_checked_host_budget() {
    assert!(!source_busy(0, 0, 0));
    assert!(source_busy(1, 0, 0));
    assert!(source_busy(0, 1, 0));
    assert!(source_busy(0, 0, 2));
    assert!(!source_busy(u32::MAX, 1, 0));
    if let Ok(too_many) = usize::try_from(u64::from(u32::MAX) + 1) {
        assert!(WorkOwner::new(attempt(1), Budget { tickets: too_many }).is_err());
    }
}

#[test]
fn fifo_provider_callbacks_complete_and_retain_real_tickets() {
    let id = attempt(1);
    let mut owner = WorkOwner::new(id, Budget { tickets: 2 }).unwrap();
    let first = owner.enqueue(id, 11).unwrap();
    let second = owner.enqueue(id, 22).unwrap();
    assert_eq!(owner.poll(id).unwrap(), WorkPoll::Busy);
    let mut calls = Vec::new();
    assert_eq!(
        owner
            .execute_next(id, |ticket| {
                calls.push(ticket.key());
                Ok(())
            })
            .unwrap(),
        Some(first)
    );
    assert_eq!(owner.poll(id).unwrap(), WorkPoll::Busy);
    assert_eq!(
        owner
            .execute_next(id, |ticket| {
                calls.push(ticket.key());
                Ok(())
            })
            .unwrap(),
        Some(second)
    );
    assert_eq!(calls, [11, 22]);
    assert_eq!(owner.poll(id).unwrap(), WorkPoll::Complete);
    assert_eq!(
        owner.stats(),
        Stats {
            completed: 2,
            ..Stats::default()
        }
    );
    assert_eq!(owner.tickets().len(), 2);
    assert!(
        owner.enqueue(id, 33).is_err(),
        "retained completed tickets still consume the bounded owner budget"
    );
    assert_eq!(
        owner
            .execute_next(id, |_| panic!("empty FIFO must not invoke a provider"))
            .unwrap(),
        None
    );
}

#[test]
fn callback_failure_is_latched_even_after_other_queued_work_finishes() {
    let id = attempt(3);
    let mut owner = WorkOwner::new(id, Budget { tickets: 2 }).unwrap();
    owner.enqueue(id, 1).unwrap();
    owner.enqueue(id, 2).unwrap();
    assert!(
        owner
            .execute_next(id, |_| anyhow::bail!("authored finalizer failure"))
            .is_err()
    );
    assert_eq!(owner.tickets()[0].state, TaskState::Failed);
    assert!(owner.poll(id).is_err());
    owner
        .execute_next(id, |ticket| {
            assert_eq!(ticket.key(), 2);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        owner.stats(),
        Stats {
            failed: 1,
            completed: 1,
            ..Stats::default()
        }
    );
    assert!(
        owner.poll(id).is_err(),
        "empty pending queue cannot erase the failed callback"
    );
}

#[test]
fn cancellation_retains_completed_history_and_rejects_stale_attempts() {
    let id = attempt(4);
    let other = attempt(5);
    let mut owner = WorkOwner::new(id, Budget { tickets: 2 }).unwrap();
    owner.enqueue(id, 1).unwrap();
    owner.execute_next(id, |_| Ok(())).unwrap();
    owner.enqueue(id, 2).unwrap();
    assert!(owner.enqueue(other, 3).is_err());
    assert!(
        owner
            .execute_next(other, |_| panic!("stale callback"))
            .is_err()
    );
    assert!(owner.cancel(other).is_err());
    assert_eq!(
        owner.stats(),
        Stats {
            queued: 1,
            completed: 1,
            ..Stats::default()
        }
    );
    owner.cancel(id).unwrap();
    assert_eq!(
        owner.stats(),
        Stats {
            cancelled: 1,
            completed: 1,
            ..Stats::default()
        }
    );
    assert!(owner.poll(id).is_err());
    assert!(
        owner
            .execute_next(id, |_| panic!("cancelled callback"))
            .is_err()
    );
}

#[test]
fn a_callback_that_does_not_return_cannot_publish_completed_work() {
    let id = attempt(6);
    let mut owner = WorkOwner::new(id, Budget { tickets: 1 }).unwrap();
    owner.enqueue(id, 1).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.execute_next(id, |_| panic!("authored callback interruption"));
    }));
    assert!(panic.is_err());
    assert_eq!(owner.stats().running, 1);
    assert_eq!(owner.poll(id).unwrap(), WorkPoll::Busy);
    owner.cancel(id).unwrap();
    assert_eq!(owner.stats().cancelled, 1);
    assert!(owner.poll(id).is_err());
}

#[test]
fn zero_budget_empty_owner_is_not_a_successful_enqueued_task() {
    let id = attempt(7);
    let mut owner = WorkOwner::new(id, Budget { tickets: 0 }).unwrap();
    assert!(owner.enqueue(id, 1).is_err());
    assert!(owner.tickets().is_empty());
    assert_eq!(owner.poll(id).unwrap(), WorkPoll::Complete);
    assert_eq!(owner.stats(), Stats::default());
}
