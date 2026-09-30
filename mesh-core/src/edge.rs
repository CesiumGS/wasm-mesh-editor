//! Edge component and its view.

use crate::{EdgeKey, FaceKey, LoopKey, Topology, VertKey};

/// An edge between two vertices.
pub struct Edge {
    pub verts: [VertKey; 2],
    /// Disk-cycle neighbors around `verts[i]`.
    pub disk_next: [EdgeKey; 2],
    pub disk_prev: [EdgeKey; 2],
    /// A loop in this edge's radial cycle; `None` if a wire edge.
    pub loop_: Option<LoopKey>,
}

/// An edge view.
pub struct EdgeRef<'a> {
    pub(crate) topo: &'a Topology,
    pub(crate) key: EdgeKey,
}

impl<'a> EdgeRef<'a> {
    pub fn key(&self) -> EdgeKey {
        self.key
    }

    pub fn verts(&self) -> [VertKey; 2] {
        todo!()
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }
}
