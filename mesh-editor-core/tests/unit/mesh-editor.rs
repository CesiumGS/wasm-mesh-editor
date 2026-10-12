use mesh_core::{ComponentMask, MeshBuffers};
use slotmap::Key;

use super::*;

fn triangle_mesh() -> Mesh {
    Mesh::from_buffers(MeshBuffers {
        positions: vec![
            [0.0, 0.0, 0.0].into(),
            [1.0, 0.0, 0.0].into(),
            [0.0, 1.0, 0.0].into(),
        ],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2],
        face_vertex_counts: None,
    })
    .unwrap()
}

fn assert_empty_editor(editor: &MeshEditor) {
    assert_eq!(editor.mode(), EditMode::NONE);
    assert!(editor.mesh_keys().is_empty());
    assert!(editor.active_meshes().is_empty());
    assert_eq!(editor.active_tool(), None);
}

#[test]
fn new_creates_empty_editor() {
    let editor = MeshEditor::new();

    assert_empty_editor(&editor);
}

#[test]
fn default_creates_empty_editor() {
    let editor = MeshEditor::default();

    assert_empty_editor(&editor);
}

#[test]
fn empty_editor_rejects_unknown_mesh_keys() {
    let mut meshes = SlotMap::<MeshKey, ()>::with_key();
    let keys = [MeshKey::null(), meshes.insert(())];

    for mut editor in [MeshEditor::new(), MeshEditor::default()] {
        for key in keys {
            assert!(!editor.has_mesh(key));
            assert!(editor.mesh(key).is_none());
            assert!(!editor.is_mesh_active(key));
            assert!(editor.overlay(key).is_none());
            assert!(editor.remove_mesh(key).is_none());
        }
    }
}

#[test]
fn add_mesh_registers_inactive_mesh_and_builds_overlay() {
    let mut editor = MeshEditor::new();
    let mut mesh = triangle_mesh();
    let vertex = mesh.topology().verts().next().unwrap();

    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    mesh.selection_mut().select(&[vertex]);

    let key = editor.add_mesh(mesh);

    assert!(editor.has_mesh(key));
    assert_eq!(editor.mesh_keys(), &[key]);
    assert!(editor.active_meshes().is_empty());
    assert!(!editor.is_mesh_active(key));
    assert!(editor.overlay(key).is_some());
    assert_eq!(editor.mode(), EditMode::NONE);

    let mesh = editor.mesh(key).unwrap();
    assert_eq!(mesh.topology().vert_count(), 3);
    assert_eq!(mesh.selection().level(), ComponentMask::VERTEX);
    assert!(mesh.selection().contains(vertex));

    let buffers = editor.overlays[key].update(&editor.meshes[key]).unwrap();

    assert!(buffers.full_refresh);
    assert_eq!(
        buffers.positions.data,
        vec![
            [0.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
        ]
    );
    assert_eq!(buffers.edges.data.len(), 3);
    assert_eq!(buffers.triangles.data.len(), 1);
    assert_eq!(buffers.selection.verts.data, vec![255, 0, 0]);
}

#[test]
fn remove_mesh_returns_original_mesh_and_removes_overlay() {
    let mut editor = MeshEditor::new();
    let mut mesh = triangle_mesh();
    let vertices: Vec<_> = mesh.topology().verts().collect();

    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    mesh.selection_mut().select(&[vertices[1]]);

    let key = editor.add_mesh(mesh);
    let mesh = editor.remove_mesh(key).unwrap();

    assert_empty_editor(&editor);
    assert!(!editor.has_mesh(key));
    assert!(editor.mesh(key).is_none());
    assert!(editor.overlay(key).is_none());

    assert_eq!(mesh.topology().verts().collect::<Vec<_>>(), vertices);
    assert_eq!(mesh.topology().edge_count(), 3);
    assert_eq!(mesh.topology().face_count(), 1);
    assert_eq!(
        mesh.attributes().positions[vertices[1]].to_array(),
        [1.0, 0.0, 0.0]
    );
    assert_eq!(mesh.selection().level(), ComponentMask::VERTEX);
    assert!(mesh.selection().contains(vertices[1]));
}

#[test]
fn remove_mesh_preserves_remaining_meshes_and_active_keys() {
    let mut editor = MeshEditor::new();
    let first = editor.add_mesh(Mesh::new());
    let middle = editor.add_mesh(Mesh::new());
    let last = editor.add_mesh(Mesh::new());

    assert_eq!(editor.mesh_keys(), &[first, middle, last]);

    editor.active_meshes.extend([first, middle, last]);
    assert!(editor.remove_mesh(middle).is_some());

    assert_eq!(editor.mesh_keys(), &[first, last]);
    assert_eq!(editor.active_meshes(), &[first, last]);
    assert!(!editor.is_mesh_active(middle));
    assert!(editor.overlay(middle).is_none());

    for key in [first, last] {
        assert!(editor.has_mesh(key));
        assert!(editor.mesh(key).is_some());
        assert!(editor.overlay(key).is_some());
    }
}

#[test]
fn removed_mesh_key_cannot_access_or_remove_replacement() {
    let mut editor = MeshEditor::new();
    let removed = editor.add_mesh(Mesh::new());

    assert!(editor.remove_mesh(removed).is_some());

    let replacement = editor.add_mesh(Mesh::new());

    assert_ne!(removed, replacement);

    for key in [MeshKey::null(), removed] {
        assert!(!editor.has_mesh(key));
        assert!(editor.mesh(key).is_none());
        assert!(!editor.is_mesh_active(key));
        assert!(editor.overlay(key).is_none());
        assert!(editor.remove_mesh(key).is_none());
    }

    assert_eq!(editor.mesh_keys(), &[replacement]);
    assert!(editor.has_mesh(replacement));
    assert!(editor.mesh(replacement).is_some());
    assert!(editor.overlay(replacement).is_some());
}
