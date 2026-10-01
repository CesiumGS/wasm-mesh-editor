//! Vertex component and its composed view.

use glam::Vec3;

use crate::{Attributes, EdgeKey, EdgeRef, FaceKey, Topology, VertKey};

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
        let first = self.topo.verts[self.key].edge;
        let mut current = first;
        std::iter::from_fn(move || {
            let key = current?;
            let edge = &self.topo.edges[key];
            let side = usize::from(edge.verts[1] == self.key);
            let next = edge.disk_next[side];
            current = (Some(next) != first).then_some(next);
            Some(key)
        })
    }

    /// The incident faces, each yielded once, including non-manifold fans.
    /// Each face has exactly one outgoing loop at this vertex, so filtering on
    /// the loop's vertex avoids allocating a set to deduplicate faces.
    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        self.edges()
            .flat_map(|key| {
                EdgeRef {
                    topo: self.topo,
                    key,
                }
                .loops()
            })
            .filter_map(|key| {
                let loop_ = &self.topo.loops[key];
                (loop_.vert == self.key).then_some(loop_.face)
            })
    }

    /// The vertices connected to this vertex by an edge, including wire edges.
    pub fn neighbors(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.edges().map(|key| {
            let verts = self.topo.edges[key].verts;
            verts[usize::from(verts[0] == self.key)]
        })
    }

    pub fn position(&self) -> Vec3 {
        todo!()
    }
}
