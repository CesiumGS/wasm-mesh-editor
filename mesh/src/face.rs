//! Face component and its composed view.

use glam::{DVec3, Vec2, Vec3};

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

    /// One stored shading normal per corner, in the same winding order as [`Self::verts`].
    /// These may differ from the geometric face normal returned by [`Self::normal`].
    pub fn corner_normals(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.loops().map(|key| self.attrs.normals[key])
    }

    /// The stored shading normal at this vertex's corner, or `None` if it is not in the face.
    pub fn corner_normal(&self, vertex: VertKey) -> Option<Vec3> {
        let key = self.loops().find(|&key| self.topo.loops[key].vert == vertex)?;
        Some(self.attrs.normals[key])
    }

    /// One UV per corner, in the same winding order as [`Self::verts`].
    /// Missing UVs yield `None` without skipping corners.
    pub fn corner_uvs(&self) -> impl Iterator<Item = Option<Vec2>> + '_ {
        self.loops().map(|key| self.attrs.uvs.get(key).copied())
    }

    /// The UV at this vertex's corner, or `None` if it is not in the face or has no UV.
    pub fn corner_uv(&self, vertex: VertKey) -> Option<Vec2> {
        let key = self.loops().find(|&key| self.topo.loops[key].vert == vertex)?;
        self.attrs.uvs.get(key).copied()
    }

    pub(crate) fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        self.topo.face_loops(self.key)
    }

    /// A triangle fan anchored at the first vertex, preserving boundary winding.
    /// Not guaranteed to triangulate concave polygons correctly.
    pub fn triangulation(&self) -> impl Iterator<Item = [VertKey; 3]> + '_ {
        let mut vertices = self.verts();
        let anchor = vertices.next().unwrap();
        let mut previous = vertices.next().unwrap();
        vertices.map(move |next| {
            let triangle = [anchor, previous, next];
            previous = next;
            triangle
        })
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
