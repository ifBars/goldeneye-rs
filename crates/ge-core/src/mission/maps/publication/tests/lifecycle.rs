use super::*;

#[test]
fn initial_base_publication_owns_zone_record_bounds_and_real_refresh() -> Result<()> {
    let mut source = Fixture::new(vec![0]);
    let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    assert!(!state.zones()[0].initialized());
    assert!(state.placement_bounds()[0].is_none());
    let receipt = run(&mut state, &sections, &mut source, Budget::default())?;
    assert_eq!(
        (
            receipt.initialized,
            receipt.bindings_allocated,
            receipt.placement_updates
        ),
        (1, 1, 1)
    );
    assert_eq!(
        (
            receipt.hierarchy_refreshes,
            receipt.hierarchy_nodes,
            receipt.hierarchy_entries
        ),
        (1, 1, 1)
    );
    assert!(state.zones()[0].initialized());
    assert_eq!(state.zones()[0].bindings()[0].words(), &[0, 0]);
    assert_eq!(state.placement_bounds()[0].unwrap().maximum, unit().maximum);
    assert_eq!(state.aggregate().unwrap().minimum[..3], [0.; 3]);
    assert_eq!(receipt.branch_bounds_written, 0);
    assert!(!state.failed());
    Ok(())
}

#[test]
fn repeated_hook_preserves_zone_6a_bindings_and_bounds_without_reallocation() -> Result<()> {
    let mut source = Fixture::new(vec![0]);
    let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    run(&mut state, &sections, &mut source, Budget::default())?;
    let pointer = state.zones()[0].bindings().as_ptr();
    let bounds = state.placement_bounds()[0].unwrap();
    let second = run(&mut state, &sections, &mut source, Budget::default())?;
    assert_eq!(
        (
            second.initialized,
            second.retained,
            second.bindings_allocated,
            second.placement_updates
        ),
        (0, 1, 0, 0)
    );
    assert_eq!(second.hierarchy_refreshes, 0);
    assert_eq!((source.matrices_read, source.trees_read), (1, 1));
    assert_eq!(state.zones()[0].bindings().as_ptr(), pointer);
    assert_eq!(state.placement_bounds()[0].unwrap().minimum, bounds.minimum);
    Ok(())
}

#[test]
fn cached_nonbase_geometry_is_not_addressed_or_published() -> Result<()> {
    let mut source = Fixture::new(vec![1, 2]);
    source.root.dynamic = 1; // poison: lazy unrequested roots must remain unread
    source.fail_tree = true;
    let sections = Owner::from_fresh_base(source.key.wrapper, 3, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 2, 1, Budget::default())?;
    let receipt = run(&mut state, &sections, &mut source, Budget::default())?;
    assert_eq!(
        (
            receipt.section_queries,
            receipt.unrequested,
            receipt.initialized
        ),
        (2, 2, 0)
    );
    assert_eq!(
        (source.roots_read, source.matrices_read, source.trees_read),
        (0, 0, 0)
    );
    assert!(
        state
            .zones()
            .iter()
            .all(|z| !z.initialized() && z.bindings().is_empty())
    );
    assert!(state.placement_bounds()[0].is_none());
    Ok(())
}

#[test]
fn any_new_zone_refreshes_all_base_block_trees_in_source_order() -> Result<()> {
    let mut source = Fixture::new(vec![0, 1]);
    let sections = Owner::from_fresh_base(source.key.wrapper, 2, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 2, 1, Budget::default())?;
    let receipt = run(&mut state, &sections, &mut source, Budget::default())?;
    assert_eq!(source.roots_read, 1);
    assert_eq!(
        (
            source.trees_read,
            receipt.hierarchy_refreshes,
            receipt.hierarchy_entries
        ),
        (2, 2, 2)
    );
    assert!(!state.zones()[1].initialized());
    Ok(())
}

#[test]
fn actual_bounds_kernel_uses_only_masked_components_and_optional_special() -> Result<()> {
    let mut source = Fixture::new(vec![0]);
    source.entity.as_mut().unwrap().component_mask = 0x43;
    let sections = Owner::from_fresh_base(source.key.wrapper, 1, 16)?;
    let mut state = State::from_fresh_constructor(source.key, 1, 1, Budget::default())?;
    run(&mut state, &sections, &mut source, Budget::default())?;
    assert_eq!(source.components, [0, 1]);
    assert_eq!(source.special_reads, 1);
    assert_eq!(state.placement_bounds()[0].unwrap().minimum[..3], [-3.; 3]);
    assert_eq!(state.placement_bounds()[0].unwrap().maximum[..3], [3.; 3]);
    Ok(())
}
