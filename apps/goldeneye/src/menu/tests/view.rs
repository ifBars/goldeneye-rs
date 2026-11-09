use super::*;
#[test]
fn hits_follow_page_rows_gaps_slider_arrows_and_letterboxing() {
    let mut state = State::new(false);
    assert_eq!(hit([220., 120.], [640, 480], &state), Some(Hit::Row(0)));
    assert_eq!(hit([220., 136.], [640, 480], &state), None);
    assert_eq!(hit([40., 120.], [800, 480], &state), None);
    assert_eq!(hit([300., 120.], [800, 480], &state), Some(Hit::Row(0)));
    state.select_index(5);
    state.input(ge_core::frontend::Input::Confirm);
    assert_eq!(hit([220., 203.], [640, 480], &state), None);
    state.select_index(2);
    state.input(ge_core::frontend::Input::Confirm);
    assert_eq!(
        hit([435., 120.], [640, 480], &state),
        Some(Hit::Decrease(0))
    );
    assert_eq!(
        hit([575., 120.], [640, 480], &state),
        Some(Hit::Increase(0))
    );
    assert_eq!(hit([300., 410.], [640, 480], &state), Some(Hit::Reset));
    assert_eq!(
        hit([480., 410.], [640, 480], &state),
        Some(Hit::Decrease(0))
    );
    assert_eq!(
        hit([575., 410.], [640, 480], &state),
        Some(Hit::Increase(0))
    );
    assert_eq!(hit([0., 0.], [0, 0], &state), None);
    assert_eq!(hit([f64::NAN, 120.], [640, 480], &state), None);
}
#[test]
fn brightness_changes_rgb_without_geometry_or_alpha_mutation() {
    let mut layers = vec![
        Quad {
            texture: 7,
            rect: [1., 2., 3., 4.],
            uv: [0., 0., 1., 1.],
            color: [0.8, 0.4, 0.2, 0.7],
        }
        .into(),
    ];
    brightness(&mut layers, 100);
    brightness(&mut layers, 25);
    let Layer::Quad(quad) = &layers[0] else {
        panic!()
    };
    assert_eq!(quad.color, [0.2, 0.1, 0.05, 0.7]);
    assert_eq!(quad.rect, [1., 2., 3., 4.]);
    assert_eq!(quad.texture, 7);
}
