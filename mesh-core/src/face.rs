//! Face component and its composed view.

use glam::{DVec3, Vec3};

use crate::geometry::polygon_normal;
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

    /// The face's vertices in boundary winding order.
    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.topo.face_verts(self.key)
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        self.topo.face_edges(self.key)
    }

    pub(crate) fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        self.topo.face_loops(self.key)
    }

    pub fn triangulate(&self) -> impl Iterator<Item = [VertKey; 3]> + '_ {
        std::iter::empty::<[VertKey; 3]>()
    }

    /// The face's geometric normal (distinct from the per-loop shading normals).
    /// Computed from current positions without changing stored shading normals.
    /// Returns zero for a zero-area face.
    pub fn normal(&self) -> Vec3 {
        polygon_normal(self.verts().map(|key| self.attrs.positions[key].as_dvec3()))
            .unwrap_or(DVec3::ZERO)
            .as_vec3()
    }
}
