use glam::{DMat4, DVec3};
use mesh_core::{Mesh, MeshBuffers};

use crate::{MeshEditor, Modifiers, PickResult, PointerInput};

use super::{DragStartInput, MeshTransform, WorldRay};

#[test]
fn default_input_does_not_imply_a_pick_miss_or_spatial_data() {
    let input = PointerInput::default();

    assert_eq!(input.modifiers, Modifiers::default());
    assert_eq!(input.pick, PickResult::NotPerformed);
    assert_ne!(input.pick, PickResult::Miss);
    assert_eq!(input.ray, None);
    assert_eq!(input.drag_start, None);
}

#[test]
fn selection_input_carries_a_hit_without_spatial_data() {
    let mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![
            DVec3::ZERO.as_vec3(),
            DVec3::X.as_vec3(),
            DVec3::Y.as_vec3(),
        ],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2],
        face_vertex_counts: None,
    })
    .unwrap();
    let component = mesh.topology().verts().next().unwrap().into();
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(mesh);
    let input = PointerInput {
        modifiers: Modifiers {
            shift: true,
            ..Default::default()
        },
        pick: PickResult::Hit { mesh, component },
        ..Default::default()
    };

    assert!(input.modifiers.shift);
    assert_eq!(input.pick, PickResult::Hit { mesh, component });
    assert_eq!(input.ray, None);
    assert_eq!(input.drag_start, None);
}

#[test]
fn drag_input_borrows_transforms_and_followup_input_omits_them() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(Mesh::new());
    let offset = DVec3::new(6_378_137.125, 0.0, 0.0);
    let mesh_transforms = [MeshTransform {
        mesh,
        local_to_world: DMat4::from_translation(offset),
    }];
    let ray = WorldRay {
        origin: offset + DVec3::Z,
        direction: DVec3::NEG_Z,
    };
    let start = PointerInput {
        ray: Some(ray),
        drag_start: Some(DragStartInput {
            view_direction: DVec3::NEG_Z,
            mesh_transforms: &mesh_transforms,
        }),
        ..Default::default()
    };
    let copied = start;
    let spatial = copied.drag_start.unwrap();

    assert_eq!(copied, start);
    assert_eq!(spatial.mesh_transforms.as_ptr(), mesh_transforms.as_ptr());
    assert_eq!(spatial.mesh_transforms[0].local_to_world.w_axis.x, offset.x);
    assert_eq!(copied.ray.unwrap().origin.x, offset.x);

    let moved = PointerInput {
        ray: Some(WorldRay {
            origin: ray.origin + DVec3::X,
            ..ray
        }),
        ..Default::default()
    };

    assert_eq!(moved.drag_start, None);
    assert_eq!(moved.pick, PickResult::NotPerformed);
    assert_eq!(moved.ray.unwrap().origin, ray.origin + DVec3::X);
}
