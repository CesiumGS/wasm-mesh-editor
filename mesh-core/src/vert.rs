//! Vertex component and its composed view.

use glam::Vec3;

use crate::{Attributes, EdgeKey, FaceKey, Topology, VertKey};

/// A vertex. Its position lives in attribute storage, keyed by [`VertKey`].
pub struct Vert {
    /// An edge in this vertex's disk cycle; `None` if isolated.
    pub edge: Option<EdgeKey>,
}

/// A vertex view composing connectivity with geometry.
pub struct VertRef<'a> {
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
    pub(crate) key: VertKey,
}

impl<'a> VertRef<'a> {
    pub fn key(&self) -> VertKey {
        self.key
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        std::iter::empty::<EdgeKey>()
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }

    pub fn neighbors(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn position(&self) -> Vec3 {
        todo!()
    }
}
