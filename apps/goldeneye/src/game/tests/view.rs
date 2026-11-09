use super::hit_back;

#[test]
fn back_target_matches_menu_footer_and_letterboxed_coordinates() {
    assert!(hit_back([106., 396.], [640, 480]));
    assert!(hit_back([229.9, 423.9], [640, 480]));
    assert!(!hit_back([230., 410.], [640, 480]));
    assert!(!hit_back([120., 424.], [640, 480]));
    assert!(!hit_back([105.9, 410.], [640, 480]));
    assert!(!hit_back([120., 395.9], [640, 480]));
    assert!(hit_back([200., 410.], [800, 480]));
    assert!(!hit_back([120., 410.], [800, 480]));
    assert!(hit_back([240., 820.], [1280, 960]));
    assert!(hit_back([120., 470.], [640, 600]));
}

#[test]
fn minimized_invalid_and_nonfooter_input_never_requests_cancellation() {
    for size in [[0, 0], [0, 480], [640, 0]] {
        assert!(!hit_back([120., 410.], size));
    }
    for position in [
        [f64::NAN, 410.],
        [120., f64::INFINITY],
        [f64::NEG_INFINITY, 410.],
        [120., 200.],
        [480., 410.],
        [-1., -1.],
    ] {
        assert!(!hit_back(position, [640, 480]));
    }
}
