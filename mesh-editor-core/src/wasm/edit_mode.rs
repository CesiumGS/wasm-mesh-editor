//! JS adapters for [`EditMode`].

use mesh_core::{ComponentMask, ComponentType};
use wasm_bindgen::prelude::*;

use crate::EditMode;

#[wasm_bindgen]
impl EditMode {
    /// [`EditMode::NONE`]: nothing is selectable.
    #[wasm_bindgen(getter = NONE)]
    pub fn none_mode() -> EditMode {
        EditMode::NONE
    }

    /// [`EditMode::VERTEX`]
    #[wasm_bindgen(getter = VERTEX)]
    pub fn vertex_mode() -> EditMode {
        EditMode::VERTEX
    }

    /// [`EditMode::EDGE`]
    #[wasm_bindgen(getter = EDGE)]
    pub fn edge_mode() -> EditMode {
        EditMode::EDGE
    }

    /// [`EditMode::FACE`]
    #[wasm_bindgen(getter = FACE)]
    pub fn face_mode() -> EditMode {
        EditMode::FACE
    }

    /// Whether the renderer should draw this component kind.
    #[wasm_bindgen(js_name = isRenderable)]
    pub fn is_renderable(&self, kind: ComponentType) -> bool {
        self.renderable_components
            .contains(ComponentMask::from(kind))
    }

    /// Whether this component kind can be selected directly.
    #[wasm_bindgen(js_name = isSelectable)]
    pub fn is_selectable(&self, kind: ComponentType) -> bool {
        self.selection_level.contains(ComponentMask::from(kind))
    }

    /// Compares policies; each JS read returns a new object, so `===` never matches.
    pub fn equals(&self, other: &EditMode) -> bool {
        self == other
    }
}
