use super::*;

fn right_triangle() -> Triangle {
    Triangle::new([DVec3::ZERO, DVec3::X, DVec3::Y]).unwrap()
}

#[test]
fn new_rejects_triangles_without_usable_area() {
    for positions in [
        [DVec3::ZERO, DVec3::X, 2.0 * DVec3::X],
        [DVec3::ZERO, DVec3::ZERO, DVec3::Y],
        [DVec3::ZERO, DVec3::X, DVec3::NAN],
        [DVec3::ZERO, DVec3::X, DVec3::INFINITY],
    ] {
        assert_eq!(Triangle::new(positions), None);
    }

    assert_eq!(
        right_triangle().positions(),
        [DVec3::ZERO, DVec3::X, DVec3::Y]
    );
}

#[test]
fn closest_point_handles_interior_edge_and_vertex_regions() {
    for (point, expected) in [
        (DVec3::new(0.25, 0.25, 2.0), [0.5, 0.25, 0.25]),
        (DVec3::new(-1.0, -1.0, 0.0), [1.0, 0.0, 0.0]),
        (DVec3::new(2.0, -0.5, 0.0), [0.0, 1.0, 0.0]),
        (DVec3::new(-0.5, 2.0, 1.0), [0.0, 0.0, 1.0]),
        (DVec3::new(0.5, -1.0, 0.0), [0.5, 0.5, 0.0]),
        (DVec3::new(-1.0, 0.25, 0.0), [0.75, 0.0, 0.25]),
        (DVec3::new(1.0, 1.0, 0.0), [0.0, 0.5, 0.5]),
    ] {
        assert_eq!(right_triangle().closest_point(point), expected);
    }
}

#[test]
fn point_at_weights_the_positions() {
    let triangle = Triangle::new([DVec3::ZERO, 2.0 * DVec3::X, 4.0 * DVec3::Y]).unwrap();

    assert_eq!(
        triangle.point_at([0.5, 0.25, 0.25]),
        DVec3::new(0.5, 1.0, 0.0)
    );
}

#[test]
fn to_frame_is_affine_in_the_plane_and_unit_distance_along_the_normal() {
    let triangle = Triangle::new([DVec3::ZERO, 2.0 * DVec3::X, 2.0 * DVec3::Y]).unwrap();

    assert_eq!(
        triangle.to_frame(DVec3::new(0.5, 1.0, 3.0)),
        DVec3::new(0.25, 0.5, 3.0)
    );
    assert_eq!(
        triangle.to_frame(DVec3::new(1.0, -1.0, -2.0)),
        DVec3::new(0.5, -0.5, -2.0)
    );
}

#[test]
fn to_frame_reconstructs_points_off_a_tilted_triangle() {
    let triangle = Triangle::new([
        DVec3::new(1.0, -2.0, 0.5),
        DVec3::new(3.0, 0.0, 1.0),
        DVec3::new(0.0, 2.0, 2.5),
    ])
    .unwrap();
    let [p0, p1, p2] = triangle.positions();
    let normal = (p1 - p0).cross(p2 - p0).normalize();

    for point in [DVec3::new(1.0, 0.0, 4.0), DVec3::new(-3.0, 5.0, -1.0), p2] {
        let [a, b, c] = triangle.to_frame(point).to_array();
        let reconstructed = p0 + a * (p1 - p0) + b * (p2 - p0) + c * normal;

        assert!(reconstructed.abs_diff_eq(point, 1e-12));
    }
}
