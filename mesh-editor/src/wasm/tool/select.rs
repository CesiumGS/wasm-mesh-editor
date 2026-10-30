//! JS adapters for [`SelectTool`].

use wasm_bindgen::prelude::*;

use crate::{MeshEditor, SelectTool, ToolId};

#[wasm_bindgen]
impl SelectTool {
    /// Adds a new, inactive select tool to `editor` and returns its ID.
    /// Each call adds a separate tool.
    #[wasm_bindgen(js_name = addTo)]
    pub fn add_to(editor: &mut MeshEditor) -> ToolId {
        editor.add_tool(SelectTool).id()
    }
}
