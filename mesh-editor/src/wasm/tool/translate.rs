//! JS adapters for [`TranslateTool`].

use wasm_bindgen::prelude::*;

use super::BoxedTool;
use crate::TranslateTool;

#[wasm_bindgen]
impl TranslateTool {
    /// Creates a translate tool to register with `MeshEditor.addTool`.
    pub fn create() -> BoxedTool {
        BoxedTool::new(TranslateTool::default())
    }
}
