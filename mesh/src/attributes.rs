//! Struct-of-Arrays geometry attribute storage.

use std::fmt;

use glam::{IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use slotmap::{Key, SecondaryMap};

use crate::{EdgeKey, FaceKey, LoopKey, Topology, VertKey};

/// Named attributes, in one list per key type. Each list starts with its built-in attributes:
/// positions (`POSITION`) for vertices, and normals (`NORMAL`) then UVs (`TEXCOORD_0`, if the mesh
/// has them) for corners. Custom attributes follow in insertion order. Names are unique across all
/// lists.
pub struct Attributes {
    vertex: Vec<(String, AttributeMap<VertKey>)>,
    edge: Vec<(String, AttributeMap<EdgeKey>)>,
    face: Vec<(String, AttributeMap<FaceKey>)>,
    corner: Vec<(String, AttributeMap<LoopKey>)>,
}

impl Default for Attributes {
    fn default() -> Self {
        Self::with_capacity(0, 0, None)
    }
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

/// The values of one attribute, keyed by an [`AttributeKey`].
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

    pub(crate) fn typed_mut<T: AttributeValue>(&mut self) -> Option<&mut SecondaryMap<K, T>> {
        T::from_map_mut(self)
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
    let default = T::from_buffer(default).expect("default must match the values' type")[0];

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

    use super::{AttributeBuffer, AttributeMap, Attributes, Topology};

    pub trait Sealed: Sized {
        /// The matching variant's values, or `None` for any other variant.
        fn from_map<K: Key>(map: &AttributeMap<K>) -> Option<&SecondaryMap<K, Self>>;

        fn from_map_mut<K: Key>(map: &mut AttributeMap<K>) -> Option<&mut SecondaryMap<K, Self>>;

        /// The matching variant's values, or `None` for any other variant.
        fn from_buffer(buffer: &AttributeBuffer) -> Option<&[Self]>;

        /// Wraps the values in the matching variant.
        fn wrap_map<K: Key>(values: SecondaryMap<K, Self>) -> AttributeMap<K>;
    }

    pub trait SealedKey: Key {
        fn list(attributes: &Attributes) -> &Vec<(String, AttributeMap<Self>)>;

        fn list_mut(attributes: &mut Attributes) -> &mut Vec<(String, AttributeMap<Self>)>;

        /// Every component of this key type.
        fn keys(topology: &Topology) -> impl Iterator<Item = Self> + '_;
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

            fn from_map_mut<K: Key>(
                map: &mut AttributeMap<K>,
            ) -> Option<&mut SecondaryMap<K, Self>> {
                match map {
                    AttributeMap::$variant(values) => Some(values),
                    _ => None,
                }
            }

            fn from_buffer(buffer: &AttributeBuffer) -> Option<&[Self]> {
                match buffer {
                    AttributeBuffer::$variant(values) => Some(values),
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

    /// Creates empty built-in attributes with space for `position_capacity` positions and
    /// `normal_capacity` normals, and UVs with space for `uv_capacity` values if it's `Some`.
    pub fn with_capacity(
        position_capacity: usize,
        normal_capacity: usize,
        uv_capacity: Option<usize>,
    ) -> Self {
        let normals = SecondaryMap::with_capacity(normal_capacity);
        let mut corner = vec![(Self::NORMAL.into(), AttributeMap::Vec3(normals))];
        if let Some(capacity) = uv_capacity {
            let uvs = SecondaryMap::with_capacity(capacity);
            corner.push((Self::TEXCOORD_0.into(), AttributeMap::Vec2(uvs)));
        }

        let positions = SecondaryMap::with_capacity(position_capacity);
        Self {
            vertex: vec![(Self::POSITION.into(), AttributeMap::Vec3(positions))],
            edge: Vec::new(),
            face: Vec::new(),
            corner,
        }
    }

    pub fn positions(&self) -> &SecondaryMap<VertKey, Vec3> {
        self.values(Self::POSITION).expect(BUILT_IN)
    }

    pub fn normals(&self) -> &SecondaryMap<LoopKey, Vec3> {
        self.values(Self::NORMAL).expect(BUILT_IN)
    }

    /// `None` if the mesh has no UVs.
    pub fn uvs(&self) -> Option<&SecondaryMap<LoopKey, Vec2>> {
        self.values(Self::TEXCOORD_0).ok()
    }

    pub(crate) fn positions_mut(&mut self) -> &mut SecondaryMap<VertKey, Vec3> {
        self.values_mut(Self::POSITION).expect(BUILT_IN)
    }

    pub(crate) fn normals_mut(&mut self) -> &mut SecondaryMap<LoopKey, Vec3> {
        self.values_mut(Self::NORMAL).expect(BUILT_IN)
    }

    pub(crate) fn uvs_mut(&mut self) -> Option<&mut SecondaryMap<LoopKey, Vec2>> {
        self.values_mut(Self::TEXCOORD_0).ok()
    }

    /// Every attribute of the mesh, by domain: vertex, edge, face, then corner attributes, each in
    /// list order.
    pub fn descriptors(&self) -> impl Iterator<Item = AttributeDescriptor<'_>> {
        self.descriptors_of::<VertKey>()
            .chain(self.descriptors_of::<EdgeKey>())
            .chain(self.descriptors_of::<FaceKey>())
            .chain(self.descriptors_of::<LoopKey>())
    }

    /// The attribute called `name`, or `None` if there isn't one.
    pub fn descriptor(&self, name: &str) -> Option<AttributeDescriptor<'_>> {
        self.descriptors()
            .find(|descriptor| descriptor.name == name)
    }

    /// Attributes keyed by `K` and their names, in list order.
    pub fn iter<K: AttributeKey>(&self) -> impl Iterator<Item = (&str, &AttributeMap<K>)> {
        K::list(self)
            .iter()
            .map(|(name, values)| (name.as_str(), values))
    }

    /// The named attribute keyed by `K`, or `None` if there isn't one.
    pub fn get<K: AttributeKey>(&self, name: &str) -> Option<&AttributeMap<K>> {
        // Meshes typically have few attributes, so a linear scan is as fast as hashing.
        K::list(self)
            .iter()
            .find(|(attribute, _)| attribute == name)
            .map(|(_, values)| values)
    }

    /// The values of the attribute called `name`.
    ///
    /// # Errors
    /// If there's no attribute called `name` keyed by `K`, or it holds a different type than `T`.
    pub fn values<K: AttributeKey, T: AttributeValue>(
        &self,
        name: &str,
    ) -> Result<&SecondaryMap<K, T>, AttributeError> {
        self.get::<K>(name)
            .ok_or_else(|| AttributeError::NotFound { name: name.into() })?
            .typed()
            .ok_or_else(|| AttributeError::WrongType { name: name.into() })
    }

    /// Like [`Self::values`]. Crate-only, since writes have side effects; see the edits on
    /// [`crate::Mesh`].
    pub(crate) fn values_mut<K: AttributeKey, T: AttributeValue>(
        &mut self,
        name: &str,
    ) -> Result<&mut SecondaryMap<K, T>, AttributeError> {
        K::list_mut(self)
            .iter_mut()
            .find(|(attribute, _)| attribute == name)
            .ok_or_else(|| AttributeError::NotFound { name: name.into() })?
            .1
            .typed_mut()
            .ok_or_else(|| AttributeError::WrongType { name: name.into() })
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

    /// Adds an attribute. The name must not be used by any attribute.
    pub(crate) fn add<K: AttributeKey>(&mut self, name: String, values: AttributeMap<K>) {
        debug_assert!(
            self.descriptor(&name).is_none(),
            "duplicate attribute {name:?}"
        );
        K::list_mut(self).push((name, values));
    }

    /// Removes the attribute called `name`, in any domain. `false` if there isn't one.
    pub(crate) fn remove(&mut self, name: &str) -> bool {
        remove_named(&mut self.vertex, name)
            || remove_named(&mut self.edge, name)
            || remove_named(&mut self.face, name)
            || remove_named(&mut self.corner, name)
    }

    fn descriptors_of<K: AttributeKey>(&self) -> impl Iterator<Item = AttributeDescriptor<'_>> {
        self.iter::<K>().map(|(name, values)| AttributeDescriptor {
            name,
            domain: K::DOMAIN,
            value_type: values.value_type(),
        })
    }
}

fn remove_named<K: Key>(list: &mut Vec<(String, AttributeMap<K>)>, name: &str) -> bool {
    let count = list.len();
    list.retain(|(attribute, _)| attribute != name);
    list.len() != count
}

const BUILT_IN: &str = "built-in attributes always exist with their type";

/// Why an attribute couldn't be read, written, added or removed as requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeError {
    /// No attribute with this name in the requested domain.
    NotFound { name: String },
    /// The attribute stores a different value type than the one requested.
    WrongType { name: String },
    /// An attribute with this name already exists.
    AlreadyExists { name: String },
    /// The name is one of [`Attributes::RESERVED_NAMES`], so it can't be added or removed.
    Reserved { name: String },
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
            Self::AlreadyExists { name } => write!(formatter, "attribute {name:?} already exists"),
            Self::Reserved { name } => {
                write!(formatter, "attribute name {name:?} is reserved")
            }
        }
    }
}

impl std::error::Error for AttributeError {}

/// Implements [`AttributeKey`] for each `key type => domain, field, keys`, where `field` is the
/// [`Attributes`] list holding the key type's attributes and `keys` the [`Topology`] method listing
/// its components. See macro use below.
macro_rules! impl_attribute_key {
    ($($key:ty => $domain:ident, $field:ident, $keys:ident);* $(;)?) => {$(
        impl sealed::SealedKey for $key {
            fn list(attributes: &Attributes) -> &Vec<(String, AttributeMap<Self>)> {
                &attributes.$field
            }

            fn list_mut(attributes: &mut Attributes) -> &mut Vec<(String, AttributeMap<Self>)> {
                &mut attributes.$field
            }

            fn keys(topology: &Topology) -> impl Iterator<Item = Self> + '_ {
                topology.$keys()
            }
        }

        impl AttributeKey for $key {
            const DOMAIN: AttributeDomain = AttributeDomain::$domain;
        }
    )*};
}

impl_attribute_key!(
    VertKey => Vertex, vertex, verts;
    EdgeKey => Edge, edge, edges;
    FaceKey => Face, face, faces;
    LoopKey => Corner, corner, loops;
);

#[cfg(test)]
#[path = "../tests/unit/attributes.rs"]
mod tests;
