use slotmap::Key;

use super::*;

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

    for editor in [MeshEditor::new(), MeshEditor::default()] {
        for key in keys {
            assert!(!editor.has_mesh(key));
            assert!(editor.mesh(key).is_none());
            assert!(!editor.is_mesh_active(key));
        }
    }
}
