//! JS adapters for [`SelectTool`].

use wasm_bindgen::prelude::*;

use super::BoxedTool;
use crate::SelectTool;

#[wasm_bindgen]
impl SelectTool {
    /// Creates a select tool to register with `MeshEditor.addTool`.
    pub fn create() -> BoxedTool {
        BoxedTool::new(SelectTool)
    }
}
