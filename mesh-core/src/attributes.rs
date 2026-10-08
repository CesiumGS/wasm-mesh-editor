//! Struct-of-Arrays geometry attribute storage.

use glam::{Vec2, Vec3};
use slotmap::SecondaryMap;

use crate::{LoopKey, VertKey};

/// The domain a geometry attribute is defined over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeDomain {
    Vertex,
    Loop,
}

/// A built-in geometry attribute, independent of its storage domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeId {
    Position,
    Normal,
    Uv,
}

impl AttributeId {
    pub fn domain(self) -> AttributeDomain {
        match self {
            Self::Position => AttributeDomain::Vertex,
            Self::Normal | Self::Uv => AttributeDomain::Loop,
        }
    }
}

/// Typed, borrowed storage selected by an [`AttributeId`].
pub enum AttributeRef<'a> {
    Position(&'a SecondaryMap<VertKey, Vec3>),
    Normal(&'a SecondaryMap<LoopKey, Vec3>),
    Uv(&'a SecondaryMap<LoopKey, Vec2>),
}

/// Geometry attributes, each keyed by the handle of its domain.
/// (In the future: we may want to restructure this to support user-defined attributes)
#[derive(Default)]
pub struct Attributes {
    pub positions: SecondaryMap<VertKey, Vec3>,
    pub normals: SecondaryMap<LoopKey, Vec3>,
    pub uvs: SecondaryMap<LoopKey, Vec2>,
}

impl Attributes {
    /// Borrows an attribute's storage, which may be empty when no values are present.
    pub fn get(&self, attribute: AttributeId) -> AttributeRef<'_> {
        match attribute {
            AttributeId::Position => AttributeRef::Position(&self.positions),
            AttributeId::Normal => AttributeRef::Normal(&self.normals),
            AttributeId::Uv => AttributeRef::Uv(&self.uvs),
        }
    }

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
        }
    }
}
