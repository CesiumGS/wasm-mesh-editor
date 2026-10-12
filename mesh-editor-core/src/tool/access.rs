use super::{PointerInput, Tool, ToolContext};

/// Short-lived access to a tool and its editor's mesh data.
/// Obtained from `MeshEditor`; callers do not construct the context.
#[allow(dead_code)]
pub struct ToolAccessor<'editor> {
    pub(crate) tool: &'editor mut dyn Tool,
    pub(crate) context: ToolContext<'editor>,
}

#[allow(unused_variables)]
impl ToolAccessor<'_> {
    /// Handles a pointer press. Returns whether it was handled.
    pub fn pointer_down(&mut self, input: &PointerInput) -> bool {
        todo!()
    }

    /// Handles a pointer release. Returns whether it was handled.
    pub fn pointer_up(&mut self, input: &PointerInput) -> bool {
        todo!()
    }

    /// Handles pointer movement. Returns whether it was handled.
    pub fn pointer_move(&mut self, input: &PointerInput) -> bool {
        todo!()
    }

    /// Handles a click. Returns whether it was handled.
    pub fn pointer_click(&mut self, input: &PointerInput) -> bool {
        todo!()
    }

    /// Confirms the tool's current interaction.
    pub fn confirm(&mut self) {
        todo!()
    }

    /// Cancels the tool's current interaction.
    pub fn cancel(&mut self) {
        todo!()
    }
}
