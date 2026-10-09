use event_emitter::EventEmitter;
use glam::Vec3;
use mesh_core::{ComponentTypes, MeshBuffers, TopologyChange};
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
    assert!(matches!(overlay.pending_upload, PendingChanges::Full));
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
    assert!(matches!(overlay.pending_upload, PendingChanges::Full));
}

#[test]
fn update_delivers_initial_upload_once() {
    for mut mesh in [Mesh::new(), mixed_face_mesh()] {
        let mut overlay = TopologyOverlay::new(&mut mesh);

        let buffers = overlay.update(&mesh).unwrap();
        assert!(buffers.full_refresh);
        assert!(buffers.positions.dirty_ranges.is_empty());
        assert!(matches!(overlay.pending_upload, PendingChanges::None));
        assert!(overlay.update(&mesh).is_none());
        assert!(overlay.update(&mesh).is_none());
    }
}

#[test]
fn update_includes_changes_queued_before_initial_upload() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);

    mesh.selection_mut().select(&[vertex]);
    mesh.translate_selected(Vec3::X);

    let buffers = overlay.update(&mesh).unwrap();
    assert!(buffers.full_refresh);
    assert_eq!(buffers.positions.data[5], [3.0, 3.0, 4.0, 0.0]);
    assert_eq!(
        buffers.selection[ComponentType::Vertex].data,
        vec![0, 0, 0, 0, 0, 255]
    );
    assert!(buffers.positions.dirty_ranges.is_empty());
    assert!(
        buffers.selection[ComponentType::Vertex]
            .dirty_ranges
            .is_empty()
    );
    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_returns_partial_uploads_and_clears_previous_ranges() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.selection_mut().select(&[vertex]);

    let buffers = overlay.update(&mesh).unwrap();
    assert!(!buffers.full_refresh);
    assert_eq!(buffers.selection[ComponentType::Vertex].data[5], 255);
    assert_eq!(
        buffers.selection[ComponentType::Vertex].dirty_ranges,
        vec![5..6]
    );
    assert!(buffers.positions.dirty_ranges.is_empty());

    mesh.translate_selected(Vec3::X);
    mesh.translate_selected(Vec3::Y);

    let buffers = overlay.update(&mesh).unwrap();
    assert!(!buffers.full_refresh);
    assert_eq!(buffers.positions.data[5], [3.0, 4.0, 4.0, 0.0]);
    assert_eq!(buffers.positions.dirty_ranges, vec![5..6, 5..6]);
    assert!(
        buffers.selection[ComponentType::Vertex]
            .dirty_ranges
            .is_empty()
    );

    mesh.selection_mut().clear();

    let buffers = overlay.update(&mesh).unwrap();
    assert!(!buffers.full_refresh);
    assert_eq!(buffers.selection[ComponentType::Vertex].data[5], 0);
    assert!(buffers.positions.dirty_ranges.is_empty());
    assert!(overlay.update(&mesh).is_none());
    assert!(
        overlay.buffers.selection[ComponentType::Vertex]
            .dirty_ranges
            .is_empty()
    );
}

#[test]
fn update_uses_current_selection_for_queued_changes() {
    let mut mesh = mixed_face_mesh();
    let faces: Vec<_> = mesh.topology().faces().collect();
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.selection_mut().select(&[faces[1]]);
    mesh.selection_mut().clear();
    mesh.selection_mut().select(&[faces[0]]);

    let buffers = overlay.update(&mesh).unwrap();
    assert!(!buffers.full_refresh);
    assert_eq!(
        buffers.selection[ComponentType::Vertex].data,
        vec![255, 255, 255, 255, 0, 0]
    );
    assert_eq!(buffers.selection[ComponentType::Face].data, vec![255, 0]);

    for (index, key) in mesh.topology().edges().enumerate() {
        assert_eq!(
            buffers.selection[ComponentType::Edge].data[index],
            if mesh.selection().contains(key) {
                255
            } else {
                0
            }
        );
    }

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(!buffers.selection[kind].dirty_ranges.is_empty());
    }

    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_ignores_shading_changes() {
    let mut mesh = mixed_face_mesh();
    let face = mesh.topology().faces().next().unwrap();
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    mesh.selection_mut().select(&[face]);
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.shade_flat();
    mesh.shade_smooth();

    assert!(overlay.update(&mesh).is_none());
    assert!(matches!(overlay.pending_upload, PendingChanges::None));
}

#[test]
fn update_rebuilds_buffers_and_maps_on_topology_events() {
    let mut mesh = Mesh::new();
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    let mut changes = EventEmitter::new();
    overlay.mesh_changes = changes.subscribe();

    for next_mesh in [mixed_face_mesh(), Mesh::new()] {
        let removed_verts = mesh.topology().verts().collect();
        let removed_edges = mesh.topology().edges().collect();
        let removed_faces = mesh.topology().faces().collect();
        mesh = next_mesh;

        changes.emit(MeshChange::Topology(TopologyChange {
            added_verts: mesh.topology().verts().collect(),
            removed_verts,
            added_edges: mesh.topology().edges().collect(),
            removed_edges,
            added_faces: mesh.topology().faces().collect(),
            removed_faces,
        }));

        let mut selection_change = SelectionChange::default();
        selection_change.added.verts = mesh.topology().verts().collect();
        changes.emit(MeshChange::Selection(selection_change));

        let buffers = overlay.update(&mesh).unwrap();
        let topology = mesh.topology();
        assert!(buffers.full_refresh);
        assert_eq!(buffers.positions.data.len(), topology.vert_count());
        assert_eq!(buffers.edges.data.len(), topology.edge_count());
        assert_eq!(
            buffers.triangles.data.len(),
            topology.loop_count() - 2 * topology.face_count()
        );
        assert!(buffers.positions.dirty_ranges.is_empty());
        assert!(
            buffers.selection[ComponentType::Vertex]
                .dirty_ranges
                .is_empty()
        );
        assert_eq!(
            overlay.vertex_key_to_buffer_index.len(),
            topology.vert_count()
        );
        assert_eq!(
            overlay.edge_key_to_buffer_index.len(),
            topology.edge_count()
        );
        assert_eq!(
            overlay.face_key_to_buffer_index.len(),
            topology.face_count()
        );
        assert_eq!(
            overlay.buffer_index_to_vertex_key,
            topology.verts().collect::<Vec<_>>()
        );
        assert_eq!(
            overlay.buffer_index_to_edge_key,
            topology.edges().collect::<Vec<_>>()
        );
        assert_eq!(
            overlay.buffer_index_to_face_key,
            topology.faces().collect::<Vec<_>>()
        );
        assert!(overlay.update(&mesh).is_none());
    }
}
