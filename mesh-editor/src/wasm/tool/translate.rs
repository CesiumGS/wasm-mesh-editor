//! JS adapters for [`TranslateTool`].

use wasm_bindgen::prelude::*;

use crate::{MeshEditor, ToolId, TranslateTool};

#[wasm_bindgen]
impl TranslateTool {
    /// Adds a new, inactive translate tool to `editor` and returns its ID.
    /// Each call adds a separate tool.
    #[wasm_bindgen(js_name = addTo)]
    pub fn add_to(editor: &mut MeshEditor) -> ToolId {
        editor.add_tool(TranslateTool::default()).id()
    }
}
