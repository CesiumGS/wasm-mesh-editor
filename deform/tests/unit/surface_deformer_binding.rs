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
        .deformer_verts
        .map(|key| mesh.attributes().positions[key].as_dvec3());
    let normal = (p1 - p0).cross(p2 - p0).normalize();
    let [a, b, c] = point.frame_coords.to_array();
    p0 + a * (p1 - p0) + b * (p2 - p0) + c * normal
}

#[test]
fn frame_coords_reconstruct_points_above_below_and_beyond_edges_and_vertices() {
    let (mesh, _) = deformer();
    let positions = [
        Vec3::new(0.5, 0.5, 1.0),
        Vec3::new(0.5, 0.5, -1.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(1.5, 1.5, 3.0),
        Vec3::new(3.0, 3.0, 2.0),
    ];
    let binding = SurfaceDeformerBinding::new(&mesh, &positions, DMat4::IDENTITY).unwrap();

    assert_eq!(binding.points.len(), positions.len());
    for (point, position) in binding.points.iter().zip(positions) {
        assert!(reconstruct(&mesh, point).abs_diff_eq(position.as_dvec3(), 1e-12));
    }
}

#[test]
fn margins_are_the_largest_normal_distance_and_overshoot() {
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
    let binding = SurfaceDeformerBinding::new(&mesh, &positions, DMat4::IDENTITY).unwrap();

    // Weights (0.25, 0.25, 0.5) and (1.0, 0.5, -0.5).
    assert_eq!(binding.max_normal_distance, 3.0);
    assert_eq!(binding.max_overshoot, 0.5);
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
    let binding = SurfaceDeformerBinding::new(&mesh, &positions, transform).unwrap();

    for (point, position) in binding.points.iter().zip(positions) {
        let expected = transform.transform_point3(position.as_dvec3());
        assert!(reconstruct(&mesh, point).abs_diff_eq(expected, 1e-12));
    }
}

#[test]
fn deformed_points_stay_within_the_expanded_deformer_bounds() {
    let (mesh, verts) = deformer();
    let positions = [
        Vec3::new(0.5, 0.5, 1.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(3.0, 3.0, 2.0),
        Vec3::new(-0.5, 2.5, 0.0),
    ];
    let binding = SurfaceDeformerBinding::new(&mesh, &positions, DMat4::IDENTITY).unwrap();
    assert!(binding.max_overshoot > 0.0);

    // Stretches, shears, and flattens the deformer.
    let moved = |key: VertKey| {
        let p = mesh.attributes().positions[key].as_dvec3();
        DVec3::new(5.0 * p.x + p.y, 0.5 * p.y, 0.1 * p.z - p.x)
    };
    let corners: Vec<_> = verts.iter().map(|&key| moved(key)).collect();
    let min = corners.iter().copied().reduce(DVec3::min).unwrap();
    let max = corners.iter().copied().reduce(DVec3::max).unwrap();
    let margin = binding.max_overshoot * (max - min) + binding.max_normal_distance;

    for point in &binding.points {
        let [p0, p1, p2] = point.deformer_verts.map(moved);
        let normal = (p1 - p0).cross(p2 - p0).normalize();
        let [a, b, c] = point.frame_coords.to_array();
        let deformed = p0 + a * (p1 - p0) + b * (p2 - p0) + c * normal;

        assert!(deformed.cmpge(min - margin - 1e-12).all());
        assert!(deformed.cmple(max + margin + 1e-12).all());
    }
}

#[test]
fn binding_is_none_without_a_surface_and_empty_without_positions() {
    let (mesh, _) = deformer();
    let binding = SurfaceDeformerBinding::new(&mesh, &[], DMat4::IDENTITY).unwrap();
    assert!(binding.points.is_empty());
    assert_eq!(binding.max_normal_distance, 0.0);
    assert_eq!(binding.max_overshoot, 0.0);

    assert!(SurfaceDeformerBinding::new(&Mesh::new(), &[Vec3::ZERO], DMat4::IDENTITY).is_none());
    assert!(SurfaceDeformerBinding::new(&mesh, &[Vec3::NAN], DMat4::IDENTITY).is_none());
}

#[test]
fn pack_writes_mapped_indices_and_frame_coords_per_point() {
    let (mesh, verts) = deformer();
    let binding = SurfaceDeformerBinding::new(
        &mesh,
        &[Vec3::new(0.5, 0.5, 1.0), Vec3::new(1.5, 1.5, 3.0)],
        DMat4::IDENTITY,
    )
    .unwrap();
    let index = |key| 10 * verts.iter().position(|&vert| vert == key).unwrap() as u32;
    let mut out = [0.0; 12];

    binding.pack(index, &mut out);

    for (point, out) in binding.points.iter().zip(out.chunks_exact(6)) {
        let indices = point.deformer_verts.map(|key| index(key) as f32);
        assert_eq!(out[..3], indices);
        assert_eq!(out[3..], point.frame_coords.as_vec3().to_array());
    }
    assert_eq!(out[..3], [0.0, 10.0, 20.0]);
    assert_eq!(out[6..9], [10.0, 30.0, 20.0]);
}

#[test]
#[should_panic(expected = "6 values per point")]
fn pack_panics_for_a_mismatched_output_length() {
    let (mesh, _) = deformer();
    let binding = SurfaceDeformerBinding::new(&mesh, &[Vec3::ZERO], DMat4::IDENTITY).unwrap();

    binding.pack(|_| 0, &mut [0.0; 5]);
}
