use super::*;

fn request(file: u32) -> Request {
    Request {
        file,
        map_reference: 0x8500_0000,
    }
}

fn preferences() -> Preferences {
    Preferences {
        prefixes_98c: vec![0x018b_0000, 0x01a1_0000, 0x01a3_0000],
        prefixes_9ac: vec![0x0181_0000, 0x01a0_0000, 0x01a2_0000],
        prefixes_9cc: vec![0x0183_0000, 0x01a4_0000],
        filter_9ec: vec![0x1f00_0088],
        filter_a2c: vec![
            0x1f00_0006,
            0x1f00_0034,
            0x1f00_0096,
            0x1f00_00ac,
            0x1f00_0054,
            0x1f00_009a,
        ],
    }
}

fn budget() -> Budget {
    Budget {
        rows: 16,
        preference_words: 64,
    }
}

#[test]
fn singleton_sets_both_row_bytes_without_requiring_a_preference_match() {
    let prefs = Preferences {
        prefixes_98c: vec![],
        prefixes_9ac: vec![],
        prefixes_9cc: vec![],
        filter_9ec: vec![],
        filter_a2c: vec![],
    };
    let input = [request(0)];
    let plan = RegistrationPlan::build(&input, prefs, budget()).unwrap();
    assert_eq!(plan.incoming(), &input);
    assert_eq!(plan.rows()[0].request(), input[0]);
    assert_eq!(plan.rows()[0].flags_raw(), [0xff, 1, 1]);
    assert_eq!(plan.row_filter(0).unwrap(), None);
}

#[test]
fn first_preferred_row_moves_to_index_one_with_other_rows_stable() {
    let input = [
        request(0x0181_0010),
        request(0x0183_0020),
        request(0x018b_0030),
        request(0x0999_0040),
    ];
    let plan = RegistrationPlan::build(&input, preferences(), budget()).unwrap();
    assert_eq!(plan.incoming(), &input);
    assert_eq!(
        plan.rows()
            .iter()
            .map(Row::source_index)
            .collect::<Vec<_>>(),
        [0, 2, 1, 3]
    );
    assert_eq!(plan.rows()[0].flags_raw(), [0xff, 0, 1]);
    assert_eq!(plan.rows()[1].flags_raw(), [0xff, 1, 1]);
    assert_eq!(plan.rows()[2].flags_raw(), [0xff, 0, 1]);
    assert_eq!(plan.row_filter(0).unwrap(), None);
    assert_eq!(plan.rows()[1].filter(), Some(FilterSet::At9ec));
    assert_eq!(plan.row_filter(1).unwrap(), Some(&[0x1f00_0088][..]));
    assert_eq!(plan.rows()[2].filter(), Some(FilterSet::AtA2c));
    assert_eq!(
        plan.row_filter(2).unwrap(),
        Some(plan.preferences().filter_a2c.as_slice())
    );
    assert_eq!(plan.row_filter(3).unwrap(), plan.row_filter(2).unwrap());
}

#[test]
fn moving_index_zero_and_rescanning_can_flag_a_different_preferred_row() {
    let input = [
        request(0x018b_0001),
        request(0x01a1_0002),
        request(0x0181_0003),
    ];
    let plan = RegistrationPlan::build(&input, preferences(), budget()).unwrap();
    assert_eq!(
        plan.rows()
            .iter()
            .map(Row::source_index)
            .collect::<Vec<_>>(),
        [1, 0, 2]
    );
    assert_eq!(plan.rows()[0].flags_raw(), [0xff, 1, 1]);
    assert_eq!(plan.rows()[0].filter(), Some(FilterSet::At9ec));
    assert_eq!(plan.rows()[1].flags_raw(), [0xff, 0, 1]);
    assert_eq!(plan.rows()[1].filter(), Some(FilterSet::AtA2c));
    assert_eq!(plan.rows()[2].filter(), None);
}

#[test]
fn preference_search_is_rows_outer_and_ignores_preference_low_words() {
    let input = [
        request(0x0181_0001),
        request(0x01a1_0002),
        request(0x018b_0003),
    ];
    let mut prefs = preferences();
    prefs.prefixes_98c = vec![0x018b_ffff, 0x01a1_1234];
    let plan = RegistrationPlan::build(&input, prefs, budget()).unwrap();
    assert_eq!(
        plan.rows()
            .iter()
            .map(Row::source_index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(plan.rows()[1].flags_raw(), [0xff, 1, 1]);
    // A file with low-half zero does not match, even though the high-half does.
    assert!(
        RegistrationPlan::build(
            &[request(0x018b_0000), request(0x0181_0001)],
            preferences(),
            budget()
        )
        .is_err()
    );
}

#[test]
fn absent_exempt_group_leaves_preferred_row_unfiltered_and_other_rows_filtered() {
    let input = [
        request(0x9999_0001),
        request(0x018b_0002),
        request(0x0183_0003),
    ];
    let plan = RegistrationPlan::build(&input, preferences(), budget()).unwrap();
    assert_eq!(plan.rows()[1].filter(), None);
    assert_eq!(plan.rows()[0].filter(), Some(FilterSet::AtA2c));
    assert_eq!(plan.rows()[2].filter(), Some(FilterSet::AtA2c));
    assert_eq!(plan.rows()[1].flags_raw(), [0xff, 1, 1]);
}

#[test]
fn overlapping_groups_assign_the_selected_filter_instead_of_clearing_it() {
    let mut prefs = preferences();
    prefs.prefixes_9ac = vec![0x018b_0000];
    let plan = RegistrationPlan::build(
        &[request(0x9999_0001), request(0x018b_0002)],
        prefs,
        budget(),
    )
    .unwrap();
    assert_eq!(plan.rows()[1].filter(), Some(FilterSet::At9ec));
    assert_eq!(plan.rows()[0].filter(), Some(FilterSet::AtA2c));
}

#[test]
fn plans_preserve_duplicate_input_pairs_map_references_and_empty_owned_filters() {
    let input = [
        Request {
            file: 0x0181_0001,
            map_reference: 0x1234,
        },
        request(0x018b_0002),
        request(0x0181_0001),
    ];
    let mut prefs = preferences();
    prefs.filter_a2c.clear();
    let plan = RegistrationPlan::build(&input, prefs.clone(), budget()).unwrap();
    assert_eq!(plan.incoming(), &input);
    assert_eq!(plan.rows()[0].request().map_reference, 0x1234);
    assert_eq!(plan.rows().len(), 3);
    assert_eq!(plan.row_filter(0).unwrap(), None);
    assert_eq!(plan.row_filter(2).unwrap(), Some(&[][..]));
    prefs.prefixes_9cc = vec![0, u32::MAX, 0x018b_0000];
    let other = RegistrationPlan::build(&input, prefs, budget()).unwrap();
    assert_eq!(plan.rows(), other.rows());
}

#[test]
fn empty_unsupported_or_over_budget_inputs_return_errors_without_mutating_callers() {
    let input = [request(0x0181_0001), request(0x0183_0002)];
    let saved = input;
    assert!(RegistrationPlan::build(&input, preferences(), budget()).is_err());
    assert_eq!(input, saved);
    assert!(RegistrationPlan::build(&[], preferences(), budget()).is_err());
    assert!(
        RegistrationPlan::build(
            &[request(1)],
            preferences(),
            Budget {
                rows: 0,
                ..budget()
            }
        )
        .is_err()
    );
    assert!(
        RegistrationPlan::build(
            &[request(1)],
            preferences(),
            Budget {
                preference_words: 0,
                ..budget()
            }
        )
        .is_err()
    );
    let plan = RegistrationPlan::build(&[request(1)], preferences(), budget()).unwrap();
    assert!(plan.row_filter(1).is_err());
}
