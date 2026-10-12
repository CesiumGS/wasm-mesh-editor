use std::any::Any;
use std::marker::PhantomData;

mod access;
mod context;
mod input;

pub use access::ToolAccessor;
pub use context::ToolContext;
pub use input::PointerInput;

use crate::EditMode;

/// Identifies an editor-owned tool. Identity storage is deferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolId {
    _private: (),
}

/// Identifies an editor-owned tool while retaining its concrete type.
pub struct ToolHandle<ToolType: Tool> {
    id: ToolId,
    marker: PhantomData<fn() -> ToolType>,
}

impl<ToolType: Tool> ToolHandle<ToolType> {
    /// Returns the ID used for activation and input dispatch.
    pub fn id(&self) -> ToolId {
        self.id
    }
}

/// Tool behavior. The editor supplies mesh access for each call.
pub trait Tool: Any {
    /// Returns this tool for checked, concrete-type configuration access.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn supports_edit_mode(&self, mode: EditMode) -> bool;

    fn activate(&mut self, context: &mut ToolContext<'_>);

    fn deactivate(&mut self, context: &mut ToolContext<'_>);

    fn pointer_down(&mut self, context: &mut ToolContext<'_>, input: &PointerInput) -> bool;

    fn pointer_up(&mut self, context: &mut ToolContext<'_>, input: &PointerInput) -> bool;

    fn pointer_move(&mut self, context: &mut ToolContext<'_>, input: &PointerInput) -> bool;

    fn pointer_click(&mut self, context: &mut ToolContext<'_>, input: &PointerInput) -> bool;

    fn confirm(&mut self, context: &mut ToolContext<'_>);

    fn cancel(&mut self, context: &mut ToolContext<'_>);

    fn update(&mut self, context: &mut ToolContext<'_>, delta_seconds: f64);
}
