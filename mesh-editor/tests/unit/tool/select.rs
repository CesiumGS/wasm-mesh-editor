use mesh::{ComponentKey, ComponentMask, Mesh, MeshBuffers, VertKey};
use slotmap::{Key, SlotMap};

use crate::mesh_editor::ActiveMeshes;
use crate::{MeshKey, Modifiers};

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

#[test]
fn plain_hit_replaces_selection_across_active_meshes() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(triangle_mesh());
    let second = meshes.insert(triangle_mesh());
    let vertices: Vec<_> = meshes[first].topology().verts().collect();
    let other_vertex = meshes[second].topology().verts().next().unwrap();

    meshes[first].selection_mut().select(&[vertices[0]]);
    meshes[second].selection_mut().select(&[other_vertex]);

    let mut active_meshes = ActiveMeshes::from(vec![first, second]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);
    let input = PointerInput {
        pick: PickResult::Hit {
            mesh: first,
            component: vertices[1].into(),
        },
        modifiers: Modifiers {
            control: true,
            alt: true,
            meta: true,
            ..Modifiers::default()
        },
        ..PointerInput::default()
    };

    assert!(SelectTool.pointer_click(&mut context, &input));

    let selected = context.mesh(first).unwrap().selection();
    assert!(selected.contains(vertices[1]));
    assert_eq!(selected.len(), 1);
    assert!(context.mesh(second).unwrap().selection().is_empty());

    assert!(SelectTool.pointer_click(&mut context, &input));
    assert_eq!(context.mesh(first).unwrap().selection().len(), 1);
}

#[test]
fn shift_hit_toggles_each_selectable_kind_and_preserves_other_meshes() {
    for mode in [EditMode::VERTEX, EditMode::EDGE, EditMode::FACE] {
        let mut meshes = SlotMap::with_key();
        let target = meshes.insert(triangle_mesh());
        let other = meshes.insert(triangle_mesh());
        let other_vertex = meshes[other].topology().verts().next().unwrap();
        meshes[other].selection_mut().select(&[other_vertex]);
        meshes[target]
            .selection_mut()
            .set_level(mode.selection_level);

        let component = if mode == EditMode::VERTEX {
            ComponentKey::from(meshes[target].topology().verts().next().unwrap())
        } else if mode == EditMode::EDGE {
            meshes[target].topology().edges().next().unwrap().into()
        } else {
            meshes[target].topology().faces().next().unwrap().into()
        };
        let mut active_meshes = ActiveMeshes::from(vec![target, other]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);
        let input = PointerInput {
            pick: PickResult::Hit {
                mesh: target,
                component,
            },
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            ..PointerInput::default()
        };

        assert!(SelectTool.pointer_click(&mut context, &input));
        assert!(
            context
                .mesh(target)
                .unwrap()
                .selection()
                .contains(component)
        );
        assert!(context.mesh(target).unwrap().selection().verts().count() > 0);
        assert!(
            context
                .mesh(other)
                .unwrap()
                .selection()
                .contains(other_vertex)
        );

        assert!(SelectTool.pointer_click(&mut context, &input));
        assert!(context.mesh(target).unwrap().selection().is_empty());
        assert!(
            context
                .mesh(other)
                .unwrap()
                .selection()
                .contains(other_vertex)
        );
    }
}

#[test]
fn only_plain_explicit_misses_clear_active_meshes() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(triangle_mesh());
    let second = meshes.insert(triangle_mesh());
    let inactive = meshes.insert(triangle_mesh());

    for key in [first, second, inactive] {
        let vertex = meshes[key].topology().verts().next().unwrap();
        meshes[key].selection_mut().select(&[vertex]);
    }

    let mut active_meshes = ActiveMeshes::from(vec![first, second]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);

    for (pick, shift) in [
        (PickResult::NotPerformed, false),
        (PickResult::NotPerformed, true),
        (PickResult::Miss, true),
    ] {
        let input = PointerInput {
            pick,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            ..PointerInput::default()
        };

        assert!(!SelectTool.pointer_click(&mut context, &input));
        assert_eq!(context.mesh(first).unwrap().selection().len(), 1);
        assert_eq!(context.mesh(second).unwrap().selection().len(), 1);
    }

    assert!(SelectTool.pointer_click(
        &mut context,
        &PointerInput {
            pick: PickResult::Miss,
            ..PointerInput::default()
        }
    ));
    assert!(context.mesh(first).unwrap().selection().is_empty());
    assert!(context.mesh(second).unwrap().selection().is_empty());
    assert_eq!(meshes[inactive].selection().len(), 1);
}

#[test]
fn rejected_hits_act_as_misses() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let other = meshes.insert(triangle_mesh());
    let inactive = meshes.insert(triangle_mesh());
    let removed = meshes.insert(triangle_mesh());
    meshes.remove(removed).unwrap();
    let replacement = meshes.insert(triangle_mesh());
    let vertex = meshes[target].topology().verts().next().unwrap();
    let edge = meshes[target].topology().edges().next().unwrap();
    let other_vertex = meshes[other].topology().verts().next().unwrap();

    for key in [inactive, replacement] {
        let selected = meshes[key].topology().verts().next().unwrap();
        meshes[key].selection_mut().select(&[selected]);
    }

    let mut active_meshes = ActiveMeshes::from(vec![target, other]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);

    for (mesh, component) in [
        (inactive, vertex.into()),
        (removed, vertex.into()),
        (MeshKey::null(), vertex.into()),
        (target, VertKey::null().into()),
        (target, edge.into()),
    ] {
        for shift in [true, false] {
            context
                .mesh_mut(target)
                .unwrap()
                .selection_mut()
                .set(&[vertex]);
            context
                .mesh_mut(other)
                .unwrap()
                .selection_mut()
                .set(&[other_vertex]);

            let input = PointerInput {
                pick: PickResult::Hit { mesh, component },
                modifiers: Modifiers {
                    shift,
                    ..Modifiers::default()
                },
                ..PointerInput::default()
            };

            assert_eq!(SelectTool.pointer_click(&mut context, &input), !shift);

            let expected_len = usize::from(shift);
            assert_eq!(
                context.mesh(target).unwrap().selection().len(),
                expected_len
            );
            assert_eq!(context.mesh(other).unwrap().selection().len(), expected_len);
        }
    }

    assert_eq!(meshes[inactive].selection().len(), 1);
    assert_eq!(meshes[replacement].selection().len(), 1);
}

#[test]
fn combined_selection_levels_accept_each_enabled_kind() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let mode = EditMode {
        renderable_components: ComponentMask::empty(),
        selection_level: ComponentMask::VERTEX | ComponentMask::EDGE,
    };
    meshes[target]
        .selection_mut()
        .set_level(mode.selection_level);
    let vertex = meshes[target].topology().verts().next().unwrap();
    let edge = meshes[target].topology().edges().next().unwrap();
    let face = meshes[target].topology().faces().next().unwrap();
    let mut active_meshes = ActiveMeshes::from(vec![target]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

    assert!(SelectTool.supports_edit_mode(mode));

    for component in [ComponentKey::from(vertex), edge.into()] {
        assert!(SelectTool.pointer_click(
            &mut context,
            &PointerInput {
                pick: PickResult::Hit {
                    mesh: target,
                    component
                },
                ..PointerInput::default()
            }
        ));
        assert!(
            context
                .mesh(target)
                .unwrap()
                .selection()
                .contains(component)
        );
    }

    assert!(SelectTool.pointer_click(
        &mut context,
        &PointerInput {
            pick: PickResult::Hit {
                mesh: target,
                component: face.into()
            },
            ..PointerInput::default()
        }
    ));
    assert!(context.mesh(target).unwrap().selection().is_empty());
}

#[test]
fn lifecycle_and_non_click_callbacks_preserve_selection() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let vertex = meshes[target].topology().verts().next().unwrap();
    meshes[target].selection_mut().select(&[vertex]);
    let mut active_meshes = ActiveMeshes::from(vec![target]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);
    let mut tool = SelectTool;
    let input = PointerInput {
        pick: PickResult::Miss,
        ..PointerInput::default()
    };

    tool.activate(&mut context);
    assert!(!tool.pointer_down(&mut context, &input));
    assert!(!tool.pointer_move(&mut context, &input));
    assert!(!tool.pointer_up(&mut context, &input));
    tool.update(&mut context, 1.0 / 60.0);
    tool.confirm(&mut context);
    tool.cancel(&mut context);
    tool.cancel(&mut context);
    tool.deactivate(&mut context);

    assert!(context.mesh(target).unwrap().selection().contains(vertex));
    assert_eq!(context.mesh(target).unwrap().selection().len(), 1);
}

const EMPTY_SELECTION_MODES: [EditMode; 2] = [
    EditMode::NONE,
    EditMode {
        renderable_components: ComponentMask::all(),
        selection_level: ComponentMask::empty(),
    },
];

fn mesh_click(pick: PickResult<ComponentKey>, shift: bool) -> PointerInput<'static, ComponentKey> {
    PointerInput {
        pick,
        modifiers: Modifiers {
            shift,
            ..Modifiers::default()
        },
        ..PointerInput::default()
    }
}

fn face_hit(meshes: &SlotMap<MeshKey, Mesh>, mesh: MeshKey) -> PickResult<ComponentKey> {
    PickResult::Hit {
        mesh,
        component: meshes[mesh].topology().faces().next().unwrap().into(),
    }
}

#[test]
fn every_mode_is_supported() {
    for mode in
        EMPTY_SELECTION_MODES
            .into_iter()
            .chain([EditMode::VERTEX, EditMode::EDGE, EditMode::FACE])
    {
        assert!(SelectTool.supports_edit_mode(mode));
    }
}

#[test]
fn empty_selection_levels_make_plain_hits_the_only_active_mesh() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let first = meshes.insert(triangle_mesh());
        let second = meshes.insert(triangle_mesh());
        let clicked = meshes.insert(triangle_mesh());
        let vertex = meshes[first].topology().verts().next().unwrap();

        meshes[first].selection_mut().select(&[vertex]);
        meshes[clicked]
            .selection_mut()
            .set_level(ComponentMask::VERTEX);

        let hit = face_hit(&meshes, clicked);
        let mut active_meshes = ActiveMeshes::from(vec![first, second]);

        {
            let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

            for _ in 0..2 {
                assert!(SelectTool.pointer_click(&mut context, &mesh_click(hit, false)));
                assert_eq!(context.active_meshes(), &[clicked]);
            }
        }

        assert_eq!(
            meshes[clicked].selection().level(),
            mode.selection_level,
            "activation applies the mode"
        );
        assert!(
            meshes[first].selection().contains(vertex),
            "deactivation preserves selection"
        );
    }
}

#[test]
fn empty_selection_levels_toggle_shift_hits_and_preserve_other_meshes() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let active = meshes.insert(triangle_mesh());
        let target = meshes.insert(triangle_mesh());
        let hit = face_hit(&meshes, target);
        let mut active_meshes = ActiveMeshes::from(vec![active]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

        assert!(SelectTool.pointer_click(&mut context, &mesh_click(hit, true)));
        assert_eq!(context.active_meshes(), &[active, target]);

        assert!(SelectTool.pointer_click(&mut context, &mesh_click(hit, true)));
        assert_eq!(context.active_meshes(), &[active]);
    }
}

#[test]
fn empty_selection_levels_deactivate_all_only_on_plain_misses() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let first = meshes.insert(triangle_mesh());
        let second = meshes.insert(triangle_mesh());
        let removed = meshes.insert(triangle_mesh());
        let removed_hit = face_hit(&meshes, removed);

        meshes.remove(removed).unwrap();

        let unknown_hits = [
            removed_hit,
            PickResult::Hit {
                mesh: MeshKey::null(),
                component: VertKey::null().into(),
            },
        ];
        let mut active_meshes = ActiveMeshes::from(vec![first, second]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

        for (pick, shift) in [
            (PickResult::NotPerformed, false),
            (PickResult::NotPerformed, true),
            (PickResult::Miss, true),
            (unknown_hits[0], true),
            (unknown_hits[1], true),
        ] {
            assert!(!SelectTool.pointer_click(&mut context, &mesh_click(pick, shift)));
            assert_eq!(context.active_meshes(), &[first, second]);
        }

        for pick in [PickResult::Miss, unknown_hits[0], unknown_hits[1]] {
            context.retain_active_meshes(|_| false);
            context.set_mesh_active(first).unwrap();
            context.set_mesh_active(second).unwrap();

            assert!(SelectTool.pointer_click(&mut context, &mesh_click(pick, false)));
            assert!(context.active_meshes().is_empty());
        }
    }
}

#[test]
fn empty_selection_levels_ignore_hit_components() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let vertex = meshes[target].topology().verts().next().unwrap();
    let edge = meshes[target].topology().edges().next().unwrap();
    let mut active_meshes = ActiveMeshes::default();
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::NONE);

    for component in [
        ComponentKey::from(vertex),
        edge.into(),
        VertKey::null().into(),
    ] {
        let hit = PickResult::Hit {
            mesh: target,
            component,
        };

        context.retain_active_meshes(|_| false);

        assert!(SelectTool.pointer_click(&mut context, &mesh_click(hit, false)));
        assert_eq!(context.active_meshes(), &[target]);
        assert!(context.mesh(target).unwrap().selection().is_empty());
    }
}
