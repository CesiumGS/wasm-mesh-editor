use mesh::MeshBuffers;

use super::*;

/// Two tilted triangles sharing the edge from vertex 1 to vertex 2.
fn deformer() -> (Mesh, Vec<VertKey>) {
    let mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 0.5),
            Vec3::new(0.0, 2.0, 0.5),
            Vec3::new(2.0, 2.0, 2.0),
        ],
        indices: vec![0, 1, 2, 1, 3, 2],
        ..Default::default()
    })
    .unwrap();
    let verts = mesh.topology().verts().collect();
    (mesh, verts)
}

/// `P0 + a * (P1 - P0) + b * (P2 - P0) + c * n` for a bound point.
fn reconstruct(mesh: &Mesh, point: &BoundPoint) -> DVec3 {
    let [p0, p1, p2] = point
        .verts
        .map(|key| mesh.attributes().positions[key].as_dvec3());
    let normal = (p1 - p0).cross(p2 - p0).normalize();
    let [a, b, c] = point.coords.to_array();
    p0 + a * (p1 - p0) + b * (p2 - p0) + c * normal
}

#[test]
fn coords_reconstruct_points_above_below_and_beyond_edges_and_vertices() {
    let (mesh, _) = deformer();
    let positions = [
        Vec3::new(0.5, 0.5, 1.0),
        Vec3::new(0.5, 0.5, -1.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(1.5, 1.5, 3.0),
        Vec3::new(3.0, 3.0, 2.0),
    ];
    let binding = SurfaceBinding::new(&mesh, &positions, DMat4::IDENTITY).unwrap();

    assert_eq!(binding.points.len(), positions.len());
    for (point, position) in binding.points.iter().zip(positions) {
        assert!(reconstruct(&mesh, point).abs_diff_eq(position.as_dvec3(), 1e-12));
    }
}

#[test]
fn coords_are_affine_in_the_plane_and_unit_distance_along_the_normal() {
    let mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ],
        indices: vec![0, 1, 2],
        ..Default::default()
    })
    .unwrap();
    let positions = [Vec3::new(0.5, 1.0, 3.0), Vec3::new(1.0, -1.0, -2.0)];
    let binding = SurfaceBinding::new(&mesh, &positions, DMat4::IDENTITY).unwrap();

    assert_eq!(binding.points[0].coords, DVec3::new(0.25, 0.5, 3.0));
    assert_eq!(binding.points[1].coords, DVec3::new(0.5, -0.5, -2.0));
}

#[test]
fn positions_are_transformed_into_deformer_space() {
    let (mesh, _) = deformer();
    let transform = DMat4::from_scale_rotation_translation(
        DVec3::splat(2.0),
        glam::DQuat::from_rotation_z(0.5),
        DVec3::new(1.0, -2.0, 3.0),
    );
    let positions = [Vec3::new(0.1, 0.2, 0.3), Vec3::new(-0.4, 0.5, -0.6)];
    let binding = SurfaceBinding::new(&mesh, &positions, transform).unwrap();

    for (point, position) in binding.points.iter().zip(positions) {
        let expected = transform.transform_point3(position.as_dvec3());
        assert!(reconstruct(&mesh, point).abs_diff_eq(expected, 1e-12));
    }
}

#[test]
fn max_distance_is_the_largest_distance_to_the_surface() {
    let (mesh, _) = deformer();
    // Beyond the edge from vertex 0 to vertex 1, the closest point is on the edge, so the distance
    // exceeds the distance along the normal.
    let positions = [Vec3::new(1.0, 0.5, 0.25), Vec3::new(1.0, -1.0, 0.25)];
    let binding = SurfaceBinding::new(&mesh, &positions, DMat4::IDENTITY).unwrap();

    assert!((binding.max_distance - 1.0).abs() < 1e-12);
    assert!(binding.points[1].coords.z.abs() < binding.max_distance);
}

#[test]
fn binding_is_none_without_a_surface_and_empty_without_positions() {
    let (mesh, _) = deformer();
    let binding = SurfaceBinding::new(&mesh, &[], DMat4::IDENTITY).unwrap();
    assert!(binding.points.is_empty());
    assert_eq!(binding.max_distance, 0.0);

    assert!(SurfaceBinding::new(&Mesh::new(), &[Vec3::ZERO], DMat4::IDENTITY).is_none());
    assert!(SurfaceBinding::new(&mesh, &[Vec3::NAN], DMat4::IDENTITY).is_none());
}

#[test]
fn pack_writes_mapped_indices_and_coords_per_point() {
    let (mesh, verts) = deformer();
    let binding = SurfaceBinding::new(
        &mesh,
        &[Vec3::new(0.5, 0.5, 1.0), Vec3::new(1.5, 1.5, 3.0)],
        DMat4::IDENTITY,
    )
    .unwrap();
    let index = |key| 10 * verts.iter().position(|&vert| vert == key).unwrap() as u32;
    let mut out = [0.0; 12];

    binding.pack(index, &mut out);

    for (point, out) in binding.points.iter().zip(out.chunks_exact(6)) {
        let indices = point.verts.map(|key| index(key) as f32);
        assert_eq!(out[..3], indices);
        assert_eq!(out[3..], point.coords.as_vec3().to_array());
    }
    assert_eq!(out[..3], [0.0, 10.0, 20.0]);
    assert_eq!(out[6..9], [10.0, 30.0, 20.0]);
}

#[test]
#[should_panic(expected = "6 values per point")]
fn pack_panics_for_a_mismatched_output_length() {
    let (mesh, _) = deformer();
    let binding = SurfaceBinding::new(&mesh, &[Vec3::ZERO], DMat4::IDENTITY).unwrap();

    binding.pack(|_| 0, &mut [0.0; 5]);
}
