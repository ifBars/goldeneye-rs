use super::*;

#[test]
fn boot_inputs_produce_the_verified_fresh_positive_zero_coordinate() {
    let globals = Globals::from_boot();
    assert_eq!(globals.upper_basis_bits, 0.5f32.to_bits());
    assert_eq!(globals.dispatch_lower_bits, 0);
    assert_eq!(globals.suffix_lower_bits, 0);
    for kind in [LowerKind::Dispatch, LowerKind::Suffix] {
        assert_eq!(coordinate_bits(1f32.to_bits(), 0, globals, kind), 0);
    }
}

#[test]
fn source_bounds_select_upper_lower_or_unchanged_input() {
    for (input, expected) in [(-1f32, 0f32), (0.25, 0.25), (0.5, 0.5), (2.0, 0.5)] {
        assert_eq!(
            coordinate_bits(
                1f32.to_bits(),
                input.to_bits(),
                Globals::from_boot(),
                LowerKind::Dispatch,
            ),
            expected.to_bits()
        );
    }
}

#[test]
fn callers_read_their_distinct_retained_lower_inputs() {
    let globals = Globals {
        dispatch_lower_bits: 0.25f32.to_bits(),
        suffix_lower_bits: 0.375f32.to_bits(),
        ..Globals::from_boot()
    };
    assert_eq!(
        coordinate_bits(1f32.to_bits(), 0, globals, LowerKind::Dispatch),
        0.25f32.to_bits()
    );
    assert_eq!(
        coordinate_bits(1f32.to_bits(), 0, globals, LowerKind::Suffix),
        0.375f32.to_bits()
    );
}

#[test]
fn multiplication_rounds_before_addition_instead_of_fusing() {
    let globals = Globals {
        upper_basis_bits: 0.7f32.to_bits(),
        ..Globals::from_boot()
    };
    assert_eq!(
        coordinate_bits(
            0.1f32.to_bits(),
            2f32.to_bits(),
            globals,
            LowerKind::Dispatch
        ),
        0x3f78_51ec
    );
}

#[test]
fn upper_test_takes_priority_for_inverted_bounds() {
    assert_eq!(
        coordinate_bits(4f32.to_bits(), 0, Globals::from_boot(), LowerKind::Dispatch),
        (-1f32).to_bits()
    );
}

#[test]
fn ordered_zero_selection_preserves_the_selected_bound_sign() {
    let globals = Globals {
        suffix_lower_bits: (-0f32).to_bits(),
        ..Globals::from_boot()
    };
    assert_eq!(
        coordinate_bits(
            1f32.to_bits(),
            (-0f32).to_bits(),
            globals,
            LowerKind::Dispatch
        ),
        0f32.to_bits()
    );
    assert_eq!(
        coordinate_bits(1f32.to_bits(), 0, globals, LowerKind::Suffix),
        (-0f32).to_bits()
    );
    // An equal upper bound is selected before the distinct negative-zero lower.
    assert_eq!(
        coordinate_bits(
            2f32.to_bits(),
            (-0f32).to_bits(),
            globals,
            LowerKind::Suffix
        ),
        0f32.to_bits()
    );
}

#[test]
fn unordered_comparisons_follow_the_source_branches_without_clamp_validation() {
    let nan_bits = 0x7fc1_2345;
    assert_eq!(
        coordinate_bits(
            1f32.to_bits(),
            nan_bits,
            Globals::from_boot(),
            LowerKind::Dispatch
        ),
        nan_bits
    );
    // Infinity times zero produces an unordered upper; the lower still applies.
    let globals = Globals {
        upper_basis_bits: 1f32.to_bits(),
        ..Globals::from_boot()
    };
    assert_eq!(
        coordinate_bits(
            f32::INFINITY.to_bits(),
            0.25f32.to_bits(),
            globals,
            LowerKind::Dispatch
        ),
        0.25f32.to_bits()
    );
    assert_eq!(
        coordinate_bits(
            f32::INFINITY.to_bits(),
            (-1f32).to_bits(),
            globals,
            LowerKind::Dispatch
        ),
        0
    );
}
