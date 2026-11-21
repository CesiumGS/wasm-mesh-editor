//! Edge component and its view.

use crate::{AttributeValue, Attributes, EdgeKey, FaceKey, LoopKey, Topology, VertKey};

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
    pub(crate) attrs: &'a Attributes,
    pub(crate) key: EdgeKey,
}

impl<'a> EdgeRef<'a> {
    pub fn key(&self) -> EdgeKey {
        self.key
    }

    pub fn verts(&self) -> [VertKey; 2] {
        self.topo.edge_verts(self.key)
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + use<'a> {
        self.topo.edge_faces(self.key)
    }

    /// The loops in this edge's radial cycle, one per incident face.
    pub(crate) fn loops(&self) -> impl Iterator<Item = LoopKey> + use<'a> {
        self.topo.edge_loops(self.key)
    }

    /// This edge's value of the named custom edge attribute, or `None` if the attribute doesn't
    /// exist or stores a different type.
    pub fn attribute<T: AttributeValue>(&self, name: &str) -> Option<T> {
        let values = self.attrs.custom_attribute::<EdgeKey>(name)?.typed::<T>()?;
        values.get(self.key).copied()
    }
}
