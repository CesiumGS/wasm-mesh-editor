//! JS adapters shared by operations on the vertices whose custom attribute matches a value.

use glam::{Vec2, Vec3, Vec4};
use mesh::{AttributeLookupError, AttributeMap, AttributeValue, Mesh};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

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

/// An operation on the vertices whose `name` attribute satisfies a predicate, like
/// [`mesh::Selection::select_verts_by_attribute`].
pub(super) trait VertsByAttribute {
    fn apply<T: AttributeValue>(
        &self,
        mesh: &mut Mesh,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeLookupError>;
}

/// Applies `operation` to the vertices whose `name` attribute compares to `value`, converted to the
/// attribute's stored type, as `comparison` says.
pub(super) fn apply_by_js_value(
    mesh: &mut Mesh,
    name: &str,
    comparison: AttributeComparison,
    value: &JsValue,
    operation: &impl VertsByAttribute,
) -> VertsByAttributeResult {
    let Some(attribute) = mesh.attributes().vertex_attribute(name) else {
        return VertsByAttributeResult::AttributeNotFound;
    };

    match attribute {
        AttributeMap::F32(_) => apply(
            operation,
            mesh,
            name,
            scalar_predicate(comparison, from_js::<f32>(value)),
        ),
        AttributeMap::Vec2(_) => apply(
            operation,
            mesh,
            name,
            vector_predicate(comparison, floats_from_js(value).map(Vec2::from_array)),
        ),
        AttributeMap::Vec3(_) => apply(
            operation,
            mesh,
            name,
            vector_predicate(comparison, floats_from_js(value).map(Vec3::from_array)),
        ),
        AttributeMap::Vec4(_) => apply(
            operation,
            mesh,
            name,
            vector_predicate(comparison, floats_from_js(value).map(Vec4::from_array)),
        ),
        AttributeMap::U32(_) => apply(
            operation,
            mesh,
            name,
            scalar_predicate(comparison, from_js::<u32>(value)),
        ),
        AttributeMap::I32(_) => apply(
            operation,
            mesh,
            name,
            scalar_predicate(comparison, from_js::<i32>(value)),
        ),
    }
}

fn apply<T: AttributeValue>(
    operation: &impl VertsByAttribute,
    mesh: &mut Mesh,
    name: &str,
    predicate: Result<impl Fn(T) -> bool, VertsByAttributeResult>,
) -> VertsByAttributeResult {
    let predicate = match predicate {
        Ok(predicate) => predicate,
        Err(result) => return result,
    };

    match operation.apply(mesh, name, predicate) {
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

/// `value` as exactly `N` floats, from an array or typed array.
fn floats_from_js<const N: usize>(value: &JsValue) -> Option<[f32; N]> {
    // Going through a `Vec` rejects extra elements, which deserializing `[f32; N]` ignores.
    from_js::<Vec<f32>>(value)?.try_into().ok()
}
