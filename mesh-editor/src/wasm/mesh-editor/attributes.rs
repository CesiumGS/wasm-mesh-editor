//! JS adapters for listing, reading and writing mesh attributes, and shared by operations on the
//! components whose attribute matches a value.

use std::cmp::Ordering;

use bytemuck::Pod;
use glam::{IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use js_sys::{Float32Array, Int32Array, Uint8Array, Uint32Array};
use mesh::{
    AttributeComponentType, AttributeDescriptor, AttributeDomain, AttributeError, AttributeKey,
    AttributeType, AttributeValue, EdgeKey, FaceKey, IntoComponentKey, LoopKey, Mesh, VertKey,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use super::{ComponentKeysJs, active_mesh, ffi, key_from_ffi};
use crate::{MeshEditor, MeshKey, MeshNotFound};

/// The result of an attribute function, like `selectByAttribute` or `addAttribute`. Only `Ok`
/// indicates success.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeResult {
    /// The call succeeded. By-attribute functions apply to any matching components, which may be
    /// none if nothing matched or the components aren't selectable in the current mode.
    Ok = 0,
    /// The mesh has no attribute with that name.
    AttributeNotFound = 1,
    /// A value or `noData` doesn't fit the attribute's type, like `1.5` for an integer attribute or
    /// a number for a vector attribute, or a `defaultValue` isn't a `Float32Array`, `Uint32Array`
    /// or `Int32Array` of 1 to 4 values.
    WrongType = 2,
    /// The mesh isn't active, so it can't be edited.
    MeshInactive = 3,
    /// By-attribute functions: the comparison doesn't apply to the attribute's type, since vector
    /// attributes only support `Equal` and `NotEqual`.
    UnsupportedComparison = 4,
    /// The operation doesn't apply to corner attributes, since JS has no corner keys.
    UnsupportedDomain = 5,
    /// `addAttribute`: the name is already in use.
    NameInUse = 6,
    /// `addAttribute` with a reserved name, or `removeAttribute` on a built-in attribute.
    Reserved = 7,
    /// `attributeValues`: a key isn't in the mesh, and there's no `noData`.
    KeyNotFound = 8,
}

/// How functions that operate on components by attribute compare each component's attribute
/// value to the given value. Float comparisons follow IEEE 754, so a NaN only matches `NotEqual`.
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

/// Evaluates `$body` with `$K` and `$T` as the key and value types of an attribute in `$domain`
/// with `$value_type` values, or evaluates `$corner` for corner attributes, since JS has no corner
/// keys. A macro since each pair of types needs its own instance of `$body`.
macro_rules! with_attribute_types {
    ($domain:expr, $value_type:expr, $K:ident, $T:ident => $body:expr, corner => $corner:expr) => {
        match $domain {
            AttributeDomain::Vertex => {
                type $K = VertKey;
                with_value_type!($value_type, $T => $body)
            }
            AttributeDomain::Edge => {
                type $K = EdgeKey;
                with_value_type!($value_type, $T => $body)
            }
            AttributeDomain::Face => {
                type $K = FaceKey;
                with_value_type!($value_type, $T => $body)
            }
            AttributeDomain::Corner => $corner,
        }
    };
}

/// Evaluates `$body` with `$T` as the [`AttributeValue`] type whose `VALUE_TYPE` is `$value_type`.
macro_rules! with_value_type {
    ($value_type:expr, $T:ident => $body:expr) => {
        with_value_type!(
            $value_type, $T => $body;
            f32, Vec2, Vec3, Vec4, u32, UVec2, UVec3, UVec4, i32, IVec2, IVec3, IVec4
        )
    };
    ($value_type:expr, $T:ident => $body:expr; $($type:ty),*) => {{
        let value_type: AttributeType = $value_type;
        $(
            if value_type == <$type as AttributeValue>::VALUE_TYPE {
                type $T = $type;
                $body
            } else
        )* {
            unreachable!("every attribute type is an AttributeValue's")
        }
    }};
}

#[wasm_bindgen]
extern "C" {
    /// One attribute value: a number for scalar attributes and an array of numbers for vector ones.
    #[wasm_bindgen(typescript_type = "number | ArrayLike<number>")]
    pub type AttributeValueJs;
}

/// The result of `queryByAttribute`. `keys` holds the matching components by type, like the
/// `componentKeys` that functions such as `setSelection` take. Empty unless `status` is `Ok`.
#[derive(Serialize, Tsify)]
#[tsify(rename = "ByAttributeQuery")]
pub struct ByAttributeQueryJs {
    #[tsify(type = "AttributeResult")]
    status: u8,
    // tsify names a field's type after the Rust type, not its `rename`.
    #[tsify(type = "ComponentKeys")]
    keys: ComponentKeysJs,
}

/// One of a mesh's attributes, as listed by `meshAttributes`.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
#[tsify(rename = "MeshAttribute")]
pub struct MeshAttributeJs {
    /// The glTF name of a built-in attribute (`POSITION`, `NORMAL` or `TEXCOORD_0`), or a custom
    /// attribute's name.
    name: String,
    #[tsify(type = "AttributeDomain")]
    domain: u8,
    #[tsify(type = "AttributeComponentType")]
    component_type: u8,
    /// 1 for scalars, or 2 to 4 for vectors.
    component_count: u8,
}

impl From<AttributeDescriptor<'_>> for MeshAttributeJs {
    fn from(descriptor: AttributeDescriptor<'_>) -> Self {
        Self {
            name: descriptor.name.into(),
            domain: descriptor.domain as u8,
            component_type: descriptor.value_type.component_type as u8,
            component_count: descriptor.value_type.component_count,
        }
    }
}

#[wasm_bindgen]
impl MeshEditor {
    /// The mesh's attributes by domain: vertex, edge, face, then corner attributes. Each domain
    /// lists its built-in attributes first (`POSITION`, or `NORMAL` then `TEXCOORD_0` if the mesh
    /// has UVs), then custom attributes in the order they were added. Throws for an unknown mesh
    /// key.
    #[wasm_bindgen(js_name = meshAttributes, unchecked_return_type = "MeshAttribute[]")]
    pub fn mesh_attributes(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
    ) -> Result<JsValue, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;
        let attributes: Vec<MeshAttributeJs> = mesh
            .attributes()
            .descriptors()
            .map(MeshAttributeJs::from)
            .collect();

        Ok(serde_wasm_bindgen::to_value(&attributes)?)
    }

    /// The vertices, edges or faces whose `name` attribute compares to `value` as `comparison`
    /// says, like `selectByAttribute`. Works on inactive meshes. Throws for an unknown mesh key.
    #[wasm_bindgen(js_name = queryByAttribute)]
    pub fn query_by_attribute(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        comparison: AttributeComparison,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
    ) -> Result<Ts<ByAttributeQueryJs>, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;

        let none = ComponentKeysJs::default;
        let (status, keys) = match descriptor_types(mesh, name) {
            None => (AttributeResult::AttributeNotFound, none()),
            Some((domain, value_type)) => with_attribute_types!(domain, value_type, K, T => {
                match predicate::<T>(comparison, &value) {
                    Ok(predicate) => (AttributeResult::Ok, keys_where::<K, T>(mesh, name, predicate)),
                    Err(status) => (status, none()),
                }
            }, corner => (AttributeResult::UnsupportedDomain, none())),
        };

        Ok(ByAttributeQueryJs {
            status: status as u8,
            keys,
        }
        .into_ts()?)
    }

    /// Writes the `name` attribute's values at vertex, edge or face `keys` to `out`, a typed array
    /// matching the attribute's component type, with `componentCount` values per key. Keys that
    /// aren't in the mesh get `noData`: a number for scalar attributes and an array of numbers for
    /// vector ones. Works on inactive meshes. Leaves `out` unchanged unless the result is `Ok`.
    /// Throws for an unknown mesh key or a wrong `out`.
    #[wasm_bindgen(js_name = attributeValues)]
    pub fn attribute_values(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        keys: Vec<u64>,
        #[wasm_bindgen(unchecked_param_type = "Float32Array | Uint32Array | Int32Array")]
        out: JsValue,
        #[wasm_bindgen(js_name = noData)] no_data: Option<AttributeValueJs>,
    ) -> Result<AttributeResult, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;
        let Some((domain, value_type)) = descriptor_types(mesh, name) else {
            return Ok(AttributeResult::AttributeNotFound);
        };

        with_attribute_types!(domain, value_type, K, T => {
            read_values::<K, T>(mesh, name, &keys, &out, no_data.as_deref())
        }, corner => Ok(AttributeResult::UnsupportedDomain))
    }

    /// Writes `values` to an active mesh's `name` attribute at vertex, edge or face `keys`, like
    /// `attributeValues` in reverse. Ignores keys that aren't in the mesh. Writing `POSITION` also
    /// recomputes flat normals on the faces using the written vertices, so prefer
    /// `translateSelected` for moving the selection. Writes nothing unless the result is `Ok`.
    /// Throws for an unknown mesh key or a wrong `values`, like `attributeValues`.
    #[wasm_bindgen(js_name = setAttributeValues)]
    pub fn set_attribute_values(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        keys: Vec<u64>,
        #[wasm_bindgen(unchecked_param_type = "Float32Array | Uint32Array | Int32Array")]
        values: JsValue,
    ) -> Result<AttributeResult, JsError> {
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(AttributeResult::MeshInactive);
        };
        let Some((domain, value_type)) = descriptor_types(mesh, name) else {
            return Ok(AttributeResult::AttributeNotFound);
        };

        with_attribute_types!(domain, value_type, K, T => {
            write_values::<K, T>(mesh, name, &keys, &values)
        }, corner => Ok(AttributeResult::UnsupportedDomain))
    }
}

#[wasm_bindgen]
impl MeshEditor {
    /// Adds a custom attribute called `name` to an active mesh, giving every component of `domain`
    /// `defaultValue`. The typed array's type sets the attribute's component type, and its length
    /// (1 to 4) the component count. Write values with `setAttributeValues`. Only throws for an
    /// unknown mesh key.
    #[wasm_bindgen(js_name = addAttribute)]
    pub fn add_attribute(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        domain: AttributeDomain,
        #[wasm_bindgen(
            js_name = defaultValue,
            unchecked_param_type = "Float32Array | Uint32Array | Int32Array"
        )]
        default_value: JsValue,
    ) -> Result<AttributeResult, JsError> {
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(AttributeResult::MeshInactive);
        };
        let Some((component_type, bytes, length @ 1..=4)) = typed_array(&default_value) else {
            return Ok(AttributeResult::WrongType);
        };
        let value_type = AttributeType {
            component_type,
            component_count: length as u8,
        };

        Ok(with_value_type!(value_type, T => {
            let mut default: T = bytemuck::Zeroable::zeroed();
            bytes.copy_to(bytemuck::bytes_of_mut(&mut default));

            status(match domain {
                AttributeDomain::Vertex => mesh.add_attribute::<VertKey, T>(name, default),
                AttributeDomain::Edge => mesh.add_attribute::<EdgeKey, T>(name, default),
                AttributeDomain::Face => mesh.add_attribute::<FaceKey, T>(name, default),
                AttributeDomain::Corner => mesh.add_attribute::<LoopKey, T>(name, default),
            })
        }))
    }

    /// Removes a custom attribute from an active mesh. Only throws for an unknown mesh key.
    #[wasm_bindgen(js_name = removeAttribute)]
    pub fn remove_attribute(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
    ) -> Result<AttributeResult, JsError> {
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(AttributeResult::MeshInactive);
        };

        Ok(status(mesh.remove_attribute(name)))
    }
}

/// An operation on the components whose `name` attribute satisfies a predicate, like
/// [`mesh::Selection::select_by_attribute`].
pub(super) trait ByAttributeOperation {
    fn apply<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &self,
        mesh: &mut Mesh,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeError>;
}

/// Applies `operation` to the vertices, edges or faces whose `name` attribute compares to `value`,
/// converted to the attribute's type, as `comparison` says.
pub(super) fn apply_by_js_value(
    mesh: &mut Mesh,
    name: &str,
    comparison: AttributeComparison,
    value: &JsValue,
    operation: &impl ByAttributeOperation,
) -> AttributeResult {
    let Some((domain, value_type)) = descriptor_types(mesh, name) else {
        return AttributeResult::AttributeNotFound;
    };

    with_attribute_types!(domain, value_type, K, T => {
        match predicate::<T>(comparison, value) {
            Ok(predicate) => status(operation.apply::<K, T>(mesh, name, predicate)),
            Err(status) => status,
        }
    }, corner => AttributeResult::UnsupportedDomain)
}

/// The domain and value type of the attribute called `name`, if there is one.
fn descriptor_types(mesh: &Mesh, name: &str) -> Option<(AttributeDomain, AttributeType)> {
    let descriptor = mesh.attributes().descriptor(name)?;
    Some((descriptor.domain, descriptor.value_type))
}

/// The vertex, edge or face keys whose `name` attribute satisfies `predicate`.
fn keys_where<K: AttributeKey, T: AttributeValue>(
    mesh: &Mesh,
    name: &str,
    predicate: impl Fn(T) -> bool,
) -> ComponentKeysJs {
    let keys = mesh
        .attributes()
        .keys_where::<K, T, _>(name, predicate)
        .expect("the descriptor matches the attribute")
        .map(ffi)
        .collect();

    let mut result = ComponentKeysJs::default();
    match K::DOMAIN {
        AttributeDomain::Vertex => result.verts = keys,
        AttributeDomain::Edge => result.edges = keys,
        AttributeDomain::Face => result.faces = keys,
        AttributeDomain::Corner => unreachable!("JS has no corner keys"),
    }
    result
}

fn read_values<K: AttributeKey, T: AttributeValue + Pod>(
    mesh: &Mesh,
    name: &str,
    keys: &[u64],
    out: &JsValue,
    no_data: Option<&JsValue>,
) -> Result<AttributeResult, JsError> {
    let out = typed_array_bytes(out, T::VALUE_TYPE, keys.len(), "out")?;
    let no_data = match no_data.map(value_from_js::<T>) {
        Some(None) => return Ok(AttributeResult::WrongType),
        no_data => no_data.flatten(),
    };
    let map = mesh
        .attributes()
        .values::<K, T>(name)
        .expect("the descriptor matches the attribute");

    let values: Option<Vec<T>> = keys
        .iter()
        .map(|&key| {
            key_from_ffi(key)
                .and_then(|key| map.get(key).copied())
                .or(no_data)
        })
        .collect();
    let Some(values) = values else {
        return Ok(AttributeResult::KeyNotFound);
    };

    out.copy_from(bytemuck::cast_slice(&values));
    Ok(AttributeResult::Ok)
}

fn write_values<K: AttributeKey, T: AttributeValue + Pod>(
    mesh: &mut Mesh,
    name: &str,
    keys: &[u64],
    values: &JsValue,
) -> Result<AttributeResult, JsError> {
    let bytes = typed_array_bytes(values, T::VALUE_TYPE, keys.len(), "values")?;
    let mut values = vec![T::zeroed(); keys.len()];
    bytes.copy_to(bytemuck::cast_slice_mut(&mut values));

    let values: Vec<(K, T)> = keys
        .iter()
        .zip(values)
        .filter_map(|(&key, value)| Some((key_from_ffi(key)?, value)))
        .collect();

    Ok(status(mesh.set_attribute_values(name, &values)))
}

/// A byte view of `array`, if it's the typed array for `value_type`'s components and holds `count`
/// values. `argument` names it in errors.
fn typed_array_bytes(
    array: &JsValue,
    value_type: AttributeType,
    count: usize,
    argument: &str,
) -> Result<Uint8Array, JsError> {
    let class = match value_type.component_type {
        AttributeComponentType::Float32 => "a Float32Array",
        AttributeComponentType::Uint32 => "a Uint32Array",
        AttributeComponentType::Int32 => "an Int32Array",
    };
    let (_, bytes, length) = typed_array(array)
        .filter(|&(component_type, ..)| component_type == value_type.component_type)
        .ok_or_else(|| JsError::new(&format!("{argument} is not {class}")))?;

    let expected = count * usize::from(value_type.component_count);
    if length as usize != expected {
        return Err(JsError::new(&format!(
            "{argument} length {length} is not {expected}"
        )));
    }

    Ok(bytes)
}

/// The component type, a byte view and the length of `array`, if it's a `Float32Array`,
/// `Uint32Array` or `Int32Array`.
fn typed_array(array: &JsValue) -> Option<(AttributeComponentType, Uint8Array, u32)> {
    use AttributeComponentType::{Float32, Int32, Uint32};

    let (component_type, buffer, offset, length) =
        if let Some(array) = array.dyn_ref::<Float32Array>() {
            (Float32, array.buffer(), array.byte_offset(), array.length())
        } else if let Some(array) = array.dyn_ref::<Uint32Array>() {
            (Uint32, array.buffer(), array.byte_offset(), array.length())
        } else {
            let array = array.dyn_ref::<Int32Array>()?;
            (Int32, array.buffer(), array.byte_offset(), array.length())
        };

    // Every component type is 4 bytes.
    let bytes = Uint8Array::new_with_byte_offset_and_length(&buffer, offset, length * 4);
    Some((component_type, bytes, length))
}

fn status(result: Result<(), AttributeError>) -> AttributeResult {
    match result {
        Ok(()) => AttributeResult::Ok,
        Err(AttributeError::NotFound { .. }) => AttributeResult::AttributeNotFound,
        Err(AttributeError::WrongType { .. }) => AttributeResult::WrongType,
        Err(AttributeError::AlreadyExists { .. }) => AttributeResult::NameInUse,
        Err(AttributeError::Reserved { .. }) => AttributeResult::Reserved,
    }
}

/// A predicate comparing values to `value`, converted from JS. `UnsupportedComparison` for an
/// ordering comparison on vectors, and `WrongType` if `value` doesn't fit `T`.
fn predicate<T: AttributeValue + Pod + PartialEq>(
    comparison: AttributeComparison,
    value: &JsValue,
) -> Result<impl Fn(T) -> bool, AttributeResult> {
    let is_vector = T::VALUE_TYPE.component_count > 1;
    let is_equality = matches!(
        comparison,
        AttributeComparison::Equal | AttributeComparison::NotEqual
    );
    if is_vector && !is_equality {
        return Err(AttributeResult::UnsupportedComparison);
    }

    let value = value_from_js::<T>(value).ok_or(AttributeResult::WrongType)?;

    Ok(move |other: T| {
        let order = || scalar_order(other, value);
        match comparison {
            AttributeComparison::Equal => other == value,
            AttributeComparison::NotEqual => other != value,
            AttributeComparison::Less => order().is_some_and(Ordering::is_lt),
            AttributeComparison::LessOrEqual => order().is_some_and(Ordering::is_le),
            AttributeComparison::Greater => order().is_some_and(Ordering::is_gt),
            AttributeComparison::GreaterOrEqual => order().is_some_and(Ordering::is_ge),
        }
    })
}

/// How scalar `a` compares to `b`, or `None` if either is NaN.
fn scalar_order<T: AttributeValue + Pod>(a: T, b: T) -> Option<Ordering> {
    match T::VALUE_TYPE.component_type {
        AttributeComponentType::Float32 => {
            bytemuck::cast::<T, f32>(a).partial_cmp(&bytemuck::cast(b))
        }
        AttributeComponentType::Uint32 => Some(bytemuck::cast::<T, u32>(a).cmp(&bytemuck::cast(b))),
        AttributeComponentType::Int32 => Some(bytemuck::cast::<T, i32>(a).cmp(&bytemuck::cast(b))),
    }
}

/// `value` as `T`: a number for scalars, or an array or typed array of exactly as many numbers as
/// `T` has components. `None` if it doesn't fit, like a fractional number for an integer type.
fn value_from_js<T: AttributeValue + Pod>(value: &JsValue) -> Option<T> {
    let mut result = T::zeroed();
    let bytes = bytemuck::bytes_of_mut(&mut result);
    let count = usize::from(T::VALUE_TYPE.component_count);

    match T::VALUE_TYPE.component_type {
        AttributeComponentType::Float32 => bytes.copy_from_slice(bytemuck::cast_slice(
            &components_from_js::<f32>(value, count)?,
        )),
        AttributeComponentType::Uint32 => bytes.copy_from_slice(bytemuck::cast_slice(
            &components_from_js::<u32>(value, count)?,
        )),
        AttributeComponentType::Int32 => bytes.copy_from_slice(bytemuck::cast_slice(
            &components_from_js::<i32>(value, count)?,
        )),
    }

    Some(result)
}

fn components_from_js<S: DeserializeOwned>(value: &JsValue, count: usize) -> Option<Vec<S>> {
    let components = if count == 1 {
        vec![from_js(value)?]
    } else {
        from_js::<Vec<S>>(value)?
    };

    (components.len() == count).then_some(components)
}

/// `value` as `T`, or `None` if it doesn't fit, like a fractional number for an integer.
fn from_js<T: DeserializeOwned>(value: &JsValue) -> Option<T> {
    serde_wasm_bindgen::from_value(value.clone()).ok()
}
