use event_emitter::EventEmitter;
use glam::Vec3;
use mesh_core::{ComponentMask, MeshBuffers, TopologyChange};
use std::sync::mpsc::TryRecvError;

use super::*;

#[test]
fn selection_buffers_index_independent_component_kinds() {
    let mut buffers = SelectionBuffers::default();
    let values = [
        (ComponentType::Vertex, 1),
        (ComponentType::Edge, 2),
        (ComponentType::Face, 3),
    ];

    for (kind, value) in values {
        buffers[kind] = OverlayBuffer::new(vec![value]);
    }

    assert_eq!(buffers.verts.data, vec![1]);
    assert_eq!(buffers.edges.data, vec![2]);
    assert_eq!(buffers.faces.data, vec![3]);

    let reader = &buffers;

    for (kind, value) in values {
        assert_eq!(reader[kind].data, vec![value]);
        assert!(reader[kind].dirty_ranges.is_empty());
    }
}

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
            overlay.component_key(OverlayComponent {
                kind: ComponentType::Vertex,
                index: index as u32
            }),
            Some(key.into())
        );
        assert_eq!(
            buffers.positions.data[index],
            mesh.attributes().positions[key].extend(0.0).to_array()
        );
    }

    for (index, &key) in overlay.buffer_index_to_edge_key.iter().enumerate() {
        assert_eq!(overlay.edge_key_to_buffer_index[key], index as u32);
        assert_eq!(
            overlay.component_key(OverlayComponent {
                kind: ComponentType::Edge,
                index: index as u32
            }),
            Some(key.into())
        );
        assert_eq!(
            buffers.edges.data[index]
                .map(|vertex| overlay.buffer_index_to_vertex_key[vertex as usize]),
            mesh.edge(key).unwrap().verts()
        );
    }

    for (index, &key) in overlay.buffer_index_to_face_key.iter().enumerate() {
        assert_eq!(overlay.face_key_to_buffer_index[key], index as u32);
        assert_eq!(
            overlay.component_key(OverlayComponent {
                kind: ComponentType::Face,
                index: index as u32
            }),
            Some(key.into())
        );

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
fn component_key_uses_face_indices_not_triangle_indices() {
    let mut mesh = mixed_face_mesh();
    let faces: Vec<_> = mesh.topology().faces().collect();
    let overlay = TopologyOverlay::new(&mut mesh);
    let picked_faces: Vec<_> = overlay
        .buffers()
        .triangles
        .data
        .iter()
        .map(|triangle| {
            overlay.component_key(OverlayComponent {
                kind: ComponentType::Face,
                index: triangle[3],
            })
        })
        .collect();

    assert_eq!(
        picked_faces,
        vec![
            Some(faces[0].into()),
            Some(faces[0].into()),
            Some(faces[1].into())
        ]
    );
    assert_eq!(
        overlay.component_key(OverlayComponent {
            kind: ComponentType::Face,
            index: 2
        }),
        None
    );
}

#[test]
fn component_key_rejects_out_of_bounds_indices() {
    let mut mesh = mixed_face_mesh();
    let overlay = TopologyOverlay::new(&mut mesh);

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        let count = overlay.buffers().selection[kind].data.len() as u32;

        assert_eq!(
            overlay.component_key(OverlayComponent { kind, index: count }),
            None
        );
        assert_eq!(
            overlay.component_key(OverlayComponent {
                kind,
                index: u32::MAX
            }),
            None
        );
    }
}

#[test]
fn new_seeds_selection_and_subscribes_to_future_changes() {
    let mut mesh = mixed_face_mesh();
    let face = mesh.topology().faces().nth(1).unwrap();
    mesh.selection_mut().set_level(ComponentMask::FACE);
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
        assert_eq!(
            overlay.component_key(OverlayComponent { kind, index: 0 }),
            None
        );
        assert_eq!(
            overlay.component_key(OverlayComponent {
                kind,
                index: u32::MAX
            }),
            None
        );
    }

    assert!(overlay.buffers.full_refresh);
    assert!(matches!(overlay.pending_upload, PendingChanges::Full));
}

#[test]
fn update_delivers_initial_upload_once() {
    for mut mesh in [Mesh::new(), mixed_face_mesh()] {
        let mut overlay = TopologyOverlay::new(&mut mesh);

        assert!(overlay.buffers().full_refresh);
        assert_eq!(
            overlay.buffers().positions.data.len(),
            mesh.topology().vert_count()
        );

        let buffers = overlay.update(&mesh).unwrap();
        assert!(buffers.full_refresh);
        assert!(buffers.positions.dirty_ranges.is_empty());
        let returned = std::ptr::from_ref(buffers);

        assert!(std::ptr::eq(returned, overlay.buffers()));
        assert!(std::ptr::eq(returned, overlay.buffers()));
        assert!(matches!(overlay.pending_upload, PendingChanges::None));
        assert!(overlay.update(&mesh).is_none());
        assert!(!overlay.buffers().full_refresh);
        assert!(std::ptr::eq(returned, overlay.buffers()));
        assert_eq!(
            overlay.buffers().positions.data.len(),
            mesh.topology().vert_count()
        );
        assert!(overlay.update(&mesh).is_none());
    }
}

#[test]
fn buffers_preserves_data_between_updates() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);

    overlay.update(&mesh).unwrap();
    mesh.selection_mut().select(&[vertex]);
    overlay.update(&mesh).unwrap();
    mesh.selection_mut().clear();

    for _ in 0..2 {
        let buffers = overlay.buffers();

        assert!(!buffers.full_refresh);
        assert_eq!(buffers.selection.verts.data[5], 255);
        assert_eq!(buffers.selection.verts.dirty_ranges, vec![5..6]);
        assert!(buffers.positions.dirty_ranges.is_empty());
    }

    overlay.update(&mesh).unwrap();

    assert_eq!(overlay.buffers().selection.verts.data[5], 0);
    assert!(overlay.update(&mesh).is_none());
    assert_eq!(overlay.buffers().selection.verts.data[5], 0);
    assert!(overlay.buffers().selection.verts.dirty_ranges.is_empty());
    assert!(!overlay.buffers().full_refresh);
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
fn update_replays_queued_selection_changes_in_order() {
    let mut mesh = mixed_face_mesh();
    let faces: Vec<_> = mesh.topology().faces().collect();
    mesh.selection_mut().set_level(ComponentMask::FACE);
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

    mesh.selection_mut().set(&[faces[1]]);

    let buffers = overlay.update(&mesh).unwrap();
    assert_eq!(
        buffers.selection[ComponentType::Vertex].data,
        vec![255, 255, 0, 0, 255, 0]
    );
    assert_eq!(buffers.selection[ComponentType::Face].data, vec![0, 255]);

    mesh.selection_mut().clear();
    mesh.selection_mut().select(&[faces[0]]);
    mesh.selection_mut().clear();

    let buffers = overlay.update(&mesh).unwrap();
    assert!(!buffers.full_refresh);

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(buffers.selection[kind].data.iter().all(|&value| value == 0));
        assert!(!buffers.selection[kind].dirty_ranges.is_empty());
    }

    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_reports_nonempty_selection_deltas_even_when_keys_are_unmapped() {
    use slotmap::Key;

    let mut mesh = Mesh::new();
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    let mut changes = EventEmitter::new();
    overlay.mesh_changes = changes.subscribe();
    changes.emit(MeshChange::Selection(SelectionChange::default()));
    assert!(overlay.update(&mesh).is_none());

    let mut change = SelectionChange::default();
    change.added.verts.push(VertKey::null());
    change.removed.edges.push(EdgeKey::null());
    change.added.faces.push(FaceKey::null());
    changes.emit(MeshChange::Selection(change));

    let buffers = overlay.update(&mesh).unwrap();
    assert!(!buffers.full_refresh);

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(buffers.selection[kind].data.is_empty());
        assert!(buffers.selection[kind].dirty_ranges.is_empty());
    }

    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_ignores_shading_changes() {
    let mut mesh = mixed_face_mesh();
    let face = mesh.topology().faces().next().unwrap();
    mesh.selection_mut().set_level(ComponentMask::FACE);
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

        for (kind, keys) in [
            (
                ComponentType::Vertex,
                topology.verts().map(ComponentKey::from).collect::<Vec<_>>(),
            ),
            (
                ComponentType::Edge,
                topology.edges().map(ComponentKey::from).collect(),
            ),
            (
                ComponentType::Face,
                topology.faces().map(ComponentKey::from).collect(),
            ),
        ] {
            for (index, &key) in keys.iter().enumerate() {
                assert_eq!(
                    overlay.component_key(OverlayComponent {
                        kind,
                        index: index as u32
                    }),
                    Some(key)
                );
            }

            assert_eq!(
                overlay.component_key(OverlayComponent {
                    kind,
                    index: keys.len() as u32
                }),
                None
            );
        }

        assert!(overlay.update(&mesh).is_none());
    }
}
