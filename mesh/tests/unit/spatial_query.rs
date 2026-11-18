use geometry::Triangle;
use glam::Vec3;

use crate::{Attributes, MeshBuffers};

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

/// A bumpy `size` x `size` grid of quads.
fn grid(size: u32) -> Mesh {
    let row = size + 1;
    let positions = (0..row * row)
        .map(|index| {
            let (x, y) = ((index % row) as f32, (index / row) as f32);
            Vec3::new(x, y, (x * 0.7).sin() * (y * 0.4).cos())
        })
        .collect();
    let indices = (0..size * size)
        .flat_map(|quad| {
            let corner = quad / size * row + quad % size;
            [corner, corner + 1, corner + row + 1, corner + row]
        })
        .collect();

    Mesh::from_buffers(MeshBuffers {
        positions,
        indices,
        face_vertex_counts: Some(vec![4; (size * size) as usize]),
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn closest_point_matches_brute_force() {
    let mesh = grid(20);
    let positions = mesh.attributes.positions();

    for index in 0..200 {
        let t = index as f64;
        let point = DVec3::new(
            (t * 0.37) % 24.0 - 2.0,
            (t * 0.61) % 24.0 - 2.0,
            (t * 0.13) % 6.0 - 3.0,
        );
        let brute_force = mesh
            .topology
            .faces()
            .flat_map(|face| mesh.face(face).unwrap().triangulation().collect::<Vec<_>>())
            .filter_map(|verts| Triangle::new(verts.map(|key| positions[key].as_dvec3())))
            .map(|triangle| point.distance(triangle.point_at(triangle.closest_point(point))))
            .fold(f64::INFINITY, f64::min);
        let closest = mesh.spatial_query().closest_point(point).unwrap();

        assert!((point.distance(closest.position) - brute_force).abs() < 1e-12);
    }
}

#[test]
fn closest_point_follows_translated_vertices() {
    let (mut mesh, v) = build_mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [10.0, 0.0, 0.0],
            [11.0, 0.0, 0.0],
            [10.0, 1.0, 0.0],
        ],
        &[3, 3],
    );
    let point = DVec3::new(0.25, 0.25, 2.0);
    assert_eq!(
        mesh.spatial_query().closest_point(point).unwrap().verts,
        [v[0], v[1], v[2]]
    );

    mesh.selection_mut().select(&v[3..]);
    mesh.translate_selected(Vec3::new(-10.0, 0.0, 1.5));

    let closest = mesh.spatial_query().closest_point(point).unwrap();
    assert_eq!(closest.verts, [v[3], v[4], v[5]]);
    assert_eq!(closest.position, DVec3::new(0.25, 0.25, 1.5));
}

#[test]
fn closest_point_follows_written_positions() {
    let (mut mesh, v) = build_mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [10.0, 0.0, 0.0],
            [11.0, 0.0, 0.0],
            [10.0, 1.0, 0.0],
        ],
        &[3, 3],
    );
    let point = DVec3::new(0.25, 0.25, 2.0);
    assert_eq!(
        mesh.spatial_query().closest_point(point).unwrap().verts,
        [v[0], v[1], v[2]]
    );

    let moved = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)].map(|(x, y)| Vec3::new(x, y, 1.5));
    let values: Vec<_> = v[3..].iter().copied().zip(moved).collect();
    mesh.set_attribute_values(Attributes::POSITION, &values)
        .unwrap();

    let closest = mesh.spatial_query().closest_point(point).unwrap();
    assert_eq!(closest.verts, [v[3], v[4], v[5]]);
    assert_eq!(closest.position, DVec3::new(0.25, 0.25, 1.5));
}
