//! JS adapters for selection edits on the meshes in a [`MeshEditor`].

use glam::{Vec2, Vec3, Vec4};
use mesh::{AttributeLookupError, AttributeMap, AttributeValue, Mesh};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

use crate::{MeshEditor, MeshKey, MeshNotFound};

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
}

#[wasm_bindgen]
impl MeshEditor {
    /// Selects every vertex in an active mesh whose `name` attribute equals `value`, like
    /// selecting them directly. Pass a number for scalar attributes and an array of numbers
    /// for vector ones. Only throws for an unknown mesh key.
    #[wasm_bindgen(js_name = selectVertsByAttribute)]
    pub fn select_verts_by_attribute(
        &mut self,
        key: MeshKey,
        name: &str,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
    ) -> Result<SelectByAttributeResult, JsError> {
        if !self.has_mesh(key) {
            return Err(MeshNotFound { key }.into());
        }

        let Some(mesh) = self.active_mesh_mut(key) else {
            return Ok(SelectByAttributeResult::MeshInactive);
        };

        Ok(select_verts_by_js_value(mesh, name, &value))
    }
}

/// Selects the vertices whose `name` attribute equals `value`, converted to the attribute's
/// stored type.
fn select_verts_by_js_value(
    mesh: &mut Mesh,
    name: &str,
    value: &JsValue,
) -> SelectByAttributeResult {
    let Some(attribute) = mesh.attributes().vertex_attribute(name) else {
        return SelectByAttributeResult::AttributeNotFound;
    };

    match attribute {
        AttributeMap::F32(_) => select_verts(mesh, name, from_js::<f32>(value)),
        AttributeMap::Vec2(_) => {
            select_verts(mesh, name, floats_from_js(value).map(Vec2::from_array))
        }
        AttributeMap::Vec3(_) => {
            select_verts(mesh, name, floats_from_js(value).map(Vec3::from_array))
        }
        AttributeMap::Vec4(_) => {
            select_verts(mesh, name, floats_from_js(value).map(Vec4::from_array))
        }
        AttributeMap::U32(_) => select_verts(mesh, name, from_js::<u32>(value)),
        AttributeMap::I32(_) => select_verts(mesh, name, from_js::<i32>(value)),
    }
}

/// `WrongType` if `value` is `None`, meaning the JS value didn't convert.
fn select_verts<T: AttributeValue + PartialEq>(
    mesh: &mut Mesh,
    name: &str,
    value: Option<T>,
) -> SelectByAttributeResult {
    let Some(value) = value else {
        return SelectByAttributeResult::WrongType;
    };

    match mesh.selection_mut().select_verts_by_attribute(name, value) {
        Ok(()) => SelectByAttributeResult::Selected,
        Err(AttributeLookupError::NotFound { .. }) => SelectByAttributeResult::AttributeNotFound,
        Err(AttributeLookupError::WrongType { .. }) => SelectByAttributeResult::WrongType,
    }
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
