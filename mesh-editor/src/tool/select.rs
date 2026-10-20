use std::any::Any;

use mesh::{ComponentKey, Selection};

use crate::{EditMode, MeshKey, Modifiers, PickHit, PickResult, PointerInput, Tool, ToolContext};

/// How a select tool changes the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectAction {
    Replace,
    Add,
    Remove,
    Toggle,
}

/// The [`SelectAction`] to apply for a given modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectModifiersPolicy {
    pub no_modifier: SelectAction,
    pub shift: SelectAction,
    /// Ctrl or Meta (Cmd). Takes precedence over `shift`.
    pub control: SelectAction,
}

impl Default for SelectModifiersPolicy {
    fn default() -> Self {
        Self {
            no_modifier: SelectAction::Replace,
            shift: SelectAction::Toggle,
            control: SelectAction::Toggle,
        }
    }
}

impl SelectModifiersPolicy {
    /// The action for `modifiers`.
    pub fn action(&self, modifiers: Modifiers) -> SelectAction {
        match modifiers {
            Modifiers { control: true, .. } | Modifiers { meta: true, .. } => self.control,
            Modifiers { shift: true, .. } => self.shift,
            _ => self.no_modifier,
        }
    }
}

/// A tool to enable selection of mesh components or meshes (in EditMode.NONE mode).
#[derive(Debug, Default)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub struct SelectTool {
    pub modifiers_policy: SelectModifiersPolicy,
}

impl SelectTool {
    /// Applies `action` to which meshes are active, using the meshes in `mesh_hits`.
    fn select_meshes(
        context: &mut ToolContext<'_>,
        action: SelectAction,
        mesh_hits: &[PickHit<'_>],
    ) {
        let is_mesh_hit =
            |mesh_key: &MeshKey| mesh_hits.iter().any(|mesh_hit| mesh_hit.mesh == *mesh_key);
        let hit_mesh_keys = mesh_hits.iter().map(|mesh_hit| mesh_hit.mesh);

        match action {
            SelectAction::Replace => {
                context.retain_active_meshes(is_mesh_hit);
                Self::activate_meshes(context, hit_mesh_keys);
            }
            SelectAction::Add => Self::activate_meshes(context, hit_mesh_keys),
            SelectAction::Remove => context.retain_active_meshes(|mesh_key| !is_mesh_hit(mesh_key)),
            SelectAction::Toggle => {
                let (active, inactive): (Vec<_>, Vec<_>) =
                    hit_mesh_keys.partition(|&mesh_key| context.is_mesh_active(mesh_key));

                context.retain_active_meshes(|mesh_key| !active.contains(mesh_key));
                Self::activate_meshes(context, inactive);
            }
        }
    }

    /// Applies `action` to the selections of active meshes, using the components in `mesh_hits`.
    fn select_components(
        context: &mut ToolContext<'_>,
        action: SelectAction,
        mesh_hits: &[PickHit<'_>],
    ) {
        let edit: fn(&mut Selection<'_>, &[ComponentKey]) = match action {
            SelectAction::Replace => |selection, components| selection.set(components),
            SelectAction::Add => |selection, components| selection.select(components),
            SelectAction::Remove => |selection, components| selection.deselect(components),
            SelectAction::Toggle => |selection, components| selection.toggle(components),
        };

        if action == SelectAction::Replace {
            Self::clear_meshes_without_hits(context, mesh_hits);
        }

        for mesh_hit in mesh_hits {
            if let Some(mesh) = context.mesh_mut(mesh_hit.mesh) {
                edit(&mut mesh.selection_mut(), mesh_hit.components);
            }
        }
    }

    /// Clears the selections of active meshes that aren't in `mesh_hits`.
    fn clear_meshes_without_hits(context: &mut ToolContext<'_>, mesh_hits: &[PickHit<'_>]) {
        for mesh_key in context.active_meshes().to_vec() {
            if mesh_hits.iter().any(|mesh_hit| mesh_hit.mesh == mesh_key) {
                continue;
            }

            if let Some(mesh) = context.mesh_mut(mesh_key) {
                mesh.selection_mut().clear();
            }
        }
    }

    fn activate_meshes(
        context: &mut ToolContext<'_>,
        mesh_keys: impl IntoIterator<Item = MeshKey>,
    ) {
        for mesh_key in mesh_keys {
            // Stale picks can name removed meshes
            let _ = context.set_mesh_active(mesh_key);
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

    fn pointer_down(&mut self, _context: &mut ToolContext<'_>, _input: &PointerInput<'_>) -> bool {
        false
    }

    fn pointer_up(&mut self, _context: &mut ToolContext<'_>, _input: &PointerInput<'_>) -> bool {
        false
    }

    fn pointer_move(&mut self, _context: &mut ToolContext<'_>, _input: &PointerInput<'_>) -> bool {
        false
    }

    fn pointer_click(&mut self, context: &mut ToolContext<'_>, input: &PointerInput<'_>) -> bool {
        let PickResult::Hits(mesh_hits) = input.pick else {
            return false;
        };
        let action = self.modifiers_policy.action(input.modifiers);

        if context.mode().selection_level.is_empty() {
            Self::select_meshes(context, action, mesh_hits);
        } else {
            Self::select_components(context, action, mesh_hits);
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
