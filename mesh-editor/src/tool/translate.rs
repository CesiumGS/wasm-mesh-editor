use std::any::Any;

use glam::DVec3;
use mesh::{ComponentKey, Plane};

use crate::{EditMode, MeshTransform, PointerInput, Tool, ToolContext};

use super::common::{DragState, collect_active_mesh_transforms, selection_centroid};

struct TranslationDrag {
    plane: Plane,
    state: DragState,
}

impl TranslationDrag {
    fn cancellation_delta(&self) -> DVec3 {
        self.state.start_position - self.state.last_update_position
    }
}

/// Translates selected vertices along a fixed, camera-facing world plane,
/// whose position is derived from the selection centroid of the participating meshes.
/// Scene context is required at drag start; its plane and placements are retained until completion.
#[derive(Default)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub struct TranslateTool {
    meshes: Vec<MeshTransform>,
    drag: Option<TranslationDrag>,
}

impl TranslateTool {
    fn begin(
        &mut self,
        context: &ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> Option<TranslationDrag> {
        let ray = input.ray?;
        let scene = input.scene?;

        collect_active_mesh_transforms(context, scene.mesh_transforms, &mut self.meshes)?;

        let anchor = selection_centroid(context, &self.meshes)?;
        let plane = Plane::new(anchor, scene.view_direction)?;
        let start_position = plane.intersect(ray)?;

        Some(TranslationDrag {
            plane,
            state: DragState::new(start_position),
        })
    }

    fn apply_drag(&mut self, context: &mut ToolContext<'_>) {
        let Some(drag) = &mut self.drag else {
            return;
        };

        let world_delta = drag.state.update();

        if world_delta == DVec3::ZERO {
            return;
        }

        translate_meshes(context, &self.meshes, world_delta);
    }

    fn clear_interaction(&mut self) {
        self.drag = None;
        self.meshes.clear();
    }
}

impl Tool for TranslateTool {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn supports_edit_mode(&self, mode: EditMode) -> bool {
        !mode.selection_level.is_empty()
    }

    fn activate(&mut self, _context: &mut ToolContext<'_>) {}

    fn deactivate(&mut self, context: &mut ToolContext<'_>) {
        self.cancel(context);
    }

    fn pointer_down(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        if self.drag.is_some() || !self.supports_edit_mode(context.mode()) {
            return false;
        }

        let Some(drag) = self.begin(context, input) else {
            self.meshes.clear();
            return false;
        };

        self.drag = Some(drag);

        true
    }

    fn pointer_up(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        if self.drag.is_none() {
            return false;
        }

        self.pointer_move(context, input);
        self.confirm(context);

        true
    }

    fn pointer_move(
        &mut self,
        _context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        let Some(drag) = &mut self.drag else {
            return false;
        };

        let Some(ray) = input.ray else {
            return false;
        };

        drag.state.update_position(drag.plane.intersect(ray));

        true
    }

    fn pointer_click(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        false
    }

    fn confirm(&mut self, context: &mut ToolContext<'_>) {
        self.apply_drag(context);
        self.clear_interaction();
    }

    fn cancel(&mut self, context: &mut ToolContext<'_>) {
        let Some(drag) = &self.drag else {
            return;
        };

        let world_delta = drag.cancellation_delta();

        if world_delta == DVec3::ZERO {
            self.clear_interaction();
            return;
        }

        translate_meshes(context, &self.meshes, world_delta);

        self.clear_interaction();
    }

    /// Apply movement since the last editor update.
    fn update(&mut self, context: &mut ToolContext<'_>, _delta_seconds: f64) {
        self.apply_drag(context);
    }
}

fn translate_meshes(
    context: &mut ToolContext<'_>,
    transforms: &[MeshTransform],
    world_delta: DVec3,
) {
    for transform in transforms {
        let mesh = context
            .mesh_mut(transform.mesh)
            .expect("translation mesh must remain active during a drag");

        mesh.translate_selected(transform.transform_world_vec(world_delta));
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tool/translate.rs"]
mod tests;
