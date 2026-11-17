use glam::Vec3;

use crate::MeshBuffers;

use super::*;

fn build_mesh(positions: &[[f32; 3]], face_vertex_counts: &[u32]) -> (Mesh, Vec<VertKey>) {
    let mesh = Mesh::from_buffers(MeshBuffers {
        positions: positions.iter().copied().map(Vec3::from).collect(),
        indices: (0..positions.len() as u32).collect(),
        face_vertex_counts: Some(face_vertex_counts.to_vec()),
        ..Default::default()
    })
    .unwrap();
    let verts = mesh.topology.verts().collect();
    (mesh, verts)
}

#[test]
fn closest_point_reports_face_triangle_and_position() {
    let (mesh, v) = build_mesh(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], &[3]);
    let face = mesh.topology.faces().next().unwrap();

    assert_eq!(
        mesh.spatial_query()
            .closest_point(DVec3::new(0.25, 0.25, 2.0)),
        Some(ClosestPoint {
            face,
            verts: [v[0], v[1], v[2]],
            bary: [0.5, 0.25, 0.25],
            position: DVec3::new(0.25, 0.25, 0.0),
        })
    );
    assert_eq!(
        mesh.spatial_query()
            .closest_point(DVec3::new(0.5, -1.0, 0.0)),
        Some(ClosestPoint {
            face,
            verts: [v[0], v[1], v[2]],
            bary: [0.5, 0.5, 0.0],
            position: DVec3::new(0.5, 0.0, 0.0),
        })
    );
}

#[test]
fn closest_point_uses_fan_triangulation_of_ngons() {
    let (mesh, v) = build_mesh(
        &[
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        &[5],
    );

    for (point, verts) in [
        (DVec3::new(1.5, 0.25, 1.0), [v[0], v[1], v[2]]),
        (DVec3::new(1.0, 1.0, 1.0), [v[0], v[2], v[3]]),
        (DVec3::new(0.25, 1.0, 1.0), [v[0], v[3], v[4]]),
    ] {
        let closest = mesh.spatial_query().closest_point(point).unwrap();

        assert_eq!(closest.verts, verts);
        assert!(closest.position.abs_diff_eq(point.with_z(0.0), 1e-12));
    }
}

#[test]
fn closest_point_skips_degenerate_triangles() {
    let (mesh, v) = build_mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 0.0, 5.0],
            [1.0, 0.0, 5.0],
            [0.0, 1.0, 5.0],
        ],
        &[3, 3],
    );
    let closest = mesh
        .spatial_query()
        .closest_point(DVec3::new(1.0, 0.0, 0.0))
        .unwrap();

    assert_eq!(closest.verts, [v[3], v[4], v[5]]);
    assert_eq!(closest.position, DVec3::new(1.0, 0.0, 5.0));

    let (degenerate, _) = build_mesh(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], &[3]);
    assert_eq!(degenerate.spatial_query().closest_point(DVec3::ZERO), None);
}

#[test]
fn closest_point_is_none_for_empty_meshes_and_non_finite_points() {
    assert_eq!(Mesh::new().spatial_query().closest_point(DVec3::ZERO), None);

    let (mesh, _) = build_mesh(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], &[3]);
    for point in [DVec3::NAN, DVec3::INFINITY] {
        assert_eq!(mesh.spatial_query().closest_point(point), None);
    }
}
