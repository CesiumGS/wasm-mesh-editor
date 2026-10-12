//! Coordinates editable meshes and tools without owning input listeners or a render loop.
//!
//! Implementation is in progress. Pointer data is a placeholder;
//! undo and redo are out of scope.
//! Changes to tools, modes, or active meshes cancel the current interaction first.
//! Invalid tool selections return an error without interrupting the active tool.

mod edit_mode;
#[path = "mesh-editor.rs"]
mod mesh_editor;
mod tool;

pub use edit_mode::EditMode;
pub use mesh_editor::{MeshEditor, MeshKey, MeshNotFound, ToolActivationError};
pub use tool::{PointerInput, Tool, ToolAccessor, ToolContext, ToolHandle, ToolId};
