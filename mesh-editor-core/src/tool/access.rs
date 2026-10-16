use mesh_core::ComponentKey;
use slotmap::SecondaryMap;
use topology_overlay::TopologyOverlay;

use super::{PointerInput, Tool, ToolContext};
use crate::MeshKey;

/// Short-lived access to a tool and its editor's mesh data.
/// Obtained from `MeshEditor`; callers do not construct the context.
/// Pointer methods borrow renderer input only for the duration of the call, resolving its pick
/// against the editor's overlays before the tool sees it.
pub struct ToolAccessor<'editor> {
    pub(crate) tool: &'editor mut dyn Tool,
    pub(crate) context: ToolContext<'editor>,
    pub(crate) overlays: &'editor SecondaryMap<MeshKey, TopologyOverlay>,
}

impl ToolAccessor<'_> {
    /// Handles a pointer press. Returns whether it was handled.
    pub fn pointer_down(&mut self, input: &PointerInput<'_>) -> bool {
        self.forward(input, Tool::pointer_down)
    }

    /// Handles a pointer release, applying final movement before finishing the drag.
    /// Returns whether it was handled.
    pub fn pointer_up(&mut self, input: &PointerInput<'_>) -> bool {
        self.forward(input, Tool::pointer_up)
    }

    /// Handles pointer movement. Returns whether it was handled.
    pub fn pointer_move(&mut self, input: &PointerInput<'_>) -> bool {
        self.forward(input, Tool::pointer_move)
    }

    /// Handles a click. Returns whether it was handled.
    pub fn pointer_click(&mut self, input: &PointerInput<'_>) -> bool {
        self.forward(input, Tool::pointer_click)
    }

    /// Applies any pending preview and confirms the tool's current interaction.
    pub fn confirm(&mut self) {
        self.tool.confirm(&mut self.context);
    }

    /// Cancels the tool's current interaction, including on renderer-reported pointer loss.
    pub fn cancel(&mut self) {
        self.tool.cancel(&mut self.context);
    }

    /// Resolves the input's pick against the overlays, then calls `handler` on the tool.
    fn forward(&mut self, input: &PointerInput<'_>, handler: PointerHandler) -> bool {
        let input = input.resolve(self.overlays);

        handler(self.tool, &mut self.context, &input)
    }
}

type PointerHandler =
    fn(&mut dyn Tool, &mut ToolContext<'_>, &PointerInput<'_, ComponentKey>) -> bool;
