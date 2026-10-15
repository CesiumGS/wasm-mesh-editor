use super::{PointerInput, Tool, ToolContext};

/// Short-lived access to a tool and its editor's mesh data.
/// Obtained from `MeshEditor`; callers do not construct the context.
/// Pointer methods borrow renderer input only for the duration of the call.
pub struct ToolAccessor<'editor> {
    pub(crate) tool: &'editor mut dyn Tool,
    pub(crate) context: ToolContext<'editor>,
}

impl ToolAccessor<'_> {
    /// Handles a pointer press. Returns whether it was handled.
    pub fn pointer_down(&mut self, input: &PointerInput<'_>) -> bool {
        self.tool.pointer_down(&mut self.context, input)
    }

    /// Handles a pointer release, applying final movement before finishing the drag.
    /// Returns whether it was handled.
    pub fn pointer_up(&mut self, input: &PointerInput<'_>) -> bool {
        self.tool.pointer_up(&mut self.context, input)
    }

    /// Handles pointer movement. Returns whether it was handled.
    pub fn pointer_move(&mut self, input: &PointerInput<'_>) -> bool {
        self.tool.pointer_move(&mut self.context, input)
    }

    /// Handles a click. Returns whether it was handled.
    pub fn pointer_click(&mut self, input: &PointerInput<'_>) -> bool {
        self.tool.pointer_click(&mut self.context, input)
    }

    /// Applies any pending preview and confirms the tool's current interaction.
    pub fn confirm(&mut self) {
        self.tool.confirm(&mut self.context);
    }

    /// Cancels the tool's current interaction, including on renderer-reported pointer loss.
    pub fn cancel(&mut self) {
        self.tool.cancel(&mut self.context);
    }
}
