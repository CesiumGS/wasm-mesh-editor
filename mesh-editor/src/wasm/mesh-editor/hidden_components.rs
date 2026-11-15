//! JS adapters for hiding and showing components of the meshes in a [`MeshEditor`].

use mesh::{
    AttributeError, AttributeKey, AttributeValue, ComponentKey, EdgeKey, FaceKey, HideOptions,
    IntoComponentKey, Mesh, VertKey,
};
use serde::{Deserialize, Deserializer};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use super::active_mesh;
use super::attributes::{
    AttributeComparison, ByAttributeOperation, ByAttributeResult, apply_by_js_value,
};
use crate::{MeshEditor, MeshKey};

/// Component keys grouped by type, like `OverlayViews.componentKeys`. Omitted types have no keys.
/// Copy keys out of wasm memory first: views over `memory.buffer` can detach during the call.
#[derive(Deserialize, Tsify, Default)]
#[serde(default)]
#[tsify(rename = "ComponentKeys")]
pub struct ComponentKeysJs {
    #[serde(deserialize_with = "big_uint64_array")]
    #[tsify(optional, type = "BigUint64Array")]
    verts: Vec<u64>,
    #[serde(deserialize_with = "big_uint64_array")]
    #[tsify(optional, type = "BigUint64Array")]
    edges: Vec<u64>,
    #[serde(deserialize_with = "big_uint64_array")]
    #[tsify(optional, type = "BigUint64Array")]
    faces: Vec<u64>,
}

/// Options for `hideComponents` and `showComponents`. Omitted options are `false`.
#[derive(Deserialize, Tsify, Default)]
#[serde(default, rename_all = "camelCase")]
#[tsify(rename = "HideOptions")]
pub struct HideOptionsJs {
    /// Also hides or shows faces using any of the given vertices or edges, even if faces aren't
    /// interactable.
    extend_to_connected_faces: bool,
}

#[wasm_bindgen]
impl MeshEditor {
    /// Hides components of an active mesh. Ignores keys of non-interactable types and unknown
    /// keys. Returns `false` and does nothing if the mesh is inactive. Throws for an unknown mesh
    /// key.
    #[wasm_bindgen(js_name = hideComponents)]
    pub fn hide_components(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = componentKeys)] component_keys: Ts<ComponentKeysJs>,
        options: Option<Ts<HideOptionsJs>>,
    ) -> Result<bool, JsError> {
        let (keys, options) = convert_args(&component_keys, options.as_ref())?;
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(false);
        };

        mesh.hidden_components_mut().hide(&keys, options);
        Ok(true)
    }

    /// Shows components of an active mesh, like `hideComponents`.
    #[wasm_bindgen(js_name = showComponents)]
    pub fn show_components(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = componentKeys)] component_keys: Ts<ComponentKeysJs>,
        options: Option<Ts<HideOptionsJs>>,
    ) -> Result<bool, JsError> {
        let (keys, options) = convert_args(&component_keys, options.as_ref())?;
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(false);
        };

        mesh.hidden_components_mut().show(&keys, options);
        Ok(true)
    }

    /// Shows every hidden component of an active mesh, regardless of type.
    /// Returns `false` and does nothing if the mesh is inactive. Throws for an unknown mesh key.
    #[wasm_bindgen(js_name = showAllComponents)]
    pub fn show_all_components(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
    ) -> Result<bool, JsError> {
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(false);
        };

        mesh.hidden_components_mut().show_all();
        Ok(true)
    }

    /// Deselects and hides the selected components of interactable types in an active mesh.
    /// Returns `false` and does nothing if the mesh is inactive. Throws for an unknown mesh key.
    #[wasm_bindgen(js_name = hideSelected)]
    pub fn hide_selected(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        options: Option<Ts<HideOptionsJs>>,
    ) -> Result<bool, JsError> {
        let options = convert_options(options.as_ref())?;
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(false);
        };

        let keys: Vec<_> = mesh.selection().iter().collect();
        mesh.selection_mut().deselect(&keys);
        mesh.hidden_components_mut().hide(&keys, options);
        Ok(true)
    }

    /// Hides every vertex, edge or face in an active mesh whose `name` attribute compares to
    /// `value` as `comparison` says, like `hideComponents`. Pass a number for scalar attributes and
    /// an array of numbers for vector ones. Only throws for an unknown mesh key.
    #[wasm_bindgen(js_name = hideByAttribute)]
    pub fn hide_by_attribute(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        comparison: AttributeComparison,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
        options: Option<Ts<HideOptionsJs>>,
    ) -> Result<ByAttributeResult, JsError> {
        let options = convert_options(options.as_ref())?;
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(ByAttributeResult::MeshInactive);
        };

        Ok(apply_by_js_value(
            mesh,
            name,
            comparison,
            &value,
            &Hide(options),
        ))
    }

    /// Shows every vertex, edge or face in an active mesh whose `name` attribute compares to
    /// `value`, like `hideByAttribute`.
    #[wasm_bindgen(js_name = showByAttribute)]
    pub fn show_by_attribute(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        comparison: AttributeComparison,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
        options: Option<Ts<HideOptionsJs>>,
    ) -> Result<ByAttributeResult, JsError> {
        let options = convert_options(options.as_ref())?;
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(ByAttributeResult::MeshInactive);
        };

        Ok(apply_by_js_value(
            mesh,
            name,
            comparison,
            &value,
            &Show(options),
        ))
    }
}

/// Converts the JS arguments. Keys that no slot map could have issued are dropped.
fn convert_args(
    component_keys: &Ts<ComponentKeysJs>,
    options: Option<&Ts<HideOptionsJs>>,
) -> Result<(Vec<ComponentKey>, HideOptions), JsError> {
    // `Tsify::from_js` rather than `Ts::to_rust`, whose errors name internal Rust types.
    let component_keys = ComponentKeysJs::from_js(component_keys.js_value())?;

    let keys = component_keys
        .verts
        .into_iter()
        .filter_map(|key| VertKey::try_from_ffi(key).map(ComponentKey::from))
        .chain(
            component_keys
                .edges
                .into_iter()
                .filter_map(|key| EdgeKey::try_from_ffi(key).map(ComponentKey::from)),
        )
        .chain(
            component_keys
                .faces
                .into_iter()
                .filter_map(|key| FaceKey::try_from_ffi(key).map(ComponentKey::from)),
        )
        .collect();

    Ok((keys, convert_options(options)?))
}

fn convert_options(options: Option<&Ts<HideOptionsJs>>) -> Result<HideOptions, JsError> {
    let options = match options {
        Some(options) => HideOptionsJs::from_js(options.js_value())?,
        None => HideOptionsJs::default(),
    };

    Ok(HideOptions {
        extend_to_connected_faces: options.extend_to_connected_faces,
    })
}

struct Hide(HideOptions);

impl ByAttributeOperation for Hide {
    fn apply<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &self,
        mesh: &mut Mesh,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeError> {
        mesh.hidden_components_mut()
            .hide_by_attribute::<K, T>(name, predicate, self.0)
    }
}

struct Show(HideOptions);

impl ByAttributeOperation for Show {
    fn apply<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &self,
        mesh: &mut Mesh,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeError> {
        mesh.hidden_components_mut()
            .show_by_attribute::<K, T>(name, predicate, self.0)
    }
}

fn big_uint64_array<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u64>, D::Error> {
    let array: js_sys::BigUint64Array = serde_wasm_bindgen::preserve::deserialize(deserializer)?;

    Ok(array.to_vec())
}
