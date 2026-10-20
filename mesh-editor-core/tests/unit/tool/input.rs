use glam::{DMat4, DVec3};
use mesh_core::{ComponentType, Mesh};

use crate::{
    MeshEditor, MeshTransform, Modifiers, OverlayComponent, PickResult, PointerInput, WorldRay,
};

use super::SceneInput;

#[test]
fn default_input_does_not_imply_a_pick_miss_or_spatial_data() {
    let input: PointerInput = PointerInput::default();

    assert_eq!(input.modifiers, Modifiers::default());
    assert_eq!(input.pick, PickResult::NotPerformed);
    assert_ne!(input.pick, PickResult::Miss);
    assert_eq!(input.ray, None);
    assert_eq!(input.scene, None);
}

#[test]
fn selection_input_carries_a_hit_without_spatial_data() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(Mesh::new());
    let pick = PickResult::Hit {
        mesh,
        component: OverlayComponent {
            kind: ComponentType::Vertex,
            index: 0,
        },
    };
    let input = PointerInput {
        modifiers: Modifiers {
            shift: true,
            ..Default::default()
        },
        pick,
        ..Default::default()
    };

    assert!(input.modifiers.shift);
    assert_eq!(input.pick, pick);
    assert_eq!(input.ray, None);
    assert_eq!(input.scene, None);
}

#[test]
fn pointer_input_borrows_optional_scene_context() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(Mesh::new());
    let offset = DVec3::new(6_378_137.125, 0.0, 0.0);
    let mesh_transforms = [MeshTransform::new(mesh, DMat4::from_translation(offset)).unwrap()];
    let ray = WorldRay {
        origin: offset + DVec3::Z,
        direction: DVec3::NEG_Z,
    };
    let start: PointerInput = PointerInput {
        ray: Some(ray),
        scene: Some(SceneInput {
            view_direction: DVec3::NEG_Z,
            mesh_transforms: &mesh_transforms,
        }),
        ..Default::default()
    };
    let copied = start;
    let spatial = copied.scene.unwrap();

    assert_eq!(copied, start);
    assert_eq!(spatial.mesh_transforms.as_ptr(), mesh_transforms.as_ptr());
    assert_eq!(
        spatial.mesh_transforms[0].local_to_world().w_axis.x,
        offset.x
    );
    assert_eq!(copied.ray.unwrap().origin.x, offset.x);

    let moved: PointerInput = PointerInput {
        ray: Some(WorldRay {
            origin: ray.origin + DVec3::X,
            ..ray
        }),
        ..Default::default()
    };

    assert_eq!(moved.scene, None);
    assert_eq!(moved.pick, PickResult::NotPerformed);
    assert_eq!(moved.ray.unwrap().origin, ray.origin + DVec3::X);

    let moved_with_scene = PointerInput {
        scene: Some(spatial),
        ..moved
    };
    assert_eq!(moved_with_scene.scene, start.scene);
    assert_eq!(
        moved_with_scene.scene.unwrap().mesh_transforms.as_ptr(),
        mesh_transforms.as_ptr()
    );
}
