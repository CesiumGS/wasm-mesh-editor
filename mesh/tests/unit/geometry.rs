use super::*;

fn ray(origin: DVec3) -> Ray {
    Ray {
        origin,
        direction: -DVec3::Z,
    }
}

#[test]
fn plane_validates_geometry_and_intersects_forward_rays() {
    for (position, normal) in [
        (DVec3::NAN, DVec3::Z),
        (DVec3::splat(f64::INFINITY), DVec3::Z),
        (DVec3::ZERO, DVec3::ZERO),
        (DVec3::ZERO, 2.0 * DVec3::Z),
        (DVec3::ZERO, DVec3::NAN),
    ] {
        assert!(Plane::new(position, normal).is_none());
    }

    let plane = Plane::new(2.0 * DVec3::Z, DVec3::Z).unwrap();
    assert_eq!(
        plane.intersect(ray(DVec3::new(3.0, 4.0, 10.0))),
        Some(DVec3::new(3.0, 4.0, 2.0))
    );
    assert_eq!(plane.intersect(ray(2.0 * DVec3::Z)), Some(2.0 * DVec3::Z));
    assert_eq!(plane.intersect(ray(DVec3::Z)), None);
    assert_eq!(plane.position, 2.0 * DVec3::Z);
    assert_eq!(plane.normal, DVec3::Z);
}

#[test]
fn plane_rejects_invalid_and_near_parallel_rays() {
    let plane = Plane::new(DVec3::ZERO, DVec3::Z).unwrap();
    let initial = ray(DVec3::Z);
    for invalid in [
        Ray {
            direction: DVec3::X,
            ..initial
        },
        Ray {
            direction: DVec3::new(1.0, 0.0, -1e-14).normalize(),
            ..initial
        },
        Ray {
            direction: DVec3::Z,
            ..initial
        },
        Ray {
            direction: DVec3::ZERO,
            ..initial
        },
        Ray {
            direction: -2.0 * DVec3::Z,
            ..initial
        },
        Ray {
            direction: DVec3::NAN,
            ..initial
        },
        Ray {
            origin: DVec3::splat(f64::INFINITY),
            ..initial
        },
    ] {
        assert_eq!(plane.intersect(invalid), None);
    }
}

#[test]
fn oblique_plane_preserves_small_motion_at_world_scale() {
    let position = DVec3::new(6_378_137.125, 4_000_000.5, 2_000_000.25);
    let plane = Plane::new(position, DVec3::new(0.0, 1.0, 1.0).normalize()).unwrap();
    let initial = ray(position + 10.0 * DVec3::Z);
    let start = plane.intersect(initial).unwrap();
    let current = plane
        .intersect(ray(initial.origin + DVec3::new(0.125, 0.25, 0.0)))
        .unwrap();

    assert_eq!(current - start, DVec3::new(0.125, 0.25, -0.25));
}

#[test]
fn affine_inverse_accepts_affine_matrices_and_rejects_unusable_ones() {
    let mut shear = DMat4::IDENTITY;
    shear.y_axis.x = 0.5;
    for matrix in [
        DMat4::IDENTITY,
        DMat4::from_translation(DVec3::splat(6_378_137.125)),
        DMat4::from_scale(DVec3::new(-2.0, 3.0, 4.0)),
        shear,
    ] {
        let inverse = try_inverse_affine(matrix).unwrap();
        assert!((matrix * inverse).abs_diff_eq(DMat4::IDENTITY, 1e-12));
    }

    let mut projective = DMat4::IDENTITY;
    projective.x_axis.w = 1.0;
    for matrix in [
        DMat4::ZERO,
        DMat4::from_scale(DVec3::new(0.0, 1.0, 1.0)),
        DMat4::from_scale(DVec3::splat(f64::INFINITY)),
        DMat4::from_scale(DVec3::splat(f64::NAN)),
        DMat4::from_scale(DVec3::splat(1e200)),
        DMat4::from_scale(DVec3::new(1e-310, 1.0, 1.0)),
        projective,
    ] {
        assert_eq!(try_inverse_affine(matrix), None);
    }
}

#[test]
fn centroid_handles_empty_input_and_counts_points() {
    assert_eq!(centroid([]), Some((DVec3::ZERO, 0)));
    assert_eq!(centroid([DVec3::X]), Some((DVec3::X, 1)));
    assert_eq!(
        centroid([DVec3::ZERO, 2.0 * DVec3::X, 4.0 * DVec3::Y]),
        Some((DVec3::new(2.0 / 3.0, 4.0 / 3.0, 0.0), 3))
    );
}

#[test]
fn centroid_preserves_f64_means_and_sums_beyond_f32_range() {
    for (positions, expected) in [
        ([6_378_137.0_f32, 6_378_137.5_f32], 6_378_137.25),
        ([f32::MAX, f32::MAX], f32::MAX as f64),
    ] {
        assert_eq!(
            centroid(
                positions
                    .into_iter()
                    .map(|position| DVec3::splat(position as f64))
            ),
            Some((DVec3::splat(expected), 2))
        );
    }
}

#[test]
fn centroid_rejects_non_finite_positions_and_sum_overflow() {
    for position in [
        DVec3::NAN,
        DVec3::splat(f64::INFINITY),
        DVec3::splat(f64::NEG_INFINITY),
    ] {
        assert_eq!(centroid([DVec3::ZERO, position]), None);
    }

    assert_eq!(centroid([DVec3::splat(f64::MAX); 2]), None);
}

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
