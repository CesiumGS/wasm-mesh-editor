//! JS adapters for selection edits on the meshes in a [`MeshEditor`].
//!
//! Each edit returns `false` and does nothing if the mesh is inactive, and throws for an unknown
//! mesh key. Unknown component keys are ignored.

use mesh::{
    AttributeError, AttributeKey, AttributeValue, IntoComponentKey, Mesh, Selection, SelectionStep,
};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use super::attributes::{
    AttributeComparison, AttributeResult, ByAttributeOperation, apply_by_js_value,
};
use super::{ComponentKeysJs, active_mesh, ffi};
use crate::{MeshEditor, MeshKey, MeshNotFound};

#[wasm_bindgen]
impl MeshEditor {
    /// A mesh's selected components, in no particular order. Works on inactive meshes. Throws for
    /// an unknown mesh key.
    #[wasm_bindgen(js_name = selectedComponents)]
    pub fn selected_components(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
    ) -> Result<Ts<ComponentKeysJs>, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;
        let selection = mesh.selection();

        Ok(ComponentKeysJs {
            verts: selection.verts().map(ffi).collect(),
            edges: selection.edges().map(ffi).collect(),
            faces: selection.faces().map(ffi).collect(),
        }
        .into_ts()?)
    }

    /// Replaces an active mesh's selection with `componentKeys`.
    #[wasm_bindgen(js_name = setSelection)]
    pub fn set_selection(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = componentKeys)] component_keys: Ts<ComponentKeysJs>,
    ) -> Result<bool, JsError> {
        let keys = ComponentKeysJs::to_component_keys(&component_keys)?;
        edit_selection(self, mesh_key, |selection| selection.set(&keys))
    }

    /// Adds `componentKeys` to an active mesh's selection.
    #[wasm_bindgen(js_name = selectComponents)]
    pub fn select_components(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = componentKeys)] component_keys: Ts<ComponentKeysJs>,
    ) -> Result<bool, JsError> {
        let keys = ComponentKeysJs::to_component_keys(&component_keys)?;
        edit_selection(self, mesh_key, |selection| selection.select(&keys))
    }

    /// Removes `componentKeys` from an active mesh's selection.
    #[wasm_bindgen(js_name = deselectComponents)]
    pub fn deselect_components(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = componentKeys)] component_keys: Ts<ComponentKeysJs>,
    ) -> Result<bool, JsError> {
        let keys = ComponentKeysJs::to_component_keys(&component_keys)?;
        edit_selection(self, mesh_key, |selection| selection.deselect(&keys))
    }

    /// Toggles `componentKeys` in an active mesh's selection.
    #[wasm_bindgen(js_name = toggleComponents)]
    pub fn toggle_components(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = componentKeys)] component_keys: Ts<ComponentKeysJs>,
    ) -> Result<bool, JsError> {
        let keys = ComponentKeysJs::to_component_keys(&component_keys)?;
        edit_selection(self, mesh_key, |selection| selection.toggle(&keys))
    }

    /// Clears an active mesh's selection.
    #[wasm_bindgen(js_name = clearSelection)]
    pub fn clear_selection(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
    ) -> Result<bool, JsError> {
        edit_selection(self, mesh_key, |selection| selection.clear())
    }

    /// Grows an active mesh's selection by one layer of neighbors.
    #[wasm_bindgen(js_name = growSelection)]
    pub fn grow_selection(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        step: SelectionStep,
    ) -> Result<bool, JsError> {
        edit_selection(self, mesh_key, |selection| selection.grow(step))
    }

    /// Shrinks an active mesh's selection by one layer from its boundary.
    #[wasm_bindgen(js_name = shrinkSelection)]
    pub fn shrink_selection(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        step: SelectionStep,
    ) -> Result<bool, JsError> {
        edit_selection(self, mesh_key, |selection| selection.shrink(step))
    }
}

/// Applies `edit` to an active mesh's selection. `false` if the mesh is inactive.
fn edit_selection(
    editor: &mut MeshEditor,
    mesh_key: MeshKey,
    edit: impl FnOnce(&mut Selection<'_>),
) -> Result<bool, JsError> {
    let Some(mesh) = active_mesh(editor, mesh_key)? else {
        return Ok(false);
    };

    edit(&mut mesh.selection_mut());
    Ok(true)
}

#[wasm_bindgen]
impl MeshEditor {
    /// Selects every vertex, edge or face in an active mesh whose `name` attribute compares to
    /// `value` as `comparison` says, like selecting them directly. Pass a number for scalar
    /// attributes and an array of numbers for vector ones. Only throws for an unknown mesh key.
    #[wasm_bindgen(js_name = selectByAttribute)]
    pub fn select_by_attribute(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        comparison: AttributeComparison,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
    ) -> Result<AttributeResult, JsError> {
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(AttributeResult::MeshInactive);
        };

        Ok(apply_by_js_value(mesh, name, comparison, &value, &Select))
    }
}

struct Select;

impl ByAttributeOperation for Select {
    fn apply<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &self,
        mesh: &mut Mesh,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeError> {
        mesh.selection_mut()
            .select_by_attribute::<K, T>(name, predicate)
    }
}
