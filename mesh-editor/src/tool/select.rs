use std::any::Any;

use mesh::{ComponentKey, Mesh};

use crate::{EditMode, MeshKey, PickResult, PointerInput, Tool, ToolContext};

/// Selects renderer-picked components on click; Shift toggles them.
/// Plain hits replace only the hit mesh's selection. Plain misses clear all active meshes.
/// Hits that can't be selected (inactive or unknown mesh, missing component, or a type outside
/// the selection level) count as misses. Shift-misses and unpicked clicks are ignored.
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

    fn clear_selection(context: &mut ToolContext<'_>) {
        for &key in context.active_meshes() {
            if let Some(mesh) = context.mesh_mut(key) {
                mesh.selection_mut().clear();
            }
        }
    }
}

impl Tool for SelectTool {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn supports_edit_mode(&self, mode: EditMode) -> bool {
        !mode.selection_level.is_empty()
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
        if !self.supports_edit_mode(context.mode()) {
            return false;
        }

        let hit = match input.pick {
            PickResult::NotPerformed => return false,
            PickResult::Miss => None,
            PickResult::Hit { mesh, component } => Self::validate_hit(context, mesh, component),
        };

        let Some((mesh, component)) = hit else {
            // Shift-click on empty space does not clear selection
            if input.modifiers.shift {
                return false;
            }

            Self::clear_selection(context);
            return true;
        };

        if input.modifiers.shift {
            mesh.selection_mut().toggle(&[component]);
        } else {
            mesh.selection_mut().set(&[component]);
        }

        true
    }

    fn confirm(&mut self, _context: &mut ToolContext<'_>) {}

    fn cancel(&mut self, _context: &mut ToolContext<'_>) {}

    fn update(&mut self, _context: &mut ToolContext<'_>, _delta_seconds: f64) {}
}

#[cfg(test)]
#[path = "../../tests/unit/tool/select.rs"]
mod tests;
