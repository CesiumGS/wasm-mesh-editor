use mesh::ComponentMask;
use slotmap::Key;

use super::*;

#[test]
fn empty_context_has_no_mesh_access() {
    let mut meshes = SlotMap::with_key();
    let mut context = ToolContext::new(&mut meshes, &[], EditMode::NONE);

    assert_eq!(context.mode(), EditMode::NONE);
    assert!(context.active_meshes().is_empty());
    assert!(context.mesh(MeshKey::null()).is_none());
    assert!(context.mesh_mut(MeshKey::null()).is_none());
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

    let active_meshes = [second, first];
    let mode = EditMode {
        renderable_components: ComponentMask::FACE,
        selection_level: ComponentMask::VERTEX.union(ComponentMask::EDGE),
    };
    let context = ToolContext::new(&mut meshes, &active_meshes, mode);

    assert_eq!(context.mode(), mode);
    assert_eq!(context.active_meshes(), &[second, first]);
    assert!(std::ptr::eq(context.active_meshes(), &active_meshes[..]));
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
    let active_meshes = [active, removed, MeshKey::null()];
    let mut context = ToolContext::new(&mut meshes, &active_meshes, EditMode::VERTEX);

    assert!(context.mesh(active).is_some());
    assert!(context.mesh_mut(active).is_some());

    for key in [inactive, removed, replacement, MeshKey::null()] {
        assert!(context.mesh(key).is_none());
        assert!(context.mesh_mut(key).is_none());
    }
}

#[test]
fn active_keys_can_be_iterated_while_editing_original_meshes() {
    let mut meshes = SlotMap::<MeshKey, Mesh>::with_key();
    let first = meshes.insert(Mesh::new());
    let second = meshes.insert(Mesh::new());
    let inactive = meshes.insert(Mesh::new());

    for key in [first, second, inactive] {
        meshes[key].selection_mut().set_level(ComponentMask::FACE);
    }

    let active_meshes = [second, first];

    {
        let mut context = ToolContext::new(&mut meshes, &active_meshes, EditMode::FACE);

        for &key in context.active_meshes() {
            context
                .mesh_mut(key)
                .unwrap()
                .selection_mut()
                .set_level(ComponentMask::EDGE);

            assert_eq!(
                context.mesh(key).unwrap().selection().level(),
                ComponentMask::EDGE
            );
        }
    }

    for key in [first, second] {
        assert_eq!(meshes[key].selection().level(), ComponentMask::EDGE);
    }

    assert_eq!(meshes[inactive].selection().level(), ComponentMask::FACE);
}
