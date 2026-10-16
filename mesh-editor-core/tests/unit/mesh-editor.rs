use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;

use glam::{DMat4, DVec3, Vec3};
use mesh_core::{ComponentKey, ComponentMask, ComponentType, MeshBuffers};
use slotmap::Key;

use crate::{
    MeshTransform, Modifiers, OverlayComponent, PickResult, PointerInput, SceneInput, SelectTool,
    ToolContext, TranslateTool, WorldRay,
};

use super::*;

struct TestTool<Value> {
    value: Value,
}

impl<Value: 'static> Tool for TestTool<Value> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn supports_edit_mode(&self, _mode: EditMode) -> bool {
        panic!("unexpected tool callback");
    }

    fn activate(&mut self, _context: &mut ToolContext<'_>) {
        panic!("unexpected tool callback");
    }

    fn deactivate(&mut self, _context: &mut ToolContext<'_>) {
        panic!("unexpected tool callback");
    }

    fn pointer_down(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        panic!("unexpected tool callback");
    }

    fn pointer_up(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        panic!("unexpected tool callback");
    }

    fn pointer_move(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        panic!("unexpected tool callback");
    }

    fn pointer_click(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        panic!("unexpected tool callback");
    }

    fn confirm(&mut self, _context: &mut ToolContext<'_>) {
        panic!("unexpected tool callback");
    }

    fn cancel(&mut self, _context: &mut ToolContext<'_>) {
        panic!("unexpected tool callback");
    }

    fn update(&mut self, _context: &mut ToolContext<'_>, _delta_seconds: f64) {
        panic!("unexpected tool callback");
    }
}

/// What a tool received from one pointer call. Scene transforms are kept by address to check
/// that they're borrowed through, not copied.
#[derive(Debug, PartialEq)]
struct ReceivedInput {
    modifiers: Modifiers,
    pick: PickResult<ComponentKey>,
    ray: Option<WorldRay>,
    scene: Option<(DVec3, *const MeshTransform)>,
}

impl ReceivedInput {
    fn new(input: &PointerInput<'_, ComponentKey>) -> Self {
        Self {
            modifiers: input.modifiers,
            pick: input.pick,
            ray: input.ray,
            scene: input
                .scene
                .map(|scene| (scene.view_direction, scene.mesh_transforms.as_ptr())),
        }
    }
}

#[derive(Debug)]
struct ToolCall {
    name: &'static str,
    action: &'static str,
    mode: EditMode,
    meshes: Vec<(MeshKey, ComponentMask)>,
}

struct RecordingTool {
    name: &'static str,
    calls: Rc<RefCell<Vec<ToolCall>>>,
    supported_mode: Option<EditMode>,
    clear_selection_on_cancel: bool,
    update_deltas: Vec<f64>,
    translation_on_update: Option<[f32; 3]>,
    received_inputs: Vec<ReceivedInput>,
}

impl RecordingTool {
    fn new(name: &'static str, calls: &Rc<RefCell<Vec<ToolCall>>>) -> Self {
        Self {
            name,
            calls: Rc::clone(calls),
            supported_mode: None,
            clear_selection_on_cancel: false,
            update_deltas: Vec::new(),
            translation_on_update: None,
            received_inputs: Vec::new(),
        }
    }

    fn record(&self, action: &'static str, context: &ToolContext<'_>) {
        let meshes = context
            .active_meshes()
            .iter()
            .map(|&key| (key, context.mesh(key).unwrap().selection().level()))
            .collect();

        self.calls.borrow_mut().push(ToolCall {
            name: self.name,
            action,
            mode: context.mode(),
            meshes,
        });
    }
}

impl Tool for RecordingTool {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn supports_edit_mode(&self, mode: EditMode) -> bool {
        self.supported_mode
            .is_none_or(|supported| supported == mode)
    }

    fn activate(&mut self, context: &mut ToolContext<'_>) {
        self.record("activate", context);
    }

    fn deactivate(&mut self, context: &mut ToolContext<'_>) {
        self.record("deactivate", context);
    }

    fn pointer_down(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        self.record("pointer_down", context);
        self.received_inputs.push(ReceivedInput::new(input));
        true
    }

    fn pointer_up(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        self.record("pointer_up", context);
        self.received_inputs.push(ReceivedInput::new(input));
        false
    }

    fn pointer_move(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        self.record("pointer_move", context);
        self.received_inputs.push(ReceivedInput::new(input));
        true
    }

    fn pointer_click(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        self.record("pointer_click", context);
        self.received_inputs.push(ReceivedInput::new(input));
        false
    }

    fn confirm(&mut self, context: &mut ToolContext<'_>) {
        self.record("confirm", context);
    }

    fn cancel(&mut self, context: &mut ToolContext<'_>) {
        self.record("cancel", context);

        if self.clear_selection_on_cancel {
            for &key in context.active_meshes() {
                context.mesh_mut(key).unwrap().selection_mut().clear();
            }
        }
    }

    fn update(&mut self, context: &mut ToolContext<'_>, delta_seconds: f64) {
        self.record("update", context);
        self.update_deltas.push(delta_seconds);

        let Some(translation) = self.translation_on_update else {
            return;
        };

        for &key in context.active_meshes() {
            let mesh = context.mesh_mut(key).unwrap();
            let vertex = mesh.topology().verts().next().unwrap();

            mesh.selection_mut().select(&[vertex]);
            mesh.translate_selected(translation.into());
        }
    }
}

fn assert_tool_calls(calls: &RefCell<Vec<ToolCall>>, expected: &[(&str, &str)]) {
    let calls: Vec<_> = calls
        .borrow()
        .iter()
        .map(|call| (call.name, call.action))
        .collect();

    assert_eq!(calls, expected);
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
    })
    .unwrap()
}

fn assert_empty_editor(editor: &MeshEditor) {
    assert_eq!(editor.mode(), EditMode::NONE);
    assert!(editor.mesh_keys().is_empty());
    assert!(editor.active_meshes().is_empty());
    assert_eq!(editor.active_tool(), None);
}

fn assert_no_overlay_changes(buffers: &OverlayBuffers) {
    assert!(!buffers.full_refresh);
    assert!(buffers.positions.dirty_range.is_none());
    assert!(buffers.edges.dirty_range.is_none());
    assert!(buffers.triangles.dirty_range.is_none());

    for selection in [
        &buffers.selection.verts,
        &buffers.selection.edges,
        &buffers.selection.faces,
    ] {
        assert!(selection.dirty_range.is_none());
    }
}

#[test]
fn translate_tool_updates_selected_components_and_overlay_positions() {
    for (mode, kind) in [
        (EditMode::VERTEX, ComponentType::Vertex),
        (EditMode::EDGE, ComponentType::Edge),
        (EditMode::FACE, ComponentType::Face),
    ] {
        let mut mesh = triangle_mesh();
        let component: ComponentKey = match kind {
            ComponentType::Vertex => mesh.topology().verts().next().unwrap().into(),
            ComponentType::Edge => mesh.topology().edges().next().unwrap().into(),
            ComponentType::Face => mesh.topology().faces().next().unwrap().into(),
        };
        mesh.selection_mut().set_level(mode.selection_level);
        mesh.selection_mut().select(&[component]);
        let selected: Vec<_> = mesh.selection().verts().collect();
        let original: Vec<_> = mesh
            .topology()
            .verts()
            .map(|key| (key, mesh.attributes().positions[key]))
            .collect();
        let mut editor = MeshEditor::new();
        let target = editor.add_mesh(mesh);
        let inactive = editor.add_mesh(triangle_mesh());
        let tool = editor.add_tool(TranslateTool::default());
        let mut result = UpdateResult::default();
        editor.set_mode(mode);
        editor.set_mesh_active(target).unwrap();
        editor.set_active_tool(Some(tool.id())).unwrap();
        editor.update(0.0, &mut result);

        let allocation = editor
            .overlay_buffers(target)
            .unwrap()
            .positions
            .data
            .as_ptr();
        let transforms: Vec<_> = editor
            .mesh_keys()
            .iter()
            .map(|&key| MeshTransform::new(key, DMat4::IDENTITY).unwrap())
            .collect();
        let start = PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::Z,
                direction: -DVec3::Z,
            }),
            scene: Some(SceneInput {
                view_direction: -DVec3::Z,
                mesh_transforms: &transforms,
            }),
            ..PointerInput::default()
        };
        let movement = PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::new(1.0, 0.0, 1.0),
                direction: -DVec3::Z,
            }),
            ..PointerInput::default()
        };

        assert!(editor.active_tool_mut().unwrap().pointer_down(&start));
        assert!(editor.active_tool_mut().unwrap().pointer_move(&movement));
        assert_eq!(
            editor.mesh(target).unwrap().attributes().positions[original[0].0],
            original[0].1
        );

        editor.update(0.016, &mut result);

        assert_eq!(result.changed_overlays, vec![target]);
        assert_eq!(
            editor
                .overlay_buffers(target)
                .unwrap()
                .positions
                .data
                .as_ptr(),
            allocation
        );
        assert_no_overlay_changes(editor.overlay_buffers(inactive).unwrap());

        for (vertex, position) in &original {
            let expected = if selected.contains(vertex) {
                *position + Vec3::X
            } else {
                *position
            };
            assert_eq!(
                editor.mesh(target).unwrap().attributes().positions[*vertex],
                expected
            );
        }

        assert!(editor.active_tool_mut().unwrap().pointer_move(&movement));
        editor.update(0.016, &mut result);

        assert!(result.changed_overlays.is_empty());

        editor.update(0.016, &mut result);
        assert!(result.changed_overlays.is_empty());

        let release = PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::new(2.0, 1.0, 1.0),
                direction: -DVec3::Z,
            }),
            ..PointerInput::default()
        };
        assert!(editor.active_tool_mut().unwrap().pointer_up(&release));

        for (vertex, position) in &original {
            let expected = if selected.contains(vertex) {
                *position + Vec3::new(2.0, 1.0, 0.0)
            } else {
                *position
            };
            assert_eq!(
                editor.mesh(target).unwrap().attributes().positions[*vertex],
                expected
            );
        }

        editor.update(0.016, &mut result);

        assert_eq!(result.changed_overlays, vec![target]);
        let buffers = editor.overlay_buffers(target).unwrap();
        let selected_indices: Vec<usize> = original
            .iter()
            .enumerate()
            .filter(|(_, (vertex, _))| selected.contains(vertex))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            buffers.positions.dirty_range,
            Some(selected_indices[0]..selected_indices[selected_indices.len() - 1] + 1)
        );

        for (index, (vertex, _)) in original.iter().enumerate() {
            assert_eq!(
                buffers.positions.data[index],
                editor.mesh(target).unwrap().attributes().positions[*vertex]
                    .extend(0.0)
                    .to_array()
            );
        }

        editor.active_tool_mut().unwrap().cancel();
        editor.update(0.016, &mut result);

        assert!(result.changed_overlays.is_empty());
    }
}

#[test]
fn translate_tool_cancels_before_editor_context_transitions() {
    for transition in ["tool", "mode", "none", "inactive", "active", "remove"] {
        let mut mesh = triangle_mesh();
        let vertex = mesh.topology().verts().next().unwrap();
        mesh.selection_mut().select(&[vertex]);
        let mut editor = MeshEditor::new();
        let target = editor.add_mesh(mesh);
        let other = editor.add_mesh(triangle_mesh());
        let tool = editor.add_tool(TranslateTool::default());
        let select = editor.add_tool(SelectTool);
        let mut result = UpdateResult::default();
        editor.set_mode(EditMode::VERTEX);
        editor.set_mesh_active(target).unwrap();
        editor.set_active_tool(Some(tool.id())).unwrap();
        let transforms: Vec<_> = editor
            .mesh_keys()
            .iter()
            .map(|&key| MeshTransform::new(key, DMat4::IDENTITY).unwrap())
            .collect();
        let start = PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::Z,
                direction: -DVec3::Z,
            }),
            scene: Some(SceneInput {
                view_direction: -DVec3::Z,
                mesh_transforms: &transforms,
            }),
            ..PointerInput::default()
        };
        assert!(editor.active_tool_mut().unwrap().pointer_down(&start));
        let mut movement = PointerInput {
            ray: Some(WorldRay {
                origin: DVec3::new(2.0, 0.0, 1.0),
                direction: -DVec3::Z,
            }),
            ..PointerInput::default()
        };
        editor.active_tool_mut().unwrap().pointer_move(&movement);
        editor.update(0.016, &mut result);

        assert_eq!(
            editor.mesh(target).unwrap().attributes().positions[vertex],
            2.0 * Vec3::X
        );

        movement.ray.as_mut().unwrap().origin.x = 4.0;
        editor.active_tool_mut().unwrap().pointer_move(&movement);
        let mut removed = None;

        match transition {
            "tool" => editor.set_active_tool(Some(select.id())).unwrap(),
            "mode" => editor.set_mode(EditMode::FACE),
            "none" => editor.set_mode(EditMode::NONE),
            "inactive" => editor.set_mesh_inactive(target).unwrap(),
            "active" => editor.set_mesh_active(other).unwrap(),
            "remove" => removed = editor.remove_mesh(target),
            _ => unreachable!(),
        }

        let mesh = removed.as_ref().or_else(|| editor.mesh(target)).unwrap();
        assert_eq!(
            mesh.attributes().positions[vertex],
            Vec3::ZERO,
            "{transition}"
        );

        if let Some(mut active) = editor.active_tool_mut() {
            assert!(!active.pointer_up(&movement));
            active.cancel();
        }

        editor.update(0.016, &mut result);

        if let Some(buffers) = editor.overlay_buffers(target) {
            assert_eq!(buffers.positions.data[0], [0.0; 4]);
        }
    }
}

#[test]
fn select_tool_clicks_update_selection_immediately_and_overlays_on_update() {
    for (mode, kind, selected_vertices) in [
        (EditMode::VERTEX, ComponentType::Vertex, 1),
        (EditMode::EDGE, ComponentType::Edge, 2),
        (EditMode::FACE, ComponentType::Face, 3),
    ] {
        let mut editor = MeshEditor::new();
        let mut result = UpdateResult::default();
        let target = editor.add_mesh(triangle_mesh());
        let inactive = editor.add_mesh(triangle_mesh());
        let tool = editor.add_tool(SelectTool);

        editor.set_mode(mode);
        editor.set_mesh_active(target).unwrap();
        editor.set_active_tool(Some(tool.id())).unwrap();
        editor.update(0.0, &mut result);

        assert!(editor.tool_config_mut(&tool).is_some());

        let component = editor
            .overlay(target)
            .unwrap()
            .component_key(OverlayComponent { kind, index: 0 })
            .unwrap();
        let positions = editor
            .overlay_buffers(target)
            .unwrap()
            .positions
            .data
            .as_ptr();
        let selection = editor.overlay_buffers(target).unwrap().selection[kind]
            .data
            .as_ptr();
        let mut input = PointerInput {
            pick: PickResult::Hit {
                mesh: target,
                component: OverlayComponent { kind, index: 0 },
            },
            ..PointerInput::default()
        };

        assert!(editor.tool_mut(tool.id()).unwrap().pointer_click(&input));
        assert!(editor.mesh(target).unwrap().selection().contains(component));
        assert_eq!(
            editor.mesh(target).unwrap().selection().verts().count(),
            selected_vertices
        );
        assert_eq!(
            editor.overlay_buffers(target).unwrap().selection[kind].data[0],
            0
        );

        editor.update(0.016, &mut result);

        assert_eq!(result.changed_overlays, vec![target]);

        let buffers = editor.overlay_buffers(target).unwrap();
        assert!(!buffers.full_refresh);
        assert_eq!(buffers.positions.data.as_ptr(), positions);
        assert!(buffers.positions.dirty_range.is_none());
        assert_eq!(buffers.selection[kind].data.as_ptr(), selection);
        assert_eq!(buffers.selection[kind].data[0], 255);
        assert_eq!(buffers.selection[kind].dirty_range, Some(0..1));
        assert_eq!(
            buffers
                .selection
                .verts
                .data
                .iter()
                .filter(|&&value| value == 255)
                .count(),
            selected_vertices
        );
        assert_no_overlay_changes(editor.overlay_buffers(inactive).unwrap());

        input.modifiers.shift = true;

        assert!(editor.active_tool_mut().unwrap().pointer_click(&input));
        assert!(editor.mesh(target).unwrap().selection().is_empty());
        assert_eq!(
            editor.overlay_buffers(target).unwrap().selection[kind].data[0],
            255
        );

        editor.update(0.016, &mut result);

        assert_eq!(result.changed_overlays, vec![target]);
        let buffers = editor.overlay_buffers(target).unwrap();
        for selection in [
            &buffers.selection.verts,
            &buffers.selection.edges,
            &buffers.selection.faces,
        ] {
            assert!(selection.data.iter().all(|&value| value == 0));
        }

        assert!(
            !editor
                .active_tool_mut()
                .unwrap()
                .pointer_click(&PointerInput::default())
        );
        editor.update(0.016, &mut result);

        assert!(result.changed_overlays.is_empty());
        assert_no_overlay_changes(editor.overlay_buffers(target).unwrap());
    }
}

#[test]
fn select_tool_deactivates_in_empty_modes_and_requires_reactivation() {
    let mut editor = MeshEditor::new();
    let tool = editor.add_tool(SelectTool);

    assert_eq!(
        editor.set_active_tool(Some(tool.id())),
        Err(ToolActivationError::IncompatibleMode {
            id: tool.id(),
            mode: EditMode::NONE
        })
    );

    editor.set_mode(EditMode::EDGE);
    editor.set_active_tool(Some(tool.id())).unwrap();
    editor.set_mode(EditMode::FACE);

    assert_eq!(editor.active_tool(), Some(tool.id()));

    editor.set_mode(EditMode::NONE);

    assert_eq!(editor.active_tool(), None);
    assert!(editor.tool_mut(tool.id()).is_none());
    assert_eq!(
        editor.set_active_tool(Some(tool.id())),
        Err(ToolActivationError::IncompatibleMode {
            id: tool.id(),
            mode: EditMode::NONE
        })
    );

    editor.set_mode(EditMode::VERTEX);

    assert_eq!(editor.active_tool(), None);

    editor.set_active_tool(Some(tool.id())).unwrap();

    assert_eq!(editor.active_tool(), Some(tool.id()));
}

#[test]
fn update_without_tools_uploads_all_registered_meshes_once() {
    let mut editor = MeshEditor::new();
    let mut result = crate::UpdateResult::default();

    assert!(result.changed_overlays.is_empty());

    result.changed_overlays.reserve(4);
    result.changed_overlays.push(MeshKey::null());

    let capacity = result.changed_overlays.capacity();
    let allocation = result.changed_overlays.as_ptr();

    editor.update(0.0, &mut result);

    assert!(result.changed_overlays.is_empty());
    assert_empty_editor(&editor);
    assert!(editor.overlay_buffers(MeshKey::null()).is_none());

    let empty = editor.add_mesh(Mesh::new());
    let triangle = editor.add_mesh(triangle_mesh());

    assert!(editor.overlay_buffers(empty).unwrap().full_refresh);
    assert!(editor.overlay_buffers(triangle).unwrap().full_refresh);
    assert_eq!(
        editor
            .overlay_buffers(triangle)
            .unwrap()
            .positions
            .data
            .len(),
        3
    );

    editor.update(0.016, &mut result);

    assert_eq!(result.changed_overlays, vec![empty, triangle]);

    for &key in &result.changed_overlays {
        let buffers = editor.overlay_buffers(key).unwrap();

        assert!(buffers.full_refresh);
        assert!(std::ptr::eq(buffers, editor.overlay_buffers(key).unwrap()));
        assert!(std::ptr::eq(
            buffers,
            editor.overlay(key).unwrap().buffers()
        ));
    }

    assert!(
        editor
            .overlay_buffers(empty)
            .unwrap()
            .positions
            .data
            .is_empty()
    );
    assert_eq!(
        editor
            .overlay_buffers(triangle)
            .unwrap()
            .positions
            .data
            .len(),
        3
    );
    assert!(editor.active_meshes().is_empty());

    editor.update(0.016, &mut result);

    assert!(result.changed_overlays.is_empty());
    assert_no_overlay_changes(editor.overlay_buffers(empty).unwrap());
    assert_no_overlay_changes(editor.overlay_buffers(triangle).unwrap());
    assert_eq!(
        editor
            .overlay_buffers(triangle)
            .unwrap()
            .positions
            .data
            .len(),
        3
    );

    let added = editor.add_mesh(triangle_mesh());

    assert!(editor.overlay_buffers(added).unwrap().full_refresh);
    editor.update(0.016, &mut result);

    assert_eq!(result.changed_overlays, vec![added]);
    assert_eq!(result.changed_overlays.capacity(), capacity);
    assert_eq!(result.changed_overlays.as_ptr(), allocation);
    assert_no_overlay_changes(editor.overlay_buffers(empty).unwrap());
    assert_no_overlay_changes(editor.overlay_buffers(triangle).unwrap());
    assert!(editor.overlay_buffers(added).unwrap().full_refresh);
}

#[test]
fn update_applies_active_tool_configuration_and_edits_in_the_same_frame() {
    let mut editor = MeshEditor::new();
    let mut result = UpdateResult::default();
    let active = editor.add_mesh(triangle_mesh());
    let inactive = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let tool = editor.add_tool(RecordingTool::new("tool", &calls));

    editor.add_tool(TestTool { value: () });
    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(active).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    calls.borrow_mut().clear();

    editor.update(0.0, &mut result);

    assert_eq!(result.changed_overlays, vec![active, inactive]);

    let positions = editor
        .overlay_buffers(active)
        .unwrap()
        .positions
        .data
        .as_ptr();

    editor.tool_config_mut(&tool).unwrap().translation_on_update = Some([2.0, 0.0, 0.0]);
    assert_eq!(
        editor.overlay_buffers(active).unwrap().positions.data[0],
        [0.0; 4]
    );

    editor.update(0.25, &mut result);

    assert_eq!(result.changed_overlays, vec![active]);

    let buffers = editor.overlay_buffers(active).unwrap();

    assert!(!buffers.full_refresh);
    assert_eq!(buffers.positions.data.as_ptr(), positions);
    assert_eq!(buffers.positions.data[0], [2.0, 0.0, 0.0, 0.0]);
    assert_eq!(buffers.positions.dirty_range, Some(0..1));
    assert_eq!(buffers.selection.verts.data, vec![255, 0, 0]);
    assert_eq!(buffers.selection.verts.dirty_range, Some(0..1));
    assert_no_overlay_changes(editor.overlay_buffers(inactive).unwrap());

    let mesh = editor.mesh(active).unwrap();
    let vertex = mesh.topology().verts().next().unwrap();

    assert_eq!(
        mesh.attributes().positions[vertex].to_array(),
        [2.0, 0.0, 0.0]
    );

    editor.tool_config_mut(&tool).unwrap().translation_on_update = None;
    editor.update(0.5, &mut result);

    assert!(result.changed_overlays.is_empty());
    assert_no_overlay_changes(editor.overlay_buffers(active).unwrap());
    assert_no_overlay_changes(editor.overlay_buffers(inactive).unwrap());
    assert_eq!(
        editor.overlay_buffers(active).unwrap().positions.data[0],
        [2.0, 0.0, 0.0, 0.0]
    );
    assert_eq!(
        editor.overlay_buffers(active).unwrap().selection.verts.data,
        vec![255, 0, 0]
    );
    assert_eq!(
        editor.tool_config_mut(&tool).unwrap().update_deltas,
        vec![0.0, 0.25, 0.5]
    );
    assert_tool_calls(
        &calls,
        &[("tool", "update"), ("tool", "update"), ("tool", "update")],
    );

    for call in calls.borrow().iter() {
        assert_eq!(call.mode, EditMode::VERTEX);
        assert_eq!(call.meshes, vec![(active, ComponentMask::VERTEX)]);
    }

    editor.set_active_tool(None).unwrap();
    calls.borrow_mut().clear();
    editor.update(1.0, &mut result);

    assert!(result.changed_overlays.is_empty());
    assert!(calls.borrow().is_empty());
    assert_eq!(
        editor.tool_config_mut(&tool).unwrap().update_deltas.len(),
        3
    );
}

#[test]
fn update_processes_cancellation_changes_after_mesh_deactivation() {
    let mut editor = MeshEditor::new();
    let mut result = UpdateResult::default();
    let key = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut tool = RecordingTool::new("tool", &calls);

    tool.translation_on_update = Some([0.0; 3]);
    tool.clear_selection_on_cancel = true;

    let tool = editor.add_tool(tool);

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(key).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    editor.update(0.016, &mut result);

    assert_eq!(result.changed_overlays, vec![key]);
    assert_eq!(
        editor.overlay_buffers(key).unwrap().selection.verts.data[0],
        255
    );

    editor.set_mesh_inactive(key).unwrap();
    editor.set_active_tool(None).unwrap();

    assert_eq!(
        editor.overlay_buffers(key).unwrap().selection.verts.data[0],
        255
    );
    editor.update(0.016, &mut result);

    assert_eq!(result.changed_overlays, vec![key]);

    let buffers = editor.overlay_buffers(key).unwrap();

    assert!(!buffers.full_refresh);
    assert_eq!(buffers.selection.verts.data, vec![0, 0, 0]);
    assert_eq!(buffers.selection.verts.dirty_range, Some(0..1));
    assert!(editor.active_meshes().is_empty());

    editor.update(0.016, &mut result);

    assert!(result.changed_overlays.is_empty());
    assert_no_overlay_changes(editor.overlay_buffers(key).unwrap());
    assert_eq!(
        editor.overlay_buffers(key).unwrap().selection.verts.data,
        vec![0, 0, 0]
    );
}

#[test]
fn update_does_not_expose_buffers_for_removed_mesh_keys() {
    let mut editor = MeshEditor::new();
    let mut result = UpdateResult::default();
    let removed = editor.add_mesh(triangle_mesh());

    editor.update(0.016, &mut result);

    assert_eq!(result.changed_overlays, vec![removed]);
    assert!(editor.overlay_buffers(removed).is_some());
    editor.remove_mesh(removed).unwrap();
    assert!(editor.overlay_buffers(removed).is_none());

    let replacement = editor.add_mesh(Mesh::new());

    assert!(editor.overlay_buffers(replacement).unwrap().full_refresh);
    editor.update(0.016, &mut result);

    assert_eq!(result.changed_overlays, vec![replacement]);
    assert!(editor.overlay_buffers(removed).is_none());
    assert!(editor.overlay_buffers(MeshKey::null()).is_none());
    assert!(editor.overlay_buffers(replacement).unwrap().full_refresh);
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

/// Clicks through the editor's tool accessor and returns the pick the tool received.
fn pick_received_by_tool(editor: &mut MeshEditor, pick: PickResult) -> PickResult<ComponentKey> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let tool = editor.add_tool(RecordingTool::new("tool", &calls));

    editor.set_active_tool(Some(tool.id())).unwrap();
    editor
        .tool_mut(tool.id())
        .unwrap()
        .pointer_click(&PointerInput {
            pick,
            ..PointerInput::default()
        });

    let received = editor.tool_config_mut(&tool).unwrap().received_inputs.pop();

    received.unwrap().pick
}

#[test]
fn tool_accessors_resolve_overlay_indices_to_components() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(triangle_mesh());

    for (kind, count) in [
        (ComponentType::Vertex, 3),
        (ComponentType::Edge, 3),
        (ComponentType::Face, 1),
    ] {
        for index in 0..count {
            let component = editor
                .overlay(mesh)
                .unwrap()
                .component_key(OverlayComponent { kind, index })
                .unwrap();

            assert_eq!(component.kind(), kind);
            assert_eq!(
                pick_received_by_tool(
                    &mut editor,
                    PickResult::Hit {
                        mesh,
                        component: OverlayComponent { kind, index }
                    }
                ),
                PickResult::Hit { mesh, component }
            );
        }

        assert_eq!(
            pick_received_by_tool(
                &mut editor,
                PickResult::Hit {
                    mesh,
                    component: OverlayComponent { kind, index: count }
                }
            ),
            PickResult::Miss
        );
    }

    assert_eq!(
        pick_received_by_tool(&mut editor, PickResult::NotPerformed),
        PickResult::NotPerformed
    );
    assert_eq!(
        pick_received_by_tool(&mut editor, PickResult::Miss),
        PickResult::Miss
    );
}

#[test]
fn tool_accessors_resolve_unknown_and_removed_meshes_as_misses() {
    let mut editor = MeshEditor::new();
    let removed = editor.add_mesh(triangle_mesh());

    assert!(editor.remove_mesh(removed).is_some());

    let replacement = editor.add_mesh(triangle_mesh());

    for mesh in [MeshKey::null(), removed] {
        assert_eq!(
            pick_received_by_tool(
                &mut editor,
                PickResult::Hit {
                    mesh,
                    component: OverlayComponent {
                        kind: ComponentType::Vertex,
                        index: 0
                    }
                }
            ),
            PickResult::Miss
        );
    }

    assert!(matches!(
        pick_received_by_tool(
            &mut editor,
            PickResult::Hit { mesh: replacement, component: OverlayComponent { kind: ComponentType::Vertex, index: 0 } }
        ),
        PickResult::Hit { mesh, .. } if mesh == replacement
    ));
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

#[test]
fn tool_registration_and_configuration_preserve_active_tool() {
    let mut editor = MeshEditor::new();
    let first = editor.add_tool(TestTool { value: 1_u32 });

    assert_eq!(editor.active_tool(), None);
    assert_eq!(editor.tool_config_mut(&first).unwrap().value, 1);

    editor.tool_config_mut(&first).unwrap().value = 10;
    editor.active_tool = Some(first.id());

    let second = editor.add_tool(TestTool { value: 2_u32 });

    assert_ne!(first.id(), second.id());
    assert_eq!(editor.active_tool(), Some(first.id()));
    assert_eq!(editor.tool_config_mut(&first).unwrap().value, 10);
    assert_eq!(editor.tool_config_mut(&second).unwrap().value, 2);

    editor.tool_config_mut(&first).unwrap().value = 11;
    editor.tool_config_mut(&second).unwrap().value = 20;

    assert_eq!(editor.tool_config_mut(&first).unwrap().value, 11);
    assert_eq!(editor.tool_config_mut(&second).unwrap().value, 20);
    assert_eq!(editor.active_tool(), Some(first.id()));
}

#[test]
fn tool_configuration_supports_multiple_concrete_types() {
    let mut editor = MeshEditor::new();
    let number = editor.add_tool(TestTool { value: 3_u32 });
    let text = editor.add_tool(TestTool {
        value: String::from("tool"),
    });

    editor
        .tool_config_mut(&text)
        .unwrap()
        .value
        .push_str(" config");

    assert_ne!(number.id(), text.id());
    assert_eq!(editor.tool_config_mut(&number).unwrap().value, 3);
    assert_eq!(editor.tool_config_mut(&text).unwrap().value, "tool config");

    let mismatched = ToolHandle::<TestTool<String>>::new(number.id());

    assert!(editor.tool_config_mut(&mismatched).is_none());
    assert_eq!(editor.tool_config_mut(&number).unwrap().value, 3);
}

#[test]
fn tool_handles_cannot_configure_foreign_editors() {
    let mut first_editor = MeshEditor::new();
    let mut second_editor = MeshEditor::new();
    let first = first_editor.add_tool(TestTool { value: 1_u32 });
    let second = second_editor.add_tool(TestTool { value: 2_u32 });

    assert_ne!(first.id(), second.id());
    assert!(first_editor.tool_config_mut(&second).is_none());
    assert!(second_editor.tool_config_mut(&first).is_none());

    let mut moved_editor = first_editor;

    assert_eq!(moved_editor.tool_config_mut(&first).unwrap().value, 1);
    assert!(moved_editor.tool_config_mut(&second).is_none());
    assert_eq!(second_editor.tool_config_mut(&second).unwrap().value, 2);

    drop(moved_editor);

    let mut new_editor = MeshEditor::new();
    let new_handle = new_editor.add_tool(TestTool { value: 3_u32 });

    assert_ne!(first.id(), new_handle.id());
    assert!(new_editor.tool_config_mut(&first).is_none());
    assert_eq!(new_editor.tool_config_mut(&new_handle).unwrap().value, 3);
}

#[test]
fn tool_configuration_rejects_missing_ids() {
    let mut editor = MeshEditor::new();
    let missing = ToolHandle::<TestTool<u32>>::new(ToolId::new());

    assert!(editor.tool_config_mut(&missing).is_none());

    let removed = editor.add_tool(TestTool { value: 1_u32 });

    editor.tools.remove(&removed.id()).unwrap();

    let replacement = editor.add_tool(TestTool { value: 2_u32 });

    assert_ne!(removed.id(), replacement.id());
    assert!(editor.tool_config_mut(&removed).is_none());
    assert!(editor.tool_config_mut(&missing).is_none());
    assert_eq!(editor.tool_config_mut(&replacement).unwrap().value, 2);
}

#[test]
fn tool_switching_cancels_then_deactivates_before_activating_replacement() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let first = editor.add_tool(RecordingTool::new("first", &calls));
    let second = editor.add_tool(RecordingTool::new("second", &calls));

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(mesh).unwrap();

    assert!(editor.active_tool_mut().is_none());
    assert_eq!(editor.set_active_tool(None), Ok(()));
    assert_eq!(editor.set_active_tool(Some(first.id())), Ok(()));
    assert_eq!(editor.set_active_tool(Some(first.id())), Ok(()));
    assert_eq!(editor.set_active_tool(Some(second.id())), Ok(()));
    assert_eq!(editor.active_tool(), Some(second.id()));
    assert_eq!(editor.set_active_tool(None), Ok(()));
    assert_eq!(editor.set_active_tool(None), Ok(()));

    assert_tool_calls(
        &calls,
        &[
            ("first", "activate"),
            ("first", "cancel"),
            ("first", "deactivate"),
            ("second", "activate"),
            ("second", "cancel"),
            ("second", "deactivate"),
        ],
    );

    for call in calls.borrow().iter() {
        assert_eq!(call.mode, EditMode::VERTEX);
        assert_eq!(call.meshes, vec![(mesh, ComponentMask::VERTEX)]);
    }

    assert_eq!(editor.active_tool(), None);
    assert!(editor.active_tool_mut().is_none());
}

#[test]
fn invalid_tool_requests_leave_the_active_tool_untouched() {
    let mut editor = MeshEditor::new();
    let mut other_editor = MeshEditor::new();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let current = editor.add_tool(RecordingTool::new("current", &calls));
    let foreign = other_editor.add_tool(RecordingTool::new("foreign", &calls));
    let mut incompatible = RecordingTool::new("incompatible", &calls);

    incompatible.supported_mode = Some(EditMode::FACE);

    let incompatible = editor.add_tool(incompatible);

    editor.set_mode(EditMode::VERTEX);
    editor.set_active_tool(Some(current.id())).unwrap();
    calls.borrow_mut().clear();

    for id in [foreign.id(), ToolId::new()] {
        assert_eq!(
            editor.set_active_tool(Some(id)),
            Err(ToolActivationError::NotFound { id })
        );
    }

    assert_eq!(
        editor.set_active_tool(Some(incompatible.id())),
        Err(ToolActivationError::IncompatibleMode {
            id: incompatible.id(),
            mode: EditMode::VERTEX
        })
    );
    assert_eq!(editor.active_tool(), Some(current.id()));
    assert!(calls.borrow().is_empty());

    editor.active_tool_mut().unwrap().confirm();
    assert_tool_calls(&calls, &[("current", "confirm")]);
}

#[test]
fn retained_tool_ids_only_dispatch_to_their_active_tool() {
    let mut editor = MeshEditor::new();
    let mut other_editor = MeshEditor::new();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let first = editor.add_tool(RecordingTool::new("first", &calls));
    let second = editor.add_tool(RecordingTool::new("second", &calls));
    let foreign = other_editor.add_tool(RecordingTool::new("foreign", &calls));

    assert!(editor.tool_mut(first.id()).is_none());
    editor.set_active_tool(Some(first.id())).unwrap();
    calls.borrow_mut().clear();

    assert!(editor.tool_mut(second.id()).is_none());
    assert!(editor.tool_mut(foreign.id()).is_none());
    assert!(editor.tool_mut(ToolId::new()).is_none());
    editor.tool_mut(first.id()).unwrap().confirm();
    editor.active_tool_mut().unwrap().cancel();

    assert_tool_calls(&calls, &[("first", "confirm"), ("first", "cancel")]);

    editor.set_active_tool(Some(second.id())).unwrap();
    calls.borrow_mut().clear();

    assert!(editor.tool_mut(first.id()).is_none());
    editor.tool_mut(second.id()).unwrap().confirm();

    assert_tool_calls(&calls, &[("second", "confirm")]);
    editor.set_active_tool(None).unwrap();
    assert!(editor.tool_mut(second.id()).is_none());
}

#[test]
fn mode_changes_cancel_with_old_context_and_keep_compatible_tool_active() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let tool = editor.add_tool(RecordingTool::new("tool", &calls));

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(mesh).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();

    for mode in [
        EditMode::EDGE,
        EditMode {
            renderable_components: ComponentMask::empty(),
            ..EditMode::EDGE
        },
    ] {
        let previous = editor.mode();
        calls.borrow_mut().clear();

        editor.set_mode(mode);

        assert_tool_calls(&calls, &[("tool", "cancel")]);
        assert_eq!(calls.borrow()[0].mode, previous);
        assert_eq!(
            calls.borrow()[0].meshes,
            vec![(mesh, previous.selection_level)]
        );
        assert_eq!(editor.active_tool(), Some(tool.id()));
        assert_eq!(editor.mode(), mode);
        assert_eq!(
            editor.mesh(mesh).unwrap().selection().level(),
            mode.selection_level
        );

        editor.active_tool_mut().unwrap().confirm();

        assert_eq!(calls.borrow()[1].mode, mode);
        assert_eq!(calls.borrow()[1].meshes, vec![(mesh, mode.selection_level)]);
    }
}

#[test]
fn mode_changes_deactivate_incompatible_tool_before_applying_mode() {
    let mut editor = MeshEditor::new();
    let mesh = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut tool = RecordingTool::new("tool", &calls);

    tool.supported_mode = Some(EditMode::VERTEX);

    let tool = editor.add_tool(tool);

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(mesh).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    calls.borrow_mut().clear();

    editor.set_mode(EditMode::FACE);

    assert_tool_calls(&calls, &[("tool", "cancel"), ("tool", "deactivate")]);

    for call in calls.borrow().iter() {
        assert_eq!(call.mode, EditMode::VERTEX);
        assert_eq!(call.meshes, vec![(mesh, ComponentMask::VERTEX)]);
    }

    assert_eq!(editor.mode(), EditMode::FACE);
    assert_eq!(
        editor.mesh(mesh).unwrap().selection().level(),
        ComponentMask::FACE
    );
    assert_eq!(editor.active_tool(), None);
    assert!(editor.active_tool_mut().is_none());
    assert!(editor.tool_mut(tool.id()).is_none());

    calls.borrow_mut().clear();
    editor.set_mode(EditMode::VERTEX);

    assert_eq!(editor.active_tool(), None);
    assert!(calls.borrow().is_empty());
}

#[test]
fn active_mesh_changes_cancel_with_previous_targets() {
    let mut editor = MeshEditor::new();
    let first = editor.add_mesh(triangle_mesh());
    let second = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let tool = editor.add_tool(RecordingTool::new("tool", &calls));

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(first).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    calls.borrow_mut().clear();

    editor.set_mesh_active(second).unwrap();
    editor.set_mesh_inactive(first).unwrap();
    editor.remove_mesh(second).unwrap();

    assert_tool_calls(
        &calls,
        &[("tool", "cancel"), ("tool", "cancel"), ("tool", "cancel")],
    );
    assert_eq!(
        calls.borrow()[0].meshes,
        vec![(first, ComponentMask::VERTEX)]
    );
    assert_eq!(
        calls.borrow()[1].meshes,
        vec![
            (first, ComponentMask::VERTEX),
            (second, ComponentMask::VERTEX)
        ]
    );
    assert_eq!(
        calls.borrow()[2].meshes,
        vec![(second, ComponentMask::VERTEX)]
    );
    assert_eq!(editor.active_tool(), Some(tool.id()));
    assert!(editor.active_meshes().is_empty());

    editor.active_tool_mut().unwrap().confirm();

    assert!(calls.borrow()[3].meshes.is_empty());
}

#[test]
fn removing_active_mesh_returns_it_after_cancellation_edits() {
    let mut editor = MeshEditor::new();
    let mut mesh = triangle_mesh();
    let vertex = mesh.topology().verts().next().unwrap();

    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    mesh.selection_mut().select(&[vertex]);

    let mesh = editor.add_mesh(mesh);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut tool = RecordingTool::new("tool", &calls);

    tool.clear_selection_on_cancel = true;

    let tool = editor.add_tool(tool);

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(mesh).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    calls.borrow_mut().clear();

    let returned = editor.remove_mesh(mesh).unwrap();

    assert_tool_calls(&calls, &[("tool", "cancel")]);
    assert_eq!(
        calls.borrow()[0].meshes,
        vec![(mesh, ComponentMask::VERTEX)]
    );
    assert!(returned.selection().is_empty());
    assert_eq!(editor.active_tool(), Some(tool.id()));
    assert!(editor.overlay(mesh).is_none());
}

#[test]
fn noops_and_inactive_registration_changes_do_not_cancel() {
    let mut editor = MeshEditor::new();
    let active = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let tool = editor.add_tool(RecordingTool::new("tool", &calls));

    editor.set_mode(EditMode::VERTEX);
    editor.set_mesh_active(active).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    calls.borrow_mut().clear();

    let inactive = editor.add_mesh(triangle_mesh());

    editor.set_mode(EditMode::VERTEX);
    editor.set_active_tool(Some(tool.id())).unwrap();
    editor.set_mesh_active(active).unwrap();
    editor.set_mesh_inactive(inactive).unwrap();
    editor.remove_mesh(inactive).unwrap();

    for key in [inactive, MeshKey::null()] {
        assert_eq!(editor.set_mesh_active(key), Err(MeshNotFound { key }));
        assert_eq!(editor.set_mesh_inactive(key), Err(MeshNotFound { key }));
        assert!(editor.remove_mesh(key).is_none());
    }

    assert_eq!(editor.active_tool(), Some(tool.id()));
    assert_eq!(editor.active_meshes(), &[active]);
    assert!(calls.borrow().is_empty());
}

#[test]
fn tool_accessors_forward_pointer_input_and_actions_with_active_context() {
    let mut editor = MeshEditor::new();
    let active = editor.add_mesh(triangle_mesh());
    let inactive = editor.add_mesh(triangle_mesh());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let tool = editor.add_tool(RecordingTool::new("tool", &calls));
    let mesh_transforms = [MeshTransform::new(
        active,
        DMat4::from_translation(DVec3::new(6_378_137.125, 2.0, 3.0)),
    )
    .unwrap()];
    let ray = WorldRay {
        origin: DVec3::new(6_378_137.125, 2.0, 4.0),
        direction: DVec3::NEG_Z,
    };
    let down = PointerInput {
        ray: Some(ray),
        scene: Some(SceneInput {
            view_direction: DVec3::NEG_Z,
            mesh_transforms: &mesh_transforms,
        }),
        ..Default::default()
    };
    let moved = PointerInput {
        ray: Some(WorldRay {
            origin: ray.origin + DVec3::X,
            ..ray
        }),
        scene: down.scene,
        ..Default::default()
    };
    let up = PointerInput {
        ray: Some(WorldRay {
            origin: ray.origin + DVec3::Y,
            ..ray
        }),
        scene: down.scene,
        ..Default::default()
    };
    let edge = editor
        .overlay(active)
        .unwrap()
        .component_key(OverlayComponent {
            kind: ComponentType::Edge,
            index: 0,
        })
        .unwrap();
    let click = PointerInput {
        modifiers: Modifiers {
            shift: true,
            control: true,
            alt: true,
            meta: true,
        },
        pick: PickResult::Hit {
            mesh: active,
            component: OverlayComponent {
                kind: ComponentType::Edge,
                index: 0,
            },
        },
        scene: down.scene,
        ..Default::default()
    };

    editor.set_mode(EditMode::EDGE);
    editor.set_mesh_active(active).unwrap();
    editor.set_active_tool(Some(tool.id())).unwrap();
    calls.borrow_mut().clear();

    {
        let mut accessor = editor.tool_mut(tool.id()).unwrap();

        assert!(accessor.pointer_down(&down));
        assert!(accessor.pointer_move(&moved));
        assert!(!accessor.pointer_up(&up));
        accessor.confirm();
    }

    assert!(!editor.active_tool_mut().unwrap().pointer_click(&click));
    editor.active_tool_mut().unwrap().cancel();
    editor.active_tool_mut().unwrap().cancel();

    assert_tool_calls(
        &calls,
        &[
            ("tool", "pointer_down"),
            ("tool", "pointer_move"),
            ("tool", "pointer_up"),
            ("tool", "confirm"),
            ("tool", "pointer_click"),
            ("tool", "cancel"),
            ("tool", "cancel"),
        ],
    );

    for call in calls.borrow().iter() {
        assert_eq!(call.mode, EditMode::EDGE);
        assert_eq!(call.meshes, vec![(active, ComponentMask::EDGE)]);
    }

    let expected = |input: &PointerInput<'_>, pick| ReceivedInput {
        modifiers: input.modifiers,
        pick,
        ray: input.ray,
        scene: input
            .scene
            .map(|scene| (scene.view_direction, mesh_transforms.as_ptr())),
    };

    assert_eq!(
        editor.tool_config_mut(&tool).unwrap().received_inputs,
        [
            expected(&down, PickResult::NotPerformed),
            expected(&moved, PickResult::NotPerformed),
            expected(&up, PickResult::NotPerformed),
            expected(
                &click,
                PickResult::Hit {
                    mesh: active,
                    component: edge
                }
            ),
        ]
    );

    assert!(!editor.is_mesh_active(inactive));
    assert_eq!(editor.active_tool(), Some(tool.id()));
}

#[test]
fn tool_activation_errors_report_tool_and_mode() {
    let mut editor = MeshEditor::new();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut tool = RecordingTool::new("tool", &calls);

    tool.supported_mode = Some(EditMode::FACE);

    let tool = editor.add_tool(tool);
    let missing = ToolId::new();
    let mode = editor.mode();

    for (id, expected) in [
        (missing, format!("tool {missing:?} is not registered")),
        (
            tool.id(),
            format!("tool {:?} does not support edit mode {mode:?}", tool.id()),
        ),
    ] {
        let error: crate::ToolActivationError = editor.set_active_tool(Some(id)).unwrap_err();
        let error: &dyn std::error::Error = &error;

        assert_eq!(error.to_string(), expected);
        assert_eq!(editor.active_tool(), None);
    }

    assert!(calls.borrow().is_empty());
}
