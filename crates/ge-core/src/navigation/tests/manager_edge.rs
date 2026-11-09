use super::*;
#[test]
fn flag_transitions_preserve_direction_and_ignore_unrelated_bits() {
    let neighbors = [[Some(1), None, None], [Some(0), None, None]];
    let edge = TraversalEdge {
        from_triangle: 0,
        to_triangle: 1,
        previous_triangle: None,
        slot: 0,
    };
    for (source, destination, allowed) in [
        (0, 2, false),
        (2, 2, true),
        (2, 0, true),
        (0x10, 8, false),
        (0x10, 0, true),
        (0, 8, true),
        (0x80000001, 0x80000001, true),
    ] {
        assert_eq!(
            manager_navigation_edge(&neighbors, &[Some(source), Some(destination)], edge).unwrap(),
            allowed
        );
    }
    assert!(manager_navigation_edge(&neighbors, &[None, None], edge).unwrap());
    assert!(
        !manager_navigation_edge(
            &neighbors,
            &[],
            TraversalEdge {
                previous_triangle: Some(1),
                ..edge
            }
        )
        .unwrap()
    );
    assert!(manager_navigation_edge(&neighbors, &[], edge).is_err());
}
