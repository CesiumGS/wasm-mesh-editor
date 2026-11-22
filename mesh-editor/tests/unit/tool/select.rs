use mesh::{ComponentKey, ComponentMask, Mesh, MeshBuffers, VertKey};
use slotmap::{Key, SlotMap};

use crate::mesh_editor::ActiveMeshes;
use crate::{MeshKey, Modifiers, PickHit};

use super::*;

const SHIFT: Modifiers = Modifiers {
    shift: true,
    control: false,
    alt: false,
    meta: false,
};

const CONTROL: Modifiers = Modifiers {
    control: true,
    ..SHIFT
};

/// A click with `hits` and `modifiers`.
fn click<'a>(hits: &'a [PickHit<'a>], modifiers: Modifiers) -> PointerInput<'a> {
    PointerInput {
        pick: PickResult::Hits(hits),
        modifiers,
        ..PointerInput::default()
    }
}

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
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn hit_without_modifiers_replaces_selection_across_active_meshes() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(triangle_mesh());
    let second = meshes.insert(triangle_mesh());
    let vertices: Vec<_> = meshes[first].topology().verts().collect();
    let other_vertex = meshes[second].topology().verts().next().unwrap();

    meshes[first].selection_mut().select(&[vertices[0]]);
    meshes[second].selection_mut().select(&[other_vertex]);

    let mut active_meshes = ActiveMeshes::from(vec![first, second]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);
    let hits = [PickHit {
        mesh: first,
        components: &[vertices[1].into()],
    }];
    let input = click(
        &hits,
        Modifiers {
            alt: true,
            ..Modifiers::default()
        },
    );

    assert!(SelectTool::default().pointer_click(&mut context, &input));

    let selected = context.mesh(first).unwrap().selection();
    assert!(selected.contains(vertices[1]));
    assert_eq!(selected.len(), 1);
    assert!(context.mesh(second).unwrap().selection().is_empty());

    assert!(SelectTool::default().pointer_click(&mut context, &input));
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
        meshes[target].set_interactable_components(mode.interactable_components);

        let component = if mode == EditMode::VERTEX {
            ComponentKey::from(meshes[target].topology().verts().next().unwrap())
        } else if mode == EditMode::EDGE {
            meshes[target].topology().edges().next().unwrap().into()
        } else {
            meshes[target].topology().faces().next().unwrap().into()
        };
        let mut active_meshes = ActiveMeshes::from(vec![target, other]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);
        let hits = [PickHit {
            mesh: target,
            components: &[component],
        }];
        let input = click(&hits, SHIFT);

        assert!(SelectTool::default().pointer_click(&mut context, &input));
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

        assert!(SelectTool::default().pointer_click(&mut context, &input));
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
fn only_misses_without_modifiers_clear_active_meshes() {
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

    for (pick, shift, handled) in [
        (PickResult::NotPerformed, false, false),
        (PickResult::NotPerformed, true, false),
        (PickResult::Hits(&[]), true, true),
    ] {
        let input = PointerInput {
            pick,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            ..PointerInput::default()
        };

        assert_eq!(
            SelectTool::default().pointer_click(&mut context, &input),
            handled
        );
        assert_eq!(context.mesh(first).unwrap().selection().len(), 1);
        assert_eq!(context.mesh(second).unwrap().selection().len(), 1);
    }

    assert!(SelectTool::default().pointer_click(&mut context, &click(&[], Modifiers::default())));
    assert!(context.mesh(first).unwrap().selection().is_empty());
    assert!(context.mesh(second).unwrap().selection().is_empty());
    assert_eq!(meshes[inactive].selection().len(), 1);
}

#[test]
fn rejected_hits_are_ignored() {
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

            let hits = [PickHit {
                mesh,
                components: &[component],
            }];
            let modifiers = if shift { SHIFT } else { Modifiers::default() };

            assert!(SelectTool::default().pointer_click(&mut context, &click(&hits, modifiers)));

            // Replacing still clears the active meshes without usable hits
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
fn combined_interactable_components_accept_each_enabled_kind() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let mode = EditMode {
        renderable_components: ComponentMask::empty(),
        interactable_components: ComponentMask::VERTEX | ComponentMask::EDGE,
    };
    meshes[target].set_interactable_components(mode.interactable_components);
    let vertex = meshes[target].topology().verts().next().unwrap();
    let edge = meshes[target].topology().edges().next().unwrap();
    let face = meshes[target].topology().faces().next().unwrap();
    let mut active_meshes = ActiveMeshes::from(vec![target]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

    assert!(SelectTool::default().supports_edit_mode(mode));

    for component in [ComponentKey::from(vertex), edge.into()] {
        let hits = [PickHit {
            mesh: target,
            components: &[component],
        }];

        assert!(
            SelectTool::default().pointer_click(&mut context, &click(&hits, Modifiers::default()))
        );
        assert!(
            context
                .mesh(target)
                .unwrap()
                .selection()
                .contains(component)
        );
    }

    let hits = [PickHit {
        mesh: target,
        components: &[face.into()],
    }];

    assert!(SelectTool::default().pointer_click(&mut context, &click(&hits, Modifiers::default())));
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
    let mut tool = SelectTool::default();
    let input = click(&[], Modifiers::default());

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
        interactable_components: ComponentMask::empty(),
    },
];

fn mesh_click(pick: PickResult<'_>, shift: bool) -> PointerInput<'_> {
    PointerInput {
        pick,
        modifiers: Modifiers {
            shift,
            ..Modifiers::default()
        },
        ..PointerInput::default()
    }
}

/// A hit on `mesh`. Mesh selection doesn't use the hit components.
fn mesh_hit(mesh: MeshKey) -> [PickHit<'static>; 1] {
    [PickHit {
        mesh,
        components: &[],
    }]
}

#[test]
fn every_mode_is_supported() {
    for mode in
        EMPTY_SELECTION_MODES
            .into_iter()
            .chain([EditMode::VERTEX, EditMode::EDGE, EditMode::FACE])
    {
        assert!(SelectTool::default().supports_edit_mode(mode));
    }
}

#[test]
fn empty_interactable_components_make_hits_without_modifiers_the_only_active_mesh() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let first = meshes.insert(triangle_mesh());
        let second = meshes.insert(triangle_mesh());
        let clicked = meshes.insert(triangle_mesh());
        let vertex = meshes[first].topology().verts().next().unwrap();

        meshes[first].selection_mut().select(&[vertex]);
        meshes[clicked].set_interactable_components(ComponentMask::VERTEX);

        let hits = mesh_hit(clicked);
        let hit = PickResult::Hits(&hits);
        let mut active_meshes = ActiveMeshes::from(vec![first, second]);

        {
            let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

            for _ in 0..2 {
                assert!(SelectTool::default().pointer_click(&mut context, &mesh_click(hit, false)));
                assert_eq!(context.active_meshes(), &[clicked]);
            }
        }

        assert_eq!(
            meshes[clicked].interactable_components(),
            mode.interactable_components,
            "activation applies the mode"
        );
        assert!(
            meshes[first].selection().contains(vertex),
            "deactivation preserves selection"
        );
    }
}

#[test]
fn empty_interactable_components_toggle_shift_hits_and_preserve_other_meshes() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let active = meshes.insert(triangle_mesh());
        let target = meshes.insert(triangle_mesh());
        let hits = mesh_hit(target);
        let hit = PickResult::Hits(&hits);
        let mut active_meshes = ActiveMeshes::from(vec![active]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

        assert!(SelectTool::default().pointer_click(&mut context, &mesh_click(hit, true)));
        assert_eq!(context.active_meshes(), &[active, target]);

        assert!(SelectTool::default().pointer_click(&mut context, &mesh_click(hit, true)));
        assert_eq!(context.active_meshes(), &[active]);
    }
}

#[test]
fn empty_interactable_components_deactivate_all_only_on_misses_without_modifiers() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let first = meshes.insert(triangle_mesh());
        let second = meshes.insert(triangle_mesh());
        let removed = meshes.insert(triangle_mesh());

        meshes.remove(removed).unwrap();

        let removed_hits = mesh_hit(removed);
        let null_hits = mesh_hit(MeshKey::null());
        let unknown_picks = [
            PickResult::Hits(&removed_hits),
            PickResult::Hits(&null_hits),
        ];
        let mut active_meshes = ActiveMeshes::from(vec![first, second]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);

        for shift in [false, true] {
            let not_picked = mesh_click(PickResult::NotPerformed, shift);

            assert!(!SelectTool::default().pointer_click(&mut context, &not_picked));
            assert_eq!(context.active_meshes(), &[first, second]);
        }

        for pick in [PickResult::Hits(&[])].into_iter().chain(unknown_picks) {
            assert!(SelectTool::default().pointer_click(&mut context, &mesh_click(pick, true)));
            assert_eq!(context.active_meshes(), &[first, second]);
        }

        for pick in [PickResult::Hits(&[])].into_iter().chain(unknown_picks) {
            context.retain_active_meshes(|_| false);
            context.set_mesh_active(first).unwrap();
            context.set_mesh_active(second).unwrap();

            assert!(SelectTool::default().pointer_click(&mut context, &mesh_click(pick, false)));
            assert!(context.active_meshes().is_empty());
        }
    }
}

#[test]
fn empty_interactable_components_ignore_hit_components() {
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
        let hits = [PickHit {
            mesh: target,
            components: &[component],
        }];
        let hit = PickResult::Hits(&hits);

        context.retain_active_meshes(|_| false);

        assert!(SelectTool::default().pointer_click(&mut context, &mesh_click(hit, false)));
        assert_eq!(context.active_meshes(), &[target]);
        assert!(context.mesh(target).unwrap().selection().is_empty());
    }
}

#[test]
fn modifiers_policy_picks_control_over_shift_over_no_modifier() {
    let policy = SelectModifiersPolicy {
        no_modifier: SelectAction::Replace,
        shift: SelectAction::Add,
        control: SelectAction::Remove,
    };
    let meta = Modifiers {
        meta: true,
        ..Modifiers::default()
    };
    let alt = Modifiers {
        alt: true,
        ..Modifiers::default()
    };

    assert_eq!(policy.action(Modifiers::default()), SelectAction::Replace);
    assert_eq!(policy.action(alt), SelectAction::Replace);
    assert_eq!(policy.action(SHIFT), SelectAction::Add);
    assert_eq!(policy.action(CONTROL), SelectAction::Remove);
    assert_eq!(policy.action(meta), SelectAction::Remove);
    assert_eq!(
        policy.action(Modifiers {
            shift: true,
            ..CONTROL
        }),
        SelectAction::Remove
    );
}

#[test]
fn default_policy_toggles_with_control() {
    let mut meshes = SlotMap::with_key();
    let target = meshes.insert(triangle_mesh());
    let vertex: ComponentKey = meshes[target].topology().verts().next().unwrap().into();
    let mut active_meshes = ActiveMeshes::from(vec![target]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);
    let hits = [PickHit {
        mesh: target,
        components: &[vertex],
    }];

    assert!(SelectTool::default().pointer_click(&mut context, &click(&hits, CONTROL)));
    assert!(context.mesh(target).unwrap().selection().contains(vertex));

    assert!(SelectTool::default().pointer_click(&mut context, &click(&hits, CONTROL)));
    assert!(context.mesh(target).unwrap().selection().is_empty());
}

#[test]
fn hits_across_meshes_apply_each_action() {
    let mut meshes = SlotMap::with_key();
    let first = meshes.insert(triangle_mesh());
    let second = meshes.insert(triangle_mesh());
    let unhit = meshes.insert(triangle_mesh());
    let first_verts: Vec<ComponentKey> = meshes[first].topology().verts().map(Into::into).collect();
    let second_verts: Vec<ComponentKey> =
        meshes[second].topology().verts().map(Into::into).collect();
    let unhit_vertex = meshes[unhit].topology().verts().next().unwrap();
    meshes[unhit].selection_mut().select(&[unhit_vertex]);

    let mut active_meshes = ActiveMeshes::from(vec![first, second, unhit]);
    let mut context = ToolContext::new(&mut meshes, &mut active_meshes, EditMode::VERTEX);
    let mut tool = SelectTool {
        modifiers_policy: SelectModifiersPolicy {
            no_modifier: SelectAction::Replace,
            shift: SelectAction::Add,
            control: SelectAction::Remove,
        },
    };
    let hits = [
        PickHit {
            mesh: first,
            components: &first_verts[..2],
        },
        PickHit {
            mesh: second,
            components: &second_verts[..1],
        },
    ];
    let selected =
        |context: &ToolContext<'_>, mesh| context.mesh(mesh).unwrap().selection().verts().count();

    assert!(tool.pointer_click(&mut context, &click(&hits, Modifiers::default())));
    assert_eq!(
        [first, second, unhit].map(|mesh| selected(&context, mesh)),
        [2, 1, 0]
    );

    let more = [PickHit {
        mesh: second,
        components: &second_verts[1..],
    }];
    assert!(tool.pointer_click(&mut context, &click(&more, SHIFT)));
    assert_eq!(
        [first, second, unhit].map(|mesh| selected(&context, mesh)),
        [2, 3, 0]
    );

    assert!(tool.pointer_click(&mut context, &click(&hits, CONTROL)));
    assert_eq!(
        [first, second, unhit].map(|mesh| selected(&context, mesh)),
        [0, 2, 0]
    );

    assert!(tool.pointer_click(&mut context, &click(&[], SHIFT)));
    assert!(tool.pointer_click(&mut context, &click(&[], CONTROL)));
    assert_eq!(selected(&context, second), 2);
}

#[test]
fn hits_across_meshes_apply_each_action_to_active_meshes() {
    for mode in EMPTY_SELECTION_MODES {
        let mut meshes = SlotMap::with_key();
        let first = meshes.insert(triangle_mesh());
        let second = meshes.insert(triangle_mesh());
        let third = meshes.insert(triangle_mesh());
        let mut active_meshes = ActiveMeshes::from(vec![third]);
        let mut context = ToolContext::new(&mut meshes, &mut active_meshes, mode);
        let mut tool = SelectTool {
            modifiers_policy: SelectModifiersPolicy {
                no_modifier: SelectAction::Replace,
                shift: SelectAction::Add,
                control: SelectAction::Toggle,
            },
        };
        let both = [mesh_hit(first)[0], mesh_hit(second)[0]];

        assert!(tool.pointer_click(&mut context, &click(&both, Modifiers::default())));
        assert_eq!(context.active_meshes(), &[first, second]);

        let third_hit = mesh_hit(third);
        assert!(tool.pointer_click(&mut context, &click(&third_hit, SHIFT)));
        assert_eq!(context.active_meshes(), &[first, second, third]);

        let toggled = [mesh_hit(first)[0], mesh_hit(third)[0]];
        assert!(tool.pointer_click(&mut context, &click(&toggled, CONTROL)));
        assert_eq!(context.active_meshes(), &[second]);
    }
}
