use glam::Vec3;
use mesh_core::{ComponentTypes, MeshBuffers};
use std::sync::mpsc::TryRecvError;

use super::*;

fn mixed_face_mesh() -> Mesh {
    Mesh::from_buffers(MeshBuffers {
        positions: vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::Y,
            Vec3::Z,
            Vec3::new(2.0, 3.0, 4.0),
        ],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 3, 1, 0, 4],
        face_vertex_counts: Some(vec![4, 3]),
    })
    .unwrap()
}

#[test]
fn new_packs_buffers_and_bidirectional_maps() {
    let mut mesh = mixed_face_mesh();
    let overlay = TopologyOverlay::new(&mut mesh);
    let buffers = &overlay.buffers;

    assert_eq!(buffers.positions.data.len(), 6);
    assert_eq!(buffers.edges.data.len(), 6);
    assert_eq!(
        buffers.triangles.data,
        vec![[0, 1, 2, 0], [0, 2, 3, 0], [1, 0, 4, 1]]
    );
    assert_eq!(overlay.vertex_key_to_buffer_index.len(), 6);
    assert_eq!(overlay.edge_key_to_buffer_index.len(), 6);
    assert_eq!(overlay.face_key_to_buffer_index.len(), 2);

    for (index, &key) in overlay.buffer_index_to_vertex_key.iter().enumerate() {
        assert_eq!(overlay.vertex_key_to_buffer_index[key], index as u32);
        assert_eq!(
            buffers.positions.data[index],
            mesh.attributes().positions[key].extend(0.0).to_array()
        );
    }

    for (index, &key) in overlay.buffer_index_to_edge_key.iter().enumerate() {
        assert_eq!(overlay.edge_key_to_buffer_index[key], index as u32);
        assert_eq!(
            buffers.edges.data[index]
                .map(|vertex| overlay.buffer_index_to_vertex_key[vertex as usize]),
            mesh.edge(key).unwrap().verts()
        );
    }

    for (index, &key) in overlay.buffer_index_to_face_key.iter().enumerate() {
        assert_eq!(overlay.face_key_to_buffer_index[key], index as u32);

        let triangles: Vec<_> = buffers
            .triangles
            .data
            .iter()
            .filter(|triangle| triangle[3] == index as u32)
            .map(|triangle| {
                [triangle[0], triangle[1], triangle[2]]
                    .map(|vertex| overlay.buffer_index_to_vertex_key[vertex as usize])
            })
            .collect();

        assert_eq!(
            triangles,
            mesh.face(key).unwrap().triangulation().collect::<Vec<_>>()
        );
    }

    for (kind, count) in [
        (ComponentType::Vertex, 6),
        (ComponentType::Edge, 6),
        (ComponentType::Face, 2),
    ] {
        assert_eq!(buffers.selection[kind].data, vec![0; count]);
        assert!(buffers.selection[kind].dirty_ranges.is_empty());
    }

    assert!(buffers.full_refresh);
    assert!(overlay.initial_upload_pending);
    assert!(buffers.positions.dirty_ranges.is_empty());
    assert!(buffers.edges.dirty_ranges.is_empty());
    assert!(buffers.triangles.dirty_ranges.is_empty());
}

#[test]
fn new_seeds_selection_and_subscribes_to_future_changes() {
    let mut mesh = mixed_face_mesh();
    let face = mesh.topology().faces().nth(1).unwrap();
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    mesh.selection_mut().select(&[face]);

    let overlay = TopologyOverlay::new(&mut mesh);

    assert_eq!(
        overlay.buffers.selection[ComponentType::Vertex].data,
        vec![255, 255, 0, 0, 255, 0]
    );
    assert_eq!(
        overlay.buffers.selection[ComponentType::Face].data,
        vec![0, 255]
    );

    for (index, &key) in overlay.buffer_index_to_edge_key.iter().enumerate() {
        assert_eq!(
            overlay.buffers.selection[ComponentType::Edge].data[index],
            if mesh.selection().contains(key) {
                255
            } else {
                0
            }
        );
    }

    assert!(matches!(
        overlay.mesh_changes.try_recv(),
        Err(TryRecvError::Empty)
    ));

    mesh.selection_mut().clear();

    let event = overlay.mesh_changes.try_recv().unwrap();
    assert!(matches!(event.as_ref(), MeshChange::Selection(_)));
    assert!(matches!(
        overlay.mesh_changes.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[test]
fn new_accepts_an_empty_mesh() {
    let overlay = TopologyOverlay::new(&mut Mesh::new());

    assert!(overlay.buffers.positions.data.is_empty());
    assert!(overlay.buffers.edges.data.is_empty());
    assert!(overlay.buffers.triangles.data.is_empty());
    assert!(overlay.vertex_key_to_buffer_index.is_empty());
    assert!(overlay.edge_key_to_buffer_index.is_empty());
    assert!(overlay.face_key_to_buffer_index.is_empty());
    assert!(overlay.buffer_index_to_vertex_key.is_empty());
    assert!(overlay.buffer_index_to_edge_key.is_empty());
    assert!(overlay.buffer_index_to_face_key.is_empty());

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(overlay.buffers.selection[kind].data.is_empty());
    }

    assert!(overlay.buffers.full_refresh);
    assert!(overlay.initial_upload_pending);
}
