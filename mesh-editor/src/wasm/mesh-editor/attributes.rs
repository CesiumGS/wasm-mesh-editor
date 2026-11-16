//! JS adapters for listing and reading mesh attributes, and shared by operations on the vertices
//! whose custom attribute matches a value.

use glam::{IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use js_sys::BigUint64Array;
use mesh::{
    AttributeDescriptor, AttributeLookupError, AttributeMap, AttributeValue, Mesh, VertKey,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use slotmap::{Key, SecondaryMap};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{MeshEditor, MeshKey, MeshNotFound};

/// The result of calling a function that operates on a set of vertices specified by attribute value, like
/// `selectVertsByAttribute` or `queryVertsByAttribute`. Only `Ok` indicates success.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertsByAttributeResult {
    /// The attribute was found and the operation applied to any matching vertices. That may be
    /// none, if nothing matched or vertices aren't selectable in the current mode.
    Ok = 0,
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

/// How functions that operate on vertices by attribute compare each
/// vertex's attribute value to the given value.
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

/// Evaluates `$body` with `$predicate` comparing `$attribute`'s values to the JS `$value`.
/// A macro since each attribute type needs its own predicate type.
macro_rules! with_predicate {
    ($attribute:expr, $comparison:expr, $value:expr, $predicate:ident => $body:expr) => {
        match $attribute {
            AttributeMap::F32(_) => {
                let $predicate = scalar_predicate($comparison, from_js::<f32>($value));
                $body
            }
            AttributeMap::Vec2(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<Vec2, _, _>($value));
                $body
            }
            AttributeMap::Vec3(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<Vec3, _, _>($value));
                $body
            }
            AttributeMap::Vec4(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<Vec4, _, _>($value));
                $body
            }
            AttributeMap::U32(_) => {
                let $predicate = scalar_predicate($comparison, from_js::<u32>($value));
                $body
            }
            AttributeMap::UVec2(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<UVec2, _, _>($value));
                $body
            }
            AttributeMap::UVec3(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<UVec3, _, _>($value));
                $body
            }
            AttributeMap::UVec4(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<UVec4, _, _>($value));
                $body
            }
            AttributeMap::I32(_) => {
                let $predicate = scalar_predicate($comparison, from_js::<i32>($value));
                $body
            }
            AttributeMap::IVec2(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<IVec2, _, _>($value));
                $body
            }
            AttributeMap::IVec3(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<IVec3, _, _>($value));
                $body
            }
            AttributeMap::IVec4(_) => {
                let $predicate =
                    vector_predicate($comparison, vector_from_js::<IVec4, _, _>($value));
                $body
            }
        }
    };
}

/// The result of `queryVertsByAttribute`. `verts` is empty unless `status` is `Ok`.
#[derive(Serialize, Tsify)]
#[tsify(rename = "VertsByAttributeQuery")]
pub struct VertsByAttributeQueryJs {
    #[tsify(type = "VertsByAttributeResult")]
    status: u8,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "BigUint64Array")]
    verts: BigUint64Array,
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
    /// The mesh's attributes: `POSITION` and `NORMAL`, then `TEXCOORD_0` if the mesh has UVs,
    /// then custom vertex attributes and custom corner attributes, each in the order they were
    /// added. Throws for an unknown mesh key.
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

    /// The vertices whose `name` attribute compares to `value` as `comparison` says, like
    /// `selectVertsByAttribute`. Works on inactive meshes. Throws for an unknown mesh key.
    #[wasm_bindgen(js_name = queryVertsByAttribute)]
    pub fn query_verts_by_attribute(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        name: &str,
        comparison: AttributeComparison,
        #[wasm_bindgen(unchecked_param_type = "number | ArrayLike<number>")] value: JsValue,
    ) -> Result<Ts<VertsByAttributeQueryJs>, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;

        let (status, verts) = match mesh.attributes().vertex_attribute(name) {
            None => (VertsByAttributeResult::AttributeNotFound, Vec::new()),
            Some(attribute) => {
                with_predicate!(attribute, comparison, &value, predicate => {
                    vert_keys_where(mesh, name, predicate)
                })
            }
        };

        Ok(VertsByAttributeQueryJs {
            status: status as u8,
            verts: BigUint64Array::from(verts.as_slice()),
        }
        .into_ts()?)
    }

    /// A vertex's `name` attribute value: a number for scalar attributes and an array of numbers
    /// for vector ones. `undefined` for an unknown vertex or attribute. Throws for an unknown mesh
    /// key.
    #[wasm_bindgen(
        js_name = vertexAttributeValue,
        unchecked_return_type = "number | number[] | undefined"
    )]
    pub fn vertex_attribute_value(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        #[wasm_bindgen(js_name = vertKey)] vert_key: u64,
        name: &str,
    ) -> Result<JsValue, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;
        let (Some(vert), Some(attribute)) = (
            VertKey::try_from_ffi(vert_key),
            mesh.attributes().vertex_attribute(name),
        ) else {
            return Ok(JsValue::UNDEFINED);
        };

        let value = match attribute {
            AttributeMap::F32(values) => value_to_js(values, vert),
            AttributeMap::Vec2(values) => value_to_js(values, vert),
            AttributeMap::Vec3(values) => value_to_js(values, vert),
            AttributeMap::Vec4(values) => value_to_js(values, vert),
            AttributeMap::U32(values) => value_to_js(values, vert),
            AttributeMap::UVec2(values) => value_to_js(values, vert),
            AttributeMap::UVec3(values) => value_to_js(values, vert),
            AttributeMap::UVec4(values) => value_to_js(values, vert),
            AttributeMap::I32(values) => value_to_js(values, vert),
            AttributeMap::IVec2(values) => value_to_js(values, vert),
            AttributeMap::IVec3(values) => value_to_js(values, vert),
            AttributeMap::IVec4(values) => value_to_js(values, vert),
        };

        Ok(value?.unwrap_or(JsValue::UNDEFINED))
    }
}

/// An operation on the vertices whose `name` attribute satisfies a predicate, like
/// [`mesh::Selection::select_verts_by_attribute`].
pub(super) trait VertsByAttributeOperation {
    fn operate_on_verts_where<T: AttributeValue>(
        &self,
        mesh: &mut Mesh,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeLookupError>;
}

/// Applies `operation` to the vertices whose `name` attribute compares to `value`, converted to the
/// attribute's stored type, as `comparison` says.
pub(super) fn operate_on_verts_by_js_value(
    mesh: &mut Mesh,
    name: &str,
    comparison: AttributeComparison,
    value: &JsValue,
    operation: &impl VertsByAttributeOperation,
) -> VertsByAttributeResult {
    let Some(attribute) = mesh.attributes().vertex_attribute(name) else {
        return VertsByAttributeResult::AttributeNotFound;
    };

    with_predicate!(attribute, comparison, value, predicate => match predicate {
        Ok(predicate) => {
            lookup_status(operation.operate_on_verts_where(mesh, name, predicate))
        }
        Err(result) => result,
    })
}

/// The FFI keys of the vertices whose `name` attribute satisfies `predicate`, and the status.
fn vert_keys_where<T: AttributeValue>(
    mesh: &Mesh,
    name: &str,
    predicate: Result<impl Fn(T) -> bool, VertsByAttributeResult>,
) -> (VertsByAttributeResult, Vec<u64>) {
    let predicate = match predicate {
        Ok(predicate) => predicate,
        Err(result) => return (result, Vec::new()),
    };

    match mesh.attributes().verts_where(name, predicate) {
        Ok(verts) => (
            VertsByAttributeResult::Ok,
            verts.map(|vert| vert.data().as_ffi()).collect(),
        ),
        Err(error) => (lookup_status(Err(error)), Vec::new()),
    }
}

fn lookup_status(result: Result<(), AttributeLookupError>) -> VertsByAttributeResult {
    match result {
        Ok(()) => VertsByAttributeResult::Ok,
        Err(AttributeLookupError::NotFound { .. }) => VertsByAttributeResult::AttributeNotFound,
        Err(AttributeLookupError::WrongType { .. }) => VertsByAttributeResult::WrongType,
    }
}

/// A predicate comparing values to `value`. `WrongType` if `value` is `None`, meaning the JS
/// value didn't convert.
fn scalar_predicate<T: PartialOrd + Copy>(
    comparison: AttributeComparison,
    value: Option<T>,
) -> Result<impl Fn(T) -> bool, VertsByAttributeResult> {
    let value = value.ok_or(VertsByAttributeResult::WrongType)?;

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
) -> Result<impl Fn(T) -> bool, VertsByAttributeResult> {
    let matches_equal = match comparison {
        AttributeComparison::Equal => true,
        AttributeComparison::NotEqual => false,
        _ => return Err(VertsByAttributeResult::UnsupportedComparison),
    };
    let value = value.ok_or(VertsByAttributeResult::WrongType)?;

    Ok(move |vert_value: T| (vert_value == value) == matches_equal)
}

/// `value` as `T`, or `None` if it doesn't fit, like a fractional number for an integer.
fn from_js<T: DeserializeOwned>(value: &JsValue) -> Option<T> {
    serde_wasm_bindgen::from_value(value.clone()).ok()
}

/// `value` as a `V` with exactly `N` components of type `S`, from an array or typed array. `None` if
/// the length is wrong or a component doesn't fit `S`.
fn vector_from_js<V: From<[S; N]>, S: DeserializeOwned, const N: usize>(
    value: &JsValue,
) -> Option<V> {
    // Going through a `Vec` rejects extra elements, which deserializing `[S; N]` ignores.
    let components: [S; N] = from_js::<Vec<S>>(value)?.try_into().ok()?;
    Some(components.into())
}

/// `vert`'s value as a number or array of numbers, or `None` if it has none.
fn value_to_js<T: Serialize>(
    values: &SecondaryMap<VertKey, T>,
    vert: VertKey,
) -> Result<Option<JsValue>, serde_wasm_bindgen::Error> {
    values
        .get(vert)
        .map(serde_wasm_bindgen::to_value)
        .transpose()
}
