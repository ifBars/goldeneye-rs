use super::*;

#[test]
fn wrong_owner_and_existing_world_dependencies_are_not_fresh_skips() -> Result<()> {
    let mut source = Fixture::new(vec![0]);
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    let wrong = Owner::from_fresh_base(WrapperId::new(2)?, 1, 16)?;
    assert!(run(&mut state, &wrong, &mut source, Budget::default()).is_err());
    let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
    source.scope.non_null_components = 1;
    assert!(run(&mut state, &sections, &mut source, Budget::default()).is_err());
    source.scope.non_null_components = 0;
    source.scope.live_contexts = 1;
    assert!(run(&mut state, &sections, &mut source, Budget::default()).is_err());
    assert!(!state.zones()[0].initialized());
    assert_eq!(source.roots_read, 0);
    Ok(())
}

#[test]
fn failed_provider_retains_reached_stores_and_cannot_turn_into_successful_retry() -> Result<()> {
    let mut source = Fixture::new(vec![0]);
    source.fail_tree = true;
    let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    assert!(run(&mut state, &sections, &mut source, Budget::default()).is_err());
    assert!(state.failed() && state.zones()[0].initialized());
    assert!(state.placement_bounds()[0].is_some());
    assert_eq!(state.receipt().placement_updates, 1);
    source.fail_tree = false;
    assert!(run(&mut state, &sections, &mut source, Budget::default()).is_err());
    assert_eq!(source.trees_read, 1);
    Ok(())
}

#[test]
fn dynamic_roots_and_bad_links_retain_only_the_actual_prefix() -> Result<()> {
    for dynamic in [false, true] {
        let mut source = Fixture::new(vec![0]);
        if dynamic {
            source.root.dynamic = 1;
        } else {
            source.links[0] = vec![2];
        }
        let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
        let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
        assert!(run(&mut state, &sections, &mut source, Budget::default()).is_err());
        assert!(state.failed() && state.zones()[0].initialized());
        assert_eq!(state.zones()[0].bindings().len(), 1);
        assert_eq!(source.trees_read, 0);
    }
    Ok(())
}

#[test]
fn work_budgets_are_explicit_and_missing_entities_do_not_read_matrices() -> Result<()> {
    let mut source = Fixture::new(vec![0]);
    let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    assert!(
        run(
            &mut state,
            &sections,
            &mut source,
            Budget {
                binding_records: 0,
                ..Budget::default()
            }
        )
        .is_err()
    );
    assert_eq!(source.matrices_read, 0);
    let mut source = Fixture::new(vec![0]);
    source.entity = None;
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    run(&mut state, &sections, &mut source, Budget::default())?;
    assert_eq!(source.matrices_read, 0);
    assert_eq!(
        state.placement_bounds()[0].unwrap().minimum,
        Bounds::MISSING_ENTITY.minimum
    );
    Ok(())
}
