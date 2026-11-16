use super::super::fixtures::{Skin, bind_returned};
use super::*;
use crate::animation::script_child::pointee::ParameterWrite;

#[test]
fn post_bind_leaves_require_the_actual_returned_bind_and_current_head() {
    let mut registry = Registry::from_constructor(8);
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(23).unwrap();
    let (_, mut pending) = bind_returned(&mut registry, 0x8400_0007, &mut context, &mut arena);
    let head = pending.head().unwrap();
    assert_eq!(
        pending.post_bind_head().unwrap(),
        PostBindHead::ClassFour(head)
    );
    // An arbitrary NodeId is not the current class-four head.
    let foreign = NodeId::new(head.get() + 7).unwrap();
    assert!(
        pending
            .store_post_bind_parameter(foreign, 0x3f00_0000)
            .is_err()
    );
    assert_eq!(pending.factory_header().unwrap().word(0x64), Some(0));
    let ParameterWrite::Stored(receipt) = pending
        .store_post_bind_parameter(head, 0x3f00_0000)
        .unwrap()
    else {
        panic!("constructor parameter role must store")
    };
    assert_eq!(receipt.node, head);
    assert_eq!(pending.bind_return(), Some(1));

    // Before its own bind returns, a temporary exposes no post-bind leaves.
    let (_, _, mut early) = fixture(0x8400_0007);
    early
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    assert!(early.post_bind_head().is_err());
    let mut prepares = Vec::new();
    let mut guards = Vec::new();
    assert!(
        early
            .begin_post_bind_evaluation(
                &mut context,
                &mut arena,
                &mut Skin::resident(),
                &mut prepares,
                &mut guards,
            )
            .is_err()
    );
    assert_eq!(arena.live_reservations(), 0);
}

#[test]
fn post_bind_evaluation_uses_its_own_scope_and_trace_vectors() {
    let mut registry = Registry::from_constructor(8);
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(24).unwrap();
    let (_, pending) = bind_returned(&mut registry, 0x8400_0007, &mut context, &mut arena);
    let first_ordinal = pending.evaluation().unwrap().ordinal();
    let mut prepares = Vec::new();
    let mut guards = Vec::new();
    let mut second = pending
        .begin_post_bind_evaluation(
            &mut context,
            &mut arena,
            &mut Skin::resident(),
            &mut prepares,
            &mut guards,
        )
        .unwrap();
    // The first bind's trace vectors were not borrowed by the second scope.
    assert_eq!(pending.node_guards().len(), 1);
    assert_eq!(pending.skin_prepares().len(), 1);
    pending
        .advance_post_bind_evaluation(
            &mut second,
            &mut context,
            &mut arena,
            &mut Skin::resident(),
            &mut prepares,
            &mut guards,
        )
        .unwrap();
    assert_eq!((prepares.len(), guards.len()), (1, 1));
    assert!(second.ordinal() > first_ordinal);
    assert!(second.released());
    assert_eq!(pending.node_guards().len(), 1);
    assert_eq!(
        pending.evaluation().unwrap().stage(),
        scratch::evaluation::Stage::Returned
    );
}
