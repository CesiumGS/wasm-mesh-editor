use std::sync::mpsc::TryRecvError;

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
            assert_eq!(editor.set_mesh_active(key), Err(MeshNotFound { key }));
            assert_eq!(editor.set_mesh_inactive(key), Err(MeshNotFound { key }));
        }

        assert_empty_editor(&editor);
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

    for key in [first, middle, last] {
        editor.set_mesh_active(key).unwrap();
    }

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
    editor.set_mesh_active(replacement).unwrap();

    for key in [MeshKey::null(), removed] {
        assert!(!editor.has_mesh(key));
        assert!(editor.mesh(key).is_none());
        assert!(!editor.is_mesh_active(key));
        assert!(editor.overlay(key).is_none());
        assert!(editor.remove_mesh(key).is_none());
        assert_eq!(editor.set_mesh_active(key), Err(MeshNotFound { key }));
        assert_eq!(editor.set_mesh_inactive(key), Err(MeshNotFound { key }));
    }

    assert_eq!(editor.mesh_keys(), &[replacement]);
    assert_eq!(editor.active_meshes(), &[replacement]);
    assert!(editor.has_mesh(replacement));
    assert!(editor.mesh(replacement).is_some());
    assert!(editor.overlay(replacement).is_some());
}

#[test]
fn activation_applies_current_mode_and_repeated_calls_are_noops() {
    let mut editor = MeshEditor::new();
    let mut mesh = triangle_mesh();

    mesh.selection_mut().set_level(ComponentMask::FACE);
    editor.set_mode(EditMode::EDGE);

    let first = editor.add_mesh(mesh);
    let second = editor.add_mesh(triangle_mesh());

    assert_eq!(
        editor.mesh(first).unwrap().selection().level(),
        ComponentMask::FACE
    );
    assert_eq!(editor.set_mesh_active(first), Ok(()));
    assert_eq!(editor.set_mesh_active(second), Ok(()));

    let changes = editor.meshes[first].subscribe();

    assert_eq!(editor.set_mesh_active(first), Ok(()));
    editor.set_mode(EditMode::EDGE);

    assert_eq!(editor.active_meshes(), &[first, second]);

    for key in [first, second] {
        assert!(editor.is_mesh_active(key));
        assert_eq!(
            editor.mesh(key).unwrap().selection().level(),
            ComponentMask::EDGE
        );
    }

    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn deactivation_preserves_selection_and_reactivation_applies_latest_mode() {
    let mut editor = MeshEditor::new();
    let mut mesh = triangle_mesh();
    let vertex = mesh.topology().verts().next().unwrap();

    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    mesh.selection_mut().select(&[vertex]);

    let changes = mesh.subscribe();
    let key = editor.add_mesh(mesh);

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(key).unwrap();

    assert_eq!(editor.set_mesh_inactive(key), Ok(()));
    assert_eq!(editor.set_mesh_inactive(key), Ok(()));
    editor.set_mode(EditMode::FACE);

    assert!(editor.active_meshes().is_empty());
    assert!(!editor.is_mesh_active(key));
    assert!(editor.overlay(key).is_some());

    let selection = editor.mesh(key).unwrap().selection();

    assert_eq!(selection.level(), ComponentMask::VERTEX);
    assert_eq!(selection.len(), 1);
    assert!(selection.contains(vertex));
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));

    editor.set_mesh_active(key).unwrap();

    assert_eq!(editor.active_meshes(), &[key]);
    assert_eq!(
        editor.mesh(key).unwrap().selection().level(),
        ComponentMask::FACE
    );
    assert!(editor.mesh(key).unwrap().selection().is_empty());
}

#[test]
fn mode_changes_apply_only_to_active_meshes() {
    let mut editor = MeshEditor::new();
    let [first, second, inactive] =
        [triangle_mesh(), triangle_mesh(), triangle_mesh()].map(|mut mesh| {
            let vertex = mesh.topology().verts().next().unwrap();

            mesh.selection_mut().set_level(ComponentMask::VERTEX);
            mesh.selection_mut().select(&[vertex]);

            editor.add_mesh(mesh)
        });

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(first).unwrap();
    editor.set_mesh_active(second).unwrap();

    for mode in [EditMode::EDGE, EditMode::NONE] {
        editor.set_mode(mode);

        assert_eq!(editor.mode(), mode);
        assert_eq!(editor.active_meshes(), &[first, second]);

        for key in [first, second] {
            let selection = editor.mesh(key).unwrap().selection();

            assert_eq!(selection.level(), mode.selection_level);
            assert!(selection.is_empty());
        }

        let selection = editor.mesh(inactive).unwrap().selection();

        assert_eq!(selection.level(), ComponentMask::VERTEX);
        assert_eq!(selection.len(), 1);
    }
}

#[test]
fn deactivation_preserves_other_active_meshes_and_key_order() {
    let mut editor = MeshEditor::new();
    let first = editor.add_mesh(Mesh::new());
    let middle = editor.add_mesh(Mesh::new());
    let last = editor.add_mesh(Mesh::new());

    for key in [first, middle, last] {
        editor.set_mesh_active(key).unwrap();
    }

    assert_eq!(editor.set_mesh_inactive(middle), Ok(()));
    assert_eq!(editor.active_meshes(), &[first, last]);
    assert_eq!(editor.mesh_keys(), &[first, middle, last]);
    assert!(!editor.is_mesh_active(middle));

    editor.set_mesh_active(middle).unwrap();

    assert_eq!(editor.active_meshes(), &[first, last, middle]);
}

#[test]
fn rendering_policy_is_exposed_without_changing_selection() {
    let mut editor = MeshEditor::new();
    let mode = EditMode {
        renderable_components: ComponentMask::empty(),
        selection_level: ComponentMask::VERTEX.union(ComponentMask::EDGE),
    };

    editor.set_mode(mode);

    assert_eq!(editor.mode(), mode);
    assert!(editor.active_meshes().is_empty());

    let mut mesh = triangle_mesh();
    let vertex = mesh.topology().verts().next().unwrap();

    mesh.selection_mut().set_level(mode.selection_level);
    mesh.selection_mut().select(&[vertex]);

    let changes = mesh.subscribe();
    let key = editor.add_mesh(mesh);

    editor.set_mesh_active(key).unwrap();

    let mode = EditMode {
        renderable_components: ComponentMask::FACE,
        ..mode
    };

    editor.set_mode(mode);

    assert_eq!(editor.mode(), mode);

    let selection = editor.mesh(key).unwrap().selection();

    assert_eq!(selection.level(), mode.selection_level);
    assert_eq!(selection.len(), 1);
    assert!(selection.contains(vertex));
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn mesh_not_found_reports_the_missing_key() {
    let mut editor = MeshEditor::new();
    let key = MeshKey::null();
    let error: crate::MeshNotFound = editor.set_mesh_active(key).unwrap_err();

    assert_eq!(error.key, key);

    let error: &dyn std::error::Error = &error;

    assert_eq!(error.to_string(), format!("mesh {key:?} is not registered"));
}
