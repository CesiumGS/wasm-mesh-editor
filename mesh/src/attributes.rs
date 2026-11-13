//! Struct-of-Arrays geometry attribute storage.

use glam::{Vec2, Vec3, Vec4};
use slotmap::{Key, SecondaryMap};

use crate::{LoopKey, VertKey};

/// Vertex positions, face-corner normals and UVs, and named custom attributes, keyed by their handles.
#[derive(Default)]
pub struct Attributes {
    pub positions: SecondaryMap<VertKey, Vec3>,
    pub normals: SecondaryMap<LoopKey, Vec3>,
    pub uvs: SecondaryMap<LoopKey, Vec2>,
    /// Custom attributes in insertion order. Names must be unique across both domains.
    vertex_custom: Vec<(String, AttributeMap<VertKey>)>,
    corner_custom: Vec<(String, AttributeMap<LoopKey>)>,
}

/// The values of one custom attribute, keyed by vertex ([`VertKey`]) or face corner ([`LoopKey`]).
/// The variant is the value type; match on it when the type isn't known in advance,
/// or use [`Self::typed`] when it is.
pub enum AttributeMap<K: Key> {
    F32(SecondaryMap<K, f32>),
    Vec2(SecondaryMap<K, Vec2>),
    Vec3(SecondaryMap<K, Vec3>),
    Vec4(SecondaryMap<K, Vec4>),
    U32(SecondaryMap<K, u32>),
    I32(SecondaryMap<K, i32>),
}

impl<K: Key> AttributeMap<K> {
    /// The values as `T`, or `None` if this attribute stores a different type.
    pub fn typed<T: AttributeValue>(&self) -> Option<&SecondaryMap<K, T>> {
        T::from_map(self)
    }
}

/// A type that custom attribute values can have: `f32`, `Vec2`, `Vec3`, `Vec4`, `u32` or `i32`.
/// Used to request typed values, as in [`AttributeMap::typed`]. Sealed; it can't be implemented
/// outside this crate.
pub trait AttributeValue: Copy + sealed::Sealed {}

mod sealed {
    use slotmap::{Key, SecondaryMap};

    use super::AttributeMap;

    pub trait Sealed: Sized {
        /// The matching variant's values, or `None` for any other variant.
        fn from_map<K: Key>(map: &AttributeMap<K>) -> Option<&SecondaryMap<K, Self>>;
    }
}

macro_rules! impl_attribute_value {
    ($($type:ty => $variant:ident),* $(,)?) => {$(
        impl sealed::Sealed for $type {
            fn from_map<K: Key>(map: &AttributeMap<K>) -> Option<&SecondaryMap<K, Self>> {
                match map {
                    AttributeMap::$variant(values) => Some(values),
                    _ => None,
                }
            }
        }

        impl AttributeValue for $type {}
    )*};
}

impl_attribute_value!(
    f32 => F32,
    Vec2 => Vec2,
    Vec3 => Vec3,
    Vec4 => Vec4,
    u32 => U32,
    i32 => I32,
);

impl Attributes {
    /// Creates empty attribute maps with space for vertex positions and loop attributes.
    /// Use zero UV capacity when UVs are absent.
    pub fn with_capacity(
        position_capacity: usize,
        normal_capacity: usize,
        uv_capacity: usize,
    ) -> Self {
        Self {
            positions: SecondaryMap::with_capacity(position_capacity),
            normals: SecondaryMap::with_capacity(normal_capacity),
            uvs: SecondaryMap::with_capacity(uv_capacity),
            vertex_custom: Vec::new(),
            corner_custom: Vec::new(),
        }
    }

    /// Custom vertex attributes and their names, in insertion order.
    pub fn vertex_attributes(&self) -> impl Iterator<Item = (&str, &AttributeMap<VertKey>)> {
        self.vertex_custom
            .iter()
            .map(|(name, values)| (name.as_str(), values))
    }

    /// Custom corner attributes and their names, in insertion order.
    pub fn corner_attributes(&self) -> impl Iterator<Item = (&str, &AttributeMap<LoopKey>)> {
        self.corner_custom
            .iter()
            .map(|(name, values)| (name.as_str(), values))
    }

    /// The named custom vertex attribute, or `None` if there isn't one.
    pub fn vertex_attribute(&self, name: &str) -> Option<&AttributeMap<VertKey>> {
        find_custom(&self.vertex_custom, name)
    }

    /// The named custom corner attribute, or `None` if there isn't one.
    pub fn corner_attribute(&self, name: &str) -> Option<&AttributeMap<LoopKey>> {
        find_custom(&self.corner_custom, name)
    }

    /// Adds a custom vertex attribute. The name must not be used by any custom attribute.
    pub(crate) fn add_vertex_attribute(&mut self, name: String, values: AttributeMap<VertKey>) {
        debug_assert!(
            !self.has_custom(&name),
            "duplicate custom attribute {name:?}"
        );
        self.vertex_custom.push((name, values));
    }

    /// Adds a custom corner attribute. The name must not be used by any custom attribute.
    pub(crate) fn add_corner_attribute(&mut self, name: String, values: AttributeMap<LoopKey>) {
        debug_assert!(
            !self.has_custom(&name),
            "duplicate custom attribute {name:?}"
        );
        self.corner_custom.push((name, values));
    }

    fn has_custom(&self, name: &str) -> bool {
        self.vertex_attribute(name).is_some() || self.corner_attribute(name).is_some()
    }
}

/// Meshes typically have few custom attributes, so a linear scan is as fast as hashing.
fn find_custom<'a, K: Key>(
    attributes: &'a [(String, AttributeMap<K>)],
    name: &str,
) -> Option<&'a AttributeMap<K>> {
    attributes
        .iter()
        .find(|(attribute, _)| attribute == name)
        .map(|(_, values)| values)
}

#[cfg(test)]
#[path = "../tests/unit/attributes.rs"]
mod tests;
