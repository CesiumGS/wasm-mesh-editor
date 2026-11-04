use mesh::ComponentMask;
use slotmap::Key;

use super::*;

#[test]
fn empty_context_has_no_mesh_access() {
    let mut meshes = SlotMap::with_key();
    let mut active_meshes = Vec::new();
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::NONE);

    assert_eq!(context.mode(), EditMode::NONE);
    assert!(context.active_meshes().is_empty());
    assert!(!context.is_mesh_active(MeshKey::null()));
    assert!(context.mesh(MeshKey::null()).is_none());
    assert!(context.mesh_mut(MeshKey::null()).is_none());
    assert_eq!(context.active_meshes_mut().count(), 0);
}

#[test]
fn context_exposes_mode_and_borrowed_active_meshes() {
    let mut meshes = SlotMap::<MeshKey, Mesh>::with_key();
    let first = meshes.insert(Mesh::new());
    let second = meshes.insert(Mesh::new());

    meshes[first]
        .selection_mut()
        .set_level(ComponentMask::VERTEX);
    meshes[second]
        .selection_mut()
        .set_level(ComponentMask::EDGE);

    let mut active_meshes = vec![second, first];
    let active_meshes_ptr = active_meshes.as_ptr();
    let mode = EditMode {
        renderable_components: ComponentMask::FACE,
        selection_level: ComponentMask::VERTEX.union(ComponentMask::EDGE),
    };
    let context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

    assert_eq!(context.mode(), mode);
    assert_eq!(context.active_meshes(), &[second, first]);
    assert!(std::ptr::eq(
        context.active_meshes().as_ptr(),
        active_meshes_ptr
    ));
    assert_eq!(
        context.mesh(first).unwrap().selection().level(),
        ComponentMask::VERTEX
    );
    assert_eq!(
        context.mesh(second).unwrap().selection().level(),
        ComponentMask::EDGE
    );
}

#[test]
fn inactive_and_missing_meshes_are_inaccessible() {
    let mut meshes = SlotMap::with_key();
    let active = meshes.insert(Mesh::new());
    let inactive = meshes.insert(Mesh::new());
    let removed = meshes.insert(Mesh::new());

    meshes.remove(removed).unwrap();

    let replacement = meshes.insert(Mesh::new());
    let mut active_meshes = vec![active, removed, MeshKey::null()];
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);

    assert!(context.mesh(active).is_some());
    assert!(context.mesh_mut(active).is_some());

    for key in [inactive, removed, replacement, MeshKey::null()] {
        assert!(context.mesh(key).is_none());
        assert!(context.mesh_mut(key).is_none());
    }

    assert_eq!(context.active_meshes_mut().count(), 1);
}

#[test]
fn active_meshes_mut_edits_only_active_meshes() {
    let mut meshes = SlotMap::<MeshKey, Mesh>::with_key();
    let first = meshes.insert(Mesh::new());
    let second = meshes.insert(Mesh::new());
    let inactive = meshes.insert(Mesh::new());

    for key in [first, second, inactive] {
        meshes[key].selection_mut().set_level(ComponentMask::FACE);
    }

    let mut active_meshes = vec![second, first];

    {
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::FACE);

        for mesh in context.active_meshes_mut() {
            mesh.selection_mut().set_level(ComponentMask::EDGE);
        }
    }

    for key in [first, second] {
        assert_eq!(meshes[key].selection().level(), ComponentMask::EDGE);
    }

    assert_eq!(meshes[inactive].selection().level(), ComponentMask::FACE);
}

#[test]
fn activation_applies_mode_and_appends_once() {
    let mut meshes = SlotMap::<MeshKey, Mesh>::with_key();
    let first = meshes.insert(Mesh::new());
    let second = meshes.insert(Mesh::new());

    meshes[second]
        .selection_mut()
        .set_level(ComponentMask::FACE);

    let mut active_meshes = vec![first];
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);

    context.set_mesh_active(second).unwrap();
    context
        .mesh_mut(second)
        .unwrap()
        .selection_mut()
        .set_level(ComponentMask::EDGE);
    context.set_mesh_active(second).unwrap();

    assert_eq!(context.active_meshes(), &[first, second]);
    assert!(context.is_mesh_active(second));
    assert_eq!(
        context.mesh(second).unwrap().selection().level(),
        ComponentMask::EDGE
    );
}

#[test]
fn retain_active_meshes_preserves_order_and_deactivated_selection() {
    let mut meshes = SlotMap::<MeshKey, Mesh>::with_key();
    let first = meshes.insert(Mesh::new());
    let middle = meshes.insert(Mesh::new());
    let last = meshes.insert(Mesh::new());

    meshes[middle]
        .selection_mut()
        .set_level(ComponentMask::VERTEX);

    let mut active_meshes = vec![last, middle, first];

    {
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);

        context.retain_active_meshes(|&key| key != middle);

        assert_eq!(context.active_meshes(), &[last, first]);
        assert!(!context.is_mesh_active(middle));
        assert!(context.mesh(middle).is_none());
    }

    assert_eq!(active_meshes, [last, first]);
    assert_eq!(meshes[middle].selection().level(), ComponentMask::VERTEX);
}

#[test]
fn activation_rejects_unknown_keys() {
    let mut meshes = SlotMap::<MeshKey, Mesh>::with_key();
    let removed = meshes.insert(Mesh::new());

    meshes.remove(removed).unwrap();

    let mut active_meshes = Vec::new();
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);

    for key in [removed, MeshKey::null()] {
        assert_eq!(context.set_mesh_active(key), Err(MeshNotFound { key }));
    }

    assert!(context.active_meshes().is_empty());
}
