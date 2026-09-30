//! Face component and its composed view.

use glam::Vec3;

use crate::{Attributes, EdgeKey, FaceKey, LoopKey, Topology, VertKey};

/// A face (n-gon), reached from an entry loop.
pub struct Face {
    pub loop_: LoopKey,
    pub len: u32,
}

/// A face view composing connectivity with geometry.
pub struct FaceRef<'a> {
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
    pub(crate) key: FaceKey,
}

impl<'a> FaceRef<'a> {
    pub fn key(&self) -> FaceKey {
        self.key
    }

    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        std::iter::empty::<EdgeKey>()
    }

    pub fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        std::iter::empty::<LoopKey>()
    }

    pub fn triangulate(&self) -> impl Iterator<Item = [VertKey; 3]> + '_ {
        std::iter::empty::<[VertKey; 3]>()
    }

    /// The face's geometric normal (distinct from the per-loop shading normals).
    pub fn normal(&self) -> Vec3 {
        todo!()
    }
}
