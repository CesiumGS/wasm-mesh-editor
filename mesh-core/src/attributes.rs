//! Struct-of-Arrays geometry attribute storage.

use glam::{Vec2, Vec3};
use slotmap::SecondaryMap;

use crate::{LoopKey, VertKey};

/// The domain a geometry attribute is defined over.
pub enum AttributeDomain {
    Vertex,
    Loop,
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
