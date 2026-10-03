use super::*;

#[test]
fn polygon_area_and_normal_follow_size_winding_and_translation() {
    for offset in [DVec3::ZERO, DVec3::splat(1.0e12)] {
        let positions =
            [DVec3::ZERO, 2.0 * DVec3::X, 3.0 * DVec3::Y].map(|position| position + offset);
        assert_eq!(polygon_area_vector(positions), 3.0 * DVec3::Z);
        assert_eq!(polygon_normal(positions), Some(DVec3::Z));
        assert_eq!(
            polygon_area_vector(positions.into_iter().rev()),
            -3.0 * DVec3::Z
        );
        assert_eq!(polygon_normal(positions.into_iter().rev()), Some(-DVec3::Z));
    }
}

#[test]
fn polygon_normal_is_absent_without_usable_area() {
    let positions = [DVec3::ZERO, DVec3::X, 2.0 * DVec3::X];
    for count in 0..=positions.len() {
        assert_eq!(
            polygon_area_vector(positions[..count].iter().copied()),
            DVec3::ZERO
        );
        assert_eq!(polygon_normal(positions[..count].iter().copied()), None);
    }
    assert_eq!(polygon_normal([DVec3::ZERO; 3]), None);
}

#[test]
fn corner_angles_handle_convex_concave_and_collapsed_corners() {
    let right_angle = std::f64::consts::FRAC_PI_2;
    assert_eq!(corner_angle(DVec3::Y, DVec3::X, DVec3::Z), right_angle);
    assert_eq!(
        corner_angle(DVec3::X, DVec3::Y, DVec3::Z),
        3.0 * right_angle
    );
    assert_eq!(corner_angle(DVec3::X, DVec3::Y, -DVec3::Z), right_angle);
    assert_eq!(corner_angle(DVec3::ZERO, DVec3::X, DVec3::Z), 0.0);
    assert_eq!(corner_angle(DVec3::X, DVec3::ZERO, DVec3::Z), 0.0);
}
