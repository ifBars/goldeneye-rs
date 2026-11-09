use super::super::{OperationBudget, OperationKind, OperationOwner, OperationState};

#[test]
fn type_query_counts_completed_retained_entries_and_ignores_unrelated_types() {
    for state in [
        OperationState::Constructor,
        OperationState::Queued,
        OperationState::Running,
        OperationState::Complete,
        OperationState::Result,
    ] {
        let mut owner = OperationOwner::fresh(OperationBudget { entries: 2 }).unwrap();
        owner
            .enqueue(OperationKind::new(5).unwrap(), OperationState::Complete)
            .unwrap();
        assert!(!owner.contains_kind(OperationKind::TYPE_0B));
        let id = owner.enqueue(OperationKind::TYPE_0B, state).unwrap();
        assert_eq!(id.get(), 1);
        assert!(owner.contains_kind(OperationKind::TYPE_0B));
        assert_eq!(owner.entries()[1].state(), state);
        assert_eq!(owner.entries()[1].kind().raw(), 0x0b);
        assert!(!owner.original_history_verified());
    }
}

#[test]
fn bounded_owner_failure_preserves_order_and_identity() {
    let mut owner = OperationOwner::fresh(OperationBudget { entries: 1 }).unwrap();
    assert_eq!(
        owner
            .enqueue(OperationKind::new(2).unwrap(), OperationState::Queued)
            .unwrap()
            .get(),
        0
    );
    let before = owner.entries().to_vec();
    assert!(
        owner
            .enqueue(OperationKind::TYPE_0B, OperationState::Complete)
            .is_err()
    );
    assert_eq!(owner.entries(), before);
    assert!(!owner.contains_kind(OperationKind::TYPE_0B));
    assert!(OperationKind::new(0x3b).is_err());
    assert_eq!(owner.entries()[0].id().get(), 0);
}
