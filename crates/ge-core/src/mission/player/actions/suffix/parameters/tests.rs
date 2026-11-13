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
