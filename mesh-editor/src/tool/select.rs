use std::any::Any;

use mesh::{ComponentKey, Mesh};

use crate::{EditMode, MeshKey, PickResult, PointerInput, Tool, ToolContext};

/// A tool to enable selection of mesh components or meshes (in EditMode.NONE mode).
#[derive(Debug, Default)]
pub struct SelectTool;

impl SelectTool {
    fn validate_hit<'context>(
        context: &'context mut ToolContext<'_>,
        mesh: MeshKey,
        component: ComponentKey,
    ) -> Option<(&'context mut Mesh, ComponentKey)> {
        if !context
            .mode()
            .selection_level
            .contains(component.kind().into())
        {
            return None;
        }

        let mesh = context.mesh_mut(mesh)?;
        mesh.component(component)?;

        Some((mesh, component))
    }

    fn click_meshes(context: &mut ToolContext<'_>, input: &PointerInput<'_, ComponentKey>) -> bool {
        let clicked = match input.pick {
            PickResult::NotPerformed => return false,
            PickResult::Miss => None,
            PickResult::Hit { mesh, .. } => Some(mesh),
        };

        match (clicked, input.modifiers.shift) {
            (None, true) => false,
            (Some(mesh), true) if context.is_mesh_active(mesh) => {
                context.retain_active_meshes(|&key| key != mesh);
                true
            }
            (Some(mesh), true) => context.set_mesh_active(mesh).is_ok(),
            (clicked, false) => {
                context.retain_active_meshes(|&key| Some(key) == clicked);
                // An unknown mesh counts as a miss, which the retain above already handled
                clicked.map(|mesh| context.set_mesh_active(mesh));
                true
            }
        }
    }

    fn click_components(
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        let hit = match input.pick {
            PickResult::NotPerformed => return false,
            PickResult::Miss => None,
            PickResult::Hit { mesh, component } => Self::validate_hit(context, mesh, component),
        };

        match (hit, input.modifiers.shift) {
            // Shift-click on empty space does not clear selection
            (None, true) => return false,
            (None, false) => Self::clear_selection(context),
            (Some((mesh, component)), true) => mesh.selection_mut().toggle(&[component]),
            (Some((mesh, component)), false) => mesh.selection_mut().set(&[component]),
        }

        true
    }

    fn clear_selection(context: &mut ToolContext<'_>) {
        for mesh in context.active_meshes_mut() {
            mesh.selection_mut().clear();
        }
    }
}

impl Tool for SelectTool {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn supports_edit_mode(&self, _mode: EditMode) -> bool {
        true
    }

    fn activate(&mut self, _context: &mut ToolContext<'_>) {}

    fn deactivate(&mut self, _context: &mut ToolContext<'_>) {}

    fn pointer_down(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        false
    }

    fn pointer_up(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        false
    }

    fn pointer_move(
        &mut self,
        _context: &mut ToolContext<'_>,
        _input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        false
    }

    fn pointer_click(
        &mut self,
        context: &mut ToolContext<'_>,
        input: &PointerInput<'_, ComponentKey>,
    ) -> bool {
        if context.mode().selection_level.is_empty() {
            Self::click_meshes(context, input)
        } else {
            Self::click_components(context, input)
        }
    }

    fn confirm(&mut self, _context: &mut ToolContext<'_>) {}

    fn cancel(&mut self, _context: &mut ToolContext<'_>) {}

    fn update(&mut self, _context: &mut ToolContext<'_>, _delta_seconds: f64) {}
}

#[cfg(test)]
#[path = "../../tests/unit/tool/select.rs"]
mod tests;
