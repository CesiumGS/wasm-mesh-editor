//! Vertex component and its composed view.

use glam::Vec3;

use crate::{Attributes, EdgeKey, FaceKey, LoopKey, Topology, VertKey};

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
        self.topo.vert_edges(self.key)
    }

    /// The incident faces, each yielded once, including non-manifold fans.
    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        self.topo.vert_faces(self.key)
    }

    /// The face corners at this vertex, each yielded once.
    pub(crate) fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        self.topo.vert_loops(self.key)
    }

    /// The vertices connected to this vertex by an edge, including wire edges.
    pub fn neighbors(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.topo.vert_neighbors(self.key)
    }

    pub fn position(&self) -> Vec3 {
        todo!()
    }
}
