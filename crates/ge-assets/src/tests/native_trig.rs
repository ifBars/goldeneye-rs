use super::*;
#[test]
fn reduced_polynomial_tracks_host_geometry_over_authored_physical_angles() {
    for i in -4096..=4096 {
        let angle = i as f32 * std::f32::consts::TAU / 4096.;
        let (s, c) = sin_cos(angle).unwrap();
        assert!(
            (f64::from(s) - f64::from(angle).sin()).abs() < 0.000001,
            "sin {angle}"
        );
        assert!(
            (f64::from(c) - f64::from(angle).cos()).abs() < 0.000001,
            "cos {angle}"
        );
    }
    assert_eq!(sin_cos(0.).unwrap(), (0., 1.));
    assert_eq!(sin_cos(std::f32::consts::FRAC_PI_2).unwrap(), (1., 0.));
    assert!(sin_cos(f32::NAN).is_err());
    assert!(sin_cos(f32::MAX).is_err());
}
