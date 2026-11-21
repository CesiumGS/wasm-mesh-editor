//! Struct-of-Arrays geometry attribute storage.

use std::any::Any;
use std::fmt;

use glam::{IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use slotmap::{Key, SecondaryMap};

use crate::{EdgeKey, FaceKey, LoopKey, VertKey};

/// Vertex positions, face-corner normals and UVs, and named custom attributes, keyed by their handles.
#[derive(Default)]
pub struct Attributes {
    pub positions: SecondaryMap<VertKey, Vec3>,
    pub normals: SecondaryMap<LoopKey, Vec3>,
    pub uvs: SecondaryMap<LoopKey, Vec2>,
    /// Custom attributes in insertion order. Names must be unique across all domains.
    vertex_custom: Vec<(String, AttributeMap<VertKey>)>,
    edge_custom: Vec<(String, AttributeMap<EdgeKey>)>,
    face_custom: Vec<(String, AttributeMap<FaceKey>)>,
    corner_custom: Vec<(String, AttributeMap<LoopKey>)>,
}

/// Whether an attribute has one value per vertex, edge, face or face corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub enum AttributeDomain {
    /// One value per vertex. Input buffers list them in `positions` order.
    Vertex = 0,
    /// One value per face corner. Input buffers list them in `indices` order.
    Corner = 1,
    /// One value per edge. Input buffers list them in `edge_indices` order, and unlisted edges get
    /// the attribute's default.
    Edge = 2,
    /// One value per face. Input buffers list them in face order.
    Face = 3,
}

/// A key type that attributes can be keyed by: [`VertKey`], [`EdgeKey`], [`FaceKey`] or
/// [`LoopKey`]. Sealed.
pub trait AttributeKey: Key + 'static + sealed::SealedKey {
    const DOMAIN: AttributeDomain;
}

/// The scalar type of each component of an attribute's values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub enum AttributeComponentType {
    Float32 = 0,
    Uint32 = 1,
    Int32 = 2,
}

/// The type of an attribute's values: a scalar ([`Self::component_count`] 1) or a 2 to 4
/// component vector of [`Self::component_type`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeType {
    pub component_type: AttributeComponentType,
    pub component_count: u8,
}

/// Describes one attribute of a mesh, built-in or custom. See [`Attributes::descriptors`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeDescriptor<'a> {
    pub name: &'a str,
    pub domain: AttributeDomain,
    pub value_type: AttributeType,
}

/// The values of one custom attribute, keyed by an [`AttributeKey`].
/// The variant is the value type; match on it when the type isn't known in advance,
/// or use [`Self::typed`] when it is.
pub enum AttributeMap<K: Key> {
    F32(SecondaryMap<K, f32>),
    Vec2(SecondaryMap<K, Vec2>),
    Vec3(SecondaryMap<K, Vec3>),
    Vec4(SecondaryMap<K, Vec4>),
    U32(SecondaryMap<K, u32>),
    UVec2(SecondaryMap<K, UVec2>),
    UVec3(SecondaryMap<K, UVec3>),
    UVec4(SecondaryMap<K, UVec4>),
    I32(SecondaryMap<K, i32>),
    IVec2(SecondaryMap<K, IVec2>),
    IVec3(SecondaryMap<K, IVec3>),
    IVec4(SecondaryMap<K, IVec4>),
}

impl<K: Key> AttributeMap<K> {
    /// The values as `T`, or `None` if this attribute stores a different type.
    pub fn typed<T: AttributeValue>(&self) -> Option<&SecondaryMap<K, T>> {
        T::from_map(self)
    }
}

/// One custom attribute's input values, in input order.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeBuffer {
    F32(Vec<f32>),
    Vec2(Vec<Vec2>),
    Vec3(Vec<Vec3>),
    Vec4(Vec<Vec4>),
    U32(Vec<u32>),
    UVec2(Vec<UVec2>),
    UVec3(Vec<UVec3>),
    UVec4(Vec<UVec4>),
    I32(Vec<i32>),
    IVec2(Vec<IVec2>),
    IVec3(Vec<IVec3>),
    IVec4(Vec<IVec4>),
}

/// Matches every variant of [`AttributeMap`] or [`AttributeBuffer`].
macro_rules! match_variants {
    ($enum:ident, $value:expr, $values:ident => $body:expr) => {
        match $value {
            $enum::F32($values) => $body,
            $enum::Vec2($values) => $body,
            $enum::Vec3($values) => $body,
            $enum::Vec4($values) => $body,
            $enum::U32($values) => $body,
            $enum::UVec2($values) => $body,
            $enum::UVec3($values) => $body,
            $enum::UVec4($values) => $body,
            $enum::I32($values) => $body,
            $enum::IVec2($values) => $body,
            $enum::IVec3($values) => $body,
            $enum::IVec4($values) => $body,
        }
    };
}

impl<K: Key> AttributeMap<K> {
    /// The type of the stored values.
    pub fn value_type(&self) -> AttributeType {
        match_variants!(Self, self, values => value_type_of(values))
    }
}

fn value_type_of<K: Key, T: AttributeValue>(_: &SecondaryMap<K, T>) -> AttributeType {
    T::VALUE_TYPE
}

impl AttributeBuffer {
    /// The number of values (not components).
    pub fn len(&self) -> usize {
        match_variants!(Self, self, values => values.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Moves the values into a map, pairing each value with the key at the same position.
    /// `keys` must yield exactly as many keys as there are values.
    pub(crate) fn into_map<K: Key>(self, keys: impl IntoIterator<Item = K>) -> AttributeMap<K> {
        match_variants!(Self, self, values => zip_map(keys, values))
    }

    /// Like [`Self::into_map`], also giving every key in `all_keys` that `keys` doesn't list
    /// `default`'s single value. `default` must hold one value of the same type.
    pub(crate) fn into_map_with_default<K: Key>(
        self,
        keys: impl IntoIterator<Item = K>,
        default: &AttributeBuffer,
        all_keys: impl IntoIterator<Item = K>,
    ) -> AttributeMap<K> {
        match_variants!(Self, self, values => zip_map_with_default(keys, values, default, all_keys))
    }
}

fn zip_map<K: Key, T: AttributeValue>(
    keys: impl IntoIterator<Item = K>,
    values: Vec<T>,
) -> AttributeMap<K> {
    let count = values.len();
    let mut map = SecondaryMap::with_capacity(count);
    map.extend(keys.into_iter().zip(values));
    debug_assert_eq!(map.len(), count, "expected one distinct key per value");
    T::wrap_map(map)
}

fn zip_map_with_default<K: Key, T: AttributeValue>(
    keys: impl IntoIterator<Item = K>,
    values: Vec<T>,
    default: &AttributeBuffer,
    all_keys: impl IntoIterator<Item = K>,
) -> AttributeMap<K> {
    // First write default value for all keys in `all_keys`
    let default = match_variants!(AttributeBuffer, default, default => {
        (default as &dyn Any).downcast_ref::<Vec<T>>()
    })
    .expect("default must match the values' type")[0];

    // Then overwrite with the provided values for the keys in `keys`
    let mut map: SecondaryMap<K, T> = all_keys.into_iter().map(|key| (key, default)).collect();
    map.extend(keys.into_iter().zip(values));
    T::wrap_map(map)
}

/// A type that custom attribute values can have: `f32`, `u32` or `i32`, or a 2 to 4 component vector
/// of one (`Vec2`..`Vec4`, `UVec2`..`UVec4`, `IVec2`..`IVec4`).
/// Used to request typed values, as in [`AttributeMap::typed`]. Sealed; it can't be implemented
/// outside this crate.
pub trait AttributeValue: Copy + 'static + sealed::Sealed {
    /// The attribute type with values of this type.
    const VALUE_TYPE: AttributeType;
}

mod sealed {
    use slotmap::{Key, SecondaryMap};

    use super::{AttributeMap, Attributes};

    pub trait Sealed: Sized {
        /// The matching variant's values, or `None` for any other variant.
        fn from_map<K: Key>(map: &AttributeMap<K>) -> Option<&SecondaryMap<K, Self>>;

        /// Wraps the values in the matching variant.
        fn wrap_map<K: Key>(values: SecondaryMap<K, Self>) -> AttributeMap<K>;
    }

    pub trait SealedKey: Key {
        fn custom(attributes: &Attributes) -> &Vec<(String, AttributeMap<Self>)>;

        fn custom_mut(attributes: &mut Attributes) -> &mut Vec<(String, AttributeMap<Self>)>;
    }
}

/// Implements [`AttributeValue`] for each `type => AttributeMap variant (component type, component
/// count)`.
macro_rules! impl_attribute_value {
    ($($type:ty => $variant:ident ($component_type:ident, $component_count:literal)),* $(,)?) => {$(
        impl sealed::Sealed for $type {
            fn from_map<K: Key>(map: &AttributeMap<K>) -> Option<&SecondaryMap<K, Self>> {
                match map {
                    AttributeMap::$variant(values) => Some(values),
                    _ => None,
                }
            }

            fn wrap_map<K: Key>(values: SecondaryMap<K, Self>) -> AttributeMap<K> {
                AttributeMap::$variant(values)
            }
        }

        impl AttributeValue for $type {
            const VALUE_TYPE: AttributeType = AttributeType {
                component_type: AttributeComponentType::$component_type,
                component_count: $component_count,
            };
        }
    )*};
}

impl_attribute_value!(
    f32 => F32 (Float32, 1),
    Vec2 => Vec2 (Float32, 2),
    Vec3 => Vec3 (Float32, 3),
    Vec4 => Vec4 (Float32, 4),
    u32 => U32 (Uint32, 1),
    UVec2 => UVec2 (Uint32, 2),
    UVec3 => UVec3 (Uint32, 3),
    UVec4 => UVec4 (Uint32, 4),
    i32 => I32 (Int32, 1),
    IVec2 => IVec2 (Int32, 2),
    IVec3 => IVec3 (Int32, 3),
    IVec4 => IVec4 (Int32, 4),
);

impl Attributes {
    pub const POSITION: &str = "POSITION";
    pub const NORMAL: &str = "NORMAL";
    pub const TEXCOORD_0: &str = "TEXCOORD_0";

    /// Names custom attributes can't use, matched exactly: the glTF names of the built-in
    /// positions, normals and UVs.
    pub const RESERVED_NAMES: [&str; 3] = [Self::POSITION, Self::NORMAL, Self::TEXCOORD_0];

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
            edge_custom: Vec::new(),
            face_custom: Vec::new(),
            corner_custom: Vec::new(),
        }
    }

    /// Every attribute of the mesh: positions and normals, then UVs if present, using the glTF
    /// names in [`Self::RESERVED_NAMES`], then custom vertex, edge, face and corner attributes,
    /// each in insertion order.
    pub fn descriptors(&self) -> impl Iterator<Item = AttributeDescriptor<'_>> {
        let [position, normal, uv] = Self::RESERVED_NAMES;
        let built_in = [
            Some((position, AttributeDomain::Vertex, Vec3::VALUE_TYPE)),
            Some((normal, AttributeDomain::Corner, Vec3::VALUE_TYPE)),
            (!self.uvs.is_empty()).then_some((uv, AttributeDomain::Corner, Vec2::VALUE_TYPE)),
        ];

        let vertex_custom = self
            .custom_attributes::<VertKey>()
            .map(|(name, values)| (name, AttributeDomain::Vertex, values.value_type()));

        let edge_custom = self
            .custom_attributes::<EdgeKey>()
            .map(|(name, values)| (name, AttributeDomain::Edge, values.value_type()));

        let face_custom = self
            .custom_attributes::<FaceKey>()
            .map(|(name, values)| (name, AttributeDomain::Face, values.value_type()));

        let corner_custom = self
            .custom_attributes::<LoopKey>()
            .map(|(name, values)| (name, AttributeDomain::Corner, values.value_type()));

        built_in
            .into_iter()
            .flatten()
            .chain(vertex_custom)
            .chain(edge_custom)
            .chain(face_custom)
            .chain(corner_custom)
            .map(|(name, domain, value_type)| AttributeDescriptor {
                name,
                domain,
                value_type,
            })
    }

    /// The attribute called `name`, built-in or custom, or `None` if there isn't one.
    pub fn descriptor(&self, name: &str) -> Option<AttributeDescriptor<'_>> {
        self.descriptors()
            .find(|descriptor| descriptor.name == name)
    }

    /// Custom attributes keyed by `K` and their names, in insertion order.
    pub fn custom_attributes<K: AttributeKey>(
        &self,
    ) -> impl Iterator<Item = (&str, &AttributeMap<K>)> {
        K::custom(self)
            .iter()
            .map(|(name, values)| (name.as_str(), values))
    }

    /// The named custom attribute keyed by `K`, or `None` if there isn't one.
    pub fn custom_attribute<K: AttributeKey>(&self, name: &str) -> Option<&AttributeMap<K>> {
        find_custom(K::custom(self), name)
    }

    /// The values of the attribute called `name`, built-in or custom.
    ///
    /// # Errors
    /// If there's no attribute called `name` keyed by `K`, or it holds a different type than `T`.
    pub fn values<K: AttributeKey, T: AttributeValue>(
        &self,
        name: &str,
    ) -> Result<&SecondaryMap<K, T>, AttributeError> {
        self.descriptor(name)
            .filter(|descriptor| descriptor.domain == K::DOMAIN)
            .ok_or_else(|| AttributeError::NotFound { name: name.into() })?;

        let values = match name {
            Self::POSITION => (&self.positions as &dyn Any).downcast_ref(),
            Self::NORMAL => (&self.normals as &dyn Any).downcast_ref(),
            Self::TEXCOORD_0 => (&self.uvs as &dyn Any).downcast_ref(),
            _ => self.custom_attribute(name).and_then(AttributeMap::typed),
        };

        values.ok_or_else(|| AttributeError::WrongType { name: name.into() })
    }

    /// Keys whose `name` attribute value satisfies `predicate`.
    ///
    /// # Errors
    /// Like [`Self::values`].
    pub fn keys_where<K: AttributeKey, T: AttributeValue, P: Fn(T) -> bool>(
        &self,
        name: &str,
        predicate: P,
    ) -> Result<impl Iterator<Item = K> + use<'_, K, T, P>, AttributeError> {
        Ok(self
            .values::<K, T>(name)?
            .iter()
            .filter(move |&(_, &value)| predicate(value))
            .map(|(key, _)| key))
    }

    /// Adds a custom attribute. The name must not be used by any attribute.
    pub(crate) fn add_custom<K: AttributeKey>(&mut self, name: String, values: AttributeMap<K>) {
        debug_assert!(
            self.descriptor(&name).is_none(),
            "duplicate custom attribute {name:?}"
        );
        K::custom_mut(self).push((name, values));
    }
}

/// Why an attribute couldn't be read as requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeError {
    /// No attribute with this name in the requested domain.
    NotFound { name: String },
    /// The attribute stores a different value type than the one requested.
    WrongType { name: String },
}

impl fmt::Display for AttributeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { name } => write!(formatter, "attribute {name:?} not found"),
            Self::WrongType { name } => {
                write!(
                    formatter,
                    "attribute {name:?} stores a different value type"
                )
            }
        }
    }
}

impl std::error::Error for AttributeError {}

/// Implements [`AttributeKey`] for each `key type => domain, field`, where `field` is the
/// [`Attributes`] list holding the key type's custom attributes. See macro use below.
macro_rules! impl_attribute_key {
    ($($key:ty => $domain:ident, $field:ident);* $(;)?) => {$(
        impl sealed::SealedKey for $key {
            fn custom(attributes: &Attributes) -> &Vec<(String, AttributeMap<Self>)> {
                &attributes.$field
            }

            fn custom_mut(attributes: &mut Attributes) -> &mut Vec<(String, AttributeMap<Self>)> {
                &mut attributes.$field
            }
        }

        impl AttributeKey for $key {
            const DOMAIN: AttributeDomain = AttributeDomain::$domain;
        }
    )*};
}

impl_attribute_key!(
    VertKey => Vertex, vertex_custom;
    EdgeKey => Edge, edge_custom;
    FaceKey => Face, face_custom;
    LoopKey => Corner, corner_custom;
);

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
