//! JS adapters for selection edits on the meshes in a [`MeshEditor`].
//!
//! Each edit returns `false` and does nothing if the mesh is inactive, and throws for an unknown
//! mesh key. Unknown component keys are ignored.

use glam::{Vec2, Vec3, Vec4};
use mesh::{AttributeLookupError, AttributeMap, AttributeValue, Mesh, Selection, SelectionStep};
use serde::de::DeserializeOwned;
use slotmap::Key;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use super::{ComponentKeysJs, active_mesh};
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

fn ffi(key: impl Key) -> u64 {
    key.data().as_ffi()
}

/// What `selectVertsByAttribute` did. Only `Selected` changes the selection.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectByAttributeResult {
    /// The attribute was found and any matching vertices were selected. That may be none,
    /// if nothing matched or vertices aren't selectable in the current mode.
    Selected = 0,
    /// The mesh has no vertex attribute with that name.
    AttributeNotFound = 1,
    /// The value doesn't fit the attribute's type, like `1.5` for an integer attribute or a
    /// number for a vector attribute.
    WrongType = 2,
    /// The mesh isn't active, so it can't be edited.
    MeshInactive = 3,
    /// The comparison doesn't apply to the attribute's type: vector attributes only support
    /// `Equal` and `NotEqual`.
    UnsupportedComparison = 4,
}

/// How `selectVertsByAttribute` compares each vertex's attribute value to the given value.
/// Float comparisons follow IEEE 754, so a NaN only matches `NotEqual`.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeComparison {
    Equal = 0,
    NotEqual = 1,
    Less = 2,
    LessOrEqual = 3,
    Greater = 4,
    GreaterOrEqual = 5,
}

#[wasm_bindgen]
impl MeshEditor {
    /// Selects every vertex in an active mesh whose `name` attribute compares to `value` as
    /// `comparison` says, like selecting them directly. Pass a number for scalar attributes and an
    /// array of numbers for vector ones. Only throws for an unknown mesh key.
    #[wasm_bindgen(js_name = selectVertsByAttribute)]
    pub fn select_verts_by_attribute(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        comparison: AttributeComparison,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
    ) -> Result<SelectByAttributeResult, JsError> {
        if !self.has_mesh(mesh_key) {
            return Err(MeshNotFound { key: mesh_key }.into());
        }

        let Some(mesh) = self.active_mesh_mut(mesh_key) else {
            return Ok(SelectByAttributeResult::MeshInactive);
        };

        Ok(select_verts_by_js_value(mesh, name, comparison, &value))
    }
}

/// Selects the vertices whose `name` attribute compares to `value`, converted to the attribute's
/// stored type, as `comparison` says.
fn select_verts_by_js_value(
    mesh: &mut Mesh,
    name: &str,
    comparison: AttributeComparison,
    value: &JsValue,
) -> SelectByAttributeResult {
    let Some(attribute) = mesh.attributes().vertex_attribute(name) else {
        return SelectByAttributeResult::AttributeNotFound;
    };

    match attribute {
        AttributeMap::F32(_) => select_verts(
            mesh,
            name,
            scalar_predicate(comparison, from_js::<f32>(value)),
        ),
        AttributeMap::Vec2(_) => select_verts(
            mesh,
            name,
            vector_predicate(comparison, floats_from_js(value).map(Vec2::from_array)),
        ),
        AttributeMap::Vec3(_) => select_verts(
            mesh,
            name,
            vector_predicate(comparison, floats_from_js(value).map(Vec3::from_array)),
        ),
        AttributeMap::Vec4(_) => select_verts(
            mesh,
            name,
            vector_predicate(comparison, floats_from_js(value).map(Vec4::from_array)),
        ),
        AttributeMap::U32(_) => select_verts(
            mesh,
            name,
            scalar_predicate(comparison, from_js::<u32>(value)),
        ),
        AttributeMap::I32(_) => select_verts(
            mesh,
            name,
            scalar_predicate(comparison, from_js::<i32>(value)),
        ),
    }
}

fn select_verts<T: AttributeValue>(
    mesh: &mut Mesh,
    name: &str,
    predicate: Result<impl Fn(T) -> bool, SelectByAttributeResult>,
) -> SelectByAttributeResult {
    let predicate = match predicate {
        Ok(predicate) => predicate,
        Err(result) => return result,
    };

    match mesh
        .selection_mut()
        .select_verts_by_attribute(name, predicate)
    {
        Ok(()) => SelectByAttributeResult::Selected,
        Err(AttributeLookupError::NotFound { .. }) => SelectByAttributeResult::AttributeNotFound,
        Err(AttributeLookupError::WrongType { .. }) => SelectByAttributeResult::WrongType,
    }
}

/// A predicate comparing values to `value`. `WrongType` if `value` is `None`, meaning the JS
/// value didn't convert.
fn scalar_predicate<T: PartialOrd + Copy>(
    comparison: AttributeComparison,
    value: Option<T>,
) -> Result<impl Fn(T) -> bool, SelectByAttributeResult> {
    let value = value.ok_or(SelectByAttributeResult::WrongType)?;

    Ok(move |vert_value: T| match comparison {
        AttributeComparison::Equal => vert_value == value,
        AttributeComparison::NotEqual => vert_value != value,
        AttributeComparison::Less => vert_value < value,
        AttributeComparison::LessOrEqual => vert_value <= value,
        AttributeComparison::Greater => vert_value > value,
        AttributeComparison::GreaterOrEqual => vert_value >= value,
    })
}

/// Like [`scalar_predicate`], for vector types: `UnsupportedComparison` for anything but `Equal`
/// and `NotEqual`.
fn vector_predicate<T: PartialEq + Copy>(
    comparison: AttributeComparison,
    value: Option<T>,
) -> Result<impl Fn(T) -> bool, SelectByAttributeResult> {
    let matches_equal = match comparison {
        AttributeComparison::Equal => true,
        AttributeComparison::NotEqual => false,
        _ => return Err(SelectByAttributeResult::UnsupportedComparison),
    };
    let value = value.ok_or(SelectByAttributeResult::WrongType)?;

    Ok(move |vert_value: T| (vert_value == value) == matches_equal)
}

/// `value` as `T`, or `None` if it doesn't fit, like a fractional number for an integer.
fn from_js<T: DeserializeOwned>(value: &JsValue) -> Option<T> {
    serde_wasm_bindgen::from_value(value.clone()).ok()
}

/// `value` as exactly `N` floats, from an array or typed array.
fn floats_from_js<const N: usize>(value: &JsValue) -> Option<[f32; N]> {
    // Going through a `Vec` rejects extra elements, which deserializing `[f32; N]` ignores.
    from_js::<Vec<f32>>(value)?.try_into().ok()
}
