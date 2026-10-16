use glam::{DMat4, Vec3};
use mesh_core::{Mesh, MeshBuffers};
use slotmap::SlotMap;

use crate::{MeshKey, MeshTransform, SceneInput, WorldRay};

use super::*;

fn selected_mesh(positions: &[Vec3]) -> Mesh {
    let mut mesh = Mesh::from_buffers(MeshBuffers {
        positions: positions.to_vec(),
        normals: None,
        uvs: None,
        indices: Vec::new(),
        face_vertex_counts: None,
    })
    .unwrap();
    let vertices: Vec<_> = mesh.topology().verts().collect();
    mesh.selection_mut().select(&vertices);

    mesh
}

fn pointer(position: DVec3) -> PointerInput<'static, ComponentKey> {
    PointerInput {
        ray: Some(WorldRay {
            origin: position,
            direction: -DVec3::Z,
        }),
        ..PointerInput::default()
    }
}

fn start_input(transforms: &[MeshTransform], origin: DVec3) -> PointerInput<'_, ComponentKey> {
    PointerInput {
        scene: Some(SceneInput {
            view_direction: -DVec3::Z,
            mesh_transforms: transforms,
        }),
        ..pointer(origin)
    }
}

fn first_position(context: &ToolContext<'_>, key: MeshKey) -> Vec3 {
    let mesh = context.mesh(key).unwrap();
    let vertex = mesh.topology().verts().next().unwrap();

    mesh.attributes().positions[vertex]
}

#[test]
fn drag_update_advances_the_baseline_and_cancellation_ignores_pending_motion() {
    let start = DVec3::new(10.0, 20.0, 0.0);
    let mut drag = TranslationDrag {
        plane: Plane::new(DVec3::ZERO, DVec3::Z).unwrap(),
        state: DragState::new(start),
    };
    assert_eq!(drag.state.update(), DVec3::ZERO);
    assert_eq!(drag.cancellation_delta(), DVec3::ZERO);

    drag.state.update_position(Some(start + DVec3::X));
    drag.state.update_position(Some(start + 3.0 * DVec3::X));
    drag.state.update_position(None);
    assert_eq!(drag.cancellation_delta(), DVec3::ZERO);
    assert_eq!(drag.state.update(), 3.0 * DVec3::X);

    assert_eq!(drag.state.current_position, start + 3.0 * DVec3::X);
    assert_eq!(drag.state.last_update_position, drag.state.current_position);
    assert_eq!(drag.state.update(), DVec3::ZERO);
    assert_eq!(drag.cancellation_delta(), -3.0 * DVec3::X);
    drag.state.update_position(None);
    assert_eq!(drag.state.update(), DVec3::ZERO);
    assert_eq!(drag.cancellation_delta(), -3.0 * DVec3::X);

    drag.state.update_position(Some(start + 1e30 * DVec3::X));
    assert_eq!(drag.cancellation_delta(), -3.0 * DVec3::X);

    drag.state.update_position(Some(start));
    assert_eq!(drag.state.update(), -3.0 * DVec3::X);
    assert_eq!(drag.state.update(), DVec3::ZERO);
    assert_eq!(drag.cancellation_delta(), DVec3::ZERO);
}

#[test]
fn translation_is_deferred_incremental_and_release_flushes_final_motion() {
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [key];
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    assert!(tool.pointer_move(&mut context, &pointer(DVec3::new(1.0, 0.0, 1.0))));
    assert!(tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0))));
    assert_eq!(first_position(&context, key), Vec3::ZERO);

    tool.update(&mut context, 0.016);

    assert_eq!(first_position(&context, key), 2.0 * Vec3::X);

    tool.update(&mut context, 0.016);

    assert_eq!(first_position(&context, key), 2.0 * Vec3::X);
    assert!(tool.pointer_up(&mut context, &pointer(DVec3::new(3.0, 1.0, 1.0))));
    assert_eq!(first_position(&context, key), Vec3::new(3.0, 1.0, 0.0));
    assert!(tool.drag.is_none());
    assert!(tool.meshes.is_empty());

    tool.cancel(&mut context);

    assert_eq!(first_position(&context, key), Vec3::new(3.0, 1.0, 0.0));
}

#[test]
fn cancel_reverses_applied_movement_and_ignores_pending_movement() {
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [key];
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 3.0, 1.0)));
    tool.update(&mut context, 0.016);
    tool.pointer_move(&mut context, &pointer(DVec3::new(7.0, 9.0, 1.0)));
    tool.cancel(&mut context);
    tool.cancel(&mut context);
    tool.update(&mut context, 0.016);

    assert_eq!(first_position(&context, key), Vec3::ZERO);
    assert!(tool.drag.is_none());
    assert!(tool.meshes.is_empty());
}

#[test]
fn missing_transform_rejects_entire_drag_and_preserves_selection() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let second = meshes.insert(selected_mesh(&[Vec3::X]));
    let active = [first, second];
    let transforms = [MeshTransform::new(first, DMat4::IDENTITY).unwrap()];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(!tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    assert!(!tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0))));
    tool.update(&mut context, 0.016);

    assert!(tool.meshes.is_empty());
    assert_eq!(first_position(&context, first), Vec3::ZERO);
    assert_eq!(first_position(&context, second), Vec3::X);
    assert_eq!(context.mesh(first).unwrap().selection().len(), 1);
    assert_eq!(context.mesh(second).unwrap().selection().len(), 1);
}

#[test]
fn collecting_active_transforms_reuses_storage_and_clears_partial_results() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let second = meshes.insert(selected_mesh(&[Vec3::X]));
    let empty = meshes.insert(selected_mesh(&[Vec3::Y]));
    let inactive = meshes.insert(selected_mesh(&[Vec3::Z]));
    meshes[empty].selection_mut().clear();
    let active = [first, empty, second];
    let transforms = [
        MeshTransform::new(inactive, DMat4::IDENTITY).unwrap(),
        MeshTransform::new(second, DMat4::from_translation(8.0 * DVec3::Z)).unwrap(),
        MeshTransform::new(empty, DMat4::IDENTITY).unwrap(),
        MeshTransform::new(first, DMat4::IDENTITY).unwrap(),
    ];
    let context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut output = Vec::with_capacity(active.len());
    output.push(transforms[0]);
    let allocation = output.as_ptr();
    let capacity = output.capacity();

    assert_eq!(
        collect_active_mesh_transforms(&context, &transforms, &mut output),
        Some(())
    );
    assert_eq!(output, [transforms[3], transforms[1]]);
    assert_eq!(output.as_ptr(), allocation);

    assert_eq!(
        collect_active_mesh_transforms(&context, &transforms[3..], &mut output),
        None
    );
    assert!(output.is_empty());
    assert_eq!(output.capacity(), capacity);
    assert_eq!(output.as_ptr(), allocation);

    assert_eq!(
        collect_active_mesh_transforms(&context, &transforms, &mut output),
        Some(())
    );
    let empty_active = [empty];
    let context = ToolContext::new(&mut meshes, &empty_active, EditMode::VERTEX);
    assert_eq!(
        collect_active_mesh_transforms(&context, &transforms, &mut output),
        Some(())
    );
    assert!(output.is_empty());
    assert_eq!(output.as_ptr(), allocation);
}

#[test]
fn selection_centroid_weights_vertices_and_requires_complete_placements() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let second = meshes.insert(selected_mesh(&[Vec3::ZERO; 3]));
    let active = [first, second];
    let transforms = [
        MeshTransform::new(first, DMat4::IDENTITY).unwrap(),
        MeshTransform::new(second, DMat4::from_translation(8.0 * DVec3::Z)).unwrap(),
    ];
    let context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);

    assert_eq!(selection_centroid(&context, &[]), None);
    assert_eq!(
        selection_centroid(&context, &transforms),
        Some(6.0 * DVec3::Z)
    );
    assert_eq!(selection_centroid(&context, &transforms[..1]), None);
}

#[test]
fn invalid_selected_positions_prevent_drag_start() {
    for position in [Vec3::NAN, Vec3::splat(f32::INFINITY)] {
        let mut meshes = SlotMap::with_key();
        let key = meshes.insert(selected_mesh(&[position]));
        let active = [key];
        let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
        let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);

        assert_eq!(selection_centroid(&context, &transforms), None);

        let mut tool = TranslateTool::default();
        assert!(!tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
        assert!(tool.drag.is_none());
        assert!(tool.meshes.is_empty());
    }
}

#[test]
fn weighted_anchor_sets_plane_depth_and_drag_start_values_are_copied() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let second = meshes.insert(selected_mesh(&[Vec3::ZERO, Vec3::X, Vec3::Y]));
    let active = [first, second];
    let mut transforms = [
        MeshTransform::new(first, DMat4::IDENTITY).unwrap(),
        MeshTransform::new(second, DMat4::from_translation(8.0 * DVec3::Z)).unwrap(),
    ];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, 10.0 * DVec3::Z)));

    transforms[1] = MeshTransform::new(second, DMat4::from_scale(DVec3::splat(100.0))).unwrap();
    let movement = PointerInput {
        ray: Some(WorldRay {
            origin: 10.0 * DVec3::Z,
            direction: DVec3::new(1.0, 0.0, -1.0).normalize(),
        }),
        ..start_input(&transforms, DVec3::ZERO)
    };

    assert!(tool.pointer_move(&mut context, &movement));
    tool.update(&mut context, 0.016);

    assert_eq!(first_position(&context, first), 4.0 * Vec3::X);
    assert_eq!(first_position(&context, second), 4.0 * Vec3::X);
    assert!(tool.pointer_move(&mut context, &movement));
    tool.update(&mut context, 0.016);

    assert_eq!(first_position(&context, first), 4.0 * Vec3::X);
}

#[test]
fn world_displacement_maps_through_each_inverse_as_a_vector() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let second = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let third = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [first, second, third];
    let transforms = [
        MeshTransform::new(first, DMat4::IDENTITY).unwrap(),
        MeshTransform::new(
            second,
            DMat4::from_scale_rotation_translation(
                DVec3::new(2.0, 3.0, 4.0),
                glam::DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2),
                DVec3::new(100.0, -50.0, 0.0),
            ),
        )
        .unwrap(),
        MeshTransform::new(third, DMat4::from_scale(DVec3::new(-2.0, 1.0, 1.0))).unwrap(),
    ];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    tool.pointer_move(&mut context, &pointer(DVec3::new(6.0, 4.0, 1.0)));
    tool.update(&mut context, 0.016);

    for (key, expected) in [
        (first, Vec3::new(6.0, 4.0, 0.0)),
        (second, Vec3::new(2.0, -2.0, 0.0)),
        (third, Vec3::new(-3.0, 4.0, 0.0)),
    ] {
        assert!(first_position(&context, key).abs_diff_eq(expected, 1e-5));
    }

    tool.cancel(&mut context);

    for key in active {
        assert!(first_position(&context, key).abs_diff_eq(Vec3::ZERO, 1e-5));
    }
}

#[test]
fn confirmation_and_release_use_the_last_valid_pointer_position() {
    for finish in 0..3 {
        let mut meshes = SlotMap::with_key();
        let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
        let active = [key];
        let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
        let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
        let mut tool = TranslateTool::default();
        let start = start_input(&transforms, DVec3::Z);

        assert!(tool.pointer_down(&mut context, &start));
        tool.pointer_move(&mut context, &pointer(DVec3::new(1.0, 0.0, 1.0)));
        tool.update(&mut context, 0.016);
        tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0)));

        assert!(!tool.pointer_down(&mut context, &start));

        match finish {
            0 => tool.confirm(&mut context),
            1 => assert!(tool.pointer_up(&mut context, &PointerInput::default())),
            _ => assert!(tool.pointer_up(
                &mut context,
                &PointerInput {
                    ray: Some(WorldRay {
                        origin: DVec3::Z,
                        direction: DVec3::X
                    }),
                    ..PointerInput::default()
                }
            )),
        }

        let expected = 2.0 * Vec3::X;

        assert_eq!(first_position(&context, key), expected);
        assert!(!tool.pointer_move(&mut context, &pointer(DVec3::new(4.0, 0.0, 1.0))));
        assert!(!tool.pointer_up(&mut context, &start));
        tool.cancel(&mut context);

        assert_eq!(first_position(&context, key), expected);
    }
}

#[test]
fn invalid_latest_movement_preserves_unapplied_motion_and_recovers() {
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [key];
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    tool.pointer_move(&mut context, &pointer(DVec3::new(1.0, 0.0, 1.0)));
    tool.update(&mut context, 0.016);
    tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0)));
    assert!(tool.pointer_move(
        &mut context,
        &PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::Z,
                direction: DVec3::X,
            }),
            ..PointerInput::default()
        }
    ));
    tool.update(&mut context, 0.016);
    tool.update(&mut context, 0.016);

    assert_eq!(first_position(&context, key), 2.0 * Vec3::X);
    assert_eq!(
        tool.drag.as_ref().unwrap().cancellation_delta(),
        -2.0 * DVec3::X
    );

    tool.pointer_move(&mut context, &pointer(DVec3::new(3.0, 0.0, 1.0)));
    tool.update(&mut context, 0.016);
    assert_eq!(first_position(&context, key), 3.0 * Vec3::X);

    tool.cancel(&mut context);
    assert_eq!(first_position(&context, key), Vec3::ZERO);
}

#[test]
fn invalid_start_data_does_not_leave_a_drag() {
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [key];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];

    let valid = start_input(&transforms, DVec3::Z);
    for input in [
        PointerInput::default(),
        pointer(DVec3::Z),
        PointerInput { ray: None, ..valid },
        PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::Z,
                direction: DVec3::X,
            }),
            ..valid
        },
        PointerInput {
            scene: Some(SceneInput {
                view_direction: DVec3::ZERO,
                mesh_transforms: &transforms,
            }),
            ..valid
        },
    ] {
        assert!(!tool.pointer_down(&mut context, &input));
        assert!(tool.meshes.is_empty());
    }

    assert_eq!(first_position(&context, key), Vec3::ZERO);
    assert!(tool.pointer_down(&mut context, &valid));
    tool.cancel(&mut context);
}

#[test]
fn only_active_meshes_with_selected_vertices_participate() {
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let empty = meshes.insert(selected_mesh(&[Vec3::Y]));
    let inactive = meshes.insert(selected_mesh(&[Vec3::X]));
    meshes[empty].selection_mut().clear();
    let active = [key, empty];
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    assert_eq!(tool.meshes.len(), 1);
    tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0)));
    tool.confirm(&mut context);

    assert_eq!(first_position(&context, key), 2.0 * Vec3::X);
    assert_eq!(first_position(&context, empty), Vec3::Y);
    let vertex = meshes[inactive].topology().verts().next().unwrap();
    assert_eq!(meshes[inactive].attributes().positions[vertex], Vec3::X);
}

#[test]
fn supplied_empty_selection_meshes_are_excluded_at_start() {
    for cancel in [false, true] {
        let mut meshes = SlotMap::with_key();
        let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
        let empty = meshes.insert(selected_mesh(&[Vec3::Y]));
        meshes[empty].selection_mut().clear();
        let active = [key, empty];
        let transforms = [
            MeshTransform::new(
                empty,
                DMat4::from_scale_rotation_translation(
                    DVec3::new(1e-40, 1.0, 1.0),
                    glam::DQuat::IDENTITY,
                    100.0 * DVec3::Z,
                ),
            )
            .unwrap(),
            MeshTransform::new(key, DMat4::IDENTITY).unwrap(),
        ];
        let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
        let mut tool = TranslateTool::default();

        assert_eq!(selection_centroid(&context, &transforms), Some(DVec3::ZERO));
        assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
        assert_eq!(tool.meshes, [transforms[1]]);
        tool.pointer_move(&mut context, &pointer(DVec3::new(1.0, 0.0, 1.0)));
        tool.update(&mut context, 0.016);
        assert_eq!(first_position(&context, key), Vec3::X);
        assert_eq!(first_position(&context, empty), Vec3::Y);

        tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0)));
        if cancel {
            tool.cancel(&mut context);
        } else {
            tool.confirm(&mut context);
        }

        assert_eq!(
            first_position(&context, key),
            if cancel { Vec3::ZERO } else { 2.0 * Vec3::X }
        );
        assert_eq!(first_position(&context, empty), Vec3::Y);
        assert!(tool.meshes.is_empty());
        assert!(tool.drag.is_none());
    }
}

#[test]
fn full_scene_transforms_only_resolve_active_selected_meshes() {
    let mut meshes = SlotMap::with_key();
    let active = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let inactive = meshes.insert(selected_mesh(&[Vec3::X]));
    let empty = meshes.insert(selected_mesh(&[Vec3::Y]));
    meshes[empty].selection_mut().clear();
    let active_keys = [active, empty];
    let transforms = [
        MeshTransform::new(inactive, DMat4::from_translation(100.0 * DVec3::Z)).unwrap(),
        MeshTransform::new(empty, DMat4::from_translation(50.0 * DVec3::Z)).unwrap(),
        MeshTransform::new(active, DMat4::IDENTITY).unwrap(),
    ];
    let mut context = ToolContext::new(&mut meshes, &active_keys, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    assert_eq!(tool.meshes, [transforms[2]]);
    tool.pointer_move(&mut context, &pointer(DVec3::new(2.0, 0.0, 1.0)));
    tool.update(&mut context, 0.016);
    assert_eq!(first_position(&context, active), 2.0 * Vec3::X);
    assert_eq!(first_position(&context, empty), Vec3::Y);
    tool.cancel(&mut context);
    assert_eq!(first_position(&context, active), Vec3::ZERO);
    let vertex = meshes[inactive].topology().verts().next().unwrap();
    assert_eq!(meshes[inactive].attributes().positions[vertex], Vec3::X);
}

#[test]
fn empty_selections_and_empty_modes_cannot_start_a_drag() {
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [key];
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
    let mut tool = TranslateTool::default();
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::NONE);

    assert!(!tool.supports_edit_mode(EditMode::NONE));
    assert!(tool.supports_edit_mode(EditMode::FACE));
    assert!(!tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    assert!(!tool.pointer_click(&mut context, &PointerInput::default()));
    tool.confirm(&mut context);
    tool.cancel(&mut context);

    meshes[key].selection_mut().clear();
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);

    assert!(!tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    assert!(tool.meshes.is_empty());
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "world vector must be representable in local f32 coordinates")]
fn unrepresentable_local_movement_violates_the_debug_contract() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let second = meshes.insert(selected_mesh(&[Vec3::ZERO]));
    let active = [first, second];
    let transforms = [
        MeshTransform::new(first, DMat4::IDENTITY).unwrap(),
        MeshTransform::new(second, DMat4::from_scale(DVec3::new(1e-40, 1.0, 1.0))).unwrap(),
    ];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    tool.pointer_move(&mut context, &pointer(DVec3::new(1.0, 0.0, 1.0)));
    tool.update(&mut context, 0.016);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "world vector must be representable in local f32 coordinates")]
fn unrepresentable_cancellation_total_violates_the_debug_contract() {
    let initial = Vec3::new(-2e38, 0.0, 0.0);
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[initial]));
    let active = [key];
    let transforms = [MeshTransform::new(key, DMat4::IDENTITY).unwrap()];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, DVec3::Z)));
    tool.pointer_move(&mut context, &pointer(DVec3::new(2e38, 0.0, 1.0)));
    tool.update(&mut context, 0.016);
    assert_eq!(first_position(&context, key), Vec3::ZERO);

    tool.pointer_move(&mut context, &pointer(DVec3::new(4e38, 0.0, 1.0)));
    tool.update(&mut context, 0.016);
    assert_eq!(first_position(&context, key), Vec3::new(2e38, 0.0, 0.0));
    assert_eq!(
        tool.drag.as_ref().unwrap().cancellation_delta(),
        DVec3::new(-4e38, 0.0, 0.0)
    );

    tool.cancel(&mut context);
}

#[test]
fn long_drag_at_world_scale_cancels_with_bounded_drift_and_reuses_storage() {
    let initial = Vec3::new(25.125, -10.5, 0.0);
    let world = DVec3::new(6_378_137.125, 4_000_000.5, 2_000_000.25);
    let mut meshes = SlotMap::with_key();
    let key = meshes.insert(selected_mesh(&[initial]));
    let rotated = meshes.insert(selected_mesh(&[initial]));
    let reflected = meshes.insert(selected_mesh(&[initial]));
    let active = [key, rotated, reflected];
    let transforms = [
        MeshTransform::new(key, DMat4::from_translation(world)).unwrap(),
        MeshTransform::new(
            rotated,
            DMat4::from_scale_rotation_translation(
                DVec3::new(2.0, 3.0, 4.0),
                glam::DQuat::from_rotation_z(0.7),
                world,
            ),
        )
        .unwrap(),
        MeshTransform::new(
            reflected,
            DMat4::from_scale_rotation_translation(
                DVec3::new(-2.0, 1.0, 1.0),
                glam::DQuat::IDENTITY,
                world,
            ),
        )
        .unwrap(),
    ];
    let mut context = ToolContext::new(&mut meshes, &active, EditMode::VERTEX);
    let mut tool = TranslateTool::default();
    let origin = world + initial.as_dvec3() + DVec3::Z;

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, origin)));
    let allocation = tool.meshes.as_ptr();
    let capacity = tool.meshes.capacity();

    for step in 1..=4_000 {
        let movement = pointer(origin + DVec3::X * (step as f64 * 0.0035));
        assert!(tool.pointer_move(&mut context, &movement));
        tool.update(&mut context, 0.016);
    }

    for transform in &transforms {
        let delta = transform
            .local_to_world()
            .inverse()
            .transform_vector3(14.0 * DVec3::X)
            .as_vec3();
        assert!(first_position(&context, transform.mesh).abs_diff_eq(initial + delta, 0.005));
    }
    assert!((tool.drag.as_ref().unwrap().cancellation_delta().x + 14.0).abs() < 1e-5);
    assert_eq!(tool.meshes.as_ptr(), allocation);

    tool.cancel(&mut context);

    for key in active {
        assert!(first_position(&context, key).abs_diff_eq(initial, 0.005));
    }
    assert_eq!(tool.meshes.capacity(), capacity);

    assert!(tool.pointer_down(&mut context, &start_input(&transforms, origin)));
    assert_eq!(tool.meshes.as_ptr(), allocation);
    assert_eq!(
        tool.drag.as_ref().unwrap().cancellation_delta(),
        DVec3::ZERO
    );
    tool.confirm(&mut context);

    assert_eq!(tool.meshes.capacity(), capacity);
}
