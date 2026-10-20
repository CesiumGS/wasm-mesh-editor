//! Coordinates editable meshes and tools without owning input listeners or a render loop.
//!
//! The renderer supplies resolved picks, world-space rays, and optional scene context.
//! Implementation is in progress; undo and redo are out of scope.
//! Changes to tools, modes, or active meshes cancel the current interaction first.
//! Invalid tool selections return an error without interrupting the active tool.

mod edit_mode;
#[path = "mesh-editor.rs"]
mod mesh_editor;
mod mesh_transform;
mod tool;
#[cfg(feature = "wasm")]
mod wasm;

pub use edit_mode::EditMode;
/// A ray interpreted in world coordinates by editor tools.
pub use mesh::Ray as WorldRay;
pub use mesh_editor::{MeshEditor, MeshKey, MeshNotFound, ToolActivationError, UpdateResult};
pub use mesh_transform::MeshTransform;
pub use tool::{
    Modifiers, PickResult, PointerInput, SceneInput, SelectTool, Tool, ToolAccessor, ToolContext,
    ToolHandle, ToolId, TranslateTool,
};
pub use topology_overlay::OverlayComponent;
#[cfg(feature = "wasm")]
pub use wasm::BoxedTool;
