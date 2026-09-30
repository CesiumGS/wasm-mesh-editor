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
pub struct Attributes {
    pub positions: SecondaryMap<VertKey, Vec3>,
    pub normals: SecondaryMap<LoopKey, Vec3>,
    pub uvs: SecondaryMap<LoopKey, Vec2>,
}
