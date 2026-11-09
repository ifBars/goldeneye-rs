use super::*;

#[test]
fn inactive_dispatch_always_clears_success_without_callbacks_or_losing_other_bits() {
    for pending in [0, 0x100, 0x200] {
        let mut state = NavigationRequestState {
            flags_raw: 0x8000_1c00 | pending,
            binding: Some(7),
        };
        let result = state
            .dispatch(|| panic!("inactive acquire"), |_| panic!("inactive query"))
            .unwrap();
        assert_eq!(result, NavigationDispatch::Inactive);
        assert!(!result.processed());
        assert_eq!(state.flags_raw, 0x8000_1400 | pending);
        assert_eq!(state.binding, Some(7));
    }
}

#[test]
fn successful_acquisition_defers_query_and_next_dispatch_uses_cached_binding() {
    let mut state = NavigationRequestState {
        flags_raw: 0x8000_0b00,
        binding: None,
    };
    let first = state
        .dispatch(|| Ok(Some(4)), |_| panic!("query on acquisition call"))
        .unwrap();
    assert_eq!(first, NavigationDispatch::Acquire { binding: Some(4) });
    assert!(first.processed());
    assert_eq!(state.flags_raw, 0x8000_0300);
    let second = state
        .dispatch(
            || panic!("binding already cached"),
            |i| {
                assert_eq!(i, 4);
                Ok(true)
            },
        )
        .unwrap();
    assert_eq!(second, NavigationDispatch::Query { success: true });
    assert_eq!(state.flags_raw, 0x8000_0f00);
    let third = state.dispatch(|| panic!("cached"), |_| Ok(false)).unwrap();
    assert!(third.processed());
    assert_eq!(state.flags_raw, 0x8000_0700);
}

#[test]
fn acquisition_miss_is_processed_sets_attempt_bit_and_retries_without_querying() {
    let mut state = NavigationRequestState {
        flags_raw: 0x300,
        binding: None,
    };
    assert_eq!(
        state
            .dispatch(|| Ok(None), |_| panic!("no binding"))
            .unwrap(),
        NavigationDispatch::Acquire { binding: None }
    );
    assert_eq!(state.flags_raw, 0x700);
    state
        .dispatch(|| Ok(Some(2)), |_| panic!("deferred"))
        .unwrap();
    assert_eq!(state.flags_raw, 0x700); // Acquisition does not clear a prior 0x400.
    assert_eq!(state.binding, Some(2));
}

#[test]
fn host_callback_errors_are_not_normal_misses_or_success_and_keep_prior_mutations() {
    let mut state = NavigationRequestState {
        flags_raw: 0xb00,
        binding: None,
    };
    assert!(
        state
            .dispatch(|| anyhow::bail!("host failure"), |_| panic!("query"))
            .is_err()
    );
    assert_eq!(state.flags_raw, 0x300);
    state.binding = Some(0);
    state.flags_raw = 0xb00;
    assert!(
        state
            .dispatch(|| panic!("acquire"), |_| anyhow::bail!("budget exhausted"))
            .is_err()
    );
    assert_eq!(state.flags_raw, 0x700);
}
