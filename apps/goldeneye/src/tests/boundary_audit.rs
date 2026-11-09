use super::*;
#[test]
fn independent_oracle_separates_interior_misses_from_numeric_ambiguity() {
    let (a, b) = ([0., 0., 0.], [2., 0., 0.]);
    assert_eq!(
        intersection_oracle([1., 0., -1.], [1., 0., 1.], a, b),
        Some(true)
    );
    assert_eq!(
        intersection_oracle([3., 0., -1.], [3., 0., 1.], a, b),
        Some(false)
    );
    assert_eq!(intersection_oracle([0., 0., -1.], [0., 0., 0.], a, b), None);
    assert_eq!(intersection_oracle([4., 0., 0.], [5., 0., 0.], a, b), None);
}
