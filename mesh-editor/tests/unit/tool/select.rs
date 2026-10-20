use mesh::{ComponentKey, ComponentMask, Mesh, MeshBuffers, VertKey};
use slotmap::{Key, SlotMap};

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
fn plain_hit_replaces_only_the_hit_mesh_selection() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(triangle_mesh());
    let second = meshes.insert(triangle_mesh());
    let vertices: Vec<_> = meshes[first].topology().verts().collect();
    let other_vertex = meshes[second].topology().verts().next().unwrap();

    meshes[first].selection_mut().select(&[vertices[0]]);
    meshes[second].selection_mut().select(&[other_vertex]);

    let active_meshes = [first, second];
    let mut context = ToolContext::new(&mut meshes, &active_meshes, EditMode::VERTEX);
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
    assert!(
        context
            .mesh(second)
            .unwrap()
            .selection()
            .contains(other_vertex)
    );

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
        let active_meshes = [target, other];
        let mut context = ToolContext::new(&mut meshes, &active_meshes, mode);
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

    let active_meshes = [first, second];
    let mut context = ToolContext::new(&mut meshes, &active_meshes, EditMode::VERTEX);

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

    assert!(!SelectTool.pointer_click(
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
fn rejected_hits_preserve_all_selection() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let other = meshes.insert(triangle_mesh());
    let inactive = meshes.insert(triangle_mesh());
    let removed = meshes.insert(triangle_mesh());
    meshes.remove(removed).unwrap();
    let replacement = meshes.insert(triangle_mesh());
    let vertex = meshes[target].topology().verts().next().unwrap();
    let edge = meshes[target].topology().edges().next().unwrap();

    for key in [target, other, inactive, replacement] {
        let selected = meshes[key].topology().verts().next().unwrap();
        meshes[key].selection_mut().select(&[selected]);
    }

    let active_meshes = [target, other];
    let mut context = ToolContext::new(&mut meshes, &active_meshes, EditMode::VERTEX);

    for (mesh, component) in [
        (inactive, vertex.into()),
        (removed, vertex.into()),
        (MeshKey::null(), vertex.into()),
        (target, VertKey::null().into()),
        (target, edge.into()),
    ] {
        for shift in [false, true] {
            let input = PointerInput {
                pick: PickResult::Hit { mesh, component },
                modifiers: Modifiers {
                    shift,
                    ..Modifiers::default()
                },
                ..PointerInput::default()
            };

            assert!(!SelectTool.pointer_click(&mut context, &input));
            assert!(context.mesh(target).unwrap().selection().contains(vertex));
            assert_eq!(context.mesh(target).unwrap().selection().len(), 1);
            assert_eq!(context.mesh(other).unwrap().selection().len(), 1);
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
    let active_meshes = [target];
    let mut context = ToolContext::new(&mut meshes, &active_meshes, mode);

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

    assert!(!SelectTool.pointer_click(
        &mut context,
        &PointerInput {
            pick: PickResult::Hit {
                mesh: target,
                component: face.into()
            },
            ..PointerInput::default()
        }
    ));
    assert!(context.mesh(target).unwrap().selection().contains(edge));
}

#[test]
fn lifecycle_and_non_click_callbacks_preserve_selection() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let vertex = meshes[target].topology().verts().next().unwrap();
    meshes[target].selection_mut().select(&[vertex]);
    let active_meshes = [target];
    let mut context = ToolContext::new(&mut meshes, &active_meshes, EditMode::VERTEX);
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

#[test]
fn empty_selection_levels_are_incompatible_and_ignore_clicks() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let vertex = meshes[target].topology().verts().next().unwrap();
    meshes[target].selection_mut().select(&[vertex]);

    for mode in [
        EditMode::NONE,
        EditMode {
            renderable_components: ComponentMask::all(),
            selection_level: ComponentMask::empty(),
        },
    ] {
        assert!(!SelectTool.supports_edit_mode(mode));

        let active_meshes = [target];
        let mut context = ToolContext::new(&mut meshes, &active_meshes, mode);
        for pick in [
            PickResult::Miss,
            PickResult::Hit {
                mesh: target,
                component: vertex.into(),
            },
        ] {
            assert!(!SelectTool.pointer_click(
                &mut context,
                &PointerInput {
                    pick,
                    ..PointerInput::default()
                }
            ));
        }

        assert!(context.mesh(target).unwrap().selection().contains(vertex));
    }
}
