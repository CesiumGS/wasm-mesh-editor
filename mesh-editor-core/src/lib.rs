//! Coordinates editable meshes and tools without owning input listeners or a render loop.
//!
//! Interface draft: all methods are unimplemented. Tool identity storage and pointer data
//! are placeholders; undo and redo are out of scope.
//! Policy for changing tools, modes, or target meshes during an unfinished
//! interaction is undecided.

mod edit_mode;
#[path = "mesh-editor.rs"]
mod mesh_editor;
mod tool;

pub use edit_mode::EditMode;
pub use mesh_editor::{MeshEditor, MeshKey};
pub use tool::{PointerInput, Tool, ToolAccessor, ToolContext, ToolHandle, ToolId};
