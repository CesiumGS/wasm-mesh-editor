use event_emitter::EventEmitter;
use glam::Vec3;
use mesh::{ComponentMask, HideOptions, MeshBuffers, TopologyChange};
use std::sync::mpsc::TryRecvError;

use super::*;

#[test]
fn flag_buffers_index_independent_component_types() {
    let mut buffers = ComponentFlagBuffers::default();
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
        assert!(reader[kind].dirty_range.is_none());
    }
}

#[test]
fn extend_dirty_range_merges_into_one_covering_range() {
    let mut buffer = OverlayBuffer::new(vec![0u8; 10]);

    buffer.extend_dirty_range(3..3);
    assert_eq!(buffer.dirty_range, None);

    buffer.extend_dirty_range(5..6);
    assert_eq!(buffer.dirty_range, Some(5..6));

    buffer.extend_dirty_range(5..6);
    assert_eq!(buffer.dirty_range, Some(5..6));

    buffer.extend_dirty_range(8..9);
    assert_eq!(buffer.dirty_range, Some(5..9));

    buffer.extend_dirty_range(2..3);
    assert_eq!(buffer.dirty_range, Some(2..9));

    buffer.extend_dirty_range(6..7);
    assert_eq!(buffer.dirty_range, Some(2..9));

    buffer.extend_dirty_range(0..0);
    assert_eq!(buffer.dirty_range, Some(2..9));
}

#[test]
fn dirty_range_builder_covers_included_indices() {
    assert!(DirtyRangeBuilder::new().build().is_empty());

    let mut builder = DirtyRangeBuilder::new();
    for index in [7, 2, 4, 7] {
        builder.include(index);
    }

    assert_eq!(builder.build(), 2..8);
}

#[test]
fn update_merges_sparse_changes_into_covering_range() {
    let mut mesh = mixed_face_mesh();
    let verts: Vec<_> = mesh.topology().verts().collect();
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.selection_mut().select(&[verts[4], verts[1]]);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert_eq!(buffers.flags.verts.dirty_range, Some(1..5));
    assert!(buffers.positions.dirty_range.is_none());

    mesh.translate_selected(Vec3::X);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert_eq!(buffers.positions.dirty_range, Some(1..5));
    assert!(buffers.flags.verts.dirty_range.is_none());
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
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn new_packs_buffers_with_keys_in_buffer_order() {
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

    for (index, &key) in buffers.component_keys.verts.iter().enumerate() {
        assert_eq!(overlay.vertex_key_to_buffer_index[key], index as u32);
        assert_eq!(
            buffers.positions.data[index],
            mesh.attributes().positions()[key].extend(0.0).to_array()
        );
    }

    for (index, &key) in buffers.component_keys.edges.iter().enumerate() {
        assert_eq!(overlay.edge_key_to_buffer_index[key], index as u32);
        assert_eq!(
            buffers.edges.data[index].map(|vertex| buffers.component_keys.verts[vertex as usize]),
            mesh.edge(key).unwrap().verts()
        );
    }

    for (index, &key) in buffers.component_keys.faces.iter().enumerate() {
        assert_eq!(overlay.face_key_to_buffer_index[key], index as u32);

        let triangles: Vec<_> = buffers
            .triangles
            .data
            .iter()
            .filter(|triangle| triangle[3] == index as u32)
            .map(|triangle| {
                [triangle[0], triangle[1], triangle[2]]
                    .map(|vertex| buffers.component_keys.verts[vertex as usize])
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
        assert_eq!(buffers.flags[kind].data, vec![0; count]);
        assert!(buffers.flags[kind].dirty_range.is_none());
    }

    assert!(overlay.full_refresh());
    assert!(matches!(overlay.pending_upload, PendingChanges::Full));
    assert!(buffers.positions.dirty_range.is_none());
    assert!(buffers.edges.dirty_range.is_none());
    assert!(buffers.triangles.dirty_range.is_none());
}

#[test]
fn face_keys_follow_triangle_face_indices_not_triangle_indices() {
    let mut mesh = mixed_face_mesh();
    let faces: Vec<_> = mesh.topology().faces().collect();
    let overlay = TopologyOverlay::new(&mut mesh);
    let buffers = overlay.buffers();
    let picked_faces: Vec<_> = buffers
        .triangles
        .data
        .iter()
        .map(|triangle| buffers.component_keys.faces[triangle[3] as usize])
        .collect();

    assert_eq!(picked_faces, vec![faces[0], faces[0], faces[1]]);
    assert_eq!(buffers.component_keys.faces, faces);
}

#[test]
fn new_seeds_selection_and_subscribes_to_future_changes() {
    let mut mesh = mixed_face_mesh();
    let face = mesh.topology().faces().nth(1).unwrap();
    mesh.set_interactable_components(ComponentMask::FACE);
    mesh.selection_mut().select(&[face]);

    let overlay = TopologyOverlay::new(&mut mesh);

    assert_eq!(
        overlay.buffers.flags[ComponentType::Vertex].data,
        vec![1, 1, 0, 0, 1, 0]
    );
    assert_eq!(overlay.buffers.flags[ComponentType::Face].data, vec![0, 1]);

    for (index, &key) in overlay.buffers.component_keys.edges.iter().enumerate() {
        assert_eq!(
            overlay.buffers.flags[ComponentType::Edge].data[index],
            if mesh.selection().contains(key) { 1 } else { 0 }
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
    assert!(overlay.buffers.component_keys.verts.is_empty());
    assert!(overlay.buffers.component_keys.edges.is_empty());
    assert!(overlay.buffers.component_keys.faces.is_empty());

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(overlay.buffers.flags[kind].data.is_empty());
    }

    assert!(overlay.full_refresh());
    assert!(matches!(overlay.pending_upload, PendingChanges::Full));
}

#[test]
fn update_delivers_initial_upload_once() {
    for mut mesh in [Mesh::new(), mixed_face_mesh()] {
        let mut overlay = TopologyOverlay::new(&mut mesh);

        assert!(overlay.full_refresh());
        assert_eq!(
            overlay.buffers().positions.data.len(),
            mesh.topology().vert_count()
        );

        let update = overlay.update(&mesh).unwrap();
        let buffers = update.buffers;
        assert!(update.full_refresh);
        assert!(buffers.positions.dirty_range.is_none());
        let returned = std::ptr::from_ref(buffers);

        assert!(std::ptr::eq(returned, overlay.buffers()));
        assert!(std::ptr::eq(returned, overlay.buffers()));
        assert!(matches!(overlay.pending_upload, PendingChanges::None));
        assert!(overlay.update(&mesh).is_none());
        assert!(!overlay.full_refresh());
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

        assert!(!overlay.full_refresh());
        assert_eq!(buffers.flags.verts.data[5], 1);
        assert_eq!(buffers.flags.verts.dirty_range, Some(5..6));
        assert!(buffers.positions.dirty_range.is_none());
    }

    overlay.update(&mesh).unwrap();

    assert_eq!(overlay.buffers().flags.verts.data[5], 0);
    assert!(overlay.update(&mesh).is_none());
    assert_eq!(overlay.buffers().flags.verts.data[5], 0);
    assert!(overlay.buffers().flags.verts.dirty_range.is_none());
    assert!(!overlay.full_refresh());
}

#[test]
fn update_includes_changes_queued_before_initial_upload() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);

    mesh.selection_mut().select(&[vertex]);
    mesh.translate_selected(Vec3::X);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(update.full_refresh);
    assert_eq!(buffers.positions.data[5], [3.0, 3.0, 4.0, 0.0]);
    assert_eq!(
        buffers.flags[ComponentType::Vertex].data,
        vec![0, 0, 0, 0, 0, 1]
    );
    assert!(buffers.positions.dirty_range.is_none());
    assert!(buffers.flags[ComponentType::Vertex].dirty_range.is_none());
    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_reports_selection_changes_without_detail() {
    let mut mesh = mixed_face_mesh();
    let verts: Vec<_> = mesh.topology().verts().collect();
    let mut overlay = TopologyOverlay::new(&mut mesh);

    // Queued before the initial upload, which is still a selection change.
    mesh.selection_mut().select(&[verts[0]]);
    assert!(overlay.update(&mesh).unwrap().selection_changed);

    // Several edits in one frame report once, even when they cancel out.
    mesh.selection_mut().select(&[verts[1]]);
    mesh.selection_mut().deselect(&[verts[1]]);
    assert!(overlay.update(&mesh).unwrap().selection_changed);

    // Only the update that applied the change reports it.
    assert!(overlay.update(&mesh).is_none());

    // No-op selection edits are not changes.
    mesh.selection_mut().select(&[verts[0]]);
    mesh.selection_mut().deselect(&[verts[1]]);
    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_does_not_report_selection_changed_for_other_changes() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);

    mesh.selection_mut().select(&[vertex]);
    overlay.update(&mesh).unwrap();

    mesh.translate_selected(Vec3::X);

    let update = overlay.update(&mesh).unwrap();
    assert!(!update.selection_changed);
    assert!(update.buffers.positions.dirty_range.is_some());

    mesh.hidden_components_mut().hide(
        &[vertex],
        HideOptions {
            extend_to_connected_faces: false,
        },
    );

    let update = overlay.update(&mesh).unwrap();
    assert!(!update.selection_changed);
    assert!(update.buffers.flags.verts.dirty_range.is_some());
}

#[test]
fn update_returns_partial_uploads_and_clears_previous_ranges() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.selection_mut().select(&[vertex]);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);
    assert_eq!(buffers.flags[ComponentType::Vertex].data[5], 1);
    assert_eq!(buffers.flags[ComponentType::Vertex].dirty_range, Some(5..6));
    assert!(buffers.positions.dirty_range.is_none());

    mesh.translate_selected(Vec3::X);
    mesh.translate_selected(Vec3::Y);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);
    assert_eq!(buffers.positions.data[5], [3.0, 4.0, 4.0, 0.0]);
    assert_eq!(buffers.positions.dirty_range, Some(5..6));
    assert!(buffers.flags[ComponentType::Vertex].dirty_range.is_none());

    mesh.selection_mut().clear();

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);
    assert_eq!(buffers.flags[ComponentType::Vertex].data[5], 0);
    assert!(buffers.positions.dirty_range.is_none());
    assert!(overlay.update(&mesh).is_none());
    assert!(
        overlay.buffers.flags[ComponentType::Vertex]
            .dirty_range
            .is_none()
    );
}

#[test]
fn update_replays_queued_selection_changes_in_order() {
    let mut mesh = mixed_face_mesh();
    let faces: Vec<_> = mesh.topology().faces().collect();
    mesh.set_interactable_components(ComponentMask::FACE);
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.selection_mut().select(&[faces[1]]);
    mesh.selection_mut().clear();
    mesh.selection_mut().select(&[faces[0]]);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);
    assert_eq!(
        buffers.flags[ComponentType::Vertex].data,
        vec![1, 1, 1, 1, 0, 0]
    );
    assert_eq!(buffers.flags[ComponentType::Face].data, vec![1, 0]);

    for (index, key) in mesh.topology().edges().enumerate() {
        assert_eq!(
            buffers.flags[ComponentType::Edge].data[index],
            if mesh.selection().contains(key) { 1 } else { 0 }
        );
    }

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(buffers.flags[kind].dirty_range.is_some());
    }

    assert!(overlay.update(&mesh).is_none());

    mesh.selection_mut().set(&[faces[1]]);

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert_eq!(
        buffers.flags[ComponentType::Vertex].data,
        vec![1, 1, 0, 0, 1, 0]
    );
    assert_eq!(buffers.flags[ComponentType::Face].data, vec![0, 1]);

    mesh.selection_mut().clear();
    mesh.selection_mut().select(&[faces[0]]);
    mesh.selection_mut().clear();

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(buffers.flags[kind].data.iter().all(|&value| value == 0));
        assert!(buffers.flags[kind].dirty_range.is_some());
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
    changes.emit(MeshChange::Selection(ComponentSetChange::default()));
    assert!(overlay.update(&mesh).is_none());

    let mut change = ComponentSetChange::default();
    change.added.verts.push(VertKey::null());
    change.removed.edges.push(EdgeKey::null());
    change.added.faces.push(FaceKey::null());
    changes.emit(MeshChange::Selection(change));

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert!(buffers.flags[kind].data.is_empty());
        assert!(buffers.flags[kind].dirty_range.is_none());
    }

    assert!(overlay.update(&mesh).is_none());
}

#[test]
fn update_ignores_shading_changes() {
    let mut mesh = mixed_face_mesh();
    let face = mesh.topology().faces().next().unwrap();
    mesh.set_interactable_components(ComponentMask::FACE);
    mesh.selection_mut().select(&[face]);
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();

    mesh.shade_flat();
    mesh.shade_smooth();

    assert!(overlay.update(&mesh).is_none());
    assert!(matches!(overlay.pending_upload, PendingChanges::None));
}

#[test]
fn update_rebuilds_buffers_keys_and_maps_on_topology_events() {
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

        let mut selection_change = ComponentSetChange::default();
        selection_change.added.verts = mesh.topology().verts().collect();
        changes.emit(MeshChange::Selection(selection_change));

        let update = overlay.update(&mesh).unwrap();
        let buffers = update.buffers;
        let topology = mesh.topology();
        assert!(update.full_refresh);
        assert_eq!(buffers.positions.data.len(), topology.vert_count());
        assert_eq!(buffers.edges.data.len(), topology.edge_count());
        assert_eq!(
            buffers.triangles.data.len(),
            topology.loop_count() - 2 * topology.face_count()
        );
        assert!(buffers.positions.dirty_range.is_none());
        assert!(buffers.flags[ComponentType::Vertex].dirty_range.is_none());
        assert_eq!(
            buffers.component_keys.verts,
            topology.verts().collect::<Vec<_>>()
        );
        assert_eq!(
            buffers.component_keys.edges,
            topology.edges().collect::<Vec<_>>()
        );
        assert_eq!(
            buffers.component_keys.faces,
            topology.faces().collect::<Vec<_>>()
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

        assert!(overlay.update(&mesh).is_none());
    }
}

#[test]
fn new_seeds_hidden_and_selected_flags_together() {
    let mut mesh = mixed_face_mesh();
    let verts: Vec<_> = mesh.topology().verts().collect();
    mesh.selection_mut().select(&[verts[0], verts[5]]);
    mesh.hidden_components_mut().hide(
        &[verts[0], verts[1]],
        HideOptions {
            extend_to_connected_faces: true,
        },
    );

    let overlay = TopologyOverlay::new(&mut mesh);
    let selected = ComponentFlags::SELECTED.bits();
    let hidden = ComponentFlags::HIDDEN.bits();

    assert_eq!(
        overlay.buffers.flags.verts.data,
        vec![selected | hidden, hidden, 0, 0, 0, selected]
    );
    assert_eq!(overlay.buffers.flags.faces.data, vec![hidden, hidden]);
    assert!(
        overlay
            .buffers
            .flags
            .edges
            .data
            .iter()
            .all(|&flags| flags == 0)
    );
}

#[test]
fn hidden_and_selected_flags_change_independently() {
    let mut mesh = mixed_face_mesh();
    let vertex = mesh.topology().verts().last().unwrap();
    let face = mesh.topology().faces().next().unwrap();
    let mut overlay = TopologyOverlay::new(&mut mesh);
    overlay.update(&mesh).unwrap();
    let selected = ComponentFlags::SELECTED.bits();
    let hidden = ComponentFlags::HIDDEN.bits();

    mesh.selection_mut().select(&[vertex]);
    mesh.hidden_components_mut()
        .hide(&[vertex], HideOptions::default());

    let update = overlay.update(&mesh).unwrap();
    let buffers = update.buffers;
    assert!(!update.full_refresh);
    assert_eq!(buffers.flags.verts.data[5], selected | hidden);
    assert_eq!(buffers.flags.verts.dirty_range, Some(5..6));

    mesh.selection_mut().clear();

    let buffers = overlay.update(&mesh).unwrap().buffers;
    assert_eq!(buffers.flags.verts.data[5], hidden);

    mesh.set_interactable_components(ComponentMask::FACE);
    mesh.hidden_components_mut()
        .hide(&[face], HideOptions::default());
    mesh.hidden_components_mut().show_all();

    let buffers = overlay.update(&mesh).unwrap().buffers;
    assert_eq!(buffers.flags.verts.data[5], 0);
    assert_eq!(buffers.flags.faces.data, vec![0, 0]);
    assert_eq!(buffers.flags.faces.dirty_range, Some(0..1));
    assert!(buffers.flags.edges.dirty_range.is_none());
}
