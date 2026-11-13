use super::*;

#[test]
fn primary_blend_has_only_the_source_upper_and_lower_comparisons() {
    for (input, expected) in [
        (-1f32, [-2f32, 0f32]),
        (0f32, [0f32, 0f32]),
        (0.5f32, [1f32, 0f32]),
        (0.75f32, [1f32, 0.5f32]),
        (2f32, [1f32, 3f32]),
    ] {
        assert_eq!(blend_weights(input.to_bits()), expected.map(f32::to_bits));
    }
    assert_eq!(blend_weights((-0f32).to_bits()), [(-0f32).to_bits(), 0]);
    assert!(
        blend_weights(f32::NAN.to_bits())
            .into_iter()
            .all(|word| f32::from_bits(word).is_nan())
    );
}

#[test]
fn parameter_attempt_counts_keep_the_reached_prefix_on_late_failure() {
    let mut suffix = Suffix::new(1);
    suffix.apply_parameter(|| Ok(true)).unwrap();
    suffix.apply_parameter(|| Ok(false)).unwrap();
    assert!(
        suffix
            .apply_parameter(|| anyhow::bail!("missing reached scalar"))
            .is_err()
    );
    assert_eq!(suffix.parameter_visits(), 3);
    assert_eq!(suffix.parameter_writes(), 1);
}

#[test]
fn unavailable_secondary_globals_skip_the_reached_player_scalar_provider() {
    assert_eq!(
        secondary_coordinate(None, || panic!("unreached ec4/9a0 reads")).unwrap(),
        None
    );
}

#[test]
fn secondary_parameters_use_suffix_bound_and_read_player_inputs_once() {
    let globals = Globals {
        dispatch_lower_bits: 0.25f32.to_bits(),
        suffix_lower_bits: 0.375f32.to_bits(),
        ..Globals::from_boot()
    };
    let mut calls = 0;
    let bits = secondary_coordinate(Some(&globals), || {
        calls += 1;
        Ok((1f32.to_bits(), 0))
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(bits, Some(0.375f32.to_bits()));
}

#[test]
fn supplied_secondary_globals_do_not_hide_unknown_reached_player_scalars() {
    let globals = Globals::from_boot();
    let error = secondary_coordinate(Some(&globals), || {
        anyhow::bail!("required ec4 field unavailable")
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "required ec4 field unavailable");
}
